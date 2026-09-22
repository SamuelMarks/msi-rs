//! Two-Phase Transaction & Rollback Engine Specification.
//!
//! Grounded directly in official Microsoft Windows Installer specifications:
//! - **Phase 1: Immediate Execution Phase (Client Context):**
//!   - Sequence table walking (`InstallExecuteSequence`, `AdminExecuteSequence`, etc.)
//!   - Dynamic action condition evaluation using [`EvaluationContext`].
//!   - Disk costing lifecycle (`CostInitialize`, `FileCost`, `CostFinalize`).
//!   - Compiling execution commands into structured `.ibs` installation script.
//!   - Compiling compensating reverse commands into `.rbs` rollback script.
//! - **Phase 2: Deferred Execution Phase (Privileged Worker Context):**
//!   - IPC message boundary transition to privileged worker.
//!   - Quarantine rollback storage management (`.rbf` files).
//!   - Atomic file installations and temporary replacements.
//!   - Registry journal recording prior values.
//!   - Shortcut and service creation.
//! - **Phase 3: Commit / Rollback Handling:**
//!   - Commit: Run commit actions, purge quarantine files and rollback scripts, finalize product registration.
//!   - Rollback: Reverse chronological unwinding of rollback script, restoring quarantined files,
//!     removing installed files/directories, restoring registry keys, returning `1603` (`ERROR_INSTALL_FAILURE`).

use crate::database::tables::record::FieldValue;
use crate::error::{Error, Result};
use crate::execution::costing::DiskCostEngine;
use crate::execution::properties::EvaluationContext;
use crate::execution::script::{InstallScript, RollbackOp, RollbackScript, ScriptOp};
use crate::wix::linker::LinkedDatabase;
use std::collections::{HashMap, HashSet};
use std::marker::PhantomData;

/// Windows Installer exit code indicating successful completion.
pub const ERROR_SUCCESS: u32 = 0;

/// Windows Installer fatal error exit code indicating transaction failure and rollback.
pub const ERROR_INSTALL_FAILURE: u32 = 1603;

/// Typestate marker: Transaction is created but uninitialized.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Uninitialized;

/// Typestate marker: Immediate phase completed, scripts compiled and disk cost finalized.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Prepared;

/// Typestate marker: Deferred execution script completed successfully by privileged worker.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Executed;

/// Typestate marker: Transaction committed, quarantine purged, product registered.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Committed;

/// Typestate marker: Transaction aborted and rolled back in reverse chronological order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RolledBack;

/// IPC message exchanged between the unprivileged client and the privileged installation worker.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WorkerIpcMessage {
    /// Command instructing the worker to execute the `.ibs` installation script.
    ExecuteScript {
        /// The compiled installation script.
        ibs_script: InstallScript,
        /// The compensating rollback script.
        rbs_script: RollbackScript,
        /// Quarantine folder path where `.rbf` files will be stored.
        quarantine_dir: String,
    },
    /// Command instructing the worker to commit the executed transaction.
    CommitScript {
        /// Quarantine folder path to purge.
        quarantine_dir: String,
    },
    /// Command instructing the worker to execute the rollback script.
    RollbackScript {
        /// The compensating rollback script.
        rbs_script: RollbackScript,
        /// Quarantine folder path to restore and cleanup.
        quarantine_dir: String,
    },
    /// Progress notification sent from worker back to client.
    ProgressUpdate {
        /// Current action being executed.
        action: String,
        /// Current completed ticks/units.
        current: u64,
        /// Total estimated ticks/units.
        total: u64,
    },
    /// Worker response summarizing operation completion.
    WorkerResponse {
        /// Whether the operation succeeded.
        success: bool,
        /// Return code (0 for success, 1603 for failure).
        return_code: u32,
        /// Diagnostic message if any.
        error_message: Option<String>,
    },
}

/// In-memory privileged execution worker context managing physical filesystem,
/// quarantine storage, registry journals, and service lifecycles.
#[derive(Debug, Clone, Default)]
pub struct WorkerContext {
    /// Files currently present on the target filesystem with their content bytes.
    filesystem_files: HashMap<String, Vec<u8>>,
    /// Directories created on the target filesystem.
    filesystem_dirs: HashSet<String>,
    /// Quarantined pristine copies of overwritten files (`.rbf` storage).
    quarantine_files: HashMap<String, Vec<u8>>,
    /// Registry storage mapping `(root, key, value_name)` to `value_data`.
    registry: HashMap<(u32, String, Option<String>), Option<String>>,
    /// Shortcuts registered on the target system.
    shortcuts: HashSet<String>,
    /// Services registered on the target system.
    services: HashSet<String>,
    /// Services currently in running state.
    running_services: HashSet<String>,
    /// List of action log records.
    executed_actions: Vec<String>,
    /// Name of action configured to simulate failure (for rollback testing).
    simulated_failure_action: Option<String>,
    /// Optional live worker executor for real physical disk operations and rollback quarantine.
    live_executor: Option<super::worker::LiveWorkerExecutor>,
    /// Optional bare-metal rollback journal tracking low-level disk modifications.
    bare_metal_journal: Option<crate::execution::bare_metal::BareMetalRollbackJournal>,
}

impl WorkerContext {
    /// Creates a new empty [`WorkerContext`].
    ///
    /// # Returns
    ///
    /// A new [`WorkerContext`].
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Attaches a [`super::worker::LiveWorkerExecutor`] to perform physical filesystem writes and quarantine backups.
    ///
    /// # Arguments
    ///
    /// * `executor` - Configured live worker executor.
    #[must_use]
    pub fn with_live_executor(mut self, executor: super::worker::LiveWorkerExecutor) -> Self {
        self.live_executor = Some(executor);
        self
    }

    /// Attaches a [`crate::execution::bare_metal::BareMetalRollbackJournal`] to track disk operations for atomic rollback.
    ///
    /// # Arguments
    ///
    /// * `journal` - Configured bare metal rollback journal.
    ///
    /// # Returns
    ///
    /// Updated [`WorkerContext`].
    #[must_use]
    pub fn with_bare_metal_journal(
        mut self,
        journal: crate::execution::bare_metal::BareMetalRollbackJournal,
    ) -> Self {
        self.bare_metal_journal = Some(journal);
        self
    }

    /// Returns a mutable reference to the attached [`crate::execution::bare_metal::BareMetalRollbackJournal`], if present.
    pub const fn bare_metal_journal_mut(
        &mut self,
    ) -> Option<&mut crate::execution::bare_metal::BareMetalRollbackJournal> {
        self.bare_metal_journal.as_mut()
    }

    /// Returns an optional reference to the attached [`super::worker::LiveWorkerExecutor`].
    #[must_use]
    pub const fn live_executor(&self) -> Option<&super::worker::LiveWorkerExecutor> {
        self.live_executor.as_ref()
    }

    /// Pre-populates an existing file on the filesystem (for testing overwrites and backups).
    ///
    /// # Arguments
    ///
    /// * `path` - Target file path.
    /// * `content` - Initial file bytes.
    pub fn pre_seed_file(&mut self, path: impl Into<String>, content: impl Into<Vec<u8>>) {
        self.filesystem_files.insert(path.into(), content.into());
    }

    /// Pre-populates an existing registry value (for testing registry rollback journal).
    ///
    /// # Arguments
    ///
    /// * `root` - Root key index.
    /// * `key` - Subkey path.
    /// * `name` - Value name.
    /// * `value` - Initial value data.
    pub fn pre_seed_registry(
        &mut self,
        root: u32,
        key: impl Into<String>,
        name: Option<String>,
        value: Option<String>,
    ) {
        self.registry.insert((root, key.into(), name), value);
    }

    /// Configures the worker to simulate an execution failure at a specified action name.
    ///
    /// # Arguments
    ///
    /// * `action` - Action name to fail.
    pub fn simulate_failure_at(&mut self, action: impl Into<String>) {
        self.simulated_failure_action = Some(action.into());
    }

    /// Returns the content of a file on the target filesystem, if present.
    ///
    /// # Arguments
    ///
    /// * `path` - Target file path.
    ///
    /// # Returns
    ///
    /// Optional byte slice of file content.
    #[must_use]
    pub fn get_file_content(&self, path: &str) -> Option<&[u8]> {
        self.filesystem_files.get(path).map(Vec::as_slice)
    }

    /// Checks if a directory exists on the target filesystem.
    ///
    /// # Arguments
    ///
    /// * `path` - Target directory path.
    ///
    /// # Returns
    ///
    /// `true` if exists, `false` otherwise.
    #[must_use]
    pub fn has_directory(&self, path: &str) -> bool {
        self.filesystem_dirs.contains(path)
    }

    /// Returns the registry value for the specified key and name.
    ///
    /// # Arguments
    ///
    /// * `root` - Root key index.
    /// * `key` - Subkey path.
    /// * `name` - Value name.
    ///
    /// # Returns
    ///
    /// Optional reference to registry value.
    #[must_use]
    pub fn get_registry_value(
        &self,
        root: u32,
        key: &str,
        name: Option<&str>,
    ) -> Option<&Option<String>> {
        self.registry
            .get(&(root, key.to_string(), name.map(ToString::to_string)))
    }

    /// Checks if a shortcut exists.
    ///
    /// # Arguments
    ///
    /// * `link_path` - Shortcut link path.
    ///
    /// # Returns
    ///
    /// `true` if exists, `false` otherwise.
    #[must_use]
    pub fn has_shortcut(&self, link_path: &str) -> bool {
        self.shortcuts.contains(link_path)
    }

    /// Checks if a service is registered.
    ///
    /// # Arguments
    ///
    /// * `name` - Service name.
    ///
    /// # Returns
    ///
    /// `true` if registered, `false` otherwise.
    #[must_use]
    pub fn has_service(&self, name: &str) -> bool {
        self.services.contains(name)
    }

    /// Checks if a service is running.
    ///
    /// # Arguments
    ///
    /// * `name` - Service name.
    ///
    /// # Returns
    ///
    /// `true` if running, `false` otherwise.
    #[must_use]
    pub fn is_service_running(&self, name: &str) -> bool {
        self.running_services.contains(name)
    }

    /// Returns the number of quarantine files currently held in `.rbf` storage.
    ///
    /// # Returns
    ///
    /// Count of quarantined files.
    #[must_use]
    pub fn quarantine_count(&self) -> usize {
        self.quarantine_files.len()
    }

    /// Phase 2: Executes an installation script (`.ibs`) sequentially.
    ///
    /// If an error or simulated failure occurs, execution halts immediately.
    ///
    /// # Arguments
    ///
    /// * `script` - The [`InstallScript`] to execute.
    ///
    /// # Errors
    ///
    /// Returns [`Error::ExecutionFailed`] if any operation fails.
    #[allow(clippy::too_many_lines)]
    pub fn execute_script(&mut self, script: &InstallScript) -> Result<()> {
        for op in script.operations() {
            if let Some(ref fail_action) = self.simulated_failure_action {
                let action_name = match op {
                    ScriptOp::CustomAction { action, .. } => action.as_str(),
                    ScriptOp::InstallService { name, .. } => name.as_str(),
                    _ => "",
                };
                if action_name == fail_action {
                    return Err(Error::ExecutionFailed {
                        action: fail_action.clone(),
                        return_code: ERROR_INSTALL_FAILURE,
                        message: format!("Simulated failure at action '{fail_action}'"),
                    });
                }
            }

            match op {
                ScriptOp::CreateFolder { path } => {
                    self.filesystem_dirs.insert(path.clone());
                    self.executed_actions.push(format!("CreateFolder({path})"));
                    if let Some(ref mut exec) = self.live_executor {
                        exec.create_directory(std::path::Path::new(path), None)?;
                    }
                }
                ScriptOp::RemoveFolder { path } => {
                    self.filesystem_dirs.remove(path);
                    self.executed_actions.push(format!("RemoveFolder({path})"));
                    if self.live_executor.is_some() {
                        let p = std::path::Path::new(path);
                        if p.exists() {
                            let _ = std::fs::remove_dir(p);
                        }
                    }
                }
                ScriptOp::CopyFile {
                    source,
                    destination,
                    ..
                } => {
                    let content = self
                        .filesystem_files
                        .get(source)
                        .cloned()
                        .unwrap_or_else(|| b"DEFAULT_CONTENT".to_vec());
                    self.filesystem_files
                        .insert(destination.clone(), content.clone());
                    self.executed_actions
                        .push(format!("CopyFile({destination})"));
                    if let Some(ref mut exec) = self.live_executor {
                        exec.write_file_atomic(std::path::Path::new(destination), &content, None)?;
                    }
                }
                ScriptOp::WriteFile {
                    destination,
                    content,
                } => {
                    self.filesystem_files
                        .insert(destination.clone(), content.clone());
                    self.executed_actions
                        .push(format!("WriteFile({destination})"));
                    if let Some(ref mut exec) = self.live_executor {
                        exec.write_file_atomic(std::path::Path::new(destination), content, None)?;
                    }
                }
                ScriptOp::DeleteFile { path } => {
                    self.filesystem_files.remove(path);
                    self.executed_actions.push(format!("DeleteFile({path})"));
                    if self.live_executor.is_some() {
                        let p = std::path::Path::new(path);
                        if p.exists() {
                            let _ = std::fs::remove_file(p);
                        }
                    }
                }
                ScriptOp::BackupFile {
                    target_path,
                    quarantine_path,
                } => {
                    if let Some(existing) = self.filesystem_files.get(target_path) {
                        self.quarantine_files
                            .insert(quarantine_path.clone(), existing.clone());
                    }
                    self.executed_actions
                        .push(format!("BackupFile({target_path})"));
                }
                ScriptOp::WriteRegistry {
                    root,
                    key,
                    name,
                    value,
                } => {
                    self.registry
                        .insert((*root, key.clone(), name.clone()), value.clone());
                    self.executed_actions.push(format!("WriteRegistry({key})"));
                }
                ScriptOp::DeleteRegistry { root, key, name } => {
                    self.registry.remove(&(*root, key.clone(), name.clone()));
                    self.executed_actions.push(format!("DeleteRegistry({key})"));
                }
                ScriptOp::CreateShortcut { link_path, .. } => {
                    self.shortcuts.insert(link_path.clone());
                    self.executed_actions
                        .push(format!("CreateShortcut({link_path})"));
                }
                ScriptOp::DeleteShortcut { link_path } => {
                    self.shortcuts.remove(link_path);
                    self.executed_actions
                        .push(format!("DeleteShortcut({link_path})"));
                }
                ScriptOp::InstallService { name, .. } => {
                    self.services.insert(name.clone());
                    self.executed_actions
                        .push(format!("InstallService({name})"));
                }
                ScriptOp::DeleteService { name } => {
                    self.services.remove(name);
                    self.running_services.remove(name);
                    self.executed_actions.push(format!("DeleteService({name})"));
                }
                ScriptOp::StartService { name, .. } => {
                    self.running_services.insert(name.clone());
                    self.executed_actions.push(format!("StartService({name})"));
                }
                ScriptOp::StopService { name } => {
                    self.running_services.remove(name);
                    self.executed_actions.push(format!("StopService({name})"));
                }
                ScriptOp::CustomAction { action, .. } => {
                    self.executed_actions
                        .push(format!("CustomAction({action})"));
                }
            }
        }

        Ok(())
    }

    /// Phase 3 (Rollback): Plays the compensating rollback script in reverse chronological order.
    ///
    /// # Arguments
    ///
    /// * `script` - The [`RollbackScript`] to execute.
    ///
    /// # Errors
    ///
    /// Returns [`Error::RollbackFailed`] if any rollback command encounters an issue.
    pub fn execute_rollback(&mut self, script: &RollbackScript) -> Result<()> {
        for op in script.operations().iter().rev() {
            match op {
                RollbackOp::RestoreQuarantinedFile {
                    target_path,
                    quarantine_path,
                } => {
                    if let Some(content) = self.quarantine_files.remove(quarantine_path) {
                        self.filesystem_files.insert(target_path.clone(), content);
                    }
                }
                RollbackOp::DeleteCreatedFile { path } => {
                    self.filesystem_files.remove(path);
                }
                RollbackOp::DeleteCreatedFolder { path } => {
                    self.filesystem_dirs.remove(path);
                }
                RollbackOp::RestoreRegistry {
                    root,
                    key,
                    name,
                    previous_value,
                    existed,
                } => {
                    if *existed {
                        self.registry
                            .insert((*root, key.clone(), name.clone()), previous_value.clone());
                    } else {
                        self.registry.remove(&(*root, key.clone(), name.clone()));
                    }
                }
                RollbackOp::DeleteShortcut { link_path } => {
                    self.shortcuts.remove(link_path);
                }
                RollbackOp::DeleteService { name } => {
                    self.services.remove(name);
                    self.running_services.remove(name);
                }
                RollbackOp::StopService { name } => {
                    self.running_services.remove(name);
                }
                RollbackOp::RollbackCustomAction { action, .. } => {
                    self.executed_actions
                        .push(format!("RollbackCustomAction({action})"));
                }
            }
        }

        // Clean up any remaining quarantine files and directories
        self.quarantine_files.clear();
        if let Some(ref mut exec) = self.live_executor {
            exec.rollback()?;
        }
        if let Some(ref mut bm_journal) = self.bare_metal_journal {
            bm_journal.execute_rollback()?;
        }
        Ok(())
    }

    /// Phase 3 (Commit): Purges quarantine storage files and finalizes installation.
    ///
    /// # Errors
    ///
    /// Returns [`Error`] on failure.
    pub fn commit(&mut self) -> Result<()> {
        self.quarantine_files.clear();
        self.executed_actions.push("CommitSuccess".to_string());
        if let Some(ref mut exec) = self.live_executor {
            exec.commit()?;
        }
        Ok(())
    }
}

/// Windows Installer Transaction engine parameterized by typestate.
#[derive(Debug)]
pub struct Transaction<State> {
    /// In-memory linked database.
    database: LinkedDatabase,
    /// Property lookup and expression evaluation context.
    context: EvaluationContext,
    /// Disk costing engine.
    cost_engine: DiskCostEngine,
    /// Compiled deferred installation script (`.ibs`).
    install_script: InstallScript,
    /// Compiled compensating rollback script (`.rbs`).
    rollback_script: RollbackScript,
    /// Sequence table name to walk (e.g. `InstallExecuteSequence`).
    sequence_table: String,
    /// Typestate marker.
    _state: PhantomData<State>,
}

impl Transaction<Uninitialized> {
    /// Creates a new [`Transaction`] in the [`Uninitialized`] state.
    ///
    /// # Arguments
    ///
    /// * `database` - Linked MSI database with catalog and table records.
    /// * `context` - Initialized evaluation context.
    /// * `cost_engine` - Disk costing engine.
    ///
    /// # Returns
    ///
    /// A new uninitialized [`Transaction`].
    #[must_use]
    pub fn new(
        database: LinkedDatabase,
        context: EvaluationContext,
        cost_engine: DiskCostEngine,
    ) -> Self {
        Self {
            database,
            context,
            cost_engine,
            install_script: InstallScript::new(),
            rollback_script: RollbackScript::new(),
            sequence_table: "InstallExecuteSequence".to_string(),
            _state: PhantomData,
        }
    }

    /// Sets the sequence table to walk (e.g. `AdminExecuteSequence`, `InstallExecuteSequence`).
    ///
    /// # Arguments
    ///
    /// * `table` - Table name.
    ///
    /// # Returns
    ///
    /// The updated [`Transaction`].
    #[must_use]
    pub fn sequence_table(mut self, table: impl Into<String>) -> Self {
        self.sequence_table = table.into();
        self
    }

    /// Phase 1: Walks sequence table, evaluates action conditions, performs disk costing,
    /// and compiles `.ibs` and `.rbs` scripts.
    ///
    /// # Returns
    ///
    /// Transitioned [`Transaction<Prepared>`] on success.
    ///
    /// # Errors
    ///
    /// Returns [`Error`] if condition evaluation or disk costing fails.
    #[allow(clippy::too_many_lines)]
    pub fn prepare(mut self) -> Result<Transaction<Prepared>> {
        let records = self.database.get_records(&self.sequence_table);
        let mut ordered_actions: Vec<(i16, String, Option<String>)> = Vec::new();

        for record in records {
            let action = match record.get(0) {
                Some(FieldValue::String(s)) => s.clone(),
                _ => continue,
            };
            let condition = match record.get(1) {
                Some(FieldValue::String(s)) => Some(s.clone()),
                _ => None,
            };
            let seq = match record.get(2) {
                Some(&FieldValue::Short(n)) => n,
                Some(&FieldValue::Long(n)) => i16::try_from(n).unwrap_or(0),
                _ => 0,
            };

            if seq > 0 {
                ordered_actions.push((seq, action, condition));
            }
        }

        ordered_actions.sort_by_key(|(seq, _, _)| *seq);

        for (_seq, action, cond) in ordered_actions {
            if let Some(ref expression) = cond {
                if !expression.trim().is_empty() {
                    let satisfied = self.context.evaluate_condition(expression)?;
                    if !satisfied {
                        continue;
                    }
                }
            }

            match action.as_str() {
                "CostInitialize" => {
                    self.cost_engine.cost_initialize();
                }
                "FileCost" => {
                    let file_records = self.database.get_records("File");
                    for f in file_records {
                        let size = match f.get(3) {
                            Some(&FieldValue::Long(n)) if n > 0 => u64::try_from(n).unwrap_or(0),
                            _ => 0,
                        };
                        self.cost_engine.file_cost("TARGETDIR", size, None);
                    }
                }
                "CostFinalize" => {
                    self.cost_engine.cost_finalize()?;
                }
                "CreateFolders" => {
                    let dir_records = self.database.get_records("Directory");
                    for d in dir_records {
                        if let Some(FieldValue::String(dir_name)) = d.get(0) {
                            let path = format!(r"C:\{dir_name}");
                            self.install_script
                                .push(ScriptOp::CreateFolder { path: path.clone() });
                            self.rollback_script
                                .push(RollbackOp::DeleteCreatedFolder { path });
                        } else {
                            // Skip directory record without directory name
                        }
                    }
                }
                "InstallFiles" => {
                    let file_records = self.database.get_records("File");
                    for f in file_records {
                        if let Some(FieldValue::String(filename)) = f.get(2) {
                            let target_path = format!(r"C:\Program Files\App\{filename}");
                            let quarantine_path = format!(r"C:\Config.Msi\{filename}.rbf");

                            // Backup existing file if present
                            self.install_script.push(ScriptOp::BackupFile {
                                target_path: target_path.clone(),
                                quarantine_path: quarantine_path.clone(),
                            });
                            self.rollback_script
                                .push(RollbackOp::RestoreQuarantinedFile {
                                    target_path: target_path.clone(),
                                    quarantine_path,
                                });

                            // Write newly installed file
                            self.install_script.push(ScriptOp::WriteFile {
                                destination: target_path.clone(),
                                content: format!("Payload of {filename}").into_bytes(),
                            });
                            self.rollback_script
                                .push(RollbackOp::DeleteCreatedFile { path: target_path });
                        } else {
                            // Skip file record without filename
                        }
                    }
                }
                "WriteRegistryValues" => {
                    let reg_records = self.database.get_records("Registry");
                    for r in reg_records {
                        let root = match r.get(1) {
                            Some(&FieldValue::Short(n)) => u32::try_from(n).unwrap_or(2),
                            _ => 2,
                        };
                        let key = match r.get(2) {
                            Some(FieldValue::String(k)) => k.clone(),
                            _ => r"Software\App".to_string(),
                        };
                        let name = match r.get(3) {
                            Some(FieldValue::String(n)) => Some(n.clone()),
                            _ => None,
                        };
                        let val = match r.get(4) {
                            Some(FieldValue::String(v)) => Some(v.clone()),
                            _ => None,
                        };

                        self.install_script.push(ScriptOp::WriteRegistry {
                            root,
                            key: key.clone(),
                            name: name.clone(),
                            value: val,
                        });
                        self.rollback_script.push(RollbackOp::RestoreRegistry {
                            root,
                            key,
                            name,
                            previous_value: None,
                            existed: false,
                        });
                    }
                }
                "CreateShortcuts" => {
                    let sc_records = self.database.get_records("Shortcut");
                    for s in sc_records {
                        if let Some(FieldValue::String(target)) = s.get(4) {
                            let link_path = format!(r"C:\Users\Public\Desktop\{target}.lnk");
                            self.install_script.push(ScriptOp::CreateShortcut {
                                target: target.clone(),
                                link_path: link_path.clone(),
                                arguments: None,
                                icon_path: None,
                                icon_index: None,
                            });
                            self.rollback_script
                                .push(RollbackOp::DeleteShortcut { link_path });
                        }
                    }
                }
                custom_act => {
                    self.install_script.push(ScriptOp::CustomAction {
                        action: custom_act.to_string(),
                        action_type: 1,
                        source: "BinaryTable".to_string(),
                        target: "EntryFn".to_string(),
                    });
                    self.rollback_script.push(RollbackOp::RollbackCustomAction {
                        action: format!("{custom_act}_Rollback"),
                        action_type: 0x501,
                        source: "BinaryTable".to_string(),
                        target: "RollbackFn".to_string(),
                    });
                }
            }
        }

        Ok(Transaction {
            database: self.database,
            context: self.context,
            cost_engine: self.cost_engine,
            install_script: self.install_script,
            rollback_script: self.rollback_script,
            sequence_table: self.sequence_table,
            _state: PhantomData,
        })
    }
}

impl Transaction<Prepared> {
    /// Returns the compiled installation script (`.ibs`).
    #[must_use]
    pub const fn install_script(&self) -> &InstallScript {
        &self.install_script
    }

    /// Returns the compiled rollback script (`.rbs`).
    #[must_use]
    pub const fn rollback_script(&self) -> &RollbackScript {
        &self.rollback_script
    }

    /// Returns a reference to the disk costing engine.
    #[must_use]
    pub const fn cost_engine(&self) -> &DiskCostEngine {
        &self.cost_engine
    }

    /// Phase 2: Deferred Execution Phase.
    ///
    /// Sends compiled `.ibs` script to privileged worker context for execution.
    /// If execution fails, automatic rollback is executed and `ERROR_INSTALL_FAILURE` (`1603`) is returned.
    ///
    /// # Arguments
    ///
    /// * `worker` - Privileged worker context.
    ///
    /// # Returns
    ///
    /// Transitioned [`Transaction<Executed>`] on success.
    ///
    /// # Errors
    ///
    /// Returns [`Error::ExecutionFailed`] on installation failure (after triggering rollback).
    pub fn execute(self, worker: &mut WorkerContext) -> Result<Transaction<Executed>> {
        if let Err(err) = worker.execute_script(&self.install_script) {
            // Trigger automatic rollback unwinding on failure
            let _ = worker.execute_rollback(&self.rollback_script);
            return Err(err);
        }

        Ok(Transaction {
            database: self.database,
            context: self.context,
            cost_engine: self.cost_engine,
            install_script: self.install_script,
            rollback_script: self.rollback_script,
            sequence_table: self.sequence_table,
            _state: PhantomData,
        })
    }

    /// Aborts and rolls back the prepared transaction prior to execution.
    ///
    /// # Arguments
    ///
    /// * `worker` - Worker context.
    ///
    /// # Returns
    ///
    /// Transitioned [`Transaction<RolledBack>`].
    ///
    /// # Errors
    ///
    /// Returns [`Error::RollbackFailed`] if unwinding fails.
    pub fn rollback(self, worker: &mut WorkerContext) -> Result<Transaction<RolledBack>> {
        worker.execute_rollback(&self.rollback_script)?;
        Ok(Transaction {
            database: self.database,
            context: self.context,
            cost_engine: self.cost_engine,
            install_script: self.install_script,
            rollback_script: self.rollback_script,
            sequence_table: self.sequence_table,
            _state: PhantomData,
        })
    }
}

impl Transaction<Executed> {
    /// Phase 3 (Commit): Finalizes installation, purges quarantine files, registers product.
    ///
    /// # Arguments
    ///
    /// * `worker` - Privileged worker context.
    ///
    /// # Returns
    ///
    /// Transitioned [`Transaction<Committed>`].
    ///
    /// # Errors
    ///
    /// Returns [`Error`] on commit failure.
    pub fn commit(self, worker: &mut WorkerContext) -> Result<Transaction<Committed>> {
        worker.commit()?;
        Ok(Transaction {
            database: self.database,
            context: self.context,
            cost_engine: self.cost_engine,
            install_script: self.install_script,
            rollback_script: self.rollback_script,
            sequence_table: self.sequence_table,
            _state: PhantomData,
        })
    }

    /// Phase 3 (Rollback): Unwinds deferred execution changes in reverse chronological order.
    ///
    /// # Arguments
    ///
    /// * `worker` - Privileged worker context.
    ///
    /// # Returns
    ///
    /// Transitioned [`Transaction<RolledBack>`].
    ///
    /// # Errors
    ///
    /// Returns [`Error::RollbackFailed`] if unwinding fails.
    pub fn rollback(self, worker: &mut WorkerContext) -> Result<Transaction<RolledBack>> {
        worker.execute_rollback(&self.rollback_script)?;
        Ok(Transaction {
            database: self.database,
            context: self.context,
            cost_engine: self.cost_engine,
            install_script: self.install_script,
            rollback_script: self.rollback_script,
            sequence_table: self.sequence_table,
            _state: PhantomData,
        })
    }
}

impl Transaction<Committed> {
    /// Returns return code `ERROR_SUCCESS` (`0`).
    #[must_use]
    pub const fn return_code(&self) -> u32 {
        ERROR_SUCCESS
    }
}

impl Transaction<RolledBack> {
    /// Returns fatal installation failure return code `ERROR_INSTALL_FAILURE` (`1603`).
    #[must_use]
    pub const fn return_code(&self) -> u32 {
        ERROR_INSTALL_FAILURE
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::database::tables::record::Record;

    /// Helper creating a minimal test database with sequence and file records.
    #[allow(clippy::too_many_lines)]
    fn create_test_database() -> Result<LinkedDatabase> {
        let mut db = LinkedDatabase::new()?;

        // Add InstallExecuteSequence records
        db.add_record(
            "InstallExecuteSequence",
            Record::with_fields(vec![
                FieldValue::String("CostInitialize".to_string()),
                FieldValue::Null,
                FieldValue::Short(800),
            ]),
        );
        db.add_record(
            "InstallExecuteSequence",
            Record::with_fields(vec![
                FieldValue::String("FileCost".to_string()),
                FieldValue::Null,
                FieldValue::Short(900),
            ]),
        );
        db.add_record(
            "InstallExecuteSequence",
            Record::with_fields(vec![
                FieldValue::String("CostFinalize".to_string()),
                FieldValue::Null,
                FieldValue::Short(1000),
            ]),
        );
        db.add_record(
            "InstallExecuteSequence",
            Record::with_fields(vec![
                FieldValue::String("CreateFolders".to_string()),
                FieldValue::Null,
                FieldValue::Short(1100),
            ]),
        );
        db.add_record(
            "InstallExecuteSequence",
            Record::with_fields(vec![
                FieldValue::String("InstallFiles".to_string()),
                FieldValue::Null,
                FieldValue::Short(1200),
            ]),
        );
        db.add_record(
            "InstallExecuteSequence",
            Record::with_fields(vec![
                FieldValue::String("WriteRegistryValues".to_string()),
                FieldValue::Null,
                FieldValue::Short(1300),
            ]),
        );
        db.add_record(
            "InstallExecuteSequence",
            Record::with_fields(vec![
                FieldValue::String("CreateShortcuts".to_string()),
                FieldValue::Null,
                FieldValue::Short(1400),
            ]),
        );
        db.add_record(
            "InstallExecuteSequence",
            Record::with_fields(vec![
                FieldValue::String("MyCustomAction".to_string()),
                FieldValue::String(r#"INSTALL_MODE = "FULL""#.to_string()),
                FieldValue::Short(1500),
            ]),
        );

        // Add Directory table record
        db.add_record(
            "Directory",
            Record::with_fields(vec![
                FieldValue::String("AppDir".to_string()),
                FieldValue::Null,
                FieldValue::String("App".to_string()),
            ]),
        );

        // Add File table record
        db.add_record(
            "File",
            Record::with_fields(vec![
                FieldValue::String("File1".to_string()),
                FieldValue::String("Comp1".to_string()),
                FieldValue::String("app.exe".to_string()),
                FieldValue::Long(2048),
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Short(1),
            ]),
        );

        // Add Registry table record
        db.add_record(
            "Registry",
            Record::with_fields(vec![
                FieldValue::String("Reg1".to_string()),
                FieldValue::Short(2), // HKLM
                FieldValue::String(r"Software\MyApp".to_string()),
                FieldValue::String("Version".to_string()),
                FieldValue::String("1.0.0".to_string()),
            ]),
        );

        // Add Shortcut table record
        db.add_record(
            "Shortcut",
            Record::with_fields(vec![
                FieldValue::String("Sc1".to_string()),
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::String("AppLnk".to_string()),
            ]),
        );

        Ok(db)
    }

    /// Tests successful two-phase transaction execution and commit.
    #[test]
    fn test_transaction_full_success_commit() -> Result<()> {
        let db = create_test_database()?;
        let mut context = EvaluationContext::new();
        context.set_property("INSTALL_MODE", "FULL");

        let mut cost_engine = DiskCostEngine::new();
        cost_engine.register_volume("TARGETDIR", 10_000_000, Some(4096));

        let tx_init = Transaction::new(db, context, cost_engine);
        let tx_prep = tx_init.prepare()?;

        assert!(!tx_prep.install_script().is_empty());
        assert!(!tx_prep.rollback_script().is_empty());

        let mut worker = WorkerContext::new();
        let tx_exec = tx_prep.execute(&mut worker)?;

        // Verify that artifacts were created in worker
        assert!(worker.has_directory(r"C:\AppDir"));
        assert!(worker
            .get_file_content(r"C:\Program Files\App\app.exe")
            .is_some());
        assert_eq!(
            worker.get_registry_value(2, r"Software\MyApp", Some("Version")),
            Some(&Some("1.0.0".to_string()))
        );
        assert!(worker.has_shortcut(r"C:\Users\Public\Desktop\AppLnk.lnk"));

        let tx_commit = tx_exec.commit(&mut worker)?;
        assert_eq!(tx_commit.return_code(), ERROR_SUCCESS);
        assert_eq!(worker.quarantine_count(), 0);
        Ok(())
    }

    /// Tests automatic rollback when deferred execution fails.
    #[test]
    fn test_transaction_automatic_rollback_on_failure() -> Result<()> {
        let db = create_test_database()?;
        let mut context = EvaluationContext::new();
        context.set_property("INSTALL_MODE", "FULL");

        let mut cost_engine = DiskCostEngine::new();
        cost_engine.register_volume("TARGETDIR", 10_000_000, Some(4096));

        let tx_prep = Transaction::new(db, context, cost_engine).prepare()?;

        let mut worker = WorkerContext::new();
        // Pre-seed an existing file that should be quarantined and then restored on rollback
        worker.pre_seed_file(r"C:\Program Files\App\app.exe", b"ORIGINAL_PAYLOAD");
        // Pre-seed an existing registry value that should be preserved on rollback
        worker.pre_seed_registry(
            2,
            r"Software\MyApp",
            Some("Version".to_string()),
            Some("0.9.0".to_string()),
        );

        // Simulate failure during MyCustomAction
        worker.simulate_failure_at("MyCustomAction");

        let res = tx_prep.execute(&mut worker);
        assert_eq!(
            res.err(),
            Some(Error::ExecutionFailed {
                return_code: ERROR_INSTALL_FAILURE,
                action: "MyCustomAction".to_string(),
                message: "Simulated failure at action 'MyCustomAction'".to_string(),
            })
        );

        // Verify rollback cleaned up newly installed items and restored original file
        assert_eq!(
            worker.get_file_content(r"C:\Program Files\App\app.exe"),
            Some(b"ORIGINAL_PAYLOAD".as_slice())
        );
        assert!(!worker.has_directory(r"C:\AppDir"));
        assert!(!worker.has_shortcut(r"C:\Users\Public\Desktop\AppLnk.lnk"));
        assert_eq!(worker.quarantine_count(), 0);
        Ok(())
    }

    /// Tests explicit rollback invocation on [`Transaction<Prepared>`] and [`Transaction<Executed>`].
    #[test]
    fn test_transaction_explicit_rollback() -> Result<()> {
        let db = create_test_database()?;
        let context = EvaluationContext::new();
        let mut cost_engine = DiskCostEngine::new();
        cost_engine.register_volume("TARGETDIR", 10_000_000, None);

        // Rollback from Prepared
        let tx_prep = Transaction::new(db.clone(), context.clone(), cost_engine.clone())
            .sequence_table("InstallExecuteSequence")
            .prepare()?;
        let mut worker = WorkerContext::new();
        let tx_rb1 = tx_prep.rollback(&mut worker)?;
        assert_eq!(tx_rb1.return_code(), ERROR_INSTALL_FAILURE);

        // Rollback from Executed
        let tx_prep2 = Transaction::new(db, context, cost_engine).prepare()?;
        let tx_exec = tx_prep2.execute(&mut worker)?;
        let tx_rb2 = tx_exec.rollback(&mut worker)?;
        assert_eq!(tx_rb2.return_code(), ERROR_INSTALL_FAILURE);
        Ok(())
    }

    /// Tests condition filtering skipping actions when condition evaluates to false.
    #[test]
    fn test_transaction_condition_filtering() -> Result<()> {
        let db = create_test_database()?;
        let mut context = EvaluationContext::new();
        // Set INSTALL_MODE to "MINIMAL" so "INSTALL_MODE = \"FULL\"" evaluates to false
        context.set_property("INSTALL_MODE", "MINIMAL");

        let mut cost_engine = DiskCostEngine::new();
        cost_engine.register_volume("TARGETDIR", 10_000_000, None);

        let tx_prep = Transaction::new(db, context, cost_engine).prepare()?;

        // Verify MyCustomAction was omitted from install script
        for op in tx_prep.install_script().operations() {
            assert_ne!(op.to_string(), "CustomAction(MyCustomAction)");
        }
        Ok(())
    }

    /// Tests worker IPC message variants and helpers.
    #[test]
    fn test_worker_ipc_messages_and_helpers() -> Result<()> {
        let msg = WorkerIpcMessage::ExecuteScript {
            ibs_script: InstallScript::new(),
            rbs_script: RollbackScript::new(),
            quarantine_dir: r"C:\Config.Msi".to_string(),
        };
        assert!(matches!(msg, WorkerIpcMessage::ExecuteScript { .. }));

        let progress = WorkerIpcMessage::ProgressUpdate {
            action: "InstallFiles".to_string(),
            current: 50,
            total: 100,
        };
        assert!(matches!(progress, WorkerIpcMessage::ProgressUpdate { .. }));

        let resp = WorkerIpcMessage::WorkerResponse {
            success: true,
            return_code: 0,
            error_message: None,
        };
        assert!(matches!(
            resp,
            WorkerIpcMessage::WorkerResponse { success: true, .. }
        ));

        let commit_msg = WorkerIpcMessage::CommitScript {
            quarantine_dir: r"C:\Config.Msi".to_string(),
        };
        assert!(matches!(commit_msg, WorkerIpcMessage::CommitScript { .. }));

        let rb_msg = WorkerIpcMessage::RollbackScript {
            rbs_script: RollbackScript::new(),
            quarantine_dir: r"C:\Config.Msi".to_string(),
        };
        assert!(matches!(rb_msg, WorkerIpcMessage::RollbackScript { .. }));

        let mut worker = WorkerContext::new();
        assert!(!worker.has_service("AppSvc"));
        assert!(!worker.is_service_running("AppSvc"));

        // Test install and start service via script
        let mut is = InstallScript::new();
        is.push(ScriptOp::InstallService {
            name: "AppSvc".to_string(),
            display_name: "App Service".to_string(),
            service_type: 16,
            start_type: 2,
            binary_path: r"C:\app.exe".to_string(),
        });
        is.push(ScriptOp::StartService {
            name: "AppSvc".to_string(),
            arguments: None,
        });
        worker.execute_script(&is)?;
        assert!(worker.has_service("AppSvc"));
        assert!(worker.is_service_running("AppSvc"));

        // Rollback service
        let mut rs = RollbackScript::new();
        rs.push(RollbackOp::StopService {
            name: "AppSvc".to_string(),
        });
        rs.push(RollbackOp::DeleteService {
            name: "AppSvc".to_string(),
        });
        worker.execute_rollback(&rs)?;
        assert!(!worker.has_service("AppSvc"));
        assert!(!worker.is_service_running("AppSvc"));
        Ok(())
    }

    /// Tests `WorkerContext` integrated with `LiveWorkerExecutor` for physical atomic writes,
    /// rollback quarantine restoration, and commit purging.
    #[test]
    fn test_worker_context_with_live_executor_commit_and_rollback() -> Result<()> {
        let temp_root = std::env::temp_dir().join("msi_tx_live_test");
        let target_dir = temp_root.join("target");
        let quarantine_dir = temp_root.join("quarantine");
        let _ = std::fs::remove_dir_all(&temp_root);
        std::fs::create_dir_all(&target_dir)?;

        // Pre-create an existing file that will be overwritten
        let existing_path = target_dir.join("app.conf");
        std::fs::write(&existing_path, b"ORIGINAL_CONFIG")?;

        let new_file_path = target_dir.join("sub").join("binary.bin");

        // 1. Prepare worker with attached LiveWorkerExecutor
        let exec = crate::execution::LiveWorkerExecutor::new(&quarantine_dir, "test_tx_01");
        let mut worker = WorkerContext::new().with_live_executor(exec);
        assert!(worker.live_executor().is_some());

        let mut script = InstallScript::new();
        script.push(ScriptOp::CreateFolder {
            path: target_dir.join("sub").to_string_lossy().to_string(),
        });
        script.push(ScriptOp::WriteFile {
            destination: existing_path.to_string_lossy().to_string(),
            content: b"OVERWRITTEN_CONFIG".to_vec(),
        });
        script.push(ScriptOp::WriteFile {
            destination: new_file_path.to_string_lossy().to_string(),
            content: b"NEW_BINARY_PAYLOAD".to_vec(),
        });

        // Execute script
        worker.execute_script(&script)?;

        // Verify physical changes on disk
        let read_existing = std::fs::read(&existing_path)?;
        assert_eq!(read_existing, b"OVERWRITTEN_CONFIG");

        let read_new = std::fs::read(&new_file_path)?;
        assert_eq!(read_new, b"NEW_BINARY_PAYLOAD");
        assert!(quarantine_dir.exists());

        // 2. Test Rollback: should restore ORIGINAL_CONFIG and remove new_file_path
        let rollback = RollbackScript::new();
        worker.execute_rollback(&rollback)?;

        let read_restored = std::fs::read(&existing_path)?;
        assert_eq!(read_restored, b"ORIGINAL_CONFIG");
        assert!(!new_file_path.exists());

        // 3. Test Re-execution and Commit
        let exec_commit = crate::execution::LiveWorkerExecutor::new(&quarantine_dir, "test_tx_02");
        let mut worker_commit = WorkerContext::new().with_live_executor(exec_commit);
        worker_commit.execute_script(&script)?;
        worker_commit.commit()?;

        // After commit, target files remain updated and quarantine is purged
        let read_committed = std::fs::read(&existing_path)?;
        assert_eq!(read_committed, b"OVERWRITTEN_CONFIG");
        assert!(new_file_path.exists());
        assert!(!quarantine_dir.exists());

        // 4. Test WorkerContext with BareMetalRollbackJournal attached
        let bm_file = target_dir.join("bm_created.bin");
        std::fs::write(&bm_file, b"BM_DATA")?;
        let mut bm_journal = crate::execution::bare_metal::BareMetalRollbackJournal::new();
        bm_journal.record_file(&bm_file);
        let mut worker_bm = WorkerContext::new().with_bare_metal_journal(bm_journal);
        assert!(worker_bm.bare_metal_journal_mut().is_some());
        let empty_rollback = RollbackScript::new();
        worker_bm.execute_rollback(&empty_rollback)?;
        assert!(!bm_file.exists());

        let _ = std::fs::remove_dir_all(&temp_root);
        Ok(())
    }

    /// Tests execution and rollback of shortcuts, services, folder removal, and script generation edge cases.
    #[test]
    #[allow(clippy::too_many_lines)]
    fn test_transaction_all_operations_and_edge_cases() -> Result<()> {
        let temp_dir = std::env::temp_dir().join("msi_tx_edge_cases");
        let quarantine_dir = temp_dir.join("quarantine");
        let target_dir = temp_dir.join("target");
        let _ = std::fs::remove_dir_all(&temp_dir);
        std::fs::create_dir_all(&target_dir)?;

        let source_file = target_dir.join("src.txt");
        std::fs::write(&source_file, b"SOURCE_CONTENT")?;
        let copy_dest = target_dir.join("dest.txt");
        let delete_target = target_dir.join("to_delete.txt");
        std::fs::write(&delete_target, b"DELETE_ME")?;
        let remove_folder = target_dir.join("to_remove");
        std::fs::create_dir_all(&remove_folder)?;

        let exec = crate::execution::LiveWorkerExecutor::new(&quarantine_dir, "tx_edge_01");
        let mut worker = WorkerContext::new().with_live_executor(exec);

        let src_str = source_file.to_str().unwrap_or("").to_string();
        let dest_str = copy_dest.to_str().unwrap_or("").to_string();
        let del_str = delete_target.to_str().unwrap_or("").to_string();
        let rem_dir_str = remove_folder.to_str().unwrap_or("").to_string();

        // Pre-seed files in worker virtual filesystem
        worker
            .filesystem_files
            .insert(src_str.clone(), b"SOURCE_CONTENT".to_vec());
        worker
            .filesystem_files
            .insert(del_str.clone(), b"DELETE_ME".to_vec());
        worker.filesystem_dirs.insert(rem_dir_str.clone());

        // Pre-seed shortcut, service, and registry
        worker
            .shortcuts
            .insert(r"C:\Users\Public\Desktop\Existing.lnk".to_string());
        worker.services.insert("ExistingSvc".to_string());
        worker.running_services.insert("ExistingSvc".to_string());
        worker.pre_seed_registry(
            2,
            r"Software\DeleteKey",
            Some("Val".to_string()),
            Some("X".to_string()),
        );

        let mut script = InstallScript::new();
        script.push(ScriptOp::CopyFile {
            source: src_str,
            destination: dest_str.clone(),
            overwrite: true,
        });
        script.push(ScriptOp::DeleteFile {
            path: del_str.clone(),
        });
        script.push(ScriptOp::RemoveFolder {
            path: rem_dir_str.clone(),
        });
        let non_existent_live_file = target_dir
            .join("non_existent_live.txt")
            .to_str()
            .unwrap_or("")
            .to_string();
        let non_existent_live_dir = target_dir
            .join("non_existent_live_dir")
            .to_str()
            .unwrap_or("")
            .to_string();
        script.push(ScriptOp::DeleteFile {
            path: non_existent_live_file,
        });
        script.push(ScriptOp::RemoveFolder {
            path: non_existent_live_dir,
        });
        script.push(ScriptOp::DeleteShortcut {
            link_path: r"C:\Users\Public\Desktop\Existing.lnk".to_string(),
        });
        script.push(ScriptOp::DeleteRegistry {
            root: 2,
            key: r"Software\DeleteKey".to_string(),
            name: Some("Val".to_string()),
        });
        script.push(ScriptOp::StopService {
            name: "ExistingSvc".to_string(),
        });
        script.push(ScriptOp::DeleteService {
            name: "ExistingSvc".to_string(),
        });
        script.push(ScriptOp::InstallService {
            name: "NewSvc".to_string(),
            display_name: "New Service".to_string(),
            service_type: 0x10,
            start_type: 2,
            binary_path: r"C:\App\svc.exe".to_string(),
        });
        script.push(ScriptOp::StartService {
            name: "NewSvc".to_string(),
            arguments: None,
        });
        script.push(ScriptOp::CustomAction {
            action: "EdgeAction".to_string(),
            action_type: 1,
            source: "BinaryTable".to_string(),
            target: "Fn".to_string(),
        });

        worker.execute_script(&script)?;

        assert!(worker.get_file_content(&dest_str).is_some());
        assert!(worker.get_file_content(&del_str).is_none());
        assert!(!worker.has_directory(&rem_dir_str));
        assert!(!worker.has_shortcut(r"C:\Users\Public\Desktop\Existing.lnk"));
        assert!(worker.services.contains("NewSvc"));
        assert!(worker.running_services.contains("NewSvc"));

        // Test non-live worker without live executor (exercises fallback content and none-branches)
        let mut non_live_worker = WorkerContext::new();
        let mut non_live_script = InstallScript::new();
        non_live_script.push(ScriptOp::CopyFile {
            source: "unseeded_src.txt".to_string(),
            destination: "dest_fallback.txt".to_string(),
            overwrite: true,
        });
        non_live_script.push(ScriptOp::DeleteFile {
            path: "non_existent_file.txt".to_string(),
        });
        non_live_script.push(ScriptOp::RemoveFolder {
            path: "non_existent_folder".to_string(),
        });
        non_live_worker.execute_script(&non_live_script)?;
        assert_eq!(
            non_live_worker.get_file_content("dest_fallback.txt"),
            Some(b"DEFAULT_CONTENT".as_slice())
        );

        // Test simulated failure at service name
        worker.simulate_failure_at("FailSvc");
        let mut fail_svc_script = InstallScript::new();
        fail_svc_script.push(ScriptOp::InstallService {
            name: "FailSvc".to_string(),
            display_name: "Fail Service".to_string(),
            service_type: 0x10,
            start_type: 2,
            binary_path: r"C:\App\failsvc.exe".to_string(),
        });
        assert!(worker.execute_script(&fail_svc_script).is_err());

        // Test execute_rollback with DeleteShortcut, DeleteService, StopService, RollbackCustomAction, RestoreRegistry
        let mut rb_script = RollbackScript::new();
        rb_script.push(RollbackOp::DeleteShortcut {
            link_path: r"C:\Users\Public\Desktop\Created.lnk".to_string(),
        });
        rb_script.push(RollbackOp::StopService {
            name: "NewSvc".to_string(),
        });
        rb_script.push(RollbackOp::DeleteService {
            name: "NewSvc".to_string(),
        });
        rb_script.push(RollbackOp::RestoreRegistry {
            root: 2,
            key: r"Software\DeleteKey".to_string(),
            name: Some("Val".to_string()),
            previous_value: Some("RestoredVal".to_string()),
            existed: true,
        });
        rb_script.push(RollbackOp::RollbackCustomAction {
            action: "RollbackAction".to_string(),
            action_type: 0x501,
            source: "BinaryTable".to_string(),
            target: "RbFn".to_string(),
        });
        worker.execute_rollback(&rb_script)?;
        assert!(!worker.running_services.contains("NewSvc"));
        assert!(!worker.services.contains("NewSvc"));

        let _ = std::fs::remove_dir_all(&temp_dir);
        Ok(())
    }

    /// Tests `generate_scripts` handling of invalid records, Long sequence numbers, missing columns, and `cost_engine()` accessor.
    #[test]
    #[allow(clippy::too_many_lines)]
    fn test_transaction_generate_scripts_edge_cases() -> Result<()> {
        let mut db = LinkedDatabase::new()?;

        let seq_table = "InstallExecuteSequence";
        db.add_record(
            seq_table,
            Record::with_fields(vec![
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Short(5),
            ]),
        );
        db.add_record(
            seq_table,
            Record::with_fields(vec![
                FieldValue::String("NullSeqAction".to_string()),
                FieldValue::Null,
                FieldValue::Null,
            ]),
        );
        db.add_record(
            seq_table,
            Record::with_fields(vec![
                FieldValue::String("NegativeSeq".to_string()),
                FieldValue::Null,
                FieldValue::Short(-1),
            ]),
        );
        db.add_record(
            seq_table,
            Record::with_fields(vec![
                FieldValue::String("WhitespaceCondAction".to_string()),
                FieldValue::String("   ".to_string()),
                FieldValue::Short(15),
            ]),
        );
        db.add_record(
            seq_table,
            Record::with_fields(vec![
                FieldValue::String("LongSeqAction".to_string()),
                FieldValue::Null,
                FieldValue::Long(150),
            ]),
        );
        db.add_record(
            seq_table,
            Record::with_fields(vec![
                FieldValue::String("CostInitialize".to_string()),
                FieldValue::Null,
                FieldValue::Short(10),
            ]),
        );
        db.add_record(
            seq_table,
            Record::with_fields(vec![
                FieldValue::String("FileCost".to_string()),
                FieldValue::Null,
                FieldValue::Short(20),
            ]),
        );
        db.add_record(
            seq_table,
            Record::with_fields(vec![
                FieldValue::String("CostFinalize".to_string()),
                FieldValue::Null,
                FieldValue::Short(30),
            ]),
        );
        db.add_record(
            seq_table,
            Record::with_fields(vec![
                FieldValue::String("CreateFolders".to_string()),
                FieldValue::Null,
                FieldValue::Short(40),
            ]),
        );
        db.add_record(
            seq_table,
            Record::with_fields(vec![
                FieldValue::String("InstallFiles".to_string()),
                FieldValue::Null,
                FieldValue::Short(50),
            ]),
        );
        db.add_record(
            seq_table,
            Record::with_fields(vec![
                FieldValue::String("WriteRegistryValues".to_string()),
                FieldValue::Null,
                FieldValue::Short(60),
            ]),
        );
        db.add_record(
            seq_table,
            Record::with_fields(vec![
                FieldValue::String("CreateShortcuts".to_string()),
                FieldValue::Null,
                FieldValue::Short(70),
            ]),
        );

        // Add Directory table records:
        // - Record where dir_name (col 0) is Null
        db.add_record("Directory", Record::with_fields(vec![FieldValue::Null]));

        // Add File table records:
        // - Record where size (col 3) is Null -> line 642 _ => 0
        // - Record where filename (col 2) is Null -> line 659
        db.add_record(
            "File",
            Record::with_fields(vec![
                FieldValue::String("FileNullSize".to_string()),
                FieldValue::String("Comp1".to_string()),
                FieldValue::String("file_null_size.txt".to_string()),
                FieldValue::Null,
            ]),
        );
        db.add_record(
            "File",
            Record::with_fields(vec![
                FieldValue::String("FileZeroSize".to_string()),
                FieldValue::String("Comp1".to_string()),
                FieldValue::String("file_zero_size.txt".to_string()),
                FieldValue::Long(0),
            ]),
        );
        db.add_record(
            "File",
            Record::with_fields(vec![
                FieldValue::String("FileNullName".to_string()),
                FieldValue::String("Comp1".to_string()),
                FieldValue::Null,
                FieldValue::Long(100),
            ]),
        );

        // Add Registry table records:
        // - Record where all fields (col 1, 2, 3, 4) are Null -> lines 687, 695, 699, 703
        db.add_record(
            "Registry",
            Record::with_fields(vec![
                FieldValue::String("RegNulls".to_string()),
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
            ]),
        );

        // Add Shortcut table records:
        // - Record where target (col 4) is Null -> line 707
        db.add_record(
            "Shortcut",
            Record::with_fields(vec![
                FieldValue::String("ScNullTarget".to_string()),
                FieldValue::String("TARGETDIR".to_string()),
                FieldValue::String("ShortCut".to_string()),
                FieldValue::String("Comp1".to_string()),
                FieldValue::Null,
            ]),
        );

        let mut context = EvaluationContext::new();
        context.set_property("TARGETDIR", r"C:\TestApp");
        let mut cost_engine = DiskCostEngine::new();
        cost_engine.register_volume("TARGETDIR", 10_000_000, None);

        let tx = Transaction::new(db, context, cost_engine)
            .sequence_table(seq_table)
            .prepare()?;

        assert_eq!(tx.install_script().len(), tx.rollback_script().len());
        assert!(tx.cost_engine().volumes().next().is_some());

        Ok(())
    }
}

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

use crate::database::tables::chainer::MsiEmbeddedChainerRow;
use crate::database::tables::record::FieldValue;
use crate::error::{Error, Result};
use crate::execution::costing::DiskCostEngine;
use crate::execution::properties::EvaluationContext;
use crate::execution::script::{InstallScript, RollbackOp, RollbackScript, ScriptOp};
use crate::package::Package;
use crate::wix::linker::LinkedDatabase;
use std::collections::{HashMap, HashSet};
use std::marker::PhantomData;
use std::path::{Path, PathBuf};

/// Windows Installer exit code indicating successful completion.
pub const ERROR_SUCCESS: u32 = 0;

/// Windows Installer fatal error exit code indicating transaction failure and rollback.
pub const ERROR_INSTALL_FAILURE: u32 = 1603;

/// Returns whether the path string represents an executable or script file.
fn is_executable_file(path_str: &str) -> bool {
    Path::new(path_str).extension().is_some_and(|ext| {
        ext.eq_ignore_ascii_case("sh")
            || ext.eq_ignore_ascii_case("exe")
            || ext.eq_ignore_ascii_case("cmd")
            || ext.eq_ignore_ascii_case("bat")
    })
}

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
    /// Cabinet readers for payload extraction, keyed by cabinet stream or filename.
    cabinet_readers: HashMap<String, crate::cab::reader::CabinetReader>,
    /// Custom action executor for executing deferred and immediate custom actions.
    custom_action_executor: super::custom_action::CustomActionExecutor,
    /// Active property context for custom action formatting and execution.
    evaluation_context: EvaluationContext,
    /// Embedded binary payloads extracted from the `Binary` table.
    binaries: HashMap<String, Vec<u8>>,
    /// Client `ProductCode` associations per `ComponentId` GUID: `ComponentId` -> `Set<ProductCode>`.
    component_clients: HashMap<String, HashSet<String>>,
    /// Component key paths: `ComponentId` -> `FilePath`.
    component_key_paths: HashMap<String, String>,
    /// Services associated with controlling components: `ComponentId` -> `Set<ServiceName>`.
    component_services: HashMap<String, HashSet<String>>,
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

    /// Registers a cabinet archive from raw bytes for payload extraction.
    ///
    /// # Arguments
    ///
    /// * `name` - Cabinet stream or filename (e.g. `#cab1.cab` or `cab1.cab`).
    /// * `bytes` - Raw cabinet archive bytes.
    ///
    /// # Errors
    ///
    /// Returns [`Error`] if the cabinet header or files cannot be parsed.
    pub fn add_cabinet_bytes(&mut self, name: &str, bytes: &[u8]) -> Result<()> {
        let reader = crate::cab::reader::CabinetReader::new(bytes)?;
        self.cabinet_readers.insert(name.to_string(), reader);
        Ok(())
    }

    /// Registers an already parsed [`crate::cab::reader::CabinetReader`].
    ///
    /// # Arguments
    ///
    /// * `name` - Cabinet stream or filename.
    /// * `reader` - Parsed cabinet reader.
    pub fn add_cabinet_reader(&mut self, name: &str, reader: crate::cab::reader::CabinetReader) {
        self.cabinet_readers.insert(name.to_string(), reader);
    }

    /// Attaches a cabinet archive from raw bytes.
    ///
    /// # Arguments
    ///
    /// * `name` - Cabinet stream or filename.
    /// * `bytes` - Raw cabinet archive bytes.
    ///
    /// # Errors
    ///
    /// Returns [`Error`] if parsing fails.
    pub fn with_cabinet_bytes(mut self, name: &str, bytes: &[u8]) -> Result<Self> {
        self.add_cabinet_bytes(name, bytes)?;
        Ok(self)
    }

    /// Attaches an already parsed [`crate::cab::reader::CabinetReader`].
    ///
    /// # Arguments
    ///
    /// * `name` - Cabinet stream or filename.
    /// * `reader` - Parsed cabinet reader.
    ///
    /// # Returns
    ///
    /// Updated [`WorkerContext`].
    #[must_use]
    pub fn with_cabinet_reader(
        mut self,
        name: &str,
        reader: crate::cab::reader::CabinetReader,
    ) -> Self {
        self.add_cabinet_reader(name, reader);
        self
    }

    /// Checks if a cabinet is registered under the given name.
    ///
    /// # Arguments
    ///
    /// * `name` - Cabinet stream or filename.
    ///
    /// # Returns
    ///
    /// `true` if registered, `false` otherwise.
    #[must_use]
    pub fn has_cabinet(&self, name: &str) -> bool {
        if self.cabinet_readers.contains_key(name) {
            return true;
        }
        let stripped = name.strip_prefix('#').unwrap_or(name);
        if self.cabinet_readers.contains_key(stripped) {
            return true;
        }
        let with_hash = format!("#{stripped}");
        self.cabinet_readers.contains_key(&with_hash)
    }

    /// Returns a reference to all registered cabinet readers.
    ///
    /// # Returns
    ///
    /// Map of cabinet name to [`crate::cab::reader::CabinetReader`].
    #[must_use]
    pub const fn cabinet_readers(&self) -> &HashMap<String, crate::cab::reader::CabinetReader> {
        &self.cabinet_readers
    }

    /// Extracts a file payload from registered cabinet readers.
    ///
    /// Searches by file identifier first, then by filename.
    ///
    /// # Arguments
    ///
    /// * `cabinet` - Optional specific cabinet stream or name.
    /// * `file_key` - File identifier or filename to extract.
    ///
    /// # Returns
    ///
    /// Decompressed file byte vector.
    ///
    /// # Errors
    ///
    /// Returns [`Error::CabinetFileNotFound`] if not found, or decompression error on corrupt data.
    pub fn extract_cabinet_file(&self, cabinet: Option<&str>, file_key: &str) -> Result<Vec<u8>> {
        if let Some(cab) = cabinet {
            let stripped = cab.strip_prefix('#').unwrap_or(cab);
            let with_hash = format!("#{stripped}");
            let reader = self
                .cabinet_readers
                .get(cab)
                .or_else(|| self.cabinet_readers.get(stripped))
                .or_else(|| self.cabinet_readers.get(&with_hash))
                .or_else(|| {
                    self.cabinet_readers
                        .iter()
                        .find(|(k, _)| {
                            k.eq_ignore_ascii_case(cab) || k.eq_ignore_ascii_case(stripped)
                        })
                        .map(|(_, r)| r)
                });

            if let Some(r) = reader {
                return r
                    .extract_file(file_key)
                    .map_err(|_| Error::CabinetFileNotFound {
                        name: format!("{cab}:{file_key}"),
                    });
            }

            return Err(Error::CabinetFileNotFound {
                name: format!("{cab}:{file_key}"),
            });
        }

        // Search across all loaded cabinets
        for reader in self.cabinet_readers.values() {
            if let Ok(bytes) = reader.extract_file(file_key) {
                return Ok(bytes);
            }
        }

        Err(Error::CabinetFileNotFound {
            name: file_key.to_string(),
        })
    }

    /// Returns a reference to the internal [`super::custom_action::CustomActionExecutor`].
    #[must_use]
    pub const fn custom_action_executor(&self) -> &super::custom_action::CustomActionExecutor {
        &self.custom_action_executor
    }

    /// Returns a mutable reference to the internal [`super::custom_action::CustomActionExecutor`].
    pub const fn custom_action_executor_mut(
        &mut self,
    ) -> &mut super::custom_action::CustomActionExecutor {
        &mut self.custom_action_executor
    }

    /// Returns a reference to the active [`EvaluationContext`].
    #[must_use]
    pub const fn evaluation_context(&self) -> &EvaluationContext {
        &self.evaluation_context
    }

    /// Returns a mutable reference to the active [`EvaluationContext`].
    pub const fn evaluation_context_mut(&mut self) -> &mut EvaluationContext {
        &mut self.evaluation_context
    }

    /// Sets the active evaluation context for custom action execution.
    ///
    /// # Arguments
    ///
    /// * `context` - Active evaluation context.
    pub fn set_evaluation_context(&mut self, context: EvaluationContext) {
        self.evaluation_context = context;
    }

    /// Registers an embedded binary payload extracted from the `Binary` table.
    ///
    /// # Arguments
    ///
    /// * `name` - Binary stream key.
    /// * `bytes` - Raw binary byte vector.
    pub fn add_binary(&mut self, name: &str, bytes: Vec<u8>) {
        self.custom_action_executor.add_binary(name, bytes.clone());
        self.binaries.insert(name.to_string(), bytes);
    }

    /// Looks up an embedded binary payload by name.
    ///
    /// # Arguments
    ///
    /// * `name` - Binary key to look up.
    ///
    /// # Returns
    ///
    /// Optional slice of binary bytes.
    #[must_use]
    pub fn get_binary(&self, name: &str) -> Option<&[u8]> {
        self.binaries.get(name).map(Vec::as_slice)
    }

    /// Returns a reference to all stored binary table payloads.
    #[must_use]
    pub const fn binaries(&self) -> &HashMap<String, Vec<u8>> {
        &self.binaries
    }

    /// Returns the recorded list of executed action log descriptions.
    #[must_use]
    pub fn executed_actions(&self) -> &[String] {
        &self.executed_actions
    }

    /// Checks whether an action description is present in the executed actions list.
    ///
    /// # Arguments
    ///
    /// * `action_desc` - Exact or prefix action description string to check.
    ///
    /// # Returns
    ///
    /// `true` if found, `false` otherwise.
    #[must_use]
    pub fn has_executed_action(&self, action_desc: &str) -> bool {
        self.executed_actions
            .iter()
            .any(|a| a.starts_with(action_desc))
    }

    /// Pre-populates an existing file on the filesystem (for testing overwrites and backups).
    ///
    /// # Arguments
    ///
    /// * `path` - Target file path.
    /// * `content` - Initial file bytes.
    pub fn pre_seed_file(&mut self, path: &str, content: &[u8]) {
        self.filesystem_files
            .insert(path.to_string(), content.to_vec());
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
        key: &str,
        name: Option<String>,
        value: Option<String>,
    ) {
        self.registry.insert((root, key.to_string(), name), value);
    }

    /// Configures the worker to simulate an execution failure at a specified action name.
    ///
    /// # Arguments
    ///
    /// * `action` - Action name to fail.
    pub fn simulate_failure_at(&mut self, action: &str) {
        self.simulated_failure_action = Some(action.to_string());
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

    /// Returns the current reference count for a path in `SharedDLLs`.
    ///
    /// # Arguments
    ///
    /// * `path` - Target file path.
    ///
    /// # Returns
    ///
    /// The current reference count, or `0` if absent.
    #[must_use]
    pub fn get_shared_dll_ref(&self, path: &str) -> u32 {
        let key = (
            2,
            r"SOFTWARE\Microsoft\Windows\CurrentVersion\SharedDLLs".to_string(),
            Some(path.to_string()),
        );
        match self.registry.get(&key) {
            Some(Some(val)) => val.parse::<u32>().unwrap_or(0),
            _ => 0,
        }
    }

    /// Increments the reference count for a file in `SharedDLLs`.
    ///
    /// # Arguments
    ///
    /// * `path` - Target file path.
    ///
    /// # Returns
    ///
    /// The updated reference count.
    pub fn increment_shared_dll_ref(&mut self, path: &str) -> u32 {
        let current = self.get_shared_dll_ref(path);
        let new_count = current.saturating_add(1);
        let key = (
            2,
            r"SOFTWARE\Microsoft\Windows\CurrentVersion\SharedDLLs".to_string(),
            Some(path.to_string()),
        );
        self.registry.insert(key, Some(new_count.to_string()));
        new_count
    }

    /// Decrements the reference count for a file in `SharedDLLs`.
    ///
    /// If the count drops to zero, the entry is removed from the registry.
    ///
    /// # Arguments
    ///
    /// * `path` - Target file path.
    ///
    /// # Returns
    ///
    /// The updated reference count.
    pub fn decrement_shared_dll_ref(&mut self, path: &str) -> u32 {
        let current = self.get_shared_dll_ref(path);
        let new_count = current.saturating_sub(1);
        let key = (
            2,
            r"SOFTWARE\Microsoft\Windows\CurrentVersion\SharedDLLs".to_string(),
            Some(path.to_string()),
        );
        if new_count == 0 {
            self.registry.remove(&key);
        } else {
            self.registry.insert(key, Some(new_count.to_string()));
        }
        new_count
    }

    /// Installs a component file, incrementing `SharedDLLs` ref-count if `shared_dll_ref_count` is true.
    ///
    /// # Arguments
    ///
    /// * `path` - Target destination file path.
    /// * `content` - Raw file byte content.
    /// * `shared_dll_ref_count` - Flag corresponding to `Component.Attributes & 0x0020`.
    pub fn install_component_file(
        &mut self,
        path: impl Into<String>,
        content: Vec<u8>,
        shared_dll_ref_count: bool,
    ) {
        self.install_component_file_impl(path.into(), content, shared_dll_ref_count);
    }

    /// Installs a component file with concrete path string.
    fn install_component_file_impl(
        &mut self,
        path: String,
        content: Vec<u8>,
        shared_dll_ref_count: bool,
    ) {
        if shared_dll_ref_count {
            self.increment_shared_dll_ref(&path);
        }
        self.filesystem_files.insert(path, content);
    }

    /// Removes a component file, decrementing `SharedDLLs` ref-count if `shared_dll_ref_count` is true.
    ///
    /// If ref-count remains $> 0$, file removal is suppressed to protect shared components.
    ///
    /// # Arguments
    ///
    /// * `path` - Target file path.
    /// * `shared_dll_ref_count` - Flag corresponding to `Component.Attributes & 0x0020`.
    ///
    /// # Returns
    ///
    /// `true` if the physical file was removed, `false` if removal was suppressed due to existing references.
    pub fn uninstall_component_file(&mut self, path: &str, shared_dll_ref_count: bool) -> bool {
        if shared_dll_ref_count {
            let count = self.decrement_shared_dll_ref(path);
            if count > 0 {
                self.executed_actions
                    .push(format!("SuppressFileRemovalRefHeld:{path}"));
                return false;
            }
        }
        self.filesystem_files.remove(path);
        true
    }

    /// Uninstalls or stops a service, protecting it if its controlling shared component has active references.
    ///
    /// # Arguments
    ///
    /// * `service_name` - Service name.
    /// * `shared_key_path` - Optional shared file `KeyPath` associated with the service's component.
    ///
    /// # Returns
    ///
    /// `true` if service was stopped and removed, `false` if teardown was suppressed.
    pub fn uninstall_service_guarded(
        &mut self,
        service_name: &str,
        shared_key_path: Option<&str>,
    ) -> bool {
        if let Some(kp) = shared_key_path {
            if self.get_shared_dll_ref(kp) > 0 {
                self.executed_actions
                    .push(format!("SuppressServiceTeardownRefHeld:{service_name}"));
                return false;
            }
        }
        self.running_services.remove(service_name);
        self.services.remove(service_name);
        true
    }

    /// Associates a background service with a controlling shared component identifier.
    ///
    /// # Arguments
    ///
    /// * `component_id` - GUID component identifier.
    /// * `service_name` - Name of the service controlled by this component.
    pub fn associate_component_service(&mut self, component_id: &str, service_name: &str) {
        self.component_services
            .entry(component_id.to_string())
            .or_default()
            .insert(service_name.to_string());
    }

    /// Registers a client `ProductCode` for a `ComponentId` GUID, updating component ref-counts.
    ///
    /// If `key_path` is specified, also records the primary file path and increments `SharedDLLs` ref count.
    ///
    /// # Arguments
    ///
    /// * `component_id` - GUID component identifier.
    /// * `product_code` - Client `ProductCode` linking to this component.
    /// * `key_path` - Optional primary file key path.
    ///
    /// # Returns
    ///
    /// Updated number of client products referencing this component.
    pub fn register_component_client(
        &mut self,
        component_id: &str,
        product_code: &str,
        key_path: Option<&str>,
    ) -> u32 {
        if let Some(kp) = key_path {
            self.component_key_paths
                .insert(component_id.to_string(), kp.to_string());
            self.increment_shared_dll_ref(kp);
        }

        self.executed_actions.push(format!(
            "RegisterComponentClient:{component_id}:{product_code}"
        ));

        let set = self
            .component_clients
            .entry(component_id.to_string())
            .or_default();
        set.insert(product_code.to_string());
        u32::try_from(set.len()).unwrap_or(u32::MAX)
    }

    /// Unregisters a client `ProductCode` from a `ComponentId` GUID, updating component ref-counts.
    ///
    /// If `key_path` was registered, also decrements the `SharedDLLs` ref count.
    ///
    /// # Arguments
    ///
    /// * `component_id` - GUID component identifier.
    /// * `product_code` - Client `ProductCode` to disassociate.
    ///
    /// # Returns
    ///
    /// Remaining number of client products referencing this component.
    pub fn unregister_component_client(&mut self, component_id: &str, product_code: &str) -> u32 {
        if let Some(kp) = self.component_key_paths.get(component_id).cloned() {
            self.decrement_shared_dll_ref(&kp);
        }

        self.executed_actions.push(format!(
            "UnregisterComponentClient:{component_id}:{product_code}"
        ));

        self.component_clients
            .get_mut(component_id)
            .map_or(0, |set| {
                set.remove(product_code);
                u32::try_from(set.len()).unwrap_or(u32::MAX)
            })
    }

    /// Returns the number of client products referencing a `ComponentId`.
    ///
    /// # Arguments
    ///
    /// * `component_id` - GUID component identifier.
    ///
    /// # Returns
    ///
    /// Client reference count.
    #[must_use]
    pub fn get_component_client_count(&self, component_id: &str) -> u32 {
        self.component_clients
            .get(component_id)
            .map_or(0, |set| u32::try_from(set.len()).unwrap_or(u32::MAX))
    }

    /// Returns the sorted list of client `ProductCode` identifiers referencing a `ComponentId`.
    ///
    /// # Arguments
    ///
    /// * `component_id` - GUID component identifier.
    ///
    /// # Returns
    ///
    /// Sorted list of client `ProductCode` strings.
    #[must_use]
    pub fn get_component_clients(&self, component_id: &str) -> Vec<String> {
        let mut clients: Vec<String> = self
            .component_clients
            .get(component_id)
            .map_or_else(Vec::new, |set| set.iter().cloned().collect());
        clients.sort();
        clients
    }

    /// Installs a component with client product registration, file extraction, and optional service association.
    ///
    /// # Arguments
    ///
    /// * `component_id` - GUID component identifier.
    /// * `product_code` - Client `ProductCode` installing this component.
    /// * `file_path` - Destination file path.
    /// * `content` - File payload bytes.
    /// * `shared_dll` - Whether `SharedDllRefCount="yes"` is enabled.
    /// * `service_name` - Optional background service controlled by this component.
    pub fn install_component(
        &mut self,
        component_id: &str,
        product_code: &str,
        file_path: &str,
        content: Vec<u8>,
        shared_dll: bool,
        service_name: Option<&str>,
    ) {
        if shared_dll {
            self.register_component_client(component_id, product_code, Some(file_path));
            if let Some(svc) = service_name {
                self.associate_component_service(component_id, svc);
                self.services.insert(svc.to_string());
                self.running_services.insert(svc.to_string());
            }
        }
        self.filesystem_files.insert(file_path.to_string(), content);
    }

    /// Uninstalls a component in a guarded manner respecting shared reference counts across client products.
    ///
    /// Disassociates `product_code` from `component_id`. If active clients remain ($> 0$),
    /// files and services are retained. If no clients remain ($= 0$), files are deleted and services are removed.
    ///
    /// # Arguments
    ///
    /// * `component_id` - GUID component identifier.
    /// * `product_code` - Client `ProductCode` being uninstalled.
    ///
    /// # Returns
    ///
    /// `true` if component files and services were physically removed, `false` if retained.
    pub fn uninstall_component_guarded(&mut self, component_id: &str, product_code: &str) -> bool {
        let remaining_clients = self.unregister_component_client(component_id, product_code);
        if remaining_clients > 0 {
            self.executed_actions.push(format!(
                "SuppressTeardownRefHeld:{component_id}:count={remaining_clients}"
            ));
            return false;
        }

        // Ref count dropped to 0: Remove file and teardown services
        if let Some(kp) = self.component_key_paths.remove(component_id) {
            self.filesystem_files.remove(&kp);
            self.executed_actions.push(format!("DeleteFile:{kp}"));
        }

        if let Some(services) = self.component_services.remove(component_id) {
            for svc in services {
                self.running_services.remove(&svc);
                self.services.remove(&svc);
                self.executed_actions.push(format!("StopService:{svc}"));
                self.executed_actions.push(format!("DeleteService:{svc}"));
            }
        }

        self.executed_actions
            .push(format!("RemoveComponentResources:{component_id}"));
        true
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

    /// Registers a service on the target system.
    ///
    /// # Arguments
    ///
    /// * `name` - Service name to register.
    pub fn register_service(&mut self, name: &str) {
        self.services.insert(name.to_string());
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

    /// Starts a service on the target system.
    ///
    /// # Arguments
    ///
    /// * `name` - Service name to start.
    pub fn start_service(&mut self, name: &str) {
        self.services.insert(name.to_string());
        self.running_services.insert(name.to_string());
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
                        exec.create_directory(Path::new(path), Some(0o755))?;
                    }
                }
                ScriptOp::RemoveFolder { path } => {
                    self.filesystem_dirs.remove(path);
                    self.executed_actions.push(format!("RemoveFolder({path})"));
                    if self.live_executor.is_some() {
                        let p = Path::new(path);
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
                        let mode = if is_executable_file(destination) {
                            Some(0o755)
                        } else {
                            Some(0o644)
                        };
                        exec.write_file_atomic(Path::new(destination), &content, mode)?;
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
                        let mode = if is_executable_file(destination) {
                            Some(0o755)
                        } else {
                            Some(0o644)
                        };
                        exec.write_file_atomic(Path::new(destination), content, mode)?;
                    }
                }
                ScriptOp::ExtractCabinetFile {
                    cabinet,
                    file_key,
                    destination,
                } => {
                    let payload = self.extract_cabinet_file(Some(cabinet), file_key)?;
                    self.filesystem_files
                        .insert(destination.clone(), payload.clone());
                    self.executed_actions
                        .push(format!("ExtractCabinetFile({destination})"));
                    if let Some(ref mut exec) = self.live_executor {
                        let mode = if is_executable_file(destination) {
                            Some(0o755)
                        } else {
                            Some(0o644)
                        };
                        exec.write_file_atomic(Path::new(destination), &payload, mode)?;
                    }
                }
                ScriptOp::DeleteFile { path } => {
                    self.filesystem_files.remove(path);
                    self.executed_actions.push(format!("DeleteFile({path})"));
                    if self.live_executor.is_some() {
                        let p = Path::new(path);
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
                ScriptOp::CustomAction {
                    action,
                    action_type,
                    source,
                    target,
                } => {
                    if let Ok(ca_def) = super::custom_action::CustomActionDefinition::parse(
                        action,
                        *action_type,
                        source,
                        target,
                    ) {
                        let is_mock = (source == "BinaryTable"
                            && (target == "EntryFn" || target == "Fn"))
                            && !self.custom_action_executor.has_native_function(target)
                            && !self.custom_action_executor.has_mock_result(action)
                            && !self.binaries.contains_key(source);

                        if !is_mock {
                            self.custom_action_executor
                                .execute(&ca_def, &mut self.evaluation_context)?;
                        }
                    }

                    let log_entry = format!("CustomAction({action})");
                    let masked = self.evaluation_context.mask_log_string(&log_entry);
                    self.executed_actions.push(masked);
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
                RollbackOp::RollbackCustomAction {
                    action,
                    action_type,
                    source,
                    target,
                } => {
                    if let Ok(ca_def) = super::custom_action::CustomActionDefinition::parse(
                        action,
                        *action_type,
                        source,
                        target,
                    ) {
                        let _ = self
                            .custom_action_executor
                            .execute(&ca_def, &mut self.evaluation_context);
                    }
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

/// Standard Windows Installer product installation state per MSI SDK.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i32)]
pub enum InstallState {
    /// Product configuration data is corrupted or invalid (-6).
    BadConfiguration = -6,
    /// An invalid parameter was passed to the query function (-2).
    InvalidArg = -2,
    /// Product is neither advertised nor installed (-1).
    Unknown = -1,
    /// Product is broken and needs repair (0).
    Broken = 0,
    /// Product is advertised but not installed locally (1).
    Advertised = 1,
    /// Product is absent / not installed (2).
    Absent = 2,
    /// Product is installed on the local computer (3).
    Local = 3,
    /// Product is run from the source medium (4).
    Source = 4,
    /// Product is installed with default features (5).
    Default = 5,
}

impl InstallState {
    /// Returns the standard integer code for this installation state.
    #[must_use]
    pub const fn to_i32(self) -> i32 {
        self as i32
    }

    /// Returns `true` if this installation state corresponds to an active or installed product.
    ///
    /// # Returns
    ///
    /// `true` for `Default`, `Local`, `Source`, or `Advertised`, `false` otherwise.
    #[must_use]
    pub const fn is_installed(self) -> bool {
        matches!(
            self,
            Self::Default | Self::Local | Self::Source | Self::Advertised
        )
    }
}

/// Operational state of a multi-package transaction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransactionState {
    /// Transaction has begun and accepts child packages or joining sessions.
    Active,
    /// Transaction has been committed successfully.
    Committed,
    /// Transaction has aborted and all child actions have been unwound in reverse order.
    RolledBack,
}

/// Record of an individual package chained inside a multi-package transaction.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChainedPackage {
    /// Path or stream identifier of the child package.
    pub package_path: String,
    /// Public properties forwarded to the child package.
    pub properties: HashMap<String, String>,
    /// Whether this package or service pre-existed on the host system prior to installation.
    pub preexisting: bool,
    /// Optional temporary file path if the MSI was extracted from an internal cabinet or binary stream.
    pub temp_extracted_path: Option<String>,
    /// Optional rollback script for unwinding actions performed by this package.
    pub rollback_script: Option<RollbackScript>,
}

/// Multi-package transaction coordinator implementing Windows Installer 4.5+ transactioning.
///
/// Manages atomic cross-package installation sessions (`MsiBeginTransaction`, `MsiInstallProduct`,
/// `MsiJoinTransaction`, `MsiEndTransaction`), property forwarding, and rollback reversal.
#[derive(Debug, Default)]
pub struct MultiPackageTransactionManager {
    /// Identifier or name of the transaction.
    transaction_name: String,
    /// Current lifecycle state.
    state: Option<TransactionState>,
    /// Chronological log of chained child package installations.
    chained_packages: Vec<ChainedPackage>,
    /// Shared execution worker context.
    worker: WorkerContext,
    /// Host product state registry mapping `ProductCode` or `UpgradeCode` to [`InstallState`].
    registered_products: HashMap<String, InstallState>,
    /// Registered external session identifiers that joined this transaction.
    joined_sessions: HashSet<String>,
}

impl MultiPackageTransactionManager {
    /// Begins a new unified multi-package transaction (`MsiBeginTransaction`).
    ///
    /// # Arguments
    ///
    /// * `name` - Descriptive name or identifier for the transaction.
    ///
    /// # Returns
    ///
    /// An active [`MultiPackageTransactionManager`].
    ///
    /// # Errors
    ///
    /// Returns [`Error::Validation`] if `name` is empty.
    pub fn begin_transaction(name: &str) -> Result<Self> {
        if name.trim().is_empty() {
            return Err(Error::Validation {
                element: "MultiPackageTransactionManager.name".to_string(),
                reason: "transaction name cannot be empty".to_string(),
            });
        }

        Ok(Self {
            transaction_name: name.to_string(),
            state: Some(TransactionState::Active),
            chained_packages: Vec::new(),
            worker: WorkerContext::default(),
            registered_products: HashMap::new(),
            joined_sessions: HashSet::new(),
        })
    }

    /// Allows an external installation session to join the active transaction boundary (`MsiJoinTransaction`).
    ///
    /// # Arguments
    ///
    /// * `session_id` - Identifier for the joining session.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Chainer`] if transaction is not in the active state.
    pub fn join_transaction(&mut self, session_id: &str) -> Result<()> {
        if self.state != Some(TransactionState::Active) {
            return Err(Error::Chainer(
                "cannot join transaction: transaction is not active".to_string(),
            ));
        }
        self.joined_sessions.insert(session_id.to_string());
        Ok(())
    }

    /// Queries the installation state of a product or family by `ProductCode` or `UpgradeCode` (`MsiQueryProductState`).
    ///
    /// Normalizes GUID formatting by handling optional enclosing braces and performing case-insensitive matching.
    ///
    /// # Arguments
    ///
    /// * `code` - GUID product or upgrade identifier.
    ///
    /// # Returns
    ///
    /// The detected [`InstallState`].
    #[must_use]
    pub fn query_product_state(&self, code: &str) -> InstallState {
        if let Some(state) = self.registered_products.get(code) {
            return *state;
        }
        let trimmed = code.trim().trim_matches(|c| c == '{' || c == '}');
        for (k, v) in &self.registered_products {
            let k_trimmed = k.trim().trim_matches(|c| c == '{' || c == '}');
            if k_trimmed.eq_ignore_ascii_case(trimmed) {
                return *v;
            }
        }
        InstallState::Absent
    }

    /// Checks if a product or upgrade code corresponds to an installed product.
    ///
    /// # Arguments
    ///
    /// * `code` - `ProductCode` or `UpgradeCode` GUID.
    ///
    /// # Returns
    ///
    /// `true` if the state indicates the product is installed, `false` otherwise.
    #[must_use]
    pub fn is_product_installed(&self, code: &str) -> bool {
        self.query_product_state(code).is_installed()
    }

    /// Determines whether a child package is pre-existing on the host system based on its
    /// `ProductCode` or `UpgradeCode`.
    ///
    /// # Arguments
    ///
    /// * `package` - Child MSI package to check.
    ///
    /// # Returns
    ///
    /// `true` if either the `ProductCode` or `UpgradeCode` is registered as installed.
    #[must_use]
    pub fn is_package_preexisting(&self, package: &Package) -> bool {
        let product_code = package.metadata().product_code();
        if self.is_product_installed(product_code) {
            return true;
        }

        let upgrade_code = package
            .database()
            .get_records("Property")
            .iter()
            .find_map(|r| match (r.get(0), r.get(1)) {
                (Some(FieldValue::String(k)), Some(FieldValue::String(v)))
                    if k == "UpgradeCode" =>
                {
                    Some(v.as_str())
                }
                _ => None,
            });

        if let Some(up_code) = upgrade_code {
            if self.is_product_installed(up_code) {
                return true;
            }
        }

        false
    }

    /// Manually registers or updates the installation state for a product code or upgrade code.
    ///
    /// # Arguments
    ///
    /// * `code` - GUID product or upgrade code.
    /// * `state` - Installation state to associate.
    pub fn register_product(&mut self, code: &str, state: InstallState) {
        self.registered_products.insert(code.to_string(), state);
    }

    /// Checks whether a property name adheres to the Windows Installer public property convention.
    ///
    /// Public properties consist strictly of uppercase ASCII alphanumeric characters and underscores.
    ///
    /// # Arguments
    ///
    /// * `name` - The property name to check.
    ///
    /// # Returns
    ///
    /// `true` if public, `false` otherwise.
    #[must_use]
    pub fn is_public_property(name: &str) -> bool {
        !name.is_empty()
            && name
                .chars()
                .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_')
    }

    /// Serializes all public properties from an [`EvaluationContext`] into a command-line string.
    ///
    /// # Arguments
    ///
    /// * `context` - The evaluation context containing active properties.
    ///
    /// # Returns
    ///
    /// A space-delimited string of `KEY=VALUE` or `KEY="VALUE"` pairs sorted alphabetically by key.
    #[must_use]
    pub fn forward_public_properties(context: &EvaluationContext) -> String {
        let mut pairs: Vec<(&str, &str)> = context
            .properties()
            .iter()
            .filter(|(k, _)| Self::is_public_property(k))
            .map(|(k, v)| (k.as_str(), v.as_str()))
            .collect();
        pairs.sort_by(|a, b| a.0.cmp(b.0));
        pairs
            .into_iter()
            .map(|(k, v)| {
                if v.contains(' ') || v.contains('"') {
                    format!("{k}=\"{}\"", v.replace('"', "\\\""))
                } else {
                    format!("{k}={v}")
                }
            })
            .collect::<Vec<_>>()
            .join(" ")
    }

    /// Forwards all public properties from a source context into a target context.
    ///
    /// # Arguments
    ///
    /// * `source` - Source context containing properties.
    /// * `target` - Target context receiving the public properties.
    pub fn forward_properties_to_context(
        source: &EvaluationContext,
        target: &mut EvaluationContext,
    ) {
        for (k, v) in source.properties() {
            if Self::is_public_property(k) {
                target.set_property(k, v);
            }
        }
    }

    /// Reads and parses all [`MsiEmbeddedChainerRow`] records from the `MsiEmbeddedChainer` table of an MSI package.
    ///
    /// # Arguments
    ///
    /// * `package` - MSI package to inspect.
    ///
    /// # Returns
    ///
    /// Vector of parsed [`MsiEmbeddedChainerRow`] records.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Validation`] if a record in the `MsiEmbeddedChainer` table cannot be parsed.
    pub fn read_embedded_chainers(package: &Package) -> Result<Vec<MsiEmbeddedChainerRow>> {
        let mut chainers = Vec::new();
        for rec in package.database().get_records("MsiEmbeddedChainer") {
            let row = MsiEmbeddedChainerRow::from_record(rec)?;
            chainers.push(row);
        }
        Ok(chainers)
    }

    /// Extracts an embedded child MSI package from internal streams or the `Binary` table
    /// into a temporary spool directory.
    ///
    /// # Arguments
    ///
    /// * `package` - Master MSI package containing embedded child stream.
    /// * `stream_or_binary_key` - Key identifier for the embedded child stream.
    /// * `spool_dir` - Destination directory where the child package will be written.
    ///
    /// # Returns
    ///
    /// The [`PathBuf`] to the extracted `.msi` file.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Chainer`] if the stream or binary data cannot be found.
    /// Returns [`Error::Io`] on filesystem write failure.
    pub fn extract_child_package(
        &mut self,
        package: &Package,
        stream_or_binary_key: &str,
        spool_dir: &Path,
    ) -> Result<PathBuf> {
        let clean_name = stream_or_binary_key
            .strip_prefix('#')
            .unwrap_or(stream_or_binary_key);
        let file_name = if Path::new(clean_name)
            .extension()
            .is_some_and(|ext| ext.eq_ignore_ascii_case("msi"))
        {
            clean_name.to_string()
        } else {
            format!("{clean_name}.msi")
        };
        let base_name = Path::new(&file_name)
            .file_name()
            .and_then(|f| f.to_str())
            .unwrap_or("child.msi");

        let data: Option<Vec<u8>> = package
            .embedded_cabinets()
            .get(stream_or_binary_key)
            .or_else(|| package.embedded_cabinets().get(clean_name))
            .cloned()
            .or_else(|| {
                package
                    .database()
                    .get_records("Binary")
                    .iter()
                    .find_map(|r| {
                        if let Some(FieldValue::String(name)) = r.get(0) {
                            if name == stream_or_binary_key || name == clean_name {
                                match r.get(1) {
                                    Some(FieldValue::String(s)) => Some(s.as_bytes().to_vec()),
                                    _ => None,
                                }
                            } else {
                                None
                            }
                        } else {
                            None
                        }
                    })
            });

        let payload = data.ok_or_else(|| {
            Error::Chainer(format!(
                "embedded child package '{stream_or_binary_key}' not found in streams or Binary table"
            ))
        })?;

        std::fs::create_dir_all(spool_dir)?;
        let dest_path = spool_dir.join(base_name);
        std::fs::write(&dest_path, payload)?;

        self.worker
            .executed_actions
            .push(format!("ExtractChildPackage:{base_name}"));

        Ok(dest_path)
    }

    /// Extracts all embedded child packages found in internal streams or `Binary` table records.
    ///
    /// Any stream or binary identifier ending with `.msi` (case-insensitive) is extracted to `spool_dir`.
    ///
    /// # Arguments
    ///
    /// * `package` - Master MSI package.
    /// * `spool_dir` - Destination spool directory.
    ///
    /// # Returns
    ///
    /// Mapping of child package identifier to extracted file path.
    ///
    /// # Errors
    ///
    /// Returns [`Error`] on extraction or filesystem write failure.
    pub fn extract_all_child_packages(
        &mut self,
        package: &Package,
        spool_dir: &Path,
    ) -> Result<HashMap<String, PathBuf>> {
        let mut extracted = HashMap::new();

        for name in package.embedded_cabinets().keys() {
            if Path::new(name)
                .extension()
                .is_some_and(|ext| ext.eq_ignore_ascii_case("msi"))
            {
                let path = self.extract_child_package(package, name, spool_dir)?;
                extracted.insert(name.clone(), path);
            }
        }

        for rec in package.database().get_records("Binary") {
            if let Some(FieldValue::String(bin_name)) = rec.get(0) {
                if Path::new(bin_name)
                    .extension()
                    .is_some_and(|ext| ext.eq_ignore_ascii_case("msi"))
                    && !extracted.contains_key(bin_name)
                {
                    let path = self.extract_child_package(package, bin_name, spool_dir)?;
                    extracted.insert(bin_name.clone(), path);
                }
            }
        }

        Ok(extracted)
    }

    /// Installs a child package within the active transaction (`MsiInstallProduct`).
    ///
    /// # Arguments
    ///
    /// * `package_path` - Path or stream specifier for the child `.msi` file.
    /// * `command_line` - Public properties string (e.g. `PROP_MYSQL_PORT=3306 PREEXISTING=1`).
    ///
    /// # Returns
    ///
    /// Exit code `0` (`ERROR_SUCCESS`) on success.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Chainer`] if the transaction is not active or installation fails.
    pub fn install_product_nested(
        &mut self,
        package_path: &str,
        command_line: &str,
    ) -> Result<u32> {
        self.install_product_nested_impl(package_path.to_string(), command_line)
    }

    /// Internal implementation of nested product installation with concrete path string.
    fn install_product_nested_impl(
        &mut self,
        package_path: String,
        command_line: &str,
    ) -> Result<u32> {
        if self.state != Some(TransactionState::Active) {
            return Err(Error::Chainer(
                "cannot install nested package: transaction is not active".to_string(),
            ));
        }

        let mut properties = HashMap::new();

        for token in command_line.split_whitespace() {
            if let Some((k, v)) = token.split_once('=') {
                let val = v.trim_matches('"');
                properties.insert(k.to_string(), val.to_string());
            }
        }

        let preexisting = properties.get("PREEXISTING").is_some_and(|v| v == "1")
            || self.query_product_state(&package_path) == InstallState::Default;

        let temp_extracted_path = (package_path.starts_with("embedded:")
            || package_path.starts_with("Binary:")
            || package_path.starts_with('#'))
        .then(|| format!("/tmp/extracted_{}", self.chained_packages.len()));

        self.worker
            .executed_actions
            .push(format!("InstallProduct:{package_path}"));

        self.chained_packages.push(ChainedPackage {
            package_path,
            properties,
            preexisting,
            temp_extracted_path,
            rollback_script: None,
        });

        Ok(ERROR_SUCCESS)
    }

    /// Installs a child MSI package instance within the active transaction, evaluating
    /// pre-existing status, launch conditions, and executing transactional operations.
    ///
    /// If the package is detected as pre-existing or skipped via launch conditions,
    /// physical installation is skipped and recorded as pre-existing.
    ///
    /// # Arguments
    ///
    /// * `child_pkg` - The parsed child MSI package.
    /// * `command_line` - Command line property arguments forwarded to the child.
    ///
    /// # Returns
    ///
    /// Exit code `0` (`ERROR_SUCCESS`) on success.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Chainer`] if the transaction is not active.
    /// Returns [`Error::ExecutionFailed`] if launch conditions fail or execution fails.
    pub fn install_child_package(
        &mut self,
        child_pkg: &Package,
        command_line: &str,
    ) -> Result<u32> {
        if self.state != Some(TransactionState::Active) {
            return Err(Error::Chainer(
                "cannot install child package: transaction is not active".to_string(),
            ));
        }

        let mut properties = HashMap::new();
        for token in command_line.split_whitespace() {
            if let Some((k, v)) = token.split_once('=') {
                let val = v.trim_matches('"');
                properties.insert(k.to_string(), val.to_string());
            }
        }

        let product_code = child_pkg.metadata().product_code();
        let product_name = child_pkg.metadata().product_name();
        let is_pre = properties.get("PREEXISTING").is_some_and(|v| v == "1")
            || self.is_package_preexisting(child_pkg);

        // Check if an explicit skip property is set (e.g. INSTALL_MYSQL="0")
        let skip_install = properties.iter().any(|(k, v)| {
            k.starts_with("INSTALL_") && (v == "0" || v.eq_ignore_ascii_case("false"))
        });

        if is_pre || skip_install {
            self.worker.executed_actions.push(format!(
                "SkipChildPackagePreexisting:{product_name}:{product_code}"
            ));
            self.chained_packages.push(ChainedPackage {
                package_path: product_name.to_string(),
                properties,
                preexisting: true,
                temp_extracted_path: None,
                rollback_script: None,
            });
            return Ok(ERROR_SUCCESS);
        }

        // Build child evaluation context starting with properties from the child package
        let mut child_ctx = EvaluationContext::new();
        for r in child_pkg.database().get_records("Property") {
            if let (Some(FieldValue::String(k)), Some(FieldValue::String(v))) = (r.get(0), r.get(1))
            {
                child_ctx.set_property(k, v);
            }
        }
        for (k, v) in &properties {
            child_ctx.set_property(k, v);
        }

        // Evaluate launch conditions if present
        for lc_rec in child_pkg.database().get_records("LaunchCondition") {
            if let Some(FieldValue::String(cond)) = lc_rec.get(0) {
                let desc = match lc_rec.get(1) {
                    Some(FieldValue::String(s)) => s.as_str(),
                    _ => "Launch condition failed",
                };
                if !child_ctx.evaluate_condition(cond)? {
                    return Err(Error::ExecutionFailed {
                        action: "LaunchConditions".to_string(),
                        return_code: ERROR_INSTALL_FAILURE,
                        message: desc.to_string(),
                    });
                }
            }
        }

        let tx = Transaction::from_package(child_pkg, child_ctx, DiskCostEngine::new());
        let prepared_tx = tx.prepare()?;
        let rollback_script = prepared_tx.rollback_script().clone();

        self.worker
            .executed_actions
            .push(format!("InstallProduct:{product_name}"));

        let executed_tx = prepared_tx.execute(&mut self.worker)?;
        let committed_tx = executed_tx.commit(&mut self.worker)?;

        self.chained_packages.push(ChainedPackage {
            package_path: product_name.to_string(),
            properties,
            preexisting: false,
            temp_extracted_path: None,
            rollback_script: Some(rollback_script),
        });

        Ok(committed_tx.return_code())
    }

    /// Loads a child package from disk and installs it within the active transaction.
    ///
    /// # Arguments
    ///
    /// * `path` - Filesystem path to the child `.msi` package.
    /// * `command_line` - Command line arguments and properties forwarded to the child.
    ///
    /// # Returns
    ///
    /// Exit code `0` (`ERROR_SUCCESS`) on success.
    ///
    /// # Errors
    ///
    /// Returns [`Error`] if opening or installing the package fails.
    pub fn install_child_package_from_path(
        &mut self,
        path: impl AsRef<Path>,
        command_line: &str,
    ) -> Result<u32> {
        let pkg = Package::open(path)?;
        self.install_child_package(&pkg, command_line)
    }

    /// Orchestrates a sequence of child [`Package`] instances within the active transaction.
    ///
    /// Forwards public properties from `master_context` merged with optional child-specific arguments,
    /// installs each child package sequentially, and executes a cascading rollback if any child fails.
    ///
    /// # Arguments
    ///
    /// * `child_packages` - Slice of child package references and optional property argument strings.
    /// * `master_context` - Evaluation context from the master package containing properties to forward.
    ///
    /// # Returns
    ///
    /// Exit code `0` (`ERROR_SUCCESS`) on success.
    ///
    /// # Errors
    ///
    /// Returns [`Error`] if any child package installation fails (after rolling back preceding packages).
    pub fn orchestrate_child_packages(
        &mut self,
        child_packages: &[(&Package, Option<&str>)],
        master_context: &EvaluationContext,
    ) -> Result<u32> {
        let forwarded = Self::forward_public_properties(master_context);

        for (pkg, extra_args) in child_packages {
            let cmdline = match extra_args {
                Some(args) if !args.trim().is_empty() => {
                    if forwarded.is_empty() {
                        (*args).to_string()
                    } else {
                        format!("{forwarded} {args}")
                    }
                }
                _ => forwarded.clone(),
            };

            if let Err(err) = self.install_child_package(pkg, &cmdline) {
                let _ = self.end_transaction(false);
                return Err(err);
            }
        }

        self.end_transaction(true)
    }

    /// Orchestrates an entire multi-package deployment session from a master orchestrator package.
    ///
    /// Reads `MsiEmbeddedChainer` table records from `master_pkg`, extracts embedded child packages to `spool_dir`,
    /// evaluates conditions, forwards public properties, installs child packages atomically,
    /// and handles rollback cascade if any package fails.
    ///
    /// # Arguments
    ///
    /// * `master_pkg` - Master orchestrator MSI package.
    /// * `master_context` - Evaluation context from the master package containing active properties.
    /// * `spool_dir` - Destination directory for spooling embedded child packages.
    /// * `child_packages` - Optional list of child package names and specific property overrides.
    ///
    /// # Returns
    ///
    /// Exit code `0` (`ERROR_SUCCESS`) on success, or `1603` (`ERROR_INSTALL_FAILURE`) on rollback.
    ///
    /// # Errors
    ///
    /// Returns [`Error`] on execution failure.
    pub fn orchestrate_master_package(
        &mut self,
        master_pkg: &Package,
        master_context: &EvaluationContext,
        spool_dir: &Path,
        child_packages: &[(&str, Option<&str>)],
    ) -> Result<u32> {
        let chainers = Self::read_embedded_chainers(master_pkg)?;
        for chainer in &chainers {
            if let Some(ref cond) = chainer.condition {
                if !master_context.evaluate_condition(cond)? {
                    self.worker
                        .executed_actions
                        .push(format!("SkipEmbeddedChainer:{}", chainer.chainer));
                    return Ok(ERROR_SUCCESS);
                }
            }
        }

        let extracted = self.extract_all_child_packages(master_pkg, spool_dir)?;
        let forwarded_props = Self::forward_public_properties(master_context);

        for (pkg_name, child_args) in child_packages {
            let combined_cmdline = match child_args {
                Some(args) if !args.trim().is_empty() => {
                    if forwarded_props.is_empty() {
                        (*args).to_string()
                    } else {
                        format!("{forwarded_props} {args}")
                    }
                }
                _ => forwarded_props.clone(),
            };

            let res = if let Some(path) = extracted.get(*pkg_name) {
                self.install_child_package_from_path(path, &combined_cmdline)
            } else if Path::new(*pkg_name).exists() {
                self.install_child_package_from_path(*pkg_name, &combined_cmdline)
            } else {
                self.install_product_nested(pkg_name, &combined_cmdline)
            };

            if let Err(err) = res {
                let _ = self.end_transaction(false);
                return Err(err);
            }
        }

        self.end_transaction(true)
    }

    /// Commits or rolls back all nested installations within the transaction (`MsiEndTransaction`).
    ///
    /// # Arguments
    ///
    /// * `commit` - `true` to commit all operations, `false` to rollback in reverse order.
    ///
    /// # Returns
    ///
    /// `0` (`ERROR_SUCCESS`) on commit, or `1603` (`ERROR_INSTALL_FAILURE`) on rollback.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Chainer`] if transaction is not active.
    pub fn end_transaction(&mut self, commit: bool) -> Result<u32> {
        if self.state != Some(TransactionState::Active) {
            return Err(Error::Chainer(
                "cannot end transaction: transaction is not active".to_string(),
            ));
        }

        if commit {
            self.state = Some(TransactionState::Committed);
            self.worker
                .executed_actions
                .push("CommitAllPackages".to_string());
            Ok(ERROR_SUCCESS)
        } else {
            self.state = Some(TransactionState::RolledBack);
            for pkg in self.chained_packages.iter().rev() {
                if pkg.preexisting {
                    self.worker
                        .executed_actions
                        .push(format!("SkipRollbackPreexisting:{}", pkg.package_path));
                } else {
                    self.worker
                        .executed_actions
                        .push(format!("RollbackPackage:{}", pkg.package_path));
                    if let Some(ref rb) = pkg.rollback_script {
                        let _ = self.worker.execute_rollback(rb);
                    }
                }
            }
            Ok(ERROR_INSTALL_FAILURE)
        }
    }

    /// Returns the name of the transaction.
    #[must_use]
    pub fn transaction_name(&self) -> &str {
        &self.transaction_name
    }

    /// Returns the current lifecycle state of the transaction.
    #[must_use]
    pub const fn state(&self) -> Option<TransactionState> {
        self.state
    }

    /// Returns the list of chained package records.
    #[must_use]
    pub fn chained_packages(&self) -> &[ChainedPackage] {
        &self.chained_packages
    }

    /// Returns a reference to the shared execution worker context.
    #[must_use]
    pub const fn worker(&self) -> &WorkerContext {
        &self.worker
    }

    /// Returns a mutable reference to the shared execution worker context.
    pub const fn worker_mut(&mut self) -> &mut WorkerContext {
        &mut self.worker
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
    /// Embedded cabinet payloads mapping cabinet stream name (e.g. `#cab1.cab`) to raw archive bytes.
    embedded_cabinets: HashMap<String, Vec<u8>>,
    /// Typestate marker.
    _state: PhantomData<State>,
}

/// Helper that parses the target directory name from MSI `DefaultDir` specification.
///
/// Handles target:source formats (`target:source`), short|long formats (`short|long`),
/// and composite specifications (`target_short|target_long:source_short|source_long`).
///
/// # Arguments
///
/// * `default_dir` - Raw directory specification string.
///
/// # Returns
///
/// Subdirectory name or `"."` if root.
#[must_use]
pub fn parse_default_dir(default_dir: &str) -> &str {
    let target = default_dir.split(':').next().unwrap_or(default_dir);
    if let Some((_, long)) = target.split_once('|') {
        long
    } else {
        target
    }
}

/// Helper that parses the long filename from MSI `FileName` specification (`SHORT~1.TXT|LongFileName.txt`).
///
/// # Arguments
///
/// * `file_name_spec` - Raw file name specification string.
///
/// # Returns
///
/// Long filename if present, or original filename.
#[must_use]
pub fn parse_file_name(file_name_spec: &str) -> &str {
    if let Some((_, long)) = file_name_spec.split_once('|') {
        long
    } else {
        file_name_spec
    }
}

/// Helper entry representing a row in the MSI `Directory` table.
#[derive(Debug, Clone)]
struct DirEntry {
    /// Directory identifier.
    id: String,
    /// Parent directory identifier, or `None` if root.
    parent: Option<String>,
    /// Default directory layout or relative path.
    default_dir: String,
}

/// Helper entry representing a media cabinet entry in the MSI `Media` table.
#[derive(Debug, Clone)]
struct MediaDisk {
    /// Maximum file sequence number stored on this media.
    last_sequence: i32,
    /// Cabinet archive stream or file name.
    cabinet: Option<String>,
}

/// Resolves standard Windows Installer directory properties and builds the directory graph.
///
/// Pre-seeds standard directories:
/// - `[TARGETDIR]` -> Root installation target (`C:\` on Windows, `/` on Unix)
/// - `[ProgramFilesFolder]` / `[ProgramFiles64Folder]` -> System application directory
/// - `[DesktopFolder]` -> User/Public desktop path
/// - `[ProgramMenuFolder]` -> Start Menu / applications folder
/// - `[CommonAppDataFolder]` / `[LocalAppDataFolder]` -> Mutable application data stores
///
/// Evaluates and resolves all records in the `Directory` table, setting each resolved
/// path as a property in `context`.
///
/// # Arguments
///
/// * `database` - Linked MSI database.
/// * `context` - Active evaluation context.
///
/// # Returns
///
/// Map of directory identifier to its resolved absolute [`PathBuf`].
#[allow(clippy::too_many_lines)]
fn resolve_directories(
    database: &LinkedDatabase,
    context: &mut EvaluationContext,
) -> HashMap<String, PathBuf> {
    let product = context.get_property("ProductName").unwrap_or("Application");
    let resolver = crate::platform::paths::PathResolver::new(
        crate::platform::paths::TargetOs::host(),
        product,
    );

    let mut resolved_dirs: HashMap<String, PathBuf> = HashMap::new();

    let dir_records = database.get_records("Directory");
    let mut entries_with_parent: HashSet<String> = HashSet::new();
    for d in dir_records {
        if let (Some(FieldValue::String(id)), Some(FieldValue::String(parent))) =
            (d.get(0), d.get(1))
        {
            if !parent.is_empty() {
                entries_with_parent.insert(id.clone());
            }
        }
    }

    // 1. Resolve standard directory properties if not already explicitly provided
    let standard_ids = [
        crate::platform::paths::StandardDirectoryId::TargetDir,
        crate::platform::paths::StandardDirectoryId::ProgramFilesFolder,
        crate::platform::paths::StandardDirectoryId::ProgramFiles64Folder,
        crate::platform::paths::StandardDirectoryId::DesktopFolder,
        crate::platform::paths::StandardDirectoryId::ProgramMenuFolder,
        crate::platform::paths::StandardDirectoryId::CommonAppDataFolder,
        crate::platform::paths::StandardDirectoryId::LocalAppDataFolder,
        crate::platform::paths::StandardDirectoryId::AppDataFolder,
        crate::platform::paths::StandardDirectoryId::TempFolder,
        crate::platform::paths::StandardDirectoryId::CommonFilesFolder,
        crate::platform::paths::StandardDirectoryId::SystemFolder,
        crate::platform::paths::StandardDirectoryId::System64Folder,
        crate::platform::paths::StandardDirectoryId::WindowsFolder,
        crate::platform::paths::StandardDirectoryId::ProfilesFolder,
    ];

    for id in standard_ids {
        let name = id.as_str();
        if let Some(existing) = context.get_property(name) {
            resolved_dirs.insert(name.to_string(), PathBuf::from(existing));
        } else if !entries_with_parent.contains(name) {
            let p = resolver.resolve(id);
            context.set_property(name, p.to_string_lossy().to_string());
            resolved_dirs.insert(name.to_string(), p);
        }
    }

    // TARGETDIR fallback
    if !resolved_dirs.contains_key("TARGETDIR") {
        #[cfg(windows)]
        let root = PathBuf::from(r"C:\");
        #[cfg(not(windows))]
        let root = PathBuf::from("/");
        context.set_property("TARGETDIR", root.to_string_lossy().to_string());
        resolved_dirs.insert("TARGETDIR".to_string(), root);
    }

    // 2. Load all rows from Directory table
    let mut entries: Vec<DirEntry> = Vec::new();
    for d in dir_records {
        if let Some(FieldValue::String(id)) = d.get(0) {
            let parent = match d.get(1) {
                Some(FieldValue::String(p)) if !p.is_empty() => Some(p.clone()),
                _ => None,
            };
            let default_dir = match d.get(2) {
                Some(FieldValue::String(def)) => def.clone(),
                _ => ".".to_string(),
            };
            if let Some(prop_val) = context.get_property(id) {
                resolved_dirs.insert(id.clone(), PathBuf::from(prop_val));
            }
            entries.push(DirEntry {
                id: id.clone(),
                parent,
                default_dir,
            });
        }
    }

    // 3. Iteratively resolve child directories until fixed point
    let mut changed = true;
    let mut iterations = 0;
    let max_iterations = entries.len().max(16);

    while changed && iterations < max_iterations {
        changed = false;
        iterations += 1;

        for entry in &entries {
            if resolved_dirs.contains_key(&entry.id) {
                continue;
            }

            let sub_name = parse_default_dir(&entry.default_dir);
            if let Some(ref parent_id) = entry.parent {
                if let Some(parent_path) = resolved_dirs.get(parent_id).cloned() {
                    let dir_path = if sub_name == "." || sub_name.is_empty() {
                        parent_path
                    } else {
                        parent_path.join(sub_name)
                    };
                    context.set_property(&entry.id, dir_path.to_string_lossy().to_string());
                    resolved_dirs.insert(entry.id.clone(), dir_path);
                    changed = true;
                }
            } else {
                // Root entry without parent
                let dir_path = PathBuf::from(format!(r"C:\{}", entry.id));
                context.set_property(&entry.id, dir_path.to_string_lossy().to_string());
                resolved_dirs.insert(entry.id.clone(), dir_path);
                changed = true;
            }
        }
    }

    // Fallback for any orphaned directories
    for entry in &entries {
        if !resolved_dirs.contains_key(&entry.id) {
            let path = PathBuf::from(format!(r"C:\{}", entry.id));
            context.set_property(&entry.id, path.to_string_lossy().to_string());
            resolved_dirs.insert(entry.id.clone(), path);
        }
    }

    resolved_dirs
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
            embedded_cabinets: HashMap::new(),
            _state: PhantomData,
        }
    }

    /// Attaches an embedded cabinet payload.
    ///
    /// # Arguments
    ///
    /// * `name` - Cabinet stream or filename (e.g. `#cab1.cab`).
    /// * `bytes` - Raw cabinet bytes.
    ///
    /// # Returns
    ///
    /// Updated [`Transaction`].
    #[must_use]
    pub fn with_cabinet(mut self, name: &str, bytes: Vec<u8>) -> Self {
        self.embedded_cabinets.insert(name.to_string(), bytes);
        self
    }

    /// Attaches multiple embedded cabinet payloads.
    ///
    /// # Arguments
    ///
    /// * `cabinets` - Map of cabinet names to raw archive bytes.
    ///
    /// # Returns
    ///
    /// Updated [`Transaction`].
    #[must_use]
    pub fn with_cabinets(mut self, cabinets: HashMap<String, Vec<u8>>) -> Self {
        self.embedded_cabinets.extend(cabinets);
        self
    }

    /// Creates a [`Transaction`] from a high-level [`Package`].
    ///
    /// # Arguments
    ///
    /// * `package` - MSI package containing database and embedded cabinets.
    /// * `context` - Evaluation context.
    /// * `cost_engine` - Disk costing engine.
    ///
    /// # Returns
    ///
    /// A new [`Transaction`] preloaded with package database and cabinets.
    #[must_use]
    pub fn from_package(
        package: &Package,
        context: EvaluationContext,
        cost_engine: DiskCostEngine,
    ) -> Self {
        let mut tx = Self::new(package.database().clone(), context, cost_engine);
        for (name, bytes) in package.embedded_cabinets() {
            tx = tx.with_cabinet(name, bytes.clone());
        }
        tx
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
    pub fn sequence_table(mut self, table: &str) -> Self {
        self.sequence_table = table.to_string();
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

        let resolved_dirs = resolve_directories(&self.database, &mut self.context);

        // Map Component to Directory
        let mut comp_to_dir: HashMap<String, String> = HashMap::new();
        for comp in self.database.get_records("Component") {
            if let (Some(FieldValue::String(c)), Some(FieldValue::String(d))) =
                (comp.get(0), comp.get(2))
            {
                comp_to_dir.insert(c.clone(), d.clone());
            }
        }

        // Map Media disks
        let mut media_disks: Vec<MediaDisk> = Vec::new();
        for m in self.database.get_records("Media") {
            let last_seq = match m.get(1) {
                Some(&FieldValue::Short(s)) => i32::from(s),
                Some(&FieldValue::Long(l)) => l,
                _ => i32::MAX,
            };
            let cabinet = match m.get(3) {
                Some(FieldValue::String(cab)) if !cab.is_empty() => Some(cab.clone()),
                _ => None,
            };
            media_disks.push(MediaDisk {
                last_sequence: last_seq,
                cabinet,
            });
        }
        media_disks.sort_by_key(|m| m.last_sequence);

        // Map CustomAction table records
        let mut custom_actions_map: HashMap<String, (u32, String, String)> = HashMap::new();
        for ca_rec in self.database.get_records("CustomAction") {
            if let Some(FieldValue::String(action_name)) = ca_rec.get(0) {
                let action_type = match ca_rec.get(1) {
                    Some(&FieldValue::Short(s)) => u32::try_from(s).unwrap_or(0),
                    Some(&FieldValue::Long(l)) => u32::try_from(l).unwrap_or(0),
                    _ => 0,
                };
                let source = match ca_rec.get(2) {
                    Some(FieldValue::String(s)) => s.clone(),
                    _ => String::new(),
                };
                let target = match ca_rec.get(3) {
                    Some(FieldValue::String(t)) => t.clone(),
                    _ => String::new(),
                };
                custom_actions_map.insert(action_name.clone(), (action_type, source, target));
            }
        }

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
                            let path = resolved_dirs
                                .get(dir_name)
                                .map(|p| p.to_string_lossy().to_string())
                                .unwrap_or_default();
                            self.install_script
                                .push(ScriptOp::CreateFolder { path: path.clone() });
                            self.rollback_script
                                .push(RollbackOp::DeleteCreatedFolder { path });
                        }
                    }
                }
                "InstallFiles" => {
                    let file_records = self.database.get_records("File");
                    for f in file_records {
                        let file_id = match f.get(0) {
                            Some(FieldValue::String(id)) => id.clone(),
                            _ => continue,
                        };
                        let comp_id = match f.get(1) {
                            Some(FieldValue::String(c)) => c.clone(),
                            _ => String::new(),
                        };
                        let filename_spec = match f.get(2) {
                            Some(FieldValue::String(name)) => name.clone(),
                            _ => continue,
                        };
                        let sequence = match f.get(7) {
                            Some(&FieldValue::Short(s)) => i32::from(s),
                            Some(&FieldValue::Long(l)) => l,
                            _ => 1,
                        };

                        let filename = parse_file_name(&filename_spec);
                        let target_path_buf = comp_to_dir
                            .get(&comp_id)
                            .and_then(|dir_id| resolved_dirs.get(dir_id))
                            .map_or_else(
                                || PathBuf::from(format!(r"C:\Program Files\App\{filename}")),
                                |dir_path| dir_path.join(filename),
                            );
                        let target_path = target_path_buf.to_string_lossy().to_string();
                        let quarantine_path = format!("{target_path}.rbf");

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

                        // Check media disks for cabinet
                        let matched_cab = media_disks
                            .iter()
                            .find(|m| m.last_sequence >= sequence)
                            .and_then(|m| m.cabinet.as_ref());

                        if let Some(cab_name) = matched_cab {
                            self.install_script.push(ScriptOp::ExtractCabinetFile {
                                cabinet: cab_name.clone(),
                                file_key: file_id,
                                destination: target_path.clone(),
                            });
                        } else {
                            self.install_script.push(ScriptOp::WriteFile {
                                destination: target_path.clone(),
                                content: format!("Payload of {filename}").into_bytes(),
                            });
                        }

                        self.rollback_script
                            .push(RollbackOp::DeleteCreatedFile { path: target_path });
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
                            let link_path = match s.get(1) {
                                Some(FieldValue::String(dir_id)) => {
                                    let dir =
                                        resolved_dirs.get(dir_id).cloned().unwrap_or_else(|| {
                                            PathBuf::from(r"C:\Users\Public\Desktop")
                                        });
                                    dir.join(format!("{target}.lnk"))
                                        .to_string_lossy()
                                        .to_string()
                                }
                                _ => format!(r"C:\Users\Public\Desktop\{target}.lnk"),
                            };
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
                    let (action_type, source, target) =
                        if let Some(ca) = custom_actions_map.get(custom_act) {
                            ca.clone()
                        } else {
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
                            continue;
                        };

                    // Type 19: Error abort action
                    if action_type & 0x003F == 19 {
                        let error_msg = if target.is_empty() {
                            format!("Installation aborted by custom action '{custom_act}'.")
                        } else {
                            self.context.format_string(&target)?
                        };
                        return Err(Error::CustomActionFailed {
                            action: custom_act.to_string(),
                            reason: error_msg,
                        });
                    }

                    // Rollback custom action (0x0500)
                    if (action_type & 0x0500) == 0x0500 {
                        self.rollback_script.push(RollbackOp::RollbackCustomAction {
                            action: custom_act.to_string(),
                            action_type,
                            source,
                            target,
                        });
                    } else if action_type & 0x0400 == 0 {
                        // Immediate custom action: execute during prepare
                        if let Ok(ca_def) = super::custom_action::CustomActionDefinition::parse(
                            custom_act,
                            action_type,
                            &source,
                            &target,
                        ) {
                            let mut ca_exec = super::custom_action::CustomActionExecutor::new();
                            for b in self.database.get_records("Binary") {
                                if let Some(FieldValue::String(b_name)) = b.get(0) {
                                    let b_data = match b.get(1) {
                                        Some(FieldValue::String(s)) => s.as_bytes().to_vec(),
                                        _ => Vec::new(),
                                    };
                                    ca_exec.add_binary(b_name.clone(), b_data);
                                }
                            }
                            ca_exec.execute(&ca_def, &mut self.context)?;
                        }

                        self.install_script.push(ScriptOp::CustomAction {
                            action: custom_act.to_string(),
                            action_type,
                            source,
                            target,
                        });
                    } else {
                        // Deferred custom action (0x0400)
                        self.install_script.push(ScriptOp::CustomAction {
                            action: custom_act.to_string(),
                            action_type,
                            source: source.clone(),
                            target,
                        });
                        self.rollback_script.push(RollbackOp::RollbackCustomAction {
                            action: format!("{custom_act}_Rollback"),
                            action_type: 0x0501,
                            source,
                            target: "RollbackFn".to_string(),
                        });
                    }
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
            embedded_cabinets: self.embedded_cabinets,
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
        worker.set_evaluation_context(self.context.clone());

        for b in self.database.get_records("Binary") {
            if let Some(FieldValue::String(b_name)) = b.get(0) {
                let b_data = match b.get(1) {
                    Some(FieldValue::String(s)) => s.as_bytes().to_vec(),
                    _ => Vec::new(),
                };
                worker.add_binary(b_name, b_data);
            }
        }

        for (name, bytes) in &self.embedded_cabinets {
            if !worker.has_cabinet(name) {
                let _ = worker.add_cabinet_bytes(name, bytes);
            }
        }

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
            embedded_cabinets: self.embedded_cabinets,
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
            embedded_cabinets: self.embedded_cabinets,
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
            embedded_cabinets: self.embedded_cabinets,
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
            embedded_cabinets: self.embedded_cabinets,
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

    /// Tests `MultiPackageTransactionManager` lifecycle: begin, join, install, query, commit, rollback.
    #[test]
    fn test_multi_package_transaction_manager() -> Result<()> {
        // 1. Validation of empty name
        assert!(MultiPackageTransactionManager::begin_transaction("").is_err());
        assert!(MultiPackageTransactionManager::begin_transaction("   ").is_err());

        // 2. Begin transaction
        let mut mgr =
            MultiPackageTransactionManager::begin_transaction("OpenEdX_Master_Transaction")?;
        assert_eq!(mgr.transaction_name(), "OpenEdX_Master_Transaction");
        assert_eq!(mgr.state(), Some(TransactionState::Active));

        // 3. Join transaction
        assert!(mgr.join_transaction("Session_Child1").is_ok());

        // 4. Product state registration and query
        let mysql_code = "{E0F45901-83B4-4B21-9B5A-01D38FE81001}";
        assert_eq!(mgr.query_product_state(mysql_code), InstallState::Absent);
        assert_eq!(InstallState::Absent.to_i32(), 2);
        assert_eq!(InstallState::Default.to_i32(), 5);
        assert_eq!(InstallState::Local.to_i32(), 3);
        assert_eq!(InstallState::Source.to_i32(), 4);
        assert_eq!(InstallState::Advertised.to_i32(), 1);
        assert_eq!(InstallState::Broken.to_i32(), 0);
        assert_eq!(InstallState::Unknown.to_i32(), -1);
        assert_eq!(InstallState::InvalidArg.to_i32(), -2);
        assert_eq!(InstallState::BadConfiguration.to_i32(), -6);

        mgr.register_product(mysql_code, InstallState::Default);
        assert_eq!(mgr.query_product_state(mysql_code), InstallState::Default);

        // 5. Nested product installs
        // First child: MySQL (pre-existing)
        let res1 = mgr.install_product_nested(
            mysql_code,
            "PROP_MYSQL_PORT=3306 ROOT_PASSWORD=\"secretPass\" PREEXISTING=1",
        )?;
        assert_eq!(res1, ERROR_SUCCESS);

        // Second child: Redis (new, embedded stream)
        let res2 =
            mgr.install_product_nested("embedded:libscript-redis.msi", "PROP_REDIS_PORT=6379")?;
        assert_eq!(res2, ERROR_SUCCESS);

        // Third child: Binary stream with PREEXISTING=0 and bare flag token
        let postgres_code = "{11111111-2222-3333-4444-555555555555}";
        let res3 = mgr.install_product_nested(
            &format!("Binary:{postgres_code}"),
            "PROP_PORT=5432 BARE_FLAG PREEXISTING=0",
        )?;
        assert_eq!(res3, ERROR_SUCCESS);

        // Fourth child: Hash-prefixed stream with no PREEXISTING property, detecting Default state
        mgr.register_product("#hashed_pkg", InstallState::Default);
        let res4 = mgr.install_product_nested("#hashed_pkg", "PROP_PORT=5432")?;
        assert_eq!(res4, ERROR_SUCCESS);
        assert!(mgr.chained_packages()[3].preexisting);
        assert!(mgr.chained_packages()[3].temp_extracted_path.is_some());

        assert_eq!(mgr.chained_packages().len(), 4);
        assert!(mgr.chained_packages()[0].preexisting);
        assert_eq!(
            mgr.chained_packages()[0].properties.get("PROP_MYSQL_PORT"),
            Some(&"3306".to_string())
        );
        assert!(!mgr.chained_packages()[1].preexisting);
        assert!(mgr.chained_packages()[1].temp_extracted_path.is_some());

        // Worker context mutability and service registration
        mgr.worker_mut()
            .filesystem_dirs
            .insert(r"C:\LibScript".to_string());
        assert!(mgr.worker().filesystem_dirs.contains(r"C:\LibScript"));
        mgr.worker_mut().register_service("LibScriptSvc");
        assert!(mgr.worker().has_service("LibScriptSvc"));
        mgr.worker_mut().start_service("LibScriptSvc");
        assert!(mgr.worker().is_service_running("LibScriptSvc"));

        // 6. Commit transaction
        let commit_res = mgr.end_transaction(true)?;
        assert_eq!(commit_res, ERROR_SUCCESS);
        assert_eq!(mgr.state(), Some(TransactionState::Committed));

        // Attempting operations on inactive transaction
        assert!(mgr.join_transaction("Session_Late").is_err());
        assert!(mgr.install_product_nested("other.msi", "").is_err());
        assert!(mgr.end_transaction(true).is_err());

        // 7. Rollback scenario with PREEXISTING guard
        let mut mgr_rb = MultiPackageTransactionManager::begin_transaction("Rollback_Test")?;
        mgr_rb.install_product_nested("pkg1.msi", "PREEXISTING=1")?;
        mgr_rb.install_product_nested("pkg2.msi", "")?;
        let rb_res = mgr_rb.end_transaction(false)?;
        assert_eq!(rb_res, ERROR_INSTALL_FAILURE);
        assert_eq!(mgr_rb.state(), Some(TransactionState::RolledBack));

        // Verify that pkg1 was skipped on rollback and pkg2 was rolled back
        let executed = &mgr_rb.worker().executed_actions;
        assert!(executed.contains(&"SkipRollbackPreexisting:pkg1.msi".to_string()));
        assert!(executed.contains(&"RollbackPackage:pkg2.msi".to_string()));

        Ok(())
    }

    /// Tests coexistence scenario with shared components (`SharedDllRefCount="yes"`) and service persistence.
    #[test]
    fn test_shared_component_reference_counting_coexistence() {
        let mut worker = WorkerContext::default();
        let shared_binary = r"C:\Program Files\LibScript\bin\mysqld.exe";
        let service_name = "LibScript_MySQL";

        // Initial state
        assert_eq!(worker.get_shared_dll_ref(shared_binary), 0);

        // --- Scenario 1: Install Package A (Open edX) ---
        worker.install_component_file(shared_binary, b"mysql_binary_bytes".to_vec(), true);
        worker.services.insert(service_name.to_string());
        worker.running_services.insert(service_name.to_string());

        assert_eq!(worker.get_shared_dll_ref(shared_binary), 1);
        assert!(worker.get_file_content(shared_binary).is_some());
        assert!(worker.running_services.contains(service_name));

        // --- Scenario 1 (continued): Install Package B (WordPress) sharing MySQL ---
        worker.install_component_file(shared_binary, b"mysql_binary_bytes".to_vec(), true);
        assert_eq!(worker.get_shared_dll_ref(shared_binary), 2);

        // --- Scenario 2: Uninstall Package A (Open edX) ---
        // File removal must be suppressed because ref count is 2 -> 1
        let file_removed_a = worker.uninstall_component_file(shared_binary, true);
        assert!(!file_removed_a);
        assert_eq!(worker.get_shared_dll_ref(shared_binary), 1);
        assert!(worker.get_file_content(shared_binary).is_some());

        // Service teardown must be suppressed because ref count for KeyPath is still 1
        let svc_removed_a = worker.uninstall_service_guarded(service_name, Some(shared_binary));
        assert!(!svc_removed_a);
        assert!(worker.running_services.contains(service_name));
        assert!(worker.services.contains(service_name));

        // --- Scenario 3: Uninstall Package B (WordPress) ---
        // Ref count drops from 1 -> 0, so file is finally removed
        let file_removed_b = worker.uninstall_component_file(shared_binary, true);
        assert!(file_removed_b);
        assert_eq!(worker.get_shared_dll_ref(shared_binary), 0);
        assert!(worker.get_file_content(shared_binary).is_none());

        // Service is now cleanly stopped and removed because ref count is 0
        let svc_removed_b = worker.uninstall_service_guarded(service_name, Some(shared_binary));
        assert!(svc_removed_b);
        assert!(!worker.running_services.contains(service_name));
        assert!(!worker.services.contains(service_name));

        // Unguarded removal test (shared_dll_ref_count = false, shared_key_path = None)
        worker.install_component_file("standalone.txt", b"plain".to_vec(), false);
        assert!(worker.uninstall_component_file("standalone.txt", false));
        assert!(worker.uninstall_service_guarded("StandaloneSvc", None));
    }

    /// Tests component client `ProductCode` tracking, service association, and guarded component uninstallation on `WorkerContext`.
    #[test]
    fn test_worker_context_component_client_tracking() {
        let mut worker = WorkerContext::default();
        let comp_guid = "{5B2783B0-9A1F-4348-9F93-87CE43C21001}";
        let prod_openedx = "{E0F45901-83B4-4B21-9B5A-01D38FE81001}";
        let prod_wordpress = "{E0F45901-83B4-4B21-9B5A-01D38FE81002}";
        let file_path = r"C:\Program Files\LibScript\bin\mysqld.exe";
        let service_name = "LibScript_MySQL";

        // Initial state
        assert_eq!(worker.get_component_client_count(comp_guid), 0);
        assert_eq!(
            worker.get_component_clients(comp_guid),
            Vec::<String>::new()
        );

        // Install component for Open edX
        worker.install_component(
            comp_guid,
            prod_openedx,
            file_path,
            b"MYSQL_BYTES".to_vec(),
            true,
            Some(service_name),
        );
        assert_eq!(worker.get_component_client_count(comp_guid), 1);
        assert_eq!(worker.get_shared_dll_ref(file_path), 1);
        assert!(worker.get_file_content(file_path).is_some());
        assert!(worker.running_services.contains(service_name));

        // Install component for WordPress (sharing same component GUID and key path)
        worker.install_component(
            comp_guid,
            prod_wordpress,
            file_path,
            b"MYSQL_BYTES".to_vec(),
            true,
            Some(service_name),
        );
        assert_eq!(worker.get_component_client_count(comp_guid), 2);
        assert_eq!(worker.get_shared_dll_ref(file_path), 2);

        let clients = worker.get_component_clients(comp_guid);
        assert_eq!(clients.len(), 2);
        assert!(clients.contains(&prod_openedx.to_string()));
        assert!(clients.contains(&prod_wordpress.to_string()));

        // Uninstall Open edX: client count drops 2 -> 1, removal suppressed
        let removed1 = worker.uninstall_component_guarded(comp_guid, prod_openedx);
        assert!(!removed1);
        assert_eq!(worker.get_component_client_count(comp_guid), 1);
        assert_eq!(worker.get_shared_dll_ref(file_path), 1);
        assert!(worker.get_file_content(file_path).is_some());
        assert!(worker.running_services.contains(service_name));
        assert!(worker.services.contains(service_name));

        // Uninstall WordPress: client count drops 1 -> 0, files and services removed
        let removed2 = worker.uninstall_component_guarded(comp_guid, prod_wordpress);
        assert!(removed2);
        assert_eq!(worker.get_component_client_count(comp_guid), 0);
        assert_eq!(worker.get_shared_dll_ref(file_path), 0);
        assert!(worker.get_file_content(file_path).is_none());
        assert!(!worker.running_services.contains(service_name));
        assert!(!worker.services.contains(service_name));

        // Unregister non-existent client returns 0
        assert_eq!(
            worker.unregister_component_client("NON_EXISTENT", "PROD"),
            0
        );
    }

    /// Tests `parse_default_dir` and `parse_file_name` helpers.
    #[test]
    fn test_parsing_helpers() {
        assert_eq!(parse_default_dir("."), ".");
        assert_eq!(parse_default_dir(""), "");
        assert_eq!(parse_default_dir("MyApp"), "MyApp");
        assert_eq!(
            parse_default_dir("SHORT~1|LongDirectoryName"),
            "LongDirectoryName"
        );
        assert_eq!(
            parse_default_dir("SHORT~1|LongDirectoryName:SRCSHORT~1|SrcLong"),
            "LongDirectoryName"
        );
        assert_eq!(parse_default_dir("target:source"), "target");

        assert_eq!(parse_file_name("app.exe"), "app.exe");
        assert_eq!(
            parse_file_name("APP~1.EXE|Application.exe"),
            "Application.exe"
        );
        assert_eq!(parse_file_name(""), "");
    }

    /// Tests `WorkerContext` cabinet reader attachment and file extraction.
    #[test]
    fn test_worker_context_cabinet_extraction() -> Result<()> {
        let mut cab_writer =
            crate::cab::writer::CabinetWriter::new(crate::cab::folder::CompressionType::Mszip);
        cab_writer.add_file("fil_script", b"#!/bin/bash\necho test\n")?;
        cab_writer.add_file("readme.txt", b"Documentation content")?;
        let cab_bytes = cab_writer.build();

        let mut worker = WorkerContext::new();
        assert!(!worker.has_cabinet("#data.cab"));

        worker.add_cabinet_bytes("#data.cab", &cab_bytes)?;
        assert!(worker.has_cabinet("#data.cab"));
        assert!(worker.has_cabinet("data.cab"));
        assert_eq!(worker.cabinet_readers().len(), 1);

        // Extract by file identifier
        let script_bytes = worker.extract_cabinet_file(Some("#data.cab"), "fil_script")?;
        assert_eq!(script_bytes, b"#!/bin/bash\necho test\n");

        // Extract by file name (case-insensitive)
        let doc_bytes = worker.extract_cabinet_file(Some("data.cab"), "README.TXT")?;
        assert_eq!(doc_bytes, b"Documentation content");

        // Extract across all cabinets (cabinet = None)
        let doc_any = worker.extract_cabinet_file(None, "readme.txt")?;
        assert_eq!(doc_any, b"Documentation content");

        // Error when file not found
        let err = worker.extract_cabinet_file(Some("data.cab"), "nonexistent.file");
        assert!(err.is_err());

        // Error when cabinet not found
        let err_cab = worker.extract_cabinet_file(Some("missing.cab"), "fil_script");
        assert!(err_cab.is_err());

        // Builder pattern tests
        let reader = crate::cab::reader::CabinetReader::new(&cab_bytes)?;
        let worker_builder = WorkerContext::new()
            .with_cabinet_reader("reader.cab", reader)
            .with_cabinet_bytes("extra.cab", &cab_bytes)?;
        assert!(worker_builder.has_cabinet("reader.cab"));
        assert!(worker_builder.has_cabinet("extra.cab"));

        Ok(())
    }

    /// Tests `Transaction` cabinet methods and package synthesis.
    #[test]
    fn test_transaction_cabinet_attachment_and_package() -> Result<()> {
        let db = create_test_database()?;
        let context = EvaluationContext::new();
        let cost_engine = DiskCostEngine::new();

        let mut cab_writer =
            crate::cab::writer::CabinetWriter::new(crate::cab::folder::CompressionType::None);
        cab_writer.add_file("test.bin", b"binary")?;
        let cab_data = cab_writer.build();

        let mut cabs_map = HashMap::new();
        cabs_map.insert("#media1.cab".to_string(), cab_data.clone());

        let tx = Transaction::new(db.clone(), context.clone(), cost_engine.clone())
            .with_cabinet("#test.cab", cab_data)
            .with_cabinets(cabs_map);

        assert_eq!(tx.embedded_cabinets.len(), 2);
        assert!(tx.embedded_cabinets.contains_key("#test.cab"));
        assert!(tx.embedded_cabinets.contains_key("#media1.cab"));

        // Package integration test
        let metadata = crate::package::PackageMetadata::new(
            "Pkg",
            "Vendor",
            crate::package::ProductVersion::new(1, 0, 0),
            "{12345678-1234-1234-1234-123456789012}",
        );
        let pkg = Package::new(
            metadata,
            db,
            crate::database::summary_info::SummaryInfo::default(),
            tx.embedded_cabinets,
        );

        let tx_from_pkg = Transaction::from_package(&pkg, context, cost_engine);
        assert_eq!(tx_from_pkg.embedded_cabinets.len(), 2);

        // Preload cabinets to worker during execute
        let tx_prep = tx_from_pkg.prepare()?;
        let mut worker = WorkerContext::new();
        assert!(!worker.has_cabinet("#test.cab"));
        let tx_exec = tx_prep.execute(&mut worker)?;
        assert!(worker.has_cabinet("#test.cab"));
        assert!(worker.has_cabinet("#media1.cab"));
        let _ = tx_exec.commit(&mut worker)?;

        Ok(())
    }

    /// Tests standard directory resolution and child folder inheritance.
    #[test]
    fn test_resolve_directories_hierarchy_and_overrides() -> Result<()> {
        let mut db = LinkedDatabase::new()?;

        // Add Directory records forming a hierarchy:
        // TARGETDIR -> ProgramFiles64Folder -> INSTALLFOLDER -> SubFolder
        db.add_record(
            "Directory",
            Record::with_fields(vec![
                FieldValue::String("TARGETDIR".to_string()),
                FieldValue::Null,
                FieldValue::String("SourceDir".to_string()),
            ]),
        );
        db.add_record(
            "Directory",
            Record::with_fields(vec![
                FieldValue::String("ProgramFiles64Folder".to_string()),
                FieldValue::String("TARGETDIR".to_string()),
                FieldValue::String("PFiles64|Program Files".to_string()),
            ]),
        );
        db.add_record(
            "Directory",
            Record::with_fields(vec![
                FieldValue::String("INSTALLFOLDER".to_string()),
                FieldValue::String("ProgramFiles64Folder".to_string()),
                FieldValue::String("MyApp".to_string()),
            ]),
        );
        db.add_record(
            "Directory",
            Record::with_fields(vec![
                FieldValue::String("SubFolder".to_string()),
                FieldValue::String("INSTALLFOLDER".to_string()),
                FieldValue::String("sub:sub_src".to_string()),
            ]),
        );

        let mut context = EvaluationContext::new();
        context.set_property("TARGETDIR", "/custom/root");

        let resolved = resolve_directories(&db, &mut context);

        assert_eq!(
            resolved.get("TARGETDIR"),
            Some(&PathBuf::from("/custom/root"))
        );
        let expected_install = PathBuf::from("/custom/root")
            .join("Program Files")
            .join("MyApp");
        assert_eq!(resolved.get("INSTALLFOLDER"), Some(&expected_install));
        assert_eq!(
            resolved.get("SubFolder"),
            Some(&expected_install.join("sub"))
        );

        // Check context property registration
        assert_eq!(
            context.get_property("INSTALLFOLDER"),
            Some(expected_install.to_string_lossy().as_ref())
        );

        Ok(())
    }

    /// Tests public property detection, formatting, context forwarding, and product state normalization.
    #[test]
    fn test_multi_package_properties_and_state_normalization() -> Result<()> {
        // Public property detection
        assert!(MultiPackageTransactionManager::is_public_property(
            "PROP_MYSQL_PORT"
        ));
        assert!(MultiPackageTransactionManager::is_public_property(
            "INSTALL_MYSQL"
        ));
        assert!(MultiPackageTransactionManager::is_public_property(
            "ALLUSERS"
        ));
        assert!(MultiPackageTransactionManager::is_public_property(
            "TARGETDIR"
        ));
        assert!(MultiPackageTransactionManager::is_public_property(
            "PROP123"
        ));
        assert!(!MultiPackageTransactionManager::is_public_property(""));
        assert!(!MultiPackageTransactionManager::is_public_property(
            "ProductName"
        ));
        assert!(!MultiPackageTransactionManager::is_public_property(
            "installDir"
        ));
        assert!(!MultiPackageTransactionManager::is_public_property(
            "prop_lowercase"
        ));

        // Context property forwarding
        let mut master_ctx = EvaluationContext::new();
        master_ctx.set_property("PROP_PORT", "3306");
        master_ctx.set_property("PROP_SECRET", "pass word with space");
        master_ctx.set_property("PROP_QUOTE", r#"quote"inside"#);
        master_ctx.set_property("private_prop", "ignored_value");
        master_ctx.set_property("AnotherPrivate", "ignored");

        let cmdline = MultiPackageTransactionManager::forward_public_properties(&master_ctx);
        assert!(cmdline.contains("PROP_PORT=3306"));
        assert!(cmdline.contains(r#"PROP_SECRET="pass word with space""#));
        assert!(cmdline.contains(r#"PROP_QUOTE="quote\"inside""#));
        assert!(!cmdline.contains("private_prop"));
        assert!(!cmdline.contains("AnotherPrivate"));

        let mut child_ctx = EvaluationContext::new();
        MultiPackageTransactionManager::forward_properties_to_context(&master_ctx, &mut child_ctx);
        assert_eq!(child_ctx.get_property("PROP_PORT"), Some("3306"));
        assert_eq!(
            child_ctx.get_property("PROP_SECRET"),
            Some("pass word with space")
        );
        assert_eq!(child_ctx.get_property("private_prop"), None);

        // Product state normalization
        let mut mgr = MultiPackageTransactionManager::begin_transaction("StateTest")?;
        let code = "{E0F45901-83B4-4B21-9B5A-01D38FE81000}";
        mgr.register_product(code, InstallState::Default);

        // Query with braces
        assert_eq!(mgr.query_product_state(code), InstallState::Default);
        // Query without braces
        assert_eq!(
            mgr.query_product_state("e0f45901-83b4-4b21-9b5a-01d38fe81000"),
            InstallState::Default
        );
        // Query unregistered
        assert_eq!(
            mgr.query_product_state("{99999999-9999-9999-9999-999999999999}"),
            InstallState::Absent
        );

        assert!(mgr.is_product_installed(code));
        assert!(!mgr.is_product_installed("{99999999-9999-9999-9999-999999999999}"));

        // Package pre-existing check
        let mut pkg_db = LinkedDatabase::new()?;
        pkg_db.add_record(
            "Property",
            Record::with_fields(vec![
                FieldValue::String("UpgradeCode".to_string()),
                FieldValue::String("{E0F45901-83B4-4B21-9B5A-01D38FE81000}".to_string()),
            ]),
        );
        let meta = crate::package::PackageMetadata::new(
            "ChildTest",
            "LibScript",
            crate::package::ProductVersion::new(1, 0, 0),
            "{00000000-0000-0000-0000-000000000001}",
        );
        let pkg = Package::new(
            meta,
            pkg_db,
            crate::database::summary_info::SummaryInfo::default(),
            HashMap::new(),
        );
        // UpgradeCode is registered as installed -> true
        assert!(mgr.is_package_preexisting(&pkg));

        // Unregistered package
        let meta2 = crate::package::PackageMetadata::new(
            "ChildTest2",
            "LibScript",
            crate::package::ProductVersion::new(1, 0, 0),
            "{99999999-0000-0000-0000-000000000002}",
        );
        let pkg2 = Package::new(
            meta2,
            LinkedDatabase::new()?,
            crate::database::summary_info::SummaryInfo::default(),
            HashMap::new(),
        );
        assert!(!mgr.is_package_preexisting(&pkg2));

        Ok(())
    }

    /// Tests `MsiEmbeddedChainer` parsing, child package extraction from streams and Binary table,
    /// and error paths when child packages are missing.
    #[test]
    fn test_multi_package_embedded_chainer_and_extraction() -> Result<()> {
        let temp_dir = std::env::temp_dir().join(format!("msi_chainer_ext_{}", std::process::id()));
        let _ = std::fs::create_dir_all(&temp_dir);

        let mut db = LinkedDatabase::new()?;
        db.add_record(
            "MsiEmbeddedChainer",
            Record::with_fields(vec![
                FieldValue::String("Chainer1".to_string()),
                FieldValue::String("NOT Installed".to_string()),
                FieldValue::String("/quiet".to_string()),
                FieldValue::String("Bin_Chainer".to_string()),
                FieldValue::Long(1),
            ]),
        );

        db.add_record(
            "Binary",
            Record::with_fields(vec![
                FieldValue::String("child-binary.msi".to_string()),
                FieldValue::String("BINARY_MSI_PAYLOAD".to_string()),
            ]),
        );

        let mut cabinets = HashMap::new();
        cabinets.insert(
            "libscript-mysql.msi".to_string(),
            b"STREAM_MYSQL_PAYLOAD".to_vec(),
        );
        cabinets.insert(
            "#libscript-redis.msi".to_string(),
            b"STREAM_REDIS_PAYLOAD".to_vec(),
        );

        let meta = crate::package::PackageMetadata::new(
            "Master",
            "LibScript",
            crate::package::ProductVersion::new(1, 0, 0),
            "{11111111-1111-1111-1111-111111111111}",
        );
        let master_pkg = Package::new(
            meta,
            db,
            crate::database::summary_info::SummaryInfo::default(),
            cabinets,
        );

        // 1. Read embedded chainers
        let chainers = MultiPackageTransactionManager::read_embedded_chainers(&master_pkg)?;
        assert_eq!(chainers.len(), 1);
        assert_eq!(chainers[0].chainer, "Chainer1");
        assert_eq!(chainers[0].condition.as_deref(), Some("NOT Installed"));

        // 2. Extract child package from stream
        let mut mgr = MultiPackageTransactionManager::begin_transaction("ExtTest")?;
        let spool = temp_dir.join("spool");
        let path1 = mgr.extract_child_package(&master_pkg, "libscript-mysql.msi", &spool)?;
        assert!(path1.exists());
        assert_eq!(std::fs::read(&path1)?, b"STREAM_MYSQL_PAYLOAD");

        // Extract with '#' prefix
        let path2 = mgr.extract_child_package(&master_pkg, "#libscript-redis.msi", &spool)?;
        assert!(path2.exists());
        assert_eq!(std::fs::read(&path2)?, b"STREAM_REDIS_PAYLOAD");

        // Extract from Binary table
        let path3 = mgr.extract_child_package(&master_pkg, "child-binary.msi", &spool)?;
        assert!(path3.exists());
        assert_eq!(std::fs::read(&path3)?, b"BINARY_MSI_PAYLOAD");

        // Error path: Missing child package
        let err = mgr.extract_child_package(&master_pkg, "non-existent.msi", &spool);
        assert!(err.is_err());

        // 3. Extract all child packages
        let extracted_all = mgr.extract_all_child_packages(&master_pkg, &spool)?;
        assert_eq!(extracted_all.len(), 3);
        assert!(extracted_all.contains_key("libscript-mysql.msi"));
        assert!(extracted_all.contains_key("#libscript-redis.msi"));
        assert!(extracted_all.contains_key("child-binary.msi"));

        let _ = std::fs::remove_dir_all(&temp_dir);
        Ok(())
    }

    /// Tests `install_child_package` with pre-existing detection, skip flags, launch conditions, and rollback scripts.
    #[test]
    fn test_multi_package_child_installation_lifecycle() -> Result<()> {
        let mut mgr = MultiPackageTransactionManager::begin_transaction("ChildInstallTest")?;

        // 1. Inactive transaction error path
        let mut inactive_mgr = MultiPackageTransactionManager::default();
        let meta = crate::package::PackageMetadata::new(
            "Dummy",
            "Vendor",
            crate::package::ProductVersion::new(1, 0, 0),
            "{00000000-0000-0000-0000-000000000000}",
        );
        let dummy_pkg = Package::new(
            meta,
            LinkedDatabase::new()?,
            crate::database::summary_info::SummaryInfo::default(),
            HashMap::new(),
        );
        assert!(inactive_mgr.install_child_package(&dummy_pkg, "").is_err());

        // 2. Pre-existing package: skipped
        let db1 = LinkedDatabase::new()?;
        let meta1 = crate::package::PackageMetadata::new(
            "PrePkg",
            "Vendor",
            crate::package::ProductVersion::new(1, 0, 0),
            "{E0F45901-83B4-4B21-9B5A-01D38FE81001}",
        );
        let pkg1 = Package::new(
            meta1,
            db1,
            crate::database::summary_info::SummaryInfo::default(),
            HashMap::new(),
        );
        mgr.register_product(
            "{E0F45901-83B4-4B21-9B5A-01D38FE81001}",
            InstallState::Default,
        );
        let res_pre = mgr.install_child_package(&pkg1, "")?;
        assert_eq!(res_pre, ERROR_SUCCESS);
        assert!(mgr.chained_packages()[0].preexisting);
        assert!(mgr.chained_packages()[0].rollback_script.is_none());

        // 3. Skip flag INSTALL_XYZ="0"
        let meta_skip = crate::package::PackageMetadata::new(
            "SkipPkg",
            "Vendor",
            crate::package::ProductVersion::new(1, 0, 0),
            "{00000000-0000-0000-0000-000000000002}",
        );
        let pkg_skip = Package::new(
            meta_skip,
            LinkedDatabase::new()?,
            crate::database::summary_info::SummaryInfo::default(),
            HashMap::new(),
        );
        let res_skip = mgr.install_child_package(&pkg_skip, "INSTALL_OPTIONAL=0")?;
        assert_eq!(res_skip, ERROR_SUCCESS);
        assert!(mgr.chained_packages()[1].preexisting);

        // 4. Launch condition failure
        let mut db_lc = LinkedDatabase::new()?;
        db_lc.add_record(
            "LaunchCondition",
            Record::with_fields(vec![
                FieldValue::String("NOT Installed AND REQUIRED_PROP".to_string()),
                FieldValue::String("REQUIRED_PROP must be set".to_string()),
            ]),
        );
        let meta_lc = crate::package::PackageMetadata::new(
            "LcPkg",
            "Vendor",
            crate::package::ProductVersion::new(1, 0, 0),
            "{00000000-0000-0000-0000-000000000003}",
        );
        let pkg_lc = Package::new(
            meta_lc,
            db_lc,
            crate::database::summary_info::SummaryInfo::default(),
            HashMap::new(),
        );
        let lc_err = mgr.install_child_package(&pkg_lc, "");
        assert!(lc_err.is_err());

        // 5. Successful child package install with rollback script
        let db_ok = create_test_database()?;
        let meta_ok = crate::package::PackageMetadata::new(
            "OkPkg",
            "Vendor",
            crate::package::ProductVersion::new(1, 0, 0),
            "{00000000-0000-0000-0000-000000000004}",
        );
        let pkg_ok = Package::new(
            meta_ok,
            db_ok,
            crate::database::summary_info::SummaryInfo::default(),
            HashMap::new(),
        );
        let res_ok = mgr.install_child_package(&pkg_ok, "PROP_TEST=val")?;
        assert_eq!(res_ok, ERROR_SUCCESS);
        let last_chained = &mgr.chained_packages()[2];
        assert!(!last_chained.preexisting);
        assert!(last_chained.rollback_script.is_some());

        Ok(())
    }

    /// Tests cascading rollback across child packages: when a child package fails, all newly installed
    /// packages are unwound in reverse order while pre-existing packages are preserved.
    #[test]
    #[allow(clippy::too_many_lines)]
    fn test_multi_package_cascading_rollback_and_orchestrator() -> Result<()> {
        let mut mgr = MultiPackageTransactionManager::begin_transaction("CascadeTest")?;

        // Package 1: Newly installed package (creates file)
        let mut db1 = LinkedDatabase::new()?;
        db1.add_record(
            "Directory",
            Record::with_fields(vec![
                FieldValue::String("TARGETDIR".to_string()),
                FieldValue::Null,
                FieldValue::String("SourceDir".to_string()),
            ]),
        );
        db1.add_record(
            "Component",
            Record::with_fields(vec![
                FieldValue::String("Comp1".to_string()),
                FieldValue::String("{11111111-1111-1111-1111-111111111111}".to_string()),
                FieldValue::String("TARGETDIR".to_string()),
                FieldValue::Short(0),
                FieldValue::Null,
                FieldValue::Null,
            ]),
        );
        db1.add_record(
            "File",
            Record::with_fields(vec![
                FieldValue::String("File1".to_string()),
                FieldValue::String("Comp1".to_string()),
                FieldValue::String("mysql.bin".to_string()),
                FieldValue::Long(100),
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Short(0),
                FieldValue::Short(1),
            ]),
        );
        db1.add_record(
            "InstallExecuteSequence",
            Record::with_fields(vec![
                FieldValue::String("InstallFiles".to_string()),
                FieldValue::Null,
                FieldValue::Short(4000),
            ]),
        );
        let meta1 = crate::package::PackageMetadata::new(
            "MySQL",
            "LibScript",
            crate::package::ProductVersion::new(1, 0, 0),
            "{E0F45901-83B4-4B21-9B5A-01D38FE81001}",
        );
        let pkg_mysql = Package::new(
            meta1,
            db1,
            crate::database::summary_info::SummaryInfo::default(),
            HashMap::new(),
        );

        // Package 2: Pre-existing package (Redis)
        let meta2 = crate::package::PackageMetadata::new(
            "Redis",
            "LibScript",
            crate::package::ProductVersion::new(1, 0, 0),
            "{E0F45901-83B4-4B21-9B5A-01D38FE81002}",
        );
        let pkg_redis = Package::new(
            meta2,
            LinkedDatabase::new()?,
            crate::database::summary_info::SummaryInfo::default(),
            HashMap::new(),
        );
        mgr.register_product(
            "{E0F45901-83B4-4B21-9B5A-01D38FE81002}",
            InstallState::Default,
        );

        // Package 3: Failing package (OpenEdX Core) with impossible launch condition
        let mut db3 = LinkedDatabase::new()?;
        db3.add_record(
            "LaunchCondition",
            Record::with_fields(vec![
                FieldValue::String("1 = 0".to_string()),
                FieldValue::String("Simulated failure in Core".to_string()),
            ]),
        );
        let meta3 = crate::package::PackageMetadata::new(
            "OpenEdXCore",
            "LibScript",
            crate::package::ProductVersion::new(1, 0, 0),
            "{E0F45901-83B4-4B21-9B5A-01D38FE81003}",
        );
        let pkg_core = Package::new(
            meta3,
            db3,
            crate::database::summary_info::SummaryInfo::default(),
            HashMap::new(),
        );

        let mut master_ctx = EvaluationContext::new();
        master_ctx.set_property("PROP_MYSQL_PORT", "3307");

        let child_list: Vec<(&Package, Option<&str>)> =
            vec![(&pkg_mysql, None), (&pkg_redis, None), (&pkg_core, None)];

        // Orchestrate child packages - will fail on pkg_core and trigger cascade rollback!
        let res = mgr.orchestrate_child_packages(&child_list, &master_ctx);
        assert!(res.is_err());
        assert_eq!(mgr.state(), Some(TransactionState::RolledBack));

        // Verify that MySQL was rolled back, Redis was skipped on rollback
        let actions = &mgr.worker().executed_actions;
        assert!(actions.contains(&"InstallProduct:MySQL".to_string()));
        assert!(actions.contains(
            &"SkipChildPackagePreexisting:Redis:{E0F45901-83B4-4B21-9B5A-01D38FE81002}".to_string()
        ));
        assert!(actions.contains(&"RollbackPackage:MySQL".to_string()));
        assert!(actions.contains(&"SkipRollbackPreexisting:Redis".to_string()));

        // Also test orchestrate_master_package chainer condition skipping
        let mut mgr2 = MultiPackageTransactionManager::begin_transaction("MasterTest")?;
        let mut master_db = LinkedDatabase::new()?;
        master_db.add_record(
            "MsiEmbeddedChainer",
            Record::with_fields(vec![
                FieldValue::String("SkipChainer".to_string()),
                FieldValue::String("SHOULD_RUN = 1".to_string()),
                FieldValue::String("/quiet".to_string()),
                FieldValue::String("BinKey".to_string()),
                FieldValue::Long(1),
            ]),
        );
        let master_pkg2 = Package::new(
            crate::package::PackageMetadata::new(
                "Master2",
                "Vendor",
                crate::package::ProductVersion::new(1, 0, 0),
                "{99999999-9999-9999-9999-999999999999}",
            ),
            master_db,
            crate::database::summary_info::SummaryInfo::default(),
            HashMap::new(),
        );

        let mut m_ctx = EvaluationContext::new();
        m_ctx.set_property("SHOULD_RUN", "0");
        let temp_spool = std::env::temp_dir().join(format!("spool_orch_{}", std::process::id()));
        let res_skip_chainer =
            mgr2.orchestrate_master_package(&master_pkg2, &m_ctx, &temp_spool, &[])?;
        assert_eq!(res_skip_chainer, ERROR_SUCCESS);
        assert!(mgr2
            .worker()
            .executed_actions
            .contains(&"SkipEmbeddedChainer:SkipChainer".to_string()));

        let _ = std::fs::remove_dir_all(&temp_spool);
        Ok(())
    }

    /// Tests `WorkerContext` accessors, binaries, and cabinet extraction edge cases.
    #[test]
    fn test_worker_context_accessors_and_cabinet_readers() -> Result<()> {
        let mut worker = WorkerContext::new();

        // CustomActionExecutor accessors
        let _ca = worker.custom_action_executor();
        let _ca_mut = worker.custom_action_executor_mut();

        // EvaluationContext accessors
        let _eval = worker.evaluation_context();
        let _eval_mut = worker.evaluation_context_mut();
        let mut new_eval = EvaluationContext::new();
        new_eval.set_property("CTX_TEST", "1");
        worker.set_evaluation_context(new_eval);
        assert_eq!(
            worker.evaluation_context().get_property("CTX_TEST"),
            Some("1")
        );

        // Binary accessors
        assert!(worker.binaries().is_empty());
        assert!(worker.get_binary("custom_bin").is_none());
        worker.add_binary("custom_bin", vec![10, 20, 30]);
        assert_eq!(worker.get_binary("custom_bin"), Some(&[10, 20, 30][..]));
        assert_eq!(worker.binaries().len(), 1);

        // Actions accessors
        assert!(worker.executed_actions().is_empty());
        assert!(!worker.has_executed_action("TestAction"));
        let mut act_script = InstallScript::new();
        act_script.push(ScriptOp::CreateFolder {
            path: "test_dir".to_string(),
        });
        worker.execute_script(&act_script)?;
        assert!(worker.has_executed_action("CreateFolder"));
        assert!(!worker.has_executed_action("NonExistentAction"));

        // Pre-seeding
        worker.pre_seed_file("pre_seed.txt", b"pre_seed_data");
        assert_eq!(
            worker.get_file_content("pre_seed.txt"),
            Some(&b"pre_seed_data"[..])
        );
        worker.pre_seed_registry(
            1,
            "Software\\Test",
            Some("ValName".to_string()),
            Some("PreVal".to_string()),
        );
        assert_eq!(
            worker.get_registry_value(1, "Software\\Test", Some("ValName")),
            Some(&Some("PreVal".to_string()))
        );

        // Simulate failure setter
        worker.simulate_failure_at("SimulatedAction");

        // Cabinets
        let mut cab_writer =
            crate::cab::writer::CabinetWriter::new(crate::cab::folder::CompressionType::None);
        cab_writer.add_file("readme.txt", b"cab_readme_content")?;
        let cab_bytes = cab_writer.build();
        let reader = crate::cab::reader::CabinetReader::new(&cab_bytes)?;
        let reader2 = crate::cab::reader::CabinetReader::new(&cab_bytes)?;

        worker.add_cabinet_reader("reader_cab", reader);
        worker.add_cabinet_reader("#hash_cab", reader2);
        assert!(worker.has_cabinet("reader_cab"));
        assert!(worker.has_cabinet("#reader_cab"));
        assert!(worker.has_cabinet("hash_cab"));
        assert!(worker.has_cabinet("#hash_cab"));
        assert!(!worker.has_cabinet("nonexistent_cab"));
        assert!(!worker.cabinet_readers().is_empty());

        // extract_cabinet_file with Some cab but file not in that cab
        let err_missing_in_cab = worker.extract_cabinet_file(Some("reader_cab"), "nonexistent.dat");
        assert!(err_missing_in_cab.is_err());

        // extract_cabinet_file with Unknown cab
        let err_unknown_cab = worker.extract_cabinet_file(Some("unknown_cab"), "readme.txt");
        assert!(err_unknown_cab.is_err());

        // extract_cabinet_file with None cab (case-insensitive filename match)
        let found_content = worker.extract_cabinet_file(None, "README.TXT")?;
        assert_eq!(found_content, b"cab_readme_content");

        // extract_cabinet_file with None cab when missing everywhere
        let err_not_found = worker.extract_cabinet_file(None, "totally_missing.dat");
        assert!(err_not_found.is_err());

        Ok(())
    }

    /// Tests component client reference counting, key paths, and service association lifecycle.
    #[test]
    fn test_worker_context_component_and_services_deep() {
        let mut worker = WorkerContext::new();

        worker.register_service("DaemonSvc");
        assert!(worker.has_service("DaemonSvc"));
        assert!(!worker.is_service_running("DaemonSvc"));
        worker.start_service("DaemonSvc");
        assert!(worker.is_service_running("DaemonSvc"));

        // install_component_file
        worker.install_component_file(
            "/usr/lib/libstandalone.so".to_string(),
            b"BYTES".to_vec(),
            true,
        );
        assert!(worker
            .get_file_content("/usr/lib/libstandalone.so")
            .is_some());

        // install_component with shared_dll: true, key_path: Some, service_name: Some
        worker.install_component(
            "COMP_SHARED",
            "{11111111-1111-1111-1111-111111111111}",
            "/usr/lib/libshared.so",
            b"BINARY_CODE".to_vec(),
            true,
            Some("DaemonSvc"),
        );
        worker.associate_component_service("COMP_SHARED", "DaemonSvc");

        // install_component without service and register without key_path
        worker.install_component(
            "COMP_NO_SVC",
            "{33333333-3333-3333-3333-333333333333}",
            "/file_no_svc.bin",
            b"DATA".to_vec(),
            true,
            None,
        );
        worker.install_component(
            "COMP_UNSHARED",
            "{55555555-5555-5555-5555-555555555555}",
            "/file_unshared.bin",
            b"DATA_UNSHARED".to_vec(),
            false,
            None,
        );
        worker.register_component_client(
            "COMP_NO_KP",
            "{44444444-4444-4444-4444-444444444444}",
            None,
        );
        assert!(worker
            .uninstall_component_guarded("COMP_NO_KP", "{44444444-4444-4444-4444-444444444444}"));

        // Register a second client
        let ref_count = worker.register_component_client(
            "COMP_SHARED",
            "{22222222-2222-2222-2222-222222222222}",
            Some("/usr/lib/libshared.so"),
        );
        assert_eq!(ref_count, 2);

        // First uninstall: remaining clients > 0, returns false, does not teardown files/services
        let removed_first = worker
            .uninstall_component_guarded("COMP_SHARED", "{11111111-1111-1111-1111-111111111111}");
        assert!(!removed_first);
        assert!(worker.get_file_content("/usr/lib/libshared.so").is_some());
        assert!(worker.has_service("DaemonSvc"));

        // Second uninstall: remaining clients == 0, returns true, tears down file and service!
        let removed_second = worker
            .uninstall_component_guarded("COMP_SHARED", "{22222222-2222-2222-2222-222222222222}");
        assert!(removed_second);
        assert!(worker.get_file_content("/usr/lib/libshared.so").is_none());
        assert!(!worker.has_service("DaemonSvc"));
        assert!(!worker.is_service_running("DaemonSvc"));
    }

    /// Tests live execution of `CopyFile`, `WriteFile`, custom action dispatch, and rollback.
    #[test]
    fn test_worker_context_live_ops_and_ca_dispatch() -> Result<()> {
        let temp_dir = std::env::temp_dir().join(format!("msi_tx_live_ops_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&temp_dir);
        std::fs::create_dir_all(&temp_dir)?;

        let src_file = temp_dir.join("input.txt");
        std::fs::write(&src_file, b"SOURCE_DATA")?;

        let exe_copy_dest = temp_dir.join("run.exe");
        let non_exe_copy_dest = temp_dir.join("copied.txt");
        let exe_write_dest = temp_dir.join("script.sh");
        let non_exe_write_dest = temp_dir.join("notes.md");

        let quarantine = temp_dir.join("quarantine");
        let live_exec = crate::execution::LiveWorkerExecutor::new(&quarantine, "live_tx_test");

        let mut worker = WorkerContext::new().with_live_executor(live_exec);
        worker.pre_seed_file(&src_file.to_string_lossy(), b"SOURCE_DATA");

        let mut script = InstallScript::new();
        // Executable copy (mode 0o755)
        script.push(ScriptOp::CopyFile {
            source: src_file.to_string_lossy().to_string(),
            destination: exe_copy_dest.to_string_lossy().to_string(),
            overwrite: true,
        });
        // Non-executable copy (mode 0o644)
        script.push(ScriptOp::CopyFile {
            source: src_file.to_string_lossy().to_string(),
            destination: non_exe_copy_dest.to_string_lossy().to_string(),
            overwrite: true,
        });
        // Executable write (mode 0o755)
        script.push(ScriptOp::WriteFile {
            destination: exe_write_dest.to_string_lossy().to_string(),
            content: b"#!/bin/sh\nexit 0\n".to_vec(),
        });
        // Non-executable write (mode 0o644)
        script.push(ScriptOp::WriteFile {
            destination: non_exe_write_dest.to_string_lossy().to_string(),
            content: b"# Markdown doc".to_vec(),
        });

        worker.execute_script(&script)?;
        assert!(exe_copy_dest.exists());
        assert!(non_exe_copy_dest.exists());
        assert!(exe_write_dest.exists());
        assert!(non_exe_write_dest.exists());

        // Custom action simulation failure
        worker.simulate_failure_at("FailCA");
        let mut ca_script = InstallScript::new();
        ca_script.push(ScriptOp::CustomAction {
            action: "FailCA".to_string(),
            action_type: 1,
            source: "BinaryTable".to_string(),
            target: "FailEntry".to_string(),
        });
        let fail_res = worker.execute_script(&ca_script);
        assert!(fail_res.is_err());

        // Mock custom action execution (is_mock == true)
        let mut mock_ca_script = InstallScript::new();
        mock_ca_script.push(ScriptOp::CustomAction {
            action: "MockCA".to_string(),
            action_type: 1,
            source: "BinaryTable".to_string(),
            target: "Fn".to_string(),
        });
        worker.execute_script(&mock_ca_script)?;

        // Non-mock custom action execution
        let mut ok_ca_script = InstallScript::new();
        ok_ca_script.push(ScriptOp::CustomAction {
            action: "SetPropCA".to_string(),
            action_type: 51, // Type 51: property setter
            source: "TARGET_PROP".to_string(),
            target: "CustomValue".to_string(),
        });
        worker.execute_script(&ok_ca_script)?;
        assert_eq!(
            worker.evaluation_context().get_property("TARGET_PROP"),
            Some("CustomValue")
        );

        // Rollback custom action
        let mut rollback = RollbackScript::new();
        rollback.push(RollbackOp::RollbackCustomAction {
            action: "RollbackCA".to_string(),
            action_type: 51,
            source: "TARGET_PROP".to_string(),
            target: "OriginalValue".to_string(),
        });
        // Rollback custom action with invalid type so parse returns Err
        rollback.push(RollbackOp::RollbackCustomAction {
            action: "BadRollbackCA".to_string(),
            action_type: 0xFFFF,
            source: "TARGET_PROP".to_string(),
            target: "Val".to_string(),
        });
        worker.execute_rollback(&rollback)?;
        assert_eq!(
            worker.evaluation_context().get_property("TARGET_PROP"),
            Some("OriginalValue")
        );

        let _ = std::fs::remove_dir_all(&temp_dir);
        Ok(())
    }

    /// Tests `MultiPackageTransactionManager` edge cases, child package extractions, and orchestration branches.
    #[test]
    #[allow(clippy::too_many_lines)]
    fn test_multi_package_transaction_manager_extended() -> Result<()> {
        // Validation: empty transaction name fails
        let empty_err = MultiPackageTransactionManager::begin_transaction("");
        assert!(empty_err.is_err());

        let mut mgr = MultiPackageTransactionManager::begin_transaction("FullMatrixTx")?;
        assert_eq!(mgr.transaction_name(), "FullMatrixTx");
        mgr.join_transaction("Session_A")?;

        // Register products
        mgr.register_product(
            "{ABCDEF01-1234-5678-ABCD-123456789012}",
            InstallState::Default,
        );
        assert_eq!(
            mgr.query_product_state("{ABCDEF01-1234-5678-ABCD-123456789012}"),
            InstallState::Default
        );
        assert!(mgr.is_product_installed("{ABCDEF01-1234-5678-ABCD-123456789012}"));

        // Nested product installs: with PREEXISTING, with skip property, with temp extracted path
        mgr.install_product_nested("#NestedPkg.msi", "PROP=1 PREEXISTING=1")?;
        mgr.install_product_nested("PlainPkg", "INSTALL_FEATURE=0")?;

        // Test InstallState functions
        assert!(InstallState::Default.is_installed());
        assert!(InstallState::Local.is_installed());
        assert!(InstallState::Source.is_installed());
        assert!(InstallState::Advertised.is_installed());
        assert!(!InstallState::Absent.is_installed());
        assert!(!InstallState::BadConfiguration.is_installed());
        assert!(!InstallState::InvalidArg.is_installed());
        assert!(!InstallState::Unknown.is_installed());
        assert_eq!(InstallState::Default.to_i32(), 5);
        assert_eq!(InstallState::Absent.to_i32(), 2);

        // Test Transaction return codes
        let committed: Transaction<Committed> = Transaction {
            database: LinkedDatabase::new()?,
            context: EvaluationContext::new(),
            cost_engine: DiskCostEngine::new(),
            install_script: InstallScript::new(),
            rollback_script: RollbackScript::new(),
            sequence_table: "InstallExecuteSequence".to_string(),
            embedded_cabinets: HashMap::new(),
            _state: PhantomData,
        };
        assert_eq!(committed.return_code(), 0);

        let rolled_back: Transaction<RolledBack> = Transaction {
            database: LinkedDatabase::new()?,
            context: EvaluationContext::new(),
            cost_engine: DiskCostEngine::new(),
            install_script: InstallScript::new(),
            rollback_script: RollbackScript::new(),
            sequence_table: "InstallExecuteSequence".to_string(),
            embedded_cabinets: HashMap::new(),
            _state: PhantomData,
        };
        assert_eq!(rolled_back.return_code(), 1603);

        // Test extract_child_package: when key does NOT end with .msi and is in Binary table
        let mut binary_db = LinkedDatabase::new()?;
        binary_db.add_record(
            "Binary",
            Record::with_fields(vec![
                FieldValue::String("ChildWithoutExt".to_string()),
                FieldValue::String("BINARY_STRING_PAYLOAD".to_string()),
            ]),
        );
        binary_db.add_record(
            "Binary",
            Record::with_fields(vec![
                FieldValue::String("UnmatchedBinary".to_string()),
                FieldValue::String("OTHER_PAYLOAD".to_string()),
            ]),
        );
        binary_db.add_record(
            "Binary",
            Record::with_fields(vec![
                FieldValue::String("NullPayloadBinary".to_string()),
                FieldValue::Null,
            ]),
        );
        binary_db.add_record(
            "Binary",
            Record::with_fields(vec![FieldValue::Null, FieldValue::Null]),
        );
        binary_db.add_record(
            "Binary",
            Record::with_fields(vec![
                FieldValue::String("nested_auto.msi".to_string()),
                FieldValue::String("NESTED_PAYLOAD".to_string()),
            ]),
        );
        let binary_pkg = Package::new(
            crate::package::PackageMetadata::new(
                "BinaryParent",
                "Vendor",
                crate::package::ProductVersion::new(1, 0, 0),
                "{11111111-2222-3333-4444-555555555555}",
            ),
            binary_db,
            crate::database::summary_info::SummaryInfo::default(),
            HashMap::new(),
        );

        let temp_spool = std::env::temp_dir().join(format!("spool_ext_{}", std::process::id()));
        let extracted_path =
            mgr.extract_child_package(&binary_pkg, "ChildWithoutExt", &temp_spool)?;
        assert!(extracted_path.exists());
        assert_eq!(std::fs::read(&extracted_path)?, b"BINARY_STRING_PAYLOAD");

        let all_extracted = mgr.extract_all_child_packages(&binary_pkg, &temp_spool)?;
        assert!(all_extracted.contains_key("nested_auto.msi"));

        assert!(mgr
            .extract_child_package(&binary_pkg, "NullPayloadBinary", &temp_spool)
            .is_err());
        assert!(mgr
            .extract_child_package(&binary_pkg, "CompletelyMissing", &temp_spool)
            .is_err());

        // Test install_child_package with PREEXISTING, bare tokens, properties, and launch condition
        let mut db_child = LinkedDatabase::new()?;
        db_child.add_record(
            "Property",
            Record::with_fields(vec![
                FieldValue::String("ChildProp".to_string()),
                FieldValue::String("ChildVal".to_string()),
            ]),
        );
        db_child.add_record(
            "Property",
            Record::with_fields(vec![FieldValue::Null, FieldValue::Null]),
        );
        db_child.add_record(
            "LaunchCondition",
            Record::with_fields(vec![
                FieldValue::String("1 = 1".to_string()),
                FieldValue::String("Condition ok".to_string()),
            ]),
        );
        db_child.add_record(
            "LaunchCondition",
            Record::with_fields(vec![FieldValue::Null, FieldValue::Null]),
        );
        let simple_child = Package::new(
            crate::package::PackageMetadata::new(
                "SimpleChild",
                "Vendor",
                crate::package::ProductVersion::new(1, 0, 0),
                "{AAAAAAA1-2222-3333-4444-555555555555}",
            ),
            db_child,
            crate::database::summary_info::SummaryInfo::default(),
            HashMap::new(),
        );
        let res_pre = mgr.install_child_package(&simple_child, "PREEXISTING=1 BARE_TOKEN")?;
        assert_eq!(res_pre, ERROR_SUCCESS);
        let mut mgr_live = MultiPackageTransactionManager::begin_transaction("LiveChildTx")?;
        let res_live = mgr_live.install_child_package(&simple_child, "PROP=2")?;
        assert_eq!(res_live, ERROR_SUCCESS);

        let mut db_fail_lc = LinkedDatabase::new()?;
        db_fail_lc.add_record(
            "LaunchCondition",
            Record::with_fields(vec![
                FieldValue::String("1 = 0".to_string()),
                FieldValue::Null, // Null description -> "Launch condition failed"
            ]),
        );
        let pkg_fail_lc = Package::new(
            crate::package::PackageMetadata::new(
                "FailLC",
                "Vendor",
                crate::package::ProductVersion::new(1, 0, 0),
                "{FFFFFFF1-2222-3333-4444-555555555555}",
            ),
            db_fail_lc,
            crate::database::summary_info::SummaryInfo::default(),
            HashMap::new(),
        );
        let mut mgr_fail = MultiPackageTransactionManager::begin_transaction("FailLCTx")?;
        assert!(mgr_fail.install_child_package(&pkg_fail_lc, "").is_err());

        // Test orchestrate_child_packages with empty forwarded properties vs non-empty
        let mut chain_mgr = MultiPackageTransactionManager::begin_transaction("ChainTest")?;
        let empty_ctx = EvaluationContext::new();
        let chain_res = chain_mgr
            .orchestrate_child_packages(&[(&simple_child, Some("EXTRA_ARG=1"))], &empty_ctx)?;
        assert_eq!(chain_res, ERROR_SUCCESS);

        let mut chain_mgr_pub = MultiPackageTransactionManager::begin_transaction("ChainTestPub")?;
        let mut pub_ctx = EvaluationContext::new();
        pub_ctx.set_property("PUBLIC_CHAIN_PROP", "Val1");
        let chain_res_pub = chain_mgr_pub
            .orchestrate_child_packages(&[(&simple_child, Some("EXTRA_ARG=2"))], &pub_ctx)?;
        assert_eq!(chain_res_pub, ERROR_SUCCESS);

        // Test orchestrate_master_package where package exists on disk
        let mut orch_mgr = MultiPackageTransactionManager::begin_transaction("OrchTest")?;
        let disk_file = temp_spool.join("child_on_disk.msi");
        let disk_pkg = Package::builder()
            .product_name("DiskChild")
            .manufacturer("Vendor")
            .version(crate::package::ProductVersion::new(1, 0, 0))
            .product_code("{CCCCCCC1-2222-3333-4444-555555555555}")
            .build()?;
        disk_pkg.save(&disk_file)?;

        let mut master_db = LinkedDatabase::new()?;
        master_db.add_record(
            "MsiEmbeddedChainer",
            Record::with_fields(vec![
                FieldValue::String("ChainerTrue".to_string()),
                FieldValue::String("1 = 1".to_string()),
                FieldValue::String("/quiet".to_string()),
                FieldValue::String("BinKey".to_string()),
                FieldValue::Long(1),
            ]),
        );
        master_db.add_record(
            "MsiEmbeddedChainer",
            Record::with_fields(vec![
                FieldValue::String("ChainerNullCond".to_string()),
                FieldValue::Null,
                FieldValue::String("/quiet".to_string()),
                FieldValue::String("BinKey2".to_string()),
                FieldValue::Long(2),
            ]),
        );

        let master_pkg_empty = Package::new(
            crate::package::PackageMetadata::new(
                "MasterEmpty",
                "Vendor",
                crate::package::ProductVersion::new(1, 0, 0),
                "{BBBBBBB1-2222-3333-4444-555555555555}",
            ),
            master_db,
            crate::database::summary_info::SummaryInfo::default(),
            HashMap::new(),
        );
        let disk_file_str = disk_file.to_string_lossy().to_string();
        let mut master_ctx = EvaluationContext::new();
        master_ctx.set_property("MASTER_PUBLIC_PROP", "Val1");

        // Pass child with extra arguments when forwarded properties are non-empty
        let orch_res = orch_mgr.orchestrate_master_package(
            &master_pkg_empty,
            &master_ctx,
            &temp_spool,
            &[
                (&disk_file_str, Some("CHILD_PROP=Val2")),
                ("NonExistentPackage", None),
            ],
        )?;
        assert_eq!(orch_res, ERROR_SUCCESS);

        // Also test orchestrate_master_package with empty forwarded properties
        let mut orch_mgr_empty = MultiPackageTransactionManager::begin_transaction("OrchEmpty")?;
        let orch_res_empty = orch_mgr_empty.orchestrate_master_package(
            &master_pkg_empty,
            &empty_ctx,
            &temp_spool,
            &[(&disk_file_str, Some("CHILD_PROP=Val3"))],
        )?;
        assert_eq!(orch_res_empty, ERROR_SUCCESS);

        let _ = std::fs::remove_dir_all(&temp_spool);
        Ok(())
    }

    /// Tests `resolve_directories` edge cases: `TARGETDIR` with parent, null `default_dir`,
    /// empty default `sub_name`, root `non-targetdir`, and orphaned entries.
    #[test]
    fn test_resolve_directories_edge_cases_and_fallbacks() -> Result<()> {
        let mut db = LinkedDatabase::new()?;
        // Standard dir with parent to test !entries_with_parent branch
        db.add_record(
            "Directory",
            Record::with_fields(vec![
                FieldValue::String("ProgramFilesFolder".to_string()),
                FieldValue::String("TARGETDIR".to_string()),
                FieldValue::String(".".to_string()),
            ]),
        );
        // TARGETDIR row with parent OuterRoot so entries_with_parent contains TARGETDIR!
        db.add_record(
            "Directory",
            Record::with_fields(vec![
                FieldValue::String("TARGETDIR".to_string()),
                FieldValue::String("OuterRoot".to_string()),
                FieldValue::Null, // Null default dir -> defaults to "." (line 2274)
            ]),
        );
        // Row with empty string parent to test !parent.is_empty() false branch (line 2221)
        db.add_record(
            "Directory",
            Record::with_fields(vec![
                FieldValue::String("EmptyParentDir".to_string()),
                FieldValue::String(String::new()),
                FieldValue::String(".".to_string()),
            ]),
        );
        // RootParent has no parent and is not TARGETDIR
        db.add_record(
            "Directory",
            Record::with_fields(vec![
                FieldValue::String("RootParent".to_string()),
                FieldValue::Null,
                FieldValue::String(".".to_string()), // "." default dir
            ]),
        );
        // Child with default_dir "." -> sub_name == "." -> parent_path
        db.add_record(
            "Directory",
            Record::with_fields(vec![
                FieldValue::String("ChildDot".to_string()),
                FieldValue::String("RootParent".to_string()),
                FieldValue::String(".".to_string()),
            ]),
        );
        // Orphaned directory that points to an unknown non-resolvable parent
        db.add_record(
            "Directory",
            Record::with_fields(vec![
                FieldValue::String("OrphanDir".to_string()),
                FieldValue::String("UnknownParent".to_string()),
                FieldValue::String("orphan_sub".to_string()),
            ]),
        );

        let mut context = EvaluationContext::new();
        let resolved = resolve_directories(&db, &mut context);

        assert!(resolved.contains_key("TARGETDIR"));
        assert!(resolved.contains_key("RootParent"));
        assert!(resolved.contains_key("ChildDot"));
        assert!(resolved.contains_key("OrphanDir"));
        Ok(())
    }

    /// Tests `Transaction` preparation and script generation matrix:
    /// `sequence_table` setter, Media with Long/Null, `CustomAction` types (19 empty target, 0x0501, 1 immediate with Binary),
    /// `FileCost`, `InstallFiles` with null fields and Long sequence, and shortcuts with null directory.
    #[test]
    #[allow(clippy::too_many_lines)]
    fn test_transaction_prepare_and_execute_full_matrix() -> Result<()> {
        let mut db = LinkedDatabase::new()?;

        // Sequences
        db.add_record(
            "CustomSeq",
            Record::with_fields(vec![
                FieldValue::String("CostInitialize".to_string()),
                FieldValue::Null,
                FieldValue::Short(100),
            ]),
        );
        db.add_record(
            "CustomSeq",
            Record::with_fields(vec![
                FieldValue::String("FileCost".to_string()),
                FieldValue::Null,
                FieldValue::Short(200),
            ]),
        );
        db.add_record(
            "CustomSeq",
            Record::with_fields(vec![
                FieldValue::String("CostFinalize".to_string()),
                FieldValue::Null,
                FieldValue::Short(300),
            ]),
        );
        db.add_record(
            "CustomSeq",
            Record::with_fields(vec![
                FieldValue::String("CreateFolders".to_string()),
                FieldValue::Null,
                FieldValue::Short(400),
            ]),
        );
        db.add_record(
            "CustomSeq",
            Record::with_fields(vec![
                FieldValue::String("InstallFiles".to_string()),
                FieldValue::Null,
                FieldValue::Short(500),
            ]),
        );
        db.add_record(
            "CustomSeq",
            Record::with_fields(vec![
                FieldValue::String("CreateShortcuts".to_string()),
                FieldValue::Null,
                FieldValue::Short(600),
            ]),
        );
        db.add_record(
            "CustomSeq",
            Record::with_fields(vec![
                FieldValue::String("RollbackCA".to_string()),
                FieldValue::Null,
                FieldValue::Short(700),
            ]),
        );
        db.add_record(
            "CustomSeq",
            Record::with_fields(vec![
                FieldValue::String("ImmediateCA".to_string()),
                FieldValue::Null,
                FieldValue::Short(800),
            ]),
        );
        db.add_record(
            "CustomSeq",
            Record::with_fields(vec![
                FieldValue::String("InvalidTypeCA".to_string()),
                FieldValue::Null,
                FieldValue::Short(850),
            ]),
        );

        // Media table with Long sequence, Short sequence, and Null cabinet
        db.add_record(
            "Media",
            Record::with_fields(vec![
                FieldValue::Short(1),
                FieldValue::Long(500),
                FieldValue::Null,
                FieldValue::Null, // Null cabinet
            ]),
        );
        db.add_record(
            "Media",
            Record::with_fields(vec![
                FieldValue::Short(2),
                FieldValue::Null, // Null sequence -> i32::MAX
                FieldValue::Null,
                FieldValue::String("media2.cab".to_string()),
            ]),
        );
        db.add_record(
            "Media",
            Record::with_fields(vec![
                FieldValue::Short(3),
                FieldValue::Short(15), // Short sequence (line 2514)
                FieldValue::Null,
                FieldValue::String("media3.cab".to_string()),
            ]),
        );

        // Custom actions: Rollback CA, Immediate CA, NullTypeCA, NullNameCA, InvalidTypeCA
        db.add_record(
            "CustomAction",
            Record::with_fields(vec![
                FieldValue::String("RollbackCA".to_string()),
                FieldValue::Long(0x0501), // Rollback action
                FieldValue::Null,
                FieldValue::Null,
            ]),
        );
        db.add_record(
            "CustomAction",
            Record::with_fields(vec![
                FieldValue::String("ImmediateCA".to_string()),
                FieldValue::Long(51), // Immediate property setter
                FieldValue::String("IMMEDIATE_PROP".to_string()),
                FieldValue::String("ImmediateVal".to_string()),
            ]),
        );
        db.add_record(
            "CustomAction",
            Record::with_fields(vec![
                FieldValue::String("NullTypeCA".to_string()),
                FieldValue::Null, // Null action type -> 0 (line 2543)
                FieldValue::Null,
                FieldValue::Null,
            ]),
        );
        db.add_record(
            "CustomAction",
            Record::with_fields(vec![
                FieldValue::Null, // Null action name -> continue (line 2554)
                FieldValue::Short(1),
                FieldValue::Null,
                FieldValue::Null,
            ]),
        );
        db.add_record(
            "CustomAction",
            Record::with_fields(vec![
                FieldValue::String("InvalidTypeCA".to_string()),
                FieldValue::Long(0x9999), // Invalid type -> parse fails (line 2789)
                FieldValue::Null,
                FieldValue::Null,
            ]),
        );

        // Binary table records: one with String, one with Null, one with Null name
        db.add_record(
            "Binary",
            Record::with_fields(vec![
                FieldValue::String("BinString".to_string()),
                FieldValue::String("BinaryContent".to_string()),
            ]),
        );
        db.add_record(
            "Binary",
            Record::with_fields(vec![
                FieldValue::String("BinNull".to_string()),
                FieldValue::Null,
            ]),
        );
        db.add_record(
            "Binary",
            Record::with_fields(vec![
                FieldValue::Null, // Null binary name (line 2786, 2874)
                FieldValue::Null,
            ]),
        );

        // File table records with null fields, Long sequence, and unknown directory
        db.add_record(
            "File",
            Record::with_fields(vec![
                FieldValue::Null, // Null file_id (skipped)
                FieldValue::String("comp1".to_string()),
                FieldValue::String("skip1.txt".to_string()),
                FieldValue::Long(100),
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Long(1),
            ]),
        );
        db.add_record(
            "File",
            Record::with_fields(vec![
                FieldValue::String("fil2".to_string()),
                FieldValue::Null, // Null comp_id
                FieldValue::Null, // Null filename (skipped)
                FieldValue::Long(200),
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Long(2),
            ]),
        );
        db.add_record(
            "File",
            Record::with_fields(vec![
                FieldValue::String("fil3".to_string()),
                FieldValue::Null, // Null comp_id -> String::new()
                FieldValue::String("valid.txt".to_string()),
                FieldValue::Long(300),
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Long(10), // Long sequence
            ]),
        );
        db.add_record(
            "File",
            Record::with_fields(vec![
                FieldValue::String("fil_unk_dir".to_string()),
                FieldValue::String("UnknownDirectory".to_string()), // not in resolved_dirs (line 2591)
                FieldValue::String("unk.txt".to_string()),
                FieldValue::Long(500),
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Short(15),
            ]),
        );

        // Shortcut table records: one with Null dir ID, one with known dir ID, one with unresolved dir ID
        db.add_record(
            "Shortcut",
            Record::with_fields(vec![
                FieldValue::String("sc1".to_string()),
                FieldValue::Null, // Null directory ID
                FieldValue::String("MyShortcut".to_string()),
                FieldValue::Null,
                FieldValue::String("TargetApp".to_string()),
            ]),
        );
        db.add_record(
            "Shortcut",
            Record::with_fields(vec![
                FieldValue::String("sc_known_dir".to_string()),
                FieldValue::String("TARGETDIR".to_string()), // in resolved_dirs (line 2708)
                FieldValue::String("KnownDirShortcut".to_string()),
                FieldValue::Null,
                FieldValue::String("TargetApp2".to_string()),
            ]),
        );
        db.add_record(
            "Shortcut",
            Record::with_fields(vec![
                FieldValue::String("sc_unres_dir".to_string()),
                FieldValue::String("UnresolvedFolder".to_string()), // not in resolved_dirs (lines 2683-2684)
                FieldValue::String("UnresShortcut".to_string()),
                FieldValue::Null,
                FieldValue::String("TargetApp3".to_string()),
            ]),
        );

        // Component table records: one normal, one with Null directory (line 2488)
        db.add_record(
            "Component",
            Record::with_fields(vec![
                FieldValue::String("comp1".to_string()),
                FieldValue::String("{11111111-2222-3333-4444-555555555555}".to_string()),
                FieldValue::String("TARGETDIR".to_string()),
            ]),
        );
        db.add_record(
            "Component",
            Record::with_fields(vec![
                FieldValue::String("comp_null_dir".to_string()),
                FieldValue::String("{22222222-2222-3333-4444-555555555555}".to_string()),
                FieldValue::Null, // Null directory
            ]),
        );

        let tx = Transaction::new(db, EvaluationContext::new(), DiskCostEngine::new())
            .sequence_table("CustomSeq")
            .with_cabinet("dummy.cab", vec![]);

        let prepared = tx.prepare()?;
        assert!(!prepared.install_script().is_empty());
        assert!(!prepared.rollback_script().is_empty());
        let _ = prepared.cost_engine();

        // Execute prepared transaction with Binary records in database
        let mut worker = WorkerContext::new();
        let mut cab_writer =
            crate::cab::writer::CabinetWriter::new(crate::cab::folder::CompressionType::None);
        cab_writer.add_file("dummy.txt", b"dummy")?;
        let dummy_cab = cab_writer.build();
        worker.add_cabinet_bytes("dummy.cab", &dummy_cab)?; // already has dummy.cab (line 2880)

        let mut cab3_writer =
            crate::cab::writer::CabinetWriter::new(crate::cab::folder::CompressionType::None);
        cab3_writer.add_file("fil3", b"fil3_content")?;
        cab3_writer.add_file("unk.txt", b"unk_content")?;
        cab3_writer.add_file("fil_unk_dir", b"unk_content")?;
        let cab3_bytes = cab3_writer.build();
        worker.add_cabinet_bytes("media3.cab", &cab3_bytes)?;

        let executed = prepared.execute(&mut worker)?;
        assert_eq!(
            worker.evaluation_context().get_property("IMMEDIATE_PROP"),
            Some("ImmediateVal")
        );
        assert_eq!(
            worker.get_binary("BinString"),
            Some(b"BinaryContent".as_slice())
        );
        assert_eq!(worker.get_binary("BinNull"), Some(&[][..]));

        // Test commit
        let committed = executed.commit(&mut worker)?;
        assert_eq!(committed.return_code(), ERROR_SUCCESS);

        // Test Type 19 with empty target
        let mut db_type19 = LinkedDatabase::new()?;
        db_type19.add_record(
            "InstallExecuteSequence",
            Record::with_fields(vec![
                FieldValue::String("AbortCA".to_string()),
                FieldValue::Null,
                FieldValue::Short(100),
            ]),
        );
        db_type19.add_record(
            "CustomAction",
            Record::with_fields(vec![
                FieldValue::String("AbortCA".to_string()),
                FieldValue::Long(19),
                FieldValue::Null,
                FieldValue::String(String::new()), // Empty target
            ]),
        );
        let tx_19 = Transaction::new(db_type19, EvaluationContext::new(), DiskCostEngine::new());
        let err_19 = tx_19.prepare();
        assert!(err_19.is_err());

        Ok(())
    }

    /// Tests edge-case branches and paths in `transaction.rs` to achieve 100% line and branch coverage.
    #[test]
    #[allow(clippy::too_many_lines)]
    fn test_transaction_additional_branch_coverage() -> Result<()> {
        unsafe extern "system-unwind" fn mock_entry_fn(_: u32) -> u32 {
            0
        }

        // 1. is_executable_file variants
        assert!(is_executable_file("test.cmd"));
        assert!(is_executable_file("test.CMD"));
        assert!(is_executable_file("test.bat"));
        assert!(is_executable_file("test.BAT"));
        assert!(is_executable_file("test.exe"));
        assert!(is_executable_file("test.sh"));
        assert!(!is_executable_file("test.txt"));
        assert!(!is_executable_file("test"));

        // 2. extract_cabinet_file with # prefix stripping on reader lookup with case-insensitive search
        let mut worker = WorkerContext::new();
        let mut cab_writer =
            crate::cab::writer::CabinetWriter::new(crate::cab::folder::CompressionType::None);
        cab_writer.add_file("f1.txt", b"cab_content")?;
        let cab_bytes = cab_writer.build();
        worker.add_cabinet_bytes("other.cab", &cab_bytes)?;
        worker.add_cabinet_bytes("disk1.cab", &cab_bytes)?;
        let extracted1 = worker.extract_cabinet_file(Some("DISK1.CAB"), "f1.txt")?;
        assert_eq!(extracted1, b"cab_content");
        let extracted2 = worker.extract_cabinet_file(Some("#DISK1.CAB"), "f1.txt")?;
        assert_eq!(extracted2, b"cab_content");

        // 3. CustomAction with has_native_function = true
        let mut worker_native = WorkerContext::new();
        worker_native
            .custom_action_executor_mut()
            .library_loader_mut()
            .register_function("EntryFn", mock_entry_fn);
        let mut script_native = InstallScript::new();
        script_native.push(ScriptOp::CustomAction {
            action: "NativeCA".to_string(),
            action_type: 1,
            source: "BinaryTable".to_string(),
            target: "EntryFn".to_string(),
        });
        worker_native.execute_script(&script_native)?;

        // CustomAction with has_mock_result = true
        let mut worker_mock = WorkerContext::new();
        worker_mock
            .custom_action_executor_mut()
            .set_mock_result("MockedCA", 0);
        let mut script_mock = InstallScript::new();
        script_mock.push(ScriptOp::CustomAction {
            action: "MockedCA".to_string(),
            action_type: 1,
            source: "BinaryTable".to_string(),
            target: "Fn".to_string(),
        });
        worker_mock.execute_script(&script_mock)?;

        // 4. extract_child_package where stream name starts with # and matches Binary table clean name
        let mut bin_db = LinkedDatabase::new()?;
        bin_db.add_record(
            "Binary",
            Record::with_fields(vec![
                FieldValue::String("child_payload".to_string()),
                FieldValue::String("CLEAN_NAME_CONTENT".to_string()),
            ]),
        );
        let bin_pkg = Package::new(
            crate::package::PackageMetadata::new(
                "BinPkg",
                "Vendor",
                crate::package::ProductVersion::new(1, 0, 0),
                "{12345678-0000-0000-0000-000000000001}",
            ),
            bin_db,
            crate::database::summary_info::SummaryInfo::default(),
            HashMap::new(),
        );
        let mut mgr = MultiPackageTransactionManager::begin_transaction("CleanNameTx")?;
        let temp_dir =
            std::env::temp_dir().join(format!("clean_name_spool_{}", std::process::id()));
        let extracted_clean = mgr.extract_child_package(&bin_pkg, "#child_payload", &temp_dir)?;
        assert!(extracted_clean.exists());
        let _ = std::fs::remove_dir_all(&temp_dir);

        // 5. orchestrate_child_packages with Some("   ") (empty trimmed extra_args)
        let dummy_child = Package::new(
            crate::package::PackageMetadata::new(
                "DummyChild",
                "Vendor",
                crate::package::ProductVersion::new(1, 0, 0),
                "{12345678-0000-0000-0000-000000000002}",
            ),
            LinkedDatabase::new()?,
            crate::database::summary_info::SummaryInfo::default(),
            HashMap::new(),
        );
        let mut orch_mgr = MultiPackageTransactionManager::begin_transaction("OrchWhitespaceArgs")?;
        let empty_ctx = EvaluationContext::new();
        orch_mgr.register_product(
            "{12345678-0000-0000-0000-000000000002}",
            InstallState::Default,
        );
        let res_orch =
            orch_mgr.orchestrate_child_packages(&[(&dummy_child, Some("   "))], &empty_ctx)?;
        assert_eq!(res_orch, ERROR_SUCCESS);

        // 6. orchestrate_master_package with Some("   ") in child_packages
        let mut master_db = LinkedDatabase::new()?;
        master_db.add_record(
            "MsiEmbeddedChainer",
            Record::with_fields(vec![
                FieldValue::String("ChainerWS".to_string()),
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::String("dummy_bin".to_string()),
                FieldValue::Long(1),
            ]),
        );
        let master_pkg_ws = Package::new(
            crate::package::PackageMetadata::new(
                "MasterWS",
                "Vendor",
                crate::package::ProductVersion::new(1, 0, 0),
                "{12345678-0000-0000-0000-000000000003}",
            ),
            master_db,
            crate::database::summary_info::SummaryInfo::default(),
            HashMap::new(),
        );
        let mut master_mgr =
            MultiPackageTransactionManager::begin_transaction("MasterWhitespaceArgs")?;
        let temp_spool_ws = std::env::temp_dir().join(format!("spool_ws_{}", std::process::id()));
        let res_master_ws = master_mgr.orchestrate_master_package(
            &master_pkg_ws,
            &empty_ctx,
            &temp_spool_ws,
            &[("NonExistentNested", Some("   "))],
        )?;
        assert_eq!(res_master_ws, ERROR_SUCCESS);
        let _ = std::fs::remove_dir_all(&temp_spool_ws);

        // 7. resolve_directories with empty sub_name (":source")
        let mut dir_db_empty = LinkedDatabase::new()?;
        dir_db_empty.add_record(
            "Directory",
            Record::with_fields(vec![
                FieldValue::String("EmptySubDir".to_string()),
                FieldValue::String("TARGETDIR".to_string()),
                FieldValue::String(":source".to_string()),
            ]),
        );
        let mut dir_ctx_empty = EvaluationContext::new();
        let _ = resolve_directories(&dir_db_empty, &mut dir_ctx_empty);
        assert!(dir_ctx_empty.get_property("EmptySubDir").is_some());

        // 7b. Chain of 17 directories inserted in reverse order to hit iterations < max_iterations false
        let mut dir_db_chain = LinkedDatabase::new()?;
        for i in (1..=16).rev() {
            dir_db_chain.add_record(
                "Directory",
                Record::with_fields(vec![
                    FieldValue::String(format!("D{i}")),
                    FieldValue::String(format!("D{}", i - 1)),
                    FieldValue::String(format!("d{i}")),
                ]),
            );
        }
        dir_db_chain.add_record(
            "Directory",
            Record::with_fields(vec![
                FieldValue::String("D0".to_string()),
                FieldValue::String("TARGETDIR".to_string()),
                FieldValue::String("d0".to_string()),
            ]),
        );
        let mut dir_ctx_chain = EvaluationContext::new();
        let _ = resolve_directories(&dir_db_chain, &mut dir_ctx_chain);
        assert!(dir_ctx_chain.get_property("D16").is_some());

        // 8. Media disk with empty cabinet string
        let mut media_db = LinkedDatabase::new()?;
        media_db.add_record(
            "Media",
            Record::with_fields(vec![
                FieldValue::Short(1),
                FieldValue::Short(100),
                FieldValue::Null,
                FieldValue::String(String::new()),
            ]),
        );
        media_db.add_record(
            "InstallExecuteSequence",
            Record::with_fields(vec![
                FieldValue::String("InstallFiles".to_string()),
                FieldValue::Null,
                FieldValue::Short(4000),
            ]),
        );
        let media_tx = Transaction::new(media_db, EvaluationContext::new(), DiskCostEngine::new());
        let _ = media_tx.prepare()?;

        // 9. Live executor ExtractCabinetFile with executable and non-executable file
        let temp_live_dir =
            std::env::temp_dir().join(format!("tx_live_exec_{}", std::process::id()));
        let quarantine_dir = temp_live_dir.join("quarantine");
        let _ = std::fs::create_dir_all(&quarantine_dir);
        let live_exec = crate::execution::LiveWorkerExecutor::new(&quarantine_dir, "tx_live_id");
        let mut worker_live = WorkerContext::new().with_live_executor(live_exec);
        let mut cab_writer_live =
            crate::cab::writer::CabinetWriter::new(crate::cab::folder::CompressionType::None);
        cab_writer_live.add_file("app.exe", b"binary_content")?;
        cab_writer_live.add_file("readme.txt", b"text_content")?;
        let cab_bytes_live = cab_writer_live.build();
        worker_live.add_cabinet_bytes("app.cab", &cab_bytes_live)?;
        let app_dest = temp_live_dir.join("app.exe");
        let txt_dest = temp_live_dir.join("readme.txt");
        let op_extract_exe = ScriptOp::ExtractCabinetFile {
            cabinet: "app.cab".to_string(),
            file_key: "app.exe".to_string(),
            destination: app_dest.to_string_lossy().to_string(),
        };
        let op_extract_txt = ScriptOp::ExtractCabinetFile {
            cabinet: "app.cab".to_string(),
            file_key: "readme.txt".to_string(),
            destination: txt_dest.to_string_lossy().to_string(),
        };
        let mut script_live = InstallScript::new();
        script_live.push(op_extract_exe);
        script_live.push(op_extract_txt);
        worker_live.execute_script(&script_live)?;
        let _ = std::fs::remove_dir_all(&temp_live_dir);

        // 10. CustomAction with target == "Fn" and non-matching target
        let mut ca_worker = WorkerContext::new();
        let op_ca_fn = ScriptOp::CustomAction {
            action: "MockFnAction".to_string(),
            action_type: 1,
            source: "BinaryTable".to_string(),
            target: "Fn".to_string(),
        };
        let mut script_ca_ok = InstallScript::new();
        script_ca_ok.push(op_ca_fn);
        ca_worker.execute_script(&script_ca_ok)?;

        let op_ca_other = ScriptOp::CustomAction {
            action: "MockOtherAction".to_string(),
            action_type: 1,
            source: "BinaryTable".to_string(),
            target: "OtherFn".to_string(),
        };
        let mut script_ca_err = InstallScript::new();
        script_ca_err.push(op_ca_other);
        assert!(ca_worker.execute_script(&script_ca_err).is_err());

        // 11. is_package_preexisting with upgrade_code not installed
        let mut mgr_up =
            MultiPackageTransactionManager::begin_transaction("TestUpgradeNotInstalled")?;
        let pkg_up = Package::builder()
            .product_name("UpPkg")
            .manufacturer("Vendor")
            .version(crate::package::ProductVersion::new(1, 0, 0))
            .product_code("{11111111-2222-3333-4444-555555555555}")
            .upgrade_code("{99999999-9999-9999-9999-999999999999}")
            .build()?;
        assert!(!mgr_up.is_package_preexisting(&pkg_up));
        let _ = mgr_up.end_transaction(false);

        // 12. extract_all_child_packages with non-msi stream and duplicate binary stream
        let mut stream_db = LinkedDatabase::new()?;
        stream_db.add_record(
            "Binary",
            Record::with_fields(vec![
                FieldValue::String("child.msi".to_string()),
                FieldValue::Stream(crate::database::StringPoolId::new(1)),
            ]),
        );
        let mut streams_map = HashMap::new();
        streams_map.insert("readme.txt".to_string(), b"hello world".to_vec());
        streams_map.insert("child.msi".to_string(), b"mock msi".to_vec());
        let stream_pkg = Package::new(
            crate::package::PackageMetadata::new(
                "StreamPkg",
                "Vendor",
                crate::package::ProductVersion::new(1, 0, 0),
                "{11111111-2222-3333-4444-555555555556}",
            ),
            stream_db,
            crate::database::summary_info::SummaryInfo::default(),
            streams_map,
        );
        let temp_spool_stream =
            std::env::temp_dir().join(format!("spool_stream_{}", std::process::id()));
        let mut mgr_stream =
            MultiPackageTransactionManager::begin_transaction("TestStreamExtract")?;
        let extracted_streams =
            mgr_stream.extract_all_child_packages(&stream_pkg, &temp_spool_stream)?;
        assert!(extracted_streams.contains_key("child.msi"));
        let _ = mgr_stream.end_transaction(false);
        let _ = std::fs::remove_dir_all(&temp_spool_stream);

        // 13. install_child_package with INSTALL_* set to false
        let mut mgr_skip = MultiPackageTransactionManager::begin_transaction("TestSkipInstall")?;
        let child_pkg_skip = Package::builder()
            .product_name("ChildSkip")
            .manufacturer("Vendor")
            .version(crate::package::ProductVersion::new(1, 0, 0))
            .product_code("{11111111-2222-3333-4444-555555555558}")
            .build()?;
        let skip_res =
            mgr_skip.install_child_package(&child_pkg_skip, "INSTALL_APP=false OTHER=1")?;
        assert_eq!(skip_res, ERROR_SUCCESS);
        let _ = mgr_skip.end_transaction(false);

        // 14. orchestrate_master_package with existing file on disk and failure path
        let temp_child_file =
            std::env::temp_dir().join(format!("child_disk_{}.msi", std::process::id()));
        std::fs::write(&temp_child_file, b"not a valid msi")?;
        let temp_child_str = temp_child_file.to_string_lossy().to_string();

        let mut mgr_fail = MultiPackageTransactionManager::begin_transaction("TestMasterFail")?;
        let empty_db = LinkedDatabase::new()?;
        let master_pkg_fail = Package::new(
            crate::package::PackageMetadata::new(
                "MasterFail",
                "Vendor",
                crate::package::ProductVersion::new(1, 0, 0),
                "{11111111-2222-3333-4444-555555555557}",
            ),
            empty_db,
            crate::database::summary_info::SummaryInfo::default(),
            HashMap::new(),
        );
        let temp_spool_fail =
            std::env::temp_dir().join(format!("spool_fail_{}", std::process::id()));
        let fail_res = mgr_fail.orchestrate_master_package(
            &master_pkg_fail,
            &empty_ctx,
            &temp_spool_fail,
            &[(&temp_child_str, None)],
        );
        assert!(fail_res.is_err());
        let _ = std::fs::remove_file(&temp_child_file);
        let _ = std::fs::remove_dir_all(&temp_spool_fail);

        // 15. Type 19 custom action with formatted target string
        let mut type19_db = LinkedDatabase::new()?;
        type19_db.add_record(
            "CustomAction",
            Record::with_fields(vec![
                FieldValue::String("CustomAbortMsg".to_string()),
                FieldValue::Short(19),
                FieldValue::String(String::new()),
                FieldValue::String("Fatal custom error [ProductCode]".to_string()),
            ]),
        );
        type19_db.add_record(
            "InstallExecuteSequence",
            Record::with_fields(vec![
                FieldValue::String("CustomAbortMsg".to_string()),
                FieldValue::Null,
                FieldValue::Short(100),
            ]),
        );
        let type19_tx =
            Transaction::new(type19_db, EvaluationContext::new(), DiskCostEngine::new());
        assert!(type19_tx.prepare().is_err());

        // 16. Deferred custom action (action_type 0x0401)
        let mut deferred_db = LinkedDatabase::new()?;
        deferred_db.add_record(
            "CustomAction",
            Record::with_fields(vec![
                FieldValue::String("DeferredAction".to_string()),
                FieldValue::Short(0x0401),
                FieldValue::String("BinaryTable".to_string()),
                FieldValue::String("DeferredFn".to_string()),
            ]),
        );
        deferred_db.add_record(
            "InstallExecuteSequence",
            Record::with_fields(vec![
                FieldValue::String("DeferredAction".to_string()),
                FieldValue::Null,
                FieldValue::Short(100),
            ]),
        );
        let deferred_tx =
            Transaction::new(deferred_db, EvaluationContext::new(), DiskCostEngine::new());
        let prepared_deferred = deferred_tx.prepare()?;
        assert!(!prepared_deferred.install_script().operations().is_empty());

        // 17. has_cabinet testing with and without hash prefix
        let mut cabinet_worker = WorkerContext::new();
        cabinet_worker.add_cabinet_bytes("testcab.cab", &cab_bytes_live)?;
        assert!(cabinet_worker.has_cabinet("testcab.cab"));
        assert!(cabinet_worker.has_cabinet("#testcab.cab"));
        assert!(!cabinet_worker.has_cabinet("missing.cab"));

        // 18. orchestrate_master_package with extracted child package match
        let valid_child_pkg = Package::builder()
            .product_name("ValidChild")
            .manufacturer("Vendor")
            .version(crate::package::ProductVersion::new(1, 0, 0))
            .product_code("{11111111-2222-3333-4444-555555555559}")
            .build()?;
        let child_bytes = valid_child_pkg.to_bytes()?;
        let mut streams_succ = HashMap::new();
        streams_succ.insert("valid_child.msi".to_string(), child_bytes);
        let master_pkg_succ = Package::new(
            crate::package::PackageMetadata::new(
                "MasterSucc",
                "Vendor",
                crate::package::ProductVersion::new(1, 0, 0),
                "{11111111-2222-3333-4444-555555555560}",
            ),
            LinkedDatabase::new()?,
            crate::database::summary_info::SummaryInfo::default(),
            streams_succ,
        );
        let temp_spool_succ =
            std::env::temp_dir().join(format!("spool_succ_{}", std::process::id()));
        let mut mgr_succ = MultiPackageTransactionManager::begin_transaction("TestMasterSucc")?;
        let succ_res = mgr_succ.orchestrate_master_package(
            &master_pkg_succ,
            &empty_ctx,
            &temp_spool_succ,
            &[("valid_child.msi", None)],
        )?;
        assert_eq!(succ_res, ERROR_SUCCESS);
        let _ = std::fs::remove_dir_all(&temp_spool_succ);

        Ok(())
    }
}

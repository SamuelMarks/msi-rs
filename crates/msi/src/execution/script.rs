//! Structured Windows Installer Script Engine (`.ibs` and `.rbs` formats).
//!
//! Grounded directly in official Windows Installer execution specifications:
//! - `.ibs` (Installation Binary Script): Compiled deferred execution commands executed by the
//!   privileged installation worker.
//! - `.rbs` (Rollback Binary Script): Compensating reverse commands played in reverse chronological
//!   order upon error or cancellation.
//! - `.rbf` (Rollback File): Quarantine files storing pristine original copies of overwritten artifacts.

use crate::error::{Error, Result};
use std::fmt;

/// Magic header bytes identifying an MSI Installation Binary Script (`.ibs`).
pub const IBS_MAGIC: &[u8; 8] = b"MSIIBS\x01\x00";

/// Magic header bytes identifying an MSI Rollback Binary Script (`.rbs`).
pub const RBS_MAGIC: &[u8; 8] = b"MSIRBS\x01\x00";

/// Single deferred operation in an installation script (`.ibs`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ScriptOp {
    /// Creates a directory on the target filesystem.
    CreateFolder {
        /// Target directory path.
        path: String,
    },
    /// Removes a directory from the target filesystem.
    RemoveFolder {
        /// Target directory path.
        path: String,
    },
    /// Copies a source file to destination with optional overwrite flag.
    CopyFile {
        /// Source file path.
        source: String,
        /// Destination file path.
        destination: String,
        /// Whether overwriting existing target is permitted.
        overwrite: bool,
    },
    /// Writes embedded or extracted binary content directly to a destination file.
    WriteFile {
        /// Destination file path.
        destination: String,
        /// Raw file payload bytes.
        content: Vec<u8>,
    },
    /// Deletes a file on the target filesystem.
    DeleteFile {
        /// Path of file to delete.
        path: String,
    },
    /// Backs up an existing target file to quarantine storage (`.rbf`) prior to replacement.
    BackupFile {
        /// Target file path to backup.
        target_path: String,
        /// Quarantine destination path.
        quarantine_path: String,
    },
    /// Writes a Windows registry key and value.
    WriteRegistry {
        /// Root key index (0: HKCR, 1: HKCU, 2: HKLM, 3: HKU).
        root: u32,
        /// Subkey path.
        key: String,
        /// Value name (None for default value).
        name: Option<String>,
        /// Value data.
        value: Option<String>,
    },
    /// Deletes a registry key or value.
    DeleteRegistry {
        /// Root key index.
        root: u32,
        /// Subkey path.
        key: String,
        /// Value name (None to delete subkey or default value).
        name: Option<String>,
    },
    /// Creates a shell shortcut or symlink.
    CreateShortcut {
        /// Target path of shortcut link.
        target: String,
        /// Link destination file path.
        link_path: String,
        /// Optional command-line arguments.
        arguments: Option<String>,
        /// Optional icon file path.
        icon_path: Option<String>,
        /// Optional icon index.
        icon_index: Option<i32>,
    },
    /// Deletes a shell shortcut or symlink.
    DeleteShortcut {
        /// Link path to remove.
        link_path: String,
    },
    /// Registers or installs a background service or daemon.
    InstallService {
        /// Internal service name.
        name: String,
        /// Friendly display name.
        display_name: String,
        /// Service type flags.
        service_type: u32,
        /// Start type (e.g. auto, manual, disabled).
        start_type: u32,
        /// Path to service binary executable.
        binary_path: String,
    },
    /// Deletes or unregisters a service or daemon.
    DeleteService {
        /// Service name to remove.
        name: String,
    },
    /// Starts a service or daemon.
    StartService {
        /// Service name.
        name: String,
        /// Optional launch arguments.
        arguments: Option<String>,
    },
    /// Stops a running service or daemon.
    StopService {
        /// Service name.
        name: String,
    },
    /// Executes an in-script deferred custom action.
    CustomAction {
        /// Action name.
        action: String,
        /// Custom action type flags bitmask.
        action_type: u32,
        /// Source identifier or binary reference.
        source: String,
        /// Target or parameter string.
        target: String,
    },
}

impl ScriptOp {
    /// Returns the opcode byte identifier for serialization.
    #[must_use]
    pub const fn opcode_id(&self) -> u8 {
        match self {
            Self::CreateFolder { .. } => 1,
            Self::RemoveFolder { .. } => 2,
            Self::CopyFile { .. } => 3,
            Self::WriteFile { .. } => 4,
            Self::DeleteFile { .. } => 5,
            Self::BackupFile { .. } => 6,
            Self::WriteRegistry { .. } => 7,
            Self::DeleteRegistry { .. } => 8,
            Self::CreateShortcut { .. } => 9,
            Self::DeleteShortcut { .. } => 10,
            Self::InstallService { .. } => 11,
            Self::DeleteService { .. } => 12,
            Self::StartService { .. } => 13,
            Self::StopService { .. } => 14,
            Self::CustomAction { .. } => 15,
        }
    }
}

/// Single compensating reverse operation in a rollback script (`.rbs`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RollbackOp {
    /// Restores a quarantined file (`.rbf`) back to its original location.
    RestoreQuarantinedFile {
        /// Original target file path.
        target_path: String,
        /// Quarantine file location.
        quarantine_path: String,
    },
    /// Deletes a file created during deferred execution.
    DeleteCreatedFile {
        /// File path to delete.
        path: String,
    },
    /// Deletes a directory created during deferred execution.
    DeleteCreatedFolder {
        /// Directory path to delete.
        path: String,
    },
    /// Restores a previous registry value or deletes an added registry key.
    RestoreRegistry {
        /// Root key index.
        root: u32,
        /// Subkey path.
        key: String,
        /// Value name.
        name: Option<String>,
        /// Previous value if existed.
        previous_value: Option<String>,
        /// Whether the value/key existed before transaction.
        existed: bool,
    },
    /// Deletes a shortcut created during deferred execution.
    DeleteShortcut {
        /// Link file path.
        link_path: String,
    },
    /// Deletes a service installed during deferred execution.
    DeleteService {
        /// Service name.
        name: String,
    },
    /// Stops a service started during deferred execution.
    StopService {
        /// Service name.
        name: String,
    },
    /// Executes a rollback custom action.
    RollbackCustomAction {
        /// Action name.
        action: String,
        /// Custom action type flags bitmask.
        action_type: u32,
        /// Source identifier or binary reference.
        source: String,
        /// Target or parameter string.
        target: String,
    },
}

impl RollbackOp {
    /// Returns the opcode byte identifier for serialization.
    #[must_use]
    pub const fn opcode_id(&self) -> u8 {
        match self {
            Self::RestoreQuarantinedFile { .. } => 1,
            Self::DeleteCreatedFile { .. } => 2,
            Self::DeleteCreatedFolder { .. } => 3,
            Self::RestoreRegistry { .. } => 4,
            Self::DeleteShortcut { .. } => 5,
            Self::DeleteService { .. } => 6,
            Self::StopService { .. } => 7,
            Self::RollbackCustomAction { .. } => 8,
        }
    }
}

/// In-memory representation of an installation execution script (`.ibs`).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct InstallScript {
    /// List of ordered execution operations.
    operations: Vec<ScriptOp>,
}

impl InstallScript {
    /// Creates a new empty [`InstallScript`].
    ///
    /// # Returns
    ///
    /// An empty [`InstallScript`].
    #[must_use]
    pub const fn new() -> Self {
        Self {
            operations: Vec::new(),
        }
    }

    /// Appends an operation to the script.
    ///
    /// # Arguments
    ///
    /// * `op` - The [`ScriptOp`] to add.
    pub fn push(&mut self, op: ScriptOp) {
        self.operations.push(op);
    }

    /// Returns a slice of the script operations.
    ///
    /// # Returns
    ///
    /// Slice of [`ScriptOp`].
    #[must_use]
    pub fn operations(&self) -> &[ScriptOp] {
        &self.operations
    }

    /// Returns the number of operations in the script.
    ///
    /// # Returns
    ///
    /// Count of operations.
    #[must_use]
    pub fn len(&self) -> usize {
        self.operations.len()
    }

    /// Returns true if the script contains no operations.
    ///
    /// # Returns
    ///
    /// `true` if empty, `false` otherwise.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.operations.is_empty()
    }

    /// Serializes the script into binary `.ibs` format.
    ///
    /// # Returns
    ///
    /// Byte vector of binary `.ibs` data.
    #[must_use]
    #[allow(clippy::too_many_lines)]
    pub fn serialize(&self) -> Vec<u8> {
        let mut out = Vec::new();
        out.extend_from_slice(IBS_MAGIC);
        let count = u32::try_from(self.operations.len()).unwrap_or(u32::MAX);
        out.extend_from_slice(&count.to_le_bytes());

        for op in &self.operations {
            out.push(op.opcode_id());
            match op {
                ScriptOp::CreateFolder { path }
                | ScriptOp::RemoveFolder { path }
                | ScriptOp::DeleteFile { path } => {
                    write_string(&mut out, path);
                }
                ScriptOp::CopyFile {
                    source,
                    destination,
                    overwrite,
                } => {
                    write_string(&mut out, source);
                    write_string(&mut out, destination);
                    out.push(u8::from(*overwrite));
                }
                ScriptOp::WriteFile {
                    destination,
                    content,
                } => {
                    write_string(&mut out, destination);
                    write_bytes(&mut out, content);
                }
                ScriptOp::BackupFile {
                    target_path,
                    quarantine_path,
                } => {
                    write_string(&mut out, target_path);
                    write_string(&mut out, quarantine_path);
                }
                ScriptOp::WriteRegistry {
                    root,
                    key,
                    name,
                    value,
                } => {
                    out.extend_from_slice(&root.to_le_bytes());
                    write_string(&mut out, key);
                    write_opt_string(&mut out, name.as_deref());
                    write_opt_string(&mut out, value.as_deref());
                }
                ScriptOp::DeleteRegistry { root, key, name } => {
                    out.extend_from_slice(&root.to_le_bytes());
                    write_string(&mut out, key);
                    write_opt_string(&mut out, name.as_deref());
                }
                ScriptOp::CreateShortcut {
                    target,
                    link_path,
                    arguments,
                    icon_path,
                    icon_index,
                } => {
                    write_string(&mut out, target);
                    write_string(&mut out, link_path);
                    write_opt_string(&mut out, arguments.as_deref());
                    write_opt_string(&mut out, icon_path.as_deref());
                    if let Some(idx) = icon_index {
                        out.push(1);
                        out.extend_from_slice(&idx.to_le_bytes());
                    } else {
                        out.push(0);
                    }
                }
                ScriptOp::DeleteShortcut { link_path } => {
                    write_string(&mut out, link_path);
                }
                ScriptOp::InstallService {
                    name,
                    display_name,
                    service_type,
                    start_type,
                    binary_path,
                } => {
                    write_string(&mut out, name);
                    write_string(&mut out, display_name);
                    out.extend_from_slice(&service_type.to_le_bytes());
                    out.extend_from_slice(&start_type.to_le_bytes());
                    write_string(&mut out, binary_path);
                }
                ScriptOp::DeleteService { name } | ScriptOp::StopService { name } => {
                    write_string(&mut out, name);
                }
                ScriptOp::StartService { name, arguments } => {
                    write_string(&mut out, name);
                    write_opt_string(&mut out, arguments.as_deref());
                }
                ScriptOp::CustomAction {
                    action,
                    action_type,
                    source,
                    target,
                } => {
                    write_string(&mut out, action);
                    out.extend_from_slice(&action_type.to_le_bytes());
                    write_string(&mut out, source);
                    write_string(&mut out, target);
                }
            }
        }

        out
    }

    /// Deserializes an [`InstallScript`] from binary `.ibs` data.
    ///
    /// # Arguments
    ///
    /// * `data` - Raw `.ibs` byte slice.
    ///
    /// # Returns
    ///
    /// A parsed [`InstallScript`].
    ///
    /// # Errors
    ///
    /// Returns [`Error::ScriptError`] if the magic header is invalid or data is truncated.
    #[allow(clippy::too_many_lines)]
    pub fn deserialize(data: &[u8]) -> Result<Self> {
        if data.len() < 12 {
            return Err(Error::ScriptError {
                opcode: "Header".to_string(),
                reason: "Data too short for IBS header".to_string(),
            });
        }
        if &data[0..8] != IBS_MAGIC {
            return Err(Error::ScriptError {
                opcode: "Header".to_string(),
                reason: "Invalid IBS magic header".to_string(),
            });
        }

        let mut offset = 8;
        let count = read_u32(data, &mut offset)?;
        let mut operations = Vec::with_capacity(count as usize);

        for _ in 0..count {
            if offset >= data.len() {
                return Err(Error::ScriptError {
                    opcode: "Opcode".to_string(),
                    reason: "Unexpected EOF reading opcode ID".to_string(),
                });
            }
            let opcode_id = data[offset];
            offset += 1;

            let op = match opcode_id {
                1 => ScriptOp::CreateFolder {
                    path: read_string(data, &mut offset)?,
                },
                2 => ScriptOp::RemoveFolder {
                    path: read_string(data, &mut offset)?,
                },
                3 => {
                    let source = read_string(data, &mut offset)?;
                    let destination = read_string(data, &mut offset)?;
                    if offset >= data.len() {
                        return Err(Error::ScriptError {
                            opcode: "CopyFile".to_string(),
                            reason: "Unexpected EOF reading overwrite flag".to_string(),
                        });
                    }
                    let overwrite = data[offset] != 0;
                    offset += 1;
                    ScriptOp::CopyFile {
                        source,
                        destination,
                        overwrite,
                    }
                }
                4 => {
                    let destination = read_string(data, &mut offset)?;
                    let content = read_bytes(data, &mut offset)?;
                    ScriptOp::WriteFile {
                        destination,
                        content,
                    }
                }
                5 => ScriptOp::DeleteFile {
                    path: read_string(data, &mut offset)?,
                },
                6 => {
                    let target_path = read_string(data, &mut offset)?;
                    let quarantine_path = read_string(data, &mut offset)?;
                    ScriptOp::BackupFile {
                        target_path,
                        quarantine_path,
                    }
                }
                7 => {
                    let root = read_u32(data, &mut offset)?;
                    let key = read_string(data, &mut offset)?;
                    let name = read_opt_string(data, &mut offset)?;
                    let value = read_opt_string(data, &mut offset)?;
                    ScriptOp::WriteRegistry {
                        root,
                        key,
                        name,
                        value,
                    }
                }
                8 => {
                    let root = read_u32(data, &mut offset)?;
                    let key = read_string(data, &mut offset)?;
                    let name = read_opt_string(data, &mut offset)?;
                    ScriptOp::DeleteRegistry { root, key, name }
                }
                9 => {
                    let target = read_string(data, &mut offset)?;
                    let link_path = read_string(data, &mut offset)?;
                    let arguments = read_opt_string(data, &mut offset)?;
                    let icon_path = read_opt_string(data, &mut offset)?;
                    if offset >= data.len() {
                        return Err(Error::ScriptError {
                            opcode: "CreateShortcut".to_string(),
                            reason: "Unexpected EOF reading icon index flag".to_string(),
                        });
                    }
                    let has_icon = data[offset] != 0;
                    offset += 1;
                    let icon_index = if has_icon {
                        Some(read_i32(data, &mut offset)?)
                    } else {
                        None
                    };
                    ScriptOp::CreateShortcut {
                        target,
                        link_path,
                        arguments,
                        icon_path,
                        icon_index,
                    }
                }
                10 => ScriptOp::DeleteShortcut {
                    link_path: read_string(data, &mut offset)?,
                },
                11 => {
                    let name = read_string(data, &mut offset)?;
                    let display_name = read_string(data, &mut offset)?;
                    let service_type = read_u32(data, &mut offset)?;
                    let start_type = read_u32(data, &mut offset)?;
                    let binary_path = read_string(data, &mut offset)?;
                    ScriptOp::InstallService {
                        name,
                        display_name,
                        service_type,
                        start_type,
                        binary_path,
                    }
                }
                12 => ScriptOp::DeleteService {
                    name: read_string(data, &mut offset)?,
                },
                13 => {
                    let name = read_string(data, &mut offset)?;
                    let arguments = read_opt_string(data, &mut offset)?;
                    ScriptOp::StartService { name, arguments }
                }
                14 => ScriptOp::StopService {
                    name: read_string(data, &mut offset)?,
                },
                15 => {
                    let action = read_string(data, &mut offset)?;
                    let action_type = read_u32(data, &mut offset)?;
                    let source = read_string(data, &mut offset)?;
                    let target = read_string(data, &mut offset)?;
                    ScriptOp::CustomAction {
                        action,
                        action_type,
                        source,
                        target,
                    }
                }
                other => {
                    return Err(Error::ScriptError {
                        opcode: format!("Opcode({other})"),
                        reason: "Unknown script opcode ID".to_string(),
                    });
                }
            };

            operations.push(op);
        }

        Ok(Self { operations })
    }
}

/// In-memory representation of a rollback script (`.rbs`).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RollbackScript {
    /// List of ordered compensating rollback operations.
    operations: Vec<RollbackOp>,
}

impl RollbackScript {
    /// Creates a new empty [`RollbackScript`].
    ///
    /// # Returns
    ///
    /// An empty [`RollbackScript`].
    #[must_use]
    pub const fn new() -> Self {
        Self {
            operations: Vec::new(),
        }
    }

    /// Appends a compensating operation to the rollback script.
    ///
    /// # Arguments
    ///
    /// * `op` - The [`RollbackOp`] to add.
    pub fn push(&mut self, op: RollbackOp) {
        self.operations.push(op);
    }

    /// Returns a slice of the rollback operations.
    ///
    /// # Returns
    ///
    /// Slice of [`RollbackOp`].
    #[must_use]
    pub fn operations(&self) -> &[RollbackOp] {
        &self.operations
    }

    /// Returns the number of operations in the rollback script.
    ///
    /// # Returns
    ///
    /// Count of operations.
    #[must_use]
    pub fn len(&self) -> usize {
        self.operations.len()
    }

    /// Returns true if the script contains no operations.
    ///
    /// # Returns
    ///
    /// `true` if empty, `false` otherwise.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.operations.is_empty()
    }

    /// Serializes the rollback script into binary `.rbs` format.
    ///
    /// # Returns
    ///
    /// Byte vector of binary `.rbs` data.
    #[must_use]
    pub fn serialize(&self) -> Vec<u8> {
        let mut out = Vec::new();
        out.extend_from_slice(RBS_MAGIC);
        let count = u32::try_from(self.operations.len()).unwrap_or(u32::MAX);
        out.extend_from_slice(&count.to_le_bytes());

        for op in &self.operations {
            out.push(op.opcode_id());
            match op {
                RollbackOp::RestoreQuarantinedFile {
                    target_path,
                    quarantine_path,
                } => {
                    write_string(&mut out, target_path);
                    write_string(&mut out, quarantine_path);
                }
                RollbackOp::DeleteCreatedFile { path }
                | RollbackOp::DeleteCreatedFolder { path }
                | RollbackOp::DeleteShortcut { link_path: path }
                | RollbackOp::DeleteService { name: path }
                | RollbackOp::StopService { name: path } => {
                    write_string(&mut out, path);
                }
                RollbackOp::RestoreRegistry {
                    root,
                    key,
                    name,
                    previous_value,
                    existed,
                } => {
                    out.extend_from_slice(&root.to_le_bytes());
                    write_string(&mut out, key);
                    write_opt_string(&mut out, name.as_deref());
                    write_opt_string(&mut out, previous_value.as_deref());
                    out.push(u8::from(*existed));
                }
                RollbackOp::RollbackCustomAction {
                    action,
                    action_type,
                    source,
                    target,
                } => {
                    write_string(&mut out, action);
                    out.extend_from_slice(&action_type.to_le_bytes());
                    write_string(&mut out, source);
                    write_string(&mut out, target);
                }
            }
        }

        out
    }

    /// Deserializes a [`RollbackScript`] from binary `.rbs` data.
    ///
    /// # Arguments
    ///
    /// * `data` - Raw `.rbs` byte slice.
    ///
    /// # Returns
    ///
    /// A parsed [`RollbackScript`].
    ///
    /// # Errors
    ///
    /// Returns [`Error::ScriptError`] if the magic header is invalid or data is truncated.
    pub fn deserialize(data: &[u8]) -> Result<Self> {
        if data.len() < 12 {
            return Err(Error::ScriptError {
                opcode: "Header".to_string(),
                reason: "Data too short for RBS header".to_string(),
            });
        }
        if &data[0..8] != RBS_MAGIC {
            return Err(Error::ScriptError {
                opcode: "Header".to_string(),
                reason: "Invalid RBS magic header".to_string(),
            });
        }

        let mut offset = 8;
        let count = read_u32(data, &mut offset)?;
        let mut operations = Vec::with_capacity(count as usize);

        for _ in 0..count {
            if offset >= data.len() {
                return Err(Error::ScriptError {
                    opcode: "Opcode".to_string(),
                    reason: "Unexpected EOF reading rollback opcode ID".to_string(),
                });
            }
            let opcode_id = data[offset];
            offset += 1;

            let op = match opcode_id {
                1 => {
                    let target_path = read_string(data, &mut offset)?;
                    let quarantine_path = read_string(data, &mut offset)?;
                    RollbackOp::RestoreQuarantinedFile {
                        target_path,
                        quarantine_path,
                    }
                }
                2 => RollbackOp::DeleteCreatedFile {
                    path: read_string(data, &mut offset)?,
                },
                3 => RollbackOp::DeleteCreatedFolder {
                    path: read_string(data, &mut offset)?,
                },
                4 => {
                    let root = read_u32(data, &mut offset)?;
                    let key = read_string(data, &mut offset)?;
                    let name = read_opt_string(data, &mut offset)?;
                    let previous_value = read_opt_string(data, &mut offset)?;
                    if offset >= data.len() {
                        return Err(Error::ScriptError {
                            opcode: "RestoreRegistry".to_string(),
                            reason: "Unexpected EOF reading existed flag".to_string(),
                        });
                    }
                    let existed = data[offset] != 0;
                    offset += 1;
                    RollbackOp::RestoreRegistry {
                        root,
                        key,
                        name,
                        previous_value,
                        existed,
                    }
                }
                5 => RollbackOp::DeleteShortcut {
                    link_path: read_string(data, &mut offset)?,
                },
                6 => RollbackOp::DeleteService {
                    name: read_string(data, &mut offset)?,
                },
                7 => RollbackOp::StopService {
                    name: read_string(data, &mut offset)?,
                },
                8 => {
                    let action = read_string(data, &mut offset)?;
                    let action_type = read_u32(data, &mut offset)?;
                    let source = read_string(data, &mut offset)?;
                    let target = read_string(data, &mut offset)?;
                    RollbackOp::RollbackCustomAction {
                        action,
                        action_type,
                        source,
                        target,
                    }
                }
                other => {
                    return Err(Error::ScriptError {
                        opcode: format!("RollbackOpcode({other})"),
                        reason: "Unknown rollback opcode ID".to_string(),
                    });
                }
            };

            operations.push(op);
        }

        Ok(Self { operations })
    }
}

impl fmt::Display for ScriptOp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::CreateFolder { path } => write!(f, "CreateFolder({path})"),
            Self::RemoveFolder { path } => write!(f, "RemoveFolder({path})"),
            Self::CopyFile {
                source,
                destination,
                ..
            } => write!(f, "CopyFile({source} -> {destination})"),
            Self::WriteFile { destination, .. } => write!(f, "WriteFile({destination})"),
            Self::DeleteFile { path } => write!(f, "DeleteFile({path})"),
            Self::BackupFile {
                target_path,
                quarantine_path,
            } => write!(f, "BackupFile({target_path} -> {quarantine_path})"),
            Self::WriteRegistry {
                root, key, name, ..
            } => {
                write!(f, "WriteRegistry(root={root}, key={key}, name={name:?})")
            }
            Self::DeleteRegistry { root, key, name } => {
                write!(f, "DeleteRegistry(root={root}, key={key}, name={name:?})")
            }
            Self::CreateShortcut {
                target, link_path, ..
            } => write!(f, "CreateShortcut({target} -> {link_path})"),
            Self::DeleteShortcut { link_path } => write!(f, "DeleteShortcut({link_path})"),
            Self::InstallService { name, .. } => write!(f, "InstallService({name})"),
            Self::DeleteService { name } => write!(f, "DeleteService({name})"),
            Self::StartService { name, .. } => write!(f, "StartService({name})"),
            Self::StopService { name } => write!(f, "StopService({name})"),
            Self::CustomAction { action, .. } => write!(f, "CustomAction({action})"),
        }
    }
}

impl fmt::Display for RollbackOp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::RestoreQuarantinedFile {
                target_path,
                quarantine_path,
            } => write!(
                f,
                "RestoreQuarantinedFile({quarantine_path} -> {target_path})"
            ),
            Self::DeleteCreatedFile { path } => write!(f, "DeleteCreatedFile({path})"),
            Self::DeleteCreatedFolder { path } => write!(f, "DeleteCreatedFolder({path})"),
            Self::RestoreRegistry {
                root, key, name, ..
            } => {
                write!(f, "RestoreRegistry(root={root}, key={key}, name={name:?})")
            }
            Self::DeleteShortcut { link_path } => write!(f, "DeleteShortcut({link_path})"),
            Self::DeleteService { name } => write!(f, "DeleteService({name})"),
            Self::StopService { name } => write!(f, "StopService({name})"),
            Self::RollbackCustomAction { action, .. } => {
                write!(f, "RollbackCustomAction({action})")
            }
        }
    }
}

/// Helper function to write a 4-byte length prefixed UTF-8 string.
fn write_string(out: &mut Vec<u8>, s: &str) {
    let bytes = s.as_bytes();
    let len = u32::try_from(bytes.len()).unwrap_or(u32::MAX);
    out.extend_from_slice(&len.to_le_bytes());
    out.extend_from_slice(bytes);
}

/// Helper function to write an optional UTF-8 string with presence byte.
fn write_opt_string(out: &mut Vec<u8>, s: Option<&str>) {
    if let Some(val) = s {
        out.push(1);
        write_string(out, val);
    } else {
        out.push(0);
    }
}

/// Helper function to write a 4-byte length prefixed byte slice.
fn write_bytes(out: &mut Vec<u8>, bytes: &[u8]) {
    let len = u32::try_from(bytes.len()).unwrap_or(u32::MAX);
    out.extend_from_slice(&len.to_le_bytes());
    out.extend_from_slice(bytes);
}

/// Helper function to read a `u32` integer from byte slice.
fn read_u32(data: &[u8], offset: &mut usize) -> Result<u32> {
    if *offset + 4 > data.len() {
        return Err(Error::ScriptError {
            opcode: "ReadU32".to_string(),
            reason: "Unexpected EOF reading u32".to_string(),
        });
    }
    let mut bytes = [0u8; 4];
    bytes.copy_from_slice(&data[*offset..*offset + 4]);
    *offset += 4;
    Ok(u32::from_le_bytes(bytes))
}

/// Helper function to read an `i32` integer from byte slice.
fn read_i32(data: &[u8], offset: &mut usize) -> Result<i32> {
    if *offset + 4 > data.len() {
        return Err(Error::ScriptError {
            opcode: "ReadI32".to_string(),
            reason: "Unexpected EOF reading i32".to_string(),
        });
    }
    let mut bytes = [0u8; 4];
    bytes.copy_from_slice(&data[*offset..*offset + 4]);
    *offset += 4;
    Ok(i32::from_le_bytes(bytes))
}

/// Helper function to read a 4-byte length prefixed UTF-8 string.
fn read_string(data: &[u8], offset: &mut usize) -> Result<String> {
    let len = read_u32(data, offset)? as usize;
    if *offset + len > data.len() {
        return Err(Error::ScriptError {
            opcode: "ReadString".to_string(),
            reason: "Unexpected EOF reading string content".to_string(),
        });
    }
    let s =
        std::str::from_utf8(&data[*offset..*offset + len]).map_err(|err| Error::ScriptError {
            opcode: "ReadString".to_string(),
            reason: format!("Invalid UTF-8 in string: {err}"),
        })?;
    *offset += len;
    Ok(s.to_string())
}

/// Helper function to read an optional string with presence flag byte.
fn read_opt_string(data: &[u8], offset: &mut usize) -> Result<Option<String>> {
    if *offset >= data.len() {
        return Err(Error::ScriptError {
            opcode: "ReadOptString".to_string(),
            reason: "Unexpected EOF reading optional string flag".to_string(),
        });
    }
    let has_value = data[*offset] != 0;
    *offset += 1;
    if has_value {
        read_string(data, offset).map(Some)
    } else {
        Ok(None)
    }
}

/// Helper function to read a 4-byte length prefixed byte slice.
fn read_bytes(data: &[u8], offset: &mut usize) -> Result<Vec<u8>> {
    let len = read_u32(data, offset)? as usize;
    if *offset + len > data.len() {
        return Err(Error::ScriptError {
            opcode: "ReadBytes".to_string(),
            reason: "Unexpected EOF reading byte buffer".to_string(),
        });
    }
    let bytes = data[*offset..*offset + len].to_vec();
    *offset += len;
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Tests [`InstallScript`] creation, emptiness, getters, and default constructors.
    #[test]
    fn test_install_script_basics() {
        let mut script = InstallScript::new();
        assert!(script.is_empty());
        assert_eq!(script.len(), 0);

        script.push(ScriptOp::CreateFolder {
            path: "/opt/myapp".to_string(),
        });
        assert!(!script.is_empty());
        assert_eq!(script.len(), 1);
        assert_eq!(
            format!("{}", script.operations()[0]),
            "CreateFolder(/opt/myapp)"
        );

        let default_script = InstallScript::default();
        assert!(default_script.is_empty());

        let default_rbs = RollbackScript::default();
        assert!(default_rbs.is_empty());
    }

    /// Tests serialization and deserialization of all [`ScriptOp`] variants.
    #[test]
    fn test_install_script_roundtrip_all_ops() {
        let mut script = InstallScript::new();
        script.push(ScriptOp::CreateFolder {
            path: r"C:\Program Files\App".to_string(),
        });
        script.push(ScriptOp::RemoveFolder {
            path: r"C:\Program Files\App\Old".to_string(),
        });
        script.push(ScriptOp::CopyFile {
            source: r"SourceDir\app.exe".to_string(),
            destination: r"C:\Program Files\App\app.exe".to_string(),
            overwrite: true,
        });
        script.push(ScriptOp::WriteFile {
            destination: r"C:\Program Files\App\config.json".to_string(),
            content: b"{\"test\":true}".to_vec(),
        });
        script.push(ScriptOp::DeleteFile {
            path: r"C:\Program Files\App\obsolete.dll".to_string(),
        });
        script.push(ScriptOp::BackupFile {
            target_path: r"C:\Program Files\App\app.exe".to_string(),
            quarantine_path: r"C:\Config.Msi\1234.rbf".to_string(),
        });
        script.push(ScriptOp::WriteRegistry {
            root: 2,
            key: r"Software\MyApp".to_string(),
            name: Some("Version".to_string()),
            value: Some("1.0.0".to_string()),
        });
        script.push(ScriptOp::DeleteRegistry {
            root: 1,
            key: r"Software\MyApp\Old".to_string(),
            name: None,
        });
        script.push(ScriptOp::CreateShortcut {
            target: r"C:\Program Files\App\app.exe".to_string(),
            link_path: r"C:\Users\Public\Desktop\App.lnk".to_string(),
            arguments: Some("--verbose".to_string()),
            icon_path: Some(r"C:\Program Files\App\app.ico".to_string()),
            icon_index: Some(0),
        });
        script.push(ScriptOp::CreateShortcut {
            target: r"C:\Program Files\App\app.exe".to_string(),
            link_path: r"C:\Users\Public\Desktop\App2.lnk".to_string(),
            arguments: None,
            icon_path: None,
            icon_index: None,
        });
        script.push(ScriptOp::DeleteShortcut {
            link_path: r"C:\Users\Public\Desktop\OldApp.lnk".to_string(),
        });
        script.push(ScriptOp::InstallService {
            name: "AppSvc".to_string(),
            display_name: "Application Service".to_string(),
            service_type: 16,
            start_type: 2,
            binary_path: r"C:\Program Files\App\service.exe".to_string(),
        });
        script.push(ScriptOp::DeleteService {
            name: "OldSvc".to_string(),
        });
        script.push(ScriptOp::StartService {
            name: "AppSvc".to_string(),
            arguments: Some("-run".to_string()),
        });
        script.push(ScriptOp::StopService {
            name: "AppSvc".to_string(),
        });
        script.push(ScriptOp::CustomAction {
            action: "CA_Configure".to_string(),
            action_type: 1,
            source: "BinaryTableKey".to_string(),
            target: "EntryFn".to_string(),
        });

        let serialized = script.serialize();
        assert_eq!(InstallScript::deserialize(&serialized), Ok(script.clone()));

        // Test display formatting on all variants
        for op in script.operations() {
            assert_ne!(format!("{op}"), "");
        }
    }

    /// Tests [`RollbackScript`] basics, roundtrip, and display formatting.
    #[test]
    fn test_rollback_script_roundtrip_all_ops() {
        let mut rbs = RollbackScript::new();
        assert!(rbs.is_empty());
        assert_eq!(rbs.len(), 0);

        rbs.push(RollbackOp::RestoreQuarantinedFile {
            target_path: r"C:\Program Files\App\app.exe".to_string(),
            quarantine_path: r"C:\Config.Msi\1234.rbf".to_string(),
        });
        rbs.push(RollbackOp::DeleteCreatedFile {
            path: r"C:\Program Files\App\new_file.txt".to_string(),
        });
        rbs.push(RollbackOp::DeleteCreatedFolder {
            path: r"C:\Program Files\App".to_string(),
        });
        rbs.push(RollbackOp::RestoreRegistry {
            root: 2,
            key: r"Software\MyApp".to_string(),
            name: Some("Version".to_string()),
            previous_value: Some("0.9.0".to_string()),
            existed: true,
        });
        rbs.push(RollbackOp::RestoreRegistry {
            root: 1,
            key: r"Software\MyApp\Old".to_string(),
            name: None,
            previous_value: None,
            existed: false,
        });
        rbs.push(RollbackOp::DeleteShortcut {
            link_path: r"C:\Users\Public\Desktop\App.lnk".to_string(),
        });
        rbs.push(RollbackOp::DeleteService {
            name: "AppSvc".to_string(),
        });
        rbs.push(RollbackOp::StopService {
            name: "AppSvc".to_string(),
        });
        rbs.push(RollbackOp::RollbackCustomAction {
            action: "CA_Rollback".to_string(),
            action_type: 0x501,
            source: "BinaryTableKey".to_string(),
            target: "RollbackFn".to_string(),
        });

        assert!(!rbs.is_empty());
        assert_eq!(rbs.len(), 9);

        let serialized = rbs.serialize();
        assert_eq!(RollbackScript::deserialize(&serialized), Ok(rbs.clone()));

        for op in rbs.operations() {
            assert_ne!(format!("{op}"), "");
        }
    }

    /// Tests error branches on deserializing malformed scripts.
    #[test]
    fn test_script_deserialize_errors() {
        // Too short for header
        assert!(InstallScript::deserialize(&[1, 2, 3]).is_err());
        assert!(RollbackScript::deserialize(&[1, 2, 3]).is_err());

        // Bad magic
        assert!(InstallScript::deserialize(b"BADMAGIC\x00\x00\x00\x00").is_err());
        assert!(RollbackScript::deserialize(b"BADMAGIC\x00\x00\x00\x00").is_err());

        // Count mismatch / truncated opcode
        {
            let mut bad_header = Vec::from(IBS_MAGIC);
            bad_header.extend_from_slice(&1u32.to_le_bytes());
            assert!(InstallScript::deserialize(&bad_header).is_err());
        }
        {
            let mut bad_header = Vec::from(RBS_MAGIC);
            bad_header.extend_from_slice(&1u32.to_le_bytes());
            assert!(RollbackScript::deserialize(&bad_header).is_err());
        }

        // Unknown opcode
        {
            let mut unk_header = Vec::from(IBS_MAGIC);
            unk_header.extend_from_slice(&1u32.to_le_bytes());
            unk_header.push(255);
            assert!(InstallScript::deserialize(&unk_header).is_err());
        }
        {
            let mut unk_header = Vec::from(RBS_MAGIC);
            unk_header.extend_from_slice(&1u32.to_le_bytes());
            unk_header.push(255);
            assert!(RollbackScript::deserialize(&unk_header).is_err());
        }

        // Truncated string in CreateFolder
        let mut trunc_str = Vec::from(IBS_MAGIC);
        trunc_str.extend_from_slice(&1u32.to_le_bytes());
        trunc_str.push(1); // CreateFolder
        trunc_str.extend_from_slice(&10u32.to_le_bytes()); // String length 10
        trunc_str.extend_from_slice(b"abc"); // Only 3 bytes provided
        assert!(InstallScript::deserialize(&trunc_str).is_err());

        // CopyFile: Unexpected EOF reading overwrite flag
        let mut trunc_copy = Vec::from(IBS_MAGIC);
        trunc_copy.extend_from_slice(&1u32.to_le_bytes());
        trunc_copy.push(3); // CopyFile
        write_string(&mut trunc_copy, "src");
        write_string(&mut trunc_copy, "dst");
        // missing overwrite flag byte
        assert!(InstallScript::deserialize(&trunc_copy).is_err());

        // CreateShortcut: Unexpected EOF reading icon index flag
        let mut trunc_sc = Vec::from(IBS_MAGIC);
        trunc_sc.extend_from_slice(&1u32.to_le_bytes());
        trunc_sc.push(9); // CreateShortcut
        write_string(&mut trunc_sc, "target");
        write_string(&mut trunc_sc, "link");
        write_opt_string(&mut trunc_sc, None);
        write_opt_string(&mut trunc_sc, None);
        // missing has_icon flag byte
        assert!(InstallScript::deserialize(&trunc_sc).is_err());

        // CreateShortcut: Unexpected EOF reading i32 icon_index
        let mut trunc_sc_icon = Vec::from(IBS_MAGIC);
        trunc_sc_icon.extend_from_slice(&1u32.to_le_bytes());
        trunc_sc_icon.push(9); // CreateShortcut
        write_string(&mut trunc_sc_icon, "target");
        write_string(&mut trunc_sc_icon, "link");
        write_opt_string(&mut trunc_sc_icon, None);
        write_opt_string(&mut trunc_sc_icon, None);
        trunc_sc_icon.push(1); // has_icon = true
        trunc_sc_icon.extend_from_slice(&[0u8, 1]); // only 2 bytes instead of 4 for i32
        assert!(InstallScript::deserialize(&trunc_sc_icon).is_err());

        // RestoreRegistry: Unexpected EOF reading existed flag
        let mut trunc_rr = Vec::from(RBS_MAGIC);
        trunc_rr.extend_from_slice(&1u32.to_le_bytes());
        trunc_rr.push(4); // RestoreRegistry
        trunc_rr.extend_from_slice(&1u32.to_le_bytes());
        write_string(&mut trunc_rr, "key");
        write_opt_string(&mut trunc_rr, None);
        write_opt_string(&mut trunc_rr, None);
        // missing existed flag byte
        assert!(RollbackScript::deserialize(&trunc_rr).is_err());

        // WriteRegistry: Unexpected EOF reading u32 root
        let mut trunc_u32 = Vec::from(IBS_MAGIC);
        trunc_u32.extend_from_slice(&1u32.to_le_bytes());
        trunc_u32.push(7); // WriteRegistry
        trunc_u32.extend_from_slice(&[1u8, 2]); // only 2 bytes of u32
        assert!(InstallScript::deserialize(&trunc_u32).is_err());

        // WriteRegistry: Unexpected EOF reading optional string flag
        let mut trunc_opt = Vec::from(IBS_MAGIC);
        trunc_opt.extend_from_slice(&1u32.to_le_bytes());
        trunc_opt.push(7); // WriteRegistry
        trunc_opt.extend_from_slice(&1u32.to_le_bytes());
        write_string(&mut trunc_opt, "key");
        // missing optional string flag byte for name
        assert!(InstallScript::deserialize(&trunc_opt).is_err());

        // WriteFile: Unexpected EOF reading byte buffer
        let mut trunc_bytes = Vec::from(IBS_MAGIC);
        trunc_bytes.extend_from_slice(&1u32.to_le_bytes());
        trunc_bytes.push(4); // WriteFile
        write_string(&mut trunc_bytes, "dst");
        trunc_bytes.extend_from_slice(&100u32.to_le_bytes()); // length 100
        trunc_bytes.extend_from_slice(&[1, 2, 3]); // only 3 bytes
        assert!(InstallScript::deserialize(&trunc_bytes).is_err());

        // Invalid UTF-8 in string
        let mut bad_utf8 = Vec::from(IBS_MAGIC);
        bad_utf8.extend_from_slice(&1u32.to_le_bytes());
        bad_utf8.push(1); // CreateFolder
        bad_utf8.extend_from_slice(&2u32.to_le_bytes());
        bad_utf8.extend_from_slice(&[0xFF, 0xFE]); // invalid UTF-8
        assert!(InstallScript::deserialize(&bad_utf8).is_err());
    }
}

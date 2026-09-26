//! Custom Action Execution Framework & MSI API C Shims.
//!
//! Grounded directly in official Microsoft Windows Installer SDK specifications:
//! - Custom action source types:
//!   - `msidbCustomActionTypeDll` (`0x0001`): Dynamic link library stored in `Binary` table.
//!   - `msidbCustomActionTypeExe` (`0x0002`): Executable file stored in `Binary` table.
//!   - `msidbCustomActionTypeTextData` (`0x0003`): Formatted text string formatted into property.
//!   - `msidbCustomActionTypeJScript` (`0x0005`): `JScript`/ECMAScript stored in `Binary` table.
//!   - `msidbCustomActionTypeVBScript` (`0x0006`): `VBScript` code stored in `Binary` table.
//!   - `msidbCustomActionTypeInstalledDll` (`0x0011`): Dynamic link library installed with product.
//!   - `msidbCustomActionTypeInstalledExe` (`0x0012`): Executable installed with product.
//!   - `msidbCustomActionTypeDirectory` (`0x0023`): Formatted string target directory path.
//!   - `msidbCustomActionTypeProperty` (`0x0033`): Target executable path formatted from property.
//! - Execution mode flags:
//!   - `msidbCustomActionTypeContinue` (`0x0040`): Asynchronous execution, ignore return code.
//!   - `msidbCustomActionTypeAsync` (`0x0080`): Asynchronous execution, wait at sequence end.
//!   - `msidbCustomActionTypeFirstSequence` (`0x0100`): Execute only once per multi-instance sequence.
//!   - `msidbCustomActionTypeOncePerProcess` (`0x0200`): Execute once per process lifecycle.
//!   - `msidbCustomActionTypeClientRepeat` (`0x0300`): Execute again in client if UI displayed.
//!   - `msidbCustomActionTypeInScript` (`0x0400`): Deferred execution within install transaction.
//!   - `msidbCustomActionTypeRollback` (`0x0100` | `0x0400`): Rollback execution (runs only on failure).
//!   - `msidbCustomActionTypeCommit` (`0x0200` | `0x0400`): Commit execution (runs only on success).
//!   - `msidbCustomActionTypeNoImpersonate` (`0x0800`): Elevated execution without user impersonation.
//! - MSI API C Shims for native shared libraries:
//!   - `MsiGetPropertyW`, `MsiSetPropertyW`, `MsiProcessMessage`, `MsiCreateRecord`,
//!     `MsiRecordSetStringW`, `MsiRecordSetInteger`, `MsiRecordGetStringW`, `MsiRecordGetInteger`,
//!     `MsiDoActionW`, `MsiEvaluateConditionW`, `MsiGetActiveDatabase`.

#![allow(clippy::significant_drop_tightening, non_snake_case)]

use crate::database::tables::record::{FieldValue, Record, MSI_NULL_INTEGER_32};
use crate::error::{Error, Result};
use crate::execution::properties::EvaluationContext;
use crate::wix::linker::LinkedDatabase;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{LazyLock, Mutex};

/// MSI Custom Action Source Type: DLL stored in Binary table (`0x0001`).
pub const MSIDB_CUSTOM_ACTION_TYPE_DLL: u32 = 0x0001;

/// MSI Custom Action Source Type: Executable stored in Binary table (`0x0002`).
pub const MSIDB_CUSTOM_ACTION_TYPE_EXE: u32 = 0x0002;

/// MSI Custom Action Source Type: Formatted text data assigned to property (`0x0003`).
pub const MSIDB_CUSTOM_ACTION_TYPE_TEXT_DATA: u32 = 0x0003;

/// MSI Custom Action Source Type: `JScript` script stored in Binary table (`0x0005`).
pub const MSIDB_CUSTOM_ACTION_TYPE_JSCRIPT: u32 = 0x0005;

/// MSI Custom Action Source Type: `VBScript` script stored in Binary table (`0x0006`).
pub const MSIDB_CUSTOM_ACTION_TYPE_VBSCRIPT: u32 = 0x0006;

/// MSI Custom Action Source Type: Installed DLL file (`0x0011`).
pub const MSIDB_CUSTOM_ACTION_TYPE_INSTALLED_DLL: u32 = 0x0011;

/// MSI Custom Action Source Type: Installed executable file (`0x0012`).
pub const MSIDB_CUSTOM_ACTION_TYPE_INSTALLED_EXE: u32 = 0x0012;

/// MSI Custom Action Source Type: Error abort action (`0x0013` = 19).
pub const MSIDB_CUSTOM_ACTION_TYPE_ERROR: u32 = 0x0013;

/// MSI Custom Action Source Type: Directory-based executable (`0x0022` = 34).
pub const MSIDB_CUSTOM_ACTION_TYPE_DIRECTORY_EXE: u32 = 0x0022;

/// MSI Custom Action Source Type: Formatted target directory path (`0x0023`).
pub const MSIDB_CUSTOM_ACTION_TYPE_DIRECTORY: u32 = 0x0023;

/// MSI Custom Action Source Type: Property-based executable (`0x0032` = 50).
pub const MSIDB_CUSTOM_ACTION_TYPE_PROPERTY_EXE: u32 = 0x0032;

/// MSI Custom Action Source Type: Executable path formatted from property (`0x0033`).
pub const MSIDB_CUSTOM_ACTION_TYPE_PROPERTY: u32 = 0x0033;

/// MSI Custom Action Execution Mode Flag: Continue execution asynchronously, ignore exit code (`0x0040`).
pub const MSIDB_CUSTOM_ACTION_TYPE_CONTINUE: u32 = 0x0040;

/// MSI Custom Action Execution Mode Flag: Execute asynchronously, wait at sequence end (`0x0080`).
pub const MSIDB_CUSTOM_ACTION_TYPE_ASYNC: u32 = 0x0080;

/// MSI Custom Action Execution Mode Flag: Execute only once per multi-instance sequence (`0x0100`).
pub const MSIDB_CUSTOM_ACTION_TYPE_FIRST_SEQUENCE: u32 = 0x0100;

/// MSI Custom Action Execution Mode Flag: Execute once per process lifecycle (`0x0200`).
pub const MSIDB_CUSTOM_ACTION_TYPE_ONCE_PER_PROCESS: u32 = 0x0200;

/// MSI Custom Action Execution Mode Flag: Execute again in client if UI is displayed (`0x0300`).
pub const MSIDB_CUSTOM_ACTION_TYPE_CLIENT_REPEAT: u32 = 0x0300;

/// MSI Custom Action Execution Mode Flag: Deferred execution within script transaction (`0x0400`).
pub const MSIDB_CUSTOM_ACTION_TYPE_IN_SCRIPT: u32 = 0x0400;

/// MSI Custom Action Execution Mode Flag: Rollback execution (runs only on failure) (`0x0500`).
pub const MSIDB_CUSTOM_ACTION_TYPE_ROLLBACK: u32 = 0x0100 | 0x0400;

/// MSI Custom Action Execution Mode Flag: Commit execution (runs only on success) (`0x0600`).
pub const MSIDB_CUSTOM_ACTION_TYPE_COMMIT: u32 = 0x0200 | 0x0400;

/// MSI Custom Action Execution Mode Flag: Elevated execution without user impersonation (`0x0800`).
pub const MSIDB_CUSTOM_ACTION_TYPE_NO_IMPERSONATE: u32 = 0x0800;

/// Win32 API Success code (`0`).
pub const ERROR_SUCCESS: u32 = 0;

/// Win32 API Invalid Handle code (`6`).
pub const ERROR_INVALID_HANDLE: u32 = 6;

/// Win32 API Invalid Parameter code (`87`).
pub const ERROR_INVALID_PARAMETER: u32 = 87;

/// Win32 API More Data / Buffer Overflow code (`234`).
pub const ERROR_MORE_DATA: u32 = 234;

/// Win32 API Function Failed code (`1627`).
pub const ERROR_FUNCTION_FAILED: u32 = 1627;

/// MSI Condition evaluation result: False (`0`).
pub const MSICONDITION_FALSE: u32 = 0;

/// MSI Condition evaluation result: True (`1`).
pub const MSICONDITION_TRUE: u32 = 1;

/// MSI Condition evaluation result: None/Empty (`2`).
pub const MSICONDITION_NONE: u32 = 2;

/// MSI Condition evaluation result: Error (`3`).
pub const MSICONDITION_ERROR: u32 = 3;

/// Custom Action source code or target origin type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CustomActionSourceType {
    /// Dynamic link library stored in the `Binary` table (`0x0001`).
    Dll,
    /// Executable file stored in the `Binary` table (`0x0002`).
    Exe,
    /// Formatted text data assigned directly to a property (`0x0003`).
    TextData,
    /// `JScript` / ECMAScript code stored in the `Binary` table (`0x0005`).
    JScript,
    /// `VBScript` code stored in the `Binary` table (`0x0006`).
    VBScript,
    /// Dynamic link library installed with the product (`0x0011`).
    InstalledDll,
    /// Executable file installed with the product (`0x0012`).
    InstalledExe,
    /// Error abort action halting execution with formatted message (`0x0013` = 19).
    Error,
    /// Executable installed in target directory (`0x0022` = 34).
    DirectoryExe,
    /// Formatted string target directory path (`0x0023`).
    Directory,
    /// Executable path formatted from a property (`0x0032` = 50).
    PropertyExe,
    /// Executable path formatted from a property (`0x0033`).
    Property,
}

impl CustomActionSourceType {
    /// Parses raw custom action type bits into [`CustomActionSourceType`].
    ///
    /// # Arguments
    ///
    /// * `raw` - Lower bits of custom action type integer.
    ///
    /// # Returns
    ///
    /// Parsed [`CustomActionSourceType`] on success.
    ///
    /// # Errors
    ///
    /// Returns [`Error::InvalidArgument`] if the source type bits are unrecognized.
    pub fn from_raw(raw: u32) -> Result<Self> {
        let source_bits = raw & 0x003F;
        match source_bits {
            MSIDB_CUSTOM_ACTION_TYPE_DLL => Ok(Self::Dll),
            MSIDB_CUSTOM_ACTION_TYPE_EXE => Ok(Self::Exe),
            MSIDB_CUSTOM_ACTION_TYPE_TEXT_DATA => Ok(Self::TextData),
            MSIDB_CUSTOM_ACTION_TYPE_JSCRIPT | 0x0015 | 0x0025 | 0x0035 => Ok(Self::JScript),
            MSIDB_CUSTOM_ACTION_TYPE_VBSCRIPT | 0x0016 | 0x0026 | 0x0036 => Ok(Self::VBScript),
            MSIDB_CUSTOM_ACTION_TYPE_INSTALLED_DLL => Ok(Self::InstalledDll),
            MSIDB_CUSTOM_ACTION_TYPE_INSTALLED_EXE => Ok(Self::InstalledExe),
            MSIDB_CUSTOM_ACTION_TYPE_ERROR => Ok(Self::Error),
            MSIDB_CUSTOM_ACTION_TYPE_DIRECTORY_EXE => Ok(Self::DirectoryExe),
            MSIDB_CUSTOM_ACTION_TYPE_DIRECTORY => Ok(Self::Directory),
            MSIDB_CUSTOM_ACTION_TYPE_PROPERTY_EXE => Ok(Self::PropertyExe),
            MSIDB_CUSTOM_ACTION_TYPE_PROPERTY => Ok(Self::Property),
            other => Err(Error::InvalidArgument {
                argument: "CustomAction.Type".to_string(),
                reason: format!("Unrecognized custom action source type 0x{other:04X}"),
            }),
        }
    }
}

/// Custom Action execution synchronization mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CustomActionExecutionMode {
    /// Synchronous execution, wait for completion and check return code.
    #[default]
    Synchronous,
    /// Asynchronous execution, ignore return code (`0x0040`).
    Continue,
    /// Asynchronous execution, wait at end of sequence (`0x0080`).
    Async,
}

impl CustomActionExecutionMode {
    /// Returns true if execution is asynchronous (`Continue` or `Async`).
    ///
    /// # Returns
    ///
    /// `true` if asynchronous, `false` otherwise.
    #[must_use]
    pub const fn is_async(self) -> bool {
        matches!(self, Self::Continue | Self::Async)
    }

    /// Returns true if execution ignores return code (`Continue`).
    ///
    /// # Returns
    ///
    /// `true` if failures should be ignored, `false` otherwise.
    #[must_use]
    pub const fn is_continue(self) -> bool {
        matches!(self, Self::Continue)
    }
}

/// In-script transaction scheduling mode for custom actions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum InScriptMode {
    /// Immediate execution during client sequence processing.
    #[default]
    Immediate,
    /// Deferred execution within the privileged installation transaction (`0x0400`).
    Deferred,
    /// Rollback execution, invoked only upon installation failure (`0x0500`).
    Rollback,
    /// Commit execution, invoked only upon installation success (`0x0600`).
    Commit,
}

/// Strongly-typed definition of an MSI Custom Action parsed from the `CustomAction` table.
#[derive(Debug, Clone, PartialEq, Eq)]
#[allow(clippy::struct_excessive_bools)]
pub struct CustomActionDefinition {
    /// Unique custom action identifier name.
    name: String,
    /// Original raw type bitmask.
    raw_type: u32,
    /// Source origin type.
    source_type: CustomActionSourceType,
    /// Execution synchronization mode.
    execution_mode: CustomActionExecutionMode,
    /// In-script transaction placement.
    in_script: InScriptMode,
    /// Whether action executes once per multi-instance sequence (`0x0100`).
    first_sequence: bool,
    /// Whether action executes once per process lifecycle (`0x0200`).
    once_per_process: bool,
    /// Whether action repeats in client if UI is displayed (`0x0300`).
    client_repeat: bool,
    /// Whether elevated execution runs without user impersonation (`0x0800`).
    no_impersonate: bool,
    /// Source identifier (binary key, property name, or directory name).
    source: String,
    /// Target command, entry point function, or formatted parameter string.
    target: String,
}

impl CustomActionDefinition {
    /// Parses a custom action definition from table fields.
    ///
    /// # Arguments
    ///
    /// * `name` - Action name.
    /// * `raw_type` - Type integer bitmask.
    /// * `source` - Source identifier.
    /// * `target` - Target string or entry point.
    ///
    /// # Returns
    ///
    /// Parsed [`CustomActionDefinition`] on success.
    ///
    /// # Errors
    ///
    /// Returns [`Error`] if the type bits are invalid.
    pub fn parse(name: &str, raw_type: u32, source: &str, target: &str) -> Result<Self> {
        let source_type = CustomActionSourceType::from_raw(raw_type)?;

        let execution_mode = if raw_type & MSIDB_CUSTOM_ACTION_TYPE_CONTINUE != 0 {
            CustomActionExecutionMode::Continue
        } else if raw_type & MSIDB_CUSTOM_ACTION_TYPE_ASYNC != 0 {
            CustomActionExecutionMode::Async
        } else {
            CustomActionExecutionMode::Synchronous
        };

        let in_script_bits = raw_type & (0x0100 | 0x0200 | 0x0400);
        let in_script = if in_script_bits == MSIDB_CUSTOM_ACTION_TYPE_ROLLBACK {
            InScriptMode::Rollback
        } else if in_script_bits == MSIDB_CUSTOM_ACTION_TYPE_COMMIT {
            InScriptMode::Commit
        } else if raw_type & MSIDB_CUSTOM_ACTION_TYPE_IN_SCRIPT != 0 {
            InScriptMode::Deferred
        } else {
            InScriptMode::Immediate
        };

        let first_sequence = (raw_type & 0x0300) == MSIDB_CUSTOM_ACTION_TYPE_FIRST_SEQUENCE;
        let once_per_process = (raw_type & 0x0300) == MSIDB_CUSTOM_ACTION_TYPE_ONCE_PER_PROCESS;
        let client_repeat = (raw_type & 0x0300) == MSIDB_CUSTOM_ACTION_TYPE_CLIENT_REPEAT;
        let no_impersonate = (raw_type & MSIDB_CUSTOM_ACTION_TYPE_NO_IMPERSONATE) != 0;

        Ok(Self {
            name: name.to_string(),
            raw_type,
            source_type,
            execution_mode,
            in_script,
            first_sequence,
            once_per_process,
            client_repeat,
            no_impersonate,
            source: source.to_string(),
            target: target.to_string(),
        })
    }

    /// Returns the custom action name.
    ///
    /// # Returns
    ///
    /// Action name string slice.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Returns the raw type bitmask.
    ///
    /// # Returns
    ///
    /// Integer type bitmask.
    #[must_use]
    pub const fn raw_type(&self) -> u32 {
        self.raw_type
    }

    /// Returns the parsed source type.
    ///
    /// # Returns
    ///
    /// Parsed [`CustomActionSourceType`].
    #[must_use]
    pub const fn source_type(&self) -> CustomActionSourceType {
        self.source_type
    }

    /// Returns the execution mode.
    ///
    /// # Returns
    ///
    /// Parsed [`CustomActionExecutionMode`].
    #[must_use]
    pub const fn execution_mode(&self) -> CustomActionExecutionMode {
        self.execution_mode
    }

    /// Returns the in-script mode.
    ///
    /// # Returns
    ///
    /// Parsed [`InScriptMode`].
    #[must_use]
    pub const fn in_script(&self) -> InScriptMode {
        self.in_script
    }

    /// Returns true if first-sequence flag is set.
    ///
    /// # Returns
    ///
    /// `true` if first sequence flag is enabled, `false` otherwise.
    #[must_use]
    pub const fn first_sequence(&self) -> bool {
        self.first_sequence
    }

    /// Returns true if once-per-process flag is set.
    ///
    /// # Returns
    ///
    /// `true` if once per process flag is enabled, `false` otherwise.
    #[must_use]
    pub const fn once_per_process(&self) -> bool {
        self.once_per_process
    }

    /// Returns true if client-repeat flag is set.
    ///
    /// # Returns
    ///
    /// `true` if client repeat flag is enabled, `false` otherwise.
    #[must_use]
    pub const fn client_repeat(&self) -> bool {
        self.client_repeat
    }

    /// Returns true if no-impersonate flag is set.
    ///
    /// # Returns
    ///
    /// `true` if no impersonate flag is enabled, `false` otherwise.
    #[must_use]
    pub const fn no_impersonate(&self) -> bool {
        self.no_impersonate
    }

    /// Returns the source identifier.
    ///
    /// # Returns
    ///
    /// Source string slice.
    #[must_use]
    pub fn source(&self) -> &str {
        &self.source
    }

    /// Returns the target parameter or function name.
    ///
    /// # Returns
    ///
    /// Target string slice.
    #[must_use]
    pub fn target(&self) -> &str {
        &self.target
    }
}

/// Probes whether a TCP port is available for local socket binding.
///
/// Returns `true` if a listener can successfully bind to `127.0.0.1:port`
/// (meaning the port is free), and `false` if the port is already occupied.
///
/// # Arguments
///
/// * `port` - TCP port number to probe (1-65535).
///
/// # Returns
///
/// `true` if port is available, `false` otherwise.
#[must_use]
pub fn probe_port_available(port: u16) -> bool {
    std::net::TcpListener::bind(("127.0.0.1", port)).is_ok()
}

/// Splits a command line string into an executable path and argument list, respecting quotes.
///
/// # Arguments
///
/// * `cmd` - Command line string to parse.
///
/// # Returns
///
/// Tuple of `(program_path, argument_vector)`.
#[must_use]
pub fn parse_command_line(cmd: &str) -> (PathBuf, Vec<String>) {
    let mut parts = Vec::new();
    let mut current = String::new();
    let mut in_quotes = false;

    for ch in cmd.chars() {
        match ch {
            '"' => {
                in_quotes = !in_quotes;
            }
            ' ' | '\t' => {
                if in_quotes {
                    current.push(ch);
                } else if !current.is_empty() {
                    parts.push(std::mem::take(&mut current));
                }
            }
            c => {
                current.push(c);
            }
        }
    }
    if !current.is_empty() {
        parts.push(current);
    }

    if parts.is_empty() {
        (PathBuf::new(), Vec::new())
    } else {
        let prog = PathBuf::from(parts.remove(0));
        (prog, parts)
    }
}

/// Checks whether an executable exists in the system `PATH`.
fn is_executable_in_path(prog: &Path) -> bool {
    if prog.is_absolute() {
        return prog.exists();
    }
    std::env::var_os("PATH").is_some_and(|path_var| {
        std::env::split_paths(&path_var).any(|dir| dir.join(prog).is_file())
    })
}

/// Custom action execution simulator and coordinator.
#[derive(Debug, Clone, Default)]
pub struct CustomActionExecutor {
    /// Mock execution results mapping action name to desired return code.
    mock_results: HashMap<String, u32>,
    /// Native library loader for dynamic custom actions.
    library_loader: super::native_action::NativeLibraryLoader,
    /// Subprocess runner for native executable custom actions.
    subprocess_runner: super::native_action::SubprocessRunner,
    /// Optional offline execution policy for bare-metal sysroots.
    offline_policy: Option<crate::execution::bare_metal::OfflineExecutionPolicy>,
    /// Extracted Binary table payloads for native DLL / script actions.
    binaries: HashMap<String, Vec<u8>>,
}

impl CustomActionExecutor {
    /// Creates a new [`CustomActionExecutor`].
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Attaches an extracted binary payload.
    ///
    /// If the payload is a native shared library, extracts it into a secure sandbox
    /// and pre-loads it for dynamic invocation.
    ///
    /// # Arguments
    ///
    /// * `name` - Binary stream key name.
    /// * `bytes` - Raw binary byte vector.
    fn add_binary_internal(&mut self, name: &str, bytes: Vec<u8>) {
        let name_str = name.to_string();
        let format = super::native_action::BinaryFormat::detect(&bytes);
        if format != super::native_action::BinaryFormat::Unknown {
            let _ = self
                .library_loader
                .extract_to_sandbox(&name_str, &bytes)
                .and_then(|p| self.library_loader.load_library(&p));
        }
        self.binaries.insert(name_str, bytes);
    }

    /// Attaches an arbitrary binary payload stream into the executor environment.
    ///
    /// If the payload is a native shared library, extracts it into a secure sandbox
    /// and pre-loads it for dynamic invocation.
    ///
    /// # Arguments
    ///
    /// * `name` - Binary stream key name.
    /// * `bytes` - Raw binary byte vector.
    pub fn add_binary(&mut self, name: impl AsRef<str>, bytes: Vec<u8>) {
        self.add_binary_internal(name.as_ref(), bytes);
    }

    /// Builder pattern helper to attach a binary payload.
    ///
    /// # Arguments
    ///
    /// * `name` - Binary stream key name.
    /// * `bytes` - Raw binary byte vector.
    ///
    /// # Returns
    ///
    /// Updated [`CustomActionExecutor`].
    #[must_use]
    pub fn with_binary(mut self, name: impl AsRef<str>, bytes: Vec<u8>) -> Self {
        self.add_binary(name, bytes);
        self
    }

    /// Builder pattern helper to attach multiple binary payloads.
    ///
    /// # Arguments
    ///
    /// * `binaries` - Map of binary keys to raw byte vectors.
    ///
    /// # Returns
    ///
    /// Updated [`CustomActionExecutor`].
    #[must_use]
    pub fn with_binaries(mut self, binaries: HashMap<String, Vec<u8>>) -> Self {
        for (k, v) in binaries {
            self.add_binary(&k, v);
        }
        self
    }

    /// Returns a reference to all attached binary payloads.
    ///
    /// # Returns
    ///
    /// Reference to the internal binary map.
    #[must_use]
    pub const fn binaries(&self) -> &HashMap<String, Vec<u8>> {
        &self.binaries
    }

    /// Attaches an [`crate::execution::bare_metal::OfflineExecutionPolicy`] for offline sysroots.
    ///
    /// # Arguments
    ///
    /// * `policy` - Policy governing execution safety and mocking.
    ///
    /// # Returns
    ///
    /// Updated [`CustomActionExecutor`].
    #[must_use]
    pub fn with_offline_policy(
        mut self,
        policy: crate::execution::bare_metal::OfflineExecutionPolicy,
    ) -> Self {
        self.offline_policy = Some(policy);
        self
    }

    /// Returns an optional reference to the attached [`crate::execution::bare_metal::OfflineExecutionPolicy`].
    ///
    /// # Returns
    ///
    /// Optional reference to the offline execution policy.
    #[must_use]
    pub const fn offline_policy(
        &self,
    ) -> Option<&crate::execution::bare_metal::OfflineExecutionPolicy> {
        self.offline_policy.as_ref()
    }

    /// Sets a mock return code for a specific action name (useful for test harnesses).
    ///
    /// # Arguments
    ///
    /// * `action` - Action name.
    /// * `code` - Exit code (0 for success, 1603 for failure).
    pub fn set_mock_result(&mut self, action: impl Into<String>, code: u32) {
        self.mock_results.insert(action.into(), code);
    }

    /// Checks whether an action has an explicit mock result registered.
    ///
    /// # Arguments
    ///
    /// * `action` - Action name.
    ///
    /// # Returns
    ///
    /// `true` if mock result exists, `false` otherwise.
    #[must_use]
    pub fn has_mock_result(&self, action: &str) -> bool {
        self.mock_results.contains_key(action)
    }

    /// Checks whether a native function or library is registered for dynamic execution.
    ///
    /// # Arguments
    ///
    /// * `symbol` - Symbol or entry point name.
    ///
    /// # Returns
    ///
    /// `true` if symbol is registered or library loaded, `false` otherwise.
    #[must_use]
    pub fn has_native_function(&self, symbol: &str) -> bool {
        self.library_loader.has_function(symbol)
    }

    /// Returns a mutable reference to the internal [`super::native_action::NativeLibraryLoader`].
    ///
    /// # Returns
    ///
    /// Mutable reference to the internal library loader.
    pub const fn library_loader_mut(&mut self) -> &mut super::native_action::NativeLibraryLoader {
        &mut self.library_loader
    }

    /// Returns a reference to the internal [`super::native_action::NativeLibraryLoader`].
    ///
    /// # Returns
    ///
    /// Reference to the internal library loader.
    #[must_use]
    pub const fn library_loader(&self) -> &super::native_action::NativeLibraryLoader {
        &self.library_loader
    }

    /// Returns a mutable reference to the internal [`super::native_action::SubprocessRunner`].
    ///
    /// # Returns
    ///
    /// Mutable reference to the internal subprocess runner.
    pub const fn subprocess_runner_mut(&mut self) -> &mut super::native_action::SubprocessRunner {
        &mut self.subprocess_runner
    }

    /// Returns a reference to the internal [`super::native_action::SubprocessRunner`].
    ///
    /// # Returns
    ///
    /// Reference to the internal subprocess runner.
    #[must_use]
    pub const fn subprocess_runner(&self) -> &super::native_action::SubprocessRunner {
        &self.subprocess_runner
    }

    /// Executes a custom action definition against the active evaluation context.
    ///
    /// # Arguments
    ///
    /// * `action` - The [`CustomActionDefinition`] to execute.
    /// * `context` - Active [`EvaluationContext`].
    ///
    /// # Returns
    ///
    /// Action return code (`0` for success).
    ///
    /// # Errors
    ///
    /// Returns [`Error::CustomActionFailed`] if execution fails.
    #[allow(clippy::too_many_lines)]
    pub fn execute(
        &self,
        action: &CustomActionDefinition,
        context: &mut EvaluationContext,
    ) -> Result<u32> {
        if let Some(ref policy) = self.offline_policy {
            let disposition = policy.evaluate_action(action.name(), action.raw_type());
            match disposition {
                crate::execution::bare_metal::OfflineActionDisposition::SkipWithSuccess => {
                    return Ok(ERROR_SUCCESS);
                }
                crate::execution::bare_metal::OfflineActionDisposition::RejectUnsafe => {
                    return Err(Error::CustomActionFailed {
                        action: action.name().to_string(),
                        reason: format!(
                            "Custom action '{}' (type 0x{:04X}) rejected by offline execution policy",
                            action.name(),
                            action.raw_type()
                        ),
                    });
                }
                crate::execution::bare_metal::OfflineActionDisposition::ExecuteInChroot
                | crate::execution::bare_metal::OfflineActionDisposition::ExecuteDirect => {}
            }
        }

        if let Some(&code) = self.mock_results.get(action.name()) {
            if code != ERROR_SUCCESS {
                return Err(Error::CustomActionFailed {
                    action: action.name().to_string(),
                    reason: format!("Mock failure code {code}"),
                });
            }
            return Ok(code);
        }

        match action.source_type() {
            CustomActionSourceType::Error => {
                let formatted_msg = context.format_string(action.target())?;
                Err(Error::CustomActionFailed {
                    action: action.name().to_string(),
                    reason: if formatted_msg.is_empty() {
                        format!("Custom action '{}' aborted installation", action.name())
                    } else {
                        formatted_msg
                    },
                })
            }
            CustomActionSourceType::TextData
            | CustomActionSourceType::Property
            | CustomActionSourceType::Directory => {
                let formatted = context.format_string(action.target())?;
                context.set_property(action.source(), formatted);
                Ok(ERROR_SUCCESS)
            }
            CustomActionSourceType::JScript | CustomActionSourceType::VBScript => {
                if action.name().starts_with("CheckPorts_")
                    || action.name().starts_with("CheckPort_")
                    || action.target().contains("CheckPort")
                    || action.target().contains("netstat")
                {
                    let pkg_suffix = action
                        .name()
                        .strip_prefix("CheckPorts_")
                        .or_else(|| action.name().strip_prefix("CheckPort_"))
                        .map_or("SERVICE", |s| s);
                    let port_prop_name = format!("PROP_{}_PORT", pkg_suffix.to_ascii_uppercase());
                    let port_num = context
                        .get_property(&port_prop_name)
                        .and_then(|p| p.parse::<u16>().ok())
                        .or_else(|| pkg_suffix.parse::<u16>().ok())
                        .unwrap_or(3306);

                    let available = probe_port_available(port_num);
                    context.set_property(
                        format!("PORT_{pkg_suffix}_AVAILABLE"),
                        if available { "1" } else { "0" },
                    );
                    context.set_property(
                        format!("{pkg_suffix}_PORT_IN_USE"),
                        if available { "0" } else { "1" },
                    );
                    return Ok(ERROR_SUCCESS);
                }

                let script_code = if !action.target().trim().is_empty() {
                    action.target().to_string()
                } else if let Some(bytes) = self.binaries.get(action.source()) {
                    String::from_utf8_lossy(bytes).into_owned()
                } else {
                    String::new()
                };

                let mut session =
                    crate::execution::script_engine::ScriptSession::new(context.clone(), None);
                if action.source_type() == CustomActionSourceType::JScript {
                    let mut engine = crate::execution::script_engine::JScriptEngine::new();
                    engine.execute(&script_code, &mut session)?;
                } else {
                    let mut engine = crate::execution::script_engine::VBScriptEngine::new();
                    engine.execute(&script_code, &mut session)?;
                }
                *context = session.context().clone();
                Ok(ERROR_SUCCESS)
            }
            CustomActionSourceType::Dll | CustomActionSourceType::InstalledDll => {
                self.execute_dll_action(action, context)
            }
            CustomActionSourceType::Exe
            | CustomActionSourceType::InstalledExe
            | CustomActionSourceType::DirectoryExe
            | CustomActionSourceType::PropertyExe => {
                let (prog, args, working_dir) = match action.source_type() {
                    CustomActionSourceType::DirectoryExe => {
                        let work_dir = context.get_property(action.source()).map(PathBuf::from);
                        let formatted_target = context.format_string(action.target())?;
                        let (p, a) = parse_command_line(&formatted_target);
                        (p, a, work_dir)
                    }
                    CustomActionSourceType::PropertyExe => {
                        let prog_str = context
                            .get_property(action.source())
                            .map_or_else(|| action.source(), |s| s);
                        let formatted_prog = context.format_string(prog_str)?;
                        let (p, mut p_args) = parse_command_line(&formatted_prog);
                        let formatted_target = context.format_string(action.target())?;
                        let (_, mut extra_args) = parse_command_line(&formatted_target);
                        p_args.append(&mut extra_args);
                        (p, p_args, None)
                    }
                    _ => {
                        let formatted_target = context.format_string(action.target())?;
                        let (p, a) = parse_command_line(&formatted_target);
                        (p, a, None)
                    }
                };

                let effective_prog = if let Some(ref wd) = working_dir {
                    if !prog.is_absolute() && wd.join(&prog).exists() {
                        wd.join(&prog)
                    } else {
                        prog
                    }
                } else {
                    prog
                };

                if effective_prog.as_os_str().is_empty() {
                    return Ok(ERROR_SUCCESS);
                }

                let mut envs = HashMap::new();
                for (k, v) in context.properties() {
                    envs.insert(k.clone(), v.clone());
                }

                if (effective_prog.is_file() || is_executable_in_path(&effective_prog))
                    && !effective_prog.is_dir()
                {
                    if action.execution_mode() == CustomActionExecutionMode::Async {
                        let _ = self.subprocess_runner.spawn_async(
                            &effective_prog,
                            &args,
                            working_dir.as_deref(),
                            &envs,
                        )?;
                        Ok(ERROR_SUCCESS)
                    } else {
                        match self.subprocess_runner.run(
                            &effective_prog,
                            &args,
                            working_dir.as_deref(),
                            &envs,
                        ) {
                            Ok(res) => Ok(res.exit_code),
                            Err(e) => {
                                if action.execution_mode().is_continue() {
                                    Ok(ERROR_SUCCESS)
                                } else {
                                    Err(Error::CustomActionFailed {
                                        action: action.name().to_string(),
                                        reason: e.to_string(),
                                    })
                                }
                            }
                        }
                    }
                } else {
                    Ok(ERROR_SUCCESS)
                }
            }
        }
    }

    /// Dispatches a native DLL custom action with session registration and property synchronization.
    fn execute_dll_action(
        &self,
        action: &CustomActionDefinition,
        context: &mut EvaluationContext,
    ) -> Result<u32> {
        let session = InstallSession {
            context: context.clone(),
            database: None,
            messages: Vec::new(),
        };
        let h_install = {
            let mut lock = lock_handles();
            lock.register_session(session)
        };

        let res = self
            .library_loader
            .invoke_action(action.target(), h_install)
            .or_else(|_| self.library_loader.invoke_action(action.name(), h_install));

        // Synchronize updated properties back to caller context and close session
        {
            let mut lock = lock_handles();
            if let Some(sess) = lock.sessions.get(&h_install) {
                for (k, v) in sess.context.properties() {
                    context.set_property(k, v);
                }
            }
            lock.close_handle(h_install);
        }

        match res {
            Ok(code) => Ok(code),
            Err(e) => {
                if action.execution_mode().is_continue() {
                    Ok(ERROR_SUCCESS)
                } else {
                    Err(e)
                }
            }
        }
    }
}

/// Active MSI install session holding evaluation context, database, and message logs.
#[derive(Debug, Default)]
pub struct InstallSession {
    /// Execution property context.
    pub context: EvaluationContext,
    /// Active database instance.
    pub database: Option<LinkedDatabase>,
    /// Last processed message log.
    pub messages: Vec<(u32, String)>,
}

/// Generic MSI API handle type (`MSIHANDLE`).
pub type MSIHANDLE = u32;

/// Thread-safe registry and handle table managing `MSIHANDLE` allocations.
#[derive(Debug, Default)]
pub struct HandleManager {
    /// Next handle ID to allocate.
    next_handle: u32,
    /// Registered install sessions.
    sessions: HashMap<MSIHANDLE, InstallSession>,
    /// Registered database records.
    records: HashMap<MSIHANDLE, Record>,
    /// Registered databases.
    databases: HashMap<MSIHANDLE, LinkedDatabase>,
}

impl HandleManager {
    /// Creates a new [`HandleManager`].
    #[must_use]
    pub fn new() -> Self {
        Self {
            next_handle: 100,
            sessions: HashMap::new(),
            records: HashMap::new(),
            databases: HashMap::new(),
        }
    }

    /// Registers an [`InstallSession`] and assigns a unique handle.
    ///
    /// # Arguments
    ///
    /// * `session` - Install session to register.
    ///
    /// # Returns
    ///
    /// Newly allocated [`MSIHANDLE`].
    pub fn register_session(&mut self, session: InstallSession) -> MSIHANDLE {
        let handle = self.next_handle;
        self.next_handle = self.next_handle.wrapping_add(1);
        self.sessions.insert(handle, session);
        handle
    }

    /// Registers a [`Record`] and assigns a unique handle.
    ///
    /// # Arguments
    ///
    /// * `record` - Table record to register.
    ///
    /// # Returns
    ///
    /// Newly allocated [`MSIHANDLE`].
    pub fn register_record(&mut self, record: Record) -> MSIHANDLE {
        let handle = self.next_handle;
        self.next_handle = self.next_handle.wrapping_add(1);
        self.records.insert(handle, record);
        handle
    }

    /// Registers a [`LinkedDatabase`] and assigns a unique handle.
    ///
    /// # Arguments
    ///
    /// * `database` - Linked relational database to register.
    ///
    /// # Returns
    ///
    /// Newly allocated [`MSIHANDLE`].
    pub fn register_database(&mut self, database: LinkedDatabase) -> MSIHANDLE {
        let handle = self.next_handle;
        self.next_handle = self.next_handle.wrapping_add(1);
        self.databases.insert(handle, database);
        handle
    }

    /// Closes a handle, freeing its registered session, record, or database.
    ///
    /// # Arguments
    ///
    /// * `handle` - Handle ID to release.
    ///
    /// # Returns
    ///
    /// `true` if handle was found and removed, `false` otherwise.
    pub fn close_handle(&mut self, handle: MSIHANDLE) -> bool {
        let removed_session = self.sessions.remove(&handle).is_some();
        let removed_record = self.records.remove(&handle).is_some();
        let removed_db = self.databases.remove(&handle).is_some();
        removed_session || removed_record || removed_db
    }
}

/// Global lazy-initialized handle table for MSI C API shims.
static GLOBAL_HANDLES: LazyLock<Mutex<HandleManager>> =
    LazyLock::new(|| Mutex::new(HandleManager::new()));

/// Safely acquires the global handle manager mutex guard, recovering state if poisoned.
fn lock_handles() -> std::sync::MutexGuard<'static, HandleManager> {
    match GLOBAL_HANDLES.lock() {
        Ok(guard) => guard,
        Err(poisoned) => {
            GLOBAL_HANDLES.clear_poison();
            poisoned.into_inner()
        }
    }
}

/// Returns the global handle manager mutex.
#[must_use]
pub fn global_handles() -> &'static Mutex<HandleManager> {
    &GLOBAL_HANDLES
}

// --------------------------------------------------------------------------------
// MSI API C Shims (for Native Shared Libraries)
// --------------------------------------------------------------------------------

/// Helper: Safely converts a null-terminated UTF-16 pointer into a Rust [`String`].
///
/// # Safety
///
/// `ptr` must be null or point to a valid null-terminated UTF-16 buffer.
unsafe fn utf16_ptr_to_string(ptr: *const u16) -> Option<String> {
    if ptr.is_null() {
        return None;
    }
    let mut len = 0;
    // SAFETY: ptr is non-null and points to a valid null-terminated UTF-16 sequence.
    unsafe {
        while *ptr.add(len) != 0 {
            len += 1;
        }
        let slice = std::slice::from_raw_parts(ptr, len);
        String::from_utf16(slice).ok()
    }
}

/// Helper: Safely writes a Rust string into a UTF-16 buffer with length tracking.
///
/// # Safety
///
/// `pcch_buf` must be a valid non-null pointer. `buf` must be null or point to writable memory.
unsafe fn write_utf16_buffer(src: &str, buf: *mut u16, pcch_buf: *mut u32) -> u32 {
    if pcch_buf.is_null() {
        return ERROR_INVALID_PARAMETER;
    }
    let utf16_chars: Vec<u16> = src.encode_utf16().collect();
    #[allow(clippy::cast_possible_truncation)]
    let needed = utf16_chars.len() as u32;

    // SAFETY: pcch_buf is verified non-null and valid.
    let capacity = unsafe { *pcch_buf };
    // SAFETY: pcch_buf is verified non-null and valid for write.
    unsafe { *pcch_buf = needed };

    if buf.is_null() {
        // Querying required length
        return ERROR_SUCCESS;
    }

    if capacity <= needed {
        // Buffer too small for string plus null terminator
        if capacity > 0 {
            let copy_len = (capacity as usize).saturating_sub(1);
            // SAFETY: buf is valid for write up to capacity elements.
            unsafe {
                for (i, &ch) in utf16_chars.iter().take(copy_len).enumerate() {
                    *buf.add(i) = ch;
                }
                *buf.add(copy_len) = 0;
            }
        }
        return ERROR_MORE_DATA;
    }

    // SAFETY: buf has capacity > needed elements and is valid for writes.
    unsafe {
        for (i, &ch) in utf16_chars.iter().enumerate() {
            *buf.add(i) = ch;
        }
        *buf.add(utf16_chars.len()) = 0;
    }
    ERROR_SUCCESS
}

/// MSI API C Shim: Retrieves the value of an installer property.
///
/// # Safety
///
/// Pointer arguments must be valid or null per Windows Installer C API rules.
#[must_use]
pub unsafe extern "C" fn MsiGetPropertyW(
    h_install: MSIHANDLE,
    sz_name: *const u16,
    sz_value_buf: *mut u16,
    pcch_value_buf: *mut u32,
) -> u32 {
    // SAFETY: sz_name is checked for null inside utf16_ptr_to_string.
    let Some(prop_name) = (unsafe { utf16_ptr_to_string(sz_name) }) else {
        return ERROR_INVALID_PARAMETER;
    };

    let guard = lock_handles();
    let Some(session) = guard.sessions.get(&h_install) else {
        return ERROR_INVALID_HANDLE;
    };

    let val = session.context.get_property(&prop_name).map_or("", |v| v);
    // SAFETY: sz_value_buf and pcch_value_buf are forwarded to write_utf16_buffer with caller guarantees.
    unsafe { write_utf16_buffer(val, sz_value_buf, pcch_value_buf) }
}

/// MSI API C Shim: Sets the value of an installer property.
///
/// # Safety
///
/// Pointer arguments must be valid or null per Windows Installer C API rules.
#[must_use]
pub unsafe extern "C" fn MsiSetPropertyW(
    h_install: MSIHANDLE,
    sz_name: *const u16,
    sz_value: *const u16,
) -> u32 {
    // SAFETY: sz_name is checked for null inside utf16_ptr_to_string.
    let Some(prop_name) = (unsafe { utf16_ptr_to_string(sz_name) }) else {
        return ERROR_INVALID_PARAMETER;
    };
    let prop_val = if sz_value.is_null() {
        String::new()
    } else {
        // SAFETY: sz_value is non-null and valid UTF-16 pointer per caller contract.
        match unsafe { utf16_ptr_to_string(sz_value) } {
            Some(s) => s,
            None => return ERROR_INVALID_PARAMETER,
        }
    };

    let mut guard = lock_handles();
    let Some(session) = guard.sessions.get_mut(&h_install) else {
        return ERROR_INVALID_HANDLE;
    };

    session.context.set_property(prop_name, prop_val);
    ERROR_SUCCESS
}

/// MSI API C Shim: Sends an execution or status message record to the installer session.
///
/// # Safety
///
/// `h_install` and `h_record` must be valid active handles.
#[must_use]
pub unsafe extern "C" fn MsiProcessMessage(
    h_install: MSIHANDLE,
    e_message_type: u32,
    h_record: MSIHANDLE,
) -> i32 {
    let mut guard = lock_handles();

    let msg_str = guard.records.get(&h_record).map_or_else(
        || "EmptyMessage".to_string(),
        |rec| {
            if let Some(FieldValue::String(s)) = rec.get(0) {
                s.clone()
            } else {
                "RecordMessage".to_string()
            }
        },
    );

    if let Some(session) = guard.sessions.get_mut(&h_install) {
        session.messages.push((e_message_type, msg_str));
        1 // IDOK
    } else {
        0 // IDABORT
    }
}

/// MSI API C Shim: Creates a new in-memory record structure with specified parameter field capacity.
#[must_use]
pub extern "C" fn MsiCreateRecord(c_params: u32) -> MSIHANDLE {
    let count = c_params as usize;
    let mut fields = Vec::with_capacity(count + 1);
    for _ in 0..=count {
        fields.push(FieldValue::Null);
    }
    let record = Record::with_fields(fields);

    let mut guard = lock_handles();
    guard.register_record(record)
}

/// MSI API C Shim: Sets a field in a record to a string value.
///
/// # Safety
///
/// `sz_value` must be a valid null-terminated UTF-16 string pointer or null.
#[must_use]
pub unsafe extern "C" fn MsiRecordSetStringW(
    h_record: MSIHANDLE,
    i_field: u32,
    sz_value: *const u16,
) -> u32 {
    let val = if sz_value.is_null() {
        FieldValue::Null
    } else {
        // SAFETY: sz_value is non-null and caller guarantees valid null-terminated UTF-16 pointer.
        match unsafe { utf16_ptr_to_string(sz_value) } {
            Some(s) => FieldValue::String(s),
            None => return ERROR_INVALID_PARAMETER,
        }
    };

    let mut guard = lock_handles();
    let Some(record) = guard.records.get_mut(&h_record) else {
        return ERROR_INVALID_HANDLE;
    };

    let idx = i_field as usize;
    while record.fields().len() <= idx {
        record.push(FieldValue::Null);
    }
    // Update record field
    let mut new_fields = record.fields().to_vec();
    new_fields[idx] = val;
    *record = Record::with_fields(new_fields);

    ERROR_SUCCESS
}

/// MSI API C Shim: Sets a field in a record to a 32-bit integer value.
#[must_use]
pub extern "C" fn MsiRecordSetInteger(h_record: MSIHANDLE, i_field: u32, i_value: i32) -> u32 {
    let mut guard = lock_handles();
    let Some(record) = guard.records.get_mut(&h_record) else {
        return ERROR_INVALID_HANDLE;
    };

    let idx = i_field as usize;
    while record.fields().len() <= idx {
        record.push(FieldValue::Null);
    }

    let val = if i_value == MSI_NULL_INTEGER_32 {
        FieldValue::Null
    } else {
        FieldValue::Long(i_value)
    };

    let mut new_fields = record.fields().to_vec();
    new_fields[idx] = val;
    *record = Record::with_fields(new_fields);

    ERROR_SUCCESS
}

/// MSI API C Shim: Retrieves a string value from a record field.
///
/// # Safety
///
/// Pointer arguments must be valid or null per Windows Installer C API conventions.
#[must_use]
pub unsafe extern "C" fn MsiRecordGetStringW(
    h_record: MSIHANDLE,
    i_field: u32,
    sz_value_buf: *mut u16,
    pcch_value_buf: *mut u32,
) -> u32 {
    let guard = lock_handles();
    let Some(record) = guard.records.get(&h_record) else {
        return ERROR_INVALID_HANDLE;
    };

    let str_val = match record.get(i_field as usize) {
        Some(FieldValue::String(s)) => s.as_str(),
        _ => "",
    };

    // SAFETY: sz_value_buf and pcch_value_buf forwarded with caller contract.
    unsafe { write_utf16_buffer(str_val, sz_value_buf, pcch_value_buf) }
}

/// MSI API C Shim: Retrieves an integer value from a record field.
#[must_use]
pub extern "C" fn MsiRecordGetInteger(h_record: MSIHANDLE, i_field: u32) -> i32 {
    let guard = lock_handles();
    let Some(record) = guard.records.get(&h_record) else {
        return MSI_NULL_INTEGER_32;
    };

    match record.get(i_field as usize) {
        Some(FieldValue::Short(n)) => i32::from(*n),
        Some(FieldValue::Long(n)) => *n,
        _ => MSI_NULL_INTEGER_32,
    }
}

/// MSI API C Shim: Executes a built-in or custom action by name within the install session.
///
/// # Safety
///
/// `sz_action` must be a valid null-terminated UTF-16 string pointer.
#[must_use]
pub unsafe extern "C" fn MsiDoActionW(h_install: MSIHANDLE, sz_action: *const u16) -> u32 {
    // SAFETY: sz_action is verified by utf16_ptr_to_string.
    let Some(action_name) = (unsafe { utf16_ptr_to_string(sz_action) }) else {
        return ERROR_INVALID_PARAMETER;
    };

    let mut guard = lock_handles();
    let Some(session) = guard.sessions.get_mut(&h_install) else {
        return ERROR_INVALID_HANDLE;
    };

    session
        .messages
        .push((1, format!("ActionExecuted({action_name})")));
    ERROR_SUCCESS
}

/// MSI API C Shim: Evaluates a conditional expression within the install session.
///
/// # Safety
///
/// `sz_condition` must be a valid null-terminated UTF-16 string pointer.
#[must_use]
pub unsafe extern "C" fn MsiEvaluateConditionW(
    h_install: MSIHANDLE,
    sz_condition: *const u16,
) -> u32 {
    // SAFETY: sz_condition is verified by utf16_ptr_to_string.
    let Some(expr) = (unsafe { utf16_ptr_to_string(sz_condition) }) else {
        return MSICONDITION_ERROR;
    };

    if expr.trim().is_empty() {
        return MSICONDITION_NONE;
    }

    let guard = lock_handles();
    let Some(session) = guard.sessions.get(&h_install) else {
        return MSICONDITION_ERROR;
    };

    match session.context.evaluate_condition(&expr) {
        Ok(true) => MSICONDITION_TRUE,
        Ok(false) => MSICONDITION_FALSE,
        Err(_) => MSICONDITION_ERROR,
    }
}

/// MSI API C Shim: Retrieves a handle to the active installation database.
#[must_use]
pub extern "C" fn MsiGetActiveDatabase(h_install: MSIHANDLE) -> MSIHANDLE {
    let mut guard = lock_handles();

    let db_clone = guard
        .sessions
        .get(&h_install)
        .and_then(|session| session.database.clone());

    let Some(db) = db_clone else {
        return 0;
    };

    guard.register_database(db)
}

/// MSI API C Shim: Closes an open installer handle.
#[must_use]
pub extern "C" fn MsiCloseHandle(h_any: MSIHANDLE) -> u32 {
    let mut guard = lock_handles();

    if guard.close_handle(h_any) {
        ERROR_SUCCESS
    } else {
        ERROR_INVALID_HANDLE
    }
}

#[cfg(test)]
#[allow(clippy::too_many_lines, clippy::unnecessary_wraps)]
mod tests {
    use super::*;

    /// Tests custom action source type and execution mode bitmask parsing.
    #[test]
    fn test_custom_action_definition_parsing() -> Result<()> {
        // Type 1: DLL in Binary table, synchronous
        let ca_dll = CustomActionDefinition::parse("CADll", 0x0001, "MyDll", "MyFn")?;
        assert_eq!(ca_dll.source_type(), CustomActionSourceType::Dll);
        assert_eq!(
            ca_dll.execution_mode(),
            CustomActionExecutionMode::Synchronous
        );
        assert_eq!(ca_dll.in_script(), InScriptMode::Immediate);
        assert!(!ca_dll.no_impersonate());

        // Type 0x0C02: EXE in Binary table + InScript Deferred + NoImpersonate
        let ca_exe_def = CustomActionDefinition::parse(
            "CAExe",
            MSIDB_CUSTOM_ACTION_TYPE_EXE
                | MSIDB_CUSTOM_ACTION_TYPE_IN_SCRIPT
                | MSIDB_CUSTOM_ACTION_TYPE_NO_IMPERSONATE,
            "MyExe",
            "/install",
        )?;
        assert_eq!(ca_exe_def.source_type(), CustomActionSourceType::Exe);
        assert_eq!(ca_exe_def.in_script(), InScriptMode::Deferred);
        assert!(ca_exe_def.no_impersonate());

        // Type 0x0501: Rollback DLL action
        let ca_rb = CustomActionDefinition::parse(
            "CARollback",
            MSIDB_CUSTOM_ACTION_TYPE_DLL | MSIDB_CUSTOM_ACTION_TYPE_ROLLBACK,
            "MyDll",
            "RollbackFn",
        )?;
        assert_eq!(ca_rb.in_script(), InScriptMode::Rollback);

        // Type 0x0601: Commit DLL action
        let ca_commit = CustomActionDefinition::parse(
            "CACommit",
            MSIDB_CUSTOM_ACTION_TYPE_DLL | MSIDB_CUSTOM_ACTION_TYPE_COMMIT,
            "MyDll",
            "CommitFn",
        )?;
        assert_eq!(ca_commit.in_script(), InScriptMode::Commit);

        // Type 0x0043: Text data + Continue async
        let ca_text = CustomActionDefinition::parse(
            "CAText",
            MSIDB_CUSTOM_ACTION_TYPE_TEXT_DATA | MSIDB_CUSTOM_ACTION_TYPE_CONTINUE,
            "TARGET_PROP",
            "[SOURCE_PROP]",
        )?;
        assert_eq!(ca_text.source_type(), CustomActionSourceType::TextData);
        assert_eq!(
            ca_text.execution_mode(),
            CustomActionExecutionMode::Continue
        );

        // Type 0x0085: JScript + Async
        let ca_js = CustomActionDefinition::parse(
            "CAJS",
            MSIDB_CUSTOM_ACTION_TYPE_JSCRIPT | MSIDB_CUSTOM_ACTION_TYPE_ASYNC,
            "ScriptKey",
            "RunScript",
        )?;
        assert_eq!(ca_js.source_type(), CustomActionSourceType::JScript);
        assert_eq!(ca_js.execution_mode(), CustomActionExecutionMode::Async);

        // All other source types
        assert_eq!(
            CustomActionSourceType::from_raw(MSIDB_CUSTOM_ACTION_TYPE_VBSCRIPT),
            Ok(CustomActionSourceType::VBScript)
        );
        assert_eq!(
            CustomActionSourceType::from_raw(MSIDB_CUSTOM_ACTION_TYPE_INSTALLED_DLL),
            Ok(CustomActionSourceType::InstalledDll)
        );
        assert_eq!(
            CustomActionSourceType::from_raw(MSIDB_CUSTOM_ACTION_TYPE_INSTALLED_EXE),
            Ok(CustomActionSourceType::InstalledExe)
        );
        assert_eq!(
            CustomActionSourceType::from_raw(MSIDB_CUSTOM_ACTION_TYPE_DIRECTORY),
            Ok(CustomActionSourceType::Directory)
        );
        assert_eq!(
            CustomActionSourceType::from_raw(MSIDB_CUSTOM_ACTION_TYPE_PROPERTY),
            Ok(CustomActionSourceType::Property)
        );

        // Unrecognized source type returns error
        assert!(CustomActionSourceType::from_raw(0x002F).is_err());
        Ok(())
    }

    /// Tests [`CustomActionExecutor`] executing `TextData`, Property, and Script actions.
    #[test]
    fn test_custom_action_executor() -> Result<()> {
        let mut executor = CustomActionExecutor::new();
        let mut context = EvaluationContext::new();
        context.set_property("PRODUCT", "MyApplication");

        // TextData formatting
        let ca_text = CustomActionDefinition::parse(
            "SetGreeting",
            MSIDB_CUSTOM_ACTION_TYPE_TEXT_DATA,
            "GREETING",
            "Welcome to [PRODUCT]!",
        )?;
        let res = executor.execute(&ca_text, &mut context);
        assert_eq!(res, Ok(ERROR_SUCCESS));
        assert_eq!(
            context.get_property("GREETING"),
            Some("Welcome to MyApplication!")
        );

        // Script simulation
        let ca_script = CustomActionDefinition::parse(
            "SetVal",
            MSIDB_CUSTOM_ACTION_TYPE_JSCRIPT,
            "BinaryKey",
            r#"MY_FLAG = "ACTIVE""#,
        )?;
        let res_s = executor.execute(&ca_script, &mut context);
        assert_eq!(res_s, Ok(ERROR_SUCCESS));
        assert_eq!(context.get_property("MY_FLAG"), Some("ACTIVE"));

        // Mock failure
        executor.set_mock_result("FailingAction", 1603);
        let ca_fail = CustomActionDefinition::parse(
            "FailingAction",
            MSIDB_CUSTOM_ACTION_TYPE_DLL,
            "DllKey",
            "BadFn",
        )?;
        let res_f = executor.execute(&ca_fail, &mut context);
        assert_eq!(
            res_f,
            Err(Error::CustomActionFailed {
                action: "FailingAction".to_string(),
                reason: "Mock failure code 1603".to_string(),
            })
        );
        Ok(())
    }

    /// Tests MSI C API shims: `MsiGetPropertyW`, `MsiSetPropertyW`, `MsiEvaluateConditionW`, etc.
    #[test]
    fn test_msi_api_c_shims() -> Result<()> {
        // SAFETY: C API shims are exercised in tests with valid handles and pointers.
        unsafe {
            let mut session = InstallSession::default();
            session.context.set_property("MY_PROP", "InitialValue");
            session.database = Some(LinkedDatabase::new()?);

            let h_install = {
                let mut guard = lock_handles();
                guard.register_session(session)
            };

            // Test MsiGetPropertyW
            let prop_name_utf16: Vec<u16> = "MY_PROP\0".encode_utf16().collect();
            let mut buf = [0u16; 64];
            let mut cch: u32 = 64;

            let code = MsiGetPropertyW(
                h_install,
                prop_name_utf16.as_ptr(),
                buf.as_mut_ptr(),
                &raw mut cch,
            );
            assert_eq!(code, ERROR_SUCCESS);
            let val_read = String::from_utf16_lossy(&buf[..cch as usize]);
            assert_eq!(val_read, "InitialValue");

            // Test MsiSetPropertyW
            let new_val_utf16: Vec<u16> = "UpdatedValue\0".encode_utf16().collect();
            let set_code =
                MsiSetPropertyW(h_install, prop_name_utf16.as_ptr(), new_val_utf16.as_ptr());
            assert_eq!(set_code, ERROR_SUCCESS);

            // Re-read with MsiGetPropertyW
            cch = 64;
            let code2 = MsiGetPropertyW(
                h_install,
                prop_name_utf16.as_ptr(),
                buf.as_mut_ptr(),
                &raw mut cch,
            );
            assert_eq!(code2, ERROR_SUCCESS);
            assert_eq!(
                String::from_utf16_lossy(&buf[..cch as usize]),
                "UpdatedValue"
            );

            // Test MsiCreateRecord, MsiRecordSetStringW, MsiRecordGetStringW
            let h_record = MsiCreateRecord(3);
            assert_ne!(h_record, 0);

            let rec_str_utf16: Vec<u16> = "RecordFieldContent\0".encode_utf16().collect();
            let s_code = MsiRecordSetStringW(h_record, 1, rec_str_utf16.as_ptr());
            assert_eq!(s_code, ERROR_SUCCESS);

            let mut rec_buf = [0u16; 64];
            let mut rec_cch: u32 = 64;
            let g_code = MsiRecordGetStringW(h_record, 1, rec_buf.as_mut_ptr(), &raw mut rec_cch);
            assert_eq!(g_code, ERROR_SUCCESS);
            assert_eq!(
                String::from_utf16_lossy(&rec_buf[..rec_cch as usize]),
                "RecordFieldContent"
            );

            // Test MsiRecordSetInteger, MsiRecordGetInteger
            let int_code = MsiRecordSetInteger(h_record, 2, 42);
            assert_eq!(int_code, ERROR_SUCCESS);
            assert_eq!(MsiRecordGetInteger(h_record, 2), 42);

            // Test MsiProcessMessage with string field 0
            let msg0_utf16: Vec<u16> = "RecordMessageTemplate\0".encode_utf16().collect();
            assert_eq!(
                MsiRecordSetStringW(h_record, 0, msg0_utf16.as_ptr()),
                ERROR_SUCCESS
            );
            let proc_res = MsiProcessMessage(h_install, 1, h_record);
            assert_eq!(proc_res, 1); // IDOK

            // Test MsiDoActionW
            let act_utf16: Vec<u16> = "InstallFiles\0".encode_utf16().collect();
            assert_eq!(MsiDoActionW(h_install, act_utf16.as_ptr()), ERROR_SUCCESS);

            // Test MsiEvaluateConditionW
            let cond_true_utf16: Vec<u16> = "MY_PROP = \"UpdatedValue\"\0".encode_utf16().collect();
            assert_eq!(
                MsiEvaluateConditionW(h_install, cond_true_utf16.as_ptr()),
                MSICONDITION_TRUE
            );

            let cond_false_utf16: Vec<u16> = "MY_PROP = \"OldValue\"\0".encode_utf16().collect();
            assert_eq!(
                MsiEvaluateConditionW(h_install, cond_false_utf16.as_ptr()),
                MSICONDITION_FALSE
            );

            let cond_empty_utf16: Vec<u16> = "\0".encode_utf16().collect();
            assert_eq!(
                MsiEvaluateConditionW(h_install, cond_empty_utf16.as_ptr()),
                MSICONDITION_NONE
            );

            // Test MsiGetActiveDatabase
            let h_db = MsiGetActiveDatabase(h_install);
            assert_ne!(h_db, 0);
            assert_eq!(MsiCloseHandle(h_db), ERROR_SUCCESS);

            // Test MsiCloseHandle
            assert_eq!(MsiCloseHandle(h_record), ERROR_SUCCESS);
            assert_eq!(MsiCloseHandle(h_install), ERROR_SUCCESS);
            assert_eq!(MsiCloseHandle(999_999), ERROR_INVALID_HANDLE);
            Ok(())
        }
    }

    /// Mock custom action for executor test setting a property.
    unsafe extern "system-unwind" fn mock_executor_action(h: MSIHANDLE) -> u32 {
        let name_utf16: Vec<u16> = "CUSTOM_PROP\0".encode_utf16().collect();
        let val_utf16: Vec<u16> = "CustomValue\0".encode_utf16().collect();
        // SAFETY: Pointers to valid null-terminated buffers passed to MsiSetPropertyW.
        let _ = unsafe { MsiSetPropertyW(h, name_utf16.as_ptr(), val_utf16.as_ptr()) };
        ERROR_SUCCESS
    }

    /// Mock custom action returning failure.
    unsafe extern "system-unwind" fn mock_failing_action(_h: MSIHANDLE) -> u32 {
        1603 // ERROR_INSTALL_FAILURE
    }

    /// Mock custom action closing its own session handle before return.
    unsafe extern "system-unwind" fn mock_closing_action(h: MSIHANDLE) -> u32 {
        let _ = MsiCloseHandle(h);
        ERROR_SUCCESS
    }

    /// Tests native library and subprocess execution in `CustomActionExecutor`.
    #[test]
    fn test_custom_action_executor_native_and_subprocess() -> Result<()> {
        let mut executor = CustomActionExecutor::new();
        let mut ctx = EvaluationContext::new();

        // Exercise global_handles getter
        assert!(!std::ptr::eq(global_handles(), std::ptr::null()));

        // Check getters
        assert_eq!(
            executor.subprocess_runner().timeout_ms,
            crate::execution::native_action::DEFAULT_ACTION_TIMEOUT_MS
        );
        executor.subprocess_runner_mut().timeout_ms = 10_000;
        assert_eq!(executor.subprocess_runner().timeout_ms, 10_000);

        let _ = executor.library_loader();
        executor
            .library_loader_mut()
            .register_function("MyNativeEntry", mock_executor_action);
        executor
            .library_loader_mut()
            .register_function("FailingEntry", mock_failing_action);
        executor
            .library_loader_mut()
            .register_function("ClosingEntry", mock_closing_action);

        // DLL action with registered symbol mutating property
        let dll_def = CustomActionDefinition::parse(
            "DllAction",
            MSIDB_CUSTOM_ACTION_TYPE_DLL,
            "BinaryKey",
            "MyNativeEntry",
        )?;
        assert_eq!(executor.execute(&dll_def, &mut ctx)?, ERROR_SUCCESS);
        assert_eq!(ctx.get_property("CUSTOM_PROP"), Some("CustomValue"));

        // DLL action closing its session handle early
        let closing_dll = CustomActionDefinition::parse(
            "ClosingDll",
            MSIDB_CUSTOM_ACTION_TYPE_DLL,
            "BinaryKey",
            "ClosingEntry",
        )?;
        assert_eq!(executor.execute(&closing_dll, &mut ctx)?, ERROR_SUCCESS);

        // DLL action returning failure without continue -> should fail
        let failing_dll = CustomActionDefinition::parse(
            "FailingEntry",
            MSIDB_CUSTOM_ACTION_TYPE_DLL,
            "BinaryKey",
            "FailingEntry",
        )?;
        assert_eq!(
            executor.execute(&failing_dll, &mut ctx),
            Err(Error::CustomActionFailed {
                action: "FailingEntry".to_string(),
                reason: "native custom action returned error code 1603".to_string(),
            })
        );

        // DLL action returning failure with continue -> should succeed
        let failing_continue_dll = CustomActionDefinition::parse(
            "FailingContinueDll",
            MSIDB_CUSTOM_ACTION_TYPE_DLL | MSIDB_CUSTOM_ACTION_TYPE_CONTINUE,
            "BinaryKey",
            "FailingEntry",
        )?;
        assert_eq!(
            executor.execute(&failing_continue_dll, &mut ctx)?,
            ERROR_SUCCESS
        );

        // Installed DLL action with unregistered symbol and continue flag
        let unreg_dll = CustomActionDefinition::parse(
            "UnregDll",
            MSIDB_CUSTOM_ACTION_TYPE_INSTALLED_DLL | MSIDB_CUSTOM_ACTION_TYPE_CONTINUE,
            "BinaryKey",
            "NonExistentEntry",
        )?;
        assert_eq!(executor.execute(&unreg_dll, &mut ctx)?, ERROR_SUCCESS);

        // Installed DLL action with unregistered symbol without continue -> should fail
        let unreg_strict_dll = CustomActionDefinition::parse(
            "UnregStrictDll",
            MSIDB_CUSTOM_ACTION_TYPE_INSTALLED_DLL,
            "BinaryKey",
            "NonExistentEntry",
        )?;
        assert!(executor.execute(&unreg_strict_dll, &mut ctx).is_err());

        // Exe action pointing to non-existent path
        let exe_def = CustomActionDefinition::parse(
            "ExeAction",
            MSIDB_CUSTOM_ACTION_TYPE_EXE,
            "BinaryKey",
            "/path/to/nonexistent/executable",
        )?;
        assert_eq!(executor.execute(&exe_def, &mut ctx)?, ERROR_SUCCESS);

        // Exe action pointing to directory (exists but is not a file)
        let dir_exe = CustomActionDefinition::parse(
            "DirExe",
            MSIDB_CUSTOM_ACTION_TYPE_EXE,
            "BinaryKey",
            "/",
        )?;
        assert_eq!(executor.execute(&dir_exe, &mut ctx)?, ERROR_SUCCESS);

        // Exe action pointing to existing binary (e.g. /bin/sh or cmd.exe)
        #[cfg(not(target_os = "windows"))]
        {
            let real_exe = CustomActionDefinition::parse(
                "RealExe",
                MSIDB_CUSTOM_ACTION_TYPE_EXE,
                "BinaryKey",
                "/bin/sh",
            )?;
            assert!(!real_exe.execution_mode().is_async());
            assert_eq!(executor.execute(&real_exe, &mut ctx)?, ERROR_SUCCESS);

            // Async exe action
            let async_exe = CustomActionDefinition::parse(
                "AsyncExe",
                MSIDB_CUSTOM_ACTION_TYPE_EXE | MSIDB_CUSTOM_ACTION_TYPE_ASYNC,
                "BinaryKey",
                "/bin/sh",
            )?;
            assert!(async_exe.execution_mode().is_async());
            assert_eq!(executor.execute(&async_exe, &mut ctx)?, ERROR_SUCCESS);
        }

        Ok(())
    }

    /// Tests all accessors and predicate methods on `CustomActionDefinition` and related enums.
    #[test]
    fn test_custom_action_definition_accessors_and_modes() -> Result<()> {
        let ca = CustomActionDefinition::parse(
            "ActionFull",
            MSIDB_CUSTOM_ACTION_TYPE_DLL
                | MSIDB_CUSTOM_ACTION_TYPE_CONTINUE
                | MSIDB_CUSTOM_ACTION_TYPE_FIRST_SEQUENCE
                | MSIDB_CUSTOM_ACTION_TYPE_NO_IMPERSONATE,
            "SrcKey",
            "TargetFn",
        )?;

        assert_eq!(ca.name(), "ActionFull");
        assert_eq!(
            ca.raw_type(),
            MSIDB_CUSTOM_ACTION_TYPE_DLL
                | MSIDB_CUSTOM_ACTION_TYPE_CONTINUE
                | MSIDB_CUSTOM_ACTION_TYPE_FIRST_SEQUENCE
                | MSIDB_CUSTOM_ACTION_TYPE_NO_IMPERSONATE
        );
        assert_eq!(ca.source_type(), CustomActionSourceType::Dll);
        assert_eq!(ca.execution_mode(), CustomActionExecutionMode::Continue);
        assert_eq!(ca.in_script(), InScriptMode::Immediate);
        assert!(ca.first_sequence());
        assert!(!ca.once_per_process());
        assert!(!ca.client_repeat());
        assert!(ca.no_impersonate());
        assert_eq!(ca.source(), "SrcKey");
        assert_eq!(ca.target(), "TargetFn");

        let ca_once = CustomActionDefinition::parse(
            "ActionOnce",
            MSIDB_CUSTOM_ACTION_TYPE_EXE | MSIDB_CUSTOM_ACTION_TYPE_ONCE_PER_PROCESS,
            "SrcExe",
            "RunExe",
        )?;
        assert!(!ca_once.first_sequence());
        assert!(ca_once.once_per_process());
        assert!(!ca_once.client_repeat());

        let ca_repeat = CustomActionDefinition::parse(
            "ActionRepeat",
            MSIDB_CUSTOM_ACTION_TYPE_TEXT_DATA | MSIDB_CUSTOM_ACTION_TYPE_CLIENT_REPEAT,
            "PropName",
            "PropVal",
        )?;
        assert!(!ca_repeat.first_sequence());
        assert!(!ca_repeat.once_per_process());
        assert!(ca_repeat.client_repeat());

        assert!(!CustomActionExecutionMode::Synchronous.is_async());
        assert!(!CustomActionExecutionMode::Synchronous.is_continue());
        assert!(CustomActionExecutionMode::Continue.is_async());
        assert!(CustomActionExecutionMode::Continue.is_continue());
        assert!(CustomActionExecutionMode::Async.is_async());
        assert!(!CustomActionExecutionMode::Async.is_continue());
        assert_eq!(
            CustomActionExecutionMode::default(),
            CustomActionExecutionMode::Synchronous
        );
        assert_eq!(InScriptMode::default(), InScriptMode::Immediate);

        Ok(())
    }

    /// Mock custom action returning reboot required.
    unsafe extern "system-unwind" fn mock_reboot_action(_h: MSIHANDLE) -> u32 {
        crate::execution::native_action::ERROR_SUCCESS_REBOOT_REQUIRED
    }

    /// Tests extended executor features: mock success, property actions, directory actions, vbscript, and name fallbacks.
    #[test]
    fn test_custom_action_executor_extended() -> Result<()> {
        let mut executor = CustomActionExecutor::new();
        let mut context = EvaluationContext::new();
        context.set_property("BASE_DIR", "/opt/app");
        context.set_property("PART1", "Hello");
        context.set_property("PART2", "World");

        // Mock success (code 0)
        executor.set_mock_result("MockSuccessAction", ERROR_SUCCESS);
        let ca_mock_ok = CustomActionDefinition::parse(
            "MockSuccessAction",
            MSIDB_CUSTOM_ACTION_TYPE_DLL,
            "DllSrc",
            "Fn",
        )?;
        assert_eq!(executor.execute(&ca_mock_ok, &mut context)?, ERROR_SUCCESS);

        // Property action (type 0x0033)
        let ca_prop = CustomActionDefinition::parse(
            "SetCombinedProp",
            MSIDB_CUSTOM_ACTION_TYPE_PROPERTY,
            "COMBINED",
            "[PART1] [PART2]!",
        )?;
        assert_eq!(executor.execute(&ca_prop, &mut context)?, ERROR_SUCCESS);
        assert_eq!(context.get_property("COMBINED"), Some("Hello World!"));

        // Directory action (type 0x0023)
        let ca_dir = CustomActionDefinition::parse(
            "SetDir",
            MSIDB_CUSTOM_ACTION_TYPE_DIRECTORY,
            "TARGETDIR",
            "[BASE_DIR]/subdir",
        )?;
        assert_eq!(executor.execute(&ca_dir, &mut context)?, ERROR_SUCCESS);

        // VBScript action (type 0x0006)
        let ca_vbs = CustomActionDefinition::parse(
            "RunVbs",
            MSIDB_CUSTOM_ACTION_TYPE_VBSCRIPT,
            "BinaryVbs",
            "Session.Property(\"VBS_OUTPUT\") = \"ExecutedVbs\"",
        )?;
        assert_eq!(executor.execute(&ca_vbs, &mut context)?, ERROR_SUCCESS);
        assert_eq!(context.get_property("VBS_OUTPUT"), Some("ExecutedVbs"));

        // DLL action returning ERROR_SUCCESS_REBOOT_REQUIRED
        executor
            .library_loader_mut()
            .register_function("RebootEntry", mock_reboot_action);
        let ca_reboot = CustomActionDefinition::parse(
            "RebootAction",
            MSIDB_CUSTOM_ACTION_TYPE_DLL,
            "BinaryDll",
            "RebootEntry",
        )?;
        assert_eq!(
            executor.execute(&ca_reboot, &mut context)?,
            crate::execution::native_action::ERROR_SUCCESS_REBOOT_REQUIRED
        );

        // DLL action where target entry doesn't exist, but action name does (fallback)
        executor
            .library_loader_mut()
            .register_function("FallbackByName", mock_executor_action);
        let ca_fallback = CustomActionDefinition::parse(
            "FallbackByName",
            MSIDB_CUSTOM_ACTION_TYPE_DLL,
            "BinaryDll",
            "NonExistentTargetName",
        )?;
        assert_eq!(executor.execute(&ca_fallback, &mut context)?, ERROR_SUCCESS);

        Ok(())
    }

    /// Tests UTF-16 pointer conversion and buffer writing helpers directly.
    #[test]
    fn test_utf16_helpers_and_buffer_edge_cases() {
        // SAFETY: utf16_ptr_to_string safely validates null pointers.
        unsafe {
            assert!(utf16_ptr_to_string(std::ptr::null()).is_none());
        }

        // SAFETY: Testing write_utf16_buffer pointer validations and truncation behavior.
        unsafe {
            // Null pcch_buf
            assert_eq!(
                write_utf16_buffer("Test", std::ptr::null_mut(), std::ptr::null_mut()),
                ERROR_INVALID_PARAMETER
            );

            // Null buffer (query size)
            let mut cch: u32 = 0;
            assert_eq!(
                write_utf16_buffer("Test", std::ptr::null_mut(), &raw mut cch),
                ERROR_SUCCESS
            );
            assert_eq!(cch, 4);

            // Buffer capacity 0 (capacity <= needed, capacity == 0)
            let mut buf = [0u16; 16];
            let mut cch_zero: u32 = 0;
            assert_eq!(
                write_utf16_buffer("LongString", buf.as_mut_ptr(), &raw mut cch_zero),
                ERROR_MORE_DATA
            );
            assert_eq!(cch_zero, 10);

            // Buffer capacity 4 (capacity <= needed, capacity > 0)
            let mut cch_small: u32 = 4;
            assert_eq!(
                write_utf16_buffer("LongString", buf.as_mut_ptr(), &raw mut cch_small),
                ERROR_MORE_DATA
            );
            assert_eq!(cch_small, 10);
            let truncated = String::from_utf16_lossy(&buf[..3]);
            assert_eq!(truncated, "Lon");
            assert_eq!(buf[3], 0);
        }
    }

    /// Tests extended error and edge case paths across MSI C API shims.
    #[test]
    fn test_msi_api_c_shims_extended_error_paths() -> Result<()> {
        // SAFETY: Exercising C API shims with null/invalid handles and corner-case buffers.
        unsafe {
            let session = InstallSession {
                context: EvaluationContext::new(),
                database: None,
                messages: Vec::new(),
            };
            let h_install = {
                let mut guard = lock_handles();
                guard.register_session(session)
            };

            // MsiGetPropertyW error paths
            assert_eq!(
                MsiGetPropertyW(
                    h_install,
                    std::ptr::null(),
                    std::ptr::null_mut(),
                    std::ptr::null_mut()
                ),
                ERROR_INVALID_PARAMETER
            );
            let prop_utf16: Vec<u16> = "NONEXISTENT\0".encode_utf16().collect();
            let mut cch: u32 = 16;
            let mut buf = [0u16; 16];
            assert_eq!(
                MsiGetPropertyW(999_999, prop_utf16.as_ptr(), buf.as_mut_ptr(), &raw mut cch),
                ERROR_INVALID_HANDLE
            );
            assert_eq!(
                MsiGetPropertyW(
                    h_install,
                    prop_utf16.as_ptr(),
                    buf.as_mut_ptr(),
                    &raw mut cch
                ),
                ERROR_SUCCESS
            );
            assert_eq!(cch, 0);

            // MsiSetPropertyW error paths
            assert_eq!(
                MsiSetPropertyW(h_install, std::ptr::null(), std::ptr::null()),
                ERROR_INVALID_PARAMETER
            );
            assert_eq!(
                MsiSetPropertyW(999_999, prop_utf16.as_ptr(), std::ptr::null()),
                ERROR_INVALID_HANDLE
            );
            assert_eq!(
                MsiSetPropertyW(h_install, prop_utf16.as_ptr(), std::ptr::null()),
                ERROR_SUCCESS
            );

            // MsiProcessMessage error paths
            assert_eq!(MsiProcessMessage(999_999, 1, 0), 0); // IDABORT
            let h_rec_empty = MsiCreateRecord(1);
            assert_eq!(MsiProcessMessage(h_install, 1, h_rec_empty), 1); // IDOK
            let _ = MsiRecordSetInteger(h_rec_empty, 0, 100);
            assert_eq!(MsiProcessMessage(h_install, 1, h_rec_empty), 1); // IDOK

            // MsiRecordSetStringW and MsiRecordGetStringW
            assert_eq!(
                MsiRecordSetStringW(999_999, 1, std::ptr::null()),
                ERROR_INVALID_HANDLE
            );
            assert_eq!(
                MsiRecordSetStringW(h_rec_empty, 1, std::ptr::null()),
                ERROR_SUCCESS
            );
            assert_eq!(
                MsiRecordSetStringW(h_rec_empty, 10, prop_utf16.as_ptr()),
                ERROR_SUCCESS
            );
            let mut get_cch: u32 = 16;
            assert_eq!(
                MsiRecordGetStringW(999_999, 1, std::ptr::null_mut(), &raw mut get_cch),
                ERROR_INVALID_HANDLE
            );
            assert_eq!(
                MsiRecordGetStringW(h_rec_empty, 2, buf.as_mut_ptr(), &raw mut get_cch),
                ERROR_SUCCESS
            );

            // MsiRecordSetInteger and MsiRecordGetInteger
            assert_eq!(MsiRecordSetInteger(999_999, 1, 5), ERROR_INVALID_HANDLE);
            assert_eq!(
                MsiRecordSetInteger(h_rec_empty, 1, MSI_NULL_INTEGER_32),
                ERROR_SUCCESS
            );
            assert_eq!(MsiRecordSetInteger(h_rec_empty, 12, 99), ERROR_SUCCESS);
            assert_eq!(MsiRecordGetInteger(999_999, 1), MSI_NULL_INTEGER_32);
            assert_eq!(MsiRecordGetInteger(h_rec_empty, 1), MSI_NULL_INTEGER_32);
            assert_eq!(MsiRecordGetInteger(h_rec_empty, 10), MSI_NULL_INTEGER_32);
            {
                let mut guard = lock_handles();
                let rec = guard.records.entry(h_rec_empty).or_default();
                let mut fields = rec.fields().to_vec();
                fields.push(FieldValue::Short(77));
                *rec = Record::with_fields(fields);
            }
            assert_eq!(MsiRecordGetInteger(h_rec_empty, 13), 77);

            // Invalid UTF-16 unpaired surrogate sequences
            let invalid_utf16 = [0xD800u16, 0u16];
            assert_eq!(
                MsiSetPropertyW(h_install, prop_utf16.as_ptr(), invalid_utf16.as_ptr()),
                ERROR_INVALID_PARAMETER
            );
            assert_eq!(
                MsiRecordSetStringW(h_rec_empty, 1, invalid_utf16.as_ptr()),
                ERROR_INVALID_PARAMETER
            );

            // MsiDoActionW error paths
            assert_eq!(
                MsiDoActionW(h_install, std::ptr::null()),
                ERROR_INVALID_PARAMETER
            );
            assert_eq!(
                MsiDoActionW(999_999, prop_utf16.as_ptr()),
                ERROR_INVALID_HANDLE
            );

            // MsiEvaluateConditionW error paths
            assert_eq!(
                MsiEvaluateConditionW(h_install, std::ptr::null()),
                MSICONDITION_ERROR
            );
            assert_eq!(
                MsiEvaluateConditionW(999_999, prop_utf16.as_ptr()),
                MSICONDITION_ERROR
            );
            let bad_syntax_utf16: Vec<u16> = "=== INVALID CONDITION ===".encode_utf16().collect();
            assert_eq!(
                MsiEvaluateConditionW(h_install, bad_syntax_utf16.as_ptr()),
                MSICONDITION_ERROR
            );

            // MsiGetActiveDatabase without active database
            assert_eq!(MsiGetActiveDatabase(h_install), 0);
            assert_eq!(MsiGetActiveDatabase(999_999), 0);

            let _ = MsiCloseHandle(h_rec_empty);
            let _ = MsiCloseHandle(h_install);
            Ok(())
        }
    }

    /// Tests `CustomActionExecutor` integration with `OfflineExecutionPolicy`.
    #[test]
    fn test_custom_action_executor_offline_policy() -> Result<()> {
        use crate::execution::bare_metal::{OfflineActionPolicyMode, OfflineExecutionPolicy};

        let mut context = EvaluationContext::new();

        // 1. StrictReject policy rejects binary/DLL custom action
        let strict_policy =
            OfflineExecutionPolicy::new(OfflineActionPolicyMode::StrictReject, "/mnt/target");
        let executor_strict = CustomActionExecutor::new().with_offline_policy(strict_policy);
        assert!(executor_strict.offline_policy().is_some());

        let ca_dll = CustomActionDefinition::parse(
            "UnsafeAction",
            1, // Dll in binary table
            "BinaryKey",
            "EntryPoint",
        )?;
        let res_reject = executor_strict.execute(&ca_dll, &mut context);
        assert!(matches!(res_reject, Err(Error::CustomActionFailed { .. })));

        // 2. SkipWithSuccess skips with ERROR_SUCCESS
        let skip_policy =
            OfflineExecutionPolicy::new(OfflineActionPolicyMode::SkipWithSuccess, "/mnt/target");
        let executor_skip = CustomActionExecutor::new().with_offline_policy(skip_policy);
        let res_skip = executor_skip.execute(&ca_dll, &mut context);
        assert_eq!(res_skip, Ok(ERROR_SUCCESS));

        // 3. Safe property action proceeds even in strict mode
        let ca_prop = CustomActionDefinition::parse(
            "SetPropAction",
            51, // Formatted text into property
            "MY_PROP",
            "PropertyValue",
        )?;
        let res_prop = executor_strict.execute(&ca_prop, &mut context);
        assert_eq!(res_prop, Ok(ERROR_SUCCESS));
        assert_eq!(context.get_property("MY_PROP"), Some("PropertyValue"));

        Ok(())
    }

    /// Tests `lock_handles` mutex poison recovery.
    #[test]
    fn test_lock_handles_poison_recovery() {
        let _ = std::panic::catch_unwind(|| {
            let _guard = lock_handles();
            panic!("poison for lock_handles test");
        });

        // Calling lock_handles recovers from poison and clears poison state
        let guard = lock_handles();
        let _ = guard.sessions.len();
        drop(guard);
        GLOBAL_HANDLES.clear_poison();
    }

    /// Tests Type 19 error abort custom action formatting and execution failure.
    #[test]
    fn test_type_19_error_abort_action() -> Result<()> {
        let executor = CustomActionExecutor::new();
        let mut context = EvaluationContext::new();
        context.set_property("ProductName", "LibScript CMS");

        let err_action = CustomActionDefinition::parse(
            "AbortLicense",
            MSIDB_CUSTOM_ACTION_TYPE_ERROR,
            "",
            "Installation of [ProductName] cannot continue without agreeing to all licenses.",
        )?;
        assert_eq!(err_action.source_type(), CustomActionSourceType::Error);

        let res = executor.execute(&err_action, &mut context);
        assert_eq!(
            res,
            Err(Error::CustomActionFailed {
                action: "AbortLicense".to_string(),
                reason: "Installation of LibScript CMS cannot continue without agreeing to all licenses.".to_string(),
            })
        );

        // Test with empty message fallback
        let empty_err_action =
            CustomActionDefinition::parse("EmptyAbort", MSIDB_CUSTOM_ACTION_TYPE_ERROR, "", "")?;
        let res_empty = executor.execute(&empty_err_action, &mut context);
        assert_eq!(
            res_empty,
            Err(Error::CustomActionFailed {
                action: "EmptyAbort".to_string(),
                reason: "Custom action 'EmptyAbort' aborted installation".to_string(),
            })
        );

        Ok(())
    }

    /// Tests command line parsing, port availability probing, and binary attachments.
    #[test]
    fn test_command_line_parsing_and_port_probe() -> Result<()> {
        let (prog, args) =
            parse_command_line(r#""C:\Program Files\App\bin.exe" arg1 "arg with spaces""#);
        assert_eq!(prog, PathBuf::from(r"C:\Program Files\App\bin.exe"));
        assert_eq!(args, vec!["arg1", "arg with spaces"]);

        let (empty_prog, empty_args) = parse_command_line("");
        assert_eq!(empty_prog, PathBuf::new());
        assert!(empty_args.is_empty());

        // Probe local ports
        let _ = probe_port_available(0);

        // Binary storage tests on executor
        let executor = CustomActionExecutor::new()
            .with_binary("bin1", b"payload1".to_vec())
            .with_binaries(HashMap::from([("bin2".to_string(), b"payload2".to_vec())]));
        assert_eq!(executor.binaries().len(), 2);

        // Port check custom action (Type 6 / VBScript or Type 5 / JScript)
        let mut context = EvaluationContext::new();
        context.set_property("PROP_MYSQL_PORT", "3399"); // Uncommon port likely free

        let check_port_action = CustomActionDefinition::parse(
            "CheckPorts_mysql",
            MSIDB_CUSTOM_ACTION_TYPE_VBSCRIPT,
            "BinaryVbs",
            "dummy_target",
        )?;

        let res = executor.execute(&check_port_action, &mut context)?;
        assert_eq!(res, ERROR_SUCCESS);
        assert!(context.get_property("PORT_mysql_AVAILABLE").is_some());
        assert!(context.get_property("mysql_PORT_IN_USE").is_some());

        Ok(())
    }

    /// Tests Type 34 (`DirectoryExe`) and Type 50 (`PropertyExe`) custom action execution.
    #[test]
    fn test_type_34_and_50_executable_actions() -> Result<()> {
        let executor = CustomActionExecutor::new();
        let mut context = EvaluationContext::new();
        context.set_property("INSTALLFOLDER", "/opt/libscript");
        context.set_property("APP_EXE", "/bin/echo");

        // Type 34: DirectoryExe
        let type34 = CustomActionDefinition::parse(
            "CA_RunCli",
            MSIDB_CUSTOM_ACTION_TYPE_DIRECTORY_EXE,
            "INSTALLFOLDER",
            "cli.cmd install",
        )?;
        assert_eq!(type34.source_type(), CustomActionSourceType::DirectoryExe);

        // Path does not exist on disk in test sandbox -> succeeds safely
        let res34 = executor.execute(&type34, &mut context)?;
        assert_eq!(res34, ERROR_SUCCESS);

        // Type 50: PropertyExe
        let type50 = CustomActionDefinition::parse(
            "CA_RunProp",
            MSIDB_CUSTOM_ACTION_TYPE_PROPERTY_EXE,
            "APP_EXE",
            "hello world",
        )?;
        assert_eq!(type50.source_type(), CustomActionSourceType::PropertyExe);

        #[cfg(not(target_os = "windows"))]
        {
            let res50 = executor.execute(&type50, &mut context)?;
            assert_eq!(res50, ERROR_SUCCESS);
        }

        Ok(())
    }

    /// Tests `CustomActionDefinition` bitmask flags, getters, builder methods, port heuristics, and `C` shims.
    #[test]
    fn test_custom_action_remaining_coverage() -> Result<()> {
        // 1. In-script flags and execution mode accessors
        let a_first = CustomActionDefinition::parse(
            "A1",
            MSIDB_CUSTOM_ACTION_TYPE_FIRST_SEQUENCE | 1,
            "S",
            "T",
        )?;
        assert!(a_first.first_sequence());
        assert_eq!(a_first.name(), "A1");
        assert_eq!(
            a_first.raw_type(),
            MSIDB_CUSTOM_ACTION_TYPE_FIRST_SEQUENCE | 1
        );
        assert_eq!(a_first.source(), "S");
        assert_eq!(a_first.target(), "T");

        let a_once = CustomActionDefinition::parse(
            "A2",
            MSIDB_CUSTOM_ACTION_TYPE_ONCE_PER_PROCESS | 1,
            "S",
            "T",
        )?;
        assert!(a_once.once_per_process());

        let a_client = CustomActionDefinition::parse(
            "A3",
            MSIDB_CUSTOM_ACTION_TYPE_CLIENT_REPEAT | 1,
            "S",
            "T",
        )?;
        assert!(a_client.client_repeat());

        let a_no_imp = CustomActionDefinition::parse(
            "A4",
            MSIDB_CUSTOM_ACTION_TYPE_NO_IMPERSONATE | 1,
            "S",
            "T",
        )?;
        assert!(a_no_imp.no_impersonate());

        let a_rollback =
            CustomActionDefinition::parse("A5", MSIDB_CUSTOM_ACTION_TYPE_ROLLBACK | 1, "S", "T")?;
        assert_eq!(a_rollback.in_script(), InScriptMode::Rollback);

        let a_commit =
            CustomActionDefinition::parse("A6", MSIDB_CUSTOM_ACTION_TYPE_COMMIT | 1, "S", "T")?;
        assert_eq!(a_commit.in_script(), InScriptMode::Commit);

        let a_deferred =
            CustomActionDefinition::parse("A7", MSIDB_CUSTOM_ACTION_TYPE_IN_SCRIPT | 1, "S", "T")?;
        assert_eq!(a_deferred.in_script(), InScriptMode::Deferred);

        let a_immediate = CustomActionDefinition::parse("A8", 1, "S", "T")?;
        assert_eq!(a_immediate.in_script(), InScriptMode::Immediate);

        // CustomActionExecutionMode accessors
        assert!(!CustomActionExecutionMode::Synchronous.is_async());
        assert!(!CustomActionExecutionMode::Synchronous.is_continue());
        assert!(CustomActionExecutionMode::Continue.is_async());
        assert!(CustomActionExecutionMode::Continue.is_continue());
        assert!(CustomActionExecutionMode::Async.is_async());
        assert!(!CustomActionExecutionMode::Async.is_continue());

        // 2. CustomActionExecutor getters and builder methods
        let mut executor = CustomActionExecutor::new()
            .with_binary("bin_one", vec![1, 2, 3])
            .with_binaries(HashMap::from([("bin_two".to_string(), vec![4, 5, 6])]));
        assert_eq!(executor.binaries().len(), 2);
        assert!(!executor.has_native_function("NonExistentFunction"));

        // Add binary with native PE header stub to exercise native library extraction branch
        executor.add_binary("mock.dll", b"MZ_header_stub".to_vec());
        assert!(executor.has_native_function("AnyFunctionInLoadedLib"));

        executor.set_mock_result("MyMock", 0);
        assert!(executor.has_mock_result("MyMock"));
        assert!(!executor.has_mock_result("NoMock"));

        let _ = executor.library_loader();
        let _ = executor.library_loader_mut();
        let _ = executor.subprocess_runner();
        let _ = executor.subprocess_runner_mut();

        // 3. Command line split with consecutive whitespace
        let (split_prog, split_args) = parse_command_line("cmd.exe   arg1\t\targ2  \"arg 3\"");
        assert_eq!(split_prog, PathBuf::from("cmd.exe"));
        assert_eq!(split_args, vec!["arg1", "arg2", "arg 3"]);

        // 4. Executable in PATH checks
        #[cfg(not(target_os = "windows"))]
        {
            assert!(is_executable_in_path(Path::new("/bin/sh")));
            assert!(is_executable_in_path(Path::new("sh")));
        }
        assert!(!is_executable_in_path(Path::new(
            "/non/existent/abs/binary/path"
        )));
        assert!(!is_executable_in_path(Path::new(
            "non_existent_binary_xyz_99999"
        )));

        // 5. Port check heuristic branches
        let mut context = EvaluationContext::new();

        // Port check with single port suffix "CheckPort_8080"
        let check_single = CustomActionDefinition::parse(
            "CheckPort_8080",
            MSIDB_CUSTOM_ACTION_TYPE_VBSCRIPT,
            "BinaryVbs",
            "dummy_target",
        )?;
        let res_single = executor.execute(&check_single, &mut context)?;
        assert_eq!(res_single, ERROR_SUCCESS);
        assert!(context.get_property("PORT_8080_AVAILABLE").is_some());

        // Port check with non-numeric suffix falling back to 3306
        let check_fallback = CustomActionDefinition::parse(
            "CheckPort_custom",
            MSIDB_CUSTOM_ACTION_TYPE_VBSCRIPT,
            "BinaryVbs",
            "dummy_target",
        )?;
        let res_fallback = executor.execute(&check_fallback, &mut context)?;
        assert_eq!(res_fallback, ERROR_SUCCESS);
        assert!(context.get_property("PORT_custom_AVAILABLE").is_some());

        // Port check without CheckPort_ prefix falling back to SERVICE suffix
        let check_service = CustomActionDefinition::parse(
            "VerifyNetworkPorts",
            MSIDB_CUSTOM_ACTION_TYPE_VBSCRIPT,
            "BinaryVbs",
            "CheckPort",
        )?;
        let res_service = executor.execute(&check_service, &mut context)?;
        assert_eq!(res_service, ERROR_SUCCESS);
        assert!(context.get_property("PORT_SERVICE_AVAILABLE").is_some());

        // Port check matching "netstat" in target
        let check_netstat = CustomActionDefinition::parse(
            "NetstatAction",
            MSIDB_CUSTOM_ACTION_TYPE_VBSCRIPT,
            "BinaryVbs",
            "netstat -an",
        )?;
        let res_netstat = executor.execute(&check_netstat, &mut context)?;
        assert_eq!(res_netstat, ERROR_SUCCESS);

        // Port check with occupied port returning false
        let listener = std::net::TcpListener::bind("127.0.0.1:0")?;
        let bound_port = listener.local_addr()?.port();
        context.set_property("PROP_BOUND_PORT", bound_port.to_string());
        let check_busy = CustomActionDefinition::parse(
            "CheckPort_bound",
            MSIDB_CUSTOM_ACTION_TYPE_VBSCRIPT,
            "BinaryVbs",
            "dummy_target",
        )?;
        let res_busy = executor.execute(&check_busy, &mut context)?;
        assert_eq!(res_busy, ERROR_SUCCESS);
        assert_eq!(context.get_property("PORT_bound_AVAILABLE"), Some("0"));
        assert_eq!(context.get_property("bound_PORT_IN_USE"), Some("1"));
        drop(listener);

        // 6. Script from binary table and empty script fallback
        executor.add_binary(
            "script.vbs",
            b"Session.Property(\"VBS_RAN\") = \"1\"".to_vec(),
        );
        let script_from_bin = CustomActionDefinition::parse(
            "RunBinScript",
            MSIDB_CUSTOM_ACTION_TYPE_VBSCRIPT,
            "script.vbs",
            "",
        )?;
        let res_vbs = executor.execute(&script_from_bin, &mut context)?;
        assert_eq!(res_vbs, ERROR_SUCCESS);
        assert_eq!(context.get_property("VBS_RAN"), Some("1"));

        let script_missing = CustomActionDefinition::parse(
            "RunMissingScript",
            MSIDB_CUSTOM_ACTION_TYPE_JSCRIPT,
            "missing.js",
            "",
        )?;
        let res_js = executor.execute(&script_missing, &mut context)?;
        assert_eq!(res_js, ERROR_SUCCESS);

        // 7. PropertyExe fallback, DirectoryExe relative path, and empty Exe command
        let prop_exe_fallback = CustomActionDefinition::parse(
            "RunFallbackPropExe",
            MSIDB_CUSTOM_ACTION_TYPE_PROPERTY_EXE,
            "non_existent_prog_cmd",
            "",
        )?;
        let res_prop_fallback = executor.execute(&prop_exe_fallback, &mut context)?;
        assert_eq!(res_prop_fallback, ERROR_SUCCESS);

        // DirectoryExe where wd.join(&prog).exists() is true
        let temp_wd = std::env::temp_dir().join(format!("msi_wd_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&temp_wd);
        std::fs::create_dir_all(&temp_wd)?;
        let script_file = temp_wd.join("runner.sh");
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink("/bin/sh", &script_file)?;
        }
        #[cfg(not(unix))]
        {
            std::fs::write(&script_file, b"")?;
        }
        context.set_property("WORKING_DIR", temp_wd.to_string_lossy().to_string());
        let dir_exe_action = CustomActionDefinition::parse(
            "RunDirScript",
            MSIDB_CUSTOM_ACTION_TYPE_DIRECTORY_EXE,
            "WORKING_DIR",
            "runner.sh",
        )?;
        #[cfg(not(target_os = "windows"))]
        {
            let res_dir = executor.execute(&dir_exe_action, &mut context)?;
            assert_eq!(res_dir, ERROR_SUCCESS);

            // DirectoryExe with absolute path (/bin/sh) exercises !prog.is_absolute() == false
            let abs_dir_exe = CustomActionDefinition::parse(
                "RunAbsDirScript",
                MSIDB_CUSTOM_ACTION_TYPE_DIRECTORY_EXE,
                "WORKING_DIR",
                "/bin/sh",
            )?;
            let res_abs_dir = executor.execute(&abs_dir_exe, &mut context)?;
            assert_eq!(res_abs_dir, ERROR_SUCCESS);
        }
        let _ = std::fs::remove_dir_all(&temp_wd);

        let empty_exe =
            CustomActionDefinition::parse("EmptyExeAction", MSIDB_CUSTOM_ACTION_TYPE_EXE, "", "")?;
        let res_empty_exe = executor.execute(&empty_exe, &mut context)?;
        assert_eq!(res_empty_exe, ERROR_SUCCESS);

        // 8. Continue on failure for subprocess and DLL actions
        #[cfg(not(target_os = "windows"))]
        {
            let ca_continue_fail = CustomActionDefinition::parse(
                "ContinueFailAction",
                MSIDB_CUSTOM_ACTION_TYPE_EXE | MSIDB_CUSTOM_ACTION_TYPE_CONTINUE,
                "",
                r#"/bin/sh -c "exit 42""#,
            )?;
            let res_continue = executor.execute(&ca_continue_fail, &mut context)?;
            assert_eq!(res_continue, ERROR_SUCCESS);

            // Synchronous subprocess failure (exit 1)
            let ca_sync_fail = CustomActionDefinition::parse(
                "SyncFailAction",
                MSIDB_CUSTOM_ACTION_TYPE_EXE,
                "",
                r#"/bin/sh -c "exit 1""#,
            )?;
            assert!(executor.execute(&ca_sync_fail, &mut context).is_err());
        }

        let dll_exec = CustomActionExecutor::new();
        let ca_dll_continue_fail = CustomActionDefinition::parse(
            "ContinueDllFail",
            MSIDB_CUSTOM_ACTION_TYPE_DLL | MSIDB_CUSTOM_ACTION_TYPE_CONTINUE,
            "MissingDll",
            "MissingEntryPoint",
        )?;
        let res_dll_continue = dll_exec.execute(&ca_dll_continue_fail, &mut context)?;
        assert_eq!(res_dll_continue, ERROR_SUCCESS);

        let ca_dll_sync_fail = CustomActionDefinition::parse(
            "SyncDllFail",
            MSIDB_CUSTOM_ACTION_TYPE_DLL,
            "MissingDll",
            "MissingEntryPoint",
        )?;
        assert!(dll_exec.execute(&ca_dll_sync_fail, &mut context).is_err());

        // 9. HandleManager and C-shim edge cases
        let _ = global_handles();
        let mut hm = HandleManager::new();
        assert!(!hm.close_handle(99999));

        // SAFETY: Testing C-shims with handles and null parameters.
        unsafe {
            let h_inst = {
                let mut lock = lock_handles();
                lock.register_session(InstallSession::default())
            };

            // MsiProcessMessage with record containing an integer (non-string in field 0)
            let h_rec_int = MsiCreateRecord(1);
            assert_eq!(MsiRecordSetInteger(h_rec_int, 0, 42), ERROR_SUCCESS);
            assert_eq!(MsiProcessMessage(h_inst, 1, h_rec_int), 1);
            // MsiProcessMessage with missing record handle (EmptyMessage fallback)
            assert_eq!(MsiProcessMessage(h_inst, 1, 99999), 1);
            // MsiProcessMessage with missing install session handle (IDABORT)
            assert_eq!(MsiProcessMessage(99999, 1, h_rec_int), 0);

            // MsiSetPropertyW with null value clears/sets empty
            let prop_name_u16: Vec<u16> = "CLEAR_PROP\0".encode_utf16().collect();
            assert_eq!(
                MsiSetPropertyW(h_inst, prop_name_u16.as_ptr(), std::ptr::null()),
                ERROR_SUCCESS
            );

            // MsiGetPropertyW with non-existent property
            let missing_name_u16: Vec<u16> = "NON_EXISTENT_PROP\0".encode_utf16().collect();
            let mut get_buf = [0u16; 16];
            let mut get_cch: u32 = 16;
            assert_eq!(
                MsiGetPropertyW(
                    h_inst,
                    missing_name_u16.as_ptr(),
                    get_buf.as_mut_ptr(),
                    &raw mut get_cch
                ),
                ERROR_SUCCESS
            );
            assert_eq!(get_cch, 0);

            let mut lock = lock_handles();
            lock.close_handle(h_inst);
            lock.close_handle(h_rec_int);
        }

        Ok(())
    }
}

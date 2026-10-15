//! Custom Action bitmask constants, source types, execution modes, and definition model.

use crate::error::{MsiError, Result};

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
    /// Returns [`MsiError::InvalidArgument`] if the source type bits are unrecognized.
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
            other => Err(MsiError::InvalidArgument {
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
    pub(crate) source_type: CustomActionSourceType,
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
    /// Returns [`crate::MsiError`] if the type bits are invalid.
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

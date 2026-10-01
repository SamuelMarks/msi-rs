//! Command-line interface argument parser providing complete parity with `msiexec.exe`.
//!
//! Grounded directly in official Microsoft Windows Installer Command-Line Options:
//! - Action modes: `/i`, `/x`, `/a`, `/f` (with repair flags), `/j` (advertise), `/p` (patch).
//! - UI levels: `/qn`, `/qb`, `/qb!`, `/qr`, `/qf`.
//! - Logging options: `/l` (flags `i,w,e,a,r,u,c,m,o,p,v,x,+,!`).
//! - Public property overrides: `PROPERTY=Value`.

use crate::error::{Error, Result};
use crate::execution::costing::DiskCostEngine;
use crate::execution::properties::EvaluationContext;
use crate::execution::transaction::{Transaction, WorkerContext};
use crate::package::Package;
use std::collections::HashMap;
use std::path::Path;

/// Standard Windows Installer exit code representation and constants.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum MsiExitCode {
    /// Action completed successfully (`0`).
    Success,
    /// User terminated the installation (`1602`).
    UserExit,
    /// Fatal error occurred during installation (`1603`).
    InstallFailure,
    /// This action is only valid for products that are currently installed (`1605`).
    UnknownProduct,
    /// A restart is required to complete the installation (`3010`).
    SuccessRebootRequired,
    /// Other unmapped Windows Installer exit code.
    Other(u32),
}

/// Standard MSI return code for successful installation (`0`).
pub const ERROR_SUCCESS: u32 = 0;
/// Standard MSI return code when user cancels the operation (`1602`).
pub const ERROR_INSTALL_USEREXIT: u32 = 1602;
/// Standard MSI return code for fatal failure during installation (`1603`).
pub const ERROR_INSTALL_FAILURE: u32 = 1603;
/// Standard MSI return code when product is not currently installed (`1605`).
pub const ERROR_UNKNOWN_PRODUCT: u32 = 1605;
/// Standard MSI return code when operation succeeded but system reboot is required (`3010`).
pub const ERROR_SUCCESS_REBOOT_REQUIRED: u32 = 3010;

impl MsiExitCode {
    /// Converts this [`MsiExitCode`] into its raw numeric Windows Installer return code value (`u32`).
    ///
    /// # Returns
    ///
    /// The numeric exit code (e.g. `0`, `1602`, `1603`, `1605`, `3010`).
    #[must_use]
    pub const fn to_u32(self) -> u32 {
        match self {
            Self::Success => ERROR_SUCCESS,
            Self::UserExit => ERROR_INSTALL_USEREXIT,
            Self::InstallFailure => ERROR_INSTALL_FAILURE,
            Self::UnknownProduct => ERROR_UNKNOWN_PRODUCT,
            Self::SuccessRebootRequired => ERROR_SUCCESS_REBOOT_REQUIRED,
            Self::Other(code) => code,
        }
    }

    /// Converts a raw numeric Windows Installer return code (`u32`) into an [`MsiExitCode`].
    ///
    /// # Arguments
    ///
    /// * `code` - Raw exit code value.
    ///
    /// # Returns
    ///
    /// Corresponding [`MsiExitCode`] variant.
    #[must_use]
    pub const fn from_u32(code: u32) -> Self {
        match code {
            ERROR_SUCCESS => Self::Success,
            ERROR_INSTALL_USEREXIT => Self::UserExit,
            ERROR_INSTALL_FAILURE => Self::InstallFailure,
            ERROR_UNKNOWN_PRODUCT => Self::UnknownProduct,
            ERROR_SUCCESS_REBOOT_REQUIRED => Self::SuccessRebootRequired,
            other => Self::Other(other),
        }
    }
}

impl From<MsiExitCode> for u32 {
    fn from(code: MsiExitCode) -> Self {
        code.to_u32()
    }
}

impl From<u32> for MsiExitCode {
    fn from(code: u32) -> Self {
        Self::from_u32(code)
    }
}

/// Repair mode flags corresponding to `/f[p|o|e|d|c|a|u|m|s|v]`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[allow(clippy::struct_excessive_bools)]
pub struct RepairFlags {
    /// `p`: Reinstall only if file is missing.
    pub reinstall_missing: bool,
    /// `o`: Reinstall if file is missing or older version.
    pub reinstall_older: bool,
    /// `e`: Reinstall if file is missing or equal/older version.
    pub reinstall_equal_or_older: bool,
    /// `d`: Reinstall if file is missing or different version.
    pub reinstall_different: bool,
    /// `c`: Reinstall if file is missing or checksum differs.
    pub reinstall_checksum: bool,
    /// `a`: Reinstall all files regardless of checksum/version.
    pub reinstall_all: bool,
    /// `u`: Rewrite all required user registry entries.
    pub rewrite_user_registry: bool,
    /// `m`: Rewrite all required machine registry entries.
    pub rewrite_machine_registry: bool,
    /// `s`: Overwrite all existing shortcuts.
    pub overwrite_shortcuts: bool,
    /// `v`: Run from source and re-cache local package.
    pub recache_source: bool,
}

impl RepairFlags {
    /// Parses a string of repair flag characters.
    ///
    /// # Arguments
    ///
    /// * `flags` - Character sequence (e.g. `"omus"`).
    ///
    /// # Returns
    ///
    /// Parsed [`RepairFlags`].
    ///
    /// # Errors
    ///
    /// Returns [`Error::InvalidArgument`] if an unrecognized flag character is encountered.
    pub fn parse(flags: &str) -> Result<Self> {
        let mut res = Self::default();
        for ch in flags.chars() {
            match ch.to_ascii_lowercase() {
                'p' => res.reinstall_missing = true,
                'o' => res.reinstall_older = true,
                'e' => res.reinstall_equal_or_older = true,
                'd' => res.reinstall_different = true,
                'c' => res.reinstall_checksum = true,
                'a' => res.reinstall_all = true,
                'u' => res.rewrite_user_registry = true,
                'm' => res.rewrite_machine_registry = true,
                's' => res.overwrite_shortcuts = true,
                'v' => res.recache_source = true,
                other => {
                    return Err(Error::InvalidArgument {
                        argument: format!("/f{other}"),
                        reason: format!("unknown repair flag '{other}'"),
                    });
                }
            }
        }
        Ok(res)
    }
}

impl std::fmt::Display for RepairFlags {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut s = String::new();
        if self.reinstall_missing {
            s.push('p');
        }
        if self.reinstall_older {
            s.push('o');
        }
        if self.reinstall_equal_or_older {
            s.push('e');
        }
        if self.reinstall_different {
            s.push('d');
        }
        if self.reinstall_checksum {
            s.push('c');
        }
        if self.reinstall_all {
            s.push('a');
        }
        if self.rewrite_user_registry {
            s.push('u');
        }
        if self.rewrite_machine_registry {
            s.push('m');
        }
        if self.overwrite_shortcuts {
            s.push('s');
        }
        if self.recache_source {
            s.push('v');
        }
        if s.is_empty() {
            write!(f, "pecmsu")
        } else {
            write!(f, "{s}")
        }
    }
}

/// Target scope for advertisement (`/j[u|m]`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AdvertiseScope {
    /// `/ju`: Advertise to current user.
    User,
    /// `/jm`: Advertise to all users on this machine.
    Machine,
}

/// Primary execution action mode requested.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ActionMode {
    /// `/i <package>`: Install product.
    Install {
        /// Path to the package file.
        package_path: String,
    },
    /// `/x <package>`: Uninstall product.
    Uninstall {
        /// Path or product code of package to uninstall.
        package_path: String,
    },
    /// `/a <package>`: Administrative installation.
    Administrative {
        /// Path to package file.
        package_path: String,
    },
    /// `/f <flags> <package>`: Repair product.
    Repair {
        /// Specific repair flags.
        flags: RepairFlags,
        /// Path to package file.
        package_path: String,
    },
    /// `/j <scope> <package>`: Advertise product.
    Advertise {
        /// Target scope (user or machine).
        scope: AdvertiseScope,
        /// Path to package file.
        package_path: String,
    },
    /// `/p <patch>`: Apply patch package (`.msp`).
    ApplyPatch {
        /// Path to patch file.
        patch_path: String,
    },
}

/// UI display level requested via `/q` options.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum UiLevel {
    /// `/qf`: Full UI (all wizards and dialogs).
    #[default]
    Full,
    /// `/qr`: Reduced UI.
    Reduced,
    /// `/qb`: Basic UI (progress bar only).
    Basic {
        /// Whether Cancel button is disabled (`/qb!`).
        no_cancel: bool,
    },
    /// `/qn`: Quiet / Silent (No UI).
    None,
}

/// Logging mode flags corresponding to `/l[i|w|e|a|r|u|c|m|o|p|v|x|+|!]`.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
#[allow(clippy::struct_excessive_bools)]
pub struct LoggingOptions {
    /// Log file destination path.
    pub log_file: String,
    /// `i`: Status messages.
    pub status: bool,
    /// `w`: Non-fatal warnings.
    pub warnings: bool,
    /// `e`: All error messages.
    pub errors: bool,
    /// `a`: Action starts.
    pub action_starts: bool,
    /// `r`: Action-specific records.
    pub action_records: bool,
    /// `u`: User requests.
    pub user_requests: bool,
    /// `c`: Initial UI parameters.
    pub ui_parameters: bool,
    /// `m`: Out of memory / fatal exit.
    pub out_of_memory: bool,
    /// `o`: Out of disk space messages.
    pub out_of_disk: bool,
    /// `p`: Terminal properties.
    pub terminal_props: bool,
    /// `v`: Verbose output.
    pub verbose: bool,
    /// `x`: Extra debugging information.
    pub extra_debugging: bool,
    /// `+`: Append to existing log.
    pub append: bool,
    /// `!`: Flush each line immediately to disk.
    pub flush_immediately: bool,
}

impl LoggingOptions {
    /// Parses logging flags and log file path from argument pair.
    ///
    /// # Arguments
    ///
    /// * `flags` - Flag characters following `/l` (e.g. `"iwearm+!"` or `"*v"`).
    /// * `log_file` - Destination file path.
    ///
    /// # Returns
    ///
    /// Parsed [`LoggingOptions`].
    ///
    /// # Errors
    ///
    /// Returns [`Error::InvalidArgument`] if an unrecognized logging flag is encountered.
    pub fn parse<F: AsRef<str>, S: Into<String>>(flags: F, log_file: S) -> Result<Self> {
        Self::parse_impl(flags.as_ref(), log_file.into())
    }

    /// Internal non-generic logging options parser.
    fn parse_impl(flags: &str, log_file: String) -> Result<Self> {
        let mut opts = Self {
            log_file,
            ..Self::default()
        };

        for ch in flags.chars() {
            match ch {
                'i' => opts.status = true,
                'w' => opts.warnings = true,
                'e' => opts.errors = true,
                'a' => opts.action_starts = true,
                'r' => opts.action_records = true,
                'u' => opts.user_requests = true,
                'c' => opts.ui_parameters = true,
                'm' => opts.out_of_memory = true,
                'o' => opts.out_of_disk = true,
                'p' => opts.terminal_props = true,
                'v' => opts.verbose = true,
                'x' => opts.extra_debugging = true,
                '+' => opts.append = true,
                '!' => opts.flush_immediately = true,
                '*' => {
                    // Wildcard: all status flags
                    opts.status = true;
                    opts.warnings = true;
                    opts.errors = true;
                    opts.action_starts = true;
                    opts.action_records = true;
                    opts.user_requests = true;
                    opts.ui_parameters = true;
                    opts.out_of_memory = true;
                    opts.out_of_disk = true;
                    opts.terminal_props = true;
                }
                other => {
                    return Err(Error::InvalidArgument {
                        argument: format!("/l{other}"),
                        reason: format!("unknown logging flag '{other}'"),
                    });
                }
            }
        }

        Ok(opts)
    }
}

/// Fully parsed command-line configuration for `msiexec`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MsiExecOptions {
    /// Action mode to execute.
    pub action: ActionMode,
    /// User interface display level.
    pub ui_level: UiLevel,
    /// Optional logging configuration.
    pub logging: Option<LoggingOptions>,
    /// Public property overrides (`PROPERTY=Value`).
    pub properties: HashMap<String, String>,
    /// Whether interactive terminal user interface (TUI) is requested.
    pub tui: bool,
}

impl MsiExecOptions {
    /// Parses command-line arguments matching `msiexec.exe` syntax.
    ///
    /// # Arguments
    ///
    /// * `args` - Iterator of command line strings (excluding executable name).
    ///
    /// # Returns
    ///
    /// Parsed [`MsiExecOptions`].
    ///
    /// # Errors
    ///
    /// Returns [`Error::InvalidArgument`] if options are missing, conflicting, or malformed.
    pub fn parse<I, S>(args: I) -> Result<Self>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        let arg_list: Vec<String> = args.into_iter().map(|s| s.as_ref().to_string()).collect();
        Self::parse_strings(&arg_list)
    }

    /// Internal parser operating on concrete string slice.
    #[allow(clippy::too_many_lines)]
    fn parse_strings(arg_list: &[String]) -> Result<Self> {
        if arg_list.is_empty() {
            return Err(Error::InvalidArgument {
                argument: "args".to_string(),
                reason: "no arguments provided to msiexec".to_string(),
            });
        }

        let mut action: Option<ActionMode> = None;
        let mut ui_level = UiLevel::Full;
        let mut logging: Option<LoggingOptions> = None;
        let mut properties: HashMap<String, String> = HashMap::new();
        let mut tui = false;

        let mut i = 0;
        while i < arg_list.len() {
            let arg = &arg_list[i];

            if arg.starts_with('/') || arg.starts_with('-') {
                let opt = &arg[1..];
                let opt_lower = opt.to_ascii_lowercase();

                if opt_lower.starts_with('i') {
                    // /i <package>
                    let path = Self::extract_param(opt, "i", arg_list, &mut i)?;
                    action = Some(ActionMode::Install { package_path: path });
                } else if opt_lower.starts_with('x') {
                    // /x <package>
                    let path = Self::extract_param(opt, "x", arg_list, &mut i)?;
                    action = Some(ActionMode::Uninstall { package_path: path });
                } else if opt_lower.starts_with('a') {
                    // /a <package>
                    let path = Self::extract_param(opt, "a", arg_list, &mut i)?;
                    action = Some(ActionMode::Administrative { package_path: path });
                } else if opt_lower.starts_with('f') {
                    // /f[flags] <package>
                    let flag_chars = &opt[1..];
                    let flags = if flag_chars.is_empty() {
                        RepairFlags {
                            reinstall_older: true,
                            ..RepairFlags::default()
                        }
                    } else {
                        RepairFlags::parse(flag_chars)?
                    };
                    i += 1;
                    if i >= arg_list.len() {
                        return Err(Error::InvalidArgument {
                            argument: arg.clone(),
                            reason: "missing package path for repair".to_string(),
                        });
                    }
                    action = Some(ActionMode::Repair {
                        flags,
                        package_path: arg_list[i].clone(),
                    });
                } else if opt_lower.starts_with('j') {
                    // /j[u|m] <package>
                    let scope = match opt_lower.chars().nth(1) {
                        Some('u') => AdvertiseScope::User,
                        Some('m') | None => AdvertiseScope::Machine,
                        Some(other) => {
                            return Err(Error::InvalidArgument {
                                argument: arg.clone(),
                                reason: format!("unknown advertise scope '{other}'"),
                            });
                        }
                    };
                    i += 1;
                    if i >= arg_list.len() {
                        return Err(Error::InvalidArgument {
                            argument: arg.clone(),
                            reason: "missing package path for advertise".to_string(),
                        });
                    }
                    action = Some(ActionMode::Advertise {
                        scope,
                        package_path: arg_list[i].clone(),
                    });
                } else if opt_lower.starts_with('p') {
                    // /p <patch>
                    let path = Self::extract_param(opt, "p", arg_list, &mut i)?;
                    action = Some(ActionMode::ApplyPatch { patch_path: path });
                } else if opt_lower == "tui"
                    || opt_lower == "-tui"
                    || opt_lower == "console"
                    || opt_lower == "-console"
                {
                    tui = true;
                } else if opt_lower.starts_with("qn") {
                    ui_level = UiLevel::None;
                } else if opt_lower.starts_with("qb!") {
                    ui_level = UiLevel::Basic { no_cancel: true };
                } else if opt_lower.starts_with("qb") {
                    ui_level = UiLevel::Basic { no_cancel: false };
                } else if opt_lower.starts_with("qr") {
                    ui_level = UiLevel::Reduced;
                } else if opt_lower.starts_with("qf") || opt_lower == "q" {
                    ui_level = UiLevel::Full;
                } else if opt_lower.starts_with('l') {
                    // /l[flags] <logfile>
                    let flag_chars = &opt[1..];
                    i += 1;
                    if i >= arg_list.len() {
                        return Err(Error::InvalidArgument {
                            argument: arg.clone(),
                            reason: "missing log file path".to_string(),
                        });
                    }
                    logging = Some(LoggingOptions::parse(flag_chars, &arg_list[i])?);
                } else {
                    return Err(Error::InvalidArgument {
                        argument: arg.clone(),
                        reason: format!("unrecognized command-line option '{arg}'"),
                    });
                }
            } else if let Some((key, val)) = arg.split_once('=') {
                // Public property override: PROPERTY=Value
                properties.insert(key.to_string(), val.to_string());
            } else {
                return Err(Error::InvalidArgument {
                    argument: arg.clone(),
                    reason: format!("unexpected argument '{arg}'"),
                });
            }

            i += 1;
        }

        let Some(act) = action else {
            return Err(Error::InvalidArgument {
                argument: "action".to_string(),
                reason: "no action mode specified (/i, /x, /a, /f, /j, /p)".to_string(),
            });
        };

        Ok(Self {
            action: act,
            ui_level,
            logging,
            properties,
            tui,
        })
    }

    /// Helper to extract option parameter either attached or in the next argument.
    fn extract_param(
        opt: &str,
        prefix: &str,
        arg_list: &[String],
        i: &mut usize,
    ) -> Result<String> {
        let rest = &opt[prefix.len()..];
        if rest.is_empty() {
            *i += 1;
            if *i >= arg_list.len() {
                return Err(Error::InvalidArgument {
                    argument: format!("/{prefix}"),
                    reason: format!("missing argument following /{prefix}"),
                });
            }
            Ok(arg_list[*i].clone())
        } else {
            Ok(rest.to_string())
        }
    }

    /// Executes the parsed `msiexec` configuration with optional I/O stream overrides for interactive TUI.
    ///
    /// # Arguments
    ///
    /// * `input` - Optional custom input stream for terminal events.
    /// * `output` - Optional custom output stream for terminal rendering.
    ///
    /// # Returns
    ///
    /// Process exit code indicating completion status.
    ///
    /// # Errors
    ///
    /// Returns [`Error`] on fatal filesystem, database, or transaction execution failure.
    #[allow(clippy::too_many_lines)]
    pub fn execute_with_streams(
        &self,
        mut input: Option<&mut dyn std::io::Read>,
        mut output: Option<&mut dyn std::io::Write>,
    ) -> Result<MsiExitCode> {
        let package_path = match &self.action {
            ActionMode::Install { package_path }
            | ActionMode::Uninstall { package_path }
            | ActionMode::Administrative { package_path }
            | ActionMode::Repair { package_path, .. }
            | ActionMode::Advertise { package_path, .. } => package_path.clone(),
            ActionMode::ApplyPatch { patch_path } => {
                match self
                    .properties
                    .get("PACKAGE")
                    .or_else(|| self.properties.get("TARGETPACKAGE"))
                {
                    Some(target) => target.clone(),
                    None => {
                        return Err(Error::InvalidArgument {
                            argument: patch_path.clone(),
                            reason:
                                "patch application requires target package path via PACKAGE=path"
                                    .to_string(),
                        });
                    }
                }
            }
        };

        if package_path.trim().is_empty() {
            return Err(Error::InvalidArgument {
                argument: "package".to_string(),
                reason: "package path cannot be empty".to_string(),
            });
        }

        let path = Path::new(&package_path);
        if !path.exists() {
            if package_path.starts_with('{') {
                return Ok(MsiExitCode::UnknownProduct);
            }
            return Err(Error::Io(format!(
                "Package file not found: '{package_path}'"
            )));
        }

        let pkg = Package::open(path)?;
        let mut context = EvaluationContext::new();

        for rec in pkg.database().get_records("Property") {
            if let Ok(prop) = crate::database::tables::core::PropertyRow::from_record(rec) {
                context.set_property(prop.property.as_str(), &prop.value);
            }
        }

        for (k, v) in &self.properties {
            context.set_property(k, v);
        }

        context.set_property("UILevel", format!("{:?}", self.ui_level));

        match &self.action {
            ActionMode::Install { .. } => {
                context.set_property("ACTION", "INSTALL");
            }
            ActionMode::Uninstall { .. } => {
                context.set_property("ACTION", "UNINSTALL");
                context.set_property("REMOVE", "ALL");
            }
            ActionMode::Administrative { .. } => {
                context.set_property("ACTION", "ADMIN");
            }
            ActionMode::Repair { flags, .. } => {
                context.set_property("ACTION", "REPAIR");
                context.set_property("REINSTALL", "ALL");
                context.set_property("REINSTALLMODE", flags.to_string());
            }
            ActionMode::Advertise { scope, .. } => {
                context.set_property("ACTION", "ADVERTISE");
                let scope_val = match scope {
                    AdvertiseScope::User => "u",
                    AdvertiseScope::Machine => "m",
                };
                context.set_property("ADVERTISE", scope_val);
            }
            ActionMode::ApplyPatch { patch_path } => {
                context.set_property("ACTION", "PATCH");
                context.set_property("PATCH", patch_path);
            }
        }

        let is_headless =
            std::env::var("DISPLAY").is_err() && std::env::var("WAYLAND_DISPLAY").is_err();
        let use_tui = self.tui || (is_headless && self.ui_level != UiLevel::None);
        if use_tui {
            let mut engine = crate::ui::UiEngine::new(context.clone());
            let _ = engine.load_from_database(pkg.database());
            if engine.active_dialog().is_some() {
                let mut wizard = crate::ui::TerminalWizard::new(engine);
                wizard.set_action_text(format!("Installing {}...", pkg.metadata().product_name()));
                if let (Some(ref mut inp), Some(ref mut out)) = (input.as_mut(), output.as_mut()) {
                    match wizard.run_event_stream(inp, out, 80, 24) {
                        Ok(crate::ui::DialogReturnCode::Exit) => {
                            return Ok(MsiExitCode::UserExit);
                        }
                        Ok(_) => {
                            context = wizard.engine().context().clone();
                        }
                        Err(err) => return Err(err),
                    }
                } else {
                    let mut guard = crate::ui::TerminalSafetyGuard::new();
                    let _frame = wizard.render_frame(80, 24);
                    guard.disarm();
                }
            }
        }

        let cost_engine = DiskCostEngine::new();
        let tx = Transaction::from_package(&pkg, context.clone(), cost_engine);
        let prep_res = tx.prepare();
        let prep_tx = match prep_res {
            Ok(p) => p,
            Err(err) => {
                if let Some(ref log_opts) = self.logging {
                    let dummy_worker = WorkerContext::new();
                    let _ = write_execution_log(log_opts, &context, &dummy_worker, Some(&err));
                }
                return match &err {
                    Error::CustomActionFailed { .. } => Ok(MsiExitCode::InstallFailure),
                    _ => Err(err),
                };
            }
        };

        let mut worker = WorkerContext::new();
        let exec_res = prep_tx.execute(&mut worker);

        if let Some(ref log_opts) = self.logging {
            let _ = write_execution_log(
                log_opts,
                worker.evaluation_context(),
                &worker,
                exec_res.as_ref().err(),
            );
        }

        exec_res.map_or(Ok(MsiExitCode::InstallFailure), |exec_tx| {
            let _ = exec_tx.commit(&mut worker);
            Ok(MsiExitCode::Success)
        })
    }

    /// Executes the parsed `msiexec` configuration against the target package or product.
    ///
    /// # Returns
    ///
    /// Process exit code indicating completion status.
    ///
    /// # Errors
    ///
    /// Returns [`Error`] on fatal filesystem, database, or transaction execution failure.
    pub fn execute(&self) -> Result<MsiExitCode> {
        self.execute_with_streams(None, None)
    }
}

/// Writes execution details, actions, and properties to the requested log file.
///
/// # Arguments
///
/// * `opts` - Logging options controlling verbosity and targets.
/// * `context` - Active evaluation context.
/// * `worker` - Worker context containing executed actions.
/// * `err` - Optional error encountered during execution.
///
/// # Returns
///
/// `Ok(())` on success, or [`std::io::Error`] on failure.
fn write_execution_log(
    opts: &LoggingOptions,
    context: &EvaluationContext,
    worker: &WorkerContext,
    err: Option<&Error>,
) -> std::io::Result<()> {
    use std::fs::OpenOptions;
    use std::io::Write;

    let mut file = OpenOptions::new()
        .create(true)
        .write(true)
        .append(opts.append)
        .truncate(!opts.append)
        .open(&opts.log_file)?;

    let _ = writeln!(file, "=== Verbose logging started ===");

    if opts.verbose {
        let _ = writeln!(file, "MSI (c): Initial Property Values:");
        for (k, v) in context.properties() {
            let masked = context.mask_log_string(&format!("{k}={v}"));
            let _ = writeln!(file, "    {masked}");
        }
    }

    if opts.action_starts || opts.action_records || opts.status || opts.verbose {
        for action in worker.executed_actions() {
            let _ = writeln!(file, "MSI (s): Action: {action}");
            if opts.flush_immediately {
                let _ = file.flush();
            }
        }

        for log in worker.custom_action_executor().execution_logs() {
            let _ = writeln!(file, "MSI (s): {log}");
            if opts.flush_immediately {
                let _ = file.flush();
            }
        }
    }

    if let Some(e) = err {
        if opts.errors || opts.verbose {
            let _ = writeln!(file, "MSI (s): Execution Error: {e}");
        }
    }

    if opts.terminal_props || opts.verbose {
        let _ = writeln!(file, "=== Property values at termination ===");
        for (k, v) in context.properties() {
            let masked = context.mask_log_string(&format!("{k} = {v}"));
            let _ = writeln!(file, "    Property(S): {masked}");
        }
    }

    let _ = writeln!(file, "=== Logging stopped ===");
    let _ = file.flush();
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::database::{FieldValue, Record};

    /// Tests parsing `/i` install action mode, UI level, and public property parameters.
    #[test]
    fn test_parse_install() {
        let opts =
            MsiExecOptions::parse(["/i", "setup.msi", "TRANSFORMS=custom.mst", "ADDLOCAL=ALL"]);
        assert_eq!(
            opts,
            Ok(MsiExecOptions {
                action: ActionMode::Install {
                    package_path: "setup.msi".to_string(),
                },
                ui_level: UiLevel::Full,
                logging: None,
                properties: [
                    ("TRANSFORMS".to_string(), "custom.mst".to_string()),
                    ("ADDLOCAL".to_string(), "ALL".to_string()),
                ]
                .into_iter()
                .collect(),
                tui: false,
            })
        );
    }

    /// Tests parsing `/x` uninstall action mode and `/qn` silent UI level.
    #[test]
    fn test_parse_uninstall_and_quiet() {
        let opts = MsiExecOptions::parse(["/x", "package.msi", "/qn"]);
        assert_eq!(
            opts,
            Ok(MsiExecOptions {
                action: ActionMode::Uninstall {
                    package_path: "package.msi".to_string(),
                },
                ui_level: UiLevel::None,
                logging: None,
                properties: HashMap::new(),
                tui: false,
            })
        );
    }

    /// Tests parsing `/f` repair action with granular repair flags and UI cancellation flag.
    #[test]
    fn test_parse_repair_flags() {
        let opts = MsiExecOptions::parse(["/fomus", "app.msi", "/qb!"]);
        let expected_flags = RepairFlags {
            reinstall_older: true,
            rewrite_user_registry: true,
            rewrite_machine_registry: true,
            overwrite_shortcuts: true,
            ..RepairFlags::default()
        };
        assert_eq!(
            opts,
            Ok(MsiExecOptions {
                action: ActionMode::Repair {
                    flags: expected_flags,
                    package_path: "app.msi".to_string(),
                },
                ui_level: UiLevel::Basic { no_cancel: true },
                logging: None,
                properties: HashMap::new(),
                tui: false,
            })
        );
    }

    /// Tests parsing `/a` administrative installation and `/j` advertisement modes.
    #[test]
    fn test_parse_admin_and_advertise() {
        let admin_opts = MsiExecOptions::parse(["/a", "admin.msi"]);
        assert_eq!(
            admin_opts,
            Ok(MsiExecOptions {
                action: ActionMode::Administrative {
                    package_path: "admin.msi".to_string(),
                },
                ui_level: UiLevel::Full,
                logging: None,
                properties: HashMap::new(),
                tui: false,
            })
        );

        let adv_opts = MsiExecOptions::parse(["/ju", "user.msi"]);
        assert_eq!(
            adv_opts,
            Ok(MsiExecOptions {
                action: ActionMode::Advertise {
                    scope: AdvertiseScope::User,
                    package_path: "user.msi".to_string(),
                },
                ui_level: UiLevel::Full,
                logging: None,
                properties: HashMap::new(),
                tui: false,
            })
        );

        let patch_opts = MsiExecOptions::parse(["/p", "update.msp"]);
        assert_eq!(
            patch_opts,
            Ok(MsiExecOptions {
                action: ActionMode::ApplyPatch {
                    patch_path: "update.msp".to_string(),
                },
                ui_level: UiLevel::Full,
                logging: None,
                properties: HashMap::new(),
                tui: false,
            })
        );
    }

    /// Tests parsing `/l` logging options, individual flags, and wildcards.
    #[test]
    fn test_parse_logging() {
        let opts = MsiExecOptions::parse(["/i", "pkg.msi", "/lv*+!", "install.log"]);
        let expected_log = LoggingOptions {
            log_file: "install.log".to_string(),
            status: true,
            warnings: true,
            errors: true,
            action_starts: true,
            action_records: true,
            user_requests: true,
            ui_parameters: true,
            out_of_memory: true,
            out_of_disk: true,
            terminal_props: true,
            verbose: true,
            extra_debugging: false,
            append: true,
            flush_immediately: true,
        };
        assert_eq!(
            opts,
            Ok(MsiExecOptions {
                action: ActionMode::Install {
                    package_path: "pkg.msi".to_string(),
                },
                ui_level: UiLevel::Full,
                logging: Some(expected_log),
                properties: HashMap::new(),
                tui: false,
            })
        );

        // Test all individual logging flags
        let all_log = LoggingOptions::parse("iwearucmopvx", "all.log");
        assert_eq!(
            all_log,
            Ok(LoggingOptions {
                log_file: "all.log".to_string(),
                status: true,
                warnings: true,
                errors: true,
                action_starts: true,
                action_records: true,
                user_requests: true,
                ui_parameters: true,
                out_of_memory: true,
                out_of_disk: true,
                terminal_props: true,
                verbose: true,
                extra_debugging: true,
                append: false,
                flush_immediately: false,
            })
        );

        // Uppercase /L flag
        let opts_upper_l = MsiExecOptions::parse(["/I", "pkg.msi", "/L*", "out.log"]);
        assert!(matches!(
            opts_upper_l,
            Ok(MsiExecOptions {
                logging: Some(_),
                ..
            })
        ));
    }

    /// Tests parsing attached parameters such as `-ipackage.msi`.
    #[test]
    fn test_parse_attached_params() {
        let parsed_i = MsiExecOptions::parse(["-ipackage.msi", "/qr"]);
        assert_eq!(
            parsed_i,
            Ok(MsiExecOptions {
                action: ActionMode::Install {
                    package_path: "package.msi".to_string(),
                },
                ui_level: UiLevel::Reduced,
                logging: None,
                properties: HashMap::new(),
                tui: false,
            })
        );

        let parsed_x = MsiExecOptions::parse(["-xpackage.msi", "/qb"]);
        assert_eq!(
            parsed_x,
            Ok(MsiExecOptions {
                action: ActionMode::Uninstall {
                    package_path: "package.msi".to_string(),
                },
                ui_level: UiLevel::Basic { no_cancel: false },
                logging: None,
                properties: HashMap::new(),
                tui: false,
            })
        );

        let parsed_a = MsiExecOptions::parse(["-apackage.msi", "/qf"]);
        assert_eq!(
            parsed_a,
            Ok(MsiExecOptions {
                action: ActionMode::Administrative {
                    package_path: "package.msi".to_string(),
                },
                ui_level: UiLevel::Full,
                logging: None,
                properties: HashMap::new(),
                tui: false,
            })
        );

        let parsed_p = MsiExecOptions::parse(["-ppatch.msp", "/q"]);
        assert_eq!(
            parsed_p,
            Ok(MsiExecOptions {
                action: ActionMode::ApplyPatch {
                    patch_path: "patch.msp".to_string(),
                },
                ui_level: UiLevel::Full,
                logging: None,
                properties: HashMap::new(),
                tui: false,
            })
        );
    }

    /// Tests uppercase action and UI flags and repair combinations.
    #[test]
    #[allow(clippy::too_many_lines)]
    fn test_parse_uppercase_and_repair() {
        // Uppercase UI flags
        assert_eq!(
            MsiExecOptions::parse(["/I", "package.msi", "/QR"]),
            Ok(MsiExecOptions {
                action: ActionMode::Install {
                    package_path: "package.msi".to_string(),
                },
                ui_level: UiLevel::Reduced,
                logging: None,
                properties: HashMap::new(),
                tui: false,
            })
        );
        assert_eq!(
            MsiExecOptions::parse(["/I", "package.msi", "/QN"]),
            Ok(MsiExecOptions {
                action: ActionMode::Install {
                    package_path: "package.msi".to_string(),
                },
                ui_level: UiLevel::None,
                logging: None,
                properties: HashMap::new(),
                tui: false,
            })
        );
        assert_eq!(
            MsiExecOptions::parse(["/I", "package.msi", "/QB!"]),
            Ok(MsiExecOptions {
                action: ActionMode::Install {
                    package_path: "package.msi".to_string(),
                },
                ui_level: UiLevel::Basic { no_cancel: true },
                logging: None,
                properties: HashMap::new(),
                tui: false,
            })
        );
        assert_eq!(
            MsiExecOptions::parse(["/I", "package.msi", "/QB"]),
            Ok(MsiExecOptions {
                action: ActionMode::Install {
                    package_path: "package.msi".to_string(),
                },
                ui_level: UiLevel::Basic { no_cancel: false },
                logging: None,
                properties: HashMap::new(),
                tui: false,
            })
        );
        assert_eq!(
            MsiExecOptions::parse(["/I", "package.msi", "/QF"]),
            Ok(MsiExecOptions {
                action: ActionMode::Install {
                    package_path: "package.msi".to_string(),
                },
                ui_level: UiLevel::Full,
                logging: None,
                properties: HashMap::new(),
                tui: false,
            })
        );
        assert_eq!(
            MsiExecOptions::parse(["/I", "package.msi", "/Q"]),
            Ok(MsiExecOptions {
                action: ActionMode::Install {
                    package_path: "package.msi".to_string(),
                },
                ui_level: UiLevel::Full,
                logging: None,
                properties: HashMap::new(),
                tui: false,
            })
        );

        // Uppercase action switches
        assert!(matches!(
            MsiExecOptions::parse(["/X", "package.msi"]),
            Ok(MsiExecOptions {
                action: ActionMode::Uninstall { .. },
                ..
            })
        ));
        assert!(matches!(
            MsiExecOptions::parse(["/A", "package.msi"]),
            Ok(MsiExecOptions {
                action: ActionMode::Administrative { .. },
                ..
            })
        ));
        assert!(matches!(
            MsiExecOptions::parse(["/P", "patch.msp"]),
            Ok(MsiExecOptions {
                action: ActionMode::ApplyPatch { .. },
                ..
            })
        ));

        // Advertise scopes
        assert_eq!(
            MsiExecOptions::parse(["/JU", "adv.msi"]),
            Ok(MsiExecOptions {
                action: ActionMode::Advertise {
                    scope: AdvertiseScope::User,
                    package_path: "adv.msi".to_string(),
                },
                ui_level: UiLevel::Full,
                logging: None,
                properties: HashMap::new(),
                tui: false,
            })
        );
        assert_eq!(
            MsiExecOptions::parse(["/j", "adv.msi"]),
            Ok(MsiExecOptions {
                action: ActionMode::Advertise {
                    scope: AdvertiseScope::Machine,
                    package_path: "adv.msi".to_string(),
                },
                ui_level: UiLevel::Full,
                logging: None,
                properties: HashMap::new(),
                tui: false,
            })
        );
        assert_eq!(
            MsiExecOptions::parse(["/jm", "adv.msi"]),
            Ok(MsiExecOptions {
                action: ActionMode::Advertise {
                    scope: AdvertiseScope::Machine,
                    package_path: "adv.msi".to_string(),
                },
                ui_level: UiLevel::Full,
                logging: None,
                properties: HashMap::new(),
                tui: false,
            })
        );

        // All repair flags
        let all_flags = RepairFlags::parse("poedcaumsv");
        assert_eq!(
            all_flags,
            Ok(RepairFlags {
                reinstall_missing: true,
                reinstall_older: true,
                reinstall_equal_or_older: true,
                reinstall_different: true,
                reinstall_checksum: true,
                reinstall_all: true,
                rewrite_user_registry: true,
                rewrite_machine_registry: true,
                overwrite_shortcuts: true,
                recache_source: true,
            })
        );

        // Default repair option with no flags attached
        let parsed_repair = MsiExecOptions::parse(["/f", "app.msi"]);
        let expected_default_flags = RepairFlags {
            reinstall_older: true,
            ..RepairFlags::default()
        };
        assert_eq!(
            parsed_repair,
            Ok(MsiExecOptions {
                action: ActionMode::Repair {
                    flags: expected_default_flags,
                    package_path: "app.msi".to_string(),
                },
                ui_level: UiLevel::Full,
                logging: None,
                properties: HashMap::new(),
                tui: false,
            })
        );

        // Uppercase /F
        assert!(matches!(
            MsiExecOptions::parse(["/FOMUS", "app.msi"]),
            Ok(MsiExecOptions {
                action: ActionMode::Repair { .. },
                ..
            })
        ));
    }

    /// Tests invalid command-line inputs and parsing error conditions.
    #[test]
    fn test_parse_errors() {
        assert!(MsiExecOptions::parse::<[&str; 0], &str>([]).is_err());
        assert!(MsiExecOptions::parse(["/i"]).is_err());
        assert!(MsiExecOptions::parse(["/x"]).is_err());
        assert!(MsiExecOptions::parse(["/a"]).is_err());
        assert!(MsiExecOptions::parse(["/p"]).is_err());
        assert!(MsiExecOptions::parse(["/j"]).is_err());
        assert!(MsiExecOptions::parse(["/ju"]).is_err());
        assert!(MsiExecOptions::parse(["/jm"]).is_err());
        assert!(MsiExecOptions::parse(["/f"]).is_err());
        assert!(MsiExecOptions::parse(["/fomus"]).is_err());
        assert!(MsiExecOptions::parse(["/l"]).is_err());
        assert!(MsiExecOptions::parse(["/lv"]).is_err());
        assert!(MsiExecOptions::parse(["/unknown"]).is_err());
        assert!(MsiExecOptions::parse(["/fz", "pkg.msi"]).is_err());
        assert!(MsiExecOptions::parse(["/jx", "pkg.msi"]).is_err());
        assert!(MsiExecOptions::parse(["/l?", "log.txt"]).is_err());
        assert!(MsiExecOptions::parse(["invalid_arg_without_equals"]).is_err());
        assert!(MsiExecOptions::parse(["PROPERTY=Val"]).is_err()); // no action
    }

    /// Tests conversion to and from Windows Installer exit codes.
    #[test]
    fn test_msi_exit_code() {
        assert_eq!(MsiExitCode::Success.to_u32(), ERROR_SUCCESS);
        assert_eq!(MsiExitCode::UserExit.to_u32(), ERROR_INSTALL_USEREXIT);
        assert_eq!(MsiExitCode::InstallFailure.to_u32(), ERROR_INSTALL_FAILURE);
        assert_eq!(MsiExitCode::UnknownProduct.to_u32(), ERROR_UNKNOWN_PRODUCT);
        assert_eq!(
            MsiExitCode::SuccessRebootRequired.to_u32(),
            ERROR_SUCCESS_REBOOT_REQUIRED
        );
        assert_eq!(MsiExitCode::Other(9999).to_u32(), 9999);

        assert_eq!(MsiExitCode::from_u32(0), MsiExitCode::Success);
        assert_eq!(MsiExitCode::from_u32(1602), MsiExitCode::UserExit);
        assert_eq!(MsiExitCode::from_u32(1603), MsiExitCode::InstallFailure);
        assert_eq!(MsiExitCode::from_u32(1605), MsiExitCode::UnknownProduct);
        assert_eq!(
            MsiExitCode::from_u32(3010),
            MsiExitCode::SuccessRebootRequired
        );
        assert_eq!(MsiExitCode::from_u32(1234), MsiExitCode::Other(1234));

        let code_u32: u32 = MsiExitCode::InstallFailure.into();
        assert_eq!(code_u32, 1603);
        let from_code: MsiExitCode = 1602.into();
        assert_eq!(from_code, MsiExitCode::UserExit);
    }

    /// Tests parsing terminal user interface flags (`/tui`, `--tui`, `/console`, `--console`).
    #[test]
    fn test_tui_flag_parsing() {
        assert_eq!(
            MsiExecOptions::parse(["/i", "setup.msi", "/tui"]),
            Ok(MsiExecOptions {
                action: ActionMode::Install {
                    package_path: "setup.msi".to_string(),
                },
                ui_level: UiLevel::Full,
                logging: None,
                properties: HashMap::new(),
                tui: true,
            })
        );

        assert_eq!(
            MsiExecOptions::parse(["/i", "setup.msi", "--tui"]),
            Ok(MsiExecOptions {
                action: ActionMode::Install {
                    package_path: "setup.msi".to_string(),
                },
                ui_level: UiLevel::Full,
                logging: None,
                properties: HashMap::new(),
                tui: true,
            })
        );

        assert_eq!(
            MsiExecOptions::parse(["/i", "setup.msi", "/console"]),
            Ok(MsiExecOptions {
                action: ActionMode::Install {
                    package_path: "setup.msi".to_string(),
                },
                ui_level: UiLevel::Full,
                logging: None,
                properties: HashMap::new(),
                tui: true,
            })
        );

        assert_eq!(
            MsiExecOptions::parse(["/i", "setup.msi", "--console"]),
            Ok(MsiExecOptions {
                action: ActionMode::Install {
                    package_path: "setup.msi".to_string(),
                },
                ui_level: UiLevel::Full,
                logging: None,
                properties: HashMap::new(),
                tui: true,
            })
        );

        assert_eq!(
            MsiExecOptions::parse(["/i", "setup.msi", "/qn"]),
            Ok(MsiExecOptions {
                action: ActionMode::Install {
                    package_path: "setup.msi".to_string(),
                },
                ui_level: UiLevel::None,
                logging: None,
                properties: HashMap::new(),
                tui: false,
            })
        );
    }

    /// Tests string formatting and display representation of [`RepairFlags`].
    #[test]
    fn test_repair_flags_display() {
        let all_flags = RepairFlags::parse("poedcaumsv");
        assert_eq!(
            all_flags.as_ref().map(ToString::to_string),
            Ok("poedcaumsv".to_string())
        );

        let default_flags = RepairFlags::default();
        assert_eq!(default_flags.to_string(), "pecmsu");

        let custom_flags = RepairFlags {
            reinstall_missing: true,
            reinstall_older: true,
            ..RepairFlags::default()
        };
        assert_eq!(custom_flags.to_string(), "po");
    }

    /// Tests error scenarios encountered during options execution.
    #[test]
    fn test_execute_error_cases() {
        // Empty package
        let empty_pkg = MsiExecOptions {
            action: ActionMode::Install {
                package_path: "   ".to_string(),
            },
            ui_level: UiLevel::None,
            logging: None,
            properties: HashMap::new(),
            tui: false,
        };
        assert!(empty_pkg.execute().is_err());

        // Nonexistent package
        let non_existent = MsiExecOptions {
            action: ActionMode::Install {
                package_path: "/nonexistent/path/to/app.msi".to_string(),
            },
            ui_level: UiLevel::None,
            logging: None,
            properties: HashMap::new(),
            tui: false,
        };
        assert!(non_existent.execute().is_err());

        // Uninstall with unknown product code
        let unknown_guid = MsiExecOptions {
            action: ActionMode::Uninstall {
                package_path: "{12345678-1234-1234-1234-123456789012}".to_string(),
            },
            ui_level: UiLevel::None,
            logging: None,
            properties: HashMap::new(),
            tui: false,
        };
        assert_eq!(
            unknown_guid.execute().ok(),
            Some(MsiExitCode::UnknownProduct)
        );

        // Patch without package
        let patch_no_pkg = MsiExecOptions {
            action: ActionMode::ApplyPatch {
                patch_path: "update.msp".to_string(),
            },
            ui_level: UiLevel::None,
            logging: None,
            properties: HashMap::new(),
            tui: false,
        };
        assert!(patch_no_pkg.execute().is_err());
    }

    /// Tests successful execution across all action modes, logging, and TUI dialog rendering.
    #[test]
    #[allow(clippy::too_many_lines, clippy::items_after_statements)]
    fn test_execute_success() {
        let temp_dir = std::env::temp_dir().join(format!("msi_exec_test_{}", std::process::id()));
        assert!(std::fs::create_dir_all(&temp_dir).is_ok());
        let msi_path = temp_dir.join("test_app.msi");

        let pkg = Package::builder()
            .product_name("TestExecApp")
            .version(crate::package::ProductVersion::new(1, 0, 0))
            .manufacturer("TestManufacturer")
            .product_code("{12345678-1234-1234-1234-123456789012}")
            .build()
            .unwrap_or_default();
        assert!(pkg.save(&msi_path).is_ok());

        let msi_path_str = msi_path.to_string_lossy().to_string();

        // 1. Install mode
        let opts = MsiExecOptions {
            action: ActionMode::Install {
                package_path: msi_path_str.clone(),
            },
            ui_level: UiLevel::None,
            logging: None,
            properties: std::iter::once(("CUSTOM_PROP".to_string(), "123".to_string())).collect(),
            tui: false,
        };
        assert_eq!(
            MsiExecOptions::parse(["/i", &msi_path_str, "/qn", "CUSTOM_PROP=123"]),
            Ok(opts.clone())
        );
        assert_eq!(opts.execute(), Ok(MsiExitCode::Success));

        // 2. Uninstall mode
        let uninst_opts = MsiExecOptions {
            action: ActionMode::Uninstall {
                package_path: msi_path_str.clone(),
            },
            ui_level: UiLevel::None,
            logging: None,
            properties: HashMap::new(),
            tui: false,
        };
        assert_eq!(
            MsiExecOptions::parse(["/x", &msi_path_str, "/qn"]),
            Ok(uninst_opts.clone())
        );
        assert_eq!(uninst_opts.execute(), Ok(MsiExitCode::Success));

        // 3. Admin mode
        let admin_opts = MsiExecOptions {
            action: ActionMode::Administrative {
                package_path: msi_path_str.clone(),
            },
            ui_level: UiLevel::None,
            logging: None,
            properties: HashMap::new(),
            tui: false,
        };
        assert_eq!(
            MsiExecOptions::parse(["/a", &msi_path_str, "/qn"]),
            Ok(admin_opts.clone())
        );
        assert_eq!(admin_opts.execute(), Ok(MsiExitCode::Success));

        // 4. Repair mode
        let repair_opts = MsiExecOptions {
            action: ActionMode::Repair {
                flags: RepairFlags {
                    reinstall_older: true,
                    ..RepairFlags::default()
                },
                package_path: msi_path_str.clone(),
            },
            ui_level: UiLevel::None,
            logging: None,
            properties: HashMap::new(),
            tui: false,
        };
        assert_eq!(
            MsiExecOptions::parse(["/f", &msi_path_str, "/qn"]),
            Ok(repair_opts.clone())
        );
        assert_eq!(repair_opts.execute(), Ok(MsiExitCode::Success));

        // 5. Advertise User
        let user_adv_opts = MsiExecOptions {
            action: ActionMode::Advertise {
                scope: AdvertiseScope::User,
                package_path: msi_path_str.clone(),
            },
            ui_level: UiLevel::None,
            logging: None,
            properties: HashMap::new(),
            tui: false,
        };
        assert_eq!(
            MsiExecOptions::parse(["/ju", &msi_path_str, "/qn"]),
            Ok(user_adv_opts.clone())
        );
        assert_eq!(user_adv_opts.execute(), Ok(MsiExitCode::Success));

        // 6. Advertise Machine
        let machine_adv_opts = MsiExecOptions {
            action: ActionMode::Advertise {
                scope: AdvertiseScope::Machine,
                package_path: msi_path_str.clone(),
            },
            ui_level: UiLevel::None,
            logging: None,
            properties: HashMap::new(),
            tui: false,
        };
        assert_eq!(
            MsiExecOptions::parse(["/jm", &msi_path_str, "/qn"]),
            Ok(machine_adv_opts.clone())
        );
        assert_eq!(machine_adv_opts.execute(), Ok(MsiExitCode::Success));

        // 7. Apply Patch with PACKAGE=...
        let patch_opts = MsiExecOptions {
            action: ActionMode::ApplyPatch {
                patch_path: "dummy.msp".to_string(),
            },
            ui_level: UiLevel::None,
            logging: None,
            properties: std::iter::once(("PACKAGE".to_string(), msi_path_str.clone())).collect(),
            tui: false,
        };
        assert_eq!(
            MsiExecOptions::parse(["/p", "dummy.msp", &format!("PACKAGE={msi_path_str}"), "/qn"]),
            Ok(patch_opts.clone())
        );
        assert_eq!(patch_opts.execute(), Ok(MsiExitCode::Success));

        // 8. Apply Patch with TARGETPACKAGE=...
        let target_patch_opts = MsiExecOptions {
            action: ActionMode::ApplyPatch {
                patch_path: "dummy.msp".to_string(),
            },
            ui_level: UiLevel::None,
            logging: None,
            properties: std::iter::once(("TARGETPACKAGE".to_string(), msi_path_str.clone()))
                .collect(),
            tui: false,
        };
        assert_eq!(
            MsiExecOptions::parse([
                "/p",
                "dummy.msp",
                &format!("TARGETPACKAGE={msi_path_str}"),
                "/qn"
            ]),
            Ok(target_patch_opts.clone())
        );
        assert_eq!(target_patch_opts.execute(), Ok(MsiExitCode::Success));

        // 9. Property table with non-matching record (empty property identifier)
        let mut pkg_with_extra = Package::builder()
            .product_name("ExtraPropApp")
            .version(crate::package::ProductVersion::new(1, 0, 0))
            .manufacturer("TestManufacturer")
            .product_code("{22345678-1234-1234-1234-123456789012}")
            .build()
            .unwrap_or_default();
        pkg_with_extra.database_mut().add_record(
            "Property",
            Record::with_fields(vec![
                FieldValue::String(String::new()),
                FieldValue::String("EmptyPropValue".to_string()),
            ]),
        );
        let extra_msi_path = temp_dir.join("extra_prop.msi");
        assert!(pkg_with_extra.save(&extra_msi_path).is_ok());
        let extra_opts = MsiExecOptions {
            action: ActionMode::Install {
                package_path: extra_msi_path.to_string_lossy().to_string(),
            },
            ui_level: UiLevel::None,
            logging: None,
            properties: HashMap::new(),
            tui: false,
        };
        assert_eq!(extra_opts.execute(), Ok(MsiExitCode::Success));

        // 10. TUI execution with package that has no UI dialogs
        let tui_no_dlg_opts = MsiExecOptions {
            action: ActionMode::Install {
                package_path: msi_path_str.clone(),
            },
            ui_level: UiLevel::Full,
            logging: None,
            properties: HashMap::new(),
            tui: true,
        };
        assert_eq!(tui_no_dlg_opts.execute(), Ok(MsiExitCode::Success));

        // 11. TUI execution branch with package containing UI dialog
        let wxs_src = r#"
<Wix xmlns="http://schemas.microsoft.com/wix/2006/wi">
    <Product Id="{11111111-2222-3333-4444-555555555555}" Name="UIApp" Version="1.0.0" Manufacturer="Test">
        <Package Description="Test" />
        <Directory Id="TARGETDIR" Name="SourceDir" />
        <UI>
            <Dialog Id="WelcomeDlg" Width="370" Height="270" Title="Welcome">
                <Control Id="Next" Type="PushButton" X="236" Y="243" Width="56" Height="17" Default="yes" Text="Next">
                    <Publish Event="EndDialog" Value="Return">1</Publish>
                </Control>
                <Control Id="Cancel" Type="PushButton" X="300" Y="243" Width="56" Height="17" Cancel="yes" Text="Cancel">
                    <Publish Event="EndDialog" Value="Exit">1</Publish>
                </Control>
            </Dialog>
            <InstallUISequence>
                <Show Dialog="WelcomeDlg" Before="ExecuteAction" />
            </InstallUISequence>
        </UI>
    </Product>
</Wix>
"#;
        let ui_wxs_path = temp_dir.join("ui_app.wxs");
        let ui_msi_path = temp_dir.join("ui_app.msi");
        assert!(std::fs::write(&ui_wxs_path, wxs_src).is_ok());

        let build_opts = crate::wix::toolchain::WixBuildOptions {
            output: Some(ui_msi_path.clone()),
            sources: vec![ui_wxs_path],
            suppress_ice: true,
            ..Default::default()
        };
        assert!(build_opts.execute().is_ok());

        let ui_msi_str = ui_msi_path.to_string_lossy().to_string();
        let tui_opts = MsiExecOptions {
            action: ActionMode::Install {
                package_path: ui_msi_str,
            },
            ui_level: UiLevel::Full,
            logging: None,
            properties: HashMap::new(),
            tui: true,
        };
        assert_eq!(tui_opts.execute(), Ok(MsiExitCode::Success));

        // Interactive stream with Escape -> UserExit (1602)
        let cancel_res = {
            let mut input = std::io::Cursor::new(b"\x1b");
            let mut output = Vec::new();
            tui_opts.execute_with_streams(Some(&mut input), Some(&mut output))
        };
        assert_eq!(cancel_res, Ok(MsiExitCode::UserExit));

        // Interactive stream with Enter -> Success (0)
        let enter_res = {
            let mut input = std::io::Cursor::new(b"\r");
            let mut output = Vec::new();
            tui_opts.execute_with_streams(Some(&mut input), Some(&mut output))
        };
        assert_eq!(enter_res, Ok(MsiExitCode::Success));

        // Stream write error
        struct FailingWriter;
        impl std::io::Write for FailingWriter {
            fn write(&mut self, _buf: &[u8]) -> std::io::Result<usize> {
                Err(std::io::Error::other("mock stream write error"))
            }
            fn flush(&mut self) -> std::io::Result<()> {
                Ok(())
            }
        }
        let stream_err = {
            let mut input = std::io::Cursor::new(b"\r");
            let mut failing_writer = FailingWriter;
            tui_opts.execute_with_streams(Some(&mut input), Some(&mut failing_writer))
        };
        assert!(stream_err.is_err());
        assert!(std::io::Write::flush(&mut FailingWriter).is_ok());

        // Test logging during execute()
        let log_file = temp_dir.join("exec_test.log");
        let full_logging = LoggingOptions {
            log_file: log_file.to_string_lossy().to_string(),
            status: true,
            warnings: true,
            errors: true,
            action_starts: true,
            action_records: true,
            user_requests: true,
            ui_parameters: true,
            out_of_memory: true,
            out_of_disk: true,
            terminal_props: true,
            verbose: true,
            extra_debugging: true,
            append: false,
            flush_immediately: true,
        };
        let log_opts = MsiExecOptions {
            action: ActionMode::Install {
                package_path: msi_path_str.clone(),
            },
            ui_level: UiLevel::None,
            logging: Some(full_logging.clone()),
            properties: HashMap::new(),
            tui: false,
        };
        assert_eq!(log_opts.execute(), Ok(MsiExitCode::Success));
        assert!(log_file.exists());
        let log_text = std::fs::read_to_string(&log_file).unwrap_or_default();
        assert!(log_text.contains("=== Verbose logging started ==="));
        assert!(log_text.contains("=== Logging stopped ==="));

        // Test logging in append mode with error, properties, and actions
        let mut append_logging = full_logging.clone();
        append_logging.append = true;
        let test_err = Error::ExecutionFailed {
            action: "TestAction".to_string(),
            return_code: 1603,
            message: "Simulated test error".to_string(),
        };
        let mut test_ctx = EvaluationContext::new();
        test_ctx.set_property("PROP_FOO", "BAR");
        let mut worker_with_actions = WorkerContext::new();
        worker_with_actions.register_component_client(
            "comp_sample",
            "{11111111-1111-1111-1111-111111111111}",
            None,
        );
        let _ = worker_with_actions.commit();
        worker_with_actions
            .custom_action_executor()
            .log("Custom action detail log line");

        assert!(write_execution_log(
            &append_logging,
            &test_ctx,
            &worker_with_actions,
            Some(&test_err),
        )
        .is_ok());

        // Test logging without flush_immediately
        let mut unbuffered_logging = append_logging.clone();
        unbuffered_logging.flush_immediately = false;
        assert!(
            write_execution_log(&unbuffered_logging, &test_ctx, &worker_with_actions, None,)
                .is_ok()
        );

        // Test logging with all flags disabled and an error present
        let minimal_logging = LoggingOptions {
            log_file: append_logging.log_file,
            status: false,
            warnings: false,
            errors: false,
            action_starts: false,
            action_records: false,
            user_requests: false,
            ui_parameters: false,
            out_of_memory: false,
            out_of_disk: false,
            terminal_props: false,
            verbose: false,
            extra_debugging: false,
            append: true,
            flush_immediately: false,
        };
        assert!(write_execution_log(
            &minimal_logging,
            &test_ctx,
            &worker_with_actions,
            Some(&test_err),
        )
        .is_ok());

        // Test logging with invalid path
        let bad_logging = LoggingOptions {
            log_file: "/nonexistent_dir_xyz123/impossible/test.log".to_string(),
            status: true,
            warnings: true,
            errors: true,
            action_starts: true,
            action_records: true,
            user_requests: true,
            ui_parameters: true,
            out_of_memory: true,
            out_of_disk: true,
            terminal_props: true,
            verbose: true,
            extra_debugging: false,
            append: false,
            flush_immediately: false,
        };
        assert!(write_execution_log(
            &bad_logging,
            &EvaluationContext::new(),
            &WorkerContext::new(),
            None,
        )
        .is_err());

        // 12. DISPLAY and WAYLAND_DISPLAY environment variables
        std::env::set_var("DISPLAY", ":0");
        let opts_display = MsiExecOptions {
            action: ActionMode::Install {
                package_path: msi_path_str.clone(),
            },
            ui_level: UiLevel::Full,
            logging: None,
            properties: HashMap::new(),
            tui: false,
        };
        assert_eq!(opts_display.execute(), Ok(MsiExitCode::Success));
        std::env::remove_var("DISPLAY");

        std::env::set_var("WAYLAND_DISPLAY", "wayland-0");
        let opts_wayland = MsiExecOptions {
            action: ActionMode::Install {
                package_path: msi_path_str,
            },
            ui_level: UiLevel::Full,
            logging: None,
            properties: HashMap::new(),
            tui: false,
        };
        assert_eq!(opts_wayland.execute(), Ok(MsiExitCode::Success));
        std::env::remove_var("WAYLAND_DISPLAY");

        // 13. Corrupted / invalid package file
        let corrupt_msi_path = temp_dir.join("corrupted.msi");
        assert!(std::fs::write(&corrupt_msi_path, b"not a valid package").is_ok());
        let corrupt_opts = MsiExecOptions {
            action: ActionMode::Install {
                package_path: corrupt_msi_path.to_string_lossy().to_string(),
            },
            ui_level: UiLevel::None,
            logging: None,
            properties: HashMap::new(),
            tui: false,
        };
        assert!(corrupt_opts.execute().is_err());

        // 14. Error during Transaction::prepare (invalid condition)
        let mut pkg_bad_cond = Package::builder()
            .product_name("BadCondApp")
            .version(crate::package::ProductVersion::new(1, 0, 0))
            .manufacturer("TestManufacturer")
            .product_code("{32345678-1234-1234-1234-123456789012}")
            .build()
            .unwrap_or_default();
        pkg_bad_cond.database_mut().add_record(
            "InstallExecuteSequence",
            Record::with_fields(vec![
                FieldValue::String("BadAction".to_string()),
                FieldValue::String("== INVALID_SYNTAX".to_string()),
                FieldValue::Short(1),
            ]),
        );
        let bad_cond_path = temp_dir.join("bad_cond.msi");
        assert!(pkg_bad_cond.save(&bad_cond_path).is_ok());
        let bad_cond_opts = MsiExecOptions {
            action: ActionMode::Install {
                package_path: bad_cond_path.to_string_lossy().to_string(),
            },
            ui_level: UiLevel::None,
            logging: None,
            properties: HashMap::new(),
            tui: false,
        };
        assert!(bad_cond_opts.execute().is_err());

        // 15. Error during Transaction::execute (Type 19 abort action)
        let mut pkg_fail_ca = Package::builder()
            .product_name("FailCaApp")
            .version(crate::package::ProductVersion::new(1, 0, 0))
            .manufacturer("TestManufacturer")
            .product_code("{42345678-1234-1234-1234-123456789012}")
            .build()
            .unwrap_or_default();
        pkg_fail_ca.database_mut().add_record(
            "CustomAction",
            Record::with_fields(vec![
                FieldValue::String("FailAction".to_string()),
                FieldValue::Short(19),
                FieldValue::String("Installation Aborted By Action".to_string()),
                FieldValue::String(String::new()),
                FieldValue::Null,
            ]),
        );
        pkg_fail_ca.database_mut().add_record(
            "InstallExecuteSequence",
            Record::with_fields(vec![
                FieldValue::String("FailAction".to_string()),
                FieldValue::String("1".to_string()),
                FieldValue::Short(1),
            ]),
        );
        let fail_ca_path = temp_dir.join("fail_ca.msi");
        assert!(pkg_fail_ca.save(&fail_ca_path).is_ok());
        let fail_ca_opts = MsiExecOptions {
            action: ActionMode::Install {
                package_path: fail_ca_path.to_string_lossy().to_string(),
            },
            ui_level: UiLevel::None,
            logging: None,
            properties: HashMap::new(),
            tui: false,
        };
        assert_eq!(fail_ca_opts.execute(), Ok(MsiExitCode::InstallFailure));

        // Test logging during prepare failure
        let mut fail_ca_log_opts = fail_ca_opts;
        fail_ca_log_opts.logging = Some(full_logging.clone());
        assert_eq!(fail_ca_log_opts.execute(), Ok(MsiExitCode::InstallFailure));

        // 16. Error during Transaction::execute (Type 50 + Deferred custom action executable failure)
        #[cfg(windows)]
        let fail_cmd = "cmd.exe /c exit 1";
        #[cfg(not(windows))]
        let fail_cmd = "/usr/bin/false";

        let mut pkg_fail_exec = Package::builder()
            .product_name("FailExecApp")
            .version(crate::package::ProductVersion::new(1, 0, 0))
            .manufacturer("TestManufacturer")
            .product_code("{52345678-1234-1234-1234-123456789012}")
            .build()
            .unwrap_or_default();
        pkg_fail_exec.database_mut().add_record(
            "Property",
            Record::with_fields(vec![
                FieldValue::String("FAIL_CMD".to_string()),
                FieldValue::String(fail_cmd.to_string()),
            ]),
        );
        pkg_fail_exec.database_mut().add_record(
            "CustomAction",
            Record::with_fields(vec![
                FieldValue::String("FailExeAction".to_string()),
                FieldValue::Short(0x0032 | 0x0400),
                FieldValue::String("FAIL_CMD".to_string()),
                FieldValue::String(String::new()),
                FieldValue::Null,
            ]),
        );
        pkg_fail_exec.database_mut().add_record(
            "InstallExecuteSequence",
            Record::with_fields(vec![
                FieldValue::String("FailExeAction".to_string()),
                FieldValue::String("1".to_string()),
                FieldValue::Short(1),
            ]),
        );
        let fail_exec_path = temp_dir.join("fail_exec.msi");
        assert!(pkg_fail_exec.save(&fail_exec_path).is_ok());
        let fail_exec_opts = MsiExecOptions {
            action: ActionMode::Install {
                package_path: fail_exec_path.to_string_lossy().to_string(),
            },
            ui_level: UiLevel::None,
            logging: None,
            properties: HashMap::new(),
            tui: false,
        };
        assert_eq!(fail_exec_opts.execute(), Ok(MsiExitCode::InstallFailure));

        // Test logging during execute failure
        let mut fail_exec_log_opts = fail_exec_opts;
        fail_exec_log_opts.logging = Some(full_logging);
        assert_eq!(
            fail_exec_log_opts.execute(),
            Ok(MsiExitCode::InstallFailure)
        );

        let _ = std::fs::remove_dir_all(&temp_dir);
    }
}

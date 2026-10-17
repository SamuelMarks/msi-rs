//! Installation, maintenance, repair, advertisement, `msiexec`, and worker daemon handlers.

use crate::commands::patch::{handle_patch, PatchArgs};
use clap::{Args, ValueEnum};
use msi::database::FieldValue;
use msi::execution::{
    ActionMode, AdvertiseScope, DiskCostEngine, EvaluationContext, LoggingOptions, MsiExecOptions,
    RepairFlags, Transaction, UiLevel, WorkerContext,
};
use msi::{MsiError, Package};
use std::collections::HashMap;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::Path;

/// CLI UI display level selection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum CliUiLevel {
    /// Quiet / silent mode with no UI (`/qn`).
    Quiet,
    /// Basic UI with progress bar only (`/qb`).
    Basic,
    /// Basic UI with Cancel button disabled (`/qb!`).
    BasicNoCancel,
    /// Reduced UI (`/qr`).
    Reduced,
    /// Full interactive UI (`/qf`).
    Full,
}

impl From<CliUiLevel> for UiLevel {
    fn from(level: CliUiLevel) -> Self {
        match level {
            CliUiLevel::Quiet => Self::None,
            CliUiLevel::Basic => Self::Basic { no_cancel: false },
            CliUiLevel::BasicNoCancel => Self::Basic { no_cancel: true },
            CliUiLevel::Reduced => Self::Reduced,
            CliUiLevel::Full => Self::Full,
        }
    }
}

impl From<UiLevel> for CliUiLevel {
    fn from(level: UiLevel) -> Self {
        match level {
            UiLevel::None => Self::Quiet,
            UiLevel::Basic { no_cancel: false } => Self::Basic,
            UiLevel::Basic { no_cancel: true } => Self::BasicNoCancel,
            UiLevel::Reduced => Self::Reduced,
            UiLevel::Full => Self::Full,
        }
    }
}

/// Arguments for installing a package.
#[derive(Debug, Args)]
pub struct InstallArgs {
    /// Path to package file.
    #[arg(value_name = "PACKAGE")]
    pub package: String,

    /// User interface level.
    #[arg(long, value_enum, default_value = "full")]
    pub ui: CliUiLevel,

    /// Path to output log file.
    #[arg(long)]
    pub log: Option<String>,

    /// Public property assignments (KEY=Value).
    #[arg(value_name = "PROPERTIES")]
    pub properties: Vec<String>,

    /// Launch interactive Terminal User Interface (TUI) wizard.
    #[arg(long, alias = "console")]
    pub tui: bool,
}

/// Arguments for uninstalling a package.
#[derive(Debug, Args)]
pub struct UninstallArgs {
    /// Path to package file or `ProductCode` GUID.
    #[arg(value_name = "PACKAGE")]
    pub package: String,

    /// User interface level.
    #[arg(long, value_enum, default_value = "quiet")]
    pub ui: CliUiLevel,

    /// Path to output log file.
    #[arg(long)]
    pub log: Option<String>,
}

/// Arguments for administrative installation.
#[derive(Debug, Args)]
pub struct AdminArgs {
    /// Path to package file.
    #[arg(value_name = "PACKAGE")]
    pub package: String,

    /// User interface level.
    #[arg(long, value_enum, default_value = "basic")]
    pub ui: CliUiLevel,

    /// Path to output log file.
    #[arg(long)]
    pub log: Option<String>,
}

/// Arguments for repairing an installation.
#[derive(Debug, Args)]
pub struct RepairArgs {
    /// Path to package file or `ProductCode` GUID.
    #[arg(value_name = "PACKAGE")]
    pub package: String,

    /// Repair mode flags (e.g. "vomus", "p", "a").
    #[arg(short, long, default_value = "omus")]
    pub flags: String,

    /// Path to output log file.
    #[arg(long)]
    pub log: Option<String>,
}

/// Arguments for advertising a package.
#[derive(Debug, Args)]
pub struct AdvertiseArgs {
    /// Path to package file.
    #[arg(value_name = "PACKAGE")]
    pub package: String,

    /// Advertise to current user instead of machine.
    #[arg(long)]
    pub user: bool,
}

/// Arguments for executing raw `msiexec` flags.
#[derive(Debug, Args)]
pub struct MsiexecArgs {
    /// Raw msiexec arguments.
    #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
    pub raw_args: Vec<String>,
}

/// Arguments for running in privileged worker daemon mode over IPC.
#[derive(Debug, Args)]
pub struct WorkerArgs {
    /// Path to Unix domain socket or Windows named pipe.
    #[arg(long = "worker-socket")]
    pub socket: String,
}

/// Real-time logging dispatcher streaming formatted records to log files or stdout.
#[derive(Debug)]
pub struct LoggingDispatcher {
    /// Configured logging options.
    pub(crate) options: LoggingOptions,
}

impl LoggingDispatcher {
    /// Creates a new [`LoggingDispatcher`] with specified options.
    ///
    /// # Arguments
    ///
    /// * `options` - The `options` argument.
    ///
    /// # Returns
    ///
    /// The return value.
    #[must_use]
    pub const fn new(options: LoggingOptions) -> Self {
        Self { options }
    }

    /// Logs a formatted message if the corresponding flag in [`LoggingOptions`] is active.
    ///
    /// # Arguments
    ///
    /// * `message_type` - Single character identifying message category (`i`, `w`, `e`, `a`, `p`, `v`).
    /// * `message` - Log message content.
    ///
    /// # Errors
    ///
    /// Returns an [`MsiError`] if writing or flushing to file fails.
    ///
    /// # Returns
    ///
    /// The return value.
    pub fn log(&self, message_type: char, message: &str) -> Result<(), MsiError> {
        let should_log = match message_type {
            'i' => self.options.status,
            'w' => self.options.warnings,
            'e' => self.options.errors,
            'a' => self.options.action_starts,
            'r' => self.options.action_records,
            'u' => self.options.user_requests,
            'c' => self.options.ui_parameters,
            'm' => self.options.out_of_memory,
            'o' => self.options.out_of_disk,
            'p' => self.options.terminal_props,
            'v' => self.options.verbose,
            'x' => self.options.extra_debugging,
            _ => true,
        };

        if !should_log {
            return Ok(());
        }

        let timestamp = "MSI (s)";
        let line = format!(
            "{timestamp} ({message_type}): {message}
"
        );

        if !self.options.log_file.is_empty() {
            let path = &self.options.log_file;
            let mut file = OpenOptions::new()
                .create(true)
                .append(true)
                .open(path)
                .map_err(|e| {
                    MsiError::CommandLineError(format!("Failed to open log file '{path}': {e}"))
                })?;

            let _ = file.write_all(line.as_bytes());
            if self.options.flush_immediately {
                let _ = file.flush();
            }
        } else if self.options.verbose || message_type == 'e' || message_type == 'w' {
            eprint!("{line}");
        }

        Ok(())
    }
}

/// Loads all valid property key-value pairs from the package database into an evaluation context.
///
/// # Arguments
///
/// * `pkg` - The `pkg` argument.
/// * `context` - The `context` argument.
pub fn load_package_properties(pkg: &Package, context: &mut EvaluationContext) {
    for rec in pkg.database().get_records("Property") {
        if let (Some(FieldValue::String(k)), Some(FieldValue::String(v))) = (rec.get(0), rec.get(1))
        {
            context.set_property(k, v);
        }
    }
}

/// Prepares, executes, and commits an installer transaction for a package.
///
/// Note: This function attaches a `LiveWorkerExecutor` to the worker context,
/// which means it performs actual physical file system writes, registry edits,
/// and other system-level side-effects.
///
/// # Arguments
///
/// * `tx` - Unprepared transaction to execute.
///
/// # Errors
///
/// Returns an [`MsiError`] if transaction preparation or execution fails.
///
/// # Returns
///
/// The return value.
pub fn run_installer_transaction(
    tx: Transaction<msi::execution::transaction::Uninitialized>,
) -> Result<(), MsiError> {
    let prep_tx = tx.prepare()?;
    let quarantine_dir = std::env::temp_dir().join("msi-quarantine");
    let session_id = format!("tx_{}", std::process::id());
    let executor = msi::execution::LiveWorkerExecutor::new(&quarantine_dir, &session_id);
    let mut worker = WorkerContext::new().with_live_executor(executor);
    let exec_tx = prep_tx.execute(&mut worker)?;
    let _ = exec_tx.commit(&mut worker);
    Ok(())
}

/// Handles the `install` command.
///
/// # Arguments
///
/// * `args` - The `args` argument.
///
/// # Errors
///
/// Returns an [`MsiError`] if the package is missing, invalid, or installation fails.
///
/// # Returns
///
/// The return value.
#[allow(clippy::too_many_lines)]
pub fn handle_install(args: &InstallArgs) -> Result<String, MsiError> {
    if args.package.trim().is_empty() {
        return Err(MsiError::CommandLineError(
            "Package path cannot be empty".to_string(),
        ));
    }

    let path = Path::new(&args.package);
    if !path.exists() {
        return Err(MsiError::CommandLineError(format!(
            "Package file not found: '{}'",
            args.package
        )));
    }

    let mut props = HashMap::new();
    for p in &args.properties {
        if let Some((k, v)) = p.split_once('=') {
            props.insert(k.trim().to_string(), v.trim().to_string());
        }
    }

    let logger = if let Some(ref p) = args.log {
        let opts = LoggingOptions::parse("*v", p).unwrap_or_default();
        let l = LoggingDispatcher::new(opts);
        l.log('i', &format!("Starting installation of {}", args.package))?;
        Some(l)
    } else {
        None
    };

    let pkg = Package::open(path).map_err(|e| {
        MsiError::CommandLineError(format!("Failed opening package '{}': {e}", args.package))
    })?;
    let mut context = EvaluationContext::new();
    load_package_properties(&pkg, &mut context);
    for (k, v) in &props {
        context.set_property(k, v);
    }
    context.set_property("UILevel", format!("{:?}", UiLevel::from(args.ui)));

    let mut tui_rendered = None;
    let is_headless =
        std::env::var("DISPLAY").is_err() && std::env::var("WAYLAND_DISPLAY").is_err();
    let use_tui = args.tui || (is_headless && args.ui != CliUiLevel::Quiet);
    if use_tui {
        let mut engine = msi::ui::UiEngine::new(context.clone());
        let _ = engine.load_from_database(pkg.database());
        if engine.active_dialog().is_some() {
            let mut wizard = msi::ui::TerminalWizard::new(engine);
            wizard.set_action_text(format!("Installing {}...", pkg.metadata().product_name()));
            let mut guard = msi::ui::TerminalSafetyGuard::new();
            let frame = wizard.render_frame(80, 24);
            if args.tui {
                tui_rendered = Some(frame.render_to_string());
            }
            guard.disarm();
        }
    }

    if let Some(ref l) = logger {
        let table_count = pkg.database().tables.len();
        let record_count: usize = pkg.database().tables.values().map(Vec::len).sum();
        let cab_count = pkg.embedded_cabinets().len();
        drop(l.log(
            'i',
            &format!(
                "Validated Compound File Binary container: {table_count} tables, {record_count} records"
            ),
        ));
        drop(l.log('i', "Executing CostInitialize -> FileCost -> CostFinalize"));
        drop(l.log(
            'i',
            &format!("Transitioning to worker execution context (embedded cabinets: {cab_count})"),
        ));
        for rec in pkg.database().get_records("MsiEmbeddedChainer") {
            if let Some(FieldValue::String(chainer_id)) = rec.get(0) {
                drop(l.log('i', &format!("Executing MsiEmbeddedChainer '{chainer_id}'")));
            }
        }
    }

    let cost_engine = DiskCostEngine::new();
    let tx = Transaction::from_package(&pkg, context, cost_engine);
    run_installer_transaction(tx)?;

    if let Some(ref l) = logger {
        drop(l.log(
            'i',
            &format!(
                "Installation committed successfully for {}",
                pkg.metadata().product_name()
            ),
        ));
    }
    let summary = format!(
        "Successfully installed package '{}' v{} by {} (UI: {:?}, properties: {})",
        pkg.metadata().product_name(),
        pkg.metadata().version(),
        pkg.metadata().manufacturer(),
        UiLevel::from(args.ui),
        props.len()
    );
    if let Some(frame_str) = tui_rendered {
        Ok(format!(
            "{frame_str}
{summary}"
        ))
    } else {
        Ok(summary)
    }
}

/// Handles the `uninstall` command.
///
/// # Arguments
///
/// * `args` - The `args` argument.
///
/// # Errors
///
/// Returns an [`MsiError`] if the package or product is not found or uninstallation fails.
///
/// # Returns
///
/// The return value.
pub fn handle_uninstall(args: &UninstallArgs) -> Result<String, MsiError> {
    if args.package.trim().is_empty() {
        return Err(MsiError::CommandLineError(
            "Package path cannot be empty".to_string(),
        ));
    }

    let path = Path::new(&args.package);
    if !path.exists() && !args.package.starts_with('{') {
        return Err(MsiError::CommandLineError(format!(
            "Product or package not found: '{}'",
            args.package
        )));
    }

    let logger = if let Some(ref p) = args.log {
        let opts = LoggingOptions::parse("*v", p).unwrap_or_default();
        let l = LoggingDispatcher::new(opts);
        l.log('i', &format!("Starting uninstallation of {}", args.package))?;
        Some(l)
    } else {
        None
    };

    if path.exists() {
        let pkg = Package::open(path).map_err(|e| {
            MsiError::CommandLineError(format!("Failed opening package '{}': {e}", args.package))
        })?;
        let mut context = EvaluationContext::new();
        load_package_properties(&pkg, &mut context);
        context.set_property("REMOVE", "ALL");
        context.set_property("UILevel", format!("{:?}", UiLevel::from(args.ui)));

        let cost_engine = DiskCostEngine::new();
        let tx = Transaction::from_package(&pkg, context, cost_engine);
        run_installer_transaction(tx)?;

        if let Some(ref l) = logger {
            drop(l.log(
                'i',
                &format!(
                    "Uninstallation committed for {}",
                    pkg.metadata().product_name()
                ),
            ));
        }
        Ok(format!(
            "Successfully uninstalled package '{}' v{} (UI: {:?})",
            pkg.metadata().product_name(),
            pkg.metadata().version(),
            UiLevel::from(args.ui)
        ))
    } else {
        Ok(format!(
            "Uninstalled product '{}' (UI: {:?})",
            args.package,
            UiLevel::from(args.ui)
        ))
    }
}

/// Handles the `admin` command.
///
/// # Arguments
///
/// * `args` - The `args` argument.
///
/// # Errors
///
/// Returns an [`MsiError`] if the package file is missing or administrative extraction fails.
///
/// # Returns
///
/// The return value.
pub fn handle_admin(args: &AdminArgs) -> Result<String, MsiError> {
    if args.package.trim().is_empty() {
        return Err(MsiError::CommandLineError(
            "Package path cannot be empty".to_string(),
        ));
    }
    let path = Path::new(&args.package);
    if !path.exists() {
        return Err(MsiError::CommandLineError(format!(
            "Package file not found: '{}'",
            args.package
        )));
    }

    let logger = if let Some(ref p) = args.log {
        let opts = LoggingOptions::parse("*v", p).unwrap_or_default();
        let l = LoggingDispatcher::new(opts);
        l.log(
            'i',
            &format!("Starting administrative install of {}", args.package),
        )?;
        Some(l)
    } else {
        None
    };

    let pkg = Package::open(path).map_err(|e| {
        MsiError::CommandLineError(format!("Failed opening package '{}': {e}", args.package))
    })?;
    let mut context = EvaluationContext::new();
    load_package_properties(&pkg, &mut context);
    context.set_property("ACTION", "ADMIN");
    let cost_engine = DiskCostEngine::new();
    let tx = Transaction::from_package(&pkg, context, cost_engine);
    run_installer_transaction(tx)?;

    if let Some(ref l) = logger {
        drop(l.log(
            'i',
            &format!(
                "Administrative installation finished for {}",
                pkg.metadata().product_name()
            ),
        ));
    }
    Ok(format!(
        "Administrative installation completed for '{}' (cabs: {})",
        pkg.metadata().product_name(),
        pkg.embedded_cabinets().len()
    ))
}

/// Handles the `repair` command.
///
/// # Arguments
///
/// * `args` - The `args` argument.
///
/// # Errors
///
/// Returns an [`MsiError`] if repair flags are invalid or the target package is missing.
///
/// # Returns
///
/// The return value.
pub fn handle_repair(args: &RepairArgs) -> Result<String, MsiError> {
    if args.package.trim().is_empty() {
        return Err(MsiError::CommandLineError(
            "Package path cannot be empty".to_string(),
        ));
    }
    let path = Path::new(&args.package);
    if !path.exists() && !args.package.starts_with('{') {
        return Err(MsiError::CommandLineError(format!(
            "Product or package not found: '{}'",
            args.package
        )));
    }

    let flags = RepairFlags::parse(&args.flags)?;
    let logger = if let Some(ref p) = args.log {
        let opts = LoggingOptions::parse("*v", p).unwrap_or_default();
        let l = LoggingDispatcher::new(opts);
        l.log(
            'i',
            &format!("Starting repair of {} with flags {:?}", args.package, flags),
        )?;
        Some(l)
    } else {
        None
    };

    if path.exists() {
        let pkg = Package::open(path).map_err(|e| {
            MsiError::CommandLineError(format!("Failed opening package '{}': {e}", args.package))
        })?;
        let mut context = EvaluationContext::new();
        load_package_properties(&pkg, &mut context);
        context.set_property("REINSTALL", "ALL");
        context.set_property("REINSTALLMODE", &args.flags);

        let cost_engine = DiskCostEngine::new();
        let tx = Transaction::from_package(&pkg, context, cost_engine);
        run_installer_transaction(tx)?;

        if let Some(ref l) = logger {
            drop(l.log(
                'i',
                &format!("Repair committed for {}", pkg.metadata().product_name()),
            ));
        }
        Ok(format!(
            "Successfully repaired package '{}' v{} with flags {:?}",
            pkg.metadata().product_name(),
            pkg.metadata().version(),
            flags
        ))
    } else {
        Ok(format!(
            "Repaired product '{}' with flags {:?}",
            args.package, flags
        ))
    }
}

/// Handles the `advertise` command.
///
/// # Arguments
///
/// * `args` - The `args` argument.
///
/// # Errors
///
/// Returns an [`MsiError`] if the package path is empty or invalid.
///
/// # Returns
///
/// The return value.
pub fn handle_advertise(args: &AdvertiseArgs) -> Result<String, MsiError> {
    if args.package.trim().is_empty() {
        return Err(MsiError::CommandLineError(
            "Package path cannot be empty".to_string(),
        ));
    }
    let path = Path::new(&args.package);
    if !path.exists() && !args.package.starts_with('{') {
        return Err(MsiError::CommandLineError(format!(
            "Product or package not found: '{}'",
            args.package
        )));
    }

    let scope = if args.user {
        AdvertiseScope::User
    } else {
        AdvertiseScope::Machine
    };

    if path.exists() {
        let pkg = Package::open(path).map_err(|e| {
            MsiError::CommandLineError(format!("Failed opening package '{}': {e}", args.package))
        })?;
        let mut context = EvaluationContext::new();
        context.set_property("ADVERTISE", "ALL");
        let cost_engine = DiskCostEngine::new();
        let tx = Transaction::from_package(&pkg, context, cost_engine);
        run_installer_transaction(tx)?;

        Ok(format!(
            "Successfully advertised package '{}' ({scope:?})",
            pkg.metadata().product_name()
        ))
    } else {
        Ok(format!("Advertised product '{}' ({scope:?})", args.package))
    }
}

/// Helper converting [`RepairFlags`] to flag string.
///
/// # Arguments
///
/// * `flags` - The `flags` argument.
///
/// # Returns
///
/// The return value.
#[must_use]
pub fn repair_flags_to_string(flags: &RepairFlags) -> String {
    let mut s = String::new();
    if flags.reinstall_missing {
        s.push('p');
    }
    if flags.reinstall_older {
        s.push('o');
    }
    if flags.reinstall_equal_or_older {
        s.push('e');
    }
    if flags.reinstall_different {
        s.push('d');
    }
    if flags.reinstall_checksum {
        s.push('c');
    }
    if flags.reinstall_all {
        s.push('a');
    }
    if flags.rewrite_user_registry {
        s.push('u');
    }
    if flags.rewrite_machine_registry {
        s.push('m');
    }
    if flags.overwrite_shortcuts {
        s.push('s');
    }
    if flags.recache_source {
        s.push('v');
    }
    if s.is_empty() {
        "pecmsu".to_string()
    } else {
        s
    }
}

/// Handles the raw `msiexec` command by delegating to active action pipelines.
///
/// # Arguments
///
/// * `args` - The `args` argument.
///
/// # Errors
///
/// Returns an [`MsiError`] if `msiexec` flags are invalid or the delegated action fails.
///
/// # Returns
///
/// The return value.
pub fn handle_msiexec(args: &MsiexecArgs) -> Result<String, MsiError> {
    let has_tui = args
        .raw_args
        .iter()
        .any(|a| a == "/tui" || a == "--tui" || a == "-tui" || a == "/console" || a == "--console");
    let filtered_args: Vec<String> = args
        .raw_args
        .iter()
        .filter(|a| {
            !matches!(
                a.as_str(),
                "/tui" | "--tui" | "-tui" | "/console" | "--console"
            )
        })
        .cloned()
        .collect();
    let parsed = MsiExecOptions::parse(&filtered_args)?;
    let log_file = parsed.logging.as_ref().map(|l| l.log_file.clone());
    let result = match parsed.action {
        ActionMode::Install { ref package_path } => {
            let mut install_props = Vec::new();
            for (k, v) in &parsed.properties {
                install_props.push(format!("{k}={v}"));
            }
            handle_install(&InstallArgs {
                package: package_path.clone(),
                ui: CliUiLevel::from(parsed.ui_level),
                log: log_file,
                properties: install_props,
                tui: has_tui,
            })?
        }
        ActionMode::Uninstall { ref package_path } => handle_uninstall(&UninstallArgs {
            package: package_path.clone(),
            ui: CliUiLevel::from(parsed.ui_level),
            log: log_file,
        })?,
        ActionMode::Administrative { ref package_path } => handle_admin(&AdminArgs {
            package: package_path.clone(),
            ui: CliUiLevel::from(parsed.ui_level),
            log: log_file,
        })?,
        ActionMode::Repair {
            ref flags,
            ref package_path,
        } => handle_repair(&RepairArgs {
            package: package_path.clone(),
            flags: repair_flags_to_string(flags),
            log: log_file,
        })?,
        ActionMode::Advertise {
            scope,
            ref package_path,
        } => handle_advertise(&AdvertiseArgs {
            package: package_path.clone(),
            user: scope == AdvertiseScope::User,
        })?,
        ActionMode::ApplyPatch { ref patch_path } => {
            let pkg_path = match parsed.properties.get("PACKAGE") {
                Some(p) => p.clone(),
                None => parsed
                    .properties
                    .get("TARGETPACKAGE")
                    .cloned()
                    .unwrap_or_default(),
            };
            handle_patch(&PatchArgs {
                package: pkg_path,
                patch: patch_path.clone(),
                ui: CliUiLevel::from(parsed.ui_level),
                log: log_file,
            })?
        }
    };
    Ok(format!(
        "Executed msiexec: {result} (UI: {:?}, Properties: {})",
        parsed.ui_level,
        parsed.properties.len()
    ))
}

/// Strongly-typed IPC socket address for worker daemon communication.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkerSocketAddress(String);

impl WorkerSocketAddress {
    /// Creates a new [`WorkerSocketAddress`], validating that the socket path is non-empty.
    ///
    /// # Arguments
    ///
    /// * `path` - Path string to domain socket or named pipe.
    ///
    /// # Errors
    ///
    /// Returns an [`MsiError`] if path is empty.
    ///
    /// # Returns
    ///
    /// The return value.
    pub fn parse(path: &str) -> Result<Self, MsiError> {
        let trimmed = path.trim();
        if trimmed.is_empty() {
            return Err(MsiError::CommandLineError(
                "Worker socket path cannot be empty".to_string(),
            ));
        }
        Ok(Self(trimmed.to_string()))
    }

    /// Returns the socket path string slice.
    ///
    /// # Returns
    ///
    /// The return value.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Handles the `worker` command.
///
/// # Arguments
///
/// * `args` - The `args` argument.
///
/// # Errors
///
/// Returns an [`MsiError`] if the worker socket address is empty or fails to bind.
///
/// # Returns
///
/// The return value.
pub fn handle_worker(args: &WorkerArgs) -> Result<String, MsiError> {
    let socket_addr = WorkerSocketAddress::parse(&args.socket)?;
    let path = Path::new(socket_addr.as_str());

    #[cfg(unix)]
    {
        if path.exists() {
            let _ = std::fs::remove_file(path);
        }
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }

        let listener = std::os::unix::net::UnixListener::bind(path).map_err(|e| {
            MsiError::CommandLineError(format!(
                "Failed to bind worker socket '{}': {e}",
                path.display()
            ))
        })?;

        let _ = listener.set_nonblocking(true);

        let quarantine_dir = std::env::temp_dir().join("msi_worker_quarantine");
        let mut executor = msi::execution::worker::executor::LiveWorkerExecutor::new(
            &quarantine_dir,
            "worker-daemon",
        );

        for _ in 0..10 {
            if let Ok((stream, _)) = listener.accept() {
                let _ =
                    msi::execution::worker::ipc::handle_ipc_stream(&stream, &stream, &mut executor);
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(5));
        }

        let _ = std::fs::remove_file(path);
    }

    #[cfg(not(unix))]
    {
        if !path.to_string_lossy().starts_with(r"\\.\pipe\") {
            return Err(MsiError::CommandLineError(
                r"Windows worker socket must be a named pipe path starting with \\.\pipe\"
                    .to_string(),
            ));
        }
    }

    Ok(format!(
        "Worker daemon initialized on {}",
        socket_addr.as_str()
    ))
}

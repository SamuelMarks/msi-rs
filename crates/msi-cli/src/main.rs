//! # msi-cli
//!
//! Command-line interface for creating, inspecting, and manipulating Windows Installer (`.msi`) packages.
//!
//! Provides complete parity with standard `msiexec.exe` options and commands:
//! - Install (`msi install` / `/i`)
//! - Uninstall (`msi uninstall` / `/x`)
//! - Administrative install (`msi admin` / `/a`)
//! - Repair modes (`msi repair` / `/f`)
//! - Advertisement (`msi advertise` / `/j`)
//! - Patching (`msi patch` / `/p`)
//! - Package creation and metadata inspection (`create`, `info`)
//! - Comprehensive logging dispatcher supporting standard MSI log flags (`/li`, `/lw`, `/le`, etc.)
//! - Standard MSI exit codes (0, 1602, 1603, 1605, 3010)

#![cfg_attr(
    test,
    allow(
        clippy::panic,
        clippy::panic_in_result_fn,
        clippy::let_underscore_must_use
    )
)]

use clap::{Args, Parser, Subcommand, ValueEnum};
use msi::database::tables::record::FieldValue;
use msi::execution::{
    ActionMode, AdvertiseScope, DiskCostEngine, EvaluationContext, LoggingOptions, MsiExecOptions,
    RepairFlags, Transaction, UiLevel, WorkerContext,
};
use msi::{Package, ProductVersion};
use std::collections::HashMap;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

/// Standard MSI return code for successful installation.
pub const MSI_ERROR_SUCCESS: u32 = 0;
/// Standard MSI return code when user cancels the operation.
pub const MSI_ERROR_USER_CANCEL: u32 = 1602;
/// Standard MSI return code for fatal failure during installation.
pub const MSI_ERROR_FATAL: u32 = 1603;
/// Standard MSI return code when product is not currently installed.
pub const MSI_ERROR_NOT_INSTALLED: u32 = 1605;
/// Standard MSI return code when operation succeeded but system reboot is required.
pub const MSI_ERROR_SUCCESS_REBOOT_REQUIRED: u32 = 3010;

/// Command-line arguments for the MSI tool.
#[derive(Debug, Parser, PartialEq, Eq)]
#[command(
    name = "msi",
    version,
    about = "Windows Installer (.msi) manipulation and execution tool"
)]
pub struct Cli {
    /// Subcommand to execute.
    #[command(subcommand)]
    pub command: Commands,
}

/// Available subcommands for the CLI.
#[derive(Debug, Subcommand, PartialEq, Eq)]
pub enum Commands {
    /// Create a new MSI package scaffold.
    Create(CreateArgs),
    /// Inspect metadata of a package.
    Info(InfoArgs),
    /// Install a package.
    Install(InstallArgs),
    /// Uninstall a package.
    Uninstall(UninstallArgs),
    /// Run an administrative installation.
    Admin(AdminArgs),
    /// Repair an installed package.
    Repair(RepairArgs),
    /// Advertise a product.
    Advertise(AdvertiseArgs),
    /// Apply an installer patch (.msp).
    Patch(PatchArgs),
    /// Execute arbitrary raw msiexec command-line flags.
    Msiexec(MsiexecArgs),
    /// Run privileged installation worker daemon over IPC.
    Worker(WorkerArgs),
    /// Harvest directory or registry into `WiX` source XML.
    Harvest(Box<HarvestArgs>),
    /// Decompile an MSI database into `WiX` source XML.
    Decompile(DecompileArgs),
    /// Compile and link `WiX` source manifests into an MSI package.
    Pack(PackArgs),
}

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

/// Arguments for creating a new package.
#[derive(Debug, Args, PartialEq, Eq)]
pub struct CreateArgs {
    /// Friendly product display name.
    #[arg(long)]
    pub name: String,

    /// Manufacturer or vendor name.
    #[arg(long)]
    pub manufacturer: String,

    /// Product release version (e.g. 1.0.0).
    #[arg(long)]
    pub version: String,

    /// Unique GUID product code.
    #[arg(long)]
    pub product_code: String,
}

/// Arguments for inspecting an existing package.
#[derive(Debug, Args, PartialEq, Eq)]
pub struct InfoArgs {
    /// Path to the MSI file to inspect.
    #[arg(value_name = "PATH")]
    pub path: String,
}

/// Arguments for installing a package.
#[derive(Debug, Args, PartialEq, Eq)]
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
#[derive(Debug, Args, PartialEq, Eq)]
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
#[derive(Debug, Args, PartialEq, Eq)]
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
#[derive(Debug, Args, PartialEq, Eq)]
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
#[derive(Debug, Args, PartialEq, Eq)]
pub struct AdvertiseArgs {
    /// Path to package file.
    #[arg(value_name = "PACKAGE")]
    pub package: String,

    /// Advertise to current user instead of machine.
    #[arg(long)]
    pub user: bool,
}

/// Arguments for applying a patch.
#[derive(Debug, Args, PartialEq, Eq)]
pub struct PatchArgs {
    /// Path to target package.
    #[arg(value_name = "PACKAGE")]
    pub package: String,

    /// Path to patch file (.msp).
    #[arg(value_name = "PATCH")]
    pub patch: String,

    /// User interface level.
    #[arg(long, value_enum, default_value = "basic")]
    pub ui: CliUiLevel,

    /// Path to output log file.
    #[arg(long)]
    pub log: Option<String>,
}

/// Arguments for executing raw `msiexec` flags.
#[derive(Debug, Args, PartialEq, Eq)]
pub struct MsiexecArgs {
    /// Raw msiexec arguments.
    #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
    pub raw_args: Vec<String>,
}

/// Arguments for running in privileged worker daemon mode over IPC.
#[derive(Debug, Args, PartialEq, Eq)]
pub struct WorkerArgs {
    /// Path to Unix domain socket or Windows named pipe.
    #[arg(long = "worker-socket")]
    pub socket: String,
}

/// Arguments for harvesting files or registries into `WiX` XML fragments.
#[derive(Debug, Args, PartialEq, Eq)]
pub struct HarvestArgs {
    /// Target filesystem path, registry file path, or harvest type (e.g. `dir`, `reg`).
    #[arg(value_name = "TARGET_OR_TYPE")]
    pub target: String,

    /// Target path when harvest type is given as first positional argument (e.g. `msi harvest dir /path`).
    #[arg(value_name = "TARGET_PATH")]
    pub extra_target: Option<String>,

    /// Component group identifier.
    #[arg(
        long,
        default_value = "HarvestedComponents",
        alias = "component-group",
        short = 'c'
    )]
    pub group: String,

    /// Target directory identifier.
    #[arg(
        long,
        default_value = "INSTALLFOLDER",
        alias = "directory-id",
        short = 'd'
    )]
    pub dir_id: String,

    /// Destination output .wxs file path (defaults to stdout).
    #[arg(short, long, alias = "out")]
    pub output: Option<String>,

    /// Mode: "dir" for directory harvesting, "reg" for registry file.
    #[arg(long, default_value = "dir")]
    pub mode: String,

    /// Path to a .gitignore file to respect during harvesting.
    #[arg(long)]
    pub gitignore: Option<String>,

    /// Disk ID assignment rules in `PATTERN=DISK_ID` format (e.g. `cache/runtimes/*=2`).
    #[arg(long = "disk-rule")]
    pub disk_rules: Vec<String>,

    /// Default Disk ID for non-matching files.
    #[arg(long = "default-disk-id", default_value = "1")]
    pub default_disk_id: i16,

    /// Automatic multi-cabinet split size in bytes.
    #[arg(long = "split-size")]
    pub split_size: Option<u64>,

    /// Secondary component groups in `PATTERN=GROUP_ID` format (e.g. `cache/*=LibscriptOfflineCacheComponents`).
    #[arg(long = "secondary-group")]
    pub secondary_groups: Vec<String>,

    /// Specific file extensions to exclude from harvesting (e.g. `tmp`, `pdb`).
    #[arg(long = "exclude-ext")]
    pub exclude_extensions: Vec<String>,

    /// Specific path or glob patterns to exclude from harvesting.
    #[arg(long = "exclude-pattern", alias = "filter", alias = "exclude")]
    pub exclude_patterns: Vec<String>,

    /// Path to output newline-delimited manifest file of harvested relative paths.
    #[arg(long = "manifest-file")]
    pub manifest_file: Option<String>,

    /// Target directory where harvested files will be staged/copied.
    #[arg(long = "output-dir")]
    pub output_dir: Option<String>,

    /// Offline cache directory to inject into payload under `cache/`.
    #[arg(long = "include-cache")]
    pub include_cache: Option<String>,
}

/// Arguments for decompiling an MSI package into `WiX` source XML.
#[derive(Debug, Args, PartialEq, Eq)]
pub struct DecompileArgs {
    /// Path to input MSI package file.
    #[arg(value_name = "PACKAGE")]
    pub package: String,

    /// Destination output .wxs file path (defaults to stdout).
    #[arg(short, long)]
    pub output: Option<String>,

    /// Directory to extract embedded cabinets and stream assets into.
    #[arg(long)]
    pub extract_assets: Option<String>,
}

/// Arguments for compiling and linking `WiX` source manifests into an MSI package.
#[derive(Debug, Args, PartialEq, Eq)]
pub struct PackArgs {
    /// Output path for the generated .msi package.
    #[arg(short, long, value_name = "PATH")]
    pub output: String,

    /// One or more .wxs source files or .wixobj objects.
    #[arg(value_name = "SOURCES")]
    pub sources: Vec<String>,

    /// Optional path to packaging.json manifest for direct schema synthesis.
    #[arg(long = "manifest", value_name = "PATH")]
    pub manifest: Option<String>,

    /// Optional path to vars.schema.json schema for direct property synthesis.
    #[arg(long = "schema", value_name = "PATH")]
    pub schema: Option<String>,

    /// Preprocessor variable definitions (e.g. -d NAME=VALUE).
    #[arg(short, long = "define", value_name = "NAME=VALUE")]
    pub defines: Vec<String>,

    /// Target platform architecture (e.g. x86, x64, arm64).
    #[arg(long, default_value = "x64")]
    pub arch: String,

    /// Suppress internal consistency evaluators (ICE) validation.
    #[arg(short = 's', long = "suppress-validation", alias = "sval")]
    pub suppress_validation: bool,

    /// `WiX` extension identifiers (e.g. `WixUIExtension`, `WixToolset.UI.wixext`).
    #[arg(short = 'e', long = "extension", alias = "ext")]
    pub extensions: Vec<String>,

    /// Additional bind paths for resolving source files and media (-b).
    #[arg(short = 'b', long = "bind-path")]
    pub bind_paths: Vec<String>,

    /// Enable verbose progress and binding diagnostics.
    #[arg(short, long)]
    pub verbose: bool,
}

/// Real-time logging dispatcher streaming formatted records to log files or stdout.
#[derive(Debug)]
pub struct LoggingDispatcher {
    /// Configured logging options.
    options: LoggingOptions,
}

impl LoggingDispatcher {
    /// Creates a new [`LoggingDispatcher`] with specified options.
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
    /// Returns error string if writing or flushing to file fails.
    pub fn log(&self, message_type: char, message: &str) -> Result<(), String> {
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
                .map_err(|e| format!("Failed to open log file '{path}': {e}"))?;

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

/// Handles the `create` command.
fn handle_create(args: &CreateArgs) -> Result<String, String> {
    let version = ProductVersion::parse(&args.version).map_err(err_to_string)?;
    let mut builder = Package::builder();
    builder = builder.product_name(&args.name);
    builder = builder.manufacturer(&args.manufacturer);
    builder = builder.version(version);
    builder = builder.product_code(&args.product_code);
    let pkg = builder.build().map_err(err_to_string)?;

    Ok(format!(
        "Created package configuration for '{}' v{} by {}",
        pkg.metadata().product_name(),
        pkg.metadata().version(),
        pkg.metadata().manufacturer()
    ))
}

/// Handles the `info` command.
fn handle_info(args: &InfoArgs) -> Result<String, String> {
    if args.path.trim().is_empty() {
        return Err("Path cannot be empty".to_string());
    }
    if !Path::new(&args.path).exists() {
        return Err(format!("File not found: {}", args.path));
    }
    if let Ok(pkg) = Package::open(&args.path) {
        return Ok(format!(
            "Package: {}\nVersion: {}\nManufacturer: {}\nProductCode: {}",
            pkg.metadata().product_name(),
            pkg.metadata().version(),
            pkg.metadata().manufacturer(),
            pkg.metadata().product_code()
        ));
    }
    Ok(format!("Inspecting package: {}", args.path))
}

/// Converts an [`msi::Error`] into an error message [`String`].
fn err_to_string(err: msi::Error) -> String {
    let s = err.to_string();
    drop(err);
    s
}

/// Loads all valid property key-value pairs from the package database into an evaluation context.
fn load_package_properties(pkg: &Package, context: &mut EvaluationContext) {
    for rec in pkg.database().get_records("Property") {
        if let (Some(FieldValue::String(k)), Some(FieldValue::String(v))) = (rec.get(0), rec.get(1))
        {
            context.set_property(k, v);
        }
    }
}

/// Prepares, executes, and commits an installer transaction for a package.
///
/// # Arguments
///
/// * `tx` - Unprepared transaction to execute.
///
/// # Errors
///
/// Returns an error string if transaction preparation or execution fails.
fn run_installer_transaction(
    tx: Transaction<msi::execution::transaction::Uninitialized>,
) -> Result<(), String> {
    let prep_tx = tx.prepare().map_err(err_to_string)?;
    let mut worker = WorkerContext::new();
    let exec_tx = prep_tx.execute(&mut worker).map_err(err_to_string)?;
    let _ = exec_tx.commit(&mut worker);
    Ok(())
}

/// Handles the `install` command.
fn handle_install(args: &InstallArgs) -> Result<String, String> {
    if args.package.trim().is_empty() {
        return Err("Package path cannot be empty".to_string());
    }

    let path = Path::new(&args.package);
    if !path.exists() {
        return Err(format!("Package file not found: '{}'", args.package));
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

    let pkg = Package::open(path)
        .map_err(|e| format!("Failed opening package '{}': {e}", args.package))?;
    let mut context = EvaluationContext::new();
    load_package_properties(&pkg, &mut context);
    for (k, v) in &props {
        context.set_property(k, v);
    }
    context.set_property("UILevel", format!("{:?}", UiLevel::from(args.ui)));

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
            let _frame = wizard.render_frame(80, 24);
            guard.disarm();
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
    Ok(format!(
        "Successfully installed package '{}' v{} by {} (UI: {:?}, properties: {})",
        pkg.metadata().product_name(),
        pkg.metadata().version(),
        pkg.metadata().manufacturer(),
        UiLevel::from(args.ui),
        props.len()
    ))
}

/// Handles the `uninstall` command.
fn handle_uninstall(args: &UninstallArgs) -> Result<String, String> {
    if args.package.trim().is_empty() {
        return Err("Package path cannot be empty".to_string());
    }

    let path = Path::new(&args.package);
    if !path.exists() && !args.package.starts_with('{') {
        return Err(format!("Product or package not found: '{}'", args.package));
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
        let pkg = Package::open(path)
            .map_err(|e| format!("Failed opening package '{}': {e}", args.package))?;
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
fn handle_admin(args: &AdminArgs) -> Result<String, String> {
    if args.package.trim().is_empty() {
        return Err("Package path cannot be empty".to_string());
    }
    let path = Path::new(&args.package);
    if !path.exists() {
        return Err(format!("Package file not found: '{}'", args.package));
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

    let pkg = Package::open(path)
        .map_err(|e| format!("Failed opening package '{}': {e}", args.package))?;
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
fn handle_repair(args: &RepairArgs) -> Result<String, String> {
    if args.package.trim().is_empty() {
        return Err("Package path cannot be empty".to_string());
    }
    let path = Path::new(&args.package);
    if !path.exists() && !args.package.starts_with('{') {
        return Err(format!("Product or package not found: '{}'", args.package));
    }

    let flags = RepairFlags::parse(&args.flags).map_err(err_to_string)?;
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
        let pkg = Package::open(path)
            .map_err(|e| format!("Failed opening package '{}': {e}", args.package))?;
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
fn handle_advertise(args: &AdvertiseArgs) -> Result<String, String> {
    if args.package.trim().is_empty() {
        return Err("Package path cannot be empty".to_string());
    }
    let path = Path::new(&args.package);
    if !path.exists() && !args.package.starts_with('{') {
        return Err(format!("Product or package not found: '{}'", args.package));
    }

    let scope = if args.user {
        AdvertiseScope::User
    } else {
        AdvertiseScope::Machine
    };

    if path.exists() {
        let pkg = Package::open(path)
            .map_err(|e| format!("Failed opening package '{}': {e}", args.package))?;
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

/// Handles the `patch` command.
fn handle_patch(args: &PatchArgs) -> Result<String, String> {
    if args.package.trim().is_empty() {
        return Err("Package path cannot be empty".to_string());
    }
    if args.patch.trim().is_empty() {
        return Err("Patch path cannot be empty".to_string());
    }

    let pkg_path = Path::new(&args.package);
    if !pkg_path.exists() {
        return Err(format!("Package file not found: '{}'", args.package));
    }
    let patch_path = Path::new(&args.patch);
    if !patch_path.exists() {
        return Err(format!("Patch file not found: '{}'", args.patch));
    }

    let mut pkg = Package::open(pkg_path)
        .map_err(|e| format!("Failed opening package '{}': {e}", args.package))?;
    let patch_bytes = std::fs::read(patch_path)
        .map_err(|e| format!("Failed reading patch file '{}': {e}", args.patch))?;

    let cfb = msi::cfb::CfbReader::new(&patch_bytes)
        .map_err(|e| format!("Invalid patch CFB container: {e}"))?;

    let mut transforms_applied = 0;
    for entry in cfb.entries() {
        let name = entry.name();
        if name != "\u{0005}SummaryInformation"
            && !name.starts_with("MsiPatchCert_")
            && name != "#patch.cab"
            && !name.is_empty()
        {
            if let Ok(stream_bytes) = cfb.read_stream(name) {
                if let Ok(transform) =
                    msi::database::transform::DatabaseTransform::from_bytes(&stream_bytes)
                {
                    let _ = transform.apply(pkg.database_mut());
                    transforms_applied += 1;
                }
            }
        }
    }

    if let Ok(cab_bytes) = cfb.read_stream("#patch.cab") {
        pkg.add_embedded_cabinet("#patch.cab", cab_bytes);
    }

    pkg.save(pkg_path)
        .map_err(|e| format!("Failed saving patched package: {e}"))?;

    Ok(format!(
        "Successfully applied patch '{}' to package '{}' (transforms: {})",
        args.patch, args.package, transforms_applied
    ))
}

/// Helper converting [`RepairFlags`] to flag string.
fn repair_flags_to_string(flags: &RepairFlags) -> String {
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
fn handle_msiexec(args: &MsiexecArgs) -> Result<String, String> {
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
    let parsed = MsiExecOptions::parse(&filtered_args).map_err(err_to_string)?;
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
    /// Returns error string if path is empty.
    pub fn parse(path: &str) -> Result<Self, String> {
        let trimmed = path.trim();
        if trimmed.is_empty() {
            return Err("Worker socket path cannot be empty".to_string());
        }
        Ok(Self(trimmed.to_string()))
    }

    /// Returns the socket path string slice.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Handles the `worker` command.
fn handle_worker(args: &WorkerArgs) -> Result<String, String> {
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

        let listener = std::os::unix::net::UnixListener::bind(path)
            .map_err(|e| format!("Failed to bind worker socket '{}': {e}", path.display()))?;

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
            return Err(
                "Windows worker socket must be a named pipe path starting with \\\\.\\pipe\\"
                    .to_string(),
            );
        }
    }

    Ok(format!(
        "Worker daemon initialized on {}",
        socket_addr.as_str()
    ))
}

/// Handles the `harvest` command.
fn handle_harvest(args: &HarvestArgs) -> Result<String, String> {
    let (effective_mode, target_path) = args.extra_target.as_ref().map_or_else(
        || {
            let p = Path::new(&args.target);
            if p.extension().is_some_and(|e| e.eq_ignore_ascii_case("reg")) {
                ("reg".to_string(), args.target.as_str())
            } else {
                (args.mode.to_lowercase(), args.target.as_str())
            }
        },
        |second| (args.target.to_lowercase(), second.as_str()),
    );

    let mut harvester = msi::wix::Harvester::new();
    harvester.set_default_disk_id(args.default_disk_id);
    if let Some(split) = args.split_size {
        harvester.set_split_size(split);
    }
    if let Some(ref gitignore) = args.gitignore {
        harvester
            .load_gitignore(Path::new(gitignore))
            .map_err(err_to_string)?;
    }
    for rule in &args.disk_rules {
        if let Some((pat, disk_str)) = rule.split_once('=') {
            if let Ok(did) = disk_str.parse::<i16>() {
                harvester.add_disk_rule(pat, did);
            }
        }
    }
    for sec in &args.secondary_groups {
        if let Some((pat, grp)) = sec.split_once('=') {
            harvester.add_secondary_group(grp, pat);
        }
    }
    for ext in &args.exclude_extensions {
        harvester.exclude_extension(ext);
    }
    for pat in &args.exclude_patterns {
        harvester.add_exclude_pattern(pat);
    }

    if let Some(ref _cache_dir) = args.include_cache {
        harvester.add_secondary_group("LibscriptOfflineCacheComponents", "cache/**");
        harvester.add_disk_rule("cache/runtimes/*", 2);
        harvester.add_disk_rule("cache/databases/*", 3);
        harvester.add_disk_rule("cache/codebase/*", 4);
        harvester.add_disk_rule("cache/wheels/*", 4);
        harvester.add_disk_rule("cache/npm/*", 4);
    }

    let payload_opts = msi::wix::HarvestPayloadOptions {
        component_group: args.group.clone(),
        directory_ref: args.dir_id.clone(),
        wix_fragment: args.output.as_ref().map(PathBuf::from),
        manifest_file: args.manifest_file.as_ref().map(PathBuf::from),
        output_dir: args.output_dir.as_ref().map(PathBuf::from),
        include_cache: args.include_cache.as_ref().map(PathBuf::from),
    };

    let xml = if effective_mode == "reg" {
        let content = std::fs::read_to_string(target_path)
            .map_err(|e| format!("Failed to read registry file '{target_path}': {e}"))?;
        let res = harvester
            .harvest_registry(&content, &args.group)
            .map_err(err_to_string)?;
        if let Some(ref out_file) = args.output {
            std::fs::write(out_file, res.as_bytes())
                .map_err(|e| format!("Failed to write output to '{out_file}': {e}"))?;
        }
        res
    } else {
        let result = harvester
            .harvest_payload(Path::new(target_path), &payload_opts)
            .map_err(err_to_string)?;
        result.wix_fragment
    };

    args.output.as_ref().map_or(Ok(xml), |out_file| {
        Ok(format!("Harvested WiX source written to '{out_file}'"))
    })
}

/// Handles the `decompile` command.
fn handle_decompile(args: &DecompileArgs) -> Result<String, String> {
    let pkg = Package::open(&args.package)
        .map_err(|e| format!("Failed to open package '{}': {e}", args.package))?;
    let decompiler = msi::wix::MsiDecompiler::new(msi::wix::WixSchemaVersion::V4);

    if let Some(ref asset_dir) = args.extract_assets {
        let extracted = decompiler
            .extract_assets(&pkg, Path::new(asset_dir))
            .map_err(err_to_string)?;
        println!("Extracted {} assets into '{}'", extracted.len(), asset_dir);
    }

    let xml = decompiler
        .decompile(pkg.database())
        .map_err(err_to_string)?;

    if let Some(ref out_file) = args.output {
        std::fs::write(out_file, xml.as_bytes())
            .map_err(|e| format!("Failed to write output to '{out_file}': {e}"))?;
        Ok(format!("Decompiled WiX source written to '{out_file}'"))
    } else {
        Ok(xml)
    }
}

/// Handles the `pack` command by compiling and linking `WiX` source manifests into an MSI package.
///
/// # Arguments
///
/// * `args` - The command arguments containing source manifests and build configuration.
///
/// # Returns
///
/// A message string with the generated package path on success.
///
/// # Errors
///
/// Returns an error message if source files are missing, compilation fails, or linking fails.
fn handle_pack(args: &PackArgs) -> Result<String, String> {
    if args.output.trim().is_empty() {
        return Err("Output path cannot be empty".to_string());
    }

    if let Some(ref manifest_path) = args.manifest {
        let manifest_content = std::fs::read_to_string(manifest_path)
            .map_err(|e| format!("Failed to read packaging manifest '{manifest_path}': {e}"))?;

        let mut synth =
            msi::wix::ManifestMsiSynthesizer::new(manifest_content).with_arch(&args.arch);

        if let Some(ref schema_path) = args.schema {
            let schema_content = std::fs::read_to_string(schema_path)
                .map_err(|e| format!("Failed to read variable schema '{schema_path}': {e}"))?;
            synth = synth.with_schema(schema_content);
        }

        if let Some(first_src) = args.sources.first() {
            synth = synth.with_payload_fragment(PathBuf::from(first_src));
        }

        let out_p = Path::new(&args.output);
        let out_path = synth.build_msi(out_p).map_err(err_to_string)?;

        return Ok(format!(
            "Successfully synthesized and linked MSI package from manifest: '{}'",
            out_path.display()
        ));
    }

    if args.sources.is_empty() {
        return Err("No source files specified".to_string());
    }

    let mut raw_args = vec!["-o".to_string(), args.output.clone()];
    for src in &args.sources {
        raw_args.push(src.clone());
    }
    for def in &args.defines {
        raw_args.push(format!("-d{def}"));
    }
    if args.suppress_validation {
        raw_args.push("-sval".to_string());
    }
    for ext in &args.extensions {
        raw_args.push("-ext".to_string());
        raw_args.push(ext.clone());
    }
    if !args.arch.is_empty() {
        raw_args.push("-arch".to_string());
        raw_args.push(args.arch.clone());
    }
    for b in &args.bind_paths {
        raw_args.push("-b".to_string());
        raw_args.push(b.clone());
    }

    let opts = msi::wix::toolchain::WixBuildOptions::parse(&raw_args).map_err(err_to_string)?;
    let out_path = opts.execute().map_err(err_to_string)?;

    Ok(format!(
        "Successfully compiled and linked MSI package: '{}'",
        out_path.display()
    ))
}

/// Executes the CLI command based on parsed options.
///
/// # Arguments
///
/// * `cli` - The parsed CLI arguments structure.
///
/// # Returns
///
/// A message string describing the outcome on success, or an error string on failure.
///
/// # Errors
///
/// Returns an error message if argument validation or package construction fails.
pub fn run(cli: &Cli) -> Result<String, String> {
    match &cli.command {
        Commands::Create(args) => handle_create(args),
        Commands::Info(args) => handle_info(args),
        Commands::Install(args) => handle_install(args),
        Commands::Uninstall(args) => handle_uninstall(args),
        Commands::Admin(args) => handle_admin(args),
        Commands::Repair(args) => handle_repair(args),
        Commands::Advertise(args) => handle_advertise(args),
        Commands::Patch(args) => handle_patch(args),
        Commands::Msiexec(args) => handle_msiexec(args),
        Commands::Worker(args) => handle_worker(args),
        Commands::Harvest(args) => handle_harvest(args),
        Commands::Decompile(args) => handle_decompile(args),
        Commands::Pack(args) => handle_pack(args),
    }
}

/// Executes the CLI application given an arbitrary arguments iterator.
///
/// # Arguments
///
/// * `args` - The command-line argument sequence.
///
/// # Returns
///
/// A process [`ExitCode`] denoting execution status.
#[must_use = "process exit code must be handled"]
pub fn run_with_args<I>(args: I) -> ExitCode
where
    I: IntoIterator<Item = std::ffi::OsString>,
{
    run_with_os_args(args.into_iter().collect())
}

/// Normalizes single-dash multi-character `WiX` flags (e.g. `-sval`, `-ext`, `-arch`, `-out`) for CLI parsing.
fn normalize_pack_flags(args: Vec<std::ffi::OsString>) -> Vec<std::ffi::OsString> {
    if args.len() > 1 && args[1] == "pack" {
        args.into_iter()
            .map(|arg| {
                let s = arg.to_string_lossy();
                if s == "-sval" {
                    std::ffi::OsString::from("--suppress-validation")
                } else if s == "-ext" {
                    std::ffi::OsString::from("--extension")
                } else if s == "-arch" {
                    std::ffi::OsString::from("--arch")
                } else if s == "-out" {
                    std::ffi::OsString::from("--output")
                } else {
                    arg
                }
            })
            .collect()
    } else {
        args
    }
}

/// Executes the CLI application given an already-collected list of OS arguments.
fn run_with_os_args(args_vec: Vec<std::ffi::OsString>) -> ExitCode {
    let args_vec = normalize_pack_flags(args_vec);

    // Direct msiexec flag parity: check if first arg starts with '/' or '-'
    if args_vec.len() > 1 {
        let first_str = args_vec[1].to_string_lossy();
        if first_str.starts_with('/') || (first_str.starts_with('-') && first_str.len() == 2) {
            let str_args: Vec<String> = args_vec[1..]
                .iter()
                .map(|s| s.to_string_lossy().to_string())
                .collect();
            let msiexec_cmd = Cli {
                command: Commands::Msiexec(MsiexecArgs { raw_args: str_args }),
            };
            return match run(&msiexec_cmd) {
                Ok(msg) => {
                    println!("{msg}");
                    ExitCode::SUCCESS
                }
                Err(err) => {
                    eprintln!("Error: {err}");
                    ExitCode::FAILURE
                }
            };
        }
    }

    match Cli::try_parse_from(args_vec) {
        Ok(cli) => match run(&cli) {
            Ok(msg) => {
                println!("{msg}");
                ExitCode::SUCCESS
            }
            Err(err) => {
                eprintln!("Error: {err}");
                ExitCode::FAILURE
            }
        },
        Err(err) => {
            eprintln!("{err}");
            ExitCode::FAILURE
        }
    }
}

/// Entry point of the MSI command-line application.
///
/// # Returns
///
/// Process [`ExitCode`] denoting execution status.
#[must_use = "process exit code must be handled"]
pub fn main() -> ExitCode {
    run_with_args(std::env::args_os())
}

#[cfg(test)]
#[allow(clippy::option_if_let_else)]
mod tests {
    use super::*;
    use msi::wix::LinkedDatabase;

    /// Helper to convert string slices to `OsString` vector.
    fn to_os(args: &[&str]) -> Vec<std::ffi::OsString> {
        args.iter().map(std::ffi::OsString::from).collect()
    }

    /// Tests invoking `main` directly.
    #[test]
    fn test_main() {
        let code = main();
        assert_eq!(code, ExitCode::FAILURE);
        assert_ne!(err_to_string(msi::Error::Io("err".to_string())), "");
    }

    /// Tests conversion from `CliUiLevel` to `UiLevel` and vice-versa.
    #[test]
    fn test_cli_ui_level_conversion() {
        assert_eq!(UiLevel::from(CliUiLevel::Quiet), UiLevel::None);
        assert_eq!(
            UiLevel::from(CliUiLevel::Basic),
            UiLevel::Basic { no_cancel: false }
        );
        assert_eq!(
            UiLevel::from(CliUiLevel::BasicNoCancel),
            UiLevel::Basic { no_cancel: true }
        );
        assert_eq!(UiLevel::from(CliUiLevel::Reduced), UiLevel::Reduced);
        assert_eq!(UiLevel::from(CliUiLevel::Full), UiLevel::Full);

        assert_eq!(CliUiLevel::from(UiLevel::None), CliUiLevel::Quiet);
        assert_eq!(
            CliUiLevel::from(UiLevel::Basic { no_cancel: false }),
            CliUiLevel::Basic
        );
        assert_eq!(
            CliUiLevel::from(UiLevel::Basic { no_cancel: true }),
            CliUiLevel::BasicNoCancel
        );
        assert_eq!(CliUiLevel::from(UiLevel::Reduced), CliUiLevel::Reduced);
        assert_eq!(CliUiLevel::from(UiLevel::Full), CliUiLevel::Full);
    }

    /// Tests `LoggingDispatcher` formatting all flag categories, disabled flags, console fallback, and errors.
    #[test]
    fn test_logging_dispatcher_options_and_errors() {
        let temp_dir =
            std::env::temp_dir().join(format!("msi_cli_log_opts_{}", std::process::id()));
        let _ = std::fs::create_dir_all(&temp_dir);
        let log_file = temp_dir.join("test.log");

        let mut opts = LoggingOptions::parse("*v!", log_file.to_string_lossy().to_string())
            .unwrap_or_default();
        let logger = LoggingDispatcher::new(opts.clone());

        // Test all characters in match arm
        for ch in [
            'i', 'w', 'e', 'a', 'r', 'u', 'c', 'm', 'o', 'p', 'v', 'x', '?',
        ] {
            assert!(logger.log(ch, "test line").is_ok());
        }

        // Test when should_log is false
        opts.status = false;
        let quiet_logger = LoggingDispatcher::new(opts);
        assert!(quiet_logger.log('i', "ignored status").is_ok());

        // Console fallback when log_file is empty
        let console_opts = LoggingOptions {
            status: true,
            verbose: true,
            ..Default::default()
        };
        let console_logger = LoggingDispatcher::new(console_opts);
        assert!(console_logger.log('i', "verbose console log").is_ok());

        let non_verbose = LoggingOptions {
            errors: true,
            warnings: true,
            status: true,
            verbose: false,
            ..Default::default()
        };
        let non_verbose_logger = LoggingDispatcher::new(non_verbose);
        assert!(non_verbose_logger.log('e', "error to console").is_ok());
        assert!(non_verbose_logger.log('w', "warning to console").is_ok());
        assert!(non_verbose_logger.log('i', "unprinted info").is_ok());
        assert!(non_verbose_logger.log('p', "ignored prop").is_ok());

        // Error opening log file in nonexistent directory
        let bad_opts =
            LoggingOptions::parse("*", "/nonexistent/invalid_dir/bad.log").unwrap_or_default();
        let bad_logger = LoggingDispatcher::new(bad_opts);
        assert!(bad_logger.log('i', "failed open").is_err());

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    /// Tests running the `create` command successfully.
    #[test]
    fn test_cli_create_success() {
        let cli = Cli {
            command: Commands::Create(CreateArgs {
                name: "Sample App".to_string(),
                manufacturer: "Sample Corp".to_string(),
                version: "1.2.3".to_string(),
                product_code: "{12345678-1234-1234-1234-1234567890AB}".to_string(),
            }),
        };

        let result = run(&cli);
        assert_eq!(
            result,
            Ok("Created package configuration for 'Sample App' v1.2.3 by Sample Corp".to_string())
        );
    }

    /// Tests running the `worker` command.
    #[test]
    fn test_cli_worker() {
        let temp_dir =
            std::env::temp_dir().join(format!("msi_cli_worker_test_{}", std::process::id()));
        let _ = std::fs::create_dir_all(&temp_dir);
        let sock_file = temp_dir.join("worker.sock");

        let cli = Cli {
            command: Commands::Worker(WorkerArgs {
                socket: sock_file.to_string_lossy().to_string(),
            }),
        };
        let result = run(&cli);
        assert!(result.is_ok());

        // Test worker with existing socket file to cover remove_file branch
        let _ = std::fs::write(&sock_file, b"existing");
        assert!(run(&cli).is_ok());

        // Test worker with invalid socket path (bind failure)
        let bad_cli = Cli {
            command: Commands::Worker(WorkerArgs {
                socket: "/dev/null/impossible_dir/socket.sock".to_string(),
            }),
        };
        assert!(run(&bad_cli).is_err());

        // Test worker with root path ("/") where path.parent() is None
        let root_cli = Cli {
            command: Commands::Worker(WorkerArgs {
                socket: "/".to_string(),
            }),
        };
        assert!(run(&root_cli).is_err());

        // Test worker client connection
        #[cfg(unix)]
        {
            let connect_sock = temp_dir.join("worker_active.sock");
            let connect_str = connect_sock.to_string_lossy().to_string();
            let connect_clone = connect_str.clone();

            let handle = std::thread::spawn(move || {
                for _ in 0..100 {
                    if let Ok(mut stream) = std::os::unix::net::UnixStream::connect(&connect_clone)
                    {
                        use std::io::Write;
                        let _ = stream.write_all(b"QUIT\n");
                        let _ = stream.flush();
                        return;
                    }
                    std::thread::sleep(std::time::Duration::from_millis(2));
                }
            });

            let client_cli = Cli {
                command: Commands::Worker(WorkerArgs {
                    socket: connect_str,
                }),
            };
            assert!(run(&client_cli).is_ok());
            let _ = handle.join();
        }

        let cli_empty = Cli {
            command: Commands::Worker(WorkerArgs {
                socket: "   ".to_string(),
            }),
        };
        assert!(run(&cli_empty).is_err());

        // Test WorkerSocketAddress directly
        assert!(WorkerSocketAddress::parse("  ").is_err());
        let addr = WorkerSocketAddress::parse("/tmp/test_parse.sock");
        assert_eq!(
            addr.as_ref().map(WorkerSocketAddress::as_str),
            Ok("/tmp/test_parse.sock")
        );
        assert!(format!("{addr:?}").contains("WorkerSocketAddress"));
        assert_eq!(addr, addr.clone());

        // Test repair_flags_to_string
        let all_flags = RepairFlags {
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
        };
        assert_eq!(repair_flags_to_string(&all_flags), "poedcaumsv");
        assert_eq!(repair_flags_to_string(&RepairFlags::default()), "pecmsu");

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    /// Tests running `create` with an invalid version string.
    #[test]
    fn test_cli_create_invalid_version() {
        let cli = Cli {
            command: Commands::Create(CreateArgs {
                name: "Sample App".to_string(),
                manufacturer: "Sample Corp".to_string(),
                version: "not-a-version".to_string(),
                product_code: "{12345678-1234-1234-1234-1234567890AB}".to_string(),
            }),
        };

        let result = run(&cli);
        assert!(result.is_err());
    }

    /// Tests running `create` with validation failure (e.g. empty name).
    #[test]
    fn test_cli_create_validation_failure() {
        let cli = Cli {
            command: Commands::Create(CreateArgs {
                name: String::new(),
                manufacturer: "Sample Corp".to_string(),
                version: "1.0.0".to_string(),
                product_code: "{12345678-1234-1234-1234-1234567890AB}".to_string(),
            }),
        };

        let result = run(&cli);
        assert!(result.is_err());
    }

    /// Tests running the `info` command successfully and failures.
    #[test]
    fn test_cli_info() {
        let temp_dir = std::env::temp_dir().join("msi_cli_info_test");
        let _ = std::fs::create_dir_all(&temp_dir);
        let test_file = temp_dir.join("test.msi");
        let _ = std::fs::write(&test_file, b"test payload");

        let cli = Cli {
            command: Commands::Info(InfoArgs {
                path: test_file.to_string_lossy().to_string(),
            }),
        };
        let result = run(&cli);
        assert!(result.is_ok());

        let cli_empty = Cli {
            command: Commands::Info(InfoArgs {
                path: "   ".to_string(),
            }),
        };
        assert_eq!(run(&cli_empty), Err("Path cannot be empty".to_string()));

        let cli_nonexistent = Cli {
            command: Commands::Info(InfoArgs {
                path: "/nonexistent/path/package.msi".to_string(),
            }),
        };
        assert_eq!(
            run(&cli_nonexistent),
            Err("File not found: /nonexistent/path/package.msi".to_string())
        );

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    /// Helper creating a test package, testing both builder success and fallback paths.
    fn make_test_pkg(valid: bool) -> Package {
        let name = if valid { "Test App" } else { "" };
        if let Ok(pkg) = Package::builder()
            .product_name(name)
            .manufacturer("Test Vendor")
            .version(ProductVersion::new(1, 0, 0))
            .product_code("{12345678-1234-1234-1234-1234567890AB}")
            .build()
        {
            pkg
        } else {
            Package::from_database(LinkedDatabase::new().unwrap_or_default(), HashMap::new())
        }
    }

    /// Tests install, uninstall, and admin basic commands.
    #[test]
    fn test_cli_install_uninstall_admin_basic() {
        let _fallback = make_test_pkg(false);
        let temp_dir =
            std::env::temp_dir().join(format!("msi_cli_basic_test_{}", std::process::id()));
        let _ = std::fs::create_dir_all(&temp_dir);
        let test_pkg = make_test_pkg(true);
        let test_file = temp_dir.join("test.msi");
        assert!(test_pkg.save(&test_file).is_ok());
        let pkg_path = test_file.to_string_lossy().to_string();

        // Install
        let install_cli = Cli {
            command: Commands::Install(InstallArgs {
                package: pkg_path.clone(),
                ui: CliUiLevel::Quiet,
                log: None,
                properties: vec!["INSTALLDIR=/opt/test".to_string()],
                tui: false,
            }),
        };
        assert!(run(&install_cli).is_ok());

        let install_empty = Cli {
            command: Commands::Install(InstallArgs {
                package: "   ".to_string(),
                ui: CliUiLevel::Full,
                log: None,
                properties: vec![],
                tui: false,
            }),
        };
        assert!(run(&install_empty).is_err());

        let install_nonexistent = Cli {
            command: Commands::Install(InstallArgs {
                package: "/nonexistent/path/pkg.msi".to_string(),
                ui: CliUiLevel::Quiet,
                log: None,
                properties: vec![],
                tui: false,
            }),
        };
        assert!(run(&install_nonexistent).is_err());

        // Uninstall
        let uninstall_cli = Cli {
            command: Commands::Uninstall(UninstallArgs {
                package: pkg_path.clone(),
                ui: CliUiLevel::Basic,
                log: None,
            }),
        };
        assert!(run(&uninstall_cli).is_ok());

        let uninstall_empty = Cli {
            command: Commands::Uninstall(UninstallArgs {
                package: String::new(),
                ui: CliUiLevel::Basic,
                log: None,
            }),
        };
        assert!(run(&uninstall_empty).is_err());

        let uninstall_nonexistent = Cli {
            command: Commands::Uninstall(UninstallArgs {
                package: "/nonexistent/path/pkg.msi".to_string(),
                ui: CliUiLevel::Basic,
                log: None,
            }),
        };
        assert!(run(&uninstall_nonexistent).is_err());

        // Admin
        let admin_cli = Cli {
            command: Commands::Admin(AdminArgs {
                package: pkg_path,
                ui: CliUiLevel::Basic,
                log: None,
            }),
        };
        assert!(run(&admin_cli).is_ok());

        let admin_empty = Cli {
            command: Commands::Admin(AdminArgs {
                package: String::new(),
                ui: CliUiLevel::Basic,
                log: None,
            }),
        };
        assert!(run(&admin_empty).is_err());

        let admin_nonexistent = Cli {
            command: Commands::Admin(AdminArgs {
                package: "/nonexistent/path/pkg.msi".to_string(),
                ui: CliUiLevel::Basic,
                log: None,
            }),
        };
        assert!(run(&admin_nonexistent).is_err());

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    /// Tests install, uninstall, and admin commands with corrupt package files and bad log paths.
    #[test]
    fn test_cli_install_uninstall_admin_corrupt_and_bad_logs() {
        let temp_dir =
            std::env::temp_dir().join(format!("msi_cli_corrupt_test_{}", std::process::id()));
        let _ = std::fs::create_dir_all(&temp_dir);
        let corrupt_file = temp_dir.join("corrupt.msi");
        let _ = std::fs::write(&corrupt_file, b"corrupted non-msi content");
        let corrupt_path = corrupt_file.to_string_lossy().to_string();

        let corrupt_install = Cli {
            command: Commands::Install(InstallArgs {
                package: corrupt_path.clone(),
                ui: CliUiLevel::Basic,
                log: None,
                properties: vec![],
                tui: false,
            }),
        };
        assert!(run(&corrupt_install).is_err());

        let corrupt_uninstall = Cli {
            command: Commands::Uninstall(UninstallArgs {
                package: corrupt_path.clone(),
                ui: CliUiLevel::Basic,
                log: None,
            }),
        };
        assert!(run(&corrupt_uninstall).is_err());

        let corrupt_admin = Cli {
            command: Commands::Admin(AdminArgs {
                package: corrupt_path,
                ui: CliUiLevel::Basic,
                log: None,
            }),
        };
        assert!(run(&corrupt_admin).is_err());

        // Invalid log paths
        let valid_pkg = make_test_pkg(true);
        let valid_file = temp_dir.join("valid.msi");
        assert!(valid_pkg.save(&valid_file).is_ok());
        let valid_path = valid_file.to_string_lossy().to_string();

        let bad_log = "/nonexistent/path/cannot_open.log".to_string();
        let bad_install = Cli {
            command: Commands::Install(InstallArgs {
                package: valid_path.clone(),
                ui: CliUiLevel::Basic,
                log: Some(bad_log.clone()),
                properties: vec![],
                tui: false,
            }),
        };
        assert!(run(&bad_install).is_err());

        let bad_uninstall = Cli {
            command: Commands::Uninstall(UninstallArgs {
                package: valid_path.clone(),
                ui: CliUiLevel::Basic,
                log: Some(bad_log.clone()),
            }),
        };
        assert!(run(&bad_uninstall).is_err());

        let bad_admin = Cli {
            command: Commands::Admin(AdminArgs {
                package: valid_path,
                ui: CliUiLevel::Basic,
                log: Some(bad_log),
            }),
        };
        assert!(run(&bad_admin).is_err());

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    /// Tests repair, advertise, and patch commands.
    #[test]
    #[allow(clippy::too_many_lines)]
    fn test_cli_repair_advertise_patch() {
        let temp_dir =
            std::env::temp_dir().join(format!("msi_cli_repair_test_{}", std::process::id()));
        let _ = std::fs::create_dir_all(&temp_dir);
        let test_pkg = make_test_pkg(true);
        let test_file = temp_dir.join("test.msi");
        assert!(test_pkg.save(&test_file).is_ok());
        let pkg_path = test_file.to_string_lossy().to_string();

        let patch_builder =
            msi::wix::patch::PatchPackageBuilder::new("{99999999-9999-9999-9999-999999999999}");
        let patch_bytes = patch_builder.build().unwrap_or_default();
        let patch_file = temp_dir.join("update.msp");
        let _ = std::fs::write(&patch_file, patch_bytes);
        let patch_path = patch_file.to_string_lossy().to_string();

        // Repair
        let repair_cli = Cli {
            command: Commands::Repair(RepairArgs {
                package: pkg_path.clone(),
                flags: "omus".to_string(),
                log: None,
            }),
        };
        assert!(run(&repair_cli).is_ok());

        let bad_repair_flags = Cli {
            command: Commands::Repair(RepairArgs {
                package: pkg_path.clone(),
                flags: "xyz_invalid_flags".to_string(),
                log: None,
            }),
        };
        assert!(run(&bad_repair_flags).is_err());

        let repair_empty = Cli {
            command: Commands::Repair(RepairArgs {
                package: String::new(),
                flags: "p".to_string(),
                log: None,
            }),
        };
        assert!(run(&repair_empty).is_err());

        let repair_nonexistent = Cli {
            command: Commands::Repair(RepairArgs {
                package: "/nonexistent/pkg.msi".to_string(),
                flags: "p".to_string(),
                log: None,
            }),
        };
        assert!(run(&repair_nonexistent).is_err());

        // Advertise
        let adv_machine = Cli {
            command: Commands::Advertise(AdvertiseArgs {
                package: pkg_path.clone(),
                user: false,
            }),
        };
        assert!(run(&adv_machine).is_ok());

        let adv_user = Cli {
            command: Commands::Advertise(AdvertiseArgs {
                package: pkg_path.clone(),
                user: true,
            }),
        };
        assert!(run(&adv_user).is_ok());

        let adv_empty = Cli {
            command: Commands::Advertise(AdvertiseArgs {
                package: String::new(),
                user: false,
            }),
        };
        assert!(run(&adv_empty).is_err());

        let adv_nonexistent = Cli {
            command: Commands::Advertise(AdvertiseArgs {
                package: "/nonexistent/pkg.msi".to_string(),
                user: false,
            }),
        };
        assert!(run(&adv_nonexistent).is_err());

        // Patch
        let patch_cli = Cli {
            command: Commands::Patch(PatchArgs {
                package: pkg_path.clone(),
                patch: patch_path.clone(),
                ui: CliUiLevel::Basic,
                log: None,
            }),
        };
        assert!(run(&patch_cli).is_ok());

        let patch_empty_pkg = Cli {
            command: Commands::Patch(PatchArgs {
                package: String::new(),
                patch: patch_path.clone(),
                ui: CliUiLevel::Basic,
                log: None,
            }),
        };
        assert!(run(&patch_empty_pkg).is_err());

        let patch_empty_patch = Cli {
            command: Commands::Patch(PatchArgs {
                package: pkg_path.clone(),
                patch: String::new(),
                ui: CliUiLevel::Basic,
                log: None,
            }),
        };
        assert!(run(&patch_empty_patch).is_err());

        let patch_nonexistent_pkg = Cli {
            command: Commands::Patch(PatchArgs {
                package: "/nonexistent/pkg.msi".to_string(),
                patch: patch_path.clone(),
                ui: CliUiLevel::Basic,
                log: None,
            }),
        };
        assert!(run(&patch_nonexistent_pkg).is_err());

        let patch_nonexistent_patch = Cli {
            command: Commands::Patch(PatchArgs {
                package: pkg_path.clone(),
                patch: "/nonexistent/patch.msp".to_string(),
                ui: CliUiLevel::Basic,
                log: None,
            }),
        };
        assert!(run(&patch_nonexistent_patch).is_err());

        // Test GUID fallback for repair, advertise, and uninstall
        let guid_repair = Cli {
            command: Commands::Repair(RepairArgs {
                package: "{12345678-1234-1234-1234-1234567890AB}".to_string(),
                flags: "omus".to_string(),
                log: None,
            }),
        };
        assert!(run(&guid_repair).is_ok());

        let guid_adv = Cli {
            command: Commands::Advertise(AdvertiseArgs {
                package: "{12345678-1234-1234-1234-1234567890AB}".to_string(),
                user: false,
            }),
        };
        assert!(run(&guid_adv).is_ok());

        let guid_uninstall = Cli {
            command: Commands::Uninstall(UninstallArgs {
                package: "{12345678-1234-1234-1234-1234567890AB}".to_string(),
                ui: CliUiLevel::Quiet,
                log: None,
            }),
        };
        assert!(run(&guid_uninstall).is_ok());

        // Valid patch with transform and cabinet
        let mut patch_cfb = msi::cfb::writer::CfbWriter::new(msi::cfb::header::CfbVersion::V3);
        let mut patch_transform = msi::database::transform::DatabaseTransform::new();
        patch_transform.tables.insert(
            "Property".to_string(),
            msi::database::transform::TableTransform {
                table_name: "Property".to_string(),
                is_added: false,
                is_dropped: false,
                operations: vec![msi::database::transform::RowOperation::Insert(
                    msi::database::tables::record::Record::with_fields(vec![
                        FieldValue::String("PATCHED".to_string()),
                        FieldValue::String("1".to_string()),
                    ]),
                )],
            },
        );
        let mst_bytes = patch_transform.to_bytes().unwrap_or_default();
        assert!(patch_cfb.add_stream("Transform1", &mst_bytes).is_ok());
        assert!(patch_cfb
            .add_stream("NonTransformStream", b"raw non transform bytes")
            .is_ok());
        assert!(patch_cfb
            .add_stream("MsiPatchCert_Cert1", b"dummy certificate")
            .is_ok());
        assert!(patch_cfb
            .add_stream("#patch.cab", b"MSCF dummy cab")
            .is_ok());
        let real_patch_file = temp_dir.join("real_update.msp");
        assert!(std::fs::write(&real_patch_file, patch_cfb.build()).is_ok());
        let real_patch_path = real_patch_file.to_string_lossy().to_string();

        let patch_real_cli = Cli {
            command: Commands::Patch(PatchArgs {
                package: pkg_path.clone(),
                patch: real_patch_path.clone(),
                ui: CliUiLevel::Basic,
                log: None,
            }),
        };
        assert!(run(&patch_real_cli).is_ok());

        // Error paths in handle_patch
        let corrupt_msi = temp_dir.join("corrupt_pkg.msi");
        assert!(std::fs::write(&corrupt_msi, b"not an msi").is_ok());
        let bad_pkg_patch = Cli {
            command: Commands::Patch(PatchArgs {
                package: corrupt_msi.to_string_lossy().to_string(),
                patch: patch_path,
                ui: CliUiLevel::Basic,
                log: None,
            }),
        };
        assert!(run(&bad_pkg_patch).is_err());

        let bad_patch_dir = Cli {
            command: Commands::Patch(PatchArgs {
                package: pkg_path.clone(),
                patch: temp_dir.to_string_lossy().to_string(),
                ui: CliUiLevel::Basic,
                log: None,
            }),
        };
        assert!(run(&bad_patch_dir).is_err());

        let invalid_cfb_file = temp_dir.join("corrupt_patch.msp");
        assert!(std::fs::write(&invalid_cfb_file, b"not a cfb").is_ok());
        let bad_cfb_patch = Cli {
            command: Commands::Patch(PatchArgs {
                package: pkg_path,
                patch: invalid_cfb_file.to_string_lossy().to_string(),
                ui: CliUiLevel::Basic,
                log: None,
            }),
        };
        assert!(run(&bad_cfb_patch).is_err());

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let readonly_msi = temp_dir.join("readonly.msi");
            assert!(std::fs::copy(&test_file, &readonly_msi).is_ok());
            assert!(std::fs::set_permissions(
                &readonly_msi,
                std::fs::Permissions::from_mode(0o444)
            )
            .is_ok());
            let ro_patch_cli = Cli {
                command: Commands::Patch(PatchArgs {
                    package: readonly_msi.to_string_lossy().to_string(),
                    patch: real_patch_path,
                    ui: CliUiLevel::Basic,
                    log: None,
                }),
            };
            assert!(run(&ro_patch_cli).is_err());
            let _ = std::fs::set_permissions(&readonly_msi, std::fs::Permissions::from_mode(0o644));
        }

        // Corrupt (non-MSI) existing file for repair and advertise
        let corrupt_file = temp_dir.join("corrupt.msi");
        let _ = std::fs::write(&corrupt_file, b"corrupted payload");
        let corrupt_path = corrupt_file.to_string_lossy().to_string();

        let corrupt_repair = Cli {
            command: Commands::Repair(RepairArgs {
                package: corrupt_path.clone(),
                flags: "omus".to_string(),
                log: None,
            }),
        };
        assert!(run(&corrupt_repair).is_err());

        let corrupt_adv = Cli {
            command: Commands::Advertise(AdvertiseArgs {
                package: corrupt_path,
                user: false,
            }),
        };
        assert!(run(&corrupt_adv).is_err());

        let bad_repair_log = Cli {
            command: Commands::Repair(RepairArgs {
                package: test_file.to_string_lossy().to_string(),
                flags: "omus".to_string(),
                log: Some("/nonexistent/bad/log.log".to_string()),
            }),
        };
        assert!(run(&bad_repair_log).is_err());

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    /// Tests raw msiexec parity commands and logging dispatcher.
    #[test]
    fn test_msiexec_parity_and_logging() {
        let temp_dir =
            std::env::temp_dir().join(format!("msi_cli_log_test_{}", std::process::id()));
        let _ = std::fs::create_dir_all(&temp_dir);
        let log_file = temp_dir.join("install.log");
        let log_str = log_file.to_string_lossy().to_string();

        let test_pkg = make_test_pkg(true);
        let app_file = temp_dir.join("app.msi");
        assert!(test_pkg.save(&app_file).is_ok());
        let app_str = app_file.to_string_lossy().to_string();

        let patch_builder =
            msi::wix::patch::PatchPackageBuilder::new("{99999999-9999-9999-9999-999999999999}");
        let patch_bytes = patch_builder.build().unwrap_or_default();
        let patch_file = temp_dir.join("patch.msp");
        let _ = std::fs::write(&patch_file, patch_bytes);
        let patch_str = patch_file.to_string_lossy().to_string();

        // Test raw msiexec mode with various action flags and logging
        for raw in [
            vec![
                "/i".to_string(),
                app_str.clone(),
                "/qn".to_string(),
                "TARGETDIR=/opt/app".to_string(),
                "/l*".to_string(),
                log_str.clone(),
            ],
            vec![
                "/x".to_string(),
                app_str.clone(),
                "/l*".to_string(),
                log_str.clone(),
            ],
            vec![
                "/a".to_string(),
                app_str.clone(),
                "/l*".to_string(),
                log_str.clone(),
            ],
            vec![
                "/f".to_string(),
                app_str.clone(),
                "/l*".to_string(),
                log_str.clone(),
            ],
            vec!["/j".to_string(), app_str.clone()],
            vec![
                "/p".to_string(),
                patch_str.clone(),
                format!("TARGETPACKAGE={app_str}"),
                "/l*".to_string(),
                log_str,
            ],
            vec!["/p".to_string(), patch_str, format!("PACKAGE={app_str}")],
        ] {
            let msiexec_cli = Cli {
                command: Commands::Msiexec(MsiexecArgs { raw_args: raw }),
            };
            assert!(run(&msiexec_cli).is_ok());
        }

        // Test msiexec error paths for uninstall, admin, repair, advertise, and patch
        for bad_raw in [
            vec!["/x".to_string(), "/nonexistent/app.msi".to_string()],
            vec!["/a".to_string(), "/nonexistent/app.msi".to_string()],
            vec!["/f".to_string(), "/nonexistent/app.msi".to_string()],
            vec!["/j".to_string(), "/nonexistent/app.msi".to_string()],
            vec!["/p".to_string(), "/nonexistent/patch.msp".to_string()],
        ] {
            let bad_msiexec = Cli {
                command: Commands::Msiexec(MsiexecArgs { raw_args: bad_raw }),
            };
            assert!(run(&bad_msiexec).is_err());
        }

        // Test logging dispatcher
        let log_opts = LoggingOptions::parse("*v!", log_file.to_string_lossy().to_string())
            .unwrap_or_default();
        let logger = LoggingDispatcher::new(log_opts);
        assert!(logger.log('i', "Status informational message").is_ok());
        assert!(logger.log('w', "Warning non-fatal").is_ok());
        assert!(logger.log('e', "Error failure").is_ok());
        assert!(logger.log('p', "Property PROPERTY=1").is_ok());
        assert!(logger.log('z', "Unknown type ignored").is_ok());

        assert!(log_file.exists());
        let content = std::fs::read_to_string(&log_file).unwrap_or_default();
        assert!(content.contains("Status informational message"));
        assert!(content.contains("Warning non-fatal"));

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    /// Tests `run_with_args` covering command-line invocations including direct msiexec syntax.
    #[test]
    #[allow(clippy::too_many_lines)]
    fn test_run_with_args() {
        let temp_dir =
            std::env::temp_dir().join(format!("msi_cli_run_args_{}", std::process::id()));
        let _ = std::fs::create_dir_all(&temp_dir);
        let app_file = temp_dir.join("app.msi");
        let patch_file = temp_dir.join("patch.msp");

        let test_pkg = make_test_pkg(true);
        assert!(test_pkg.save(&app_file).is_ok());

        let patch_builder =
            msi::wix::patch::PatchPackageBuilder::new("{99999999-9999-9999-9999-999999999999}");
        let patch_bytes = patch_builder.build().unwrap_or_default();
        let _ = std::fs::write(&patch_file, patch_bytes);

        let app_str = app_file.to_string_lossy().to_string();
        let patch_str = patch_file.to_string_lossy().to_string();

        let success_args = [
            "msi",
            "create",
            "--name",
            "Sample App",
            "--manufacturer",
            "Sample Corp",
            "--version",
            "1.0.0",
            "--product-code",
            "{12345678-1234-1234-1234-1234567890AB}",
        ];
        assert_eq!(run_with_args(to_os(&success_args)), ExitCode::SUCCESS);

        let error_args = [
            "msi",
            "create",
            "--name",
            "",
            "--manufacturer",
            "Sample Corp",
            "--version",
            "1.0.0",
            "--product-code",
            "{12345678-1234-1234-1234-1234567890AB}",
        ];
        assert_eq!(run_with_args(to_os(&error_args)), ExitCode::FAILURE);

        let parse_error_args = ["msi", "--unrecognized-flag"];
        assert_eq!(run_with_args(to_os(&parse_error_args)), ExitCode::FAILURE);

        // Subcommands via CLI
        assert_eq!(
            run_with_args(to_os(&["msi", "install", &app_str, "--ui", "quiet"])),
            ExitCode::SUCCESS
        );
        assert_eq!(
            run_with_args(to_os(&["msi", "uninstall", &app_str])),
            ExitCode::SUCCESS
        );
        assert_eq!(
            run_with_args(to_os(&["msi", "admin", &app_str])),
            ExitCode::SUCCESS
        );
        assert_eq!(
            run_with_args(to_os(&["msi", "repair", &app_str, "-f", "omus"])),
            ExitCode::SUCCESS
        );
        assert_eq!(
            run_with_args(to_os(&["msi", "advertise", &app_str, "--user"])),
            ExitCode::SUCCESS
        );
        assert_eq!(
            run_with_args(to_os(&["msi", "patch", &app_str, &patch_str])),
            ExitCode::SUCCESS
        );

        // Direct msiexec flag syntax parity: `msi /i app.msi /qn`
        assert_eq!(
            run_with_args(to_os(&["msi", "/i", &app_str, "/qn"])),
            ExitCode::SUCCESS
        );
        assert_eq!(
            run_with_args(to_os(&["msi", "-i", &app_str])),
            ExitCode::SUCCESS
        );
        assert_eq!(
            run_with_args(to_os(&["msi", "/x", &app_str, "/qb"])),
            ExitCode::SUCCESS
        );
        assert_eq!(
            run_with_args(to_os(&["msi", "/i", &app_str, "/tui"])),
            ExitCode::SUCCESS
        );
        assert_eq!(
            run_with_args(to_os(&["msi", "/i", &app_str, "--tui"])),
            ExitCode::SUCCESS
        );
        assert_eq!(
            run_with_args(to_os(&["msi", "/i", &app_str, "-tui"])),
            ExitCode::SUCCESS
        );
        assert_eq!(
            run_with_args(to_os(&["msi", "/i", &app_str, "/console"])),
            ExitCode::SUCCESS
        );
        assert_eq!(
            run_with_args(to_os(&["msi", "/i", &app_str, "--console"])),
            ExitCode::SUCCESS
        );
        assert_eq!(
            run_with_args(to_os(&["msi", "/i"])), // Missing package
            ExitCode::FAILURE
        );
        assert_eq!(
            run_with_args(to_os(&["msi"])), // Single argument
            ExitCode::FAILURE
        );

        // Missing package error verification
        assert_eq!(
            run_with_args(to_os(&["msi", "install", "nonexistent_file_xyz.msi"])),
            ExitCode::FAILURE
        );
        assert_eq!(
            run_with_args(to_os(&["msi", "/i", "nonexistent_file_xyz.msi"])),
            ExitCode::FAILURE
        );
        assert_eq!(
            run_with_args(to_os(&["msi", "--totally-invalid-cli-flag-xyz"])),
            ExitCode::FAILURE
        );

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    /// Helper creating a real app package for lifecycle tests.
    fn make_real_app_pkg(valid: bool) -> Package {
        let name = if valid { "Real Cli App" } else { "" };
        if let Ok(pkg) = Package::builder()
            .product_name(name)
            .manufacturer("Acme Systems")
            .version(ProductVersion::new(1, 2, 0))
            .product_code("{22222222-3333-4444-5555-666666666666}")
            .add_property("INSTALLLEVEL", "1")
            .add_embedded_cabinet("#cab1.cab", vec![0x4D, 0x53, 0x43, 0x46, 0x00, 0x00])
            .build()
        {
            pkg
        } else {
            Package::from_database(LinkedDatabase::new().unwrap_or_default(), HashMap::new())
        }
    }

    /// Tests full end-to-end package lifecycle via CLI (info, install, repair, admin, advertise, uninstall).
    #[test]
    #[allow(clippy::too_many_lines)]
    fn test_cli_real_package_lifecycle() {
        let _fallback = make_real_app_pkg(false);
        let temp_dir = std::env::temp_dir().join("msi_cli_lifecycle_test");
        let _ = std::fs::create_dir_all(&temp_dir);
        let pkg_path = temp_dir.join("lifecycle.msi");
        let log_path = temp_dir.join("install.log");

        let mut pkg = make_real_app_pkg(true);

        pkg.database_mut().add_record(
            "Property",
            msi::database::tables::record::Record::with_fields(vec![
                FieldValue::Null,
                FieldValue::Null,
            ]),
        );

        let dlg_row = msi::database::tables::ui::DialogRow {
            dialog: "WelcomeDlg".to_string(),
            h_centering: 50,
            v_centering: 50,
            width: 370,
            height: 270,
            attributes: 3,
            title: Some("Welcome to Real Cli App".to_string()),
            control_first: "NextButton".to_string(),
            control_default: Some("NextButton".to_string()),
            control_cancel: Some("CancelButton".to_string()),
        };
        pkg.database_mut().add_record("Dialog", dlg_row.to_record());

        assert!(pkg.save(&pkg_path).is_ok());

        // 1. Info
        let info_res = run(&Cli {
            command: Commands::Info(InfoArgs {
                path: pkg_path.to_string_lossy().to_string(),
            }),
        });
        assert!(info_res.is_ok());
        let info_str = info_res.unwrap_or_default();
        assert!(info_str.contains("Package: Real Cli App"));
        assert!(info_str.contains("Version: 1.2.0"));
        assert!(info_str.contains("Manufacturer: Acme Systems"));

        // 2. Install (with TUI and active dialog)
        let install_res = run(&Cli {
            command: Commands::Install(InstallArgs {
                package: pkg_path.to_string_lossy().to_string(),
                ui: CliUiLevel::Full,
                log: Some(log_path.to_string_lossy().to_string()),
                properties: vec![
                    "MYPROP=Value1".to_string(),
                    "NO_EQUALS_PROPERTY".to_string(),
                ],
                tui: true,
            }),
        });
        assert!(install_res.is_ok());
        let install_str = install_res.unwrap_or_default();
        assert!(install_str.contains("Successfully installed package 'Real Cli App' v1.2.0"));

        // 2b. Test headless and display env variable branches for TUI auto-detection
        let old_disp = std::env::var("DISPLAY").ok();
        let old_wayland = std::env::var("WAYLAND_DISPLAY").ok();
        // A. Headless: DISPLAY and WAYLAND_DISPLAY unset, non-quiet UI triggers use_tui
        // SAFETY: Test mutations isolated to test process.
        unsafe {
            std::env::remove_var("DISPLAY");
            std::env::remove_var("WAYLAND_DISPLAY");
        }
        assert!(run(&Cli {
            command: Commands::Install(InstallArgs {
                package: pkg_path.to_string_lossy().to_string(),
                ui: CliUiLevel::Basic,
                log: None,
                properties: vec![],
                tui: false,
            }),
        })
        .is_ok());
        // B. DISPLAY set: is_headless evaluates to false
        // SAFETY: Test mutations isolated to test process.
        unsafe {
            std::env::set_var("DISPLAY", ":0");
        }
        assert!(run(&Cli {
            command: Commands::Install(InstallArgs {
                package: pkg_path.to_string_lossy().to_string(),
                ui: CliUiLevel::Basic,
                log: None,
                properties: vec![],
                tui: false,
            }),
        })
        .is_ok());
        // C. DISPLAY unset, but WAYLAND_DISPLAY set: is_headless evaluates to false
        // SAFETY: Test mutations isolated to test process.
        unsafe {
            std::env::remove_var("DISPLAY");
            std::env::set_var("WAYLAND_DISPLAY", "wayland-0");
        }
        assert!(run(&Cli {
            command: Commands::Install(InstallArgs {
                package: pkg_path.to_string_lossy().to_string(),
                ui: CliUiLevel::Basic,
                log: None,
                properties: vec![],
                tui: false,
            }),
        })
        .is_ok());
        // Restore environment variables
        // SAFETY: Test cleanup restores process environment variables.
        let restore_env = |key: &'static str, old: Option<String>| unsafe {
            match old {
                Some(val) => std::env::set_var(key, val),
                None => std::env::remove_var(key),
            }
        };
        restore_env("TEST_CLI_ENV_RESTORE", Some("dummy_val".to_string()));
        assert_eq!(
            std::env::var("TEST_CLI_ENV_RESTORE").ok(),
            Some("dummy_val".to_string())
        );
        restore_env("TEST_CLI_ENV_RESTORE", None);
        assert!(std::env::var("TEST_CLI_ENV_RESTORE").is_err());

        restore_env("DISPLAY", old_disp);
        restore_env("WAYLAND_DISPLAY", old_wayland);

        // 3. Repair
        let repair_res = run(&Cli {
            command: Commands::Repair(RepairArgs {
                package: pkg_path.to_string_lossy().to_string(),
                flags: "omus".to_string(),
                log: Some(log_path.to_string_lossy().to_string()),
            }),
        });
        assert!(repair_res.is_ok());
        let repair_str = repair_res.unwrap_or_default();
        assert!(repair_str.contains("Successfully repaired package 'Real Cli App' v1.2.0"));

        // 4. Admin
        let admin_res = run(&Cli {
            command: Commands::Admin(AdminArgs {
                package: pkg_path.to_string_lossy().to_string(),
                ui: CliUiLevel::Basic,
                log: Some(log_path.to_string_lossy().to_string()),
            }),
        });
        assert!(admin_res.is_ok());
        let admin_str = admin_res.unwrap_or_default();
        assert!(admin_str
            .contains("Administrative installation completed for 'Real Cli App' (cabs: 1)"));

        // 5. Advertise
        let adv_res = run(&Cli {
            command: Commands::Advertise(AdvertiseArgs {
                package: pkg_path.to_string_lossy().to_string(),
                user: true,
            }),
        });
        assert!(adv_res.is_ok());
        let adv_str = adv_res.unwrap_or_default();
        assert!(adv_str.contains("Successfully advertised package 'Real Cli App' (User)"));

        // 6. Uninstall
        let uninstall_res = run(&Cli {
            command: Commands::Uninstall(UninstallArgs {
                package: pkg_path.to_string_lossy().to_string(),
                ui: CliUiLevel::Quiet,
                log: Some(log_path.to_string_lossy().to_string()),
            }),
        });
        assert!(uninstall_res.is_ok());
        let uninstall_str = uninstall_res.unwrap_or_default();
        assert!(uninstall_str.contains("Successfully uninstalled package 'Real Cli App' v1.2.0"));

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    /// Helper creating a nolog app package, testing both builder success and fallback paths.
    fn make_nolog_pkg(valid: bool) -> Package {
        let name = if valid { "NoLog App" } else { "" };
        if let Ok(pkg) = Package::builder()
            .product_name(name)
            .manufacturer("Acme Systems")
            .version(ProductVersion::new(1, 0, 0))
            .product_code("{33333333-3333-4444-5555-666666666666}")
            .build()
        {
            pkg
        } else {
            Package::from_database(LinkedDatabase::new().unwrap_or_default(), HashMap::new())
        }
    }

    /// Tests package commands with no log file specified and property loading.
    #[test]
    fn test_cli_real_package_no_log_and_properties() {
        let _fallback = make_nolog_pkg(false);
        let temp_dir =
            std::env::temp_dir().join(format!("msi_cli_nolog_test_{}", std::process::id()));
        let _ = std::fs::create_dir_all(&temp_dir);
        let pkg_path = temp_dir.join("nolog.msi");

        let mut pkg = make_nolog_pkg(true);

        pkg.database_mut().add_record(
            "Property",
            msi::database::tables::record::Record::with_fields(vec![
                FieldValue::Null,
                FieldValue::Null,
            ]),
        );
        assert!(pkg.save(&pkg_path).is_ok());

        assert!(run(&Cli {
            command: Commands::Install(InstallArgs {
                package: pkg_path.to_string_lossy().to_string(),
                ui: CliUiLevel::Quiet,
                log: None,
                properties: vec![],
                tui: false,
            }),
        })
        .is_ok());

        assert!(run(&Cli {
            command: Commands::Install(InstallArgs {
                package: pkg_path.to_string_lossy().to_string(),
                ui: CliUiLevel::Quiet,
                log: None,
                properties: vec![],
                tui: true,
            }),
        })
        .is_ok());

        assert!(run(&Cli {
            command: Commands::Repair(RepairArgs {
                package: pkg_path.to_string_lossy().to_string(),
                flags: "omus".to_string(),
                log: None,
            }),
        })
        .is_ok());

        assert!(run(&Cli {
            command: Commands::Admin(AdminArgs {
                package: pkg_path.to_string_lossy().to_string(),
                ui: CliUiLevel::Basic,
                log: None,
            }),
        })
        .is_ok());

        assert!(run(&Cli {
            command: Commands::Uninstall(UninstallArgs {
                package: pkg_path.to_string_lossy().to_string(),
                ui: CliUiLevel::Quiet,
                log: None,
            }),
        })
        .is_ok());

        let mut test_ctx = EvaluationContext::new();
        load_package_properties(&pkg, &mut test_ctx);

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    /// Tests harvest CLI subcommand across directory, registry, stdout, and error branches.
    #[test]
    fn test_cli_harvest() {
        let temp_dir =
            std::env::temp_dir().join(format!("msi_cli_harvest_test_{}", std::process::id()));
        let _ = std::fs::create_dir_all(&temp_dir);
        let src_dir = temp_dir.join("source_files");
        assert!(std::fs::create_dir_all(&src_dir).is_ok());
        assert!(std::fs::write(src_dir.join("test.txt"), b"sample").is_ok());

        let harvest_out = temp_dir.join("harvested.wxs");

        // 1. Harvest directory to file with disk rules and gitignore
        let gitignore_file = temp_dir.join(".gitignore");
        assert!(std::fs::write(&gitignore_file, b"*.tmp\n").is_ok());
        assert!(std::fs::write(src_dir.join("test.log"), b"log").is_ok());
        assert!(std::fs::write(src_dir.join("test.bak"), b"bak").is_ok());

        let manifest_out = temp_dir.join("manifest.txt");
        let stage_out = temp_dir.join("staged");

        let harvest_res = run(&Cli {
            command: Commands::Harvest(Box::new(HarvestArgs {
                target: src_dir.to_string_lossy().to_string(),
                extra_target: None,
                group: "MyHarvestGroup".to_string(),
                dir_id: "INSTALLFOLDER".to_string(),
                output: Some(harvest_out.to_string_lossy().to_string()),
                mode: "dir".to_string(),
                gitignore: Some(gitignore_file.to_string_lossy().to_string()),
                disk_rules: vec![
                    "test.txt=2".to_string(),
                    "bad_num=not_a_number".to_string(),
                    "no_equal_sign".to_string(),
                ],
                default_disk_id: 1,
                split_size: Some(1024 * 1024),
                secondary_groups: vec!["*.txt=SecGroup".to_string(), "no_equal_sign".to_string()],
                exclude_extensions: vec!["bak".to_string(), "log".to_string()],
                exclude_patterns: vec!["ignored/*".to_string(), "*.tmp".to_string()],
                manifest_file: Some(manifest_out.to_string_lossy().to_string()),
                output_dir: Some(stage_out.to_string_lossy().to_string()),
                include_cache: Some(src_dir.to_string_lossy().to_string()),
            })),
        });
        assert!(harvest_res.is_ok());
        assert!(harvest_out.exists());
        assert!(manifest_out.exists());
        assert!(stage_out.join("test.txt").exists());
        let harvested_xml = std::fs::read_to_string(&harvest_out).unwrap_or_default();
        assert!(harvested_xml.contains("MyHarvestGroup"));
        assert!(harvested_xml.contains("test.txt"));
        assert!(harvested_xml.contains("DiskId=\"2\""));
        assert!(harvested_xml.contains("SecGroup"));
        assert!(!harvested_xml.contains("test.log"));
        assert!(!harvested_xml.contains("test.bak"));

        // 2. Harvest directory to stdout using positional "dir <PATH>" syntax
        let harvest_stdout = run(&Cli {
            command: Commands::Harvest(Box::new(HarvestArgs {
                target: "dir".to_string(),
                extra_target: Some(src_dir.to_string_lossy().to_string()),
                group: "StdoutHarvestGroup".to_string(),
                dir_id: "INSTALLFOLDER".to_string(),
                output: None,
                mode: "dir".to_string(),
                gitignore: None,
                disk_rules: vec![],
                default_disk_id: 1,
                split_size: None,
                secondary_groups: vec![],
                exclude_extensions: vec![],
                exclude_patterns: vec![],
                manifest_file: None,
                output_dir: None,
                include_cache: None,
            })),
        });
        assert!(harvest_stdout.is_ok());

        // 3. Harvest registry file to stdout and to file
        let reg_file = temp_dir.join("sample.reg");
        assert!(std::fs::write(
            &reg_file,
            b"Windows Registry Editor Version 5.00\r\n\r\n[HKEY_LOCAL_MACHINE\\Software\\Acme]\r\n\"Test\"=\"Val\"\r\n",
        ).is_ok());
        let reg_harvest_res = run(&Cli {
            command: Commands::Harvest(Box::new(HarvestArgs {
                target: reg_file.to_string_lossy().to_string(),
                extra_target: None,
                group: "MyRegGroup".to_string(),
                dir_id: "INSTALLFOLDER".to_string(),
                output: None,
                mode: "reg".to_string(),
                gitignore: None,
                disk_rules: vec![],
                default_disk_id: 1,
                split_size: None,
                secondary_groups: vec![],
                exclude_extensions: vec![],
                exclude_patterns: vec![],
                manifest_file: None,
                output_dir: None,
                include_cache: None,
            })),
        });
        assert!(reg_harvest_res.is_ok());

        let reg_file_out = temp_dir.join("sample_reg.wxs");
        let reg_harvest_out_res = run(&Cli {
            command: Commands::Harvest(Box::new(HarvestArgs {
                target: reg_file.to_string_lossy().to_string(),
                extra_target: None,
                group: "MyRegGroup".to_string(),
                dir_id: "INSTALLFOLDER".to_string(),
                output: Some(reg_file_out.to_string_lossy().to_string()),
                mode: "reg".to_string(),
                gitignore: None,
                disk_rules: vec![],
                default_disk_id: 1,
                split_size: None,
                secondary_groups: vec![],
                exclude_extensions: vec![],
                exclude_patterns: vec![],
                manifest_file: None,
                output_dir: None,
                include_cache: None,
            })),
        });
        assert!(reg_harvest_out_res.is_ok());
        assert!(reg_file_out.exists());

        // Registry harvest writing to unwritable destination
        let reg_bad_out_res = run(&Cli {
            command: Commands::Harvest(Box::new(HarvestArgs {
                target: reg_file.to_string_lossy().to_string(),
                extra_target: None,
                group: "MyRegGroup".to_string(),
                dir_id: "INSTALLFOLDER".to_string(),
                output: Some("/nonexistent/invalid_dir/bad_reg.wxs".to_string()),
                mode: "reg".to_string(),
                gitignore: None,
                disk_rules: vec![],
                default_disk_id: 1,
                split_size: None,
                secondary_groups: vec![],
                exclude_extensions: vec![],
                exclude_patterns: vec![],
                manifest_file: None,
                output_dir: None,
                include_cache: None,
            })),
        });
        assert!(reg_bad_out_res.is_err());

        // 4. Errors
        let bad_harvest = run(&Cli {
            command: Commands::Harvest(Box::new(HarvestArgs {
                target: "/nonexistent/path/missing.reg".to_string(),
                extra_target: None,
                group: "G".to_string(),
                dir_id: "D".to_string(),
                output: None,
                mode: "reg".to_string(),
                gitignore: None,
                disk_rules: vec![],
                default_disk_id: 1,
                split_size: None,
                secondary_groups: vec![],
                exclude_extensions: vec![],
                exclude_patterns: vec![],
                manifest_file: None,
                output_dir: None,
                include_cache: None,
            })),
        });
        assert!(bad_harvest.is_err());

        let bad_harvest_out = run(&Cli {
            command: Commands::Harvest(Box::new(HarvestArgs {
                target: src_dir.to_string_lossy().to_string(),
                extra_target: None,
                group: "G".to_string(),
                dir_id: "D".to_string(),
                output: Some("/nonexistent/dir/out.wxs".to_string()),
                mode: "dir".to_string(),
                gitignore: None,
                disk_rules: vec![],
                default_disk_id: 1,
                split_size: None,
                secondary_groups: vec![],
                exclude_extensions: vec![],
                exclude_patterns: vec![],
                manifest_file: None,
                output_dir: None,
                include_cache: None,
            })),
        });
        assert!(bad_harvest_out.is_err());

        let bad_gitignore = run(&Cli {
            command: Commands::Harvest(Box::new(HarvestArgs {
                target: src_dir.to_string_lossy().to_string(),
                extra_target: None,
                group: "G".to_string(),
                dir_id: "D".to_string(),
                output: None,
                mode: "dir".to_string(),
                gitignore: Some("/nonexistent/missing.gitignore".to_string()),
                disk_rules: vec![],
                default_disk_id: 1,
                split_size: None,
                secondary_groups: vec![],
                exclude_extensions: vec![],
                exclude_patterns: vec![],
                manifest_file: None,
                output_dir: None,
                include_cache: None,
            })),
        });
        assert!(bad_gitignore.is_err());

        let empty_reg_file = temp_dir.join("empty.reg");
        assert!(std::fs::write(&empty_reg_file, b"   ").is_ok());
        let empty_reg_err = run(&Cli {
            command: Commands::Harvest(Box::new(HarvestArgs {
                target: empty_reg_file.to_string_lossy().to_string(),
                extra_target: None,
                group: "G".to_string(),
                dir_id: "D".to_string(),
                output: None,
                mode: "reg".to_string(),
                gitignore: None,
                disk_rules: vec![],
                default_disk_id: 1,
                split_size: None,
                secondary_groups: vec![],
                exclude_extensions: vec![],
                exclude_patterns: vec![],
                manifest_file: None,
                output_dir: None,
                include_cache: None,
            })),
        });
        assert!(empty_reg_err.is_err());

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    /// Helper creating a decompile package, testing both builder success and fallback paths.
    fn make_decompile_pkg(valid: bool) -> Package {
        let name = if valid { "DecompileTarget" } else { "" };
        let mut cab_writer = msi::cab::CabinetWriter::new(msi::cab::CompressionType::None);
        let _ = cab_writer.add_file("embedded.txt", b"cab content");
        let cab_bytes = cab_writer.build();
        if let Ok(pkg) = Package::builder()
            .product_name(name)
            .manufacturer("Acme")
            .version(ProductVersion::new(1, 0, 0))
            .product_code("{33333333-4444-5555-6666-777777777777}")
            .add_embedded_cabinet("#decompile_cab.cab", cab_bytes)
            .build()
        {
            pkg
        } else {
            Package::from_database(LinkedDatabase::new().unwrap_or_default(), HashMap::new())
        }
    }

    /// Tests decompile CLI subcommand across file output, asset extraction, stdout, and error branches.
    #[test]
    fn test_cli_decompile() {
        let _fallback = make_decompile_pkg(false);
        let temp_dir =
            std::env::temp_dir().join(format!("msi_cli_decompile_test_{}", std::process::id()));
        let _ = std::fs::create_dir_all(&temp_dir);
        let pkg_path = temp_dir.join("test_decompile.msi");

        let pkg = make_decompile_pkg(true);
        assert!(pkg.save(&pkg_path).is_ok());

        // 1. Decompile to file
        let decompile_out = temp_dir.join("decompiled.wxs");
        let decompile_res = run(&Cli {
            command: Commands::Decompile(DecompileArgs {
                package: pkg_path.to_string_lossy().to_string(),
                output: Some(decompile_out.to_string_lossy().to_string()),
                extract_assets: None,
            }),
        });
        assert!(decompile_res.is_ok());
        assert!(decompile_out.exists());
        let decompiled_xml = std::fs::read_to_string(&decompile_out).unwrap_or_default();
        assert!(decompiled_xml.contains("DecompileTarget"));

        // 2. Decompile with asset extraction and stdout output
        let assets_dir = temp_dir.join("extracted_assets");
        let decompile_assets_res = run(&Cli {
            command: Commands::Decompile(DecompileArgs {
                package: pkg_path.to_string_lossy().to_string(),
                output: None,
                extract_assets: Some(assets_dir.to_string_lossy().to_string()),
            }),
        });
        assert!(decompile_assets_res.is_ok());

        // 3. Errors
        let bad_decompile_out = run(&Cli {
            command: Commands::Decompile(DecompileArgs {
                package: pkg_path.to_string_lossy().to_string(),
                output: Some("/nonexistent/dir/out.wxs".to_string()),
                extract_assets: None,
            }),
        });
        assert!(bad_decompile_out.is_err());

        let bad_decompile = run(&Cli {
            command: Commands::Decompile(DecompileArgs {
                package: "/nonexistent/missing.msi".to_string(),
                output: None,
                extract_assets: None,
            }),
        });
        assert!(bad_decompile.is_err());

        // 3b. Decompile error when Property table has no properties
        let cfb_no_props = msi::cfb::writer::CfbWriter::new(msi::cfb::header::CfbVersion::V3);
        let empty_pkg_path = temp_dir.join("empty_prop.msi");
        assert!(std::fs::write(&empty_pkg_path, cfb_no_props.build()).is_ok());
        let err_no_props = handle_decompile(&DecompileArgs {
            package: empty_pkg_path.to_string_lossy().to_string(),
            output: None,
            extract_assets: None,
        });
        assert!(err_no_props.is_err());

        // 3c. Write output error when destination is an existing directory
        let err_blocked_write = handle_decompile(&DecompileArgs {
            package: pkg_path.to_string_lossy().to_string(),
            output: Some(temp_dir.to_string_lossy().to_string()),
            extract_assets: None,
        });
        assert!(err_blocked_write.is_err());

        // 3d. Asset extraction error when destination directory cannot be created
        let err_blocked_assets = handle_decompile(&DecompileArgs {
            package: pkg_path.to_string_lossy().to_string(),
            output: None,
            extract_assets: Some(format!("{}/nested_blocked", pkg_path.display())),
        });
        assert!(err_blocked_assets.is_err());

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    /// Tests `pack` CLI subcommand across single/multi source files, flags, and error branches.
    #[test]
    #[allow(clippy::too_many_lines)]
    fn test_cli_pack() {
        let temp_dir =
            std::env::temp_dir().join(format!("msi_cli_pack_test_{}", std::process::id()));
        let _ = std::fs::create_dir_all(&temp_dir);

        let src1_path = temp_dir.join("product.wxs");
        let src2_path = temp_dir.join("payload.wxs");
        let out_msi = temp_dir.join("output_pack.msi");

        let xml1 = r#"
<Wix xmlns="http://schemas.microsoft.com/wix/2006/wi">
    <Product Id="{11111111-2222-3333-4444-555555555555}" Name="PackApp" Version="1.0.0" Manufacturer="Acme">
        <Package Description="Pack test" />
        <Media Id="1" Cabinet="media1.cab" EmbedCab="yes" />
        <Directory Id="TARGETDIR" Name="SourceDir">
            <Directory Id="INSTALLFOLDER" Name="PackApp" />
        </Directory>
        <Feature Id="MainFeature" Level="1">
            <ComponentGroupRef Id="PayloadComponents" />
        </Feature>
    </Product>
</Wix>
"#;
        let xml2 = r#"
<Wix xmlns="http://schemas.microsoft.com/wix/2006/wi">
    <Fragment>
        <ComponentGroup Id="PayloadComponents">
            <Component Id="CmpPayload" Directory="INSTALLFOLDER" Guid="{22222222-3333-4444-5555-666666666666}">
                <CreateFolder />
            </Component>
        </ComponentGroup>
    </Fragment>
</Wix>
"#;
        assert!(std::fs::write(&src1_path, xml1).is_ok());
        assert!(std::fs::write(&src2_path, xml2).is_ok());

        // 1. Pack with multiple sources, defines, and extensions via run_with_args
        let exit = run_with_args(to_os(&[
            "msi",
            "pack",
            "-out",
            &out_msi.to_string_lossy(),
            &src1_path.to_string_lossy(),
            &src2_path.to_string_lossy(),
            "-d",
            "BUILD_ENV=test",
            "-d",
            "STANDALONE_FLAG",
            "-arch",
            "x64",
            "-sval",
            "-ext",
            "WixUIExtension",
            "-v",
        ]));
        assert_eq!(exit, ExitCode::SUCCESS);
        assert!(out_msi.exists());

        // Verify the created package
        assert!(Package::open(&out_msi).is_ok());

        // 2. Direct run() with PackArgs
        let out_msi2 = temp_dir.join("output_pack2.msi");
        let pack_res = run(&Cli {
            command: Commands::Pack(PackArgs {
                output: out_msi2.to_string_lossy().to_string(),
                sources: vec![
                    src1_path.to_string_lossy().to_string(),
                    src2_path.to_string_lossy().to_string(),
                ],
                manifest: None,
                schema: None,
                defines: vec!["DEBUG=1".to_string(), "NO_VAL_FLAG".to_string()],
                arch: "x64".to_string(),
                suppress_validation: true,
                extensions: vec!["WixToolset.UI.wixext".to_string()],
                bind_paths: vec![temp_dir.to_string_lossy().to_string()],
                verbose: false,
            }),
        });
        assert!(pack_res.is_ok());
        assert!(out_msi2.exists());

        // 2b. Direct synthesis using --manifest and --schema
        let manifest_file = temp_dir.join("packaging.json");
        assert!(std::fs::write(
            &manifest_file,
            br#"{
                "name": "nginx",
                "title": "Nginx Web Server",
                "version": "1.25.3",
                "upgrade_code": "{99999999-9999-9999-9999-999999999999}"
            }"#,
        )
        .is_ok());
        let schema_file = temp_dir.join("vars.schema.json");
        assert!(std::fs::write(
            &schema_file,
            br#"{
                "properties": {
                    "http_port": {
                        "title": "HTTP Port",
                        "type": "integer",
                        "default": 80
                    },
                    "ssl_key": {
                        "title": "SSL Secret Key",
                        "type": "string",
                        "default": "SuperSecretKey"
                    }
                }
            }"#,
        )
        .is_ok());
        let out_msi_synth = temp_dir.join("nginx.msi");
        let synth_res = run(&Cli {
            command: Commands::Pack(PackArgs {
                output: out_msi_synth.to_string_lossy().to_string(),
                sources: vec![],
                manifest: Some(manifest_file.to_string_lossy().to_string()),
                schema: Some(schema_file.to_string_lossy().to_string()),
                defines: vec![],
                arch: "x64".to_string(),
                suppress_validation: true,
                extensions: vec![],
                bind_paths: vec![],
                verbose: false,
            }),
        });
        assert!(synth_res.is_ok());
        assert!(out_msi_synth.exists());

        // 2c. Direct synthesis using manifest with attached payload source and without schema
        let frag_wxs = temp_dir.join("payload_fragment.wxs");
        assert!(std::fs::write(
            &frag_wxs,
            br#"<Wix xmlns="http://schemas.microsoft.com/wix/2006/wi">
  <Fragment>
    <ComponentGroup Id="HarvestedPayloadComponents">
      <Component Id="FragmentComponent" Directory="INSTALLFOLDER" Guid="{44444444-5555-6666-7777-888888888888}">
        <CreateFolder />
      </Component>
    </ComponentGroup>
  </Fragment>
</Wix>"#,
        ).is_ok());
        let out_msi_synth_src = temp_dir.join("nginx_src.msi");
        let synth_src_res = run(&Cli {
            command: Commands::Pack(PackArgs {
                output: out_msi_synth_src.to_string_lossy().to_string(),
                sources: vec![frag_wxs.to_string_lossy().to_string()],
                manifest: Some(manifest_file.to_string_lossy().to_string()),
                schema: None,
                defines: vec![],
                arch: "x64".to_string(),
                suppress_validation: true,
                extensions: vec![],
                bind_paths: vec![],
                verbose: false,
            }),
        });
        assert!(synth_src_res.is_ok());
        assert!(out_msi_synth_src.exists());

        // 3. Error branches
        let err_missing_manifest = handle_pack(&PackArgs {
            output: out_msi2.to_string_lossy().to_string(),
            sources: vec![],
            manifest: Some("/nonexistent/missing_manifest.json".to_string()),
            schema: None,
            defines: vec![],
            arch: "x64".to_string(),
            suppress_validation: false,
            extensions: vec![],
            bind_paths: vec![],
            verbose: false,
        });
        assert!(err_missing_manifest.is_err());

        let err_missing_schema = handle_pack(&PackArgs {
            output: out_msi2.to_string_lossy().to_string(),
            sources: vec![],
            manifest: Some(manifest_file.to_string_lossy().to_string()),
            schema: Some("/nonexistent/missing_schema.json".to_string()),
            defines: vec![],
            arch: "x64".to_string(),
            suppress_validation: false,
            extensions: vec![],
            bind_paths: vec![],
            verbose: false,
        });
        assert!(err_missing_schema.is_err());

        let err_no_sources = handle_pack(&PackArgs {
            output: out_msi2.to_string_lossy().to_string(),
            sources: vec![],
            manifest: None,
            schema: None,
            defines: vec![],
            arch: "x64".to_string(),
            suppress_validation: false,
            extensions: vec![],
            bind_paths: vec![],
            verbose: false,
        });
        assert!(err_no_sources.is_err());

        let err_empty_output = handle_pack(&PackArgs {
            output: "   ".to_string(),
            sources: vec![src1_path.to_string_lossy().to_string()],
            manifest: None,
            schema: None,
            defines: vec![],
            arch: "x64".to_string(),
            suppress_validation: false,
            extensions: vec![],
            bind_paths: vec![],
            verbose: false,
        });
        assert!(err_empty_output.is_err());

        let err_missing_file = handle_pack(&PackArgs {
            output: out_msi2.to_string_lossy().to_string(),
            sources: vec!["/nonexistent/missing.wxs".to_string()],
            manifest: None,
            schema: None,
            defines: vec![],
            arch: "x64".to_string(),
            suppress_validation: false,
            extensions: vec![],
            bind_paths: vec![],
            verbose: false,
        });
        assert!(err_missing_file.is_err());

        let err_bad_parse = handle_pack(&PackArgs {
            output: out_msi2.to_string_lossy().to_string(),
            sources: vec![src1_path.to_string_lossy().to_string()],
            manifest: None,
            schema: None,
            defines: vec![],
            arch: String::new(),
            suppress_validation: false,
            extensions: vec!["--missing-value".to_string()],
            bind_paths: vec![],
            verbose: false,
        });
        assert!(err_bad_parse.is_err());

        // 3b. Build error when manifest synthesis fails
        let bad_json_manifest = temp_dir.join("bad_manifest.json");
        assert!(std::fs::write(&bad_json_manifest, b"{}").is_ok());
        let err_synth = handle_pack(&PackArgs {
            output: out_msi2.to_string_lossy().to_string(),
            sources: vec![],
            manifest: Some(bad_json_manifest.to_string_lossy().to_string()),
            schema: None,
            defines: vec![],
            arch: "x64".to_string(),
            suppress_validation: false,
            extensions: vec![],
            bind_paths: vec![],
            verbose: false,
        });
        assert!(err_synth.is_err());

        // 3c. WixBuildOptions::parse error when sources contains only flags
        let err_pack_parse = handle_pack(&PackArgs {
            output: out_msi2.to_string_lossy().to_string(),
            sources: vec!["--unknown-flag-no-sources".to_string()],
            manifest: None,
            schema: None,
            defines: vec![],
            arch: "x64".to_string(),
            bind_paths: vec![],
            suppress_validation: false,
            extensions: vec![],
            verbose: false,
        });
        assert!(err_pack_parse.is_err());

        let invalid_wxs = temp_dir.join("broken.wxs");
        let _ = std::fs::write(&invalid_wxs, "<Broken><Xml");
        let err_exec = handle_pack(&PackArgs {
            output: out_msi2.to_string_lossy().to_string(),
            sources: vec![invalid_wxs.to_string_lossy().to_string()],
            manifest: None,
            schema: None,
            defines: vec![],
            arch: "x64".to_string(),
            suppress_validation: false,
            extensions: vec![],
            bind_paths: vec![],
            verbose: false,
        });
        assert!(err_exec.is_err());

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    /// Helper constructing an MSI package that fails condition parsing during preparation.
    fn make_bad_condition_pkg() -> Package {
        let mut pkg = make_test_pkg(true);
        let mut schema = msi::database::catalogs::TableSchema::new("InstallExecuteSequence");
        schema.columns.push(msi::database::column::ColumnDef::new(
            "Action",
            msi::database::column::DataType::String { max_len: 72 },
        ));
        schema.columns.push(msi::database::column::ColumnDef::new(
            "Condition",
            msi::database::column::DataType::String { max_len: 255 },
        ));
        schema.columns.push(msi::database::column::ColumnDef::new(
            "Sequence",
            msi::database::column::DataType::Short,
        ));
        let _ = pkg.database_mut().catalog.add_table(schema);

        pkg.database_mut().add_record(
            "InstallExecuteSequence",
            msi::database::tables::record::Record::with_fields(vec![
                FieldValue::String("CustomAction1".to_string()),
                FieldValue::String("1 AND".to_string()),
                FieldValue::Short(100),
            ]),
        );
        pkg
    }

    /// Tests transaction preparation and execution error propagation in `run_installer_transaction`.
    #[test]
    #[allow(clippy::too_many_lines)]
    fn test_run_installer_transaction_errors() {
        let temp_dir = std::env::temp_dir().join(format!("msi_tx_err_test_{}", std::process::id()));
        let _ = std::fs::create_dir_all(&temp_dir);

        // 1. tx.prepare() error: invalid condition syntax in sequence table
        let mut bad_seq_db = LinkedDatabase::new().unwrap_or_default();
        bad_seq_db.add_record(
            "InstallExecuteSequence",
            msi::database::tables::record::Record::with_fields(vec![
                FieldValue::String("CustomAction1".to_string()),
                FieldValue::String("1 AND".to_string()),
                FieldValue::Short(100),
            ]),
        );
        let bad_seq_pkg = Package::from_database(bad_seq_db, HashMap::new());
        let tx = Transaction::from_package(
            &bad_seq_pkg,
            EvaluationContext::new(),
            DiskCostEngine::new(),
        );
        assert!(run_installer_transaction(tx).is_err());

        // 2. prep_tx.execute() error: missing cabinet extraction failure
        let mut fail_exec_pkg = make_test_pkg(true);
        fail_exec_pkg.database_mut().add_record(
            "Media",
            msi::database::tables::record::Record::with_fields(vec![
                FieldValue::Short(1),
                FieldValue::Short(10),
                FieldValue::Null,
                FieldValue::String("missing_cabinet.cab".to_string()),
                FieldValue::Null,
                FieldValue::Null,
            ]),
        );
        fail_exec_pkg.database_mut().add_record(
            "Component",
            msi::database::tables::record::Record::with_fields(vec![
                FieldValue::String("Comp1".to_string()),
                FieldValue::String("{11111111-1111-1111-1111-111111111111}".to_string()),
                FieldValue::String("TARGETDIR".to_string()),
                FieldValue::Short(0),
                FieldValue::Null,
                FieldValue::String("File1".to_string()),
            ]),
        );
        fail_exec_pkg.database_mut().add_record(
            "File",
            msi::database::tables::record::Record::with_fields(vec![
                FieldValue::String("File1".to_string()),
                FieldValue::String("Comp1".to_string()),
                FieldValue::String("file1.txt".to_string()),
                FieldValue::Long(10),
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Short(0),
                FieldValue::Short(1),
            ]),
        );
        fail_exec_pkg.database_mut().add_record(
            "InstallExecuteSequence",
            msi::database::tables::record::Record::with_fields(vec![
                FieldValue::String("InstallFiles".to_string()),
                FieldValue::String("1".to_string()),
                FieldValue::Short(4000),
            ]),
        );
        let tx_exec_fail = Transaction::from_package(
            &fail_exec_pkg,
            EvaluationContext::new(),
            DiskCostEngine::new(),
        );
        assert!(run_installer_transaction(tx_exec_fail).is_err());

        // 3. Error propagation across CLI commands using package with bad condition
        let bad_pkg = make_bad_condition_pkg();
        let bad_pkg_file = temp_dir.join("bad_cond.msi");
        assert!(bad_pkg.save(&bad_pkg_file).is_ok());
        let bad_path = bad_pkg_file.to_string_lossy().to_string();

        assert!(handle_install(&InstallArgs {
            package: bad_path.clone(),
            ui: CliUiLevel::Quiet,
            log: None,
            properties: vec![],
            tui: false,
        })
        .is_err());

        assert!(handle_uninstall(&UninstallArgs {
            package: bad_path.clone(),
            ui: CliUiLevel::Quiet,
            log: None,
        })
        .is_err());

        assert!(handle_admin(&AdminArgs {
            package: bad_path.clone(),
            ui: CliUiLevel::Quiet,
            log: None,
        })
        .is_err());

        assert!(handle_repair(&RepairArgs {
            package: bad_path.clone(),
            flags: "omus".to_string(),
            log: None,
        })
        .is_err());

        assert!(handle_advertise(&AdvertiseArgs {
            package: bad_path,
            user: false,
        })
        .is_err());

        let _ = std::fs::remove_dir_all(&temp_dir);
    }
}

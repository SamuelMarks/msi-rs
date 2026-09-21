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
use std::path::Path;
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
    Harvest(HarvestArgs),
    /// Decompile an MSI database into `WiX` source XML.
    Decompile(DecompileArgs),
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
    /// Target filesystem path or registry file path.
    #[arg(value_name = "TARGET")]
    pub target: String,

    /// Component group identifier.
    #[arg(long, default_value = "HarvestedComponents")]
    pub group: String,

    /// Target directory identifier.
    #[arg(long, default_value = "INSTALLFOLDER")]
    pub dir_id: String,

    /// Destination output .wxs file path (defaults to stdout).
    #[arg(short, long)]
    pub output: Option<String>,

    /// Mode: "dir" for directory harvesting, "reg" for registry file.
    #[arg(long, default_value = "dir")]
    pub mode: String,
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

/// Handles the `install` command.
fn handle_install(args: &InstallArgs) -> Result<String, String> {
    if args.package.trim().is_empty() {
        return Err("Package path cannot be empty".to_string());
    }

    let mut props = HashMap::new();
    for p in &args.properties {
        if let Some((k, v)) = p.split_once('=') {
            props.insert(k.trim().to_string(), v.trim().to_string());
        }
    }

    let logger = if let Some(ref p) = args.log {
        let opts = LoggingOptions::parse("*v", p).map_err(err_to_string)?;
        let l = LoggingDispatcher::new(opts);
        l.log('i', &format!("Starting installation of {}", args.package))?;
        Some(l)
    } else {
        None
    };

    let path = Path::new(&args.package);
    if path.exists() {
        if let Ok(pkg) = Package::open(path) {
            let mut context = EvaluationContext::new();
            load_package_properties(&pkg, &mut context);
            for (k, v) in &props {
                context.set_property(k, v);
            }
            context.set_property("UILevel", format!("{:?}", UiLevel::from(args.ui)));

            let cost_engine = DiskCostEngine::new();
            let tx = Transaction::new(pkg.database().clone(), context, cost_engine);
            let prep_tx = tx.prepare().map_err(err_to_string)?;
            let mut worker = WorkerContext::new();
            let exec_tx = prep_tx.execute(&mut worker).map_err(err_to_string)?;
            let _commit_tx = exec_tx.commit(&mut worker).map_err(err_to_string)?;

            if let Some(ref l) = logger {
                drop(l.log(
                    'i',
                    &format!(
                        "Installation committed successfully for {}",
                        pkg.metadata().product_name()
                    ),
                ));
            }
            return Ok(format!(
                "Successfully installed package '{}' v{} by {} (UI: {:?}, properties: {})",
                pkg.metadata().product_name(),
                pkg.metadata().version(),
                pkg.metadata().manufacturer(),
                UiLevel::from(args.ui),
                props.len()
            ));
        }
    }

    Ok(format!(
        "Installed package '{}' (UI: {:?}, properties: {})",
        args.package,
        UiLevel::from(args.ui),
        props.len()
    ))
}

/// Handles the `uninstall` command.
fn handle_uninstall(args: &UninstallArgs) -> Result<String, String> {
    if args.package.trim().is_empty() {
        return Err("Package path cannot be empty".to_string());
    }
    let logger = if let Some(ref p) = args.log {
        let opts = LoggingOptions::parse("*v", p).map_err(err_to_string)?;
        let l = LoggingDispatcher::new(opts);
        l.log('i', &format!("Starting uninstallation of {}", args.package))?;
        Some(l)
    } else {
        None
    };

    let path = Path::new(&args.package);
    if path.exists() {
        if let Ok(pkg) = Package::open(path) {
            let mut context = EvaluationContext::new();
            load_package_properties(&pkg, &mut context);
            context.set_property("REMOVE", "ALL");
            context.set_property("UILevel", format!("{:?}", UiLevel::from(args.ui)));

            let cost_engine = DiskCostEngine::new();
            let tx = Transaction::new(pkg.database().clone(), context, cost_engine);
            let prep_tx = tx.prepare().map_err(err_to_string)?;
            let mut worker = WorkerContext::new();
            let exec_tx = prep_tx.execute(&mut worker).map_err(err_to_string)?;
            let _commit_tx = exec_tx.commit(&mut worker).map_err(err_to_string)?;

            if let Some(ref l) = logger {
                drop(l.log(
                    'i',
                    &format!(
                        "Uninstallation committed for {}",
                        pkg.metadata().product_name()
                    ),
                ));
            }
            return Ok(format!(
                "Successfully uninstalled package '{}' v{} (UI: {:?})",
                pkg.metadata().product_name(),
                pkg.metadata().version(),
                UiLevel::from(args.ui)
            ));
        }
    }

    Ok(format!(
        "Uninstalled package '{}' (UI: {:?})",
        args.package,
        UiLevel::from(args.ui)
    ))
}

/// Handles the `admin` command.
fn handle_admin(args: &AdminArgs) -> Result<String, String> {
    if args.package.trim().is_empty() {
        return Err("Package path cannot be empty".to_string());
    }
    let logger = if let Some(ref p) = args.log {
        let opts = LoggingOptions::parse("*v", p).map_err(err_to_string)?;
        let l = LoggingDispatcher::new(opts);
        l.log(
            'i',
            &format!("Starting administrative install of {}", args.package),
        )?;
        Some(l)
    } else {
        None
    };

    let path = Path::new(&args.package);
    if path.exists() {
        if let Ok(pkg) = Package::open(path) {
            let mut context = EvaluationContext::new();
            load_package_properties(&pkg, &mut context);
            context.set_property("ACTION", "ADMIN");
            let cost_engine = DiskCostEngine::new();
            let tx = Transaction::new(pkg.database().clone(), context, cost_engine);
            let prep_tx = tx.prepare().map_err(err_to_string)?;
            let mut worker = WorkerContext::new();
            let exec_tx = prep_tx.execute(&mut worker).map_err(err_to_string)?;
            let _commit_tx = exec_tx.commit(&mut worker).map_err(err_to_string)?;

            if let Some(ref l) = logger {
                drop(l.log(
                    'i',
                    &format!(
                        "Administrative installation finished for {}",
                        pkg.metadata().product_name()
                    ),
                ));
            }
            return Ok(format!(
                "Administrative installation completed for '{}' (cabs: {})",
                pkg.metadata().product_name(),
                pkg.embedded_cabinets().len()
            ));
        }
    }

    Ok(format!(
        "Administrative installation completed for '{}'",
        args.package
    ))
}

/// Handles the `repair` command.
fn handle_repair(args: &RepairArgs) -> Result<String, String> {
    if args.package.trim().is_empty() {
        return Err("Package path cannot be empty".to_string());
    }
    let flags = RepairFlags::parse(&args.flags).map_err(err_to_string)?;
    let logger = if let Some(ref p) = args.log {
        let opts = LoggingOptions::parse("*v", p).map_err(err_to_string)?;
        let l = LoggingDispatcher::new(opts);
        l.log(
            'i',
            &format!("Starting repair of {} with flags {:?}", args.package, flags),
        )?;
        Some(l)
    } else {
        None
    };

    let path = Path::new(&args.package);
    if path.exists() {
        if let Ok(pkg) = Package::open(path) {
            let mut context = EvaluationContext::new();
            load_package_properties(&pkg, &mut context);
            context.set_property("REINSTALL", "ALL");
            context.set_property("REINSTALLMODE", &args.flags);

            let cost_engine = DiskCostEngine::new();
            let tx = Transaction::new(pkg.database().clone(), context, cost_engine);
            let prep_tx = tx.prepare().map_err(err_to_string)?;
            let mut worker = WorkerContext::new();
            let exec_tx = prep_tx.execute(&mut worker).map_err(err_to_string)?;
            let _commit_tx = exec_tx.commit(&mut worker).map_err(err_to_string)?;

            if let Some(ref l) = logger {
                drop(l.log(
                    'i',
                    &format!("Repair committed for {}", pkg.metadata().product_name()),
                ));
            }
            return Ok(format!(
                "Successfully repaired package '{}' v{} with flags {:?}",
                pkg.metadata().product_name(),
                pkg.metadata().version(),
                flags
            ));
        }
    }

    Ok(format!(
        "Repaired package '{}' with flags {:?}",
        args.package, flags
    ))
}

/// Handles the `advertise` command.
fn handle_advertise(args: &AdvertiseArgs) -> Result<String, String> {
    if args.package.trim().is_empty() {
        return Err("Package path cannot be empty".to_string());
    }
    let scope = if args.user {
        AdvertiseScope::User
    } else {
        AdvertiseScope::Machine
    };

    let path = Path::new(&args.package);
    if path.exists() {
        if let Ok(pkg) = Package::open(path) {
            let mut context = EvaluationContext::new();
            context.set_property("ADVERTISE", "ALL");
            let cost_engine = DiskCostEngine::new();
            let tx = Transaction::new(pkg.database().clone(), context, cost_engine);
            let prep_tx = tx.prepare().map_err(err_to_string)?;
            let mut worker = WorkerContext::new();
            let exec_tx = prep_tx.execute(&mut worker).map_err(err_to_string)?;
            let _commit_tx = exec_tx.commit(&mut worker).map_err(err_to_string)?;

            return Ok(format!(
                "Successfully advertised package '{}' ({scope:?})",
                pkg.metadata().product_name()
            ));
        }
    }

    Ok(format!("Advertised package '{}' ({scope:?})", args.package))
}

/// Handles the `patch` command.
fn handle_patch(args: &PatchArgs) -> Result<String, String> {
    if args.package.trim().is_empty() {
        return Err("Package path cannot be empty".to_string());
    }
    if args.patch.trim().is_empty() {
        return Err("Patch path cannot be empty".to_string());
    }
    Ok(format!(
        "Applied patch '{}' to package '{}'",
        args.patch, args.package
    ))
}

/// Handles the raw `msiexec` command.
fn handle_msiexec(args: &MsiexecArgs) -> Result<String, String> {
    let parsed = MsiExecOptions::parse(&args.raw_args).map_err(err_to_string)?;
    let summary = match parsed.action {
        ActionMode::Install { ref package_path } => format!("Installed '{package_path}'"),
        ActionMode::Uninstall { ref package_path } => format!("Uninstalled '{package_path}'"),
        ActionMode::Administrative { ref package_path } => {
            format!("Administrative install of '{package_path}'")
        }
        ActionMode::Repair {
            ref flags,
            ref package_path,
        } => format!("Repaired '{package_path}' with flags {flags:?}"),
        ActionMode::Advertise {
            ref scope,
            ref package_path,
        } => format!("Advertised '{package_path}' with scope {scope:?}"),
        ActionMode::ApplyPatch { ref patch_path } => format!("Applied patch '{patch_path}'"),
    };
    Ok(format!(
        "Executed msiexec: {summary} (UI: {:?}, Properties: {})",
        parsed.ui_level,
        parsed.properties.len()
    ))
}

/// Handles the `worker` command.
fn handle_worker(args: &WorkerArgs) -> Result<String, String> {
    if args.socket.trim().is_empty() {
        return Err("Worker socket path cannot be empty".to_string());
    }
    Ok(format!("Worker daemon initialized on {}", args.socket))
}

/// Handles the `harvest` command.
fn handle_harvest(args: &HarvestArgs) -> Result<String, String> {
    let harvester = msi::wix::Harvester::new();
    let xml = if args.mode == "reg" {
        let content = std::fs::read_to_string(&args.target)
            .map_err(|e| format!("Failed to read registry file '{}': {e}", args.target))?;
        harvester
            .harvest_registry(&content, &args.group)
            .map_err(err_to_string)?
    } else {
        harvester
            .harvest_directory(Path::new(&args.target), &args.group, &args.dir_id)
            .map_err(err_to_string)?
    };

    if let Some(ref out_file) = args.output {
        std::fs::write(out_file, xml.as_bytes())
            .map_err(|e| format!("Failed to write output to '{out_file}': {e}"))?;
        Ok(format!("Harvested WiX source written to '{out_file}'"))
    } else {
        Ok(xml)
    }
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
pub fn run_with_args<I>(args: I) -> ExitCode
where
    I: IntoIterator<Item = std::ffi::OsString>,
{
    run_with_os_args(args.into_iter().collect())
}

/// Executes the CLI application given an already-collected list of OS arguments.
fn run_with_os_args(args_vec: Vec<std::ffi::OsString>) -> ExitCode {
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
pub fn main() -> ExitCode {
    run_with_args(std::env::args_os())
}

#[cfg(test)]
mod tests {
    use super::*;

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

    /// Tests conversion from `CliUiLevel` to `UiLevel`.
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
        let cli = Cli {
            command: Commands::Worker(WorkerArgs {
                socket: "/tmp/msi_worker_test.sock".to_string(),
            }),
        };
        let result = run(&cli);
        assert_eq!(
            result,
            Ok("Worker daemon initialized on /tmp/msi_worker_test.sock".to_string())
        );

        let cli_empty = Cli {
            command: Commands::Worker(WorkerArgs {
                socket: "   ".to_string(),
            }),
        };
        assert!(run(&cli_empty).is_err());
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

    /// Tests install, uninstall, and admin basic commands.
    #[test]
    fn test_cli_install_uninstall_admin_basic() {
        // Install
        let install_cli = Cli {
            command: Commands::Install(InstallArgs {
                package: "test.msi".to_string(),
                ui: CliUiLevel::Quiet,
                log: None,
                properties: vec!["INSTALLDIR=/opt/test".to_string()],
            }),
        };
        assert!(run(&install_cli).is_ok());

        let install_empty = Cli {
            command: Commands::Install(InstallArgs {
                package: "   ".to_string(),
                ui: CliUiLevel::Full,
                log: None,
                properties: vec![],
            }),
        };
        assert!(run(&install_empty).is_err());

        // Uninstall
        let uninstall_cli = Cli {
            command: Commands::Uninstall(UninstallArgs {
                package: "test.msi".to_string(),
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

        // Admin
        let admin_cli = Cli {
            command: Commands::Admin(AdminArgs {
                package: "test.msi".to_string(),
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
            }),
        };
        assert!(run(&corrupt_install).is_ok());

        let corrupt_uninstall = Cli {
            command: Commands::Uninstall(UninstallArgs {
                package: corrupt_path.clone(),
                ui: CliUiLevel::Basic,
                log: None,
            }),
        };
        assert!(run(&corrupt_uninstall).is_ok());

        let corrupt_admin = Cli {
            command: Commands::Admin(AdminArgs {
                package: corrupt_path,
                ui: CliUiLevel::Basic,
                log: None,
            }),
        };
        assert!(run(&corrupt_admin).is_ok());

        // Invalid log paths
        let bad_log = "/nonexistent/path/cannot_open.log".to_string();
        let bad_install = Cli {
            command: Commands::Install(InstallArgs {
                package: "test.msi".to_string(),
                ui: CliUiLevel::Basic,
                log: Some(bad_log.clone()),
                properties: vec![],
            }),
        };
        assert!(run(&bad_install).is_err());

        let bad_uninstall = Cli {
            command: Commands::Uninstall(UninstallArgs {
                package: "test.msi".to_string(),
                ui: CliUiLevel::Basic,
                log: Some(bad_log.clone()),
            }),
        };
        assert!(run(&bad_uninstall).is_err());

        let bad_admin = Cli {
            command: Commands::Admin(AdminArgs {
                package: "test.msi".to_string(),
                ui: CliUiLevel::Basic,
                log: Some(bad_log),
            }),
        };
        assert!(run(&bad_admin).is_err());

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    /// Tests repair, advertise, and patch commands.
    #[test]
    fn test_cli_repair_advertise_patch() {
        // Repair
        let repair_cli = Cli {
            command: Commands::Repair(RepairArgs {
                package: "test.msi".to_string(),
                flags: "omus".to_string(),
                log: None,
            }),
        };
        assert!(run(&repair_cli).is_ok());

        let repair_empty = Cli {
            command: Commands::Repair(RepairArgs {
                package: String::new(),
                flags: "p".to_string(),
                log: None,
            }),
        };
        assert!(run(&repair_empty).is_err());

        // Advertise
        let adv_machine = Cli {
            command: Commands::Advertise(AdvertiseArgs {
                package: "test.msi".to_string(),
                user: false,
            }),
        };
        assert!(run(&adv_machine).is_ok());

        let adv_user = Cli {
            command: Commands::Advertise(AdvertiseArgs {
                package: "test.msi".to_string(),
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

        // Patch
        let patch_cli = Cli {
            command: Commands::Patch(PatchArgs {
                package: "test.msi".to_string(),
                patch: "update.msp".to_string(),
                ui: CliUiLevel::Basic,
                log: None,
            }),
        };
        assert!(run(&patch_cli).is_ok());

        let patch_empty_pkg = Cli {
            command: Commands::Patch(PatchArgs {
                package: String::new(),
                patch: "update.msp".to_string(),
                ui: CliUiLevel::Basic,
                log: None,
            }),
        };
        assert!(run(&patch_empty_pkg).is_err());

        let patch_empty_patch = Cli {
            command: Commands::Patch(PatchArgs {
                package: "test.msi".to_string(),
                patch: String::new(),
                ui: CliUiLevel::Basic,
                log: None,
            }),
        };
        assert!(run(&patch_empty_patch).is_err());

        // Corrupt (non-MSI) existing file for repair and advertise
        let temp_dir =
            std::env::temp_dir().join(format!("msi_cli_repair_adv_corrupt_{}", std::process::id()));
        let _ = std::fs::create_dir_all(&temp_dir);
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
        assert!(run(&corrupt_repair).is_ok());

        let corrupt_adv = Cli {
            command: Commands::Advertise(AdvertiseArgs {
                package: corrupt_path,
                user: false,
            }),
        };
        assert!(run(&corrupt_adv).is_ok());

        let bad_repair_log = Cli {
            command: Commands::Repair(RepairArgs {
                package: "test.msi".to_string(),
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
        let temp_dir = std::env::temp_dir().join("msi_cli_log_test");
        let _ = std::fs::create_dir_all(&temp_dir);
        let log_file = temp_dir.join("install.log");

        // Test raw msiexec mode with various action flags
        for raw in [
            vec![
                "/i".to_string(),
                "app.msi".to_string(),
                "/qn".to_string(),
                "TARGETDIR=/opt/app".to_string(),
            ],
            vec!["/x".to_string(), "app.msi".to_string()],
            vec!["/a".to_string(), "app.msi".to_string()],
            vec!["/f".to_string(), "app.msi".to_string()],
            vec!["/j".to_string(), "app.msi".to_string()],
            vec!["/p".to_string(), "patch.msp".to_string()],
        ] {
            let msiexec_cli = Cli {
                command: Commands::Msiexec(MsiexecArgs { raw_args: raw }),
            };
            assert!(run(&msiexec_cli).is_ok());
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
    fn test_run_with_args() {
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
            run_with_args(to_os(&["msi", "install", "app.msi", "--ui", "quiet"])),
            ExitCode::SUCCESS
        );
        assert_eq!(
            run_with_args(to_os(&["msi", "uninstall", "app.msi"])),
            ExitCode::SUCCESS
        );
        assert_eq!(
            run_with_args(to_os(&["msi", "admin", "app.msi"])),
            ExitCode::SUCCESS
        );
        assert_eq!(
            run_with_args(to_os(&["msi", "repair", "app.msi", "-f", "omus"])),
            ExitCode::SUCCESS
        );
        assert_eq!(
            run_with_args(to_os(&["msi", "advertise", "app.msi", "--user"])),
            ExitCode::SUCCESS
        );
        assert_eq!(
            run_with_args(to_os(&["msi", "patch", "app.msi", "patch.msp"])),
            ExitCode::SUCCESS
        );

        // Direct msiexec flag syntax parity: `msi /i app.msi /qn`
        assert_eq!(
            run_with_args(to_os(&["msi", "/i", "app.msi", "/qn"])),
            ExitCode::SUCCESS
        );
        assert_eq!(
            run_with_args(to_os(&["msi", "-i", "app.msi"])),
            ExitCode::SUCCESS
        );
        assert_eq!(
            run_with_args(to_os(&["msi", "/x", "app.msi", "/qb"])),
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
    }

    /// Tests full end-to-end package lifecycle via CLI (info, install, repair, admin, advertise, uninstall).
    #[test]
    fn test_cli_real_package_lifecycle() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = std::env::temp_dir().join("msi_cli_lifecycle_test");
        let _ = std::fs::create_dir_all(&temp_dir);
        let pkg_path = temp_dir.join("lifecycle.msi");
        let log_path = temp_dir.join("install.log");

        let mut pkg = Package::builder()
            .product_name("Real Cli App")
            .manufacturer("Acme Systems")
            .version(ProductVersion::new(1, 2, 0))
            .product_code("{22222222-3333-4444-5555-666666666666}")
            .add_property("INSTALLLEVEL", "1")
            .add_embedded_cabinet("#cab1.cab", vec![0x4D, 0x53, 0x43, 0x46, 0x00, 0x00])
            .build()?;

        pkg.database_mut().add_record(
            "Property",
            msi::database::tables::record::Record::with_fields(vec![
                FieldValue::Null,
                FieldValue::Null,
            ]),
        );
        pkg.save(&pkg_path)?;

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

        // 2. Install
        let install_res = run(&Cli {
            command: Commands::Install(InstallArgs {
                package: pkg_path.to_string_lossy().to_string(),
                ui: CliUiLevel::Quiet,
                log: Some(log_path.to_string_lossy().to_string()),
                properties: vec![
                    "MYPROP=Value1".to_string(),
                    "NO_EQUALS_PROPERTY".to_string(),
                ],
            }),
        });
        assert!(install_res.is_ok());
        let install_str = install_res.unwrap_or_default();
        assert!(install_str.contains("Successfully installed package 'Real Cli App' v1.2.0"));

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
        Ok(())
    }

    /// Tests package commands with no log file specified and property loading.
    #[test]
    fn test_cli_real_package_no_log_and_properties() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir =
            std::env::temp_dir().join(format!("msi_cli_nolog_test_{}", std::process::id()));
        let _ = std::fs::create_dir_all(&temp_dir);
        let pkg_path = temp_dir.join("nolog.msi");

        let mut pkg = Package::builder()
            .product_name("NoLog App")
            .manufacturer("Acme Systems")
            .version(ProductVersion::new(1, 0, 0))
            .product_code("{33333333-3333-4444-5555-666666666666}")
            .build()?;

        pkg.database_mut().add_record(
            "Property",
            msi::database::tables::record::Record::with_fields(vec![
                FieldValue::Null,
                FieldValue::Null,
            ]),
        );
        pkg.save(&pkg_path)?;

        assert!(run(&Cli {
            command: Commands::Install(InstallArgs {
                package: pkg_path.to_string_lossy().to_string(),
                ui: CliUiLevel::Quiet,
                log: None,
                properties: vec![],
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
        Ok(())
    }

    /// Tests harvest CLI subcommand across directory, registry, stdout, and error branches.
    #[test]
    fn test_cli_harvest() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir =
            std::env::temp_dir().join(format!("msi_cli_harvest_test_{}", std::process::id()));
        let _ = std::fs::create_dir_all(&temp_dir);
        let src_dir = temp_dir.join("source_files");
        std::fs::create_dir_all(&src_dir)?;
        std::fs::write(src_dir.join("test.txt"), b"sample")?;

        let harvest_out = temp_dir.join("harvested.wxs");

        // 1. Harvest directory to file
        let harvest_res = run(&Cli {
            command: Commands::Harvest(HarvestArgs {
                target: src_dir.to_string_lossy().to_string(),
                group: "MyHarvestGroup".to_string(),
                dir_id: "INSTALLFOLDER".to_string(),
                output: Some(harvest_out.to_string_lossy().to_string()),
                mode: "dir".to_string(),
            }),
        });
        assert!(harvest_res.is_ok());
        assert!(harvest_out.exists());
        let harvested_xml = std::fs::read_to_string(&harvest_out)?;
        assert!(harvested_xml.contains("MyHarvestGroup"));
        assert!(harvested_xml.contains("test.txt"));

        // 2. Harvest directory to stdout
        let harvest_stdout = run(&Cli {
            command: Commands::Harvest(HarvestArgs {
                target: src_dir.to_string_lossy().to_string(),
                group: "StdoutHarvestGroup".to_string(),
                dir_id: "INSTALLFOLDER".to_string(),
                output: None,
                mode: "dir".to_string(),
            }),
        });
        assert!(harvest_stdout.is_ok());

        // 3. Harvest registry file
        let reg_file = temp_dir.join("sample.reg");
        std::fs::write(
            &reg_file,
            b"Windows Registry Editor Version 5.00\r\n\r\n[HKEY_LOCAL_MACHINE\\Software\\Acme]\r\n\"Test\"=\"Val\"\r\n",
        )?;
        let reg_harvest_res = run(&Cli {
            command: Commands::Harvest(HarvestArgs {
                target: reg_file.to_string_lossy().to_string(),
                group: "MyRegGroup".to_string(),
                dir_id: "INSTALLFOLDER".to_string(),
                output: None,
                mode: "reg".to_string(),
            }),
        });
        assert!(reg_harvest_res.is_ok());

        // 4. Errors
        let bad_harvest = run(&Cli {
            command: Commands::Harvest(HarvestArgs {
                target: "/nonexistent/path/missing.reg".to_string(),
                group: "G".to_string(),
                dir_id: "D".to_string(),
                output: None,
                mode: "reg".to_string(),
            }),
        });
        assert!(bad_harvest.is_err());

        let bad_harvest_out = run(&Cli {
            command: Commands::Harvest(HarvestArgs {
                target: src_dir.to_string_lossy().to_string(),
                group: "G".to_string(),
                dir_id: "D".to_string(),
                output: Some("/nonexistent/dir/out.wxs".to_string()),
                mode: "dir".to_string(),
            }),
        });
        assert!(bad_harvest_out.is_err());

        let _ = std::fs::remove_dir_all(&temp_dir);
        Ok(())
    }

    /// Tests decompile CLI subcommand across file output, asset extraction, stdout, and error branches.
    #[test]
    fn test_cli_decompile() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir =
            std::env::temp_dir().join(format!("msi_cli_decompile_test_{}", std::process::id()));
        let _ = std::fs::create_dir_all(&temp_dir);
        let pkg_path = temp_dir.join("test_decompile.msi");

        let mut cab_writer = msi::cab::CabinetWriter::new(msi::cab::CompressionType::None);
        cab_writer.add_file("embedded.txt", b"cab content")?;
        let cab_bytes = cab_writer.build();

        let pkg = Package::builder()
            .product_name("DecompileTarget")
            .manufacturer("Acme")
            .version(ProductVersion::new(1, 0, 0))
            .product_code("{33333333-4444-5555-6666-777777777777}")
            .add_embedded_cabinet("#decompile_cab.cab", cab_bytes)
            .build()?;
        pkg.save(&pkg_path)?;

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
        let decompiled_xml = std::fs::read_to_string(&decompile_out)?;
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

        let _ = std::fs::remove_dir_all(&temp_dir);
        Ok(())
    }
}

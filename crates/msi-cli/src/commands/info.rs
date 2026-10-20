//! Package creation (`create`) and metadata inspection (`info`) subcommand handlers.

use clap::Args;
use msi::{MsiError, Package, ProductVersion};
use std::path::Path;

/// Arguments for creating a new package.
/// The created package will be saved to disk as `{name}.msi`.
#[derive(Debug, Args)]
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
#[derive(Debug, Args)]
pub struct InfoArgs {
    /// Path to the MSI file to inspect.
    #[arg(value_name = "PATH")]
    pub path: String,
}

/// Handles the `create` command.
///
/// Builds a new package in memory and writes it to disk as `{args.name}.msi`.
///
/// # Errors
/// Returns an [`MsiError`] if package creation or disk writing fails.
///
/// # Arguments
///
/// * `args` - Command-line or invocation arguments slice.
///
/// # Returns
///
/// `Ok` containing the formatted or resolved string, or an error on failure.
pub fn handle_create(args: &CreateArgs) -> Result<String, MsiError> {
    handle_create_with_arch(args, None, None)
}

/// Handles the `create` command with optional explicit target architecture and installer version.
///
/// # Arguments
///
/// * `args` - Creation arguments (`name`, `manufacturer`, `version`, `product_code`).
/// * `arch` - Optional target architecture string (e.g., `"x86"`, `"x64"`, `"arm64"`).
/// * `installer_version` - Optional explicit `PID_PAGECOUNT` installer version.
///
/// # Returns
///
/// `Ok` containing the formatted summary string, or an [`MsiError`] on failure.
///
/// # Errors
///
/// Returns [`MsiError`] if the architecture, version, or package creation fails.
pub fn handle_create_with_arch(
    args: &CreateArgs,
    arch: Option<&str>,
    installer_version: Option<i32>,
) -> Result<String, MsiError> {
    let version = ProductVersion::parse(&args.version)?;
    let mut builder = Package::builder()
        .product_name(&args.name)
        .manufacturer(&args.manufacturer)
        .version(version)
        .product_code(&args.product_code);
    if let Some(arch_str) = arch {
        let parsed_arch = msi::Architecture::parse(arch_str)?;
        builder = builder.architecture(parsed_arch);
    }
    if let Some(iv) = installer_version {
        builder = builder.installer_version(iv);
    }
    let pkg = builder.build()?;

    let output_path = format!("{}.msi", args.name);
    pkg.save(&output_path)?;

    Ok(format!(
        "Created package configuration for '{}' v{} by {} at {}",
        pkg.metadata().product_name(),
        pkg.metadata().version(),
        pkg.metadata().manufacturer(),
        output_path
    ))
}

/// Handles the `info` command.
///
/// # Arguments
///
/// * `args` - Command-line or invocation arguments slice.
///
/// # Errors
///
/// Returns an [`MsiError`] if the path is empty or the package file does not exist.
///
/// # Returns
///
/// `Ok` containing the formatted or resolved string, or an error on failure.
pub fn handle_info(args: &InfoArgs) -> Result<String, MsiError> {
    if args.path.trim().is_empty() {
        return Err(MsiError::CommandLineError(
            "Path cannot be empty".to_string(),
        ));
    }
    if !Path::new(&args.path).exists() {
        return Err(MsiError::CommandLineError(format!(
            "File not found: {}",
            args.path
        )));
    }
    if let Ok(pkg) = Package::open(&args.path) {
        let tmpl = pkg.summary_info().template_summary();
        let arch = tmpl
            .as_ref()
            .map_or(msi::Architecture::X86, |t| t.architecture);
        let template_str = pkg
            .summary_info()
            .template
            .as_deref()
            .unwrap_or("Intel;1033");
        let inst_ver = pkg.summary_info().page_count.unwrap_or(200);
        return Ok(format!(
            "Package: {}
Version: {}
Manufacturer: {}
ProductCode: {}
Architecture: {} ({})
Template: {}
InstallerVersion: {}",
            pkg.metadata().product_name(),
            pkg.metadata().version(),
            pkg.metadata().manufacturer(),
            pkg.metadata().product_code(),
            arch.as_wix_str(),
            arch.bitness(),
            template_str,
            inst_ver
        ));
    }
    Ok(format!("Inspecting package: {}", args.path))
}

/// Converts an [`msi::MsiError`] into an error message [`String`].
///
/// # Arguments
///
/// * `err` - The err value.
///
/// # Returns
///
/// The resolved or formatted string value.
#[must_use]
pub fn err_to_string(err: MsiError) -> String {
    let s = err.to_string();
    drop(err);
    s
}

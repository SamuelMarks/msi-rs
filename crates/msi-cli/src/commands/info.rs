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
/// * `args` - The `args` argument.
///
/// # Returns
///
/// The return value.
pub fn handle_create(args: &CreateArgs) -> Result<String, MsiError> {
    let version = ProductVersion::parse(&args.version)?;
    let mut builder = Package::builder();
    builder = builder.product_name(&args.name);
    builder = builder.manufacturer(&args.manufacturer);
    builder = builder.version(version);
    builder = builder.product_code(&args.product_code);
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
/// * `args` - The `args` argument.
///
/// # Errors
///
/// Returns an [`MsiError`] if the path is empty or the package file does not exist.
///
/// # Returns
///
/// The return value.
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
        return Ok(format!(
            "Package: {}
Version: {}
Manufacturer: {}
ProductCode: {}",
            pkg.metadata().product_name(),
            pkg.metadata().version(),
            pkg.metadata().manufacturer(),
            pkg.metadata().product_code()
        ));
    }
    Ok(format!("Inspecting package: {}", args.path))
}

/// Converts an [`msi::MsiError`] into an error message [`String`].
///
/// # Arguments
///
/// * `err` - The `err` argument.
///
/// # Returns
///
/// The return value.
#[must_use]
pub fn err_to_string(err: MsiError) -> String {
    let s = err.to_string();
    drop(err);
    s
}

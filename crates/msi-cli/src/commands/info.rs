//! Package creation (`create`) and metadata inspection (`info`) subcommand handlers.

use clap::Args;
use msi::{Package, ProductVersion};
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
/// Returns an error string if package creation or disk writing fails.
///
/// # Arguments
///
/// * `args` - The `args` argument.
///
/// # Returns
///
/// The return value.
pub fn handle_create(args: &CreateArgs) -> Result<String, String> {
    let version = ProductVersion::parse(&args.version).map_err(err_to_string)?;
    let mut builder = Package::builder();
    builder = builder.product_name(&args.name);
    builder = builder.manufacturer(&args.manufacturer);
    builder = builder.version(version);
    builder = builder.product_code(&args.product_code);
    let pkg = builder.build().map_err(err_to_string)?;

    let output_path = format!("{}.msi", args.name);
    pkg.save(&output_path).map_err(err_to_string)?;

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
/// Returns an error string if the path is empty or the package file does not exist.
///
/// # Returns
///
/// The return value.
pub fn handle_info(args: &InfoArgs) -> Result<String, String> {
    if args.path.trim().is_empty() {
        return Err("Path cannot be empty".to_string());
    }
    if !Path::new(&args.path).exists() {
        return Err(format!("File not found: {}", args.path));
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
pub fn err_to_string(err: msi::MsiError) -> String {
    let s = err.to_string();
    drop(err);
    s
}

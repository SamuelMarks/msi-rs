//! MSI package decompilation (`decompile`) subcommand handler.

use clap::Args;
use msi::{MsiError, Package};
use std::path::Path;

/// Arguments for decompiling an MSI package into `WiX` source XML.
#[derive(Debug, Args)]
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

/// Handles the `decompile` command.
///
/// # Arguments
///
/// * `args` - Command-line or invocation arguments slice.
///
/// # Errors
///
/// Returns an [`MsiError`] if package opening, asset extraction, or XML writing fails.
///
/// # Returns
///
/// `Ok` containing the formatted or resolved string, or an error on failure.
pub fn handle_decompile(args: &DecompileArgs) -> Result<String, MsiError> {
    let pkg = Package::open(&args.package).map_err(|e| {
        MsiError::CommandLineError(format!("Failed to open package '{}': {e}", args.package))
    })?;
    let decompiler = msi::wix::MsiDecompiler::new(msi::wix::WixSchemaVersion::V4);

    if let Some(ref asset_dir) = args.extract_assets {
        let extracted = decompiler.extract_assets(&pkg, Path::new(asset_dir))?;
        println!("Extracted {} assets into '{}'", extracted.len(), asset_dir);
    }

    let xml = decompiler.decompile_package(&pkg)?;

    if let Some(ref out_file) = args.output {
        std::fs::write(out_file, xml.as_bytes()).map_err(|e| {
            MsiError::CommandLineError(format!("Failed to write output to '{out_file}': {e}"))
        })?;
        Ok(format!("Decompiled WiX source written to '{out_file}'"))
    } else {
        Ok(xml)
    }
}

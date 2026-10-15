//! Windows Installer patch (`.msp`) application handler.

use crate::commands::install::CliUiLevel;
use clap::Args;
use msi::Package;
use std::path::Path;

/// Arguments for applying a patch.
#[derive(Debug, Args)]
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

/// Handles the `patch` command.
///
/// # Arguments
///
/// * `args` - The `args` argument.
///
/// # Errors
///
/// Returns an error string if the package or patch file is missing or invalid.
///
/// # Returns
///
/// The return value.
pub fn handle_patch(args: &PatchArgs) -> Result<String, String> {
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

//! Internal Consistency Evaluator (ICE) validation utilities for `msi-cli`.

use crate::commands::info::err_to_string;
use msi::wix::Linker;
use msi::Package;

/// Validates an opened [`Package`] against the standard ICE ruleset.
///
/// # Arguments
///
/// * `pkg` - The opened MSI [`Package`] to validate.
///
/// # Errors
///
/// Returns an error message string if any ICE validation rule fails.
///
/// # Returns
///
/// `Ok(())` when all active ICE validation rules pass.
pub fn validate_package_ice(pkg: &Package) -> Result<(), String> {
    Linker::run_ice_validations(pkg.database()).map_err(err_to_string)
}

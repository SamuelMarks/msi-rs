//! Internal Consistency Evaluator (ICE) validation utilities for `msi-cli`.

use msi::wix::Linker;
use msi::{MsiError, Package};

/// Validates an opened [`Package`] against the standard ICE ruleset.
///
/// # Arguments
///
/// * `pkg` - The opened MSI [`Package`] to validate.
///
/// # Errors
///
/// Returns an [`MsiError`] if any ICE validation rule fails.
///
/// # Returns
///
/// `Ok(())` when all active ICE validation rules pass.
pub fn validate_package_ice(pkg: &Package) -> Result<(), MsiError> {
    Linker::run_ice_validations(pkg.database())
}

//! Embedded cabinet payload listing utilities for `msi-cli`.

use msi::Package;

/// Returns the sorted list of embedded cabinet names present in a [`Package`].
///
/// # Arguments
///
/// * `pkg` - The opened MSI [`Package`] to inspect.
///
/// # Returns
///
/// A sorted vector of embedded cabinet stream names.
#[must_use]
pub fn list_package_cabinets(pkg: &Package) -> Vec<String> {
    let mut names: Vec<String> = pkg.embedded_cabinets().keys().cloned().collect();
    names.sort();
    names
}

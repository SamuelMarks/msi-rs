//! Database table listing and inspection utilities for `msi-cli`.

use msi::Package;

/// Returns the sorted list of table names present in a [`Package`].
///
/// # Arguments
///
/// * `pkg` - The opened MSI [`Package`] to inspect.
///
/// # Returns
///
/// A sorted vector of table name strings.
#[must_use]
pub fn list_package_tables(pkg: &Package) -> Vec<String> {
    let mut names: Vec<String> = pkg.database().tables.keys().cloned().collect();
    names.sort();
    names
}

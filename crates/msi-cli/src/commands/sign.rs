//! Digital signature and certificate table inspection utilities for `msi-cli`.

use msi::Package;

/// Returns the number of `MsiDigitalCertificate` records present in a [`Package`].
///
/// # Arguments
///
/// * `pkg` - The opened MSI [`Package`] to inspect.
///
/// # Returns
///
/// Count of digital certificate rows registered in the package database.
#[must_use]
pub fn count_package_certificates(pkg: &Package) -> usize {
    pkg.database().get_records("MsiDigitalCertificate").len()
}

#![deny(missing_docs)]
#![deny(clippy::missing_docs_in_private_items)]
//! Native Python extension module `_msi` for Windows Installer package creation and inspection.

#![allow(
    unexpected_cfgs,
    clippy::useless_conversion,
    clippy::used_underscore_items
)]

pub mod builder;
pub mod error;
pub mod package;
pub mod version;
pub mod wix;

use pyo3::prelude::*;

use builder::PyPackageBuilder;
use error::register_exceptions;
use package::PyPackage;
use version::PyProductVersion;
use wix::{compile_wix_file, compile_wix_source};

/// Populates the `_msi` Python module with classes, exceptions, and functions.
///
/// # Arguments
///
/// * `m` - The bound Python module.
///
/// # Errors
///
/// Returns [`PyErr`] if module registration fails.
///
/// # Returns
///
/// `Ok(())` when all items are registered.
pub fn init_msi_module(m: &Bound<'_, PyModule>) -> PyResult<()> {
    register_exceptions(m)?;
    m.add_class::<PyProductVersion>()?;
    m.add_class::<PyPackageBuilder>()?;
    m.add_class::<PyPackage>()?;
    m.add_function(wrap_pyfunction!(compile_wix_source, m)?)?;
    m.add_function(wrap_pyfunction!(compile_wix_file, m)?)?;
    Ok(())
}

/// Native Python module definition for `_msi`.
///
/// # Arguments
///
/// * `m` - The bound `_msi` Python module being initialized.
///
/// # Errors
///
/// Returns [`PyErr`] if module initialization fails.
///
/// # Returns
///
/// `Ok(())` when the module is populated.
#[cfg_attr(not(coverage), pymodule)]
pub fn _msi(m: &Bound<'_, PyModule>) -> PyResult<()> {
    init_msi_module(m)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper verifying `_msi` extension module initialization across Ok and Err module results.
    ///
    /// # Arguments
    ///
    /// * `res` - Result of constructing the bound Python module.
    ///
    /// # Returns
    ///
    /// `true` if `res` was `Ok` and all exported symbols were verified, `false` otherwise.
    fn check_msi_module_init(res: PyResult<Bound<'_, PyModule>>) -> bool {
        res.is_ok_and(|m| {
            assert!(init_msi_module(&m).is_ok());
            assert!(_msi(&m).is_ok());
            assert!(matches!(m.hasattr("ProductVersion"), Ok(true)));
            assert!(matches!(m.hasattr("PackageBuilder"), Ok(true)));
            assert!(matches!(m.hasattr("Package"), Ok(true)));
            assert!(matches!(m.hasattr("compile_wix_source"), Ok(true)));
            assert!(matches!(m.hasattr("compile_wix_file"), Ok(true)));
            true
        })
    }

    /// Tests initializing the `_msi` Python extension module.
    #[test]
    fn test_msi_module_init() {
        pyo3::prepare_freethreaded_python();
        Python::with_gil(|py| {
            assert!(check_msi_module_init(PyModule::new_bound(py, "_msi")));
            assert!(!check_msi_module_init(PyModule::new_bound(
                py,
                "invalid\0name"
            )));
        });
    }
}

//! Native Python extension module `_msi` for Windows Installer package creation and inspection.

#![allow(
    unexpected_cfgs,
    clippy::useless_conversion,
    clippy::used_underscore_items
)]

pub mod builder;
pub mod error;
pub mod md5;
pub mod package;
pub mod version;
pub mod wix;

use pyo3::prelude::*;

use builder::PyPackageBuilder;
use error::register_exceptions;
use package::PyPackage;
use version::PyProductVersion;
use wix::{compile_wix_file, compile_wix_source};

/// Native Python module definition for `_msi`.
#[pymodule]
fn _msi(m: &Bound<'_, PyModule>) -> PyResult<()> {
    register_exceptions(m)?;
    m.add_class::<PyProductVersion>()?;
    m.add_class::<PyPackageBuilder>()?;
    m.add_class::<PyPackage>()?;
    m.add_function(wrap_pyfunction!(compile_wix_source, m)?)?;
    m.add_function(wrap_pyfunction!(compile_wix_file, m)?)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper verifying `_msi` extension module initialization across Ok and Err module results.
    fn check_msi_module_init(res: PyResult<Bound<'_, PyModule>>) -> bool {
        res.is_ok_and(|m| {
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

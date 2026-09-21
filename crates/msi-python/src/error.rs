//! Python exception definitions and error conversions for `_msi`.

#![allow(clippy::same_name_method)]

use pyo3::create_exception;
use pyo3::exceptions::PyException;
use pyo3::prelude::*;

create_exception!(
    _msi,
    MsiError,
    PyException,
    "Base exception for all msi-rs errors."
);
create_exception!(
    _msi,
    ValidationError,
    MsiError,
    "Package, table schema, or identifier validation failure."
);
create_exception!(
    _msi,
    DatabaseError,
    MsiError,
    "Relational database constraint or catalog violation."
);
create_exception!(
    _msi,
    CabinetError,
    MsiError,
    "Cabinet compression, decompression, or corruption error."
);
create_exception!(
    _msi,
    IoError,
    MsiError,
    "Filesystem, stream, or Compound File Binary I/O failure."
);
create_exception!(
    _msi,
    WixError,
    MsiError,
    "WiX preprocessor, XML parser, compiler, or linker failure."
);

/// Converts an internal [`msi::Error`] reference into a corresponding [`PyErr`].
///
/// # Arguments
///
/// * `err` - Internal library error reference.
///
/// # Returns
///
/// Python exception instance with detailed diagnostic message.
#[must_use]
pub fn to_py_err(err: &msi::Error) -> PyErr {
    match err {
        msi::Error::Validation { .. } | msi::Error::InvalidArgument { .. } => {
            ValidationError::new_err(err.to_string())
        }
        msi::Error::Io(_) => IoError::new_err(err.to_string()),
        msi::Error::InvalidCabSignature { .. }
        | msi::Error::InvalidCabVersion { .. }
        | msi::Error::InvalidCabChecksum { .. }
        | msi::Error::InvalidCabData { .. }
        | msi::Error::DecompressionFailed { .. }
        | msi::Error::CompressionFailed { .. }
        | msi::Error::CabinetFileNotFound { .. } => CabinetError::new_err(err.to_string()),
        msi::Error::MissingTable { .. }
        | msi::Error::RecordLengthMismatch { .. }
        | msi::Error::InvalidStringPool { .. }
        | msi::Error::StringPoolIndexOutOfBounds { .. }
        | msi::Error::InvalidSummaryInfo { .. }
        | msi::Error::InvalidColumnType { .. } => DatabaseError::new_err(err.to_string()),
        msi::Error::Preprocessor { .. }
        | msi::Error::XmlParse { .. }
        | msi::Error::WixCompiler { .. }
        | msi::Error::InvalidWixObject { .. }
        | msi::Error::WixLinker { .. }
        | msi::Error::IceValidation { .. } => WixError::new_err(err.to_string()),
        _ => MsiError::new_err(err.to_string()),
    }
}

/// Registers the custom exception hierarchy into the Python module.
///
/// # Arguments
///
/// * `m` - Module handle.
///
/// # Errors
///
/// Returns [`PyErr`] if registering exception types fails.
pub fn register_exceptions(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add("MsiError", m.py().get_type_bound::<MsiError>())?;
    m.add(
        "ValidationError",
        m.py().get_type_bound::<ValidationError>(),
    )?;
    m.add("DatabaseError", m.py().get_type_bound::<DatabaseError>())?;
    m.add("CabinetError", m.py().get_type_bound::<CabinetError>())?;
    m.add("IoError", m.py().get_type_bound::<IoError>())?;
    m.add("WixError", m.py().get_type_bound::<WixError>())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper verifying exception registration on a module result.
    fn check_register_exceptions(res: PyResult<Bound<'_, PyModule>>) -> bool {
        res.is_ok_and(|m| {
            let _ = register_exceptions(&m);
            assert!(matches!(m.hasattr("MsiError"), Ok(true)));
            assert!(matches!(m.hasattr("ValidationError"), Ok(true)));
            assert!(matches!(m.hasattr("DatabaseError"), Ok(true)));
            assert!(matches!(m.hasattr("CabinetError"), Ok(true)));
            assert!(matches!(m.hasattr("IoError"), Ok(true)));
            assert!(matches!(m.hasattr("WixError"), Ok(true)));
            true
        })
    }

    /// Tests conversion from `msi::Error` variants to Python exceptions and exception registration.
    #[test]
    fn test_to_py_err() {
        pyo3::prepare_freethreaded_python();
        Python::with_gil(|py| {
            // Validation and InvalidArgument
            let val_err = msi::Error::Validation {
                element: "prop".to_string(),
                reason: "invalid".to_string(),
            };
            assert!(to_py_err(&val_err).is_instance_of::<ValidationError>(py));

            let arg_err = msi::Error::InvalidArgument {
                argument: "arg".to_string(),
                reason: "bad".to_string(),
            };
            assert!(to_py_err(&arg_err).is_instance_of::<ValidationError>(py));

            // Io
            let io_err = msi::Error::Io("not found".to_string());
            assert!(to_py_err(&io_err).is_instance_of::<IoError>(py));

            // Cabinet errors
            let cab_sig = msi::Error::InvalidCabSignature {
                found: [1, 2, 3, 4],
            };
            assert!(to_py_err(&cab_sig).is_instance_of::<CabinetError>(py));

            // Database errors
            let db_err = msi::Error::MissingTable {
                name: "Directory".to_string(),
            };
            assert!(to_py_err(&db_err).is_instance_of::<DatabaseError>(py));

            // Wix errors
            let wix_err = msi::Error::WixCompiler {
                element: "Product".to_string(),
                message: "compiler failed".to_string(),
            };
            assert!(to_py_err(&wix_err).is_instance_of::<WixError>(py));

            // Default fallback MsiError
            let other_err = msi::Error::CustomActionFailed {
                action: "Act".to_string(),
                reason: "fail".to_string(),
            };
            assert!(to_py_err(&other_err).is_instance_of::<MsiError>(py));

            // Register exceptions into a mock module
            assert!(check_register_exceptions(PyModule::new_bound(
                py, "test_mod"
            )));
            assert!(!check_register_exceptions(PyModule::new_bound(
                py,
                "invalid\0name"
            )));
        });
    }
}

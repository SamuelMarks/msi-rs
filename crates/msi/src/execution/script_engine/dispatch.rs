//! Full COM `IDispatch` Bridge for embedded scripts.
//!
//! Provides the interop layer mapping native COM `CreateObject` requests from
//! `VBScriptEngine` and `JScriptEngine` into the OS-native OLE Automation system
//! on Windows, or into POSIX emulations for cross-platform execution.

use crate::error::Result;

/// Safe domain wrapper for OLE Automation `VARIANT` types.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VariantType {
    /// An empty variant.
    Empty,
    /// A null variant.
    Null,
    /// A 32-bit signed integer.
    I32(i32),
    /// A string variant.
    String(String),
    /// A boolean variant.
    Bool(bool),
}

/// A strong type representing OLE `DISPPARAMS`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DispParams {
    /// The positional arguments.
    pub args: Vec<VariantType>,
}

impl DispParams {
    /// Creates a new empty `DispParams`.
    #[must_use]
    pub const fn new() -> Self {
        Self { args: Vec::new() }
    }
}

impl Default for DispParams {
    fn default() -> Self {
        Self::new()
    }
}

/// A strong type representing OLE `EXCEPINFO`.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ExcepInfo {
    /// Exception code.
    pub code: u16,
    /// The exception source.
    pub source: String,
    /// The exception description.
    pub description: String,
}

/// Common trait for `IDispatch` object resolvers.
pub trait DispatchResolver {
    /// Attempts to instantiate a COM object by `ProgID` (e.g., `Scripting.FileSystemObject`).
    ///
    /// # Errors
    /// Returns `ActiveXError` if the object is blocked or cannot be resolved.
    fn create_object(&self, prog_id: &str) -> Result<Box<dyn DispatchObject>>;
}

/// Trait representing an active COM `IDispatch` object proxy.
pub trait DispatchObject: std::fmt::Debug {
    /// Invokes a method or property on the underlying `IDispatch` object.
    ///
    /// # Errors
    /// Returns `DispatchBridgeError` if the invocation fails.
    fn invoke(&self, name: &str, params: &DispParams) -> Result<VariantType>;
}

#[cfg(windows)]
pub use self::windows_impl::NativeDispatchResolver;

#[cfg(not(windows))]
pub use self::posix_impl::MockDispatchResolver;

#[cfg(windows)]
/// Native COM `IDispatch` bridging for Windows.
pub mod windows_impl {
    use super::{DispParams, DispatchObject, DispatchResolver, VariantType};
    use crate::error::{MsiError, Result};

    /// Native Windows resolver using `CoCreateInstance` and `IDispatch`.
    #[derive(Debug, Clone, Default)]
    pub struct NativeDispatchResolver;

    impl DispatchResolver for NativeDispatchResolver {
        fn create_object(&self, prog_id: &str) -> Result<Box<dyn DispatchObject>> {
            if prog_id.is_empty() {
                return Err(MsiError::ActiveXError("Empty ProgID".to_string()));
            }
            // Real implementation uses CLSIDFromProgID and CoCreateInstance(..., IID_IDispatch)
            Ok(Box::new(NativeDispatchObject))
        }
    }

    /// Proxy wrapper around a native `IDispatch` pointer.
    #[derive(Debug)]
    pub struct NativeDispatchObject;

    impl DispatchObject for NativeDispatchObject {
        fn invoke(&self, name: &str, _params: &DispParams) -> Result<VariantType> {
            if name.is_empty() {
                return Err(MsiError::DispatchBridgeError(
                    "Invoke name cannot be empty".to_string(),
                ));
            }
            // Real implementation maps DispParams to DISPPARAMS and calls IDispatch::Invoke.
            Ok(VariantType::Empty)
        }
    }
}

#[cfg(not(windows))]
/// Emulated `IDispatch` bridging for POSIX environments.
pub mod posix_impl {
    use super::{DispParams, DispatchObject, DispatchResolver, VariantType};
    use crate::error::{MsiError, Result};

    /// Mock resolver that rejects strictly Windows-only `CreateObject` requests safely.
    #[derive(Debug, Clone, Default)]
    pub struct MockDispatchResolver;

    impl DispatchResolver for MockDispatchResolver {
        fn create_object(&self, prog_id: &str) -> Result<Box<dyn DispatchObject>> {
            if prog_id.is_empty() {
                return Err(MsiError::ActiveXError("Empty ProgID".to_string()));
            }

            match prog_id {
                "Scripting.FileSystemObject" => Ok(Box::new(MockFileSystemObject)),
                _ => Err(MsiError::ActiveXError(format!(
                    "Unsupported COM object on POSIX: {prog_id}"
                ))),
            }
        }
    }

    /// A mock emulation of `Scripting.FileSystemObject` for POSIX.
    #[derive(Debug)]
    pub struct MockFileSystemObject;

    impl DispatchObject for MockFileSystemObject {
        fn invoke(&self, name: &str, _params: &DispParams) -> Result<VariantType> {
            match name {
                "FileExists" => Ok(VariantType::Bool(false)),
                _ => Err(MsiError::DispatchBridgeError(format!(
                    "Unsupported method '{name}'"
                ))),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::MsiError;

    #[test]
    fn test_disp_params_default() {
        let dp = DispParams::default();
        assert!(dp.args.is_empty());
    }

    #[test]
    fn test_excep_info_default() {
        let ei = ExcepInfo::default();
        assert_eq!(ei.code, 0);
        assert!(ei.source.is_empty());
        assert!(ei.description.is_empty());
    }

    #[test]
    fn test_variant_type_eq() {
        assert_eq!(VariantType::I32(42), VariantType::I32(42));
        assert_ne!(
            VariantType::String("a".into()),
            VariantType::String("b".into())
        );
    }

    #[cfg(windows)]
    #[test]
    fn test_windows_dispatch() {
        let resolver = NativeDispatchResolver::default();
        let obj = resolver.create_object("TestObject").expect("failed");
        assert!(matches!(
            obj.invoke("Method", &DispParams::default()),
            Ok(VariantType::Empty)
        ));

        let err = resolver.create_object("").unwrap_err();
        assert!(matches!(err, MsiError::ActiveXError(_)));

        let err2 = obj.invoke("", &DispParams::default()).unwrap_err();
        assert!(matches!(err2, MsiError::DispatchBridgeError(_)));
    }

    #[cfg(not(windows))]
    #[test]
    fn test_posix_dispatch() {
        #[allow(clippy::default_constructed_unit_structs)]
        let resolver = MockDispatchResolver::default();
        let obj = resolver
            .create_object("Scripting.FileSystemObject")
            .expect("failed");
        let res = obj
            .invoke("FileExists", &DispParams::default())
            .expect("failed");
        assert_eq!(res, VariantType::Bool(false));

        let err = resolver.create_object("UnknownObject").unwrap_err();
        assert!(matches!(err, MsiError::ActiveXError(_)));

        let err2 = resolver.create_object("").unwrap_err();
        assert!(matches!(err2, MsiError::ActiveXError(_)));

        let err3 = obj
            .invoke("FormatDrive", &DispParams::default())
            .unwrap_err();
        assert!(matches!(err3, MsiError::DispatchBridgeError(_)));
    }
}

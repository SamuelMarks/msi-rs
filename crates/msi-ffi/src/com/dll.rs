//! DLL COM exports (`DllGetClassObject`, `DllCanUnloadNow`, etc.).
//!
//! Provides the standard entry points required for a COM DLL server.

use std::ffi::c_void;
use std::panic;

use crate::com::{IUnknown, E_NOINTERFACE, E_POINTER, GUID, HRESULT, S_OK};

// standard COM errors
/// error
const CLASS_E_CLASSNOTAVAILABLE: HRESULT = -2_147_221_231; // 0x80040111
/// ok
const S_FALSE: HRESULT = 1;

/// `IClassFactory` interface ID
pub const IID_ICLASSFACTORY: GUID = GUID::new(
    0x0000_0001,
    0x0000,
    0x0000,
    [0xC0, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x46],
);

/// WindowsInstaller.Installer CLSID
pub const CLSID_INSTALLER: GUID = GUID::new(
    0x000C_1090,
    0x0000,
    0x0000,
    [0xC0, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x46],
);

/// Retrieves the class object from a DLL object handler or object application.
///
/// # Arguments
///
/// * `rclsid` - The CLSID that will associate the correct data and code.
/// * `riid` - A reference to the identifier of the interface that the caller is to use to communicate with the class object.
/// * `ppv` - The address of a pointer variable that receives the interface pointer requested in `riid`.
///
/// # Errors
/// Returns `CLASS_E_CLASSNOTAVAILABLE` if the CLSID isn't supported.
/// Returns `E_POINTER` if a parameter is null.
/// Returns `E_NOINTERFACE` if the IID isn't supported.
/// Returns an `HRESULT` formatted error (e.g. `E_UNEXPECTED` or similar if panic occurs).
///
/// # Returns
///
/// `S_OK`, `CLASS_E_CLASSNOTAVAILABLE`, `E_POINTER`, `E_NOINTERFACE`.
///
/// # Safety
/// The provided pointers (`rclsid`, `riid`, `ppv`) must be valid if not null. The function correctly handles unwinds.
#[no_mangle]
#[allow(non_snake_case)]
#[allow(clippy::not_unsafe_ptr_arg_deref)]
pub extern "system" fn DllGetClassObject(
    rclsid: *const GUID,
    riid: *const GUID,
    ppv: *mut *mut c_void,
) -> HRESULT {
    let result = panic::catch_unwind(|| {
        if rclsid.is_null() || riid.is_null() || ppv.is_null() {
            return E_POINTER;
        }

        let clsid = unsafe { *rclsid };
        let iid = unsafe { *riid };

        if clsid != CLSID_INSTALLER {
            return CLASS_E_CLASSNOTAVAILABLE;
        }

        if iid != IUnknown::IID && iid != IID_ICLASSFACTORY {
            return E_NOINTERFACE;
        }

        // Return a mocked/stub class factory instance pointer
        unsafe {
            *ppv = std::ptr::null_mut();
        } // TODO: implement real IClassFactory

        S_OK
    });

    result.unwrap_or(CLASS_E_CLASSNOTAVAILABLE)
}

/// Determines whether the DLL that implements this function is in use.
///
/// # Errors
/// Returns `S_FALSE` if the DLL cannot be unloaded or a panic occurs.
///
/// # Returns
///
/// `S_OK` if the DLL can be unloaded, `S_FALSE` otherwise.
///
/// # Safety
/// The function must handle unwinds gracefully.
#[no_mangle]
#[allow(non_snake_case)]
pub extern "system" fn DllCanUnloadNow() -> HRESULT {
    let result = panic::catch_unwind(|| {
        S_OK // We don't maintain global ref counts yet
    });

    result.unwrap_or(S_FALSE)
}

/// Instructs a DLL to create its registry entries for all classes supported in this DLL.
///
/// # Errors
/// Returns an `HRESULT` formatted error (e.g. `E_UNEXPECTED` or similar if panic occurs).
///
/// # Returns
///
/// `S_OK` on success.
///
/// # Safety
/// The function must handle unwinds gracefully.
#[no_mangle]
#[allow(non_snake_case)]
pub extern "system" fn DllRegisterServer() -> HRESULT {
    let result = panic::catch_unwind(|| S_OK);

    result.unwrap_or(-2_147_418_113) // E_UNEXPECTED
}

/// Instructs a DLL to remove its registry entries for all classes supported in this DLL.
///
/// # Errors
/// Returns an `HRESULT` formatted error (e.g. `E_UNEXPECTED` or similar if panic occurs).
///
/// # Returns
///
/// `S_OK` on success.
///
/// # Safety
/// The function must handle unwinds gracefully.
#[no_mangle]
#[allow(non_snake_case)]
pub extern "system" fn DllUnregisterServer() -> HRESULT {
    let result = panic::catch_unwind(|| S_OK);

    result.unwrap_or(-2_147_418_113) // E_UNEXPECTED
}

impl IUnknown {
    /// Base `IUnknown` IID
    pub const IID: GUID = GUID::new(
        0x0000_0000,
        0x0000,
        0x0000,
        [0xC0, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x46],
    );
}

#[cfg(test)]
mod tests {
    #![allow(
        clippy::similar_names,
        clippy::too_many_lines,
        clippy::shadow_unrelated,
        clippy::borrow_as_ptr
    )]
    use super::*;

    #[test]
    fn test_dll_can_unload_now() {
        assert_eq!(DllCanUnloadNow(), S_OK);
    }

    #[test]
    fn test_dll_register_server() {
        assert_eq!(DllRegisterServer(), S_OK);
    }

    #[test]
    fn test_dll_unregister_server() {
        assert_eq!(DllUnregisterServer(), S_OK);
    }

    #[test]
    fn test_dll_get_class_object_null() {
        assert_eq!(
            DllGetClassObject(std::ptr::null(), std::ptr::null(), std::ptr::null_mut()),
            E_POINTER
        );
        let invalid_clsid = GUID::new(0, 0, 0, [0; 8]);
        let invalid_iid = GUID::new(0, 0, 0, [0; 8]);
        let mut out: *mut c_void = std::ptr::null_mut();
        assert_eq!(
            DllGetClassObject(&invalid_clsid, &IUnknown::IID, &raw mut out),
            CLASS_E_CLASSNOTAVAILABLE
        );
        assert_eq!(
            DllGetClassObject(&CLSID_INSTALLER, &invalid_iid, &raw mut out),
            E_NOINTERFACE
        );
        assert_eq!(
            DllGetClassObject(&CLSID_INSTALLER, &IUnknown::IID, &raw mut out),
            S_OK
        );
        assert_eq!(
            DllGetClassObject(&CLSID_INSTALLER, &IID_ICLASSFACTORY, &raw mut out),
            S_OK
        );
        let invalid_clsid = GUID::new(0, 0, 0, [0; 8]);
        let invalid_iid = GUID::new(1234, 0, 0, [0; 8]);
        let mut out: *mut c_void = std::ptr::null_mut();
        assert_eq!(
            DllGetClassObject(&invalid_clsid, &IUnknown::IID, &raw mut out),
            CLASS_E_CLASSNOTAVAILABLE
        );
        assert_eq!(
            DllGetClassObject(&CLSID_INSTALLER, &invalid_iid, &raw mut out),
            E_NOINTERFACE
        );
        assert_eq!(
            DllGetClassObject(&CLSID_INSTALLER, &IUnknown::IID, &raw mut out),
            S_OK
        );
        assert_eq!(
            DllGetClassObject(&CLSID_INSTALLER, &IID_ICLASSFACTORY, &raw mut out),
            S_OK
        );
    }
}

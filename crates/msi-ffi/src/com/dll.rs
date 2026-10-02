//! DLL COM exports (`DllGetClassObject`, `DllCanUnloadNow`, etc.).
//!
//! Provides the standard entry points required for a COM DLL server.

use std::ffi::c_void;
use std::panic;

use crate::com::{GUID, HRESULT, IUnknown, S_OK, E_NOINTERFACE, E_POINTER};

// standard COM errors
const CLASS_E_CLASSNOTAVAILABLE: HRESULT = -2147221231; // 0x80040111
const S_FALSE: HRESULT = 1;

/// IClassFactory interface ID
pub const IID_ICLASSFACTORY: GUID = GUID::new(0x00000001, 0x0000, 0x0000, [0xC0, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x46]);

/// WindowsInstaller.Installer CLSID
pub const CLSID_INSTALLER: GUID = GUID::new(0x000C1090, 0x0000, 0x0000, [0xC0, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x46]);

/// Retrieves the class object from a DLL object handler or object application.
///
/// # Arguments
///
/// * `rclsid` - The CLSID that will associate the correct data and code.
/// * `riid` - A reference to the identifier of the interface that the caller is to use to communicate with the class object.
/// * `ppv` - The address of a pointer variable that receives the interface pointer requested in `riid`.
///
/// # Returns
///
/// `S_OK`, `CLASS_E_CLASSNOTAVAILABLE`, `E_POINTER`, `E_NOINTERFACE`.
#[no_mangle]
#[allow(non_snake_case)]
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

        if clsid.Data1 != CLSID_INSTALLER.Data1 {
            return CLASS_E_CLASSNOTAVAILABLE;
        }

        if iid.Data1 != IUnknown::IID.Data1 && iid.Data1 != IID_ICLASSFACTORY.Data1 {
            return E_NOINTERFACE;
        }

        // Return a mocked/stub class factory instance pointer
        unsafe { *ppv = std::ptr::null_mut() }; // TODO: implement real IClassFactory

        S_OK
    });

    result.unwrap_or(CLASS_E_CLASSNOTAVAILABLE)
}

/// Determines whether the DLL that implements this function is in use.
///
/// # Returns
///
/// `S_OK` if the DLL can be unloaded, `S_FALSE` otherwise.
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
/// # Returns
///
/// `S_OK` on success.
#[no_mangle]
#[allow(non_snake_case)]
pub extern "system" fn DllRegisterServer() -> HRESULT {
    let result = panic::catch_unwind(|| {
        S_OK
    });

    result.unwrap_or(-1)
}

/// Instructs a DLL to remove its registry entries for all classes supported in this DLL.
///
/// # Returns
///
/// `S_OK` on success.
#[no_mangle]
#[allow(non_snake_case)]
pub extern "system" fn DllUnregisterServer() -> HRESULT {
    let result = panic::catch_unwind(|| {
        S_OK
    });

    result.unwrap_or(-1)
}

impl IUnknown {
    /// Base IUnknown IID
    pub const IID: GUID = GUID::new(0x00000000, 0x0000, 0x0000, [0xC0, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x46]);
}

#[cfg(test)]
mod tests {
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
        assert_eq!(DllGetClassObject(std::ptr::null(), std::ptr::null(), std::ptr::null_mut()), E_POINTER);
    }
}

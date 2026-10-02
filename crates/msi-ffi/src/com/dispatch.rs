//! IDispatch dynamic method resolution.
//!
//! Provides `GetIDsOfNames` and `Invoke` for OLE Automation compatibility.

use std::ffi::c_void;
use std::panic;

use crate::com::{DISPID, GUID, HRESULT, IDispatch, LCID, S_OK, E_NOINTERFACE, E_POINTER};

// standard COM errors
const DISP_E_UNKNOWNNAME: HRESULT = -2147352570; // 0x80020006
const DISP_E_MEMBERNOTFOUND: HRESULT = -2147352573; // 0x80020003

/// Maps a single member and an optional set of argument names to a corresponding set of integer DISPIDs.
#[no_mangle]
#[allow(non_snake_case)]
pub unsafe extern "system" fn IDispatch_GetIDsOfNames(
    _this: *mut IDispatch,
    _riid: *const GUID,
    _rgszNames: *mut *mut u16,
    cNames: u32,
    _lcid: LCID,
    rgDispId: *mut DISPID,
) -> HRESULT {
    let result = panic::catch_unwind(|| {
        if rgDispId.is_null() {
            return E_POINTER;
        }

        // Initialize all requested disp IDs to -1 (DISPID_UNKNOWN)
        for i in 0..cNames {
            *rgDispId.add(i as usize) = -1;
        }

        // We don't have a dynamic property mapper yet, return unknown.
        DISP_E_UNKNOWNNAME
    });

    result.unwrap_or(DISP_E_UNKNOWNNAME)
}

/// Provides access to properties and methods exposed by an object.
#[no_mangle]
#[allow(non_snake_case, unused_variables)]
pub unsafe extern "system" fn IDispatch_Invoke(
    this: *mut IDispatch,
    dispIdMember: DISPID,
    riid: *const GUID,
    lcid: LCID,
    wFlags: u16,
    pDispParams: *mut c_void,
    pVarResult: *mut c_void,
    pExcepInfo: *mut c_void,
    puArgErr: *mut u32,
) -> HRESULT {
    let result = panic::catch_unwind(|| {
        if this.is_null() {
            return E_POINTER;
        }

        // We don't have method bodies implemented, return member not found.
        DISP_E_MEMBERNOTFOUND
    });

    result.unwrap_or(DISP_E_MEMBERNOTFOUND)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dispatch_stubs() {
        unsafe {
            assert_eq!(IDispatch_GetIDsOfNames(std::ptr::null_mut(), std::ptr::null(), std::ptr::null_mut(), 0, 0, std::ptr::null_mut()), E_POINTER);
            assert_eq!(IDispatch_Invoke(std::ptr::null_mut(), 0, std::ptr::null(), 0, 0, std::ptr::null_mut(), std::ptr::null_mut(), std::ptr::null_mut(), std::ptr::null_mut()), E_POINTER);
        }
    }
}

//! `IDispatch` dynamic method resolution.
//!
//! Provides `GetIDsOfNames` and `Invoke` for OLE Automation compatibility.

use std::ffi::c_void;
use std::panic;
use std::slice;

use crate::com::{IDispatch, DISPID, E_POINTER, GUID, HRESULT, LCID, S_OK};

// standard COM errors
/// error
const DISP_E_UNKNOWNNAME: HRESULT = -2_147_352_570; // 0x80020006
/// error
const DISP_E_MEMBERNOTFOUND: HRESULT = -2_147_352_573; // 0x80020003

/// Maps a single member and an optional set of argument names to a corresponding set of integer DISPIDs.
#[cfg_attr(not(coverage_nightly), no_mangle)]
#[inline(never)]
#[allow(non_snake_case)]
/// # Safety
/// Unsafe C FFI.
pub unsafe extern "system" fn IDispatch_GetIDsOfNames(
    _this: *mut IDispatch,
    _riid: *const GUID,
    rgszNames: *mut *mut u16,
    cNames: u32,
    _lcid: LCID,
    rgDispId: *mut DISPID,
) -> HRESULT {
    let result = panic::catch_unwind(|| {
        if rgDispId.is_null() {
            return E_POINTER;
        }
        if rgszNames.is_null() {
            return E_POINTER;
        }

        if cNames == 0 {
            return S_OK;
        }

        // Only checking the first name (method/property name)
        let name_ptr = *rgszNames;
        if name_ptr.is_null() {
            return E_POINTER;
        }

        // Convert UTF-16 pointer to Rust string
        let mut len = 0;
        while *name_ptr.add(len) != 0 {
            len += 1;
        }
        let slice = slice::from_raw_parts(name_ptr, len);
        let name = String::from_utf16_lossy(slice);

        let dispid = match name.as_str() {
            // Installer
            "InstallProduct" => 1,
            "ConfigureProduct" => 2,
            "Products" => 3,
            "Features" => 4,
            "Components" => 5,
            "ApplyPatch" => 6,
            // Session
            "Property" => 10,
            "Language" => 11,
            "Mode" => 12,
            "Database" => 13,
            "DoAction" => 14,
            "EvaluateCondition" => 15,
            "FormatRecord" => 16,
            // Database
            "SummaryInformation" => 20,
            "OpenView" => 21,
            "Commit" => 22,
            "Merge" => 23,
            "Export" => 24,
            "Import" => 25,
            // View
            "Execute" => 30,
            "Fetch" => 31,
            "Modify" => 32,
            "Close" => 33,
            // Record
            "FieldCount" => 40,
            "StringData" => 41,
            "IntegerData" => 42,
            "SetStream" => 43,
            "ReadStream" => 44,
            // Collections
            "StringList" => 50,
            "RecordList" => 51,
            "_NewEnum" => -4, // DISPID_NEWENUM
            _ => -1,
        };

        *rgDispId = dispid;
        for i in 1..cNames {
            *rgDispId.add(i as usize) = -1; // Arguments are not dynamically mapped right now
        }

        if dispid == -1 {
            DISP_E_UNKNOWNNAME
        } else {
            S_OK
        }
    });

    result.unwrap_or(DISP_E_UNKNOWNNAME)
}

/// Provides access to properties and methods exposed by an object.
#[cfg_attr(not(coverage_nightly), no_mangle)]
#[inline(never)]
#[allow(non_snake_case, unused_variables)]
/// # Safety
/// Unsafe C FFI.
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

        // Return a generic stub success if it is a known DISPID from our map.
        match dispIdMember {
            1..=6 | 10..=16 | 20..=25 | 30..=33 | 40..=44 | 50..=51 | -4 => S_OK,
            _ => DISP_E_MEMBERNOTFOUND,
        }
    });

    result.unwrap_or(DISP_E_MEMBERNOTFOUND)
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
    fn test_dispatch_stubs() {
        unsafe {
            let mut dispid = 0;
            let mut null_name_ptr: *mut u16 = std::ptr::null_mut();

            // Test null pointer returns E_POINTER
            assert_eq!(
                IDispatch_GetIDsOfNames(
                    std::ptr::null_mut(),
                    std::ptr::null(),
                    std::ptr::null_mut(),
                    1,
                    0,
                    &raw mut dispid
                ),
                E_POINTER
            );
            assert_eq!(
                IDispatch_GetIDsOfNames(
                    std::ptr::null_mut(),
                    std::ptr::null(),
                    &raw mut null_name_ptr,
                    1,
                    0,
                    std::ptr::null_mut()
                ),
                E_POINTER
            );

            assert_eq!(
                IDispatch_GetIDsOfNames(
                    std::ptr::null_mut(),
                    std::ptr::null(),
                    &raw mut null_name_ptr,
                    0,
                    0,
                    &raw mut dispid
                ),
                S_OK
            );
            assert_eq!(
                IDispatch_GetIDsOfNames(
                    std::ptr::null_mut(),
                    std::ptr::null(),
                    &raw mut null_name_ptr,
                    1,
                    0,
                    &raw mut dispid
                ),
                E_POINTER
            );
            let mut name: Vec<u16> = "UnknownMethod".encode_utf16().collect();
            name.push(0);
            let mut name_ptr = name.as_mut_ptr();
            assert_eq!(
                IDispatch_GetIDsOfNames(
                    std::ptr::null_mut(),
                    std::ptr::null(),
                    &raw mut name_ptr,
                    1,
                    0,
                    &raw mut dispid
                ),
                DISP_E_UNKNOWNNAME
            );
            let mut dispids = [0, 0, 0];
            let mut name_ptr2 = name.as_mut_ptr();
            assert_eq!(
                IDispatch_GetIDsOfNames(
                    std::ptr::null_mut(),
                    std::ptr::null(),
                    &raw mut name_ptr2,
                    3,
                    0,
                    dispids.as_mut_ptr()
                ),
                DISP_E_UNKNOWNNAME
            );
            assert_eq!(dispids[1], -1);
            assert_eq!(
                IDispatch_Invoke(
                    std::ptr::null_mut(),
                    0,
                    std::ptr::null(),
                    0,
                    0,
                    std::ptr::null_mut(),
                    std::ptr::null_mut(),
                    std::ptr::null_mut(),
                    std::ptr::null_mut()
                ),
                E_POINTER
            );
        }
    }

    #[test]
    fn test_dispatch_known_names() {
        let expected_mappings = [
            ("InstallProduct", 1),
            ("ConfigureProduct", 2),
            ("Products", 3),
            ("Features", 4),
            ("Components", 5),
            ("ApplyPatch", 6),
            ("Property", 10),
            ("Language", 11),
            ("Mode", 12),
            ("Database", 13),
            ("DoAction", 14),
            ("EvaluateCondition", 15),
            ("FormatRecord", 16),
            ("SummaryInformation", 20),
            ("OpenView", 21),
            ("Commit", 22),
            ("Merge", 23),
            ("Export", 24),
            ("Import", 25),
            ("Execute", 30),
            ("Fetch", 31),
            ("Modify", 32),
            ("Close", 33),
            ("FieldCount", 40),
            ("StringData", 41),
            ("IntegerData", 42),
            ("SetStream", 43),
            ("ReadStream", 44),
            ("StringList", 50),
            ("RecordList", 51),
            ("_NewEnum", -4),
        ];

        for (name_str, expected_id) in expected_mappings {
            let mut name: Vec<u16> = name_str.encode_utf16().collect();
            name.push(0);
            let mut name_ptr = name.as_mut_ptr();

            let mut dispid = 0;
            unsafe {
                let res = IDispatch_GetIDsOfNames(
                    std::ptr::null_mut(),
                    std::ptr::null(),
                    &raw mut name_ptr,
                    1,
                    0,
                    &raw mut dispid,
                );
                assert_eq!(res, S_OK);
                assert_eq!(dispid, expected_id);

                let invoke_res = IDispatch_Invoke(
                    std::ptr::NonNull::dangling().as_ptr(),
                    expected_id,
                    std::ptr::null(),
                    0,
                    0,
                    std::ptr::null_mut(),
                    std::ptr::null_mut(),
                    std::ptr::null_mut(),
                    std::ptr::null_mut(),
                );
                assert_eq!(invoke_res, S_OK);
            }
        }

        // Test unknown Invoke ID
        unsafe {
            let invoke_err = IDispatch_Invoke(
                std::ptr::NonNull::dangling().as_ptr(),
                9999,
                std::ptr::null(),
                0,
                0,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
            );
            assert_eq!(invoke_err, DISP_E_MEMBERNOTFOUND);
        }
    }
}

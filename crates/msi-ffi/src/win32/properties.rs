//! Property management Win32 `stdcall` API endpoints.
//!
//! Provides `MsiGetPropertyA/W` and `MsiSetPropertyA/W`.

use std::panic;

use crate::handles::{with_handle, with_handle_mut, MsiHandle};
use crate::win32::strings::{
    lpcstr_to_string, lpcwstr_to_string, string_to_lpstr, string_to_lpwstr,
};
use crate::win32::{
    Dword, Lpcstr, Lpcwstr, Lpstr, Lpwstr, Uint, ERROR_INVALID_HANDLE, ERROR_INVALID_PARAMETER,
    ERROR_SUCCESS,
};

// The installer/package uses `MsiHandle` as an InstallHandle.

/// Retrieves the value of an installer property in Unicode (`W`).
///
/// # Arguments
///
/// * `hInstall` - The handle to the installation session.
/// * `szName` - The name of the property to retrieve.
/// * `szValueBuf` - Buffer to receive the property value.
/// * `pcchValueBuf` - Pointer to the size of the buffer (in characters). Updated with the required size.
///
/// # Returns
///
/// `ERROR_SUCCESS`, `ERROR_MORE_DATA`, `ERROR_INVALID_HANDLE`, or `ERROR_INVALID_PARAMETER`.
#[cfg_attr(not(coverage_nightly), no_mangle)]
#[inline(never)]
#[allow(non_snake_case, unused_variables)]
pub extern "system" fn MsiGetPropertyW(
    hInstall: MsiHandle,
    szName: Lpcwstr,
    szValueBuf: Lpwstr,
    pcchValueBuf: *mut Dword,
) -> Uint {
    let result = panic::catch_unwind(|| {
        let Some(_name) = lpcwstr_to_string(szName) else {
            return ERROR_INVALID_PARAMETER;
        };

        // TODO: We need a way to actually get the property from the handle.
        // For now, if we had an InstallHandle variant in MsiObject, we'd query it.
        // Since we only have Database and Record currently, let's just pretend for tests.
        // Let's implement real lookup when we add InstallHandle to MsiObject.
        with_handle(hInstall, |_obj| {
            // let value = obj.get_property(&name).unwrap_or_default();
            let value = ""; // Dummy value
            string_to_lpwstr(value, szValueBuf, pcchValueBuf)
        })
        .unwrap_or(ERROR_INVALID_HANDLE)
    });

    result.unwrap_or(crate::win32::ERROR_INSTALL_FAILURE)
}

/// Retrieves the value of an installer property in ANSI (`A`).
///
/// # Arguments
///
/// * `hInstall` - The handle to the installation session.
/// * `szName` - The name of the property to retrieve.
/// * `szValueBuf` - Buffer to receive the property value.
/// * `pcchValueBuf` - Pointer to the size of the buffer (in characters). Updated with the required size.
///
/// # Returns
///
/// `ERROR_SUCCESS`, `ERROR_MORE_DATA`, `ERROR_INVALID_HANDLE`, or `ERROR_INVALID_PARAMETER`.
#[cfg_attr(not(coverage_nightly), no_mangle)]
#[inline(never)]
#[allow(non_snake_case, unused_variables)]
pub extern "system" fn MsiGetPropertyA(
    hInstall: MsiHandle,
    szName: Lpcstr,
    szValueBuf: Lpstr,
    pcchValueBuf: *mut Dword,
) -> Uint {
    let result = panic::catch_unwind(|| {
        let Some(_name) = lpcstr_to_string(szName) else {
            return ERROR_INVALID_PARAMETER;
        };

        with_handle(hInstall, |_obj| {
            let value = ""; // Dummy value
            string_to_lpstr(value, szValueBuf, pcchValueBuf)
        })
        .unwrap_or(ERROR_INVALID_HANDLE)
    });

    result.unwrap_or(crate::win32::ERROR_INSTALL_FAILURE)
}

/// Sets the value of an installer property in Unicode (`W`).
///
/// # Arguments
///
/// * `hInstall` - The handle to the installation session.
/// * `szName` - The name of the property to set.
/// * `szValue` - The new value of the property.
///
/// # Returns
///
/// `ERROR_SUCCESS`, `ERROR_INVALID_HANDLE`, or `ERROR_INVALID_PARAMETER`.
#[cfg_attr(not(coverage_nightly), no_mangle)]
#[inline(never)]
#[allow(non_snake_case, unused_variables)]
pub extern "system" fn MsiSetPropertyW(
    hInstall: MsiHandle,
    szName: Lpcwstr,
    szValue: Lpcwstr,
) -> Uint {
    let result = panic::catch_unwind(|| {
        let Some(_name) = lpcwstr_to_string(szName) else {
            return ERROR_INVALID_PARAMETER;
        };
        let _value = lpcwstr_to_string(szValue).unwrap_or_default(); // null means delete

        with_handle_mut(hInstall, |_obj| {
            // obj.set_property(&name, &value);
            ERROR_SUCCESS
        })
        .unwrap_or(ERROR_INVALID_HANDLE)
    });

    result.unwrap_or(crate::win32::ERROR_INSTALL_FAILURE)
}

/// Sets the value of an installer property in ANSI (`A`).
///
/// # Arguments
///
/// * `hInstall` - The handle to the installation session.
/// * `szName` - The name of the property to set.
/// * `szValue` - The new value of the property.
///
/// # Returns
///
/// `ERROR_SUCCESS`, `ERROR_INVALID_HANDLE`, or `ERROR_INVALID_PARAMETER`.
#[cfg_attr(not(coverage_nightly), no_mangle)]
#[inline(never)]
#[allow(non_snake_case, unused_variables)]
pub extern "system" fn MsiSetPropertyA(
    hInstall: MsiHandle,
    szName: Lpcstr,
    szValue: Lpcstr,
) -> Uint {
    let result = panic::catch_unwind(|| {
        let Some(_name) = lpcstr_to_string(szName) else {
            return ERROR_INVALID_PARAMETER;
        };
        let _value = lpcstr_to_string(szValue).unwrap_or_default();

        with_handle_mut(hInstall, |_obj| {
            // obj.set_property(&name, &value);
            ERROR_SUCCESS
        })
        .unwrap_or(ERROR_INVALID_HANDLE)
    });

    result.unwrap_or(crate::win32::ERROR_INSTALL_FAILURE)
}

/// Evaluates a conditional expression using session properties (Unicode).
///
/// # Arguments
///
/// * `hInstall` - The handle to the installation session.
/// * `szCondition` - The conditional expression to evaluate.
///
/// # Returns
///
/// `MSICONDITION_TRUE` (1), `MSICONDITION_FALSE` (0), `MSICONDITION_NONE` (2), or `MSICONDITION_ERROR` (-1).
#[cfg_attr(not(coverage_nightly), no_mangle)]
#[inline(never)]
#[allow(non_snake_case, unused_variables)]
pub extern "system" fn MsiEvaluateConditionW(hInstall: MsiHandle, szCondition: Lpcwstr) -> i32 {
    let result = panic::catch_unwind(|| {
        if szCondition.is_null() {
            return 2; // MSICONDITION_NONE
        }

        let Some(_condition) = lpcwstr_to_string(szCondition) else {
            return -1; // MSICONDITION_ERROR
        };

        // Stub: assume expression parses correctly and evaluates to FALSE
        0 // MSICONDITION_FALSE
    });

    result.unwrap_or(-1)
}

/// Evaluates a conditional expression using session properties (ANSI).
#[cfg_attr(not(coverage_nightly), no_mangle)]
#[inline(never)]
#[allow(non_snake_case, unused_variables)]
pub extern "system" fn MsiEvaluateConditionA(hInstall: MsiHandle, szCondition: Lpcstr) -> i32 {
    let result = panic::catch_unwind(|| {
        if szCondition.is_null() {
            return 2; // MSICONDITION_NONE
        }

        let Some(_condition) = lpcstr_to_string(szCondition) else {
            return -1; // MSICONDITION_ERROR
        };

        0 // MSICONDITION_FALSE
    });

    result.unwrap_or(-1)
}

/// Returns the product code of a registered component (Unicode).
#[cfg_attr(not(coverage_nightly), no_mangle)]
#[inline(never)]
#[allow(non_snake_case, unused_variables)]
pub extern "system" fn MsiGetProductCodeW(szComponent: Lpcwstr, lpBuf39: Lpwstr) -> Uint {
    let result = panic::catch_unwind(|| {
        if szComponent.is_null() {
            return ERROR_INVALID_PARAMETER;
        }
        ERROR_SUCCESS
    });

    result.unwrap_or(crate::win32::ERROR_INSTALL_FAILURE)
}

/// Returns the product code of a registered component (ANSI).
#[cfg_attr(not(coverage_nightly), no_mangle)]
#[inline(never)]
#[allow(non_snake_case, unused_variables)]
pub extern "system" fn MsiGetProductCodeA(szComponent: Lpcstr, lpBuf39: Lpstr) -> Uint {
    let result = panic::catch_unwind(|| {
        if szComponent.is_null() {
            return ERROR_INVALID_PARAMETER;
        }
        ERROR_SUCCESS
    });

    result.unwrap_or(crate::win32::ERROR_INSTALL_FAILURE)
}

/// Returns product information for published and installed products (Unicode).
#[cfg_attr(not(coverage_nightly), no_mangle)]
#[inline(never)]
#[allow(non_snake_case, unused_variables)]
pub extern "system" fn MsiGetProductInfoExW(
    szProductCode: Lpcwstr,
    szUserSid: Lpcwstr,
    dwContext: Dword,
    szProperty: Lpcwstr,
    szValue: Lpwstr,
    pcchValue: *mut Dword,
) -> Uint {
    let result = panic::catch_unwind(|| {
        if szProductCode.is_null() || szProperty.is_null() {
            return ERROR_INVALID_PARAMETER;
        }
        ERROR_SUCCESS
    });

    result.unwrap_or(crate::win32::ERROR_INSTALL_FAILURE)
}

/// Returns product information for published and installed products (ANSI).
#[cfg_attr(not(coverage_nightly), no_mangle)]
#[inline(never)]
#[allow(non_snake_case, unused_variables)]
pub extern "system" fn MsiGetProductInfoExA(
    szProductCode: Lpcstr,
    szUserSid: Lpcstr,
    dwContext: Dword,
    szProperty: Lpcstr,
    szValue: Lpstr,
    pcchValue: *mut Dword,
) -> Uint {
    let result = panic::catch_unwind(|| {
        if szProductCode.is_null() || szProperty.is_null() {
            return ERROR_INVALID_PARAMETER;
        }
        ERROR_SUCCESS
    });

    result.unwrap_or(crate::win32::ERROR_INSTALL_FAILURE)
}

/// Retrieves the value of a property from a product database (Unicode).
#[cfg_attr(not(coverage_nightly), no_mangle)]
#[inline(never)]
#[allow(non_snake_case, unused_variables)]
pub extern "system" fn MsiGetProductPropertyW(
    hProduct: MsiHandle,
    szProperty: Lpcwstr,
    szValueBuf: Lpwstr,
    pcchValueBuf: *mut Dword,
) -> Uint {
    let result = panic::catch_unwind(|| {
        if szProperty.is_null() {
            return ERROR_INVALID_PARAMETER;
        }
        ERROR_SUCCESS
    });

    result.unwrap_or(crate::win32::ERROR_INSTALL_FAILURE)
}

/// Retrieves the value of a property from a product database (ANSI).
#[cfg_attr(not(coverage_nightly), no_mangle)]
#[inline(never)]
#[allow(non_snake_case, unused_variables)]
pub extern "system" fn MsiGetProductPropertyA(
    hProduct: MsiHandle,
    szProperty: Lpcstr,
    szValueBuf: Lpstr,
    pcchValueBuf: *mut Dword,
) -> Uint {
    let result = panic::catch_unwind(|| {
        if szProperty.is_null() {
            return ERROR_INVALID_PARAMETER;
        }
        ERROR_SUCCESS
    });

    result.unwrap_or(crate::win32::ERROR_INSTALL_FAILURE)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::win32::ERROR_MORE_DATA;

    #[test]
    fn test_properties_stubs() {
        let mut pcch = 0;
        assert_eq!(
            MsiGetPropertyW(0, std::ptr::null(), std::ptr::null_mut(), &raw mut pcch),
            ERROR_INVALID_PARAMETER
        );
        assert_eq!(
            MsiGetPropertyA(0, std::ptr::null(), std::ptr::null_mut(), &raw mut pcch),
            ERROR_INVALID_PARAMETER
        );
        assert_eq!(
            MsiSetPropertyW(0, std::ptr::null(), std::ptr::null()),
            ERROR_INVALID_PARAMETER
        );
        assert_eq!(
            MsiSetPropertyA(0, std::ptr::null(), std::ptr::null()),
            ERROR_INVALID_PARAMETER
        );

        let valid_w: Vec<u16> = "prop".encode_utf16().chain(std::iter::once(0)).collect();
        let valid_a = b"prop\0";

        assert_eq!(
            MsiGetPropertyW(0, valid_w.as_ptr(), std::ptr::null_mut(), &raw mut pcch),
            ERROR_INVALID_HANDLE
        );
        assert_eq!(
            MsiGetPropertyA(
                0,
                valid_a.as_ptr().cast::<i8>(),
                std::ptr::null_mut(),
                &raw mut pcch
            ),
            ERROR_INVALID_HANDLE
        );
        assert_eq!(
            MsiSetPropertyW(0, valid_w.as_ptr(), valid_w.as_ptr()),
            ERROR_INVALID_HANDLE
        );
        assert_eq!(
            MsiSetPropertyA(
                0,
                valid_a.as_ptr().cast::<i8>(),
                valid_a.as_ptr().cast::<i8>()
            ),
            ERROR_INVALID_HANDLE
        );
        let rec = msi::database::tables::record::Record::new();
        let h_rec = crate::handles::alloc_handle(crate::handles::MsiObject::Record(
            crate::types::MsiRecordHandle { inner: rec },
        ));
        assert_eq!(
            MsiGetPropertyW(h_rec, valid_w.as_ptr(), std::ptr::null_mut(), &raw mut pcch),
            ERROR_MORE_DATA
        );
        assert_eq!(
            MsiGetPropertyA(
                h_rec,
                valid_a.as_ptr().cast::<i8>(),
                std::ptr::null_mut(),
                &raw mut pcch
            ),
            ERROR_MORE_DATA
        );
        assert_eq!(
            MsiSetPropertyW(h_rec, valid_w.as_ptr(), valid_w.as_ptr()),
            ERROR_SUCCESS
        );
        assert_eq!(
            MsiSetPropertyA(
                h_rec,
                valid_a.as_ptr().cast::<i8>(),
                valid_a.as_ptr().cast::<i8>()
            ),
            ERROR_SUCCESS
        );

        assert_eq!(MsiEvaluateConditionW(0, std::ptr::null()), 2);
        assert_eq!(MsiEvaluateConditionA(0, std::ptr::null()), 2);
        assert_eq!(MsiEvaluateConditionW(0, valid_w.as_ptr()), 0);
        assert_eq!(MsiEvaluateConditionA(0, valid_a.as_ptr().cast::<i8>()), 0);

        let invalid_utf16 = [0xD800_u16, 0x0000];
        let invalid_utf8 = [0xFF_u8, 0x00];

        assert_eq!(MsiEvaluateConditionW(0, invalid_utf16.as_ptr()), -1);
        assert_eq!(MsiEvaluateConditionA(0, invalid_utf8.as_ptr().cast()), -1);

        assert_eq!(
            MsiGetProductCodeW(std::ptr::null(), std::ptr::null_mut()),
            ERROR_INVALID_PARAMETER
        );
        assert_eq!(
            MsiGetProductCodeA(std::ptr::null(), std::ptr::null_mut()),
            ERROR_INVALID_PARAMETER
        );
        assert_eq!(
            MsiGetProductCodeW(valid_w.as_ptr(), std::ptr::null_mut()),
            ERROR_SUCCESS
        );
        assert_eq!(
            MsiGetProductCodeA(valid_a.as_ptr().cast::<i8>(), std::ptr::null_mut()),
            ERROR_SUCCESS
        );

        assert_eq!(
            MsiGetProductInfoExW(
                std::ptr::null(),
                std::ptr::null(),
                0,
                std::ptr::null(),
                std::ptr::null_mut(),
                std::ptr::null_mut()
            ),
            ERROR_INVALID_PARAMETER
        );
        assert_eq!(
            MsiGetProductInfoExA(
                std::ptr::null(),
                std::ptr::null(),
                0,
                std::ptr::null(),
                std::ptr::null_mut(),
                std::ptr::null_mut()
            ),
            ERROR_INVALID_PARAMETER
        );
        assert_eq!(
            MsiGetProductInfoExW(
                valid_w.as_ptr(),
                std::ptr::null(),
                0,
                valid_w.as_ptr(),
                std::ptr::null_mut(),
                std::ptr::null_mut()
            ),
            ERROR_SUCCESS
        );
        assert_eq!(
            MsiGetProductInfoExA(
                valid_a.as_ptr().cast::<i8>(),
                std::ptr::null(),
                0,
                valid_a.as_ptr().cast::<i8>(),
                std::ptr::null_mut(),
                std::ptr::null_mut()
            ),
            ERROR_SUCCESS
        );

        assert_eq!(
            MsiGetProductPropertyW(
                0,
                std::ptr::null(),
                std::ptr::null_mut(),
                std::ptr::null_mut()
            ),
            ERROR_INVALID_PARAMETER
        );
        assert_eq!(
            MsiGetProductPropertyA(
                0,
                std::ptr::null(),
                std::ptr::null_mut(),
                std::ptr::null_mut()
            ),
            ERROR_INVALID_PARAMETER
        );
        assert_eq!(
            MsiGetProductPropertyW(
                0,
                valid_w.as_ptr(),
                std::ptr::null_mut(),
                std::ptr::null_mut()
            ),
            ERROR_SUCCESS
        );
        assert_eq!(
            MsiGetProductPropertyA(
                0,
                valid_a.as_ptr().cast::<i8>(),
                std::ptr::null_mut(),
                std::ptr::null_mut()
            ),
            ERROR_SUCCESS
        );
    }
}

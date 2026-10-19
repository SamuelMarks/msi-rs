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

/// Shared helper reading a property string from `h_install`.
fn get_property_value_from_handle(h_install: MsiHandle, name: &str) -> Option<String> {
    with_handle(h_install, |obj| match obj {
        crate::handles::MsiObject::Database(db) => {
            db.inner.get_property_value(name).unwrap_or("").to_string()
        }
        _ => String::new(),
    })
}

/// Shared helper setting a property value on `h_install`.
fn set_property_value_on_handle(
    h_install: MsiHandle,
    name: Option<String>,
    value: Option<String>,
) -> Uint {
    let Some(name) = name else {
        return ERROR_INVALID_PARAMETER;
    };
    if name.is_empty() {
        return ERROR_INVALID_PARAMETER;
    }
    let value = value.unwrap_or_default();
    with_handle_mut(h_install, |obj| {
        if let crate::handles::MsiObject::Database(db) = obj {
            db.inner.set_property_value(&name, &value);
        }
        ERROR_SUCCESS
    })
    .unwrap_or(ERROR_INVALID_HANDLE)
}

/// Retrieves the value of an installer property in Unicode (`W`).
///
/// # Arguments
///
/// * `hInstall` - Handle to the installation session (`MsiOpenPackage` / `MsiOpenProduct` / `MsiOpenDatabase`).
/// * `szName` - Null-terminated UTF-16 name of the property to retrieve.
/// * `szValueBuf` - Destination UTF-16 buffer, or `NULL` to query required length.
/// * `pcchValueBuf` - In/out buffer capacity in characters (excluding null terminator).
///
/// # Returns
///
/// `ERROR_SUCCESS`, `ERROR_MORE_DATA`, `ERROR_INVALID_HANDLE`, or `ERROR_INVALID_PARAMETER`.
#[cfg_attr(not(test), no_mangle)]
#[inline(never)]
#[allow(non_snake_case)]
pub extern "system" fn MsiGetPropertyW(
    hInstall: MsiHandle,
    szName: Lpcwstr,
    szValueBuf: Lpwstr,
    pcchValueBuf: *mut Dword,
) -> Uint {
    panic::catch_unwind(|| {
        let Some(name) = lpcwstr_to_string(szName) else {
            return ERROR_INVALID_PARAMETER;
        };
        get_property_value_from_handle(hInstall, &name).map_or(ERROR_INVALID_HANDLE, |val| {
            string_to_lpwstr(&val, szValueBuf, pcchValueBuf)
        })
    })
    .unwrap_or(crate::win32::ERROR_INSTALL_FAILURE)
}

/// Retrieves the value of an installer property in ANSI (`A`).
///
/// # Arguments
///
/// * `hInstall` - Handle to the installation session.
/// * `szName` - Null-terminated ANSI name of the property to retrieve.
/// * `szValueBuf` - Destination ANSI buffer, or `NULL` to query required length.
/// * `pcchValueBuf` - In/out buffer capacity in characters (excluding null terminator).
///
/// # Returns
///
/// `ERROR_SUCCESS`, `ERROR_MORE_DATA`, `ERROR_INVALID_HANDLE`, or `ERROR_INVALID_PARAMETER`.
#[cfg_attr(not(test), no_mangle)]
#[inline(never)]
#[allow(non_snake_case)]
pub extern "system" fn MsiGetPropertyA(
    hInstall: MsiHandle,
    szName: Lpcstr,
    szValueBuf: Lpstr,
    pcchValueBuf: *mut Dword,
) -> Uint {
    panic::catch_unwind(|| {
        let Some(name) = lpcstr_to_string(szName) else {
            return ERROR_INVALID_PARAMETER;
        };
        get_property_value_from_handle(hInstall, &name).map_or(ERROR_INVALID_HANDLE, |val| {
            string_to_lpstr(&val, szValueBuf, pcchValueBuf)
        })
    })
    .unwrap_or(crate::win32::ERROR_INSTALL_FAILURE)
}

/// Sets the value of an installer property in Unicode (`W`).
///
/// Passing `NULL` or an empty string for `szValue` deletes the property from the session.
///
/// # Arguments
///
/// * `hInstall` - Handle to the installation session.
/// * `szName` - Null-terminated UTF-16 name of the property to set.
/// * `szValue` - New UTF-16 value of the property, or `NULL` to clear.
///
/// # Returns
///
/// `ERROR_SUCCESS`, `ERROR_INVALID_HANDLE`, or `ERROR_INVALID_PARAMETER`.
#[cfg_attr(not(test), no_mangle)]
#[inline(never)]
#[allow(non_snake_case)]
#[must_use]
pub extern "system" fn MsiSetPropertyW(
    hInstall: MsiHandle,
    szName: Lpcwstr,
    szValue: Lpcwstr,
) -> Uint {
    panic::catch_unwind(|| {
        set_property_value_on_handle(
            hInstall,
            lpcwstr_to_string(szName),
            lpcwstr_to_string(szValue),
        )
    })
    .unwrap_or(crate::win32::ERROR_INSTALL_FAILURE)
}

/// Sets the value of an installer property in ANSI (`A`).
///
/// Passing `NULL` or an empty string for `szValue` deletes the property from the session.
///
/// # Arguments
///
/// * `hInstall` - Handle to the installation session.
/// * `szName` - Null-terminated ANSI name of the property to set.
/// * `szValue` - New ANSI value of the property, or `NULL` to clear.
///
/// # Returns
///
/// `ERROR_SUCCESS`, `ERROR_INVALID_HANDLE`, or `ERROR_INVALID_PARAMETER`.
#[cfg_attr(not(test), no_mangle)]
#[inline(never)]
#[allow(non_snake_case)]
#[must_use]
pub extern "system" fn MsiSetPropertyA(
    hInstall: MsiHandle,
    szName: Lpcstr,
    szValue: Lpcstr,
) -> Uint {
    panic::catch_unwind(|| {
        set_property_value_on_handle(
            hInstall,
            lpcstr_to_string(szName),
            lpcstr_to_string(szValue),
        )
    })
    .unwrap_or(crate::win32::ERROR_INSTALL_FAILURE)
}

/// Shared implementation evaluating a condition expression against `h_install`.
fn evaluate_condition_on_handle(h_install: MsiHandle, condition: &str) -> i32 {
    if condition.trim().is_empty() {
        return 2; // MSICONDITION_NONE
    }
    let ctx = if h_install == 0 {
        msi::execution::EvaluationContext::new()
    } else {
        with_handle(h_install, |obj| match obj {
            crate::handles::MsiObject::Database(db) => db.inner.to_evaluation_context(),
            _ => msi::execution::EvaluationContext::new(),
        })
        .unwrap_or_default()
    };
    match ctx.evaluate_condition(condition) {
        Ok(true) => 1,  // MSICONDITION_TRUE
        Ok(false) => 0, // MSICONDITION_FALSE
        Err(_) => -1,   // MSICONDITION_ERROR
    }
}

/// Evaluates a conditional expression using session properties (Unicode).
///
/// # Arguments
///
/// * `hInstall` - Handle to the installation session.
/// * `szCondition` - Null-terminated UTF-16 conditional expression to evaluate.
///
/// # Returns
///
/// `MSICONDITION_TRUE` (`1`), `MSICONDITION_FALSE` (`0`), `MSICONDITION_NONE` (`2`), or `MSICONDITION_ERROR` (`-1`).
#[cfg_attr(not(test), no_mangle)]
#[inline(never)]
#[allow(non_snake_case)]
#[must_use]
pub extern "system" fn MsiEvaluateConditionW(hInstall: MsiHandle, szCondition: Lpcwstr) -> i32 {
    let result = panic::catch_unwind(|| {
        if szCondition.is_null() {
            return 2; // MSICONDITION_NONE
        }

        let Some(condition) = lpcwstr_to_string(szCondition) else {
            return -1; // MSICONDITION_ERROR
        };

        evaluate_condition_on_handle(hInstall, &condition)
    });

    result.unwrap_or(-1)
}

/// Evaluates a conditional expression using session properties (ANSI).
///
/// # Arguments
///
/// * `hInstall` - Handle to the installation session.
/// * `szCondition` - Null-terminated ANSI conditional expression to evaluate.
///
/// # Returns
///
/// `MSICONDITION_TRUE` (`1`), `MSICONDITION_FALSE` (`0`), `MSICONDITION_NONE` (`2`), or `MSICONDITION_ERROR` (`-1`).
#[cfg_attr(not(test), no_mangle)]
#[inline(never)]
#[allow(non_snake_case)]
#[must_use]
pub extern "system" fn MsiEvaluateConditionA(hInstall: MsiHandle, szCondition: Lpcstr) -> i32 {
    let result = panic::catch_unwind(|| {
        if szCondition.is_null() {
            return 2; // MSICONDITION_NONE
        }

        let Some(condition) = lpcstr_to_string(szCondition) else {
            return -1; // MSICONDITION_ERROR
        };

        evaluate_condition_on_handle(hInstall, &condition)
    });

    result.unwrap_or(-1)
}

/// Shared helper resolving the product code of a registered component from the registry.
fn resolve_product_code_for_component(component_code: &str) -> Option<String> {
    let comp_id = msi::platform::squid::encode_squid(component_code)
        .unwrap_or_else(|_| component_code.to_string());
    let key = format!(
        r"Software\Microsoft\Windows\CurrentVersion\Installer\UserData\S-1-5-18\Components\{comp_id}"
    );
    crate::win32::registry_backend::with_registry(|reg| {
        let vals = reg.list_values(
            msi::platform::registry_store::RegistryRoot::LocalMachine,
            &key,
        );
        vals.into_iter().find_map(|(name, _)| {
            let client_id = name?;
            Some(msi::platform::squid::decode_squid(&client_id).unwrap_or(client_id))
        })
    })
}

/// Returns the product code of a registered component (Unicode).
#[cfg_attr(not(test), no_mangle)]
#[inline(never)]
#[allow(non_snake_case)]
#[allow(unused_variables)]
pub extern "system" fn MsiGetProductCodeW(szComponent: Lpcwstr, lpBuf39: Lpwstr) -> Uint {
    let result = panic::catch_unwind(|| {
        if szComponent.is_null() {
            return ERROR_INVALID_PARAMETER;
        }
        let Some(comp) = lpcwstr_to_string(szComponent) else {
            return ERROR_INVALID_PARAMETER;
        };
        if let Some(prod_code) = resolve_product_code_for_component(&comp) {
            if !lpBuf39.is_null() {
                let mut cch: Dword = 39;
                let _ = string_to_lpwstr(&prod_code, lpBuf39, &raw mut cch);
            }
        }
        ERROR_SUCCESS
    });

    result.unwrap_or(crate::win32::ERROR_INSTALL_FAILURE)
}

/// Returns the product code of a registered component (ANSI).
#[cfg_attr(not(test), no_mangle)]
#[inline(never)]
#[allow(non_snake_case)]
#[allow(unused_variables)]
pub extern "system" fn MsiGetProductCodeA(szComponent: Lpcstr, lpBuf39: Lpstr) -> Uint {
    let result = panic::catch_unwind(|| {
        if szComponent.is_null() {
            return ERROR_INVALID_PARAMETER;
        }
        let Some(comp) = lpcstr_to_string(szComponent) else {
            return ERROR_INVALID_PARAMETER;
        };
        if let Some(prod_code) = resolve_product_code_for_component(&comp) {
            if !lpBuf39.is_null() {
                let mut cch: Dword = 39;
                let _ = string_to_lpstr(&prod_code, lpBuf39, &raw mut cch);
            }
        }
        ERROR_SUCCESS
    });

    result.unwrap_or(crate::win32::ERROR_INSTALL_FAILURE)
}

/// Shared helper resolving product info from `UserData\<SID>\Products\<SQUID>\InstallProperties` or `HKCR\Installer\Products\<SQUID>`.
fn lookup_product_info_ex(
    product_code: &str,
    user_sid: Option<&str>,
    property: &str,
) -> Option<String> {
    let sid = match user_sid {
        Some(s) if !s.is_empty() => s,
        _ => "S-1-5-18",
    };
    let prod_id = msi::platform::squid::encode_squid(product_code)
        .ok()
        .unwrap_or_default();
    let prod_id = if prod_id.is_empty() {
        product_code.to_string()
    } else {
        prod_id
    };
    let prop_key = format!(
        r"Software\Microsoft\Windows\CurrentVersion\Installer\UserData\{sid}\Products\{prod_id}\InstallProperties"
    );
    let hkcr_key = format!(r"Installer\Products\{prod_id}");
    crate::win32::registry_backend::with_registry(|reg| {
        if let Some(msi::platform::registry_store::RegistryValue::Sz(s)) = reg.get_value(
            msi::platform::registry_store::RegistryRoot::LocalMachine,
            &prop_key,
            Some(property),
        ) {
            return Some(s);
        }
        if let Some(msi::platform::registry_store::RegistryValue::Sz(s)) = reg.get_value(
            msi::platform::registry_store::RegistryRoot::ClassesRoot,
            &hkcr_key,
            Some(property),
        ) {
            return Some(s);
        }
        None
    })
}

/// Returns product information for published and installed products (Unicode).
#[cfg_attr(not(test), no_mangle)]
#[inline(never)]
#[allow(non_snake_case)]
#[allow(unused_variables)]
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
        let Some(product_code) = lpcwstr_to_string(szProductCode) else {
            return ERROR_INVALID_PARAMETER;
        };
        let Some(property_name) = lpcwstr_to_string(szProperty) else {
            return ERROR_INVALID_PARAMETER;
        };
        let sid = lpcwstr_to_string(szUserSid);
        if let Some(val) = lookup_product_info_ex(&product_code, sid.as_deref(), &property_name) {
            if !szValue.is_null() || !pcchValue.is_null() {
                return string_to_lpwstr(&val, szValue, pcchValue);
            }
        }
        ERROR_SUCCESS
    });

    result.unwrap_or(crate::win32::ERROR_INSTALL_FAILURE)
}

/// Returns product information for published and installed products (ANSI).
#[cfg_attr(not(test), no_mangle)]
#[inline(never)]
#[allow(non_snake_case)]
#[allow(unused_variables)]
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
        let Some(product_code) = lpcstr_to_string(szProductCode) else {
            return ERROR_INVALID_PARAMETER;
        };
        let Some(property_name) = lpcstr_to_string(szProperty) else {
            return ERROR_INVALID_PARAMETER;
        };
        let sid = lpcstr_to_string(szUserSid);
        if let Some(val) = lookup_product_info_ex(&product_code, sid.as_deref(), &property_name) {
            if !szValue.is_null() || !pcchValue.is_null() {
                return string_to_lpstr(&val, szValue, pcchValue);
            }
        }
        ERROR_SUCCESS
    });

    result.unwrap_or(crate::win32::ERROR_INSTALL_FAILURE)
}

/// Retrieves the value of a property from a product database (Unicode).
#[cfg_attr(not(test), no_mangle)]
#[inline(never)]
#[allow(non_snake_case)]
#[allow(unused_variables)]
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
        let Some(prop) = lpcwstr_to_string(szProperty) else {
            return ERROR_INVALID_PARAMETER;
        };
        if hProduct != 0 && (!szValueBuf.is_null() || !pcchValueBuf.is_null()) {
            let val = with_handle(hProduct, |obj| match obj {
                crate::handles::MsiObject::Database(db) => {
                    db.inner.get_property_value(&prop).unwrap_or("").to_string()
                }
                _ => String::new(),
            })
            .unwrap_or_default();
            return string_to_lpwstr(&val, szValueBuf, pcchValueBuf);
        }
        ERROR_SUCCESS
    });

    result.unwrap_or(crate::win32::ERROR_INSTALL_FAILURE)
}

/// Retrieves the value of a property from a product database (ANSI).
#[cfg_attr(not(test), no_mangle)]
#[inline(never)]
#[allow(non_snake_case)]
#[allow(unused_variables)]
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
        let Some(prop) = lpcstr_to_string(szProperty) else {
            return ERROR_INVALID_PARAMETER;
        };
        if hProduct != 0 && (!szValueBuf.is_null() || !pcchValueBuf.is_null()) {
            let val = with_handle(hProduct, |obj| match obj {
                crate::handles::MsiObject::Database(db) => {
                    db.inner.get_property_value(&prop).unwrap_or("").to_string()
                }
                _ => String::new(),
            })
            .unwrap_or_default();
            return string_to_lpstr(&val, szValueBuf, pcchValueBuf);
        }
        ERROR_SUCCESS
    });

    result.unwrap_or(crate::win32::ERROR_INSTALL_FAILURE)
}

#[cfg(test)]
#[allow(clippy::too_many_lines)]
#[allow(clippy::cognitive_complexity)]
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
        assert_eq!(MsiEvaluateConditionW(h_rec, valid_w.as_ptr()), 0);

        // Live database handle property round-trip and condition evaluation
        let db = msi::wix::linker::LinkedDatabase::default();
        let h_db = crate::handles::alloc_handle(crate::handles::MsiObject::Database(
            crate::types::MsiDatabaseHandle {
                inner: db,
                state: 0,
            },
        ));
        let empty_w = [0u16];
        let empty_a = b"\0";
        assert_eq!(
            MsiSetPropertyW(h_db, empty_w.as_ptr(), valid_w.as_ptr()),
            ERROR_INVALID_PARAMETER
        );
        assert_eq!(
            MsiSetPropertyA(
                h_db,
                empty_a.as_ptr().cast::<i8>(),
                valid_a.as_ptr().cast::<i8>()
            ),
            ERROR_INVALID_PARAMETER
        );
        assert_eq!(
            MsiSetPropertyW(h_db, valid_w.as_ptr(), valid_w.as_ptr()),
            ERROR_SUCCESS
        );
        let mut out_w = [0u16; 16];
        let mut len_w: Dword = 16;
        assert_eq!(
            MsiGetPropertyW(h_db, valid_w.as_ptr(), out_w.as_mut_ptr(), &raw mut len_w),
            ERROR_SUCCESS
        );
        assert_eq!(len_w, 4);

        assert_eq!(
            MsiSetPropertyA(
                h_db,
                valid_a.as_ptr().cast::<i8>(),
                valid_a.as_ptr().cast::<i8>()
            ),
            ERROR_SUCCESS
        );
        let mut out_a = [0i8; 16];
        let mut len_a: Dword = 16;
        assert_eq!(
            MsiGetPropertyA(
                h_db,
                valid_a.as_ptr().cast::<i8>(),
                out_a.as_mut_ptr(),
                &raw mut len_a
            ),
            ERROR_SUCCESS
        );
        assert_eq!(len_a, 4);

        // Condition evaluates to TRUE (1) when prop is set, and empty condition evaluates to NONE (2)
        assert_eq!(MsiEvaluateConditionW(h_db, valid_w.as_ptr()), 1);
        assert_eq!(
            MsiEvaluateConditionA(h_db, valid_a.as_ptr().cast::<i8>()),
            1
        );
        assert_eq!(MsiEvaluateConditionW(h_db, empty_w.as_ptr()), 2);
        let bad_cond_a = b"1 ==\0";
        assert_eq!(
            MsiEvaluateConditionA(h_db, bad_cond_a.as_ptr().cast::<i8>()),
            -1
        );

        // Clearing property with NULL
        assert_eq!(
            MsiSetPropertyW(h_db, valid_w.as_ptr(), std::ptr::null()),
            ERROR_SUCCESS
        );
        assert_eq!(MsiEvaluateConditionW(h_db, valid_w.as_ptr()), 0);
        let _ = crate::handles::close_handle(h_db);
        let _ = crate::handles::close_handle(h_rec);

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

        // Test populated registry for MsiGetProductCode and MsiGetProductInfoEx
        let comp_guid = "{11111111-1111-1111-1111-111111111111}";
        let prod_guid = "{22222222-2222-2222-2222-222222222222}";
        let comp_squid = msi::platform::squid::encode_squid(comp_guid).unwrap_or_default();
        let prod_squid = msi::platform::squid::encode_squid(prod_guid).unwrap_or_default();
        let comp_reg_key = format!(
            r"Software\Microsoft\Windows\CurrentVersion\Installer\UserData\S-1-5-18\Components\{comp_squid}"
        );
        let prop_reg_key = format!(
            r"Software\Microsoft\Windows\CurrentVersion\Installer\UserData\S-1-5-18\Products\{prod_squid}\InstallProperties"
        );
        let hkcr_prod_key = format!(r"Installer\Products\{prod_squid}");
        crate::win32::registry_backend::with_registry(|reg| {
            reg.set_value(
                msi::platform::registry_store::RegistryRoot::LocalMachine,
                &comp_reg_key,
                Some(&prod_squid),
                msi::platform::registry_store::RegistryValue::Sz("C:\\App\\file.exe".to_string()),
            );
            reg.set_value(
                msi::platform::registry_store::RegistryRoot::LocalMachine,
                &prop_reg_key,
                Some("DisplayName"),
                msi::platform::registry_store::RegistryValue::Sz("MyProduct".to_string()),
            );
            reg.set_value(
                msi::platform::registry_store::RegistryRoot::ClassesRoot,
                &hkcr_prod_key,
                Some("ProductName"),
                msi::platform::registry_store::RegistryValue::Sz("PublishedName".to_string()),
            );
        });

        let comp_w: Vec<u16> = comp_guid.encode_utf16().chain(std::iter::once(0)).collect();
        let comp_a = std::ffi::CString::new(comp_guid).unwrap_or_default();
        let mut prod_buf_w = [0u16; 39];
        let mut prod_buf_a = [0i8; 39];
        assert_eq!(
            MsiGetProductCodeW(comp_w.as_ptr(), prod_buf_w.as_mut_ptr()),
            ERROR_SUCCESS
        );
        assert_eq!(
            MsiGetProductCodeA(comp_a.as_ptr(), prod_buf_a.as_mut_ptr()),
            ERROR_SUCCESS
        );

        let prod_w: Vec<u16> = prod_guid.encode_utf16().chain(std::iter::once(0)).collect();
        let prod_a = std::ffi::CString::new(prod_guid).unwrap_or_default();
        let disp_w: Vec<u16> = "DisplayName\0".encode_utf16().collect();
        let pname_a = std::ffi::CString::new("ProductName").unwrap_or_default();
        let mut info_w = [0u16; 64];
        let mut info_cch_w: Dword = 64;
        assert_eq!(
            MsiGetProductInfoExW(
                prod_w.as_ptr(),
                std::ptr::null(),
                4,
                disp_w.as_ptr(),
                info_w.as_mut_ptr(),
                &raw mut info_cch_w
            ),
            ERROR_SUCCESS
        );
        assert_eq!(info_cch_w, 9);

        let mut info_a = [0i8; 64];
        let mut info_cch_a: Dword = 64;
        assert_eq!(
            MsiGetProductInfoExA(
                prod_a.as_ptr(),
                std::ptr::null(),
                4,
                pname_a.as_ptr(),
                info_a.as_mut_ptr(),
                &raw mut info_cch_a
            ),
            ERROR_SUCCESS
        );
        assert_eq!(info_cch_a, 13);

        crate::win32::registry_backend::with_registry(|reg| {
            reg.delete_subtree(
                msi::platform::registry_store::RegistryRoot::LocalMachine,
                &comp_reg_key,
            );
            reg.delete_subtree(
                msi::platform::registry_store::RegistryRoot::LocalMachine,
                &prop_reg_key,
            );
            reg.delete_subtree(
                msi::platform::registry_store::RegistryRoot::ClassesRoot,
                &hkcr_prod_key,
            );
        });

        // Test MsiGetProductPropertyA/W with a live database handle
        let mut prop_db = msi::wix::linker::LinkedDatabase::default();
        prop_db.set_property_value("prop", "val123");
        let h_prop_db = crate::handles::alloc_handle(crate::handles::MsiObject::Database(
            crate::types::MsiDatabaseHandle {
                inner: prop_db,
                state: 0,
            },
        ));
        let mut pval_w = [0u16; 16];
        let mut wide_len: Dword = 16;
        assert_eq!(
            MsiGetProductPropertyW(
                h_prop_db,
                valid_w.as_ptr(),
                pval_w.as_mut_ptr(),
                &raw mut wide_len
            ),
            ERROR_SUCCESS
        );
        assert_eq!(wide_len, 6);
        let mut pval_a = [0i8; 16];
        let mut ansi_len: Dword = 16;
        assert_eq!(
            MsiGetProductPropertyA(
                h_prop_db,
                valid_a.as_ptr().cast::<i8>(),
                pval_a.as_mut_ptr(),
                &raw mut ansi_len
            ),
            ERROR_SUCCESS
        );
        assert_eq!(ansi_len, 6);
        let _ = crate::handles::close_handle(h_prop_db);
    }
}

//! Feature and component state endpoints.

use crate::win32::strings::{lpcstr_to_string, lpcwstr_to_string};
use crate::win32::{
    Bool, Dword, Lpcstr, Lpcwstr, Uint, ERROR_INSTALL_FAILURE, ERROR_INVALID_PARAMETER,
    ERROR_SUCCESS, FALSE,
};
use std::panic;

/// Determines whether the product is installed with elevated privileges.
///
/// # Arguments
///
/// * `szProduct` - Product code.
/// * `pfElevated` - Pointer to a boolean that receives the result.
///
/// # Returns
///
/// `ERROR_SUCCESS` or `ERROR_INVALID_PARAMETER`.
#[no_mangle]
#[allow(non_snake_case, unused_variables)]
#[allow(clippy::not_unsafe_ptr_arg_deref)]
pub extern "system" fn MsiIsProductElevatedW(szProduct: Lpcwstr, pfElevated: *mut Bool) -> Uint {
    let result = panic::catch_unwind(|| {
        if szProduct.is_null() || pfElevated.is_null() {
            return ERROR_INVALID_PARAMETER;
        }

        let Some(_product) = lpcwstr_to_string(szProduct) else {
            return ERROR_INVALID_PARAMETER;
        };

        // TODO: Real elevation check (e.g., euid == 0 on POSIX or Token check on Windows).
        // For now, mock as false.
        unsafe {
            *pfElevated = FALSE;
        }

        ERROR_SUCCESS
    });

    result.unwrap_or(ERROR_INSTALL_FAILURE)
}

/// Determines whether the product is installed with elevated privileges (ANSI).
#[no_mangle]
#[allow(non_snake_case, unused_variables)]
#[allow(clippy::not_unsafe_ptr_arg_deref)]
pub extern "system" fn MsiIsProductElevatedA(szProduct: Lpcstr, pfElevated: *mut Bool) -> Uint {
    let result = panic::catch_unwind(|| {
        if szProduct.is_null() || pfElevated.is_null() {
            return ERROR_INVALID_PARAMETER;
        }

        let Some(_product) = lpcstr_to_string(szProduct) else {
            return ERROR_INVALID_PARAMETER;
        };

        unsafe {
            *pfElevated = FALSE;
        }

        ERROR_SUCCESS
    });

    result.unwrap_or(ERROR_INSTALL_FAILURE)
}

/// Retrieves the internal engine mode flags for an active installation session.
///
/// # Arguments
///
/// * `hInstall` - The handle to the installation session.
/// * `eRunMode` - The mode to check (e.g., `MSIRUNMODE_REBOOTATEND`).
///
/// # Returns
///
/// `TRUE` if the mode is set, `FALSE` otherwise.
#[no_mangle]
#[allow(non_snake_case, unused_variables)]
pub extern "system" fn MsiGetMode(hInstall: crate::handles::MsiHandle, eRunMode: Uint) -> Bool {
    let result = panic::catch_unwind(|| {
        // TODO: Map to actual execution engine state.
        FALSE
    });

    result.unwrap_or(FALSE)
}

/// Sets an internal engine mode flag for an active installation session.
///
/// # Arguments
///
/// * `hInstall` - The handle to the installation session.
/// * `eRunMode` - The mode to set.
/// * `fState` - The boolean state to set it to.
///
/// # Returns
///
/// `ERROR_SUCCESS` on success, or an error code.
#[no_mangle]
#[allow(non_snake_case, unused_variables)]
pub extern "system" fn MsiSetMode(
    hInstall: crate::handles::MsiHandle,
    eRunMode: Uint,
    fState: Bool,
) -> Uint {
    let result = panic::catch_unwind(|| {
        // TODO: Propagate flag into execution engine state.
        ERROR_SUCCESS
    });

    result.unwrap_or(ERROR_INSTALL_FAILURE)
}

/// Sets the installation level for a full product installation.
#[no_mangle]
#[allow(non_snake_case, unused_variables)]
pub extern "system" fn MsiSetInstallLevel(
    hInstall: crate::handles::MsiHandle,
    iInstallLevel: i32,
) -> Uint {
    let result = panic::catch_unwind(|| ERROR_SUCCESS);
    result.unwrap_or(ERROR_INSTALL_FAILURE)
}

/// Configures the installed state for a product feature (Unicode).
#[no_mangle]
#[allow(non_snake_case, unused_variables)]
pub extern "system" fn MsiConfigureFeatureW(
    szProduct: Lpcwstr,
    szFeature: Lpcwstr,
    eInstallState: i32,
) -> Uint {
    let result = panic::catch_unwind(|| {
        if szProduct.is_null() || szFeature.is_null() {
            return ERROR_INVALID_PARAMETER;
        }
        let Some(_product) = lpcwstr_to_string(szProduct) else {
            return ERROR_INVALID_PARAMETER;
        };
        let Some(_feature) = lpcwstr_to_string(szFeature) else {
            return ERROR_INVALID_PARAMETER;
        };
        ERROR_SUCCESS
    });
    result.unwrap_or(ERROR_INSTALL_FAILURE)
}

/// Configures the installed state for a product feature (ANSI).
#[no_mangle]
#[allow(non_snake_case, unused_variables)]
pub extern "system" fn MsiConfigureFeatureA(
    szProduct: Lpcstr,
    szFeature: Lpcstr,
    eInstallState: i32,
) -> Uint {
    let result = panic::catch_unwind(|| {
        if szProduct.is_null() || szFeature.is_null() {
            return ERROR_INVALID_PARAMETER;
        }
        let Some(_product) = lpcstr_to_string(szProduct) else {
            return ERROR_INVALID_PARAMETER;
        };
        let Some(_feature) = lpcstr_to_string(szFeature) else {
            return ERROR_INVALID_PARAMETER;
        };
        ERROR_SUCCESS
    });
    result.unwrap_or(ERROR_INSTALL_FAILURE)
}

/// Reinstalls a feature (Unicode).
#[no_mangle]
#[allow(non_snake_case, unused_variables)]
pub extern "system" fn MsiReinstallFeatureW(
    szProduct: Lpcwstr,
    szFeature: Lpcwstr,
    dwReinstallMode: Dword,
) -> Uint {
    let result = panic::catch_unwind(|| {
        if szProduct.is_null() || szFeature.is_null() {
            return ERROR_INVALID_PARAMETER;
        }
        let Some(_product) = lpcwstr_to_string(szProduct) else {
            return ERROR_INVALID_PARAMETER;
        };
        let Some(_feature) = lpcwstr_to_string(szFeature) else {
            return ERROR_INVALID_PARAMETER;
        };
        ERROR_SUCCESS
    });
    result.unwrap_or(ERROR_INSTALL_FAILURE)
}

/// Reinstalls a feature (ANSI).
#[no_mangle]
#[allow(non_snake_case, unused_variables)]
pub extern "system" fn MsiReinstallFeatureA(
    szProduct: Lpcstr,
    szFeature: Lpcstr,
    dwReinstallMode: Dword,
) -> Uint {
    let result = panic::catch_unwind(|| {
        if szProduct.is_null() || szFeature.is_null() {
            return ERROR_INVALID_PARAMETER;
        }
        let Some(_product) = lpcstr_to_string(szProduct) else {
            return ERROR_INVALID_PARAMETER;
        };
        let Some(_feature) = lpcstr_to_string(szFeature) else {
            return ERROR_INVALID_PARAMETER;
        };
        ERROR_SUCCESS
    });
    result.unwrap_or(ERROR_INSTALL_FAILURE)
}

/// Modifies the runtime attributes of a feature (Unicode).
#[no_mangle]
#[allow(non_snake_case, unused_variables)]
pub extern "system" fn MsiSetFeatureAttributesW(
    hInstall: crate::handles::MsiHandle,
    szFeature: Lpcwstr,
    dwAttributes: Dword,
) -> Uint {
    let result = panic::catch_unwind(|| {
        if szFeature.is_null() {
            return ERROR_INVALID_PARAMETER;
        }
        let Some(_feature) = lpcwstr_to_string(szFeature) else {
            return ERROR_INVALID_PARAMETER;
        };
        ERROR_SUCCESS
    });
    result.unwrap_or(ERROR_INSTALL_FAILURE)
}

/// Modifies the runtime attributes of a feature (ANSI).
#[no_mangle]
#[allow(non_snake_case, unused_variables)]
pub extern "system" fn MsiSetFeatureAttributesA(
    hInstall: crate::handles::MsiHandle,
    szFeature: Lpcstr,
    dwAttributes: Dword,
) -> Uint {
    let result = panic::catch_unwind(|| {
        if szFeature.is_null() {
            return ERROR_INVALID_PARAMETER;
        }
        let Some(_feature) = lpcstr_to_string(szFeature) else {
            return ERROR_INVALID_PARAMETER;
        };
        ERROR_SUCCESS
    });
    result.unwrap_or(ERROR_INSTALL_FAILURE)
}

/// Sets the installed state for a product feature (Unicode).
#[no_mangle]
#[allow(non_snake_case, unused_variables)]
pub extern "system" fn MsiSetFeatureStateW(
    hInstall: crate::handles::MsiHandle,
    szFeature: Lpcwstr,
    iState: i32,
) -> Uint {
    let result = panic::catch_unwind(|| {
        if szFeature.is_null() {
            return ERROR_INVALID_PARAMETER;
        }
        let Some(_feature) = lpcwstr_to_string(szFeature) else {
            return ERROR_INVALID_PARAMETER;
        };
        ERROR_SUCCESS
    });
    result.unwrap_or(ERROR_INSTALL_FAILURE)
}

/// Sets the installed state for a product feature (ANSI).
#[no_mangle]
#[allow(non_snake_case, unused_variables)]
pub extern "system" fn MsiSetFeatureStateA(
    hInstall: crate::handles::MsiHandle,
    szFeature: Lpcstr,
    iState: i32,
) -> Uint {
    let result = panic::catch_unwind(|| {
        if szFeature.is_null() {
            return ERROR_INVALID_PARAMETER;
        }
        let Some(_feature) = lpcstr_to_string(szFeature) else {
            return ERROR_INVALID_PARAMETER;
        };
        ERROR_SUCCESS
    });
    result.unwrap_or(ERROR_INSTALL_FAILURE)
}

/// Queries the installed state of a component.
///
/// # Arguments
///
/// * `szProductCode` - A null-terminated string that specifies the `ProductCode`.
/// * `szUserSid` - A null-terminated string that specifies the user SID.
/// * `dwContext` - A flag that specifies the context.
/// * `szComponentCode` - A null-terminated string that specifies the component code.
/// * `pdwState` - Pointer to a variable that receives the state.
///
/// # Returns
///
/// `ERROR_SUCCESS` or `ERROR_INVALID_PARAMETER`.
#[no_mangle]
#[allow(non_snake_case, unused_variables)]
#[allow(clippy::not_unsafe_ptr_arg_deref)]
pub extern "system" fn MsiQueryComponentStateW(
    szProductCode: Lpcwstr,
    szUserSid: Lpcwstr,
    dwContext: Dword,
    szComponentCode: Lpcwstr,
    pdwState: *mut i32,
) -> Uint {
    let result = panic::catch_unwind(|| {
        if szProductCode.is_null() || szComponentCode.is_null() || pdwState.is_null() {
            return ERROR_INVALID_PARAMETER;
        }

        let Some(_product_code) = lpcwstr_to_string(szProductCode) else {
            return ERROR_INVALID_PARAMETER;
        };

        let Some(_component_code) = lpcwstr_to_string(szComponentCode) else {
            return ERROR_INVALID_PARAMETER;
        };

        // TODO: Map to execution engine / registry component tracking
        unsafe {
            *pdwState = 1;
        } // INSTALLSTATE_LOCAL

        ERROR_SUCCESS
    });

    result.unwrap_or(ERROR_INSTALL_FAILURE)
}

/// Queries the installed state of a component (ANSI).
#[no_mangle]
#[allow(non_snake_case, unused_variables)]
#[allow(clippy::not_unsafe_ptr_arg_deref)]
pub extern "system" fn MsiQueryComponentStateA(
    szProductCode: Lpcstr,
    szUserSid: Lpcstr,
    dwContext: Dword,
    szComponentCode: Lpcstr,
    pdwState: *mut i32,
) -> Uint {
    let result = panic::catch_unwind(|| {
        if szProductCode.is_null() || szComponentCode.is_null() || pdwState.is_null() {
            return ERROR_INVALID_PARAMETER;
        }

        let Some(_product_code) = lpcstr_to_string(szProductCode) else {
            return ERROR_INVALID_PARAMETER;
        };

        let Some(_component_code) = lpcstr_to_string(szComponentCode) else {
            return ERROR_INVALID_PARAMETER;
        };

        unsafe {
            *pdwState = 1;
        } // INSTALLSTATE_LOCAL

        ERROR_SUCCESS
    });

    result.unwrap_or(ERROR_INSTALL_FAILURE)
}

/// Sets the installed state of a component.
#[no_mangle]
#[allow(non_snake_case, unused_variables)]
pub extern "system" fn MsiSetComponentStateW(
    hInstall: crate::handles::MsiHandle,
    szComponent: Lpcwstr,
    iState: i32,
) -> Uint {
    let result = panic::catch_unwind(|| {
        if szComponent.is_null() {
            return ERROR_INVALID_PARAMETER;
        }

        let Some(_component) = lpcwstr_to_string(szComponent) else {
            return ERROR_INVALID_PARAMETER;
        };

        ERROR_SUCCESS
    });

    result.unwrap_or(ERROR_INSTALL_FAILURE)
}

/// Sets the installed state of a component (ANSI).
#[no_mangle]
#[allow(non_snake_case, unused_variables)]
pub extern "system" fn MsiSetComponentStateA(
    hInstall: crate::handles::MsiHandle,
    szComponent: Lpcstr,
    iState: i32,
) -> Uint {
    let result = panic::catch_unwind(|| {
        if szComponent.is_null() {
            return ERROR_INVALID_PARAMETER;
        }

        let Some(_component) = lpcstr_to_string(szComponent) else {
            return ERROR_INVALID_PARAMETER;
        };

        ERROR_SUCCESS
    });

    result.unwrap_or(ERROR_INSTALL_FAILURE)
}

/// Retrieves the usage metrics for a product feature.
#[no_mangle]
#[allow(non_snake_case, unused_variables)]
#[allow(clippy::not_unsafe_ptr_arg_deref)]
pub extern "system" fn MsiGetFeatureUsageW(
    szProduct: Lpcwstr,
    szFeature: Lpcwstr,
    pdwUseCount: *mut Dword,
    pwDateUsed: *mut u16,
) -> Uint {
    let result = panic::catch_unwind(|| {
        if szProduct.is_null() || szFeature.is_null() {
            return ERROR_INVALID_PARAMETER;
        }

        let Some(_product) = lpcwstr_to_string(szProduct) else {
            return ERROR_INVALID_PARAMETER;
        };

        let Some(_feature) = lpcwstr_to_string(szFeature) else {
            return ERROR_INVALID_PARAMETER;
        };

        if !pdwUseCount.is_null() {
            unsafe {
                *pdwUseCount = 0;
            }
        }

        if !pwDateUsed.is_null() {
            unsafe {
                *pwDateUsed = 0;
            }
        }

        ERROR_SUCCESS
    });

    result.unwrap_or(ERROR_INSTALL_FAILURE)
}

/// Retrieves the usage metrics for a product feature (ANSI).
#[no_mangle]
#[allow(non_snake_case, unused_variables)]
#[allow(clippy::not_unsafe_ptr_arg_deref)]
pub extern "system" fn MsiGetFeatureUsageA(
    szProduct: Lpcstr,
    szFeature: Lpcstr,
    pdwUseCount: *mut Dword,
    pwDateUsed: *mut u16,
) -> Uint {
    let result = panic::catch_unwind(|| {
        if szProduct.is_null() || szFeature.is_null() {
            return ERROR_INVALID_PARAMETER;
        }

        let Some(_product) = lpcstr_to_string(szProduct) else {
            return ERROR_INVALID_PARAMETER;
        };

        let Some(_feature) = lpcstr_to_string(szFeature) else {
            return ERROR_INVALID_PARAMETER;
        };

        if !pdwUseCount.is_null() {
            unsafe {
                *pdwUseCount = 0;
            }
        }

        if !pwDateUsed.is_null() {
            unsafe {
                *pwDateUsed = 0;
            }
        }

        ERROR_SUCCESS
    });

    result.unwrap_or(ERROR_INSTALL_FAILURE)
}

/// Increments the usage count for a particular feature and returns the installation state.
#[no_mangle]
#[allow(non_snake_case, unused_variables)]
pub extern "system" fn MsiUseFeatureExW(
    szProduct: Lpcwstr,
    szFeature: Lpcwstr,
    dwInstallMode: Dword,
    dwReserved: Dword,
) -> i32 {
    let result = panic::catch_unwind(|| {
        if szProduct.is_null() || szFeature.is_null() {
            return -2; // INSTALLSTATE_INVALIDARG
        }

        let Some(_product) = lpcwstr_to_string(szProduct) else {
            return -2;
        };

        let Some(_feature) = lpcwstr_to_string(szFeature) else {
            return -2;
        };

        1 // INSTALLSTATE_LOCAL
    });

    result.unwrap_or(-1) // INSTALLSTATE_UNKNOWN
}

/// Increments the usage count for a particular feature and returns the installation state (ANSI).
#[no_mangle]
#[allow(non_snake_case, unused_variables)]
pub extern "system" fn MsiUseFeatureExA(
    szProduct: Lpcstr,
    szFeature: Lpcstr,
    dwInstallMode: Dword,
    dwReserved: Dword,
) -> i32 {
    let result = panic::catch_unwind(|| {
        if szProduct.is_null() || szFeature.is_null() {
            return -2; // INSTALLSTATE_INVALIDARG
        }

        let Some(_product) = lpcstr_to_string(szProduct) else {
            return -2;
        };

        let Some(_feature) = lpcstr_to_string(szFeature) else {
            return -2;
        };

        1 // INSTALLSTATE_LOCAL
    });

    result.unwrap_or(-1) // INSTALLSTATE_UNKNOWN
}

/// Installs files that are unexpectedly missing.
#[no_mangle]
#[allow(non_snake_case, unused_variables)]
pub extern "system" fn MsiInstallMissingComponentW(
    szProduct: Lpcwstr,
    szComponent: Lpcwstr,
    eInstallState: i32,
) -> Uint {
    let result = panic::catch_unwind(|| {
        if szProduct.is_null() || szComponent.is_null() {
            return ERROR_INVALID_PARAMETER;
        }

        let Some(_product) = lpcwstr_to_string(szProduct) else {
            return ERROR_INVALID_PARAMETER;
        };

        let Some(_component) = lpcwstr_to_string(szComponent) else {
            return ERROR_INVALID_PARAMETER;
        };

        ERROR_SUCCESS
    });

    result.unwrap_or(ERROR_INSTALL_FAILURE)
}

/// Installs files that are unexpectedly missing (ANSI).
#[no_mangle]
#[allow(non_snake_case, unused_variables)]
pub extern "system" fn MsiInstallMissingComponentA(
    szProduct: Lpcstr,
    szComponent: Lpcstr,
    eInstallState: i32,
) -> Uint {
    let result = panic::catch_unwind(|| {
        if szProduct.is_null() || szComponent.is_null() {
            return ERROR_INVALID_PARAMETER;
        }

        let Some(_product) = lpcstr_to_string(szProduct) else {
            return ERROR_INVALID_PARAMETER;
        };

        let Some(_component) = lpcstr_to_string(szComponent) else {
            return ERROR_INVALID_PARAMETER;
        };

        ERROR_SUCCESS
    });

    result.unwrap_or(ERROR_INSTALL_FAILURE)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::win32::TRUE;

    #[test]
    fn test_component_state() {
        let mut state = 0;

        assert_eq!(
            MsiQueryComponentStateW(
                std::ptr::null(),
                std::ptr::null(),
                0,
                std::ptr::null(),
                &raw mut state
            ),
            ERROR_INVALID_PARAMETER
        );
        assert_eq!(
            MsiQueryComponentStateA(
                std::ptr::null(),
                std::ptr::null(),
                0,
                std::ptr::null(),
                &raw mut state
            ),
            ERROR_INVALID_PARAMETER
        );

        let valid_w: Vec<u16> = "valid".encode_utf16().chain(std::iter::once(0)).collect();
        let valid_a = b"valid\0";
        let _ = valid_a;

        assert_eq!(
            MsiQueryComponentStateW(
                valid_w.as_ptr(),
                std::ptr::null(),
                0,
                valid_w.as_ptr(),
                &raw mut state
            ),
            ERROR_SUCCESS
        );
        assert_eq!(state, 1);

        assert_eq!(
            MsiSetComponentStateW(0, std::ptr::null(), 0),
            ERROR_INVALID_PARAMETER
        );
        assert_eq!(
            MsiSetComponentStateA(0, std::ptr::null(), 0),
            ERROR_INVALID_PARAMETER
        );

        assert_eq!(MsiSetComponentStateW(0, valid_w.as_ptr(), 1), ERROR_SUCCESS);
        assert_eq!(
            MsiSetComponentStateA(0, valid_a.as_ptr().cast::<i8>(), 1),
            ERROR_SUCCESS
        );
    }

    #[test]
    fn test_feature_usage() {
        let mut count = 0;
        let mut date = 0;

        assert_eq!(
            MsiGetFeatureUsageW(
                std::ptr::null(),
                std::ptr::null(),
                &raw mut count,
                &raw mut date
            ),
            ERROR_INVALID_PARAMETER
        );
        assert_eq!(
            MsiGetFeatureUsageA(
                std::ptr::null(),
                std::ptr::null(),
                &raw mut count,
                &raw mut date
            ),
            ERROR_INVALID_PARAMETER
        );

        let valid_w: Vec<u16> = "valid".encode_utf16().chain(std::iter::once(0)).collect();
        let valid_a = b"valid\0";
        let _ = valid_a;

        assert_eq!(
            MsiGetFeatureUsageW(
                valid_w.as_ptr(),
                valid_w.as_ptr(),
                &raw mut count,
                &raw mut date
            ),
            ERROR_SUCCESS
        );
        assert_eq!(count, 0);

        assert_eq!(
            MsiUseFeatureExW(std::ptr::null(), std::ptr::null(), 0, 0),
            -2
        );
        assert_eq!(
            MsiUseFeatureExA(std::ptr::null(), std::ptr::null(), 0, 0),
            -2
        );

        let invalid_utf16 = [0xD800_u16, 0x0000];
        let invalid_utf8 = [0xFF_u8, 0x00];

        assert_eq!(
            MsiUseFeatureExW(valid_w.as_ptr(), valid_w.as_ptr(), 0, 0),
            1
        );
        assert_eq!(
            MsiUseFeatureExA(valid_a.as_ptr().cast(), valid_a.as_ptr().cast(), 0, 0),
            1
        );

        assert_eq!(
            MsiUseFeatureExW(invalid_utf16.as_ptr(), std::ptr::null(), 0, 0),
            -2
        );
        assert_eq!(
            MsiUseFeatureExA(invalid_utf8.as_ptr().cast(), std::ptr::null(), 0, 0),
            -2
        );
        assert_eq!(
            MsiUseFeatureExW(valid_w.as_ptr(), invalid_utf16.as_ptr(), 0, 0),
            -2
        );
        assert_eq!(
            MsiUseFeatureExA(valid_a.as_ptr().cast(), invalid_utf8.as_ptr().cast(), 0, 0),
            -2
        );
        assert_eq!(
            MsiUseFeatureExW(invalid_utf16.as_ptr(), valid_w.as_ptr(), 0, 0),
            -2
        );
        assert_eq!(
            MsiUseFeatureExA(invalid_utf8.as_ptr().cast(), valid_a.as_ptr().cast(), 0, 0),
            -2
        );

        assert_eq!(
            MsiInstallMissingComponentW(std::ptr::null(), std::ptr::null(), 0),
            ERROR_INVALID_PARAMETER
        );
        assert_eq!(
            MsiInstallMissingComponentA(std::ptr::null(), std::ptr::null(), 0),
            ERROR_INVALID_PARAMETER
        );

        assert_eq!(
            MsiInstallMissingComponentW(valid_w.as_ptr(), valid_w.as_ptr(), 0),
            ERROR_SUCCESS
        );
        assert_eq!(
            MsiInstallMissingComponentA(valid_a.as_ptr().cast(), valid_a.as_ptr().cast(), 0),
            ERROR_SUCCESS
        );
    }

    #[test]
    fn test_mode_and_elevation() {
        let mut elevated = TRUE;

        assert_eq!(
            MsiIsProductElevatedW(std::ptr::null(), std::ptr::null_mut()),
            ERROR_INVALID_PARAMETER
        );
        assert_eq!(
            MsiIsProductElevatedA(std::ptr::null(), std::ptr::null_mut()),
            ERROR_INVALID_PARAMETER
        );

        let valid_w: Vec<u16> = "valid".encode_utf16().chain(std::iter::once(0)).collect();
        let valid_a = b"valid\0";

        assert_eq!(
            MsiIsProductElevatedW(valid_w.as_ptr(), &mut elevated),
            ERROR_SUCCESS
        );
        assert_eq!(elevated, FALSE);
        assert_eq!(
            MsiIsProductElevatedA(valid_a.as_ptr().cast(), &mut elevated),
            ERROR_SUCCESS
        );
        assert_eq!(elevated, FALSE);

        let invalid_utf16 = [0xD800_u16, 0x0000];
        let invalid_utf8 = [0xFF_u8, 0x00];

        assert_eq!(
            MsiIsProductElevatedW(invalid_utf16.as_ptr(), &mut elevated),
            ERROR_INVALID_PARAMETER
        );
        assert_eq!(
            MsiIsProductElevatedA(invalid_utf8.as_ptr().cast(), &mut elevated),
            ERROR_INVALID_PARAMETER
        );

        let mut st = 0;
        assert_eq!(
            MsiQueryComponentStateW(
                valid_w.as_ptr(),
                valid_w.as_ptr(),
                0,
                valid_w.as_ptr(),
                &mut st
            ),
            ERROR_SUCCESS
        );
        assert_eq!(
            MsiQueryComponentStateA(
                valid_a.as_ptr().cast(),
                valid_a.as_ptr().cast(),
                0,
                valid_a.as_ptr().cast(),
                &mut st
            ),
            ERROR_SUCCESS
        );
        assert_eq!(
            MsiQueryComponentStateW(
                invalid_utf16.as_ptr(),
                valid_w.as_ptr(),
                0,
                valid_w.as_ptr(),
                &mut st
            ),
            ERROR_INVALID_PARAMETER
        );
        assert_eq!(
            MsiQueryComponentStateA(
                invalid_utf8.as_ptr().cast(),
                valid_a.as_ptr().cast(),
                0,
                valid_a.as_ptr().cast(),
                &mut st
            ),
            ERROR_INVALID_PARAMETER
        );
        assert_eq!(
            MsiQueryComponentStateW(
                valid_w.as_ptr(),
                valid_w.as_ptr(),
                0,
                invalid_utf16.as_ptr(),
                &mut st
            ),
            ERROR_INVALID_PARAMETER
        );
        assert_eq!(
            MsiQueryComponentStateA(
                valid_a.as_ptr().cast(),
                valid_a.as_ptr().cast(),
                0,
                invalid_utf8.as_ptr().cast(),
                &mut st
            ),
            ERROR_INVALID_PARAMETER
        );
        assert_eq!(
            MsiQueryComponentStateW(
                invalid_utf16.as_ptr(),
                valid_w.as_ptr(),
                0,
                valid_w.as_ptr(),
                &mut st
            ),
            ERROR_INVALID_PARAMETER
        );
        assert_eq!(
            MsiQueryComponentStateA(
                invalid_utf8.as_ptr().cast(),
                valid_a.as_ptr().cast(),
                0,
                valid_a.as_ptr().cast(),
                &mut st
            ),
            ERROR_INVALID_PARAMETER
        );

        assert_eq!(
            MsiSetComponentStateW(0, invalid_utf16.as_ptr(), 0),
            ERROR_INVALID_PARAMETER
        );
        assert_eq!(
            MsiSetComponentStateA(0, invalid_utf8.as_ptr().cast(), 0),
            ERROR_INVALID_PARAMETER
        );

        let mut use_count = 0;
        let mut date_used = 0;
        assert_eq!(
            MsiGetFeatureUsageW(
                valid_w.as_ptr(),
                valid_w.as_ptr(),
                &mut use_count,
                &mut date_used
            ),
            ERROR_SUCCESS
        );
        assert_eq!(
            MsiGetFeatureUsageA(
                valid_a.as_ptr().cast(),
                valid_a.as_ptr().cast(),
                &mut use_count,
                &mut date_used
            ),
            ERROR_SUCCESS
        );
        assert_eq!(
            MsiGetFeatureUsageW(
                invalid_utf16.as_ptr(),
                std::ptr::null(),
                std::ptr::null_mut(),
                std::ptr::null_mut()
            ),
            ERROR_INVALID_PARAMETER
        );
        assert_eq!(
            MsiGetFeatureUsageA(
                invalid_utf8.as_ptr().cast(),
                std::ptr::null(),
                std::ptr::null_mut(),
                std::ptr::null_mut()
            ),
            ERROR_INVALID_PARAMETER
        );
        assert_eq!(
            MsiGetFeatureUsageW(
                valid_w.as_ptr(),
                invalid_utf16.as_ptr(),
                std::ptr::null_mut(),
                std::ptr::null_mut()
            ),
            ERROR_INVALID_PARAMETER
        );
        assert_eq!(
            MsiGetFeatureUsageA(
                valid_a.as_ptr().cast(),
                invalid_utf8.as_ptr().cast(),
                std::ptr::null_mut(),
                std::ptr::null_mut()
            ),
            ERROR_INVALID_PARAMETER
        );
        assert_eq!(
            MsiGetFeatureUsageW(
                invalid_utf16.as_ptr(),
                valid_w.as_ptr(),
                std::ptr::null_mut(),
                std::ptr::null_mut()
            ),
            ERROR_INVALID_PARAMETER
        );
        assert_eq!(
            MsiGetFeatureUsageA(
                invalid_utf8.as_ptr().cast(),
                valid_a.as_ptr().cast(),
                std::ptr::null_mut(),
                std::ptr::null_mut()
            ),
            ERROR_INVALID_PARAMETER
        );

        assert_eq!(
            MsiConfigureFeatureW(invalid_utf16.as_ptr(), std::ptr::null(), 0),
            ERROR_INVALID_PARAMETER
        );
        assert_eq!(
            MsiConfigureFeatureA(invalid_utf8.as_ptr().cast(), std::ptr::null(), 0),
            ERROR_INVALID_PARAMETER
        );
        assert_eq!(
            MsiConfigureFeatureW(valid_w.as_ptr(), invalid_utf16.as_ptr(), 0),
            ERROR_INVALID_PARAMETER
        );
        assert_eq!(
            MsiConfigureFeatureA(valid_a.as_ptr().cast(), invalid_utf8.as_ptr().cast(), 0),
            ERROR_INVALID_PARAMETER
        );
        assert_eq!(
            MsiConfigureFeatureW(invalid_utf16.as_ptr(), valid_w.as_ptr(), 0),
            ERROR_INVALID_PARAMETER
        );
        assert_eq!(
            MsiConfigureFeatureA(invalid_utf8.as_ptr().cast(), valid_a.as_ptr().cast(), 0),
            ERROR_INVALID_PARAMETER
        );

        assert_eq!(
            MsiReinstallFeatureW(invalid_utf16.as_ptr(), std::ptr::null(), 0),
            ERROR_INVALID_PARAMETER
        );
        assert_eq!(
            MsiReinstallFeatureA(invalid_utf8.as_ptr().cast(), std::ptr::null(), 0),
            ERROR_INVALID_PARAMETER
        );
        assert_eq!(
            MsiReinstallFeatureW(valid_w.as_ptr(), invalid_utf16.as_ptr(), 0),
            ERROR_INVALID_PARAMETER
        );
        assert_eq!(
            MsiReinstallFeatureA(valid_a.as_ptr().cast(), invalid_utf8.as_ptr().cast(), 0),
            ERROR_INVALID_PARAMETER
        );
        assert_eq!(
            MsiReinstallFeatureW(invalid_utf16.as_ptr(), valid_w.as_ptr(), 0),
            ERROR_INVALID_PARAMETER
        );
        assert_eq!(
            MsiReinstallFeatureA(invalid_utf8.as_ptr().cast(), valid_a.as_ptr().cast(), 0),
            ERROR_INVALID_PARAMETER
        );

        assert_eq!(
            MsiSetFeatureAttributesW(0, invalid_utf16.as_ptr(), 0),
            ERROR_INVALID_PARAMETER
        );
        assert_eq!(
            MsiSetFeatureAttributesA(0, invalid_utf8.as_ptr().cast(), 0),
            ERROR_INVALID_PARAMETER
        );

        assert_eq!(
            MsiSetFeatureStateW(0, invalid_utf16.as_ptr(), 0),
            ERROR_INVALID_PARAMETER
        );
        assert_eq!(
            MsiSetFeatureStateA(0, invalid_utf8.as_ptr().cast(), 0),
            ERROR_INVALID_PARAMETER
        );

        assert_eq!(
            MsiInstallMissingComponentW(invalid_utf16.as_ptr(), std::ptr::null(), 0),
            ERROR_INVALID_PARAMETER
        );
        assert_eq!(
            MsiInstallMissingComponentA(invalid_utf8.as_ptr().cast(), std::ptr::null(), 0),
            ERROR_INVALID_PARAMETER
        );
        assert_eq!(
            MsiInstallMissingComponentW(valid_w.as_ptr(), invalid_utf16.as_ptr(), 0),
            ERROR_INVALID_PARAMETER
        );
        assert_eq!(
            MsiInstallMissingComponentA(valid_a.as_ptr().cast(), invalid_utf8.as_ptr().cast(), 0),
            ERROR_INVALID_PARAMETER
        );
        assert_eq!(
            MsiInstallMissingComponentW(invalid_utf16.as_ptr(), valid_w.as_ptr(), 0),
            ERROR_INVALID_PARAMETER
        );
        assert_eq!(
            MsiInstallMissingComponentA(invalid_utf8.as_ptr().cast(), valid_a.as_ptr().cast(), 0),
            ERROR_INVALID_PARAMETER
        );

        assert_eq!(MsiGetMode(0, 0), FALSE);
        assert_eq!(MsiSetMode(0, 0, TRUE), ERROR_SUCCESS);
        assert_eq!(MsiSetInstallLevel(0, 1), ERROR_SUCCESS);

        assert_eq!(
            MsiConfigureFeatureW(std::ptr::null(), std::ptr::null(), 0),
            ERROR_INVALID_PARAMETER
        );
        assert_eq!(
            MsiConfigureFeatureA(std::ptr::null(), std::ptr::null(), 0),
            ERROR_INVALID_PARAMETER
        );
        assert_eq!(
            MsiConfigureFeatureW(valid_w.as_ptr(), valid_w.as_ptr(), 0),
            ERROR_SUCCESS
        );
        assert_eq!(
            MsiConfigureFeatureA(
                valid_a.as_ptr().cast::<i8>(),
                valid_a.as_ptr().cast::<i8>(),
                0
            ),
            ERROR_SUCCESS
        );

        assert_eq!(
            MsiReinstallFeatureW(std::ptr::null(), std::ptr::null(), 0),
            ERROR_INVALID_PARAMETER
        );
        assert_eq!(
            MsiReinstallFeatureA(std::ptr::null(), std::ptr::null(), 0),
            ERROR_INVALID_PARAMETER
        );
        assert_eq!(
            MsiReinstallFeatureW(valid_w.as_ptr(), valid_w.as_ptr(), 0),
            ERROR_SUCCESS
        );
        assert_eq!(
            MsiReinstallFeatureA(
                valid_a.as_ptr().cast::<i8>(),
                valid_a.as_ptr().cast::<i8>(),
                0
            ),
            ERROR_SUCCESS
        );

        assert_eq!(
            MsiSetFeatureAttributesW(0, std::ptr::null(), 0),
            ERROR_INVALID_PARAMETER
        );
        assert_eq!(
            MsiSetFeatureAttributesA(0, std::ptr::null(), 0),
            ERROR_INVALID_PARAMETER
        );
        assert_eq!(
            MsiSetFeatureAttributesW(0, valid_w.as_ptr(), 0),
            ERROR_SUCCESS
        );
        assert_eq!(
            MsiSetFeatureAttributesA(0, valid_a.as_ptr().cast::<i8>(), 0),
            ERROR_SUCCESS
        );

        assert_eq!(
            MsiSetFeatureStateW(0, std::ptr::null(), 0),
            ERROR_INVALID_PARAMETER
        );
        assert_eq!(
            MsiSetFeatureStateA(0, std::ptr::null(), 0),
            ERROR_INVALID_PARAMETER
        );
        assert_eq!(MsiSetFeatureStateW(0, valid_w.as_ptr(), 0), ERROR_SUCCESS);
        assert_eq!(
            MsiSetFeatureStateA(0, valid_a.as_ptr().cast::<i8>(), 0),
            ERROR_SUCCESS
        );
    }
}

//! Product and Registry Enumeration Win32 `stdcall` API endpoints.
//!
//! Provides `MsiEnumProducts`, `MsiEnumFeatures`, `MsiEnumComponents`,
//! `MsiQueryProductState`, `MsiQueryFeatureState`, and `MsiGetProductInfo`.

use std::panic;

use crate::handles::MsiHandle;
use crate::win32::{
    Dword, Lpcstr, Lpcwstr, Lpstr, Lpwstr, Uint, ERROR_INSTALL_FAILURE, ERROR_INVALID_PARAMETER,
    ERROR_SUCCESS,
};

// The Windows Installer uses standard return codes for enumeration
/// No more items available.
const ERROR_NO_MORE_ITEMS: Uint = 259;

/// INSTALLSTATE enum
const INSTALLSTATE_UNKNOWN: i32 = -1;

/// Enumerates through all the products currently advertised or installed (Unicode).
#[cfg_attr(not(coverage_nightly), no_mangle)]
#[inline(never)]
#[allow(non_snake_case, unused_variables)]
#[allow(clippy::missing_const_for_fn)]
pub extern "system" fn MsiEnumProductsW(
    iProductIndex: Dword,
    lpProductBuf: Lpwstr, // 39 characters for GUID
) -> Uint {
    let result = panic::catch_unwind(|| {
        if lpProductBuf.is_null() {
            return ERROR_INVALID_PARAMETER;
        }

        // We do not have a real registry backend hooked up yet.
        ERROR_NO_MORE_ITEMS
    });

    result.unwrap_or(ERROR_INSTALL_FAILURE)
}

/// Enumerates through all the products currently advertised or installed (ANSI).
#[cfg_attr(not(coverage_nightly), no_mangle)]
#[inline(never)]
#[allow(non_snake_case, unused_variables)]
#[allow(clippy::missing_const_for_fn)]
pub extern "system" fn MsiEnumProductsA(
    iProductIndex: Dword,
    lpProductBuf: Lpstr, // 39 characters for GUID
) -> Uint {
    let result = panic::catch_unwind(|| {
        if lpProductBuf.is_null() {
            return ERROR_INVALID_PARAMETER;
        }

        // We do not have a real registry backend hooked up yet.
        ERROR_NO_MORE_ITEMS
    });

    result.unwrap_or(ERROR_INSTALL_FAILURE)
}

/// Enumerates the published features for a given product (Unicode).
#[cfg_attr(not(coverage_nightly), no_mangle)]
#[inline(never)]
#[allow(non_snake_case, unused_variables)]
#[allow(clippy::missing_const_for_fn)]
pub extern "system" fn MsiEnumFeaturesW(
    szProduct: Lpcwstr,
    iFeatureIndex: Dword,
    lpFeatureBuf: Lpwstr,
    lpParentBuf: Lpwstr,
) -> Uint {
    let result = panic::catch_unwind(|| {
        if szProduct.is_null() || lpFeatureBuf.is_null() {
            return ERROR_INVALID_PARAMETER;
        }
        ERROR_NO_MORE_ITEMS
    });

    result.unwrap_or(ERROR_INSTALL_FAILURE)
}

/// Enumerates the published features for a given product (ANSI).
#[cfg_attr(not(coverage_nightly), no_mangle)]
#[inline(never)]
#[allow(non_snake_case, unused_variables)]
#[allow(clippy::missing_const_for_fn)]
pub extern "system" fn MsiEnumFeaturesA(
    szProduct: Lpcstr,
    iFeatureIndex: Dword,
    lpFeatureBuf: Lpstr,
    lpParentBuf: Lpstr,
) -> Uint {
    let result = panic::catch_unwind(|| {
        if szProduct.is_null() || lpFeatureBuf.is_null() {
            return ERROR_INVALID_PARAMETER;
        }
        ERROR_NO_MORE_ITEMS
    });

    result.unwrap_or(ERROR_INSTALL_FAILURE)
}

/// Enumerates the installed components (Unicode).
#[cfg_attr(not(coverage_nightly), no_mangle)]
#[inline(never)]
#[allow(non_snake_case, unused_variables)]
#[allow(clippy::missing_const_for_fn)]
pub extern "system" fn MsiEnumComponentsW(
    iComponentIndex: Dword,
    lpComponentBuf: Lpwstr, // 39 chars GUID
) -> Uint {
    let result = panic::catch_unwind(|| {
        if lpComponentBuf.is_null() {
            return ERROR_INVALID_PARAMETER;
        }
        ERROR_NO_MORE_ITEMS
    });

    result.unwrap_or(ERROR_INSTALL_FAILURE)
}

/// Enumerates the installed components (ANSI).
#[cfg_attr(not(coverage_nightly), no_mangle)]
#[inline(never)]
#[allow(non_snake_case, unused_variables)]
#[allow(clippy::missing_const_for_fn)]
pub extern "system" fn MsiEnumComponentsA(
    iComponentIndex: Dword,
    lpComponentBuf: Lpstr, // 39 chars GUID
) -> Uint {
    let result = panic::catch_unwind(|| {
        if lpComponentBuf.is_null() {
            return ERROR_INVALID_PARAMETER;
        }
        ERROR_NO_MORE_ITEMS
    });

    result.unwrap_or(ERROR_INSTALL_FAILURE)
}

/// Returns the installed state for a product (Unicode).
#[cfg_attr(not(coverage_nightly), no_mangle)]
#[inline(never)]
#[allow(non_snake_case, unused_variables)]
#[allow(clippy::missing_const_for_fn)]
pub extern "system" fn MsiQueryProductStateW(szProduct: Lpcwstr) -> i32 {
    let result = panic::catch_unwind(|| {
        if szProduct.is_null() {
            return INSTALLSTATE_UNKNOWN;
        }
        INSTALLSTATE_UNKNOWN
    });

    result.unwrap_or(INSTALLSTATE_UNKNOWN)
}

/// Returns the installed state for a product (ANSI).
#[cfg_attr(not(coverage_nightly), no_mangle)]
#[inline(never)]
#[allow(non_snake_case, unused_variables)]
#[allow(clippy::missing_const_for_fn)]
pub extern "system" fn MsiQueryProductStateA(szProduct: Lpcstr) -> i32 {
    let result = panic::catch_unwind(|| {
        if szProduct.is_null() {
            return INSTALLSTATE_UNKNOWN;
        }
        INSTALLSTATE_UNKNOWN
    });

    result.unwrap_or(INSTALLSTATE_UNKNOWN)
}

/// Returns the installed state for a product feature (Unicode).
#[cfg_attr(not(coverage_nightly), no_mangle)]
#[inline(never)]
#[allow(non_snake_case, unused_variables)]
#[allow(clippy::missing_const_for_fn)]
pub extern "system" fn MsiQueryFeatureStateW(szProduct: Lpcwstr, szFeature: Lpcwstr) -> i32 {
    let result = panic::catch_unwind(|| {
        if szProduct.is_null() || szFeature.is_null() {
            return INSTALLSTATE_UNKNOWN;
        }
        INSTALLSTATE_UNKNOWN
    });

    result.unwrap_or(INSTALLSTATE_UNKNOWN)
}

/// Returns the installed state for a product feature (ANSI).
#[cfg_attr(not(coverage_nightly), no_mangle)]
#[inline(never)]
#[allow(non_snake_case, unused_variables)]
#[allow(clippy::missing_const_for_fn)]
pub extern "system" fn MsiQueryFeatureStateA(szProduct: Lpcstr, szFeature: Lpcstr) -> i32 {
    let result = panic::catch_unwind(|| {
        if szProduct.is_null() || szFeature.is_null() {
            return INSTALLSTATE_UNKNOWN;
        }
        INSTALLSTATE_UNKNOWN
    });

    result.unwrap_or(INSTALLSTATE_UNKNOWN)
}

/// Returns product information (Unicode).
#[cfg_attr(not(coverage_nightly), no_mangle)]
#[inline(never)]
#[allow(non_snake_case, unused_variables)]
#[allow(clippy::missing_const_for_fn)]
pub extern "system" fn MsiGetProductInfoW(
    szProduct: Lpcwstr,
    szAttribute: Lpcwstr,
    lpValueBuf: Lpwstr,
    pcchValueBuf: *mut Dword,
) -> Uint {
    let result = panic::catch_unwind(|| {
        if szProduct.is_null() || szAttribute.is_null() {
            return ERROR_INVALID_PARAMETER;
        }
        // Not found, or unknown product
        crate::win32::ERROR_INVALID_DATA
    });

    result.unwrap_or(ERROR_INSTALL_FAILURE)
}

/// Returns product information (ANSI).
#[cfg_attr(not(coverage_nightly), no_mangle)]
#[inline(never)]
#[allow(non_snake_case, unused_variables)]
#[allow(clippy::missing_const_for_fn)]
pub extern "system" fn MsiGetProductInfoA(
    szProduct: Lpcstr,
    szAttribute: Lpcstr,
    lpValueBuf: Lpstr,
    pcchValueBuf: *mut Dword,
) -> Uint {
    let result = panic::catch_unwind(|| {
        if szProduct.is_null() || szAttribute.is_null() {
            return ERROR_INVALID_PARAMETER;
        }
        crate::win32::ERROR_INVALID_DATA
    });

    result.unwrap_or(ERROR_INSTALL_FAILURE)
}

/// Enumerates the clients for a given component (Unicode).
#[cfg_attr(not(coverage_nightly), no_mangle)]
#[inline(never)]
#[allow(non_snake_case, unused_variables)]
#[allow(clippy::missing_const_for_fn)]
pub extern "system" fn MsiEnumClientsW(
    szComponent: Lpcwstr,
    iProductIndex: Dword,
    lpProductBuf: Lpwstr,
) -> Uint {
    let result = panic::catch_unwind(|| {
        if szComponent.is_null() || lpProductBuf.is_null() {
            return ERROR_INVALID_PARAMETER;
        }
        ERROR_NO_MORE_ITEMS
    });
    result.unwrap_or(ERROR_INSTALL_FAILURE)
}

/// Enumerates the clients for a given component (ANSI).
#[cfg_attr(not(coverage_nightly), no_mangle)]
#[inline(never)]
#[allow(non_snake_case, unused_variables)]
#[allow(clippy::missing_const_for_fn)]
pub extern "system" fn MsiEnumClientsA(
    szComponent: Lpcstr,
    iProductIndex: Dword,
    lpProductBuf: Lpstr,
) -> Uint {
    let result = panic::catch_unwind(|| {
        if szComponent.is_null() || lpProductBuf.is_null() {
            return ERROR_INVALID_PARAMETER;
        }
        ERROR_NO_MORE_ITEMS
    });
    result.unwrap_or(ERROR_INSTALL_FAILURE)
}

/// Enumerates the clients for a given component with advanced options (Unicode).
#[cfg_attr(not(coverage_nightly), no_mangle)]
#[inline(never)]
#[allow(non_snake_case, unused_variables)]
#[allow(clippy::missing_const_for_fn)]
pub extern "system" fn MsiEnumClientsExW(
    szComponent: Lpcwstr,
    szUserSid: Lpcwstr,
    dwContext: Dword,
    dwProductIndex: Dword,
    szProductBuf: Lpwstr,
    pdwContext: *mut Dword,
    szSidBuf: Lpwstr,
    pcchSidBuf: *mut Dword,
) -> Uint {
    let result = panic::catch_unwind(|| {
        if szComponent.is_null() {
            return ERROR_INVALID_PARAMETER;
        }
        ERROR_NO_MORE_ITEMS
    });
    result.unwrap_or(ERROR_INSTALL_FAILURE)
}

/// Enumerates the clients for a given component with advanced options (ANSI).
#[cfg_attr(not(coverage_nightly), no_mangle)]
#[inline(never)]
#[allow(non_snake_case, unused_variables)]
#[allow(clippy::missing_const_for_fn)]
pub extern "system" fn MsiEnumClientsExA(
    szComponent: Lpcstr,
    szUserSid: Lpcstr,
    dwContext: Dword,
    dwProductIndex: Dword,
    szProductBuf: Lpstr,
    pdwContext: *mut Dword,
    szSidBuf: Lpstr,
    pcchSidBuf: *mut Dword,
) -> Uint {
    let result = panic::catch_unwind(|| {
        if szComponent.is_null() {
            return ERROR_INVALID_PARAMETER;
        }
        ERROR_NO_MORE_ITEMS
    });
    result.unwrap_or(ERROR_INSTALL_FAILURE)
}

/// Enumerates the qualifiers for a given component (Unicode).
#[cfg_attr(not(coverage_nightly), no_mangle)]
#[inline(never)]
#[allow(non_snake_case, unused_variables)]
#[allow(clippy::missing_const_for_fn)]
pub extern "system" fn MsiEnumComponentQualifiersW(
    szComponent: Lpcwstr,
    iIndex: Dword,
    lpQualifierBuf: Lpwstr,
    pcchQualifierBuf: *mut Dword,
    lpApplicationDataBuf: Lpwstr,
    pcchApplicationDataBuf: *mut Dword,
) -> Uint {
    let result = panic::catch_unwind(|| {
        if szComponent.is_null() {
            return ERROR_INVALID_PARAMETER;
        }
        ERROR_NO_MORE_ITEMS
    });
    result.unwrap_or(ERROR_INSTALL_FAILURE)
}

/// Enumerates the qualifiers for a given component (ANSI).
#[cfg_attr(not(coverage_nightly), no_mangle)]
#[inline(never)]
#[allow(non_snake_case, unused_variables)]
#[allow(clippy::missing_const_for_fn)]
pub extern "system" fn MsiEnumComponentQualifiersA(
    szComponent: Lpcstr,
    iIndex: Dword,
    lpQualifierBuf: Lpstr,
    pcchQualifierBuf: *mut Dword,
    lpApplicationDataBuf: Lpstr,
    pcchApplicationDataBuf: *mut Dword,
) -> Uint {
    let result = panic::catch_unwind(|| {
        if szComponent.is_null() {
            return ERROR_INVALID_PARAMETER;
        }
        ERROR_NO_MORE_ITEMS
    });
    result.unwrap_or(ERROR_INSTALL_FAILURE)
}

/// Enumerates the patches for a given product (Unicode).
#[cfg_attr(not(coverage_nightly), no_mangle)]
#[inline(never)]
#[allow(non_snake_case, unused_variables)]
#[allow(clippy::missing_const_for_fn)]
pub extern "system" fn MsiEnumPatchesW(
    szProduct: Lpcwstr,
    iPatchIndex: Dword,
    lpPatchBuf: Lpwstr,
    lpTransformsBuf: Lpwstr,
    pcchTransformsBuf: *mut Dword,
) -> Uint {
    let result = panic::catch_unwind(|| {
        if szProduct.is_null() || lpPatchBuf.is_null() {
            return ERROR_INVALID_PARAMETER;
        }
        ERROR_NO_MORE_ITEMS
    });
    result.unwrap_or(ERROR_INSTALL_FAILURE)
}

/// Enumerates the patches for a given product (ANSI).
#[cfg_attr(not(coverage_nightly), no_mangle)]
#[inline(never)]
#[allow(non_snake_case, unused_variables)]
#[allow(clippy::missing_const_for_fn)]
pub extern "system" fn MsiEnumPatchesA(
    szProduct: Lpcstr,
    iPatchIndex: Dword,
    lpPatchBuf: Lpstr,
    lpTransformsBuf: Lpstr,
    pcchTransformsBuf: *mut Dword,
) -> Uint {
    let result = panic::catch_unwind(|| {
        if szProduct.is_null() || lpPatchBuf.is_null() {
            return ERROR_INVALID_PARAMETER;
        }
        ERROR_NO_MORE_ITEMS
    });
    result.unwrap_or(ERROR_INSTALL_FAILURE)
}

/// Enumerates the patches with advanced options (Unicode).
#[cfg_attr(not(coverage_nightly), no_mangle)]
#[inline(never)]
#[allow(non_snake_case, unused_variables)]
#[allow(clippy::missing_const_for_fn)]
pub extern "system" fn MsiEnumPatchesExW(
    szProductCode: Lpcwstr,
    szUserSid: Lpcwstr,
    dwContext: Dword,
    dwFilter: Dword,
    dwIndex: Dword,
    szPatchCode: Lpwstr,
    szTargetProductCode: Lpwstr,
    pdwTargetProductContext: *mut Dword,
    szTargetUserSid: Lpwstr,
    pcchTargetUserSid: *mut Dword,
) -> Uint {
    let result = panic::catch_unwind(|| {
        if szProductCode.is_null() {
            return ERROR_INVALID_PARAMETER;
        }
        ERROR_NO_MORE_ITEMS
    });
    result.unwrap_or(ERROR_INSTALL_FAILURE)
}

/// Enumerates the patches with advanced options (ANSI).
#[cfg_attr(not(coverage_nightly), no_mangle)]
#[inline(never)]
#[allow(non_snake_case, unused_variables)]
#[allow(clippy::missing_const_for_fn)]
pub extern "system" fn MsiEnumPatchesExA(
    szProductCode: Lpcstr,
    szUserSid: Lpcstr,
    dwContext: Dword,
    dwFilter: Dword,
    dwIndex: Dword,
    szPatchCode: Lpstr,
    szTargetProductCode: Lpstr,
    pdwTargetProductContext: *mut Dword,
    szTargetUserSid: Lpstr,
    pcchTargetUserSid: *mut Dword,
) -> Uint {
    let result = panic::catch_unwind(|| {
        if szProductCode.is_null() {
            return ERROR_INVALID_PARAMETER;
        }
        ERROR_NO_MORE_ITEMS
    });
    result.unwrap_or(ERROR_INSTALL_FAILURE)
}

/// Enumerates the related products (Unicode).
#[cfg_attr(not(coverage_nightly), no_mangle)]
#[inline(never)]
#[allow(non_snake_case, unused_variables)]
#[allow(clippy::missing_const_for_fn)]
pub extern "system" fn MsiEnumRelatedProductsW(
    szUpgradeCode: Lpcwstr,
    dwReserved: Dword,
    iProductIndex: Dword,
    lpProductBuf: Lpwstr,
) -> Uint {
    let result = panic::catch_unwind(|| {
        if szUpgradeCode.is_null() || lpProductBuf.is_null() {
            return ERROR_INVALID_PARAMETER;
        }
        ERROR_NO_MORE_ITEMS
    });
    result.unwrap_or(ERROR_INSTALL_FAILURE)
}

/// Enumerates the related products (ANSI).
#[cfg_attr(not(coverage_nightly), no_mangle)]
#[inline(never)]
#[allow(non_snake_case, unused_variables)]
#[allow(clippy::missing_const_for_fn)]
pub extern "system" fn MsiEnumRelatedProductsA(
    szUpgradeCode: Lpcstr,
    dwReserved: Dword,
    iProductIndex: Dword,
    lpProductBuf: Lpstr,
) -> Uint {
    let result = panic::catch_unwind(|| {
        if szUpgradeCode.is_null() || lpProductBuf.is_null() {
            return ERROR_INVALID_PARAMETER;
        }
        ERROR_NO_MORE_ITEMS
    });
    result.unwrap_or(ERROR_INSTALL_FAILURE)
}

/// Enumerates the products with advanced options (Unicode).
#[cfg_attr(not(coverage_nightly), no_mangle)]
#[inline(never)]
#[allow(non_snake_case, unused_variables)]
#[allow(clippy::missing_const_for_fn)]
pub extern "system" fn MsiEnumProductsExW(
    szProductCode: Lpcwstr,
    szUserSid: Lpcwstr,
    dwContext: Dword,
    dwIndex: Dword,
    szInstalledProductCode: Lpwstr,
    pdwInstalledContext: *mut Dword,
    szSid: Lpwstr,
    pcchSid: *mut Dword,
) -> Uint {
    let result = panic::catch_unwind(|| ERROR_NO_MORE_ITEMS);
    result.unwrap_or(ERROR_INSTALL_FAILURE)
}

/// Enumerates the products with advanced options (ANSI).
#[cfg_attr(not(coverage_nightly), no_mangle)]
#[inline(never)]
#[allow(non_snake_case, unused_variables)]
#[allow(clippy::missing_const_for_fn)]
pub extern "system" fn MsiEnumProductsExA(
    szProductCode: Lpcstr,
    szUserSid: Lpcstr,
    dwContext: Dword,
    dwIndex: Dword,
    szInstalledProductCode: Lpstr,
    pdwInstalledContext: *mut Dword,
    szSid: Lpstr,
    pcchSid: *mut Dword,
) -> Uint {
    let result = panic::catch_unwind(|| ERROR_NO_MORE_ITEMS);
    result.unwrap_or(ERROR_INSTALL_FAILURE)
}

/// Enumerates the components with advanced options (Unicode).
#[cfg_attr(not(coverage_nightly), no_mangle)]
#[inline(never)]
#[allow(non_snake_case, unused_variables)]
#[allow(clippy::missing_const_for_fn)]
pub extern "system" fn MsiEnumComponentsExW(
    szComponentCode: Lpcwstr,
    dwContext: Dword,
    dwIndex: Dword,
    szInstalledProductCode: Lpwstr,
    pdwInstalledContext: *mut Dword,
    szSid: Lpwstr,
    pcchSid: *mut Dword,
) -> Uint {
    let result = panic::catch_unwind(|| ERROR_NO_MORE_ITEMS);
    result.unwrap_or(ERROR_INSTALL_FAILURE)
}

/// Enumerates the components with advanced options (ANSI).
#[cfg_attr(not(coverage_nightly), no_mangle)]
#[inline(never)]
#[allow(non_snake_case, unused_variables)]
#[allow(clippy::missing_const_for_fn)]
pub extern "system" fn MsiEnumComponentsExA(
    szComponentCode: Lpcstr,
    dwContext: Dword,
    dwIndex: Dword,
    szInstalledProductCode: Lpstr,
    pdwInstalledContext: *mut Dword,
    szSid: Lpstr,
    pcchSid: *mut Dword,
) -> Uint {
    let result = panic::catch_unwind(|| ERROR_NO_MORE_ITEMS);
    result.unwrap_or(ERROR_INSTALL_FAILURE)
}

/// Advanced feature state check (Unicode).
#[cfg_attr(not(coverage_nightly), no_mangle)]
#[inline(never)]
#[allow(non_snake_case, unused_variables)]
#[allow(clippy::missing_const_for_fn)]
pub extern "system" fn MsiQueryFeatureStateExW(
    szProductCode: Lpcwstr,
    szUserSid: Lpcwstr,
    dwContext: Dword,
    szFeature: Lpcwstr,
    pdwState: *mut Dword,
) -> Uint {
    let result = panic::catch_unwind(|| {
        if szProductCode.is_null() || szFeature.is_null() {
            return ERROR_INVALID_PARAMETER;
        }
        ERROR_NO_MORE_ITEMS
    });
    result.unwrap_or(ERROR_INSTALL_FAILURE)
}

/// Advanced feature state check (ANSI).
#[cfg_attr(not(coverage_nightly), no_mangle)]
#[inline(never)]
#[allow(non_snake_case, unused_variables)]
#[allow(clippy::missing_const_for_fn)]
pub extern "system" fn MsiQueryFeatureStateExA(
    szProductCode: Lpcstr,
    szUserSid: Lpcstr,
    dwContext: Dword,
    szFeature: Lpcstr,
    pdwState: *mut Dword,
) -> Uint {
    let result = panic::catch_unwind(|| {
        if szProductCode.is_null() || szFeature.is_null() {
            return ERROR_INVALID_PARAMETER;
        }
        ERROR_NO_MORE_ITEMS
    });
    result.unwrap_or(ERROR_INSTALL_FAILURE)
}

/// Get current and action state for component (Unicode).
#[cfg_attr(not(coverage_nightly), no_mangle)]
#[inline(never)]
#[allow(non_snake_case, unused_variables)]
#[allow(clippy::missing_const_for_fn)]
pub extern "system" fn MsiGetComponentStateW(
    hInstall: MsiHandle,
    szComponent: Lpcwstr,
    piInstalled: *mut i32,
    piAction: *mut i32,
) -> Uint {
    let result = panic::catch_unwind(|| {
        if szComponent.is_null() {
            return ERROR_INVALID_PARAMETER;
        }
        ERROR_NO_MORE_ITEMS
    });
    result.unwrap_or(ERROR_INSTALL_FAILURE)
}

/// Get current and action state for component (ANSI).
#[cfg_attr(not(coverage_nightly), no_mangle)]
#[inline(never)]
#[allow(non_snake_case, unused_variables)]
#[allow(clippy::missing_const_for_fn)]
pub extern "system" fn MsiGetComponentStateA(
    hInstall: MsiHandle,
    szComponent: Lpcstr,
    piInstalled: *mut i32,
    piAction: *mut i32,
) -> Uint {
    let result = panic::catch_unwind(|| {
        if szComponent.is_null() {
            return ERROR_INVALID_PARAMETER;
        }
        ERROR_NO_MORE_ITEMS
    });
    result.unwrap_or(ERROR_INSTALL_FAILURE)
}

/// Get current and action state for feature (Unicode).
#[cfg_attr(not(coverage_nightly), no_mangle)]
#[inline(never)]
#[allow(non_snake_case, unused_variables)]
#[allow(clippy::missing_const_for_fn)]
pub extern "system" fn MsiGetFeatureStateW(
    hInstall: MsiHandle,
    szFeature: Lpcwstr,
    piInstalled: *mut i32,
    piAction: *mut i32,
) -> Uint {
    let result = panic::catch_unwind(|| {
        if szFeature.is_null() {
            return ERROR_INVALID_PARAMETER;
        }
        ERROR_NO_MORE_ITEMS
    });
    result.unwrap_or(ERROR_INSTALL_FAILURE)
}

/// Get current and action state for feature (ANSI).
#[cfg_attr(not(coverage_nightly), no_mangle)]
#[inline(never)]
#[allow(non_snake_case, unused_variables)]
#[allow(clippy::missing_const_for_fn)]
pub extern "system" fn MsiGetFeatureStateA(
    hInstall: MsiHandle,
    szFeature: Lpcstr,
    piInstalled: *mut i32,
    piAction: *mut i32,
) -> Uint {
    let result = panic::catch_unwind(|| {
        if szFeature.is_null() {
            return ERROR_INVALID_PARAMETER;
        }
        ERROR_NO_MORE_ITEMS
    });
    result.unwrap_or(ERROR_INSTALL_FAILURE)
}

/// Locate component file path (Unicode).
#[cfg_attr(not(coverage_nightly), no_mangle)]
#[inline(never)]
#[allow(non_snake_case, unused_variables)]
#[allow(clippy::missing_const_for_fn)]
pub extern "system" fn MsiGetComponentPathExW(
    szProductCode: Lpcwstr,
    szComponentCode: Lpcwstr,
    szUserSid: Lpcwstr,
    dwContext: Dword,
    lpOutPathBuffer: Lpwstr,
    pcchOutPathBuffer: *mut Dword,
) -> Uint {
    let result = panic::catch_unwind(|| {
        if szProductCode.is_null() || szComponentCode.is_null() {
            return ERROR_INVALID_PARAMETER;
        }
        ERROR_NO_MORE_ITEMS
    });
    result.unwrap_or(ERROR_INSTALL_FAILURE)
}

/// Locate component file path (ANSI).
#[cfg_attr(not(coverage_nightly), no_mangle)]
#[inline(never)]
#[allow(non_snake_case, unused_variables)]
#[allow(clippy::missing_const_for_fn)]
pub extern "system" fn MsiGetComponentPathExA(
    szProductCode: Lpcstr,
    szComponentCode: Lpcstr,
    szUserSid: Lpcstr,
    dwContext: Dword,
    lpOutPathBuffer: Lpstr,
    pcchOutPathBuffer: *mut Dword,
) -> Uint {
    let result = panic::catch_unwind(|| {
        if szProductCode.is_null() || szComponentCode.is_null() {
            return ERROR_INVALID_PARAMETER;
        }
        ERROR_NO_MORE_ITEMS
    });
    result.unwrap_or(ERROR_INSTALL_FAILURE)
}

/// Locate component file path for the current user (Unicode).
#[cfg_attr(not(coverage_nightly), no_mangle)]
#[inline(never)]
#[allow(non_snake_case, unused_variables)]
#[allow(clippy::missing_const_for_fn)]
pub extern "system" fn MsiGetComponentPathW(
    szProduct: Lpcwstr,
    szComponent: Lpcwstr,
    lpPathBuf: Lpwstr,
    pcchBuf: *mut Dword,
) -> i32 {
    let result = panic::catch_unwind(|| {
        if szProduct.is_null() || szComponent.is_null() {
            return INSTALLSTATE_UNKNOWN;
        }
        1 // INSTALLSTATE_LOCAL
    });
    result.unwrap_or(INSTALLSTATE_UNKNOWN)
}

/// Locate component file path for the current user (ANSI).
#[cfg_attr(not(coverage_nightly), no_mangle)]
#[inline(never)]
#[allow(non_snake_case, unused_variables)]
#[allow(clippy::missing_const_for_fn)]
pub extern "system" fn MsiGetComponentPathA(
    szProduct: Lpcstr,
    szComponent: Lpcstr,
    lpPathBuf: Lpstr,
    pcchBuf: *mut Dword,
) -> i32 {
    let result = panic::catch_unwind(|| {
        if szProduct.is_null() || szComponent.is_null() {
            return INSTALLSTATE_UNKNOWN;
        }
        1 // INSTALLSTATE_LOCAL
    });
    result.unwrap_or(INSTALLSTATE_UNKNOWN)
}

/// Retrieve valid installation states for feature (Unicode).
#[cfg_attr(not(coverage_nightly), no_mangle)]
#[inline(never)]
#[allow(non_snake_case, unused_variables)]
#[allow(clippy::missing_const_for_fn)]
pub extern "system" fn MsiGetFeatureValidStatesW(
    hInstall: MsiHandle,
    szFeature: Lpcwstr,
    dwInstallStates: *mut Dword,
) -> Uint {
    let result = panic::catch_unwind(|| {
        if szFeature.is_null() {
            return ERROR_INVALID_PARAMETER;
        }
        ERROR_NO_MORE_ITEMS
    });
    result.unwrap_or(ERROR_INSTALL_FAILURE)
}

/// Retrieve valid installation states for feature (ANSI).
#[cfg_attr(not(coverage_nightly), no_mangle)]
#[inline(never)]
#[allow(non_snake_case, unused_variables)]
#[allow(clippy::missing_const_for_fn)]
pub extern "system" fn MsiGetFeatureValidStatesA(
    hInstall: MsiHandle,
    szFeature: Lpcstr,
    dwInstallStates: *mut Dword,
) -> Uint {
    let result = panic::catch_unwind(|| {
        if szFeature.is_null() {
            return ERROR_INVALID_PARAMETER;
        }
        ERROR_NO_MORE_ITEMS
    });
    result.unwrap_or(ERROR_INSTALL_FAILURE)
}

/// Calculate feature disk cost (Unicode).
#[cfg_attr(not(coverage_nightly), no_mangle)]
#[inline(never)]
#[allow(non_snake_case, unused_variables)]
#[allow(clippy::missing_const_for_fn)]
pub extern "system" fn MsiGetFeatureCostW(
    hInstall: MsiHandle,
    szFeature: Lpcwstr,
    iCostTree: i32,
    iState: i32,
    piCost: *mut i32,
) -> Uint {
    let result = panic::catch_unwind(|| {
        if szFeature.is_null() {
            return ERROR_INVALID_PARAMETER;
        }
        ERROR_NO_MORE_ITEMS
    });
    result.unwrap_or(ERROR_INSTALL_FAILURE)
}

/// Calculate feature disk cost (ANSI).
#[cfg_attr(not(coverage_nightly), no_mangle)]
#[inline(never)]
#[allow(non_snake_case, unused_variables)]
#[allow(clippy::missing_const_for_fn)]
pub extern "system" fn MsiGetFeatureCostA(
    hInstall: MsiHandle,
    szFeature: Lpcstr,
    iCostTree: i32,
    iState: i32,
    piCost: *mut i32,
) -> Uint {
    let result = panic::catch_unwind(|| {
        if szFeature.is_null() {
            return ERROR_INVALID_PARAMETER;
        }
        ERROR_NO_MORE_ITEMS
    });
    result.unwrap_or(ERROR_INSTALL_FAILURE)
}

/// Enumerate drive costs for component (Unicode).
#[cfg_attr(not(coverage_nightly), no_mangle)]
#[inline(never)]
#[allow(non_snake_case, unused_variables)]
#[allow(clippy::missing_const_for_fn)]
pub extern "system" fn MsiEnumComponentCostsW(
    hInstall: MsiHandle,
    szComponent: Lpcwstr,
    dwIndex: Dword,
    iState: i32,
    szDriveBuf: Lpwstr,
    pcchDriveBuf: *mut Dword,
    piCost: *mut i32,
    piTempCost: *mut i32,
) -> Uint {
    let result = panic::catch_unwind(|| {
        if szComponent.is_null() {
            return ERROR_INVALID_PARAMETER;
        }
        ERROR_NO_MORE_ITEMS
    });
    result.unwrap_or(ERROR_INSTALL_FAILURE)
}

/// Enumerate drive costs for component (ANSI).
#[cfg_attr(not(coverage_nightly), no_mangle)]
#[inline(never)]
#[allow(non_snake_case, unused_variables)]
#[allow(clippy::missing_const_for_fn)]
pub extern "system" fn MsiEnumComponentCostsA(
    hInstall: MsiHandle,
    szComponent: Lpcstr,
    dwIndex: Dword,
    iState: i32,
    szDriveBuf: Lpstr,
    pcchDriveBuf: *mut Dword,
    piCost: *mut i32,
    piTempCost: *mut i32,
) -> Uint {
    let result = panic::catch_unwind(|| {
        if szComponent.is_null() {
            return ERROR_INVALID_PARAMETER;
        }
        ERROR_NO_MORE_ITEMS
    });
    result.unwrap_or(ERROR_INSTALL_FAILURE)
}

/// Parse MSI component descriptor (Unicode).
#[cfg_attr(not(coverage_nightly), no_mangle)]
#[inline(never)]
#[allow(non_snake_case, unused_variables)]
#[allow(clippy::missing_const_for_fn)]
pub extern "system" fn MsiDecomposeDescriptorW(
    szDescriptor: Lpcwstr,
    szProductCode: Lpwstr,
    szFeatureId: Lpwstr,
    szComponentCode: Lpwstr,
    pdwArgsOffset: *mut Dword,
) -> Uint {
    let result = panic::catch_unwind(|| {
        if szDescriptor.is_null() {
            return ERROR_INVALID_PARAMETER;
        }
        ERROR_NO_MORE_ITEMS
    });
    result.unwrap_or(ERROR_INSTALL_FAILURE)
}

/// Parse MSI component descriptor (ANSI).
#[cfg_attr(not(coverage_nightly), no_mangle)]
#[inline(never)]
#[allow(non_snake_case, unused_variables)]
#[allow(clippy::missing_const_for_fn)]
pub extern "system" fn MsiDecomposeDescriptorA(
    szDescriptor: Lpcstr,
    szProductCode: Lpstr,
    szFeatureId: Lpstr,
    szComponentCode: Lpstr,
    pdwArgsOffset: *mut Dword,
) -> Uint {
    let result = panic::catch_unwind(|| {
        if szDescriptor.is_null() {
            return ERROR_INVALID_PARAMETER;
        }
        ERROR_NO_MORE_ITEMS
    });
    result.unwrap_or(ERROR_INSTALL_FAILURE)
}

/// Extract PE file version (Unicode).
#[cfg_attr(not(coverage_nightly), no_mangle)]
#[inline(never)]
#[allow(non_snake_case, unused_variables)]
#[allow(clippy::missing_const_for_fn)]
pub extern "system" fn MsiGetFileVersionW(
    szFilePath: Lpcwstr,
    szVersionPath: Lpwstr,
    pcchVersionPath: *mut Dword,
    szLangPath: Lpwstr,
    pcchLangPath: *mut Dword,
) -> Uint {
    let result = panic::catch_unwind(|| {
        if szFilePath.is_null() {
            return ERROR_INVALID_PARAMETER;
        }
        ERROR_NO_MORE_ITEMS
    });
    result.unwrap_or(ERROR_INSTALL_FAILURE)
}

/// Extract PE file version (ANSI).
#[cfg_attr(not(coverage_nightly), no_mangle)]
#[inline(never)]
#[allow(non_snake_case, unused_variables)]
#[allow(clippy::missing_const_for_fn)]
pub extern "system" fn MsiGetFileVersionA(
    szFilePath: Lpcstr,
    szVersionPath: Lpstr,
    pcchVersionPath: *mut Dword,
    szLangPath: Lpstr,
    pcchLangPath: *mut Dword,
) -> Uint {
    let result = panic::catch_unwind(|| {
        if szFilePath.is_null() {
            return ERROR_INVALID_PARAMETER;
        }
        ERROR_NO_MORE_ITEMS
    });
    result.unwrap_or(ERROR_INSTALL_FAILURE)
}

/// Obtains user information for a product (Unicode).
#[cfg_attr(not(coverage_nightly), no_mangle)]
#[inline(never)]
#[allow(non_snake_case, unused_variables)]
#[allow(clippy::missing_const_for_fn)]
pub extern "system" fn MsiCollectUserInfoW(szProduct: Lpcwstr) -> Uint {
    let result = panic::catch_unwind(|| {
        if szProduct.is_null() {
            return ERROR_INVALID_PARAMETER;
        }
        ERROR_SUCCESS
    });
    result.unwrap_or(ERROR_INSTALL_FAILURE)
}

/// Obtains user information for a product (ANSI).
#[cfg_attr(not(coverage_nightly), no_mangle)]
#[inline(never)]
#[allow(non_snake_case, unused_variables)]
#[allow(clippy::missing_const_for_fn)]
pub extern "system" fn MsiCollectUserInfoA(szProduct: Lpcstr) -> Uint {
    let result = panic::catch_unwind(|| {
        if szProduct.is_null() {
            return ERROR_INVALID_PARAMETER;
        }
        ERROR_SUCCESS
    });
    result.unwrap_or(ERROR_INSTALL_FAILURE)
}

/// Retrieves registered user information (Unicode).
#[cfg_attr(not(coverage_nightly), no_mangle)]
#[inline(never)]
#[allow(non_snake_case, unused_variables)]
#[allow(clippy::missing_const_for_fn)]
pub extern "system" fn MsiGetUserInfoW(
    szProduct: Lpcwstr,
    lpUserNameBuf: Lpwstr,
    pcchUserNameBuf: *mut Dword,
    lpOrgNameBuf: Lpwstr,
    pcchOrgNameBuf: *mut Dword,
    lpSerialBuf: Lpwstr,
    pcchSerialBuf: *mut Dword,
) -> i32 {
    let result = panic::catch_unwind(|| {
        if szProduct.is_null() {
            return INSTALLSTATE_UNKNOWN;
        }
        INSTALLSTATE_UNKNOWN
    });
    result.unwrap_or(INSTALLSTATE_UNKNOWN)
}

/// Retrieves registered user information (ANSI).
#[cfg_attr(not(coverage_nightly), no_mangle)]
#[inline(never)]
#[allow(non_snake_case, unused_variables)]
#[allow(clippy::missing_const_for_fn)]
pub extern "system" fn MsiGetUserInfoA(
    szProduct: Lpcstr,
    lpUserNameBuf: Lpstr,
    pcchUserNameBuf: *mut Dword,
    lpOrgNameBuf: Lpstr,
    pcchOrgNameBuf: *mut Dword,
    lpSerialBuf: Lpstr,
    pcchSerialBuf: *mut Dword,
) -> i32 {
    let result = panic::catch_unwind(|| {
        if szProduct.is_null() {
            return INSTALLSTATE_UNKNOWN;
        }
        INSTALLSTATE_UNKNOWN
    });
    result.unwrap_or(INSTALLSTATE_UNKNOWN)
}

/// Internal shim for migrating cached packages (Unicode).
#[cfg_attr(not(coverage_nightly), no_mangle)]
#[inline(never)]
#[allow(non_snake_case, unused_variables)]
/// Migrates cached packages from Windows Installer 1.0 format.
///
/// This is a legacy API that is no longer used by modern installers. It is safely stubbed to return `ERROR_SUCCESS`.
///
/// # Arguments
/// * `szProductCode` - Product code to migrate.
/// * `szUserSid` - User SID to migrate.
/// * `szPreflightCheck` - Preflight check string.
/// * `dwReserved` - Reserved parameter.
///
/// # Returns
/// `ERROR_SUCCESS`.
#[allow(clippy::missing_const_for_fn)]
pub extern "system" fn Migrate10CachedPackagesW(
    szProductCode: Lpcwstr,
    szUserSid: Lpcwstr,
    szPreflightCheck: Lpcwstr,
    dwReserved: Dword,
) -> Uint {
    let result = panic::catch_unwind(|| ERROR_SUCCESS); // No-op
    result.unwrap_or(ERROR_INSTALL_FAILURE)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::locator::{MsiGetShortcutTargetA, MsiGetShortcutTargetW};

    #[test]
    #[allow(clippy::too_many_lines)]
    fn test_registry_stubs() {
        let mut buf_a = [0i8; 39];
        let mut buf_w = [0u16; 39];
        assert_eq!(
            MsiEnumProductsW(0, std::ptr::null_mut()),
            ERROR_INVALID_PARAMETER
        );
        assert_eq!(
            MsiEnumProductsA(0, std::ptr::null_mut()),
            ERROR_INVALID_PARAMETER
        );

        assert_eq!(MsiEnumProductsW(0, buf_w.as_mut_ptr()), ERROR_NO_MORE_ITEMS);

        assert_eq!(
            MsiQueryProductStateW(std::ptr::null()),
            INSTALLSTATE_UNKNOWN
        );
        assert_eq!(
            MsiQueryProductStateA(std::ptr::null()),
            INSTALLSTATE_UNKNOWN
        );

        assert_eq!(
            MsiQueryFeatureStateW(std::ptr::null(), std::ptr::null()),
            INSTALLSTATE_UNKNOWN
        );

        assert_eq!(
            MsiGetProductInfoW(
                std::ptr::null(),
                std::ptr::null(),
                std::ptr::null_mut(),
                std::ptr::null_mut()
            ),
            ERROR_INVALID_PARAMETER
        );

        assert_eq!(MsiEnumProductsA(0, buf_a.as_mut_ptr()), ERROR_NO_MORE_ITEMS);

        let valid_w: Vec<u16> = "prod".encode_utf16().chain(std::iter::once(0)).collect();
        let valid_a = b"prod\0";

        assert_eq!(
            MsiEnumFeaturesW(
                valid_w.as_ptr(),
                0,
                buf_w.as_mut_ptr(),
                std::ptr::null_mut()
            ),
            ERROR_NO_MORE_ITEMS
        );
        assert_eq!(
            MsiEnumFeaturesA(
                valid_a.as_ptr().cast::<i8>(),
                0,
                buf_a.as_mut_ptr(),
                std::ptr::null_mut()
            ),
            ERROR_NO_MORE_ITEMS
        );

        assert_eq!(
            MsiEnumComponentsW(0, buf_w.as_mut_ptr()),
            ERROR_NO_MORE_ITEMS
        );
        assert_eq!(
            MsiEnumComponentsA(0, buf_a.as_mut_ptr()),
            ERROR_NO_MORE_ITEMS
        );

        assert_eq!(
            MsiQueryProductStateW(valid_w.as_ptr()),
            INSTALLSTATE_UNKNOWN
        );
        assert_eq!(
            MsiQueryProductStateA(valid_a.as_ptr().cast::<i8>()),
            INSTALLSTATE_UNKNOWN
        );

        let feat_w: Vec<u16> = "feat".encode_utf16().chain(std::iter::once(0)).collect();
        let feat_a = b"feat\0";

        assert_eq!(
            MsiQueryFeatureStateW(valid_w.as_ptr(), feat_w.as_ptr()),
            INSTALLSTATE_UNKNOWN
        );
        assert_eq!(
            MsiQueryFeatureStateA(valid_a.as_ptr().cast::<i8>(), feat_a.as_ptr().cast::<i8>()),
            INSTALLSTATE_UNKNOWN
        );

        let mut pcch = 0;
        assert_eq!(
            MsiGetProductInfoW(
                valid_w.as_ptr(),
                feat_w.as_ptr(),
                buf_w.as_mut_ptr(),
                &raw mut pcch
            ),
            crate::win32::ERROR_INVALID_DATA
        );
        assert_eq!(
            MsiGetProductInfoA(
                valid_a.as_ptr().cast::<i8>(),
                feat_a.as_ptr().cast::<i8>(),
                buf_a.as_mut_ptr(),
                &raw mut pcch
            ),
            crate::win32::ERROR_INVALID_DATA
        );

        assert_eq!(
            MsiEnumFeaturesW(
                std::ptr::null(),
                0,
                buf_w.as_mut_ptr(),
                std::ptr::null_mut()
            ),
            ERROR_INVALID_PARAMETER
        );
        assert_eq!(
            MsiEnumFeaturesW(
                valid_w.as_ptr(),
                0,
                std::ptr::null_mut(),
                std::ptr::null_mut()
            ),
            ERROR_INVALID_PARAMETER
        );
        assert_eq!(
            MsiEnumFeaturesA(
                std::ptr::null(),
                0,
                buf_a.as_mut_ptr(),
                std::ptr::null_mut()
            ),
            ERROR_INVALID_PARAMETER
        );
        assert_eq!(
            MsiEnumFeaturesA(
                valid_a.as_ptr().cast::<i8>(),
                0,
                std::ptr::null_mut(),
                std::ptr::null_mut()
            ),
            ERROR_INVALID_PARAMETER
        );

        assert_eq!(
            MsiEnumComponentsW(0, std::ptr::null_mut()),
            ERROR_INVALID_PARAMETER
        );
        assert_eq!(
            MsiEnumComponentsA(0, std::ptr::null_mut()),
            ERROR_INVALID_PARAMETER
        );

        assert_eq!(
            MsiQueryFeatureStateA(std::ptr::null(), feat_a.as_ptr().cast::<i8>()),
            INSTALLSTATE_UNKNOWN
        );
        assert_eq!(
            MsiQueryFeatureStateA(valid_a.as_ptr().cast::<i8>(), std::ptr::null()),
            INSTALLSTATE_UNKNOWN
        );

        assert_eq!(
            MsiGetProductInfoA(
                std::ptr::null(),
                feat_a.as_ptr().cast::<i8>(),
                buf_a.as_mut_ptr(),
                &raw mut pcch
            ),
            ERROR_INVALID_PARAMETER
        );
        assert_eq!(
            MsiGetProductInfoA(
                valid_a.as_ptr().cast::<i8>(),
                std::ptr::null(),
                buf_a.as_mut_ptr(),
                &raw mut pcch
            ),
            ERROR_INVALID_PARAMETER
        );

        assert_eq!(
            MsiEnumClientsW(std::ptr::null(), 0, buf_w.as_mut_ptr()),
            ERROR_INVALID_PARAMETER
        );
        assert_eq!(
            MsiEnumClientsA(std::ptr::null(), 0, buf_a.as_mut_ptr()),
            ERROR_INVALID_PARAMETER
        );
        assert_eq!(
            MsiEnumClientsExW(
                std::ptr::null(),
                std::ptr::null(),
                0,
                0,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut()
            ),
            ERROR_INVALID_PARAMETER
        );
        assert_eq!(
            MsiEnumClientsExA(
                std::ptr::null(),
                std::ptr::null(),
                0,
                0,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut()
            ),
            ERROR_INVALID_PARAMETER
        );
        assert_eq!(
            MsiEnumComponentQualifiersW(
                std::ptr::null(),
                0,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut()
            ),
            ERROR_INVALID_PARAMETER
        );
        assert_eq!(
            MsiEnumComponentQualifiersA(
                std::ptr::null(),
                0,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut()
            ),
            ERROR_INVALID_PARAMETER
        );
        assert_eq!(
            MsiEnumPatchesW(
                std::ptr::null(),
                0,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut()
            ),
            ERROR_INVALID_PARAMETER
        );
        assert_eq!(
            MsiEnumPatchesA(
                std::ptr::null(),
                0,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut()
            ),
            ERROR_INVALID_PARAMETER
        );
        assert_eq!(
            MsiEnumPatchesExW(
                std::ptr::null(),
                std::ptr::null(),
                0,
                0,
                0,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut()
            ),
            ERROR_INVALID_PARAMETER
        );
        assert_eq!(
            MsiEnumPatchesExA(
                std::ptr::null(),
                std::ptr::null(),
                0,
                0,
                0,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut()
            ),
            ERROR_INVALID_PARAMETER
        );
        assert_eq!(
            MsiEnumRelatedProductsW(std::ptr::null(), 0, 0, buf_w.as_mut_ptr()),
            ERROR_INVALID_PARAMETER
        );
        assert_eq!(
            MsiEnumRelatedProductsA(std::ptr::null(), 0, 0, buf_a.as_mut_ptr()),
            ERROR_INVALID_PARAMETER
        );
        assert_eq!(
            MsiEnumProductsExW(
                std::ptr::null(),
                std::ptr::null(),
                0,
                0,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut()
            ),
            ERROR_NO_MORE_ITEMS
        );
        assert_eq!(
            MsiEnumProductsExA(
                std::ptr::null(),
                std::ptr::null(),
                0,
                0,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut()
            ),
            ERROR_NO_MORE_ITEMS
        );
        assert_eq!(
            MsiEnumComponentsExW(
                std::ptr::null(),
                0,
                0,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut()
            ),
            ERROR_NO_MORE_ITEMS
        );
        assert_eq!(
            MsiEnumComponentsExA(
                std::ptr::null(),
                0,
                0,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut()
            ),
            ERROR_NO_MORE_ITEMS
        );

        assert_eq!(
            MsiQueryFeatureStateExW(
                std::ptr::null(),
                std::ptr::null(),
                0,
                std::ptr::null(),
                std::ptr::null_mut()
            ),
            ERROR_INVALID_PARAMETER
        );
        assert_eq!(
            MsiQueryFeatureStateExA(
                std::ptr::null(),
                std::ptr::null(),
                0,
                std::ptr::null(),
                std::ptr::null_mut()
            ),
            ERROR_INVALID_PARAMETER
        );
        assert_eq!(
            MsiGetComponentStateW(
                0,
                std::ptr::null(),
                std::ptr::null_mut(),
                std::ptr::null_mut()
            ),
            ERROR_INVALID_PARAMETER
        );
        assert_eq!(
            MsiGetComponentStateA(
                0,
                std::ptr::null(),
                std::ptr::null_mut(),
                std::ptr::null_mut()
            ),
            ERROR_INVALID_PARAMETER
        );
        assert_eq!(
            MsiGetFeatureStateW(
                0,
                std::ptr::null(),
                std::ptr::null_mut(),
                std::ptr::null_mut()
            ),
            ERROR_INVALID_PARAMETER
        );
        assert_eq!(
            MsiGetFeatureStateA(
                0,
                std::ptr::null(),
                std::ptr::null_mut(),
                std::ptr::null_mut()
            ),
            ERROR_INVALID_PARAMETER
        );
        assert_eq!(
            MsiGetComponentPathExW(
                std::ptr::null(),
                std::ptr::null(),
                std::ptr::null(),
                0,
                std::ptr::null_mut(),
                std::ptr::null_mut()
            ),
            ERROR_INVALID_PARAMETER
        );
        assert_eq!(
            MsiGetComponentPathExA(
                std::ptr::null(),
                std::ptr::null(),
                std::ptr::null(),
                0,
                std::ptr::null_mut(),
                std::ptr::null_mut()
            ),
            ERROR_INVALID_PARAMETER
        );
        assert_eq!(
            MsiGetComponentPathExW(
                valid_w.as_ptr(),
                valid_w.as_ptr(),
                std::ptr::null(),
                0,
                std::ptr::null_mut(),
                std::ptr::null_mut()
            ),
            ERROR_NO_MORE_ITEMS
        );
        assert_eq!(
            MsiGetComponentPathExA(
                valid_a.as_ptr().cast(),
                valid_a.as_ptr().cast(),
                std::ptr::null(),
                0,
                std::ptr::null_mut(),
                std::ptr::null_mut()
            ),
            ERROR_NO_MORE_ITEMS
        );

        assert_eq!(
            MsiGetComponentPathW(
                std::ptr::null(),
                std::ptr::null(),
                std::ptr::null_mut(),
                std::ptr::null_mut()
            ),
            INSTALLSTATE_UNKNOWN
        );
        assert_eq!(
            MsiGetComponentPathA(
                std::ptr::null(),
                std::ptr::null(),
                std::ptr::null_mut(),
                std::ptr::null_mut()
            ),
            INSTALLSTATE_UNKNOWN
        );
        assert_eq!(
            MsiGetComponentPathW(
                valid_w.as_ptr(),
                valid_w.as_ptr(),
                std::ptr::null_mut(),
                std::ptr::null_mut()
            ),
            1
        );
        assert_eq!(
            MsiGetComponentPathA(
                valid_a.as_ptr().cast(),
                valid_a.as_ptr().cast(),
                std::ptr::null_mut(),
                std::ptr::null_mut()
            ),
            1
        );
        assert_eq!(
            MsiGetFeatureValidStatesW(0, std::ptr::null(), std::ptr::null_mut()),
            ERROR_INVALID_PARAMETER
        );
        assert_eq!(
            MsiGetFeatureValidStatesA(0, std::ptr::null(), std::ptr::null_mut()),
            ERROR_INVALID_PARAMETER
        );
        assert_eq!(
            MsiGetFeatureCostW(0, std::ptr::null(), 0, 0, std::ptr::null_mut()),
            ERROR_INVALID_PARAMETER
        );
        assert_eq!(
            MsiGetFeatureCostA(0, std::ptr::null(), 0, 0, std::ptr::null_mut()),
            ERROR_INVALID_PARAMETER
        );
        assert_eq!(
            MsiEnumComponentCostsW(
                0,
                std::ptr::null(),
                0,
                0,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut()
            ),
            ERROR_INVALID_PARAMETER
        );
        assert_eq!(
            MsiEnumComponentCostsA(
                0,
                std::ptr::null(),
                0,
                0,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut()
            ),
            ERROR_INVALID_PARAMETER
        );
        unsafe {
            assert_eq!(
                MsiGetShortcutTargetW(
                    std::ptr::null(),
                    std::ptr::null_mut(),
                    std::ptr::null_mut(),
                    std::ptr::null_mut()
                ),
                ERROR_INVALID_PARAMETER
            );
            assert_eq!(
                MsiGetShortcutTargetA(
                    std::ptr::null(),
                    std::ptr::null_mut(),
                    std::ptr::null_mut(),
                    std::ptr::null_mut()
                ),
                ERROR_INVALID_PARAMETER
            );
        }
        assert_eq!(
            MsiDecomposeDescriptorW(
                std::ptr::null(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut()
            ),
            ERROR_INVALID_PARAMETER
        );
        assert_eq!(
            MsiDecomposeDescriptorA(
                std::ptr::null(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut()
            ),
            ERROR_INVALID_PARAMETER
        );
        assert_eq!(
            MsiGetFileVersionW(
                std::ptr::null(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut()
            ),
            ERROR_INVALID_PARAMETER
        );
        assert_eq!(
            MsiGetFileVersionA(
                std::ptr::null(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut()
            ),
            ERROR_INVALID_PARAMETER
        );
        assert_eq!(
            MsiEnumClientsW(valid_w.as_ptr(), 0, std::ptr::null_mut()),
            ERROR_INVALID_PARAMETER
        );
        assert_eq!(
            MsiEnumClientsA(valid_a.as_ptr().cast(), 0, std::ptr::null_mut()),
            ERROR_INVALID_PARAMETER
        );
        assert_eq!(
            MsiEnumClientsExW(
                valid_w.as_ptr(),
                std::ptr::null(),
                0,
                0,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut()
            ),
            ERROR_NO_MORE_ITEMS
        );
        assert_eq!(
            MsiEnumClientsExA(
                valid_a.as_ptr().cast(),
                std::ptr::null(),
                0,
                0,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut()
            ),
            ERROR_NO_MORE_ITEMS
        );
        assert_eq!(
            MsiEnumComponentCostsW(
                0,
                valid_w.as_ptr(),
                0,
                0,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut()
            ),
            ERROR_NO_MORE_ITEMS
        );
        assert_eq!(
            MsiEnumComponentCostsA(
                0,
                valid_a.as_ptr().cast(),
                0,
                0,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut()
            ),
            ERROR_NO_MORE_ITEMS
        );
        assert_eq!(
            MsiEnumComponentQualifiersW(
                valid_w.as_ptr(),
                0,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut()
            ),
            ERROR_NO_MORE_ITEMS
        );
        assert_eq!(
            MsiEnumComponentQualifiersA(
                valid_a.as_ptr().cast(),
                0,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut()
            ),
            ERROR_NO_MORE_ITEMS
        );
        assert_eq!(
            MsiEnumComponentsW(0, std::ptr::null_mut()),
            ERROR_INVALID_PARAMETER
        );
        assert_eq!(
            MsiEnumComponentsA(0, std::ptr::null_mut()),
            ERROR_INVALID_PARAMETER
        );
        assert_eq!(
            MsiEnumComponentsExW(
                std::ptr::null(),
                0,
                0,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut()
            ),
            ERROR_NO_MORE_ITEMS
        );
        assert_eq!(
            MsiEnumComponentsExA(
                std::ptr::null(),
                0,
                0,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut()
            ),
            ERROR_NO_MORE_ITEMS
        );
        assert_eq!(
            MsiEnumFeaturesW(
                valid_w.as_ptr(),
                0,
                std::ptr::null_mut(),
                std::ptr::null_mut()
            ),
            ERROR_INVALID_PARAMETER
        );
        assert_eq!(
            MsiEnumFeaturesA(
                valid_a.as_ptr().cast(),
                0,
                std::ptr::null_mut(),
                std::ptr::null_mut()
            ),
            ERROR_INVALID_PARAMETER
        );
        assert_eq!(
            MsiEnumPatchesW(
                valid_w.as_ptr(),
                0,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut()
            ),
            ERROR_INVALID_PARAMETER
        );
        assert_eq!(
            MsiEnumPatchesA(
                valid_a.as_ptr().cast(),
                0,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut()
            ),
            ERROR_INVALID_PARAMETER
        );
        assert_eq!(
            MsiEnumPatchesExW(
                valid_w.as_ptr(),
                std::ptr::null(),
                0,
                0,
                0,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut()
            ),
            ERROR_NO_MORE_ITEMS
        );
        assert_eq!(
            MsiEnumPatchesExA(
                valid_a.as_ptr().cast(),
                std::ptr::null(),
                0,
                0,
                0,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut()
            ),
            ERROR_NO_MORE_ITEMS
        );
        assert_eq!(
            MsiEnumProductsW(0, std::ptr::null_mut()),
            ERROR_INVALID_PARAMETER
        );
        assert_eq!(
            MsiEnumProductsA(0, std::ptr::null_mut()),
            ERROR_INVALID_PARAMETER
        );
        assert_eq!(
            MsiEnumProductsExW(
                std::ptr::null(),
                std::ptr::null(),
                0,
                0,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut()
            ),
            ERROR_NO_MORE_ITEMS
        );
        assert_eq!(
            MsiEnumProductsExA(
                std::ptr::null(),
                std::ptr::null(),
                0,
                0,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut()
            ),
            ERROR_NO_MORE_ITEMS
        );
        assert_eq!(
            MsiEnumRelatedProductsW(valid_w.as_ptr(), 0, 0, std::ptr::null_mut()),
            ERROR_INVALID_PARAMETER
        );
        assert_eq!(
            MsiEnumRelatedProductsA(valid_a.as_ptr().cast(), 0, 0, std::ptr::null_mut()),
            ERROR_INVALID_PARAMETER
        );
        assert_eq!(
            MsiGetFileVersionW(
                valid_w.as_ptr(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut()
            ),
            ERROR_NO_MORE_ITEMS
        );
        assert_eq!(
            MsiGetFileVersionA(
                valid_a.as_ptr().cast(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut()
            ),
            ERROR_NO_MORE_ITEMS
        );

        let mut buf_w = [0_u16; 39];
        let mut buf_a = [0_i8; 39];
        assert_eq!(
            MsiEnumClientsW(valid_w.as_ptr(), 0, buf_w.as_mut_ptr()),
            ERROR_NO_MORE_ITEMS
        );
        assert_eq!(
            MsiEnumClientsA(valid_a.as_ptr().cast(), 0, buf_a.as_mut_ptr()),
            ERROR_NO_MORE_ITEMS
        );
        assert_eq!(
            MsiEnumClientsExW(
                valid_w.as_ptr(),
                std::ptr::null(),
                0,
                0,
                buf_w.as_mut_ptr(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut()
            ),
            ERROR_NO_MORE_ITEMS
        );
        assert_eq!(
            MsiEnumClientsExA(
                valid_a.as_ptr().cast(),
                std::ptr::null(),
                0,
                0,
                buf_a.as_mut_ptr(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut()
            ),
            ERROR_NO_MORE_ITEMS
        );
        assert_eq!(
            MsiEnumComponentCostsW(
                0,
                valid_w.as_ptr(),
                0,
                0,
                buf_w.as_mut_ptr(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut()
            ),
            ERROR_NO_MORE_ITEMS
        );
        assert_eq!(
            MsiEnumComponentCostsA(
                0,
                valid_a.as_ptr().cast(),
                0,
                0,
                buf_a.as_mut_ptr(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut()
            ),
            ERROR_NO_MORE_ITEMS
        );
        assert_eq!(
            MsiEnumComponentQualifiersW(
                valid_w.as_ptr(),
                0,
                buf_w.as_mut_ptr(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut()
            ),
            ERROR_NO_MORE_ITEMS
        );
        assert_eq!(
            MsiEnumComponentQualifiersA(
                valid_a.as_ptr().cast(),
                0,
                buf_a.as_mut_ptr(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut()
            ),
            ERROR_NO_MORE_ITEMS
        );
        assert_eq!(
            MsiEnumComponentsW(0, buf_w.as_mut_ptr()),
            ERROR_NO_MORE_ITEMS
        );
        assert_eq!(
            MsiEnumComponentsA(0, buf_a.as_mut_ptr()),
            ERROR_NO_MORE_ITEMS
        );
        assert_eq!(
            MsiEnumComponentsExW(
                std::ptr::null(),
                0,
                0,
                buf_w.as_mut_ptr(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut()
            ),
            ERROR_NO_MORE_ITEMS
        );
        assert_eq!(
            MsiEnumComponentsExA(
                std::ptr::null(),
                0,
                0,
                buf_a.as_mut_ptr(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut()
            ),
            ERROR_NO_MORE_ITEMS
        );
        assert_eq!(
            MsiEnumFeaturesW(
                valid_w.as_ptr(),
                0,
                buf_w.as_mut_ptr(),
                std::ptr::null_mut()
            ),
            ERROR_NO_MORE_ITEMS
        );
        assert_eq!(
            MsiEnumFeaturesA(
                valid_a.as_ptr().cast(),
                0,
                buf_a.as_mut_ptr(),
                std::ptr::null_mut()
            ),
            ERROR_NO_MORE_ITEMS
        );
        assert_eq!(
            MsiEnumPatchesW(
                valid_w.as_ptr(),
                0,
                buf_w.as_mut_ptr(),
                std::ptr::null_mut(),
                std::ptr::null_mut()
            ),
            ERROR_NO_MORE_ITEMS
        );
        assert_eq!(
            MsiEnumPatchesA(
                valid_a.as_ptr().cast(),
                0,
                buf_a.as_mut_ptr(),
                std::ptr::null_mut(),
                std::ptr::null_mut()
            ),
            ERROR_NO_MORE_ITEMS
        );
        assert_eq!(
            MsiEnumPatchesExW(
                valid_w.as_ptr(),
                std::ptr::null(),
                0,
                0,
                0,
                buf_w.as_mut_ptr(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut()
            ),
            ERROR_NO_MORE_ITEMS
        );
        assert_eq!(
            MsiEnumPatchesExA(
                valid_a.as_ptr().cast(),
                std::ptr::null(),
                0,
                0,
                0,
                buf_a.as_mut_ptr(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut()
            ),
            ERROR_NO_MORE_ITEMS
        );
        assert_eq!(MsiEnumProductsW(0, buf_w.as_mut_ptr()), ERROR_NO_MORE_ITEMS);
        assert_eq!(MsiEnumProductsA(0, buf_a.as_mut_ptr()), ERROR_NO_MORE_ITEMS);
        assert_eq!(
            MsiEnumProductsExW(
                std::ptr::null(),
                std::ptr::null(),
                0,
                0,
                buf_w.as_mut_ptr(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut()
            ),
            ERROR_NO_MORE_ITEMS
        );
        assert_eq!(
            MsiEnumProductsExA(
                std::ptr::null(),
                std::ptr::null(),
                0,
                0,
                buf_a.as_mut_ptr(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut()
            ),
            ERROR_NO_MORE_ITEMS
        );
        assert_eq!(
            MsiEnumRelatedProductsW(valid_w.as_ptr(), 0, 0, buf_w.as_mut_ptr()),
            ERROR_NO_MORE_ITEMS
        );
        assert_eq!(
            MsiEnumRelatedProductsA(valid_a.as_ptr().cast(), 0, 0, buf_a.as_mut_ptr()),
            ERROR_NO_MORE_ITEMS
        );

        assert_eq!(
            MsiQueryFeatureStateExW(
                valid_w.as_ptr(),
                std::ptr::null(),
                0,
                valid_w.as_ptr(),
                std::ptr::null_mut()
            ),
            ERROR_NO_MORE_ITEMS
        );
        assert_eq!(
            MsiQueryFeatureStateExA(
                valid_a.as_ptr().cast(),
                std::ptr::null(),
                0,
                valid_a.as_ptr().cast(),
                std::ptr::null_mut()
            ),
            ERROR_NO_MORE_ITEMS
        );
        assert_eq!(
            MsiGetComponentStateW(
                0,
                valid_w.as_ptr(),
                std::ptr::null_mut(),
                std::ptr::null_mut()
            ),
            ERROR_NO_MORE_ITEMS
        );
        assert_eq!(
            MsiGetComponentStateA(
                0,
                valid_a.as_ptr().cast(),
                std::ptr::null_mut(),
                std::ptr::null_mut()
            ),
            ERROR_NO_MORE_ITEMS
        );
        assert_eq!(
            MsiGetFeatureStateW(
                0,
                valid_w.as_ptr(),
                std::ptr::null_mut(),
                std::ptr::null_mut()
            ),
            ERROR_NO_MORE_ITEMS
        );
        assert_eq!(
            MsiGetFeatureStateA(
                0,
                valid_a.as_ptr().cast(),
                std::ptr::null_mut(),
                std::ptr::null_mut()
            ),
            ERROR_NO_MORE_ITEMS
        );
        assert_eq!(
            MsiGetFeatureValidStatesW(0, valid_w.as_ptr(), std::ptr::null_mut()),
            ERROR_NO_MORE_ITEMS
        );
        assert_eq!(
            MsiGetFeatureValidStatesA(0, valid_a.as_ptr().cast(), std::ptr::null_mut()),
            ERROR_NO_MORE_ITEMS
        );
        assert_eq!(
            MsiGetFeatureCostW(0, valid_w.as_ptr(), 0, 0, std::ptr::null_mut()),
            ERROR_NO_MORE_ITEMS
        );
        assert_eq!(
            MsiGetFeatureCostA(0, valid_a.as_ptr().cast(), 0, 0, std::ptr::null_mut()),
            ERROR_NO_MORE_ITEMS
        );
        assert_eq!(
            MsiDecomposeDescriptorW(
                valid_w.as_ptr(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut()
            ),
            ERROR_NO_MORE_ITEMS
        );
        assert_eq!(
            MsiDecomposeDescriptorA(
                valid_a.as_ptr().cast(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut()
            ),
            ERROR_NO_MORE_ITEMS
        );

        assert_eq!(
            MsiCollectUserInfoW(std::ptr::null()),
            ERROR_INVALID_PARAMETER
        );
        assert_eq!(
            MsiCollectUserInfoA(std::ptr::null()),
            ERROR_INVALID_PARAMETER
        );
        assert_eq!(MsiCollectUserInfoW(valid_w.as_ptr()), ERROR_SUCCESS);
        assert_eq!(
            MsiCollectUserInfoA(valid_a.as_ptr().cast::<i8>()),
            ERROR_SUCCESS
        );

        assert_eq!(
            MsiGetUserInfoW(
                std::ptr::null(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut()
            ),
            INSTALLSTATE_UNKNOWN
        );
        assert_eq!(
            MsiGetUserInfoA(
                std::ptr::null(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut()
            ),
            INSTALLSTATE_UNKNOWN
        );
        assert_eq!(
            MsiGetUserInfoW(
                valid_w.as_ptr(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut()
            ),
            INSTALLSTATE_UNKNOWN
        );
        assert_eq!(
            MsiGetUserInfoA(
                valid_a.as_ptr().cast::<i8>(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut()
            ),
            INSTALLSTATE_UNKNOWN
        );

        assert_eq!(
            Migrate10CachedPackagesW(std::ptr::null(), std::ptr::null(), std::ptr::null(), 0),
            ERROR_SUCCESS
        );
    }
}

/// (Stub) Legacy cached packages migration.
///
/// # Arguments
///
/// * `_szProductCode` - Pointer to a string specifying the product code.
/// * `_szUserSid` - Pointer to a string specifying the user name.
/// * `_szPreflightCheck` - Reserved.
/// * `_dwReserved` - Reserved.
///
/// # Returns
///
/// Always returns `ERROR_CALL_NOT_IMPLEMENTED`.
#[cfg_attr(not(coverage_nightly), no_mangle)]
#[inline(never)]
#[allow(non_snake_case)]
#[allow(clippy::missing_const_for_fn)]
pub extern "system" fn Migrate10CachedPackagesA(
    _szProductCode: Lpcstr,
    _szUserSid: Lpcstr,
    _szPreflightCheck: Lpcstr,
    _dwReserved: Dword,
) -> Uint {
    1605 // ERROR_UNKNOWN_PRODUCT
}

#[cfg(test)]
mod additional_stubs_tests {
    use super::*;

    #[test]
    fn test_stubs_impl() {
        assert_eq!(
            Migrate10CachedPackagesA(std::ptr::null(), std::ptr::null(), std::ptr::null(), 0),
            1605
        );
    }
}

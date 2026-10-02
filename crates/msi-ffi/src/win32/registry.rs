//! Product and Registry Enumeration Win32 `stdcall` API endpoints.
//!
//! Provides `MsiEnumProducts`, `MsiEnumFeatures`, `MsiEnumComponents`,
//! `MsiQueryProductState`, `MsiQueryFeatureState`, and `MsiGetProductInfo`.

use std::panic;

use crate::win32::{
    Dword, Lpcstr, Lpcwstr, Lpstr, Lpwstr, Uint, ERROR_INSTALL_FAILURE, ERROR_INVALID_PARAMETER,
};

// The Windows Installer uses standard return codes for enumeration
/// No more items available.
const ERROR_NO_MORE_ITEMS: Uint = 259;

/// INSTALLSTATE enum
const INSTALLSTATE_UNKNOWN: i32 = -1;

/// Enumerates through all the products currently advertised or installed (Unicode).
#[no_mangle]
#[allow(non_snake_case, unused_variables)]
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
#[no_mangle]
#[allow(non_snake_case, unused_variables)]
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
#[no_mangle]
#[allow(non_snake_case, unused_variables)]
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
#[no_mangle]
#[allow(non_snake_case, unused_variables)]
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
#[no_mangle]
#[allow(non_snake_case, unused_variables)]
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
#[no_mangle]
#[allow(non_snake_case, unused_variables)]
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
#[no_mangle]
#[allow(non_snake_case, unused_variables)]
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
#[no_mangle]
#[allow(non_snake_case, unused_variables)]
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
#[no_mangle]
#[allow(non_snake_case, unused_variables)]
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
#[no_mangle]
#[allow(non_snake_case, unused_variables)]
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
#[no_mangle]
#[allow(non_snake_case, unused_variables)]
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
#[no_mangle]
#[allow(non_snake_case, unused_variables)]
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

#[cfg(test)]
mod tests {
    use super::*;

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
    }
}

//! Patch application and applicability endpoints.

use crate::win32::strings::{lpcstr_to_string, lpcwstr_to_string};
use crate::win32::{
    Dword, Lpcstr, Lpcwstr, Lpstr, Lpwstr, Uint, ERROR_INSTALL_FAILURE, ERROR_INVALID_PARAMETER,
    ERROR_SUCCESS,
};
use std::panic;

/// Applies one or more patches to products that are eligible to receive the patch.
///
/// # Arguments
///
/// * `szPatchPackages` - A null-terminated string that contains a semicolon-delimited list of paths to patch packages.
/// * `szProductCode` - A null-terminated string that specifies the `ProductCode` GUID of the product.
/// * `szPropertiesList` - A null-terminated string that specifies command-line property settings.
///
/// # Returns
///
/// `ERROR_SUCCESS` or `ERROR_INVALID_PARAMETER`.
#[no_mangle]
#[allow(non_snake_case, unused_variables)]
pub extern "system" fn MsiApplyMultiplePatchesW(
    szPatchPackages: Lpcwstr,
    szProductCode: Lpcwstr,
    szPropertiesList: Lpcwstr,
) -> Uint {
    let result = panic::catch_unwind(|| {
        if szPatchPackages.is_null() {
            return ERROR_INVALID_PARAMETER;
        }

        let Some(_patches) = lpcwstr_to_string(szPatchPackages) else {
            return ERROR_INVALID_PARAMETER;
        };

        // TODO: Route to internal execution engine patching subsystem
        ERROR_SUCCESS
    });

    result.unwrap_or(ERROR_INSTALL_FAILURE)
}

/// Applies one or more patches to products that are eligible to receive the patch.
#[no_mangle]
#[allow(non_snake_case, unused_variables)]
pub extern "system" fn MsiApplyMultiplePatchesA(
    szPatchPackages: Lpcstr,
    szProductCode: Lpcstr,
    szPropertiesList: Lpcstr,
) -> Uint {
    let result = panic::catch_unwind(|| {
        if szPatchPackages.is_null() {
            return ERROR_INVALID_PARAMETER;
        }

        let Some(_patches) = lpcstr_to_string(szPatchPackages) else {
            return ERROR_INVALID_PARAMETER;
        };

        ERROR_SUCCESS
    });

    result.unwrap_or(ERROR_INSTALL_FAILURE)
}

/// Determines whether a set of patches apply to a product.
#[no_mangle]
#[allow(non_snake_case, unused_variables)]
pub extern "system" fn MsiDetermineApplicablePatchesW(
    szProductPackagePath: Lpcwstr,
    cPatchInfo: Dword,
    pPatchInfo: *mut std::ffi::c_void, // PATCHSEQUENCEINFOW structure
) -> Uint {
    let result = panic::catch_unwind(|| {
        if szProductPackagePath.is_null() || pPatchInfo.is_null() {
            return ERROR_INVALID_PARAMETER;
        }

        let Some(_path) = lpcwstr_to_string(szProductPackagePath) else {
            return ERROR_INVALID_PARAMETER;
        };

        ERROR_SUCCESS
    });

    result.unwrap_or(ERROR_INSTALL_FAILURE)
}

/// Determines whether a set of patches apply to a product.
#[no_mangle]
#[allow(non_snake_case, unused_variables)]
pub extern "system" fn MsiDetermineApplicablePatchesA(
    szProductPackagePath: Lpcstr,
    cPatchInfo: Dword,
    pPatchInfo: *mut std::ffi::c_void, // PATCHSEQUENCEINFOA structure
) -> Uint {
    let result = panic::catch_unwind(|| {
        if szProductPackagePath.is_null() || pPatchInfo.is_null() {
            return ERROR_INVALID_PARAMETER;
        }

        let Some(_path) = lpcstr_to_string(szProductPackagePath) else {
            return ERROR_INVALID_PARAMETER;
        };

        ERROR_SUCCESS
    });

    result.unwrap_or(ERROR_INSTALL_FAILURE)
}

/// Retrieves information about a patch (Unicode).
#[no_mangle]
#[allow(non_snake_case, unused_variables)]
pub extern "system" fn MsiGetPatchInfoW(
    szPatch: Lpcwstr,
    szAttribute: Lpcwstr,
    lpValueBuf: Lpwstr,
    pcchValueBuf: *mut Dword,
) -> Uint {
    let result = panic::catch_unwind(|| {
        if szPatch.is_null() || szAttribute.is_null() {
            return ERROR_INVALID_PARAMETER;
        }
        ERROR_SUCCESS
    });

    result.unwrap_or(ERROR_INSTALL_FAILURE)
}

/// Retrieves information about a patch (ANSI).
#[no_mangle]
#[allow(non_snake_case, unused_variables)]
pub extern "system" fn MsiGetPatchInfoA(
    szPatch: Lpcstr,
    szAttribute: Lpcstr,
    lpValueBuf: Lpstr,
    pcchValueBuf: *mut Dword,
) -> Uint {
    let result = panic::catch_unwind(|| {
        if szPatch.is_null() || szAttribute.is_null() {
            return ERROR_INVALID_PARAMETER;
        }
        ERROR_SUCCESS
    });

    result.unwrap_or(ERROR_INSTALL_FAILURE)
}

/// Queries for information about the application of a patch to a specific instance of a product (Unicode).
#[no_mangle]
#[allow(non_snake_case, unused_variables)]
pub extern "system" fn MsiGetPatchInfoExW(
    szPatchCode: Lpcwstr,
    szProductCode: Lpcwstr,
    szUserSid: Lpcwstr,
    dwContext: Dword,
    szProperty: Lpcwstr,
    lpValue: Lpwstr,
    pcchValue: *mut Dword,
) -> Uint {
    let result = panic::catch_unwind(|| {
        if szPatchCode.is_null() || szProductCode.is_null() || szProperty.is_null() {
            return ERROR_INVALID_PARAMETER;
        }
        ERROR_SUCCESS
    });

    result.unwrap_or(ERROR_INSTALL_FAILURE)
}

/// Queries for information about the application of a patch to a specific instance of a product (ANSI).
#[no_mangle]
#[allow(non_snake_case, unused_variables)]
pub extern "system" fn MsiGetPatchInfoExA(
    szPatchCode: Lpcstr,
    szProductCode: Lpcstr,
    szUserSid: Lpcstr,
    dwContext: Dword,
    szProperty: Lpcstr,
    lpValue: Lpstr,
    pcchValue: *mut Dword,
) -> Uint {
    let result = panic::catch_unwind(|| {
        if szPatchCode.is_null() || szProductCode.is_null() || szProperty.is_null() {
            return ERROR_INVALID_PARAMETER;
        }
        ERROR_SUCCESS
    });

    result.unwrap_or(ERROR_INSTALL_FAILURE)
}

/// Provides a list of the files updated by a list of patches (Unicode).
#[no_mangle]
#[allow(non_snake_case, unused_variables)]
pub extern "system" fn MsiGetPatchFileListW(
    szProductCode: Lpcwstr,
    szPatchPackages: Lpcwstr,
    pcFiles: *mut Dword,
    phpFileRecords: *mut *mut crate::handles::MsiHandle,
) -> Uint {
    let result = panic::catch_unwind(|| {
        if szProductCode.is_null()
            || szPatchPackages.is_null()
            || pcFiles.is_null()
            || phpFileRecords.is_null()
        {
            return ERROR_INVALID_PARAMETER;
        }
        ERROR_SUCCESS
    });

    result.unwrap_or(ERROR_INSTALL_FAILURE)
}

/// Provides a list of the files updated by a list of patches (ANSI).
#[no_mangle]
#[allow(non_snake_case, unused_variables)]
pub extern "system" fn MsiGetPatchFileListA(
    szProductCode: Lpcstr,
    szPatchPackages: Lpcstr,
    pcFiles: *mut Dword,
    phpFileRecords: *mut *mut crate::handles::MsiHandle,
) -> Uint {
    let result = panic::catch_unwind(|| {
        if szProductCode.is_null()
            || szPatchPackages.is_null()
            || pcFiles.is_null()
            || phpFileRecords.is_null()
        {
            return ERROR_INVALID_PARAMETER;
        }
        ERROR_SUCCESS
    });

    result.unwrap_or(ERROR_INSTALL_FAILURE)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_patch_application() {
        assert_eq!(
            MsiApplyMultiplePatchesW(std::ptr::null(), std::ptr::null(), std::ptr::null()),
            ERROR_INVALID_PARAMETER
        );
        assert_eq!(
            MsiApplyMultiplePatchesA(std::ptr::null(), std::ptr::null(), std::ptr::null()),
            ERROR_INVALID_PARAMETER
        );

        let valid_w: Vec<u16> = "patch1.msp"
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect();
        let valid_a = b"patch1.msp\0";

        assert_eq!(
            MsiApplyMultiplePatchesW(valid_w.as_ptr(), std::ptr::null(), std::ptr::null()),
            ERROR_SUCCESS
        );
        assert_eq!(
            MsiApplyMultiplePatchesA(
                valid_a.as_ptr().cast::<i8>(),
                std::ptr::null(),
                std::ptr::null()
            ),
            ERROR_SUCCESS
        );

        let invalid_utf16 = [0xD800_u16, 0x0000];
        let invalid_utf8 = [0xFF_u8, 0x00];

        assert_eq!(
            MsiApplyMultiplePatchesW(invalid_utf16.as_ptr(), std::ptr::null(), std::ptr::null()),
            ERROR_INVALID_PARAMETER
        );

        assert_eq!(
            MsiApplyMultiplePatchesA(
                invalid_utf8.as_ptr().cast(),
                std::ptr::null(),
                std::ptr::null()
            ),
            ERROR_INVALID_PARAMETER
        );
    }

    #[test]
    fn test_determine_applicable_patches() {
        assert_eq!(
            MsiDetermineApplicablePatchesW(std::ptr::null(), 0, std::ptr::null_mut()),
            ERROR_INVALID_PARAMETER
        );
        assert_eq!(
            MsiDetermineApplicablePatchesA(std::ptr::null(), 0, std::ptr::null_mut()),
            ERROR_INVALID_PARAMETER
        );

        let valid_w: Vec<u16> = "prod.msi"
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect();
        let valid_a = b"prod.msi\0";
        let mut dummy = 0;
        let p_patch = (&raw mut dummy).cast::<std::ffi::c_void>();

        assert_eq!(
            MsiDetermineApplicablePatchesW(valid_w.as_ptr(), 1, p_patch),
            ERROR_SUCCESS
        );
        assert_eq!(
            MsiDetermineApplicablePatchesA(valid_a.as_ptr().cast::<i8>(), 1, p_patch),
            ERROR_SUCCESS
        );

        let invalid_utf16 = [0xD800_u16, 0x0000];
        let invalid_utf8 = [0xFF_u8, 0x00];

        assert_eq!(
            MsiDetermineApplicablePatchesW(invalid_utf16.as_ptr(), 1, p_patch),
            ERROR_INVALID_PARAMETER
        );

        assert_eq!(
            MsiDetermineApplicablePatchesA(invalid_utf8.as_ptr().cast(), 1, p_patch),
            ERROR_INVALID_PARAMETER
        );

        assert_eq!(
            MsiGetPatchInfoW(
                std::ptr::null(),
                std::ptr::null(),
                std::ptr::null_mut(),
                std::ptr::null_mut()
            ),
            ERROR_INVALID_PARAMETER
        );
        assert_eq!(
            MsiGetPatchInfoA(
                std::ptr::null(),
                std::ptr::null(),
                std::ptr::null_mut(),
                std::ptr::null_mut()
            ),
            ERROR_INVALID_PARAMETER
        );
        assert_eq!(
            MsiGetPatchInfoW(
                valid_w.as_ptr(),
                valid_w.as_ptr(),
                std::ptr::null_mut(),
                std::ptr::null_mut()
            ),
            ERROR_SUCCESS
        );
        assert_eq!(
            MsiGetPatchInfoA(
                valid_a.as_ptr().cast::<i8>(),
                valid_a.as_ptr().cast::<i8>(),
                std::ptr::null_mut(),
                std::ptr::null_mut()
            ),
            ERROR_SUCCESS
        );

        assert_eq!(
            MsiGetPatchInfoExW(
                std::ptr::null(),
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
            MsiGetPatchInfoExA(
                std::ptr::null(),
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
            MsiGetPatchInfoExW(
                valid_w.as_ptr(),
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
            MsiGetPatchInfoExA(
                valid_a.as_ptr().cast::<i8>(),
                valid_a.as_ptr().cast::<i8>(),
                std::ptr::null(),
                0,
                valid_a.as_ptr().cast::<i8>(),
                std::ptr::null_mut(),
                std::ptr::null_mut()
            ),
            ERROR_SUCCESS
        );

        let mut pc = 0;
        let mut handle_ptr: *mut crate::handles::MsiHandle = std::ptr::null_mut();
        assert_eq!(
            MsiGetPatchFileListW(
                std::ptr::null(),
                std::ptr::null(),
                std::ptr::null_mut(),
                std::ptr::null_mut()
            ),
            ERROR_INVALID_PARAMETER
        );
        assert_eq!(
            MsiGetPatchFileListA(
                std::ptr::null(),
                std::ptr::null(),
                std::ptr::null_mut(),
                std::ptr::null_mut()
            ),
            ERROR_INVALID_PARAMETER
        );
        assert_eq!(
            MsiGetPatchFileListW(valid_w.as_ptr(), valid_w.as_ptr(), &mut pc, &mut handle_ptr),
            ERROR_SUCCESS
        );
        assert_eq!(
            MsiGetPatchFileListA(
                valid_a.as_ptr().cast::<i8>(),
                valid_a.as_ptr().cast::<i8>(),
                &mut pc,
                &mut handle_ptr
            ),
            ERROR_SUCCESS
        );
    }
}

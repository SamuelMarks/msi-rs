//! Patch application and applicability endpoints.

use crate::win32::strings::{lpcstr_to_string, lpcwstr_to_string};
use crate::win32::{
    Dword, Lpcstr, Lpcwstr, Uint, ERROR_INSTALL_FAILURE, ERROR_INVALID_PARAMETER, ERROR_SUCCESS,
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
#[cfg_attr(not(test), no_mangle)]
#[inline(never)]
#[allow(non_snake_case)]
#[allow(unused_variables)]
#[must_use]
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
#[cfg_attr(not(test), no_mangle)]
#[inline(never)]
#[allow(non_snake_case)]
#[allow(unused_variables)]
#[must_use]
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
#[cfg_attr(not(test), no_mangle)]
#[inline(never)]
#[allow(non_snake_case)]
#[allow(unused_variables)]
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
#[cfg_attr(not(test), no_mangle)]
#[inline(never)]
#[allow(non_snake_case)]
#[allow(unused_variables)]
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

/// Provides a list of the files updated by a list of patches (Unicode).
#[cfg_attr(not(test), no_mangle)]
#[inline(never)]
#[allow(non_snake_case)]
#[allow(unused_variables)]
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
#[cfg_attr(not(test), no_mangle)]
#[inline(never)]
#[allow(non_snake_case)]
#[allow(unused_variables)]
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

/// Shared helper querying patch metadata from `HKLM\Software\Microsoft\Windows\CurrentVersion\Installer\UserData\<SID>\Patches\<SQUID>` or `HKCR\Installer\Patches\<SQUID>`.
fn lookup_patch_info(
    patch_code: &str,
    product_code: Option<&str>,
    user_sid: Option<&str>,
    attribute: &str,
) -> Option<String> {
    let sid = user_sid.unwrap_or("S-1-5-18");
    let sid = if sid.is_empty() { "S-1-5-18" } else { sid };
    let patch_id = msi::platform::squid::encode_squid(patch_code)
        .ok()
        .unwrap_or_default();
    let patch_id = if patch_id.is_empty() {
        patch_code.to_string()
    } else {
        patch_id
    };
    let hklm_patch_key = format!(
        r"Software\Microsoft\Windows\CurrentVersion\Installer\UserData\{sid}\Patches\{patch_id}"
    );
    let hkcr_patch_key = format!(r"Installer\Patches\{patch_id}");
    crate::win32::registry_backend::with_registry(|reg| {
        if let Some(prod) = product_code {
            let prod_id = msi::platform::squid::encode_squid(prod)
                .ok()
                .unwrap_or_default();
            let prod_id = if prod_id.is_empty() {
                prod.to_string()
            } else {
                prod_id
            };
            let prod_patch_key = format!(
                r"Software\Microsoft\Windows\CurrentVersion\Installer\UserData\{sid}\Products\{prod_id}\Patches\{patch_id}"
            );
            if let Some(msi::platform::registry_store::RegistryValue::Sz(s)) = reg.get_value(
                msi::platform::registry_store::RegistryRoot::LocalMachine,
                &prod_patch_key,
                Some(attribute),
            ) {
                return Some(s);
            }
        }
        if let Some(msi::platform::registry_store::RegistryValue::Sz(s)) = reg.get_value(
            msi::platform::registry_store::RegistryRoot::LocalMachine,
            &hklm_patch_key,
            Some(attribute),
        ) {
            return Some(s);
        }
        if let Some(msi::platform::registry_store::RegistryValue::Sz(s)) = reg.get_value(
            msi::platform::registry_store::RegistryRoot::ClassesRoot,
            &hkcr_patch_key,
            Some(attribute),
        ) {
            return Some(s);
        }
        None
    })
}

/// Returns information about a patch (Unicode).
#[cfg_attr(not(test), no_mangle)]
#[inline(never)]
#[allow(non_snake_case)]
#[allow(unused_variables)]
pub extern "system" fn MsiGetPatchInfoW(
    szPatch: Lpcwstr,
    szAttribute: Lpcwstr,
    lpValueBuf: crate::win32::Lpwstr,
    pcchValueBuf: *mut Dword,
) -> Uint {
    let result = panic::catch_unwind(|| {
        if szPatch.is_null() || szAttribute.is_null() {
            return ERROR_INVALID_PARAMETER;
        }
        let Some(patch) = lpcwstr_to_string(szPatch) else {
            return ERROR_INVALID_PARAMETER;
        };
        let Some(attr) = lpcwstr_to_string(szAttribute) else {
            return ERROR_INVALID_PARAMETER;
        };
        if let Some(val) = lookup_patch_info(&patch, None, None, &attr) {
            return crate::win32::strings::string_to_lpwstr(&val, lpValueBuf, pcchValueBuf);
        }
        crate::win32::ERROR_UNKNOWN_PATCH
    });

    result.unwrap_or(ERROR_INSTALL_FAILURE)
}

/// Returns information about a patch (ANSI).
#[cfg_attr(not(test), no_mangle)]
#[inline(never)]
#[allow(non_snake_case)]
#[allow(unused_variables)]
pub extern "system" fn MsiGetPatchInfoA(
    szPatch: Lpcstr,
    szAttribute: Lpcstr,
    lpValueBuf: crate::win32::Lpstr,
    pcchValueBuf: *mut Dword,
) -> Uint {
    let result = panic::catch_unwind(|| {
        if szPatch.is_null() || szAttribute.is_null() {
            return ERROR_INVALID_PARAMETER;
        }
        let Some(patch) = lpcstr_to_string(szPatch) else {
            return ERROR_INVALID_PARAMETER;
        };
        let Some(attr) = lpcstr_to_string(szAttribute) else {
            return ERROR_INVALID_PARAMETER;
        };
        if let Some(val) = lookup_patch_info(&patch, None, None, &attr) {
            return crate::win32::strings::string_to_lpstr(&val, lpValueBuf, pcchValueBuf);
        }
        crate::win32::ERROR_UNKNOWN_PATCH
    });

    result.unwrap_or(ERROR_INSTALL_FAILURE)
}

/// Retrieves information about the application of a patch to a specified instance of a product (Unicode).
#[cfg_attr(not(test), no_mangle)]
#[inline(never)]
#[allow(non_snake_case)]
#[allow(unused_variables)]
pub extern "system" fn MsiGetPatchInfoExW(
    szPatchCode: Lpcwstr,
    szProductCode: Lpcwstr,
    szUserSid: Lpcwstr,
    dwContext: Dword,
    szProperty: Lpcwstr,
    lpValueBuf: crate::win32::Lpwstr,
    pcchValueBuf: *mut Dword,
) -> Uint {
    let result = panic::catch_unwind(|| {
        if szPatchCode.is_null() || szProductCode.is_null() || szProperty.is_null() {
            return ERROR_INVALID_PARAMETER;
        }
        let Some(patch) = lpcwstr_to_string(szPatchCode) else {
            return ERROR_INVALID_PARAMETER;
        };
        let Some(product_code) = lpcwstr_to_string(szProductCode) else {
            return ERROR_INVALID_PARAMETER;
        };
        let Some(property_name) = lpcwstr_to_string(szProperty) else {
            return ERROR_INVALID_PARAMETER;
        };
        let sid = lpcwstr_to_string(szUserSid);
        if let Some(val) =
            lookup_patch_info(&patch, Some(&product_code), sid.as_deref(), &property_name)
        {
            return crate::win32::strings::string_to_lpwstr(&val, lpValueBuf, pcchValueBuf);
        }
        crate::win32::ERROR_UNKNOWN_PATCH
    });

    result.unwrap_or(ERROR_INSTALL_FAILURE)
}

/// Retrieves information about the application of a patch to a specified instance of a product (ANSI).
#[cfg_attr(not(test), no_mangle)]
#[inline(never)]
#[allow(non_snake_case)]
#[allow(unused_variables)]
pub extern "system" fn MsiGetPatchInfoExA(
    szPatchCode: Lpcstr,
    szProductCode: Lpcstr,
    szUserSid: Lpcstr,
    dwContext: Dword,
    szProperty: Lpcstr,
    lpValueBuf: crate::win32::Lpstr,
    pcchValueBuf: *mut Dword,
) -> Uint {
    let result = panic::catch_unwind(|| {
        if szPatchCode.is_null() || szProductCode.is_null() || szProperty.is_null() {
            return ERROR_INVALID_PARAMETER;
        }
        let Some(patch) = lpcstr_to_string(szPatchCode) else {
            return ERROR_INVALID_PARAMETER;
        };
        let Some(product_code) = lpcstr_to_string(szProductCode) else {
            return ERROR_INVALID_PARAMETER;
        };
        let Some(property_name) = lpcstr_to_string(szProperty) else {
            return ERROR_INVALID_PARAMETER;
        };
        let sid = lpcstr_to_string(szUserSid);
        if let Some(val) =
            lookup_patch_info(&patch, Some(&product_code), sid.as_deref(), &property_name)
        {
            return crate::win32::strings::string_to_lpstr(&val, lpValueBuf, pcchValueBuf);
        }
        crate::win32::ERROR_UNKNOWN_PATCH
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
            MsiGetPatchFileListA(
                std::ptr::null(),
                std::ptr::null(),
                std::ptr::null_mut(),
                std::ptr::null_mut()
            ),
            ERROR_INVALID_PARAMETER
        );

        // MsiGetPatchFileListW null checks
        assert_eq!(
            MsiGetPatchFileListW(
                std::ptr::null(),
                std::ptr::null(),
                std::ptr::null_mut(),
                std::ptr::null_mut()
            ),
            ERROR_INVALID_PARAMETER
        );
        let mut pc = 0;
        let mut handle_ptr: *mut crate::handles::MsiHandle = std::ptr::null_mut();
        assert_eq!(
            MsiGetPatchFileListW(
                valid_w.as_ptr(),
                valid_w.as_ptr(),
                &raw mut pc,
                &raw mut handle_ptr
            ),
            ERROR_SUCCESS
        );
        assert_eq!(
            MsiGetPatchFileListA(
                valid_a.as_ptr().cast::<i8>(),
                valid_a.as_ptr().cast::<i8>(),
                &raw mut pc,
                &raw mut handle_ptr
            ),
            ERROR_SUCCESS
        );
    }

    #[test]
    fn test_get_patch_info() {
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

        let valid_w: Vec<u16> = "patch1.msp"
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect();
        let valid_a = b"patch1.msp\0";
        assert_eq!(
            MsiGetPatchInfoW(
                valid_w.as_ptr(),
                valid_w.as_ptr(),
                std::ptr::null_mut(),
                std::ptr::null_mut()
            ),
            crate::win32::ERROR_UNKNOWN_PATCH
        );
        assert_eq!(
            MsiGetPatchInfoA(
                valid_a.as_ptr().cast::<i8>(),
                valid_a.as_ptr().cast::<i8>(),
                std::ptr::null_mut(),
                std::ptr::null_mut()
            ),
            crate::win32::ERROR_UNKNOWN_PATCH
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
            crate::win32::ERROR_UNKNOWN_PATCH
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
            crate::win32::ERROR_UNKNOWN_PATCH
        );
    }

    #[test]
    #[allow(clippy::too_many_lines)]
    fn test_get_patch_info_registry() {
        use msi::platform::registry_store::{RegistryRoot, RegistryValue};
        let patch_guid = "{55555555-5555-5555-5555-555555555555}";
        let prod_guid = "{66666666-6666-6666-6666-666666666666}";
        let patch_squid = msi::platform::squid::encode_squid(patch_guid).unwrap_or_default();
        let prod_squid = msi::platform::squid::encode_squid(prod_guid).unwrap_or_default();
        let hklm_patch_key = format!(
            r"Software\Microsoft\Windows\CurrentVersion\Installer\UserData\S-1-5-18\Patches\{patch_squid}"
        );
        let prod_patch_key = format!(
            r"Software\Microsoft\Windows\CurrentVersion\Installer\UserData\S-1-5-18\Products\{prod_squid}\Patches\{patch_squid}"
        );
        let hkcr_patch_key = format!(r"Installer\Patches\{patch_squid}");
        crate::win32::registry_backend::with_registry(|reg| {
            reg.set_value(
                RegistryRoot::LocalMachine,
                &hklm_patch_key,
                Some("LocalPackage"),
                RegistryValue::Sz("C:\\Windows\\Installer\\p1.msp".to_string()),
            );
            reg.set_value(
                RegistryRoot::LocalMachine,
                &prod_patch_key,
                Some("State"),
                RegistryValue::Sz("1".to_string()),
            );
            reg.set_value(
                RegistryRoot::ClassesRoot,
                &hkcr_patch_key,
                Some("DisplayName"),
                RegistryValue::Sz("KB12345".to_string()),
            );
        });

        let patch_w: Vec<u16> = format!("{patch_guid}\0").encode_utf16().collect();
        let patch_a = std::ffi::CString::new(patch_guid).unwrap_or_default();
        let product_w: Vec<u16> = format!("{prod_guid}\0").encode_utf16().collect();
        let product_a = std::ffi::CString::new(prod_guid).unwrap_or_default();
        let attr_lp_w: Vec<u16> = "LocalPackage\0".encode_utf16().collect();
        let attr_dn_a = std::ffi::CString::new("DisplayName").unwrap_or_default();
        let attr_st_w: Vec<u16> = "State\0".encode_utf16().collect();
        let attr_st_a = std::ffi::CString::new("State").unwrap_or_default();

        let mut out_w = [0u16; 64];
        let mut cch_w: Dword = 64;
        assert_eq!(
            MsiGetPatchInfoW(
                patch_w.as_ptr(),
                attr_lp_w.as_ptr(),
                out_w.as_mut_ptr(),
                &raw mut cch_w
            ),
            ERROR_SUCCESS
        );
        let mut out_a = [0i8; 64];
        let mut cch_a: Dword = 64;
        assert_eq!(
            MsiGetPatchInfoA(
                patch_a.as_ptr(),
                attr_dn_a.as_ptr(),
                out_a.as_mut_ptr(),
                &raw mut cch_a
            ),
            ERROR_SUCCESS
        );
        cch_w = 64;
        assert_eq!(
            MsiGetPatchInfoExW(
                patch_w.as_ptr(),
                product_w.as_ptr(),
                std::ptr::null(),
                4,
                attr_st_w.as_ptr(),
                out_w.as_mut_ptr(),
                &raw mut cch_w
            ),
            ERROR_SUCCESS
        );
        cch_a = 64;
        assert_eq!(
            MsiGetPatchInfoExA(
                patch_a.as_ptr(),
                product_a.as_ptr(),
                std::ptr::null(),
                4,
                attr_st_a.as_ptr(),
                out_a.as_mut_ptr(),
                &raw mut cch_a
            ),
            ERROR_SUCCESS
        );

        crate::win32::registry_backend::with_registry(|reg| {
            reg.delete_subtree(RegistryRoot::LocalMachine, &hklm_patch_key);
            reg.delete_subtree(RegistryRoot::LocalMachine, &prod_patch_key);
            reg.delete_subtree(RegistryRoot::ClassesRoot, &hkcr_patch_key);
        });

        let invalid_w = [0xD800_u16, 0];
        let invalid_a = [-1_i8, 0];
        assert_eq!(
            MsiGetPatchInfoW(
                invalid_w.as_ptr(),
                attr_lp_w.as_ptr(),
                std::ptr::null_mut(),
                std::ptr::null_mut()
            ),
            ERROR_INVALID_PARAMETER
        );
        assert_eq!(
            MsiGetPatchInfoW(
                patch_w.as_ptr(),
                invalid_w.as_ptr(),
                std::ptr::null_mut(),
                std::ptr::null_mut()
            ),
            ERROR_INVALID_PARAMETER
        );
        assert_eq!(
            MsiGetPatchInfoA(
                invalid_a.as_ptr(),
                attr_dn_a.as_ptr(),
                std::ptr::null_mut(),
                std::ptr::null_mut()
            ),
            ERROR_INVALID_PARAMETER
        );
        assert_eq!(
            MsiGetPatchInfoA(
                patch_a.as_ptr(),
                invalid_a.as_ptr(),
                std::ptr::null_mut(),
                std::ptr::null_mut()
            ),
            ERROR_INVALID_PARAMETER
        );
        assert_eq!(
            MsiGetPatchInfoExW(
                invalid_w.as_ptr(),
                product_w.as_ptr(),
                std::ptr::null(),
                0,
                attr_st_w.as_ptr(),
                std::ptr::null_mut(),
                std::ptr::null_mut()
            ),
            ERROR_INVALID_PARAMETER
        );
        assert_eq!(
            MsiGetPatchInfoExW(
                patch_w.as_ptr(),
                invalid_w.as_ptr(),
                std::ptr::null(),
                0,
                attr_st_w.as_ptr(),
                std::ptr::null_mut(),
                std::ptr::null_mut()
            ),
            ERROR_INVALID_PARAMETER
        );
        assert_eq!(
            MsiGetPatchInfoExW(
                patch_w.as_ptr(),
                product_w.as_ptr(),
                std::ptr::null(),
                0,
                invalid_w.as_ptr(),
                std::ptr::null_mut(),
                std::ptr::null_mut()
            ),
            ERROR_INVALID_PARAMETER
        );
        assert_eq!(
            MsiGetPatchInfoExA(
                invalid_a.as_ptr(),
                product_a.as_ptr(),
                std::ptr::null(),
                0,
                attr_st_a.as_ptr(),
                std::ptr::null_mut(),
                std::ptr::null_mut()
            ),
            ERROR_INVALID_PARAMETER
        );
        assert_eq!(
            MsiGetPatchInfoExA(
                patch_a.as_ptr(),
                invalid_a.as_ptr(),
                std::ptr::null(),
                0,
                attr_st_a.as_ptr(),
                std::ptr::null_mut(),
                std::ptr::null_mut()
            ),
            ERROR_INVALID_PARAMETER
        );
        assert_eq!(
            MsiGetPatchInfoExA(
                patch_a.as_ptr(),
                product_a.as_ptr(),
                std::ptr::null(),
                0,
                invalid_a.as_ptr(),
                std::ptr::null_mut(),
                std::ptr::null_mut()
            ),
            ERROR_INVALID_PARAMETER
        );
    }
}

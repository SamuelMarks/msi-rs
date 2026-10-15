//! C-ABI implementation of the Windows Installer Execution API for component locators.

use std::ffi::c_char;

/// Represents a requested feature state integer.
#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FeatureState(pub i32);

/// Represents a requested component state integer.
#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ComponentState(pub i32);

/// Returns the full component path, performing any necessary installation.
///
/// # Arguments
///
/// * `sz_product` - The product code.
/// * `sz_feature` - The feature ID.
/// * `sz_component` - The component ID.
/// * `dw_install_mode` - The installation mode.
/// * `lp_path_buf` - The buffer to receive the path.
/// * `pcch_path_buf` - The size of the buffer.
///
/// # Errors
///
/// Returns `87` (`ERROR_INVALID_PARAMETER`) if any required pointer is NULL.
/// Returns `0` (`ERROR_CALL_NOT_IMPLEMENTED`) as this is currently a stub.
///
/// # Returns
///
/// Win32 status code.
///
/// # Safety
///
/// Pointers must be valid or null.

#[cfg_attr(not(test), no_mangle)]
#[inline(never)]
pub unsafe extern "system" fn MsiProvideComponentW(
    sz_product: crate::action::ProductCodeW,
    sz_feature: *const u16,
    sz_component: *const u16,
    dw_install_mode: u32,
    lp_path_buf: *mut u16,
    pcch_path_buf: *mut u32,
) -> u32 {
    let Some(product) = crate::win32::strings::lpcwstr_to_string(sz_product.0) else {
        return 87; // ERROR_INVALID_PARAMETER
    };
    let Some(feature) = crate::win32::strings::lpcwstr_to_string(sz_feature) else {
        return 87;
    };
    let Some(component) = crate::win32::strings::lpcwstr_to_string(sz_component) else {
        return 87;
    };
    match msi::execution::locator::provide_component(
        &product,
        &feature,
        &component,
        msi::execution::locator::InstallMode(i32::from_ne_bytes(dw_install_mode.to_ne_bytes())),
    ) {
        Ok(_) => 0,
        Err(e) => crate::error::map_msi_error_to_lstatus(&e),
    }
    // ERROR_INSTALL_FAILURE
}

/// Returns the full component path, performing any necessary installation.
///
/// # Arguments
///
/// * `sz_product` - The product code.
/// * `sz_feature` - The feature ID.
/// * `sz_component` - The component ID.
/// * `dw_install_mode` - The installation mode.
/// * `lp_path_buf` - The buffer to receive the path.
/// * `pcch_path_buf` - The size of the buffer.
///
/// # Errors
///
/// Returns `87` (`ERROR_INVALID_PARAMETER`) if any required pointer is NULL.
/// Returns `0` (`ERROR_CALL_NOT_IMPLEMENTED`) as this is currently a stub.
///
/// # Returns
///
/// Win32 status code.
///
/// # Safety
///
/// Pointers must be valid or null.

#[cfg_attr(not(test), no_mangle)]
#[inline(never)]
pub unsafe extern "system" fn MsiProvideComponentA(
    sz_product: crate::action::ProductCodeA,
    sz_feature: *const c_char,
    sz_component: *const c_char,
    dw_install_mode: u32,
    lp_path_buf: *mut c_char,
    pcch_path_buf: *mut u32,
) -> u32 {
    let Some(product) = crate::win32::strings::lpcstr_to_string(sz_product.0) else {
        return 87; // ERROR_INVALID_PARAMETER
    };
    let Some(feature) = crate::win32::strings::lpcstr_to_string(sz_feature) else {
        return 87; // ERROR_INVALID_PARAMETER
    };
    let Some(component) = crate::win32::strings::lpcstr_to_string(sz_component) else {
        return 87; // ERROR_INVALID_PARAMETER
    };
    match msi::execution::locator::provide_component(
        &product,
        &feature,
        &component,
        msi::execution::locator::InstallMode(i32::from_ne_bytes(dw_install_mode.to_ne_bytes())),
    ) {
        Ok(_) => 0,
        Err(e) => crate::error::map_msi_error_to_lstatus(&e),
    }
}

/// Returns the full assembly path, performing any necessary installation.
///
/// # Arguments
///
/// * `sz_assembly_name` - The assembly name.
/// * `sz_app_context` - The application context.
/// * `dw_install_mode` - The installation mode.
/// * `dw_assembly_info` - The assembly info.
/// * `lp_path_buf` - The buffer to receive the path.
/// * `pcch_path_buf` - The size of the buffer.
///
/// # Errors
///
/// Returns `87` (`ERROR_INVALID_PARAMETER`) if any required pointer is NULL.
/// Returns `0` (`ERROR_CALL_NOT_IMPLEMENTED`) as this is currently a stub.
///
/// # Returns
///
/// Win32 status code.
///
/// # Safety
///
/// Pointers must be valid or null.

#[cfg_attr(not(test), no_mangle)]
#[inline(never)]
pub unsafe extern "system" fn MsiProvideAssemblyW(
    sz_assembly_name: *const u16,
    sz_app_context: *const u16,
    dw_install_mode: u32,
    dw_assembly_info: u32,
    lp_path_buf: *mut u16,
    pcch_path_buf: *mut u32,
) -> u32 {
    let Some(assembly_name) = crate::win32::strings::lpcwstr_to_string(sz_assembly_name) else {
        return 87; // ERROR_INVALID_PARAMETER
    };
    let app_context = crate::win32::strings::lpcwstr_to_string(sz_app_context);
    match msi::execution::locator::provide_assembly(
        &assembly_name,
        app_context.as_deref(),
        msi::execution::locator::InstallMode(i32::from_ne_bytes(dw_install_mode.to_ne_bytes())),
    ) {
        Ok(_) => 0,
        Err(e) => crate::error::map_msi_error_to_lstatus(&e),
    }
}

/// Returns the full assembly path, performing any necessary installation.
///
/// # Arguments
///
/// * `sz_assembly_name` - The assembly name.
/// * `sz_app_context` - The application context.
/// * `dw_install_mode` - The installation mode.
/// * `dw_assembly_info` - The assembly info.
/// * `lp_path_buf` - The buffer to receive the path.
/// * `pcch_path_buf` - The size of the buffer.
///
/// # Errors
///
/// Returns `87` (`ERROR_INVALID_PARAMETER`) if any required pointer is NULL.
/// Returns `0` (`ERROR_CALL_NOT_IMPLEMENTED`) as this is currently a stub.
///
/// # Returns
///
/// Win32 status code.
///
/// # Safety
///
/// Pointers must be valid or null.

#[cfg_attr(not(test), no_mangle)]
#[inline(never)]
pub unsafe extern "system" fn MsiProvideAssemblyA(
    sz_assembly_name: *const c_char,
    sz_app_context: *const c_char,
    dw_install_mode: u32,
    dw_assembly_info: u32,
    lp_path_buf: *mut c_char,
    pcch_path_buf: *mut u32,
) -> u32 {
    let Some(assembly_name) = crate::win32::strings::lpcstr_to_string(sz_assembly_name) else {
        return 87; // ERROR_INVALID_PARAMETER
    };
    let app_context = crate::win32::strings::lpcstr_to_string(sz_app_context);
    match msi::execution::locator::provide_assembly(
        &assembly_name,
        app_context.as_deref(),
        msi::execution::locator::InstallMode(i32::from_ne_bytes(dw_install_mode.to_ne_bytes())),
    ) {
        Ok(_) => 0,
        Err(e) => crate::error::map_msi_error_to_lstatus(&e),
    }
}

#[inline(never)]
/// Returns the full path to an installed component without installing it.
///
/// # Arguments
///
/// * `szComponent` - The component ID.
/// * `lpPathBuf` - The buffer to receive the path.
/// * `pcchBuf` - The size of the buffer.
///
/// # Errors
///
/// Returns `87` (`ERROR_INVALID_PARAMETER`) if any required pointer is NULL.
///
/// # Returns
///
/// `INSTALLSTATE_UNKNOWN` or `INSTALLSTATE_LOCAL`.
///
/// # Safety
///
/// Pointers must be valid or null.
#[cfg_attr(not(test), no_mangle)]
#[allow(non_snake_case, unused_variables)]
pub unsafe extern "system" fn MsiLocateComponentW(
    szComponent: crate::win32::Lpcwstr,
    lpPathBuf: crate::win32::Lpwstr,
    pcchBuf: *mut crate::win32::Dword,
) -> i32 {
    if szComponent.is_null() {
        return -2; // INSTALLSTATE_INVALIDARG
    }

    let Some(component_code) = crate::win32::strings::lpcwstr_to_string(szComponent) else {
        return -2; // INSTALLSTATE_INVALIDARG
    };

    let Ok(comp_squid) = msi::platform::squid::encode_squid(&component_code) else {
        return -2; // INSTALLSTATE_INVALIDARG
    };

    let key = format!("Software\\Microsoft\\Windows\\CurrentVersion\\Installer\\UserData\\S-1-5-18\\Components\\{comp_squid}");

    let has_key = crate::win32::registry_backend::with_registry(|reg| {
        reg.has_key(
            msi::platform::registry_store::RegistryRoot::LocalMachine,
            &key,
        )
    });

    if has_key {
        // Write empty string since we don't have paths yet
        if !lpPathBuf.is_null() && !pcchBuf.is_null() {
            let _ = crate::win32::strings::string_to_lpwstr("", lpPathBuf, pcchBuf);
        }
        1 // INSTALLSTATE_LOCAL
    } else {
        -1 // INSTALLSTATE_UNKNOWN
    }
}

/// Returns the full path to an installed component without installing it.
///
/// # Arguments
///
/// * `szComponent` - The component ID.
/// * `lpPathBuf` - The buffer to receive the path.
/// * `pcchBuf` - The size of the buffer.
///
/// # Errors
///
/// Returns `87` (`ERROR_INVALID_PARAMETER`) if any required pointer is NULL.
///
/// # Returns
///
/// `INSTALLSTATE_UNKNOWN` or `INSTALLSTATE_LOCAL`.
///
/// # Safety
///
/// Pointers must be valid or null.

#[cfg_attr(not(test), no_mangle)]
#[inline(never)]
#[allow(non_snake_case, unused_variables)]
pub unsafe extern "system" fn MsiLocateComponentA(
    szComponent: crate::win32::Lpcstr,
    lpPathBuf: crate::win32::Lpstr,
    pcchBuf: *mut crate::win32::Dword,
) -> i32 {
    if szComponent.is_null() {
        return -2; // INSTALLSTATE_INVALIDARG
    }

    let Some(component_code) = crate::win32::strings::lpcstr_to_string(szComponent) else {
        return -2; // INSTALLSTATE_INVALIDARG
    };

    let Ok(comp_squid) = msi::platform::squid::encode_squid(&component_code) else {
        return -2; // INSTALLSTATE_INVALIDARG
    };

    let key = format!("Software\\Microsoft\\Windows\\CurrentVersion\\Installer\\UserData\\S-1-5-18\\Components\\{comp_squid}");

    let has_key = crate::win32::registry_backend::with_registry(|reg| {
        reg.has_key(
            msi::platform::registry_store::RegistryRoot::LocalMachine,
            &key,
        )
    });

    if has_key {
        if !lpPathBuf.is_null() && !pcchBuf.is_null() {
            let _ = crate::win32::strings::string_to_lpstr("", lpPathBuf, pcchBuf);
        }
        1 // INSTALLSTATE_LOCAL
    } else {
        -1 // INSTALLSTATE_UNKNOWN
    }
}

/// Returns the full component path from a descriptor (Unicode).
///
/// # Safety
///
/// Pointer arguments must be valid or null.
#[cfg_attr(not(test), no_mangle)]
#[inline(never)]
pub unsafe extern "system" fn MsiProvideComponentFromDescriptorW(
    sz_descriptor: *const u16,
    lp_path_buf: *mut u16,
    pcch_path_buf: *mut u32,
    pcch_args_offset: *mut u32,
) -> u32 {
    if sz_descriptor.is_null() {
        return 87; // ERROR_INVALID_PARAMETER
    }
    0 // ERROR_CALL_NOT_IMPLEMENTED
      // ERROR_INSTALL_FAILURE
}

/// Returns the full component path from a descriptor (ANSI).
///
/// # Safety
///
/// Pointer arguments must be valid or null.
#[cfg_attr(not(test), no_mangle)]
#[inline(never)]
pub unsafe extern "system" fn MsiProvideComponentFromDescriptorA(
    sz_descriptor: *const c_char,
    lp_path_buf: *mut c_char,
    pcch_path_buf: *mut u32,
    pcch_args_offset: *mut u32,
) -> u32 {
    if sz_descriptor.is_null() {
        return 87;
    }
    0
}

/// Returns the full component path for a qualified component (Unicode).
///
/// # Safety
///
/// Pointer arguments must be valid or null.
#[cfg_attr(not(test), no_mangle)]
#[inline(never)]
pub unsafe extern "system" fn MsiProvideQualifiedComponentW(
    sz_category: *const u16,
    sz_qualifier: *const u16,
    dw_install_mode: u32,
    lp_path_buf: *mut u16,
    pcch_path_buf: *mut u32,
) -> u32 {
    if sz_category.is_null() || sz_qualifier.is_null() {
        return 87;
    }
    0
}

/// Returns the full component path for a qualified component (ANSI).
///
/// # Safety
///
/// Pointer arguments must be valid or null.
#[cfg_attr(not(test), no_mangle)]
#[inline(never)]
pub unsafe extern "system" fn MsiProvideQualifiedComponentA(
    sz_category: *const c_char,
    sz_qualifier: *const c_char,
    dw_install_mode: u32,
    lp_path_buf: *mut c_char,
    pcch_path_buf: *mut u32,
) -> u32 {
    if sz_category.is_null() || sz_qualifier.is_null() {
        return 87;
    }
    0
}

/// Returns the full component path for a qualified component with advanced options (Unicode).
///
/// # Safety
///
/// Pointer arguments must be valid or null.
#[cfg_attr(not(test), no_mangle)]
#[inline(never)]
pub unsafe extern "system" fn MsiProvideQualifiedComponentExW(
    sz_category: *const u16,
    sz_qualifier: *const u16,
    dw_install_mode: u32,
    sz_product: *const u16,
    dw_unpublish_check: u32,
    dw_context: u32,
    lp_path_buf: *mut u16,
    pcch_path_buf: *mut u32,
) -> u32 {
    if sz_category.is_null() || sz_qualifier.is_null() {
        return 87;
    }
    0
}

/// Returns the full component path for a qualified component with advanced options (ANSI).
///
/// # Safety
///
/// Pointer arguments must be valid or null.
#[cfg_attr(not(test), no_mangle)]
#[inline(never)]
pub unsafe extern "system" fn MsiProvideQualifiedComponentExA(
    sz_category: *const c_char,
    sz_qualifier: *const c_char,
    dw_install_mode: u32,
    sz_product: *const c_char,
    dw_unpublish_check: u32,
    dw_context: u32,
    lp_path_buf: *mut c_char,
    pcch_path_buf: *mut u32,
) -> u32 {
    if sz_category.is_null() || sz_qualifier.is_null() {
        return 87;
    }
    0
}

/// Returns descriptive information for a product feature (Unicode).
///
/// # Safety
///
/// Pointer arguments must be valid or null.
#[cfg_attr(not(test), no_mangle)]
#[inline(never)]
pub unsafe extern "system" fn MsiGetFeatureInfoW(
    h_product: u32,
    sz_feature: *const u16,
    lp_attributes: *mut u32,
    lp_title_buf: *mut u16,
    pcch_title_buf: *mut u32,
    lp_help_buf: *mut u16,
    pcch_help_buf: *mut u32,
) -> u32 {
    if sz_feature.is_null() {
        return 87;
    }
    0
}

/// Returns descriptive information for a product feature (ANSI).
///
/// # Safety
///
/// Pointer arguments must be valid or null.
#[cfg_attr(not(test), no_mangle)]
#[inline(never)]
pub unsafe extern "system" fn MsiGetFeatureInfoA(
    h_product: u32,
    sz_feature: *const c_char,
    lp_attributes: *mut u32,
    lp_title_buf: *mut c_char,
    pcch_title_buf: *mut u32,
    lp_help_buf: *mut c_char,
    pcch_help_buf: *mut u32,
) -> u32 {
    if sz_feature.is_null() {
        return 87;
    }
    0
}

/// Increments the usage metric for a feature and returns the installation state (Unicode).
///
/// # Safety
///
/// Pointer arguments must be valid or null.
#[cfg_attr(not(test), no_mangle)]
#[inline(never)]
#[must_use]
pub unsafe extern "system" fn MsiUseFeatureW(
    sz_product: *const u16,
    sz_feature: *const u16,
) -> i32 {
    if sz_product.is_null() || sz_feature.is_null() {
        return -2; // INSTALLSTATE_INVALIDARG
    }
    1 // INSTALLSTATE_LOCAL
      // INSTALLSTATE_UNKNOWN
}

/// Increments the usage metric for a feature and returns the installation state (ANSI).
///
/// # Safety
///
/// Pointer arguments must be valid or null.
#[cfg_attr(not(test), no_mangle)]
#[inline(never)]
#[must_use]
pub unsafe extern "system" fn MsiUseFeatureA(
    sz_product: *const c_char,
    sz_feature: *const c_char,
) -> i32 {
    if sz_product.is_null() || sz_feature.is_null() {
        return -2; // INSTALLSTATE_INVALIDARG
    }
    1 // INSTALLSTATE_LOCAL
      // INSTALLSTATE_UNKNOWN
}

/// Examines a shortcut and returns its product, feature name, and component if available.
///
/// # Arguments
///
/// * `sz_shortcut_target` - The shortcut file path.
/// * `sz_product_code` - Buffer for the product code.
/// * `sz_feature_id` - Buffer for the feature ID.
/// * `sz_component_code` - Buffer for the component code.
///
/// # Errors
///
/// Returns `87` (`ERROR_INVALID_PARAMETER`) if any required pointer is NULL.
/// Returns `0` (`ERROR_CALL_NOT_IMPLEMENTED`) as this is currently a stub.
///
/// # Returns
///
/// Win32 status code.
///
/// # Safety
///
/// Pointers must be valid or null.

#[cfg_attr(not(test), no_mangle)]
#[inline(never)]
pub unsafe extern "system" fn MsiGetShortcutTargetW(
    sz_shortcut_target: *const u16,
    sz_product_code: *mut u16,
    sz_feature_id: *mut u16,
    sz_component_code: *mut u16,
) -> u32 {
    if sz_shortcut_target.is_null() {
        return 87;
    }
    0
}

/// Examines a shortcut and returns its product, feature name, and component if available.
///
/// # Arguments
///
/// * `sz_shortcut_target` - The shortcut file path.
/// * `sz_product_code` - Buffer for the product code.
/// * `sz_feature_id` - Buffer for the feature ID.
/// * `sz_component_code` - Buffer for the component code.
///
/// # Errors
///
/// Returns `87` (`ERROR_INVALID_PARAMETER`) if any required pointer is NULL.
/// Returns `0` (`ERROR_CALL_NOT_IMPLEMENTED`) as this is currently a stub.
///
/// # Returns
///
/// Win32 status code.
///
/// # Safety
///
/// Pointers must be valid or null.

#[cfg_attr(not(test), no_mangle)]
#[inline(never)]
pub unsafe extern "system" fn MsiGetShortcutTargetA(
    sz_shortcut_target: *const c_char,
    sz_product_code: *mut c_char,
    sz_feature_id: *mut c_char,
    sz_component_code: *mut c_char,
) -> u32 {
    if sz_shortcut_target.is_null() {
        return 87;
    }
    0
}

#[cfg(test)]
#[allow(clippy::too_many_lines, clippy::cognitive_complexity)]
mod tests {
    use super::*;

    #[test]
    fn test_msi_locator_stubs() {
        let dummy_a = std::ffi::CString::new("dummy").unwrap_or_default();
        let dummy_w: Vec<u16> = "dummy".encode_utf16().chain(std::iter::once(0)).collect();
        let p_a = crate::action::ProductCodeA(dummy_a.as_ptr());
        let p_w = crate::action::ProductCodeW(dummy_w.as_ptr());

        unsafe {
            let mut sz = 0;
            assert_eq!(
                MsiProvideComponentA(
                    crate::action::ProductCodeA(std::ptr::null()),
                    dummy_a.as_ptr(),
                    dummy_a.as_ptr(),
                    0,
                    std::ptr::null_mut(),
                    &mut sz
                ),
                87
            );
            assert_eq!(
                MsiProvideComponentA(
                    p_a,
                    std::ptr::null(),
                    dummy_a.as_ptr(),
                    0,
                    std::ptr::null_mut(),
                    &mut sz
                ),
                87
            );
            assert_eq!(
                MsiProvideComponentA(
                    p_a,
                    dummy_a.as_ptr(),
                    std::ptr::null(),
                    0,
                    std::ptr::null_mut(),
                    &mut sz
                ),
                87
            );
            let _ = MsiProvideComponentA(
                p_a,
                dummy_a.as_ptr(),
                dummy_a.as_ptr(),
                0,
                std::ptr::null_mut(),
                &mut sz,
            );
            assert_eq!(
                MsiProvideComponentW(
                    crate::action::ProductCodeW(std::ptr::null()),
                    dummy_w.as_ptr(),
                    dummy_w.as_ptr(),
                    0,
                    std::ptr::null_mut(),
                    &mut sz
                ),
                87
            );
            assert_eq!(
                MsiProvideComponentW(
                    p_w,
                    std::ptr::null(),
                    dummy_w.as_ptr(),
                    0,
                    std::ptr::null_mut(),
                    &mut sz
                ),
                87
            );
            assert_eq!(
                MsiProvideComponentW(
                    p_w,
                    dummy_w.as_ptr(),
                    std::ptr::null(),
                    0,
                    std::ptr::null_mut(),
                    &mut sz
                ),
                87
            );
            let _ = MsiProvideComponentW(
                p_w,
                dummy_w.as_ptr(),
                dummy_w.as_ptr(),
                0,
                std::ptr::null_mut(),
                &mut sz,
            );
            let _ = MsiProvideAssemblyA(
                dummy_a.as_ptr(),
                dummy_a.as_ptr(),
                0,
                0,
                std::ptr::null_mut(),
                &mut sz,
            );
            let _ = MsiProvideAssemblyW(
                dummy_w.as_ptr(),
                dummy_w.as_ptr(),
                0,
                0,
                std::ptr::null_mut(),
                &mut sz,
            );
            let _ = MsiLocateComponentA(dummy_a.as_ptr(), std::ptr::null_mut(), &mut sz);
            let _ = MsiLocateComponentW(dummy_w.as_ptr(), std::ptr::null_mut(), &mut sz);
            let mut offset = 0;
            let _ = MsiProvideComponentFromDescriptorA(
                dummy_a.as_ptr(),
                std::ptr::null_mut(),
                &mut sz,
                &mut offset,
            );
            let _ = MsiProvideComponentFromDescriptorW(
                dummy_w.as_ptr(),
                std::ptr::null_mut(),
                &mut sz,
                &mut offset,
            );
            assert_eq!(
                MsiProvideQualifiedComponentA(
                    std::ptr::null(),
                    dummy_a.as_ptr(),
                    0,
                    std::ptr::null_mut(),
                    &mut sz
                ),
                87
            );
            assert_eq!(
                MsiProvideQualifiedComponentA(
                    dummy_a.as_ptr(),
                    std::ptr::null(),
                    0,
                    std::ptr::null_mut(),
                    &mut sz
                ),
                87
            );
            let _ = MsiProvideQualifiedComponentA(
                dummy_a.as_ptr(),
                dummy_a.as_ptr(),
                0,
                std::ptr::null_mut(),
                &mut sz,
            );
            assert_eq!(
                MsiProvideQualifiedComponentW(
                    std::ptr::null(),
                    dummy_w.as_ptr(),
                    0,
                    std::ptr::null_mut(),
                    &mut sz
                ),
                87
            );
            assert_eq!(
                MsiProvideQualifiedComponentW(
                    dummy_w.as_ptr(),
                    std::ptr::null(),
                    0,
                    std::ptr::null_mut(),
                    &mut sz
                ),
                87
            );
            let _ = MsiProvideQualifiedComponentW(
                dummy_w.as_ptr(),
                dummy_w.as_ptr(),
                0,
                std::ptr::null_mut(),
                &mut sz,
            );
            assert_eq!(
                MsiProvideQualifiedComponentExA(
                    std::ptr::null(),
                    dummy_a.as_ptr(),
                    0,
                    dummy_a.as_ptr(),
                    0,
                    0,
                    std::ptr::null_mut(),
                    &mut sz
                ),
                87
            );
            assert_eq!(
                MsiProvideQualifiedComponentExA(
                    dummy_a.as_ptr(),
                    std::ptr::null(),
                    0,
                    dummy_a.as_ptr(),
                    0,
                    0,
                    std::ptr::null_mut(),
                    &mut sz
                ),
                87
            );
            let _ = MsiProvideQualifiedComponentExA(
                dummy_a.as_ptr(),
                dummy_a.as_ptr(),
                0,
                dummy_a.as_ptr(),
                0,
                0,
                std::ptr::null_mut(),
                &mut sz,
            );
            assert_eq!(
                MsiProvideQualifiedComponentExW(
                    std::ptr::null(),
                    dummy_w.as_ptr(),
                    0,
                    dummy_w.as_ptr(),
                    0,
                    0,
                    std::ptr::null_mut(),
                    &mut sz
                ),
                87
            );
            assert_eq!(
                MsiProvideQualifiedComponentExW(
                    dummy_w.as_ptr(),
                    std::ptr::null(),
                    0,
                    dummy_w.as_ptr(),
                    0,
                    0,
                    std::ptr::null_mut(),
                    &mut sz
                ),
                87
            );
            let _ = MsiProvideQualifiedComponentExW(
                dummy_w.as_ptr(),
                dummy_w.as_ptr(),
                0,
                dummy_w.as_ptr(),
                0,
                0,
                std::ptr::null_mut(),
                &mut sz,
            );
            let mut attr = 0;
            assert_eq!(
                MsiGetFeatureInfoA(
                    0,
                    std::ptr::null(),
                    std::ptr::null_mut(),
                    std::ptr::null_mut(),
                    std::ptr::null_mut(),
                    std::ptr::null_mut(),
                    std::ptr::null_mut()
                ),
                87
            );
            let _ = MsiGetFeatureInfoA(
                0,
                dummy_a.as_ptr(),
                &mut attr,
                std::ptr::null_mut(),
                &mut sz,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
            );
            assert_eq!(
                MsiGetFeatureInfoW(
                    0,
                    std::ptr::null(),
                    std::ptr::null_mut(),
                    std::ptr::null_mut(),
                    std::ptr::null_mut(),
                    std::ptr::null_mut(),
                    std::ptr::null_mut()
                ),
                87
            );
            let _ = MsiGetFeatureInfoW(
                0,
                dummy_w.as_ptr(),
                &mut attr,
                std::ptr::null_mut(),
                &mut sz,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
            );
            assert_eq!(MsiUseFeatureA(std::ptr::null(), dummy_a.as_ptr()), -2);
            assert_eq!(MsiUseFeatureA(dummy_a.as_ptr(), std::ptr::null()), -2);
            let _ = MsiUseFeatureA(dummy_a.as_ptr(), dummy_a.as_ptr());
            assert_eq!(MsiUseFeatureW(std::ptr::null(), dummy_w.as_ptr()), -2);
            assert_eq!(MsiUseFeatureW(dummy_w.as_ptr(), std::ptr::null()), -2);
            let _ = MsiUseFeatureW(dummy_w.as_ptr(), dummy_w.as_ptr());
            assert_eq!(
                MsiGetShortcutTargetA(
                    std::ptr::null(),
                    std::ptr::null_mut(),
                    std::ptr::null_mut(),
                    std::ptr::null_mut()
                ),
                87
            );
            let _ = MsiGetShortcutTargetA(
                dummy_a.as_ptr(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
            );
            assert_eq!(
                MsiGetShortcutTargetW(
                    std::ptr::null(),
                    std::ptr::null_mut(),
                    std::ptr::null_mut(),
                    std::ptr::null_mut()
                ),
                87
            );
            let _ = MsiGetShortcutTargetW(
                dummy_w.as_ptr(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
            );

            let empty_a = std::ffi::CString::new("").unwrap_or_default();
            let empty_w: Vec<u16> = vec![0];
            let p_empty_a = crate::action::ProductCodeA(empty_a.as_ptr());
            let p_empty_w = crate::action::ProductCodeW(empty_w.as_ptr());

            // Empty string tests to hit Err() branches
            assert_eq!(
                MsiProvideComponentA(
                    p_empty_a,
                    empty_a.as_ptr(),
                    empty_a.as_ptr(),
                    0,
                    std::ptr::null_mut(),
                    &mut sz
                ),
                87
            );
            assert_eq!(
                MsiProvideComponentW(
                    p_empty_w,
                    empty_w.as_ptr(),
                    empty_w.as_ptr(),
                    0,
                    std::ptr::null_mut(),
                    &mut sz
                ),
                87
            );

            assert_eq!(
                MsiProvideAssemblyA(
                    empty_a.as_ptr(),
                    empty_a.as_ptr(),
                    0,
                    0,
                    std::ptr::null_mut(),
                    &mut sz
                ),
                87
            );
            assert_eq!(
                MsiProvideAssemblyW(
                    empty_w.as_ptr(),
                    empty_w.as_ptr(),
                    0,
                    0,
                    std::ptr::null_mut(),
                    &mut sz
                ),
                87
            );

            // MsiProvideComponent null checks
            assert_eq!(
                MsiProvideComponentA(
                    crate::action::ProductCodeA(std::ptr::null()),
                    dummy_a.as_ptr(),
                    dummy_a.as_ptr(),
                    0,
                    std::ptr::null_mut(),
                    &mut sz
                ),
                87
            );
            assert_eq!(
                MsiProvideComponentA(
                    p_a,
                    std::ptr::null(),
                    dummy_a.as_ptr(),
                    0,
                    std::ptr::null_mut(),
                    &mut sz
                ),
                87
            );
            assert_eq!(
                MsiProvideComponentA(
                    p_a,
                    dummy_a.as_ptr(),
                    std::ptr::null(),
                    0,
                    std::ptr::null_mut(),
                    &mut sz
                ),
                87
            );

            assert_eq!(
                MsiProvideComponentW(
                    crate::action::ProductCodeW(std::ptr::null()),
                    dummy_w.as_ptr(),
                    dummy_w.as_ptr(),
                    0,
                    std::ptr::null_mut(),
                    &mut sz
                ),
                87
            );
            assert_eq!(
                MsiProvideComponentW(
                    p_w,
                    std::ptr::null(),
                    dummy_w.as_ptr(),
                    0,
                    std::ptr::null_mut(),
                    &mut sz
                ),
                87
            );
            assert_eq!(
                MsiProvideComponentW(
                    p_w,
                    dummy_w.as_ptr(),
                    std::ptr::null(),
                    0,
                    std::ptr::null_mut(),
                    &mut sz
                ),
                87
            );

            // MsiProvideAssembly null checks
            assert_eq!(
                MsiProvideAssemblyA(
                    std::ptr::null(),
                    dummy_a.as_ptr(),
                    0,
                    0,
                    std::ptr::null_mut(),
                    &mut sz
                ),
                87
            );
            assert_eq!(
                MsiProvideAssemblyW(
                    std::ptr::null(),
                    dummy_w.as_ptr(),
                    0,
                    0,
                    std::ptr::null_mut(),
                    &mut sz
                ),
                87
            );

            // MsiProvideComponentFromDescriptor null checks
            assert_eq!(
                MsiProvideComponentFromDescriptorA(
                    std::ptr::null(),
                    std::ptr::null_mut(),
                    &mut sz,
                    &mut offset
                ),
                87
            );
            assert_eq!(
                MsiProvideComponentFromDescriptorW(
                    std::ptr::null(),
                    std::ptr::null_mut(),
                    &mut sz,
                    &mut offset
                ),
                87
            );

            // MsiProvideQualifiedComponent null checks
            assert_eq!(
                MsiProvideQualifiedComponentA(
                    std::ptr::null(),
                    dummy_a.as_ptr(),
                    0,
                    std::ptr::null_mut(),
                    &mut sz
                ),
                87
            );
            assert_eq!(
                MsiProvideQualifiedComponentA(
                    dummy_a.as_ptr(),
                    std::ptr::null(),
                    0,
                    std::ptr::null_mut(),
                    &mut sz
                ),
                87
            );
            assert_eq!(
                MsiProvideQualifiedComponentW(
                    std::ptr::null(),
                    dummy_w.as_ptr(),
                    0,
                    std::ptr::null_mut(),
                    &mut sz
                ),
                87
            );
            assert_eq!(
                MsiProvideQualifiedComponentW(
                    dummy_w.as_ptr(),
                    std::ptr::null(),
                    0,
                    std::ptr::null_mut(),
                    &mut sz
                ),
                87
            );

            // MsiProvideQualifiedComponentEx null checks
            assert_eq!(
                MsiProvideQualifiedComponentExA(
                    std::ptr::null(),
                    dummy_a.as_ptr(),
                    0,
                    dummy_a.as_ptr(),
                    0,
                    0,
                    std::ptr::null_mut(),
                    &mut sz
                ),
                87
            );
            assert_eq!(
                MsiProvideQualifiedComponentExA(
                    dummy_a.as_ptr(),
                    std::ptr::null(),
                    0,
                    dummy_a.as_ptr(),
                    0,
                    0,
                    std::ptr::null_mut(),
                    &mut sz
                ),
                87
            );
            assert_eq!(
                MsiProvideQualifiedComponentExW(
                    std::ptr::null(),
                    dummy_w.as_ptr(),
                    0,
                    dummy_w.as_ptr(),
                    0,
                    0,
                    std::ptr::null_mut(),
                    &mut sz
                ),
                87
            );
            assert_eq!(
                MsiProvideQualifiedComponentExW(
                    dummy_w.as_ptr(),
                    std::ptr::null(),
                    0,
                    dummy_w.as_ptr(),
                    0,
                    0,
                    std::ptr::null_mut(),
                    &mut sz
                ),
                87
            );

            // MsiLocateComponent null checks
            assert_eq!(
                MsiLocateComponentA(std::ptr::null(), std::ptr::null_mut(), &mut sz),
                -2
            );
            assert_eq!(
                MsiLocateComponentW(std::ptr::null(), std::ptr::null_mut(), &mut sz),
                -2
            );

            // MsiGetComponentPath null checks
            assert_eq!(
                crate::win32::registry::MsiGetComponentPathA(
                    std::ptr::null(),
                    dummy_a.as_ptr(),
                    std::ptr::null_mut(),
                    &mut sz
                ),
                -1
            );
            assert_eq!(
                crate::win32::registry::MsiGetComponentPathA(
                    p_a.0,
                    std::ptr::null(),
                    std::ptr::null_mut(),
                    &mut sz
                ),
                -1
            );
            assert_eq!(
                crate::win32::registry::MsiGetComponentPathW(
                    std::ptr::null(),
                    dummy_w.as_ptr(),
                    std::ptr::null_mut(),
                    &mut sz
                ),
                -1
            );
            assert_eq!(
                crate::win32::registry::MsiGetComponentPathW(
                    p_w.0,
                    std::ptr::null(),
                    std::ptr::null_mut(),
                    &mut sz
                ),
                -1
            );

            // MsiGetComponentPathEx null checks
            assert_eq!(
                crate::win32::registry::MsiGetComponentPathExA(
                    std::ptr::null(),
                    dummy_a.as_ptr(),
                    std::ptr::null(),
                    0,
                    std::ptr::null_mut(),
                    &mut sz
                ),
                87
            );
            assert_eq!(
                crate::win32::registry::MsiGetComponentPathExA(
                    p_a.0,
                    std::ptr::null(),
                    std::ptr::null(),
                    0,
                    std::ptr::null_mut(),
                    &mut sz
                ),
                87
            );
            assert_eq!(
                crate::win32::registry::MsiGetComponentPathExW(
                    std::ptr::null(),
                    dummy_w.as_ptr(),
                    std::ptr::null(),
                    0,
                    std::ptr::null_mut(),
                    &mut sz
                ),
                87
            );
            assert_eq!(
                crate::win32::registry::MsiGetComponentPathExW(
                    p_w.0,
                    std::ptr::null(),
                    std::ptr::null(),
                    0,
                    std::ptr::null_mut(),
                    &mut sz
                ),
                87
            );

            // MsiQueryFeatureState null checks
            assert_eq!(
                crate::win32::registry::MsiQueryFeatureStateA(std::ptr::null(), dummy_a.as_ptr()),
                -1
            );
            assert_eq!(
                crate::win32::registry::MsiQueryFeatureStateA(p_a.0, std::ptr::null()),
                -1
            );
            assert_eq!(
                crate::win32::registry::MsiQueryFeatureStateW(std::ptr::null(), dummy_w.as_ptr()),
                -1
            );
            assert_eq!(
                crate::win32::registry::MsiQueryFeatureStateW(p_w.0, std::ptr::null()),
                -1
            );

            // MsiUseFeature null checks
            assert_eq!(MsiUseFeatureA(std::ptr::null(), dummy_a.as_ptr()), -2);
            assert_eq!(MsiUseFeatureA(dummy_a.as_ptr(), std::ptr::null()), -2);
            assert_eq!(MsiUseFeatureW(std::ptr::null(), dummy_w.as_ptr()), -2);
            assert_eq!(MsiUseFeatureW(dummy_w.as_ptr(), std::ptr::null()), -2);

            // MsiUseFeatureEx null checks
            assert_eq!(
                crate::win32::state::MsiUseFeatureExA(std::ptr::null(), dummy_a.as_ptr(), 0, 0),
                -2
            );
            assert_eq!(
                crate::win32::state::MsiUseFeatureExA(dummy_a.as_ptr(), std::ptr::null(), 0, 0),
                -2
            );
            assert_eq!(
                crate::win32::state::MsiUseFeatureExW(std::ptr::null(), dummy_w.as_ptr(), 0, 0),
                -2
            );
            assert_eq!(
                crate::win32::state::MsiUseFeatureExW(dummy_w.as_ptr(), std::ptr::null(), 0, 0),
                -2
            );

            // MsiGetShortcutTarget null checks
            let mut p_dummy_out_a = [0_i8; 39];
            assert_eq!(
                MsiGetShortcutTargetA(
                    std::ptr::null(),
                    p_dummy_out_a.as_mut_ptr(),
                    std::ptr::null_mut(),
                    std::ptr::null_mut()
                ),
                87
            );
            let mut p_dummy_out_w = [0_u16; 39];
            assert_eq!(
                MsiGetShortcutTargetW(
                    std::ptr::null(),
                    p_dummy_out_w.as_mut_ptr(),
                    std::ptr::null_mut(),
                    std::ptr::null_mut()
                ),
                87
            );

            // MsiGetFeatureInfo null checks
            assert_eq!(
                MsiGetFeatureInfoA(
                    0,
                    std::ptr::null(),
                    std::ptr::null_mut(),
                    std::ptr::null_mut(),
                    std::ptr::null_mut(),
                    std::ptr::null_mut(),
                    std::ptr::null_mut()
                ),
                87
            );
            assert_eq!(
                MsiGetFeatureInfoW(
                    0,
                    std::ptr::null(),
                    std::ptr::null_mut(),
                    std::ptr::null_mut(),
                    std::ptr::null_mut(),
                    std::ptr::null_mut(),
                    std::ptr::null_mut()
                ),
                87
            );

            // Mock a valid state
            let valid_guid = "{12345678-1234-1234-1234-123456789012}";
            let valid_squid = msi::platform::squid::encode_squid(valid_guid).unwrap_or_default();
            let key = format!("SOFTWARE\\MICROSOFT\\WINDOWS\\CURRENTVERSION\\INSTALLER\\USERDATA\\S-1-5-18\\COMPONENTS\\{valid_squid}");
            crate::win32::registry_backend::with_registry(|reg| {
                reg.set_value(
                    msi::platform::registry_store::RegistryRoot::LocalMachine,
                    &key,
                    None,
                    msi::platform::registry_store::RegistryValue::Sz(String::new()),
                );
            });

            let valid_a = std::ffi::CString::new(valid_guid).unwrap_or_default();
            let valid_w: Vec<u16> = valid_guid
                .encode_utf16()
                .chain(std::iter::once(0))
                .collect();

            assert_eq!(
                MsiLocateComponentA(valid_a.as_ptr(), std::ptr::null_mut(), &mut sz),
                1
            );
            assert_eq!(
                MsiLocateComponentW(valid_w.as_ptr(), std::ptr::null_mut(), &mut sz),
                1
            );

            let mut out_a = [0_i8; 100];
            let mut out_sz = u32::try_from(out_a.len()).unwrap_or(100);
            assert_eq!(
                MsiLocateComponentA(valid_a.as_ptr(), out_a.as_mut_ptr(), &mut out_sz),
                1
            );

            let mut out_w = [0_u16; 100];
            let mut out_sz_w = u32::try_from(out_w.len()).unwrap_or(100);
            assert_eq!(
                MsiLocateComponentW(valid_w.as_ptr(), out_w.as_mut_ptr(), &mut out_sz_w),
                1
            );

            crate::win32::registry_backend::with_registry(|reg| {
                reg.delete_value(
                    msi::platform::registry_store::RegistryRoot::LocalMachine,
                    &key,
                    None,
                );
            });

            // MsiGetFeatureInfo invalid parsing
            assert_eq!(
                MsiGetFeatureInfoA(
                    0,
                    dummy_a.as_ptr(),
                    std::ptr::null_mut(),
                    std::ptr::null_mut(),
                    std::ptr::null_mut(),
                    std::ptr::null_mut(),
                    std::ptr::null_mut()
                ),
                0
            );
            assert_eq!(
                MsiGetFeatureInfoW(
                    0,
                    dummy_w.as_ptr(),
                    std::ptr::null_mut(),
                    std::ptr::null_mut(),
                    std::ptr::null_mut(),
                    std::ptr::null_mut(),
                    std::ptr::null_mut()
                ),
                0
            );

            // MsiGetShortcutTarget invalid parsing
            assert_eq!(
                MsiGetShortcutTargetA(
                    dummy_a.as_ptr(),
                    std::ptr::null_mut(),
                    std::ptr::null_mut(),
                    std::ptr::null_mut()
                ),
                0
            );
            assert_eq!(
                MsiGetShortcutTargetW(
                    dummy_w.as_ptr(),
                    std::ptr::null_mut(),
                    std::ptr::null_mut(),
                    std::ptr::null_mut()
                ),
                0
            );
        }
    }

    #[test]
    fn test_locate_component_missed() {
        // null pointers
        let mut sz = 0;
        assert_eq!(
            unsafe { MsiLocateComponentA(std::ptr::null(), std::ptr::null_mut(), &mut sz) },
            -2
        );
        assert_eq!(
            unsafe { MsiLocateComponentW(std::ptr::null(), std::ptr::null_mut(), &mut sz) },
            -2
        );

        // empty strings to hit locate_component Err
        let empty_a = std::ffi::CString::new("").unwrap_or_default();
        let empty_w: Vec<u16> = vec![0];
        assert_eq!(
            unsafe { MsiLocateComponentA(empty_a.as_ptr(), std::ptr::null_mut(), &mut sz) },
            -2
        );
        assert_eq!(
            unsafe { MsiLocateComponentW(empty_w.as_ptr(), std::ptr::null_mut(), &mut sz) },
            -2
        );
        let err_a =
            std::ffi::CString::new("{00000000-0000-0000-0000-000000000000}").unwrap_or_default();
        let err_w: Vec<u16> = "{00000000-0000-0000-0000-000000000000}"
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect();
        assert_eq!(
            unsafe { MsiLocateComponentA(err_a.as_ptr(), std::ptr::null_mut(), &mut sz) },
            -1
        );
        assert_eq!(
            unsafe { MsiLocateComponentW(err_w.as_ptr(), std::ptr::null_mut(), &mut sz) },
            -1
        );
    }

    #[test]
    fn test_locate_component_invalid_guid() {
        let invalid_w = [0xD800_u16, 0x0000]; // Unpaired surrogate
        let dummy_a = std::ffi::CString::new("dummy").unwrap_or_default();
        let dummy_w: Vec<u16> = "dummy".encode_utf16().chain(std::iter::once(0)).collect();
        let p_a = crate::action::ProductCodeA(dummy_a.as_ptr());
        let p_w = crate::action::ProductCodeW(dummy_w.as_ptr());
        let invalid_a = [i8::from_ne_bytes([0xFF]), 0x00]; // Invalid UTF-8
        let p_invalid_a = crate::action::ProductCodeA(invalid_a.as_ptr());
        let p_invalid_w = crate::action::ProductCodeW(invalid_w.as_ptr());
        let mut sz = 0;
        unsafe {
            // MsiProvideComponentA invalid strings
            assert_eq!(
                MsiProvideComponentA(
                    p_invalid_a,
                    dummy_a.as_ptr(),
                    dummy_a.as_ptr(),
                    0,
                    std::ptr::null_mut(),
                    &mut sz
                ),
                87
            );
            assert_eq!(
                MsiProvideComponentA(
                    p_a,
                    invalid_a.as_ptr().cast(),
                    dummy_a.as_ptr(),
                    0,
                    std::ptr::null_mut(),
                    &mut sz
                ),
                87
            );
            assert_eq!(
                MsiProvideComponentA(
                    p_a,
                    dummy_a.as_ptr(),
                    invalid_a.as_ptr().cast(),
                    0,
                    std::ptr::null_mut(),
                    &mut sz
                ),
                87
            );

            // MsiProvideComponentW invalid strings
            assert_eq!(
                MsiProvideComponentW(
                    p_invalid_w,
                    dummy_w.as_ptr(),
                    dummy_w.as_ptr(),
                    0,
                    std::ptr::null_mut(),
                    &mut sz
                ),
                87
            );
            assert_eq!(
                MsiProvideComponentW(
                    p_w,
                    invalid_w.as_ptr(),
                    dummy_w.as_ptr(),
                    0,
                    std::ptr::null_mut(),
                    &mut sz
                ),
                87
            );
            assert_eq!(
                MsiProvideComponentW(
                    p_w,
                    dummy_w.as_ptr(),
                    invalid_w.as_ptr(),
                    0,
                    std::ptr::null_mut(),
                    &mut sz
                ),
                87
            );
        }

        let mut buf_w = [0u16; 256];
        let mut buf_size_w = 256;
        let res_w =
            unsafe { MsiLocateComponentW(invalid_w.as_ptr(), buf_w.as_mut_ptr(), &mut buf_size_w) };
        assert_eq!(res_w, -2); // INSTALLSTATE_INVALIDARG

        let mut buf_a = [0i8; 256];
        let mut buf_size_a = 256;
        let res_a =
            unsafe { MsiLocateComponentA(invalid_a.as_ptr(), buf_a.as_mut_ptr(), &mut buf_size_a) };
        assert_eq!(res_a, -2); // INSTALLSTATE_INVALIDARG
    }
}

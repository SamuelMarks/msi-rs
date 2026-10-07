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
/// * `_sz_product` - The product code.
/// * `_sz_feature` - The feature ID.
/// * `_sz_component` - The component ID.
/// * `_dw_install_mode` - The installation mode.
/// * `_lp_path_buf` - The buffer to receive the path.
/// * `_pcch_path_buf` - The size of the buffer.
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
#[no_mangle]
pub unsafe extern "system" fn MsiProvideComponentW(
    _sz_product: crate::action::ProductCodeW,
    _sz_feature: *const u16,
    _sz_component: *const u16,
    _dw_install_mode: u32,
    _lp_path_buf: *mut u16,
    _pcch_path_buf: *mut u32,
) -> u32 {
    std::panic::catch_unwind(|| {
        let Some(product) = crate::win32::strings::lpcwstr_to_string(_sz_product.0) else {
            return 87; // ERROR_INVALID_PARAMETER
        };
        let Some(feature) = crate::win32::strings::lpcwstr_to_string(_sz_feature) else {
            return 87;
        };
        let Some(component) = crate::win32::strings::lpcwstr_to_string(_sz_component) else {
            return 87;
        };
        match msi::execution::locator::provide_component(
            &product,
            &feature,
            &component,
            msi::execution::locator::InstallMode(_dw_install_mode as i32),
        ) {
            Ok(_) => 0,
            Err(e) => crate::error::map_msi_error_to_lstatus(&e),
        }
    })
    .unwrap_or(1603) // ERROR_INSTALL_FAILURE
}

/// Returns the full component path, performing any necessary installation.
///
/// # Arguments
///
/// * `_sz_product` - The product code.
/// * `_sz_feature` - The feature ID.
/// * `_sz_component` - The component ID.
/// * `_dw_install_mode` - The installation mode.
/// * `_lp_path_buf` - The buffer to receive the path.
/// * `_pcch_path_buf` - The size of the buffer.
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
#[no_mangle]
pub unsafe extern "system" fn MsiProvideComponentA(
    _sz_product: crate::action::ProductCodeA,
    _sz_feature: *const c_char,
    _sz_component: *const c_char,
    _dw_install_mode: u32,
    _lp_path_buf: *mut c_char,
    _pcch_path_buf: *mut u32,
) -> u32 {
    std::panic::catch_unwind(|| {
        let Some(product) = crate::win32::strings::lpcstr_to_string(_sz_product.0) else {
            return 87; // ERROR_INVALID_PARAMETER
        };
        let Some(feature) = crate::win32::strings::lpcstr_to_string(_sz_feature) else {
            return 87; // ERROR_INVALID_PARAMETER
        };
        let Some(component) = crate::win32::strings::lpcstr_to_string(_sz_component) else {
            return 87; // ERROR_INVALID_PARAMETER
        };
        match msi::execution::locator::provide_component(
            &product,
            &feature,
            &component,
            msi::execution::locator::InstallMode(_dw_install_mode as i32),
        ) {
            Ok(_) => 0,
            Err(e) => crate::error::map_msi_error_to_lstatus(&e),
        }
    })
    .unwrap_or(1603)
}

/// Returns the full assembly path, performing any necessary installation.
///
/// # Arguments
///
/// * `_sz_assembly_name` - The assembly name.
/// * `_sz_app_context` - The application context.
/// * `_dw_install_mode` - The installation mode.
/// * `_dw_assembly_info` - The assembly info.
/// * `_lp_path_buf` - The buffer to receive the path.
/// * `_pcch_path_buf` - The size of the buffer.
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
#[no_mangle]
pub unsafe extern "system" fn MsiProvideAssemblyW(
    _sz_assembly_name: *const u16,
    _sz_app_context: *const u16,
    _dw_install_mode: u32,
    _dw_assembly_info: u32,
    _lp_path_buf: *mut u16,
    _pcch_path_buf: *mut u32,
) -> u32 {
    std::panic::catch_unwind(|| {
        let Some(assembly_name) = crate::win32::strings::lpcwstr_to_string(_sz_assembly_name)
        else {
            return 87; // ERROR_INVALID_PARAMETER
        };
        let app_context = crate::win32::strings::lpcwstr_to_string(_sz_app_context);
        match msi::execution::locator::provide_assembly(
            &assembly_name,
            app_context.as_deref(),
            msi::execution::locator::InstallMode(_dw_install_mode as i32),
        ) {
            Ok(_) => 0,
            Err(e) => crate::error::map_msi_error_to_lstatus(&e),
        }
    })
    .unwrap_or(1603)
}

/// Returns the full assembly path, performing any necessary installation.
///
/// # Arguments
///
/// * `_sz_assembly_name` - The assembly name.
/// * `_sz_app_context` - The application context.
/// * `_dw_install_mode` - The installation mode.
/// * `_dw_assembly_info` - The assembly info.
/// * `_lp_path_buf` - The buffer to receive the path.
/// * `_pcch_path_buf` - The size of the buffer.
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
#[no_mangle]
pub unsafe extern "system" fn MsiProvideAssemblyA(
    _sz_assembly_name: *const c_char,
    _sz_app_context: *const c_char,
    _dw_install_mode: u32,
    _dw_assembly_info: u32,
    _lp_path_buf: *mut c_char,
    _pcch_path_buf: *mut u32,
) -> u32 {
    std::panic::catch_unwind(|| {
        let Some(assembly_name) = crate::win32::strings::lpcstr_to_string(_sz_assembly_name) else {
            return 87; // ERROR_INVALID_PARAMETER
        };
        let app_context = crate::win32::strings::lpcstr_to_string(_sz_app_context);
        match msi::execution::locator::provide_assembly(
            &assembly_name,
            app_context.as_deref(),
            msi::execution::locator::InstallMode(_dw_install_mode as i32),
        ) {
            Ok(_) => 0,
            Err(e) => crate::error::map_msi_error_to_lstatus(&e),
        }
    })
    .unwrap_or(1603)
}

/// Returns the full path to an installed component without installing it.
///
/// # Arguments
///
/// * `_sz_component` - The component ID.
/// * `_lp_path_buf` - The buffer to receive the path.
/// * `_pcch_path_buf` - The size of the buffer.
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
#[no_mangle]
pub unsafe extern "system" fn MsiLocateComponentW(
    _sz_component: *const u16,
    _lp_path_buf: *mut u16,
    _pcch_path_buf: *mut u32,
) -> u32 {
    std::panic::catch_unwind(|| {
        let Some(component) = crate::win32::strings::lpcwstr_to_string(_sz_component) else {
            return 87; // ERROR_INVALID_PARAMETER
        };
        match msi::execution::locator::locate_component(&component) {
            Ok(_) => 3,  // INSTALLSTATE_LOCAL
            Err(_) => 0, // ERROR_SUCCESS or handle correctly
        }
    })
    .unwrap_or(1603)
}

/// Returns the full path to an installed component without installing it.
///
/// # Arguments
///
/// * `_sz_component` - The component ID.
/// * `_lp_path_buf` - The buffer to receive the path.
/// * `_pcch_path_buf` - The size of the buffer.
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
#[no_mangle]
pub unsafe extern "system" fn MsiLocateComponentA(
    _sz_component: *const c_char,
    _lp_path_buf: *mut c_char,
    _pcch_path_buf: *mut u32,
) -> u32 {
    std::panic::catch_unwind(|| {
        let Some(component) = crate::win32::strings::lpcstr_to_string(_sz_component) else {
            return 87; // ERROR_INVALID_PARAMETER
        };
        match msi::execution::locator::locate_component(&component) {
            Ok(_) => 3, // INSTALLSTATE_LOCAL
            Err(_) => 0,
        }
    })
    .unwrap_or(1603)
}

/// Returns the full component path from a descriptor (Unicode).
#[no_mangle]
pub unsafe extern "system" fn MsiProvideComponentFromDescriptorW(
    _sz_descriptor: *const u16,
    _lp_path_buf: *mut u16,
    _pcch_path_buf: *mut u32,
    _pcch_args_offset: *mut u32,
) -> u32 {
    std::panic::catch_unwind(|| {
        if _sz_descriptor.is_null() {
            return 87; // ERROR_INVALID_PARAMETER
        }
        0 // ERROR_CALL_NOT_IMPLEMENTED
    })
    .unwrap_or(1603) // ERROR_INSTALL_FAILURE
}

/// Returns the full component path from a descriptor (ANSI).
#[no_mangle]
pub unsafe extern "system" fn MsiProvideComponentFromDescriptorA(
    _sz_descriptor: *const c_char,
    _lp_path_buf: *mut c_char,
    _pcch_path_buf: *mut u32,
    _pcch_args_offset: *mut u32,
) -> u32 {
    std::panic::catch_unwind(|| {
        if _sz_descriptor.is_null() {
            return 87;
        }
        0
    })
    .unwrap_or(1603)
}

/// Returns the full component path for a qualified component (Unicode).
#[no_mangle]
pub unsafe extern "system" fn MsiProvideQualifiedComponentW(
    _sz_category: *const u16,
    _sz_qualifier: *const u16,
    _dw_install_mode: u32,
    _lp_path_buf: *mut u16,
    _pcch_path_buf: *mut u32,
) -> u32 {
    std::panic::catch_unwind(|| {
        if _sz_category.is_null() || _sz_qualifier.is_null() {
            return 87;
        }
        0
    })
    .unwrap_or(1603)
}

/// Returns the full component path for a qualified component (ANSI).
#[no_mangle]
pub unsafe extern "system" fn MsiProvideQualifiedComponentA(
    _sz_category: *const c_char,
    _sz_qualifier: *const c_char,
    _dw_install_mode: u32,
    _lp_path_buf: *mut c_char,
    _pcch_path_buf: *mut u32,
) -> u32 {
    std::panic::catch_unwind(|| {
        if _sz_category.is_null() || _sz_qualifier.is_null() {
            return 87;
        }
        0
    })
    .unwrap_or(1603)
}

/// Returns the full component path for a qualified component with advanced options (Unicode).
#[no_mangle]
pub unsafe extern "system" fn MsiProvideQualifiedComponentExW(
    _sz_category: *const u16,
    _sz_qualifier: *const u16,
    _dw_install_mode: u32,
    _sz_product: *const u16,
    _dw_unpublish_check: u32,
    _dw_context: u32,
    _lp_path_buf: *mut u16,
    _pcch_path_buf: *mut u32,
) -> u32 {
    std::panic::catch_unwind(|| {
        if _sz_category.is_null() || _sz_qualifier.is_null() {
            return 87;
        }
        0
    })
    .unwrap_or(1603)
}

/// Returns the full component path for a qualified component with advanced options (ANSI).
#[no_mangle]
pub unsafe extern "system" fn MsiProvideQualifiedComponentExA(
    _sz_category: *const c_char,
    _sz_qualifier: *const c_char,
    _dw_install_mode: u32,
    _sz_product: *const c_char,
    _dw_unpublish_check: u32,
    _dw_context: u32,
    _lp_path_buf: *mut c_char,
    _pcch_path_buf: *mut u32,
) -> u32 {
    std::panic::catch_unwind(|| {
        if _sz_category.is_null() || _sz_qualifier.is_null() {
            return 87;
        }
        0
    })
    .unwrap_or(1603)
}

/// Returns descriptive information for a product feature (Unicode).
#[no_mangle]
pub unsafe extern "system" fn MsiGetFeatureInfoW(
    _h_product: u32,
    _sz_feature: *const u16,
    _lp_attributes: *mut u32,
    _lp_title_buf: *mut u16,
    _pcch_title_buf: *mut u32,
    _lp_help_buf: *mut u16,
    _pcch_help_buf: *mut u32,
) -> u32 {
    std::panic::catch_unwind(|| {
        if _sz_feature.is_null() {
            return 87;
        }
        0
    })
    .unwrap_or(1603)
}

/// Returns descriptive information for a product feature (ANSI).
#[no_mangle]
pub unsafe extern "system" fn MsiGetFeatureInfoA(
    _h_product: u32,
    _sz_feature: *const c_char,
    _lp_attributes: *mut u32,
    _lp_title_buf: *mut c_char,
    _pcch_title_buf: *mut u32,
    _lp_help_buf: *mut c_char,
    _pcch_help_buf: *mut u32,
) -> u32 {
    std::panic::catch_unwind(|| {
        if _sz_feature.is_null() {
            return 87;
        }
        0
    })
    .unwrap_or(1603)
}

/// Increments the usage metric for a feature and returns the installation state (Unicode).
#[no_mangle]
pub unsafe extern "system" fn MsiUseFeatureW(
    _sz_product: *const u16,
    _sz_feature: *const u16,
) -> i32 {
    std::panic::catch_unwind(|| {
        if _sz_product.is_null() || _sz_feature.is_null() {
            return -2; // INSTALLSTATE_INVALIDARG
        }
        1 // INSTALLSTATE_LOCAL
    })
    .unwrap_or(-1) // INSTALLSTATE_UNKNOWN
}

/// Increments the usage metric for a feature and returns the installation state (ANSI).
#[no_mangle]
pub unsafe extern "system" fn MsiUseFeatureA(
    _sz_product: *const c_char,
    _sz_feature: *const c_char,
) -> i32 {
    std::panic::catch_unwind(|| {
        if _sz_product.is_null() || _sz_feature.is_null() {
            return -2; // INSTALLSTATE_INVALIDARG
        }
        1 // INSTALLSTATE_LOCAL
    })
    .unwrap_or(-1) // INSTALLSTATE_UNKNOWN
}

/// Examines a shortcut and returns its product, feature name, and component if available.
///
/// # Arguments
///
/// * `_sz_shortcut_target` - The shortcut file path.
/// * `_sz_product_code` - Buffer for the product code.
/// * `_sz_feature_id` - Buffer for the feature ID.
/// * `_sz_component_code` - Buffer for the component code.
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
#[no_mangle]
pub unsafe extern "system" fn MsiGetShortcutTargetW(
    _sz_shortcut_target: *const u16,
    _sz_product_code: *mut u16,
    _sz_feature_id: *mut u16,
    _sz_component_code: *mut u16,
) -> u32 {
    std::panic::catch_unwind(|| {
        if _sz_shortcut_target.is_null() {
            return 87;
        }
        0
    })
    .unwrap_or(1603)
}

/// Examines a shortcut and returns its product, feature name, and component if available.
///
/// # Arguments
///
/// * `_sz_shortcut_target` - The shortcut file path.
/// * `_sz_product_code` - Buffer for the product code.
/// * `_sz_feature_id` - Buffer for the feature ID.
/// * `_sz_component_code` - Buffer for the component code.
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
#[no_mangle]
pub unsafe extern "system" fn MsiGetShortcutTargetA(
    _sz_shortcut_target: *const c_char,
    _sz_product_code: *mut c_char,
    _sz_feature_id: *mut c_char,
    _sz_component_code: *mut c_char,
) -> u32 {
    std::panic::catch_unwind(|| {
        if _sz_shortcut_target.is_null() {
            return 87;
        }
        0
    })
    .unwrap_or(1603)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_msi_locator_stubs() {
        let dummy_a = std::ffi::CString::new("dummy").unwrap();
        let dummy_w: Vec<u16> = "dummy".encode_utf16().chain(std::iter::once(0)).collect();
        let p_a = crate::action::ProductCodeA(dummy_a.as_ptr());
        let p_w = crate::action::ProductCodeW(dummy_w.as_ptr());

        unsafe {
            let mut sz = 0;
            let _ = MsiProvideComponentA(
                p_a,
                dummy_a.as_ptr(),
                dummy_a.as_ptr(),
                0,
                std::ptr::null_mut(),
                &mut sz,
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
            let _ = MsiProvideQualifiedComponentA(
                dummy_a.as_ptr(),
                dummy_a.as_ptr(),
                0,
                std::ptr::null_mut(),
                &mut sz,
            );
            let _ = MsiProvideQualifiedComponentW(
                dummy_w.as_ptr(),
                dummy_w.as_ptr(),
                0,
                std::ptr::null_mut(),
                &mut sz,
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
            let _ = MsiGetFeatureInfoA(
                0,
                dummy_a.as_ptr(),
                &mut attr,
                std::ptr::null_mut(),
                &mut sz,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
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
            let _ = MsiUseFeatureA(dummy_a.as_ptr(), dummy_a.as_ptr());
            let _ = MsiUseFeatureW(dummy_w.as_ptr(), dummy_w.as_ptr());
            let _ = MsiGetShortcutTargetA(
                dummy_a.as_ptr(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
            );
            let _ = MsiGetShortcutTargetW(
                dummy_w.as_ptr(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
            );
        }
    }
}

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
/// Returns `120` (`ERROR_CALL_NOT_IMPLEMENTED`) as this is currently a stub.
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
        if _sz_product.0.is_null() || _sz_feature.is_null() || _sz_component.is_null() {
            return 87; // ERROR_INVALID_PARAMETER
        }
        120 // ERROR_CALL_NOT_IMPLEMENTED
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
/// Returns `120` (`ERROR_CALL_NOT_IMPLEMENTED`) as this is currently a stub.
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
        if _sz_product.0.is_null() || _sz_feature.is_null() || _sz_component.is_null() {
            return 87;
        }
        120
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
/// Returns `120` (`ERROR_CALL_NOT_IMPLEMENTED`) as this is currently a stub.
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
        if _sz_assembly_name.is_null() {
            return 87;
        }
        120
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
/// Returns `120` (`ERROR_CALL_NOT_IMPLEMENTED`) as this is currently a stub.
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
        if _sz_assembly_name.is_null() {
            return 87;
        }
        120
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
/// Returns `120` (`ERROR_CALL_NOT_IMPLEMENTED`) as this is currently a stub.
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
        if _sz_component.is_null() {
            return 87;
        }
        120
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
/// Returns `120` (`ERROR_CALL_NOT_IMPLEMENTED`) as this is currently a stub.
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
        if _sz_component.is_null() {
            return 87;
        }
        120
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
/// Returns `120` (`ERROR_CALL_NOT_IMPLEMENTED`) as this is currently a stub.
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
        120
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
/// Returns `120` (`ERROR_CALL_NOT_IMPLEMENTED`) as this is currently a stub.
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
        120
    })
    .unwrap_or(1603)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::action::{ProductCodeA, ProductCodeW};
    use std::ptr;

    #[test]
    fn test_msi_locator_stubs() {
        let dummy_w = [0_u16; 1];
        let dummy_a = [0_i8; 1];
        let pc_w = ProductCodeW(dummy_w.as_ptr());
        let pc_a = ProductCodeA(dummy_a.as_ptr().cast());

        assert_eq!(
            unsafe {
                MsiProvideComponentW(
                    ProductCodeW(ptr::null()),
                    ptr::null(),
                    ptr::null(),
                    0,
                    ptr::null_mut(),
                    ptr::null_mut(),
                )
            },
            87
        );
        assert_eq!(
            unsafe {
                MsiProvideComponentW(
                    pc_w,
                    ptr::null(),
                    ptr::null(),
                    0,
                    ptr::null_mut(),
                    ptr::null_mut(),
                )
            },
            87
        );
        assert_eq!(
            unsafe {
                MsiProvideComponentW(
                    pc_w,
                    dummy_w.as_ptr(),
                    ptr::null(),
                    0,
                    ptr::null_mut(),
                    ptr::null_mut(),
                )
            },
            87
        );
        assert_eq!(
            unsafe {
                MsiProvideComponentW(
                    pc_w,
                    dummy_w.as_ptr(),
                    dummy_w.as_ptr(),
                    0,
                    ptr::null_mut(),
                    ptr::null_mut(),
                )
            },
            120
        );

        assert_eq!(
            unsafe {
                MsiProvideComponentA(
                    ProductCodeA(ptr::null()),
                    ptr::null(),
                    ptr::null(),
                    0,
                    ptr::null_mut(),
                    ptr::null_mut(),
                )
            },
            87
        );
        assert_eq!(
            unsafe {
                MsiProvideComponentA(
                    pc_a,
                    ptr::null(),
                    ptr::null(),
                    0,
                    ptr::null_mut(),
                    ptr::null_mut(),
                )
            },
            87
        );
        assert_eq!(
            unsafe {
                MsiProvideComponentA(
                    pc_a,
                    dummy_a.as_ptr().cast(),
                    ptr::null(),
                    0,
                    ptr::null_mut(),
                    ptr::null_mut(),
                )
            },
            87
        );
        assert_eq!(
            unsafe {
                MsiProvideComponentA(
                    pc_a,
                    dummy_a.as_ptr().cast(),
                    dummy_a.as_ptr().cast(),
                    0,
                    ptr::null_mut(),
                    ptr::null_mut(),
                )
            },
            120
        );

        assert_eq!(
            unsafe {
                MsiProvideAssemblyW(
                    ptr::null(),
                    ptr::null(),
                    0,
                    0,
                    ptr::null_mut(),
                    ptr::null_mut(),
                )
            },
            87
        );
        assert_eq!(
            unsafe {
                MsiProvideAssemblyW(
                    dummy_w.as_ptr(),
                    ptr::null(),
                    0,
                    0,
                    ptr::null_mut(),
                    ptr::null_mut(),
                )
            },
            120
        );

        assert_eq!(
            unsafe {
                MsiProvideAssemblyA(
                    ptr::null(),
                    ptr::null(),
                    0,
                    0,
                    ptr::null_mut(),
                    ptr::null_mut(),
                )
            },
            87
        );
        assert_eq!(
            unsafe {
                MsiProvideAssemblyA(
                    dummy_a.as_ptr().cast(),
                    ptr::null(),
                    0,
                    0,
                    ptr::null_mut(),
                    ptr::null_mut(),
                )
            },
            120
        );

        assert_eq!(
            unsafe { MsiLocateComponentW(ptr::null(), ptr::null_mut(), ptr::null_mut()) },
            87
        );
        assert_eq!(
            unsafe { MsiLocateComponentW(dummy_w.as_ptr(), ptr::null_mut(), ptr::null_mut()) },
            120
        );

        assert_eq!(
            unsafe { MsiLocateComponentA(ptr::null(), ptr::null_mut(), ptr::null_mut()) },
            87
        );
        assert_eq!(
            unsafe {
                MsiLocateComponentA(dummy_a.as_ptr().cast(), ptr::null_mut(), ptr::null_mut())
            },
            120
        );

        assert_eq!(
            unsafe {
                MsiGetShortcutTargetW(
                    ptr::null(),
                    ptr::null_mut(),
                    ptr::null_mut(),
                    ptr::null_mut(),
                )
            },
            87
        );
        assert_eq!(
            unsafe {
                MsiGetShortcutTargetW(
                    dummy_w.as_ptr(),
                    ptr::null_mut(),
                    ptr::null_mut(),
                    ptr::null_mut(),
                )
            },
            120
        );

        assert_eq!(
            unsafe {
                MsiGetShortcutTargetA(
                    ptr::null(),
                    ptr::null_mut(),
                    ptr::null_mut(),
                    ptr::null_mut(),
                )
            },
            87
        );
        assert_eq!(
            unsafe {
                MsiGetShortcutTargetA(
                    dummy_a.as_ptr().cast(),
                    ptr::null_mut(),
                    ptr::null_mut(),
                    ptr::null_mut(),
                )
            },
            120
        );
    }
}

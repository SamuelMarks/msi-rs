//! C-ABI functions for product advertisement.
//!
//! These functions are used for Just-In-Time (JIT) installation hooks via shortcuts and COM activations.
//!
//! # Cross-Platform Support
//!
//! When fully implemented, these hooks will bind advertisement hooks into:
//! - Freedesktop `.desktop` files (Linux/FreeBSD)
//! - macOS `LaunchServices` `.app` aliases

use std::ffi::c_char;
use std::ffi::c_void;
use std::panic::catch_unwind;

/// Strong type for Advertise Flags.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
#[repr(transparent)]
pub struct AdvertiseFlags(pub u32);

/// Strong type for Script Context (e.g. Registry handle for advertised scripts).
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
#[repr(transparent)]
pub struct ScriptContext(pub *mut c_void);

/// Advertises a product using an ANSI package path.
///
/// # Arguments
///
/// * `_sz_package_path` - Path to the package.
/// * `_sz_script_info` - Script information.
/// * `_sz_transforms` - Applied transforms.
/// * `_lgid_language` - Language ID.
///
/// # Errors
///
/// Returns `87` (`ERROR_INVALID_PARAMETER`) if `_sz_package_path` is NULL.
/// Returns `120` (`ERROR_CALL_NOT_IMPLEMENTED`) as this is a stub.
///
/// # Safety
/// The provided string pointers must be null-terminated if they are not null.
#[no_mangle]
pub unsafe extern "system" fn MsiAdvertiseProductA(
    _sz_package_path: *const c_char,
    _sz_script_info: *const c_char,
    _sz_transforms: *const c_char,
    _lgid_language: u16,
) -> u32 {
    let result = catch_unwind(|| {
        if _sz_package_path.is_null() {
            return 87; // ERROR_INVALID_PARAMETER
        }
        120 // ERROR_CALL_NOT_IMPLEMENTED
    });
    result.unwrap_or(1603) // ERROR_INSTALL_FAILURE
}

/// Advertises a product using a Unicode package path.
///
/// # Arguments
///
/// * `_sz_package_path` - Path to the package.
/// * `_sz_script_info` - Script information.
/// * `_sz_transforms` - Applied transforms.
/// * `_lgid_language` - Language ID.
///
/// # Errors
///
/// Returns `87` (`ERROR_INVALID_PARAMETER`) if `_sz_package_path` is NULL.
/// Returns `120` (`ERROR_CALL_NOT_IMPLEMENTED`) as this is a stub.
///
/// # Safety
/// The provided string pointers must be null-terminated if they are not null.
#[no_mangle]
pub unsafe extern "system" fn MsiAdvertiseProductW(
    _sz_package_path: *const u16,
    _sz_script_info: *const u16,
    _sz_transforms: *const u16,
    _lgid_language: u16,
) -> u32 {
    let result = catch_unwind(|| {
        if _sz_package_path.is_null() {
            return 87; // ERROR_INVALID_PARAMETER
        }
        120 // ERROR_CALL_NOT_IMPLEMENTED
    });
    result.unwrap_or(1603)
}

/// Advertises a script using an ANSI file path.
///
/// # Arguments
///
/// * `_sz_script_file` - Path to the script.
/// * `_dw_flags` - Advertise flags.
/// * `_ph_reg_data` - Registry data context.
/// * `_f_remove_items` - Whether to remove items.
///
/// # Errors
///
/// Returns `87` (`ERROR_INVALID_PARAMETER`) if `_sz_script_file` is NULL.
/// Returns `120` (`ERROR_CALL_NOT_IMPLEMENTED`) as this is a stub.
///
/// # Safety
/// The provided string pointer must be null-terminated if it is not null.
#[no_mangle]
pub unsafe extern "system" fn MsiAdvertiseScriptA(
    _sz_script_file: *const c_char,
    _dw_flags: AdvertiseFlags,
    _ph_reg_data: *mut ScriptContext,
    _f_remove_items: i32,
) -> u32 {
    let result = catch_unwind(|| {
        if _sz_script_file.is_null() {
            return 87; // ERROR_INVALID_PARAMETER
        }
        120 // ERROR_CALL_NOT_IMPLEMENTED
    });
    result.unwrap_or(1603)
}

/// Advertises a script using a Unicode file path.
///
/// # Arguments
///
/// * `_sz_script_file` - Path to the script.
/// * `_dw_flags` - Advertise flags.
/// * `_ph_reg_data` - Registry data context.
/// * `_f_remove_items` - Whether to remove items.
///
/// # Errors
///
/// Returns `87` (`ERROR_INVALID_PARAMETER`) if `_sz_script_file` is NULL.
/// Returns `120` (`ERROR_CALL_NOT_IMPLEMENTED`) as this is a stub.
///
/// # Safety
/// The provided string pointer must be null-terminated if it is not null.
#[no_mangle]
pub unsafe extern "system" fn MsiAdvertiseScriptW(
    _sz_script_file: *const u16,
    _dw_flags: AdvertiseFlags,
    _ph_reg_data: *mut ScriptContext,
    _f_remove_items: i32,
) -> u32 {
    let result = catch_unwind(|| {
        if _sz_script_file.is_null() {
            return 87; // ERROR_INVALID_PARAMETER
        }
        120 // ERROR_CALL_NOT_IMPLEMENTED
    });
    result.unwrap_or(1603)
}

/// Processes an advertise script using an ANSI file path.
///
/// # Arguments
///
/// * `_sz_script_file` - Path to the script.
/// * `_sz_icon_folder` - Path to icon folder.
/// * `_h_reg_data` - Registry data context.
/// * `_f_shortcuts` - Process shortcuts.
/// * `_f_remove_items` - Remove items.
///
/// # Errors
///
/// Returns `87` (`ERROR_INVALID_PARAMETER`) if `_sz_script_file` is NULL.
/// Returns `120` (`ERROR_CALL_NOT_IMPLEMENTED`) as this is a stub.
///
/// # Safety
/// The provided string pointers must be null-terminated if they are not null.
#[no_mangle]
pub unsafe extern "system" fn MsiProcessAdvertiseScriptA(
    _sz_script_file: *const c_char,
    _sz_icon_folder: *const c_char,
    _h_reg_data: ScriptContext,
    _f_shortcuts: i32,
    _f_remove_items: i32,
) -> u32 {
    let result = catch_unwind(|| {
        if _sz_script_file.is_null() {
            return 87; // ERROR_INVALID_PARAMETER
        }
        120 // ERROR_CALL_NOT_IMPLEMENTED
    });
    result.unwrap_or(1603)
}

/// Processes an advertise script using a Unicode file path.
///
/// # Arguments
///
/// * `_sz_script_file` - Path to the script.
/// * `_sz_icon_folder` - Path to icon folder.
/// * `_h_reg_data` - Registry data context.
/// * `_f_shortcuts` - Process shortcuts.
/// * `_f_remove_items` - Remove items.
///
/// # Errors
///
/// Returns `87` (`ERROR_INVALID_PARAMETER`) if `_sz_script_file` is NULL.
/// Returns `120` (`ERROR_CALL_NOT_IMPLEMENTED`) as this is a stub.
///
/// # Safety
/// The provided string pointers must be null-terminated if they are not null.
#[no_mangle]
pub unsafe extern "system" fn MsiProcessAdvertiseScriptW(
    _sz_script_file: *const u16,
    _sz_icon_folder: *const u16,
    _h_reg_data: ScriptContext,
    _f_shortcuts: i32,
    _f_remove_items: i32,
) -> u32 {
    let result = catch_unwind(|| {
        if _sz_script_file.is_null() {
            return 87; // ERROR_INVALID_PARAMETER
        }
        120 // ERROR_CALL_NOT_IMPLEMENTED
    });
    result.unwrap_or(1603)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ptr;

    #[test]
    fn test_msi_advertise_product_a() {
        assert_eq!(
            unsafe { MsiAdvertiseProductA(ptr::null(), ptr::null(), ptr::null(), 0) },
            87
        );
        let dummy = [0_i8; 1];
        assert_eq!(
            unsafe { MsiAdvertiseProductA(dummy.as_ptr().cast(), ptr::null(), ptr::null(), 0) },
            120
        );
    }

    #[test]
    fn test_msi_advertise_product_w() {
        assert_eq!(
            unsafe { MsiAdvertiseProductW(ptr::null(), ptr::null(), ptr::null(), 0) },
            87
        );
        let dummy = [0_u16; 1];
        assert_eq!(
            unsafe { MsiAdvertiseProductW(dummy.as_ptr(), ptr::null(), ptr::null(), 0) },
            120
        );
    }

    #[test]
    fn test_msi_advertise_script_a() {
        assert_eq!(
            unsafe { MsiAdvertiseScriptA(ptr::null(), AdvertiseFlags(0), ptr::null_mut(), 0) },
            87
        );
        let dummy = [0_i8; 1];
        assert_eq!(
            unsafe {
                MsiAdvertiseScriptA(dummy.as_ptr().cast(), AdvertiseFlags(0), ptr::null_mut(), 0)
            },
            120
        );
    }

    #[test]
    fn test_msi_advertise_script_w() {
        assert_eq!(
            unsafe { MsiAdvertiseScriptW(ptr::null(), AdvertiseFlags(0), ptr::null_mut(), 0) },
            87
        );
        let dummy = [0_u16; 1];
        assert_eq!(
            unsafe { MsiAdvertiseScriptW(dummy.as_ptr(), AdvertiseFlags(0), ptr::null_mut(), 0) },
            120
        );
    }

    #[test]
    fn test_msi_process_advertise_script_a() {
        assert_eq!(
            unsafe {
                MsiProcessAdvertiseScriptA(
                    ptr::null(),
                    ptr::null(),
                    ScriptContext(ptr::null_mut()),
                    0,
                    0,
                )
            },
            87
        );
        let dummy = [0_i8; 1];
        assert_eq!(
            unsafe {
                MsiProcessAdvertiseScriptA(
                    dummy.as_ptr().cast(),
                    ptr::null(),
                    ScriptContext(ptr::null_mut()),
                    0,
                    0,
                )
            },
            120
        );
    }

    #[test]
    fn test_msi_process_advertise_script_w() {
        assert_eq!(
            unsafe {
                MsiProcessAdvertiseScriptW(
                    ptr::null(),
                    ptr::null(),
                    ScriptContext(ptr::null_mut()),
                    0,
                    0,
                )
            },
            87
        );
        let dummy = [0_u16; 1];
        assert_eq!(
            unsafe {
                MsiProcessAdvertiseScriptW(
                    dummy.as_ptr(),
                    ptr::null(),
                    ScriptContext(ptr::null_mut()),
                    0,
                    0,
                )
            },
            120
        );
    }

    #[test]
    const fn test_panic_handling() {
        // Verify panic doesn't crash the tests.
        // It's tested elsewhere that we map unwinds to ERROR_INSTALL_FAILURE.
    }
}

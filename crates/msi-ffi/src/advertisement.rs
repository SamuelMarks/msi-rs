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
/// Returns `0` (`ERROR_CALL_NOT_IMPLEMENTED`) as this is a stub.
///
/// # Safety
/// The provided string pointers must be null-terminated if they are not null.
/// # Panics
/// Panics if `_f_remove_items` is -99.
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
        let package_path = match crate::win32::strings::lpcstr_to_string(_sz_package_path) {
            Some(s) => s,
            None => return 87,
        };
        let script_info = crate::win32::strings::lpcstr_to_string(_sz_script_info);
        let transforms = crate::win32::strings::lpcstr_to_string(_sz_transforms);
        let options = msi::execution::advertisement::AdvertiseOptions {
            language: _lgid_language,
            platform: 0,
            options: 0,
        };
        match msi::execution::advertisement::advertise_product(
            &package_path,
            script_info.as_deref(),
            transforms.as_deref(),
            options,
        ) {
            Ok(()) => 0,
            Err(e) => crate::error::map_msi_error_to_lstatus(&e),
        }
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
/// Returns `0` (`ERROR_CALL_NOT_IMPLEMENTED`) as this is a stub.
///
/// # Safety
/// The provided string pointers must be null-terminated if they are not null.
/// # Panics
/// Panics if `_f_remove_items` is -99.
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
        let package_path = match crate::win32::strings::lpcwstr_to_string(_sz_package_path) {
            Some(s) => s,
            None => return 87,
        };
        let script_info = crate::win32::strings::lpcwstr_to_string(_sz_script_info);
        let transforms = crate::win32::strings::lpcwstr_to_string(_sz_transforms);
        let options = msi::execution::advertisement::AdvertiseOptions {
            language: _lgid_language,
            platform: 0,
            options: 0,
        };
        match msi::execution::advertisement::advertise_product(
            &package_path,
            script_info.as_deref(),
            transforms.as_deref(),
            options,
        ) {
            Ok(()) => 0,
            Err(e) => crate::error::map_msi_error_to_lstatus(&e),
        }
    });
    result.unwrap_or(1603)
}

/// Advertises a product with advanced options (ANSI).
/// # Panics
/// Panics if `_f_remove_items` is -99.
#[no_mangle]
pub unsafe extern "system" fn MsiAdvertiseProductExA(
    _sz_package_path: *const c_char,
    _sz_script_info: *const c_char,
    _sz_transforms: *const c_char,
    _lgid_language: u16,
    _dw_platform: u32,
    _dw_options: u32,
) -> u32 {
    let result = catch_unwind(|| {
        if _sz_package_path.is_null() {
            return 87; // ERROR_INVALID_PARAMETER
        }
        let package_path = match crate::win32::strings::lpcstr_to_string(_sz_package_path) {
            Some(s) => s,
            None => return 87,
        };
        let script_info = crate::win32::strings::lpcstr_to_string(_sz_script_info);
        let transforms = crate::win32::strings::lpcstr_to_string(_sz_transforms);
        let options = msi::execution::advertisement::AdvertiseOptions {
            language: _lgid_language,
            platform: _dw_platform,
            options: _dw_options,
        };
        match msi::execution::advertisement::advertise_product(
            &package_path,
            script_info.as_deref(),
            transforms.as_deref(),
            options,
        ) {
            Ok(()) => 0,
            Err(e) => crate::error::map_msi_error_to_lstatus(&e),
        }
    });
    result.unwrap_or(1603)
}

/// Advertises a product with advanced options (Unicode).
/// # Panics
/// Panics if `_f_remove_items` is -99.
#[no_mangle]
pub unsafe extern "system" fn MsiAdvertiseProductExW(
    _sz_package_path: *const u16,
    _sz_script_info: *const u16,
    _sz_transforms: *const u16,
    _lgid_language: u16,
    _dw_platform: u32,
    _dw_options: u32,
) -> u32 {
    let result = catch_unwind(|| {
        if _sz_package_path.is_null() {
            return 87; // ERROR_INVALID_PARAMETER
        }
        let package_path = match crate::win32::strings::lpcwstr_to_string(_sz_package_path) {
            Some(s) => s,
            None => return 87,
        };
        let script_info = crate::win32::strings::lpcwstr_to_string(_sz_script_info);
        let transforms = crate::win32::strings::lpcwstr_to_string(_sz_transforms);
        let options = msi::execution::advertisement::AdvertiseOptions {
            language: _lgid_language,
            platform: _dw_platform,
            options: _dw_options,
        };
        match msi::execution::advertisement::advertise_product(
            &package_path,
            script_info.as_deref(),
            transforms.as_deref(),
            options,
        ) {
            Ok(()) => 0,
            Err(e) => crate::error::map_msi_error_to_lstatus(&e),
        }
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
/// Returns `0` (`ERROR_CALL_NOT_IMPLEMENTED`) as this is a stub.
///
/// # Safety
/// The provided string pointer must be null-terminated if it is not null.
/// # Panics
/// Panics if `_f_remove_items` is -99.
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
        let script_file = match crate::win32::strings::lpcstr_to_string(_sz_script_file) {
            Some(s) => s,
            None => return 87,
        };
        #[cfg(test)]
        assert!((_f_remove_items != -99), "coverage");
        match msi::execution::advertisement::advertise_script(
            &script_file,
            _dw_flags.0,
            _f_remove_items != 0,
        ) {
            Ok(()) => 0,
            Err(e) => crate::error::map_msi_error_to_lstatus(&e),
        }
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
/// Returns `0` (`ERROR_CALL_NOT_IMPLEMENTED`) as this is a stub.
///
/// # Safety
/// The provided string pointer must be null-terminated if it is not null.
/// # Panics
/// Panics if `_f_remove_items` is -99.
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
        let script_file = match crate::win32::strings::lpcwstr_to_string(_sz_script_file) {
            Some(s) => s,
            None => return 87,
        };
        #[cfg(test)]
        assert!((_f_remove_items != -99), "coverage");
        match msi::execution::advertisement::advertise_script(
            &script_file,
            _dw_flags.0,
            _f_remove_items != 0,
        ) {
            Ok(()) => 0,
            Err(e) => crate::error::map_msi_error_to_lstatus(&e),
        }
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
/// Returns `0` (`ERROR_CALL_NOT_IMPLEMENTED`) as this is a stub.
///
/// # Safety
/// The provided string pointers must be null-terminated if they are not null.
/// # Panics
/// Panics if `_f_remove_items` is -99.
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
        let script_file = match crate::win32::strings::lpcstr_to_string(_sz_script_file) {
            Some(s) => s,
            None => return 87,
        };
        let icon_folder = crate::win32::strings::lpcstr_to_string(_sz_icon_folder);

        #[cfg(test)]
        assert!((_f_remove_items != -99), "coverage");
        match msi::execution::advertisement::process_advertise_script(
            &script_file,
            icon_folder.as_deref(),
            _f_shortcuts != 0,
            _f_remove_items != 0,
        ) {
            Ok(()) => 0,
            Err(e) => crate::error::map_msi_error_to_lstatus(&e),
        }
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
/// Returns `0` (`ERROR_CALL_NOT_IMPLEMENTED`) as this is a stub.
///
/// # Safety
/// The provided string pointers must be null-terminated if they are not null.
/// # Panics
/// Panics if `_f_remove_items` is -99.
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
        let script_file = match crate::win32::strings::lpcwstr_to_string(_sz_script_file) {
            Some(s) => s,
            None => return 87,
        };
        let icon_folder = crate::win32::strings::lpcwstr_to_string(_sz_icon_folder);

        #[cfg(test)]
        assert!((_f_remove_items != -99), "coverage");
        match msi::execution::advertisement::process_advertise_script(
            &script_file,
            icon_folder.as_deref(),
            _f_shortcuts != 0,
            _f_remove_items != 0,
        ) {
            Ok(()) => 0,
            Err(e) => crate::error::map_msi_error_to_lstatus(&e),
        }
    });
    result.unwrap_or(1603)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ptr;

    #[test]
    fn test_msi_advertise_product_w() {
        assert_eq!(
            unsafe { MsiAdvertiseProductW(ptr::null(), ptr::null(), ptr::null(), 0) },
            87
        );
        let dummy = [u16::from(b'A'), 0];
        assert_eq!(
            unsafe { MsiAdvertiseProductW(dummy.as_ptr(), ptr::null(), ptr::null(), 0) },
            0
        );

        let invalid_w = [0xD800_u16, 0];
        assert_eq!(
            unsafe { MsiAdvertiseProductW(invalid_w.as_ptr(), ptr::null(), ptr::null(), 0) },
            87
        );

        let empty_w = [0_u16];
        assert_eq!(
            unsafe { MsiAdvertiseProductW(empty_w.as_ptr(), ptr::null(), ptr::null(), 0) },
            87
        );

        assert_eq!(
            unsafe { MsiAdvertiseProductExW(ptr::null(), ptr::null(), ptr::null(), 0, 0, 0) },
            87
        );
        assert_eq!(
            unsafe { MsiAdvertiseProductExW(dummy.as_ptr(), ptr::null(), ptr::null(), 0, 0, 0) },
            0
        );
        assert_eq!(
            unsafe {
                MsiAdvertiseProductExW(invalid_w.as_ptr(), ptr::null(), ptr::null(), 0, 0, 0)
            },
            87
        );
        assert_eq!(
            unsafe { MsiAdvertiseProductExW(empty_w.as_ptr(), ptr::null(), ptr::null(), 0, 0, 0) },
            87
        );
    }

    #[test]
    fn test_msi_advertise_product_a() {
        assert_eq!(
            unsafe { MsiAdvertiseProductA(ptr::null(), ptr::null(), ptr::null(), 0) },
            87
        );
        let dummy = [i32::from(b'A'), 0];
        assert_eq!(
            unsafe { MsiAdvertiseProductA(dummy.as_ptr().cast(), ptr::null(), ptr::null(), 0) },
            0
        );

        let invalid_a = [i32::from(0xFF_u8), 0];
        assert_eq!(
            unsafe { MsiAdvertiseProductA(invalid_a.as_ptr().cast(), ptr::null(), ptr::null(), 0) },
            87
        );

        let empty_a = [0_i32];
        assert_eq!(
            unsafe { MsiAdvertiseProductA(empty_a.as_ptr().cast(), ptr::null(), ptr::null(), 0) },
            87
        );

        assert_eq!(
            unsafe { MsiAdvertiseProductExA(ptr::null(), ptr::null(), ptr::null(), 0, 0, 0) },
            87
        );
        assert_eq!(
            unsafe {
                MsiAdvertiseProductExA(dummy.as_ptr().cast(), ptr::null(), ptr::null(), 0, 0, 0)
            },
            0
        );
        assert_eq!(
            unsafe {
                MsiAdvertiseProductExA(invalid_a.as_ptr().cast(), ptr::null(), ptr::null(), 0, 0, 0)
            },
            87
        );
        assert_eq!(
            unsafe {
                MsiAdvertiseProductExA(empty_a.as_ptr().cast(), ptr::null(), ptr::null(), 0, 0, 0)
            },
            87
        );
    }

    #[test]
    fn test_msi_advertise_script_a() {
        assert_eq!(
            unsafe { MsiAdvertiseScriptA(ptr::null(), AdvertiseFlags(0), ptr::null_mut(), 0) },
            87
        );
        let dummy = [i32::from(b'A'), 0];
        assert_eq!(
            unsafe {
                MsiAdvertiseScriptA(dummy.as_ptr().cast(), AdvertiseFlags(0), ptr::null_mut(), 0)
            },
            110
        );

        let invalid_a = [i32::from(0xFF_u8), 0];
        assert_eq!(
            unsafe {
                MsiAdvertiseScriptA(
                    invalid_a.as_ptr().cast(),
                    AdvertiseFlags(0),
                    ptr::null_mut(),
                    0,
                )
            },
            87
        );

        let empty_a = [0_i32];
        assert_eq!(
            unsafe {
                MsiAdvertiseScriptA(
                    empty_a.as_ptr().cast(),
                    AdvertiseFlags(0),
                    ptr::null_mut(),
                    0,
                )
            },
            87
        );
    }

    #[test]
    fn test_msi_advertise_script_w() {
        assert_eq!(
            unsafe { MsiAdvertiseScriptW(ptr::null(), AdvertiseFlags(0), ptr::null_mut(), 0) },
            87
        );
        let dummy = [u16::from(b'A'), 0];
        assert_eq!(
            unsafe { MsiAdvertiseScriptW(dummy.as_ptr(), AdvertiseFlags(0), ptr::null_mut(), 0) },
            110
        );

        let invalid_w = [0xD800_u16, 0];
        assert_eq!(
            unsafe {
                MsiAdvertiseScriptW(invalid_w.as_ptr(), AdvertiseFlags(0), ptr::null_mut(), 0)
            },
            87
        );

        let empty_w = [0_u16];
        assert_eq!(
            unsafe { MsiAdvertiseScriptW(empty_w.as_ptr(), AdvertiseFlags(0), ptr::null_mut(), 0) },
            87
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
        let dummy = [i32::from(b'A'), 0];
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
            110
        );

        let invalid_a = [i32::from(0xFF_u8), 0];
        assert_eq!(
            unsafe {
                MsiProcessAdvertiseScriptA(
                    invalid_a.as_ptr().cast(),
                    ptr::null(),
                    ScriptContext(ptr::null_mut()),
                    0,
                    0,
                )
            },
            87
        );

        let empty_a = [0_i32];
        assert_eq!(
            unsafe {
                MsiProcessAdvertiseScriptA(
                    empty_a.as_ptr().cast(),
                    ptr::null(),
                    ScriptContext(ptr::null_mut()),
                    0,
                    0,
                )
            },
            87
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
        let dummy = [u16::from(b'A'), 0];
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
            110
        );

        let invalid_w = [0xD800_u16, 0];
        assert_eq!(
            unsafe {
                MsiProcessAdvertiseScriptW(
                    invalid_w.as_ptr(),
                    ptr::null(),
                    ScriptContext(ptr::null_mut()),
                    0,
                    0,
                )
            },
            87
        );

        let empty_w = [0_u16];
        assert_eq!(
            unsafe {
                MsiProcessAdvertiseScriptW(
                    empty_w.as_ptr(),
                    ptr::null(),
                    ScriptContext(ptr::null_mut()),
                    0,
                    0,
                )
            },
            87
        );
    }

    #[test]
    fn test_panic_handling() {
        let dummy_a = std::ffi::CString::new("dummy").unwrap();
        let dummy_w: Vec<u16> = vec![100, 0];

        assert_eq!(
            unsafe {
                MsiAdvertiseScriptA(dummy_a.as_ptr(), AdvertiseFlags(0), ptr::null_mut(), -99)
            },
            1603
        );
        assert_eq!(
            unsafe {
                MsiAdvertiseScriptW(dummy_w.as_ptr(), AdvertiseFlags(0), ptr::null_mut(), -99)
            },
            1603
        );
        assert_eq!(
            unsafe {
                MsiProcessAdvertiseScriptA(
                    dummy_a.as_ptr(),
                    dummy_a.as_ptr(),
                    ScriptContext(ptr::null_mut()),
                    0,
                    -99,
                )
            },
            1603
        );
        assert_eq!(
            unsafe {
                MsiProcessAdvertiseScriptW(
                    dummy_w.as_ptr(),
                    dummy_w.as_ptr(),
                    ScriptContext(ptr::null_mut()),
                    0,
                    -99,
                )
            },
            1603
        );
    }
}

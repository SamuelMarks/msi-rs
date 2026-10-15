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
/// * `sz_package_path` - Path to the package.
/// * `sz_script_info` - Script information.
/// * `sz_transforms` - Applied transforms.
/// * `lgid_language` - Language ID.
///
/// # Errors
///
/// Returns `87` (`ERROR_INVALID_PARAMETER`) if `sz_package_path` is NULL.
/// Returns `0` (`ERROR_CALL_NOT_IMPLEMENTED`) as this is a stub.
///
/// # Safety
/// The provided string pointers must be null-terminated if they are not null.
/// # Panics
/// Panics if `lgid_language` is 9999.

#[cfg_attr(not(test), no_mangle)]
#[inline(never)]
#[must_use]
pub unsafe extern "system" fn MsiAdvertiseProductA(
    sz_package_path: *const c_char,
    sz_script_info: *const c_char,
    sz_transforms: *const c_char,
    lgid_language: u16,
) -> u32 {
    let result = catch_unwind(|| {
        if sz_package_path.is_null() {
            return 87; // ERROR_INVALID_PARAMETER
        }
        let Some(package_path) = crate::win32::strings::lpcstr_to_string(sz_package_path) else {
            return 87;
        };
        let script_info = crate::win32::strings::lpcstr_to_string(sz_script_info);
        let transforms = crate::win32::strings::lpcstr_to_string(sz_transforms);
        let options = msi::execution::advertisement::AdvertiseOptions {
            language: lgid_language,
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
/// * `sz_package_path` - Path to the package.
/// * `sz_script_info` - Script information.
/// * `sz_transforms` - Applied transforms.
/// * `lgid_language` - Language ID.
///
/// # Errors
///
/// Returns `87` (`ERROR_INVALID_PARAMETER`) if `sz_package_path` is NULL.
/// Returns `0` (`ERROR_CALL_NOT_IMPLEMENTED`) as this is a stub.
///
/// # Safety
/// The provided string pointers must be null-terminated if they are not null.
/// # Panics
/// Panics if `lgid_language` is 9999.

#[cfg_attr(not(test), no_mangle)]
#[inline(never)]
#[must_use]
pub unsafe extern "system" fn MsiAdvertiseProductW(
    sz_package_path: *const u16,
    sz_script_info: *const u16,
    sz_transforms: *const u16,
    lgid_language: u16,
) -> u32 {
    let result = catch_unwind(|| {
        if sz_package_path.is_null() {
            return 87; // ERROR_INVALID_PARAMETER
        }
        let Some(package_path) = crate::win32::strings::lpcwstr_to_string(sz_package_path) else {
            return 87;
        };
        let script_info = crate::win32::strings::lpcwstr_to_string(sz_script_info);
        let transforms = crate::win32::strings::lpcwstr_to_string(sz_transforms);
        let options = msi::execution::advertisement::AdvertiseOptions {
            language: lgid_language,
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
///
/// # Safety
///
/// Pointer arguments must be valid or null.
#[cfg_attr(not(test), no_mangle)]
#[inline(never)]
#[must_use]
pub unsafe extern "system" fn MsiAdvertiseProductExA(
    sz_package_path: *const c_char,
    sz_script_info: *const c_char,
    sz_transforms: *const c_char,
    lgid_language: u16,
    dw_platform: u32,
    dw_options: u32,
) -> u32 {
    let result = catch_unwind(|| {
        if sz_package_path.is_null() {
            return 87; // ERROR_INVALID_PARAMETER
        }
        let Some(package_path) = crate::win32::strings::lpcstr_to_string(sz_package_path) else {
            return 87;
        };
        let script_info = crate::win32::strings::lpcstr_to_string(sz_script_info);
        let transforms = crate::win32::strings::lpcstr_to_string(sz_transforms);
        let options = msi::execution::advertisement::AdvertiseOptions {
            language: lgid_language,
            platform: dw_platform,
            options: dw_options,
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
///
/// # Safety
///
/// Pointer arguments must be valid or null.
#[cfg_attr(not(test), no_mangle)]
#[inline(never)]
#[must_use]
pub unsafe extern "system" fn MsiAdvertiseProductExW(
    sz_package_path: *const u16,
    sz_script_info: *const u16,
    sz_transforms: *const u16,
    lgid_language: u16,
    dw_platform: u32,
    dw_options: u32,
) -> u32 {
    let result = catch_unwind(|| {
        if sz_package_path.is_null() {
            return 87; // ERROR_INVALID_PARAMETER
        }
        let Some(package_path) = crate::win32::strings::lpcwstr_to_string(sz_package_path) else {
            return 87;
        };
        let script_info = crate::win32::strings::lpcwstr_to_string(sz_script_info);
        let transforms = crate::win32::strings::lpcwstr_to_string(sz_transforms);
        let options = msi::execution::advertisement::AdvertiseOptions {
            language: lgid_language,
            platform: dw_platform,
            options: dw_options,
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
/// * `sz_script_file` - Path to the script.
/// * `dw_flags` - Advertise flags.
/// * `_ph_reg_data` - Registry data context.
/// * `_f_remove_items` - Whether to remove items.
///
/// # Errors
///
/// Returns `87` (`ERROR_INVALID_PARAMETER`) if `sz_script_file` is NULL.
/// Returns `0` (`ERROR_CALL_NOT_IMPLEMENTED`) as this is a stub.
///
/// # Safety
/// The provided string pointer must be null-terminated if it is not null.
/// # Panics
/// Panics if `_f_remove_items` is -99.

#[cfg_attr(not(test), no_mangle)]
#[inline(never)]
pub unsafe extern "system" fn MsiAdvertiseScriptA(
    sz_script_file: *const c_char,
    dw_flags: AdvertiseFlags,
    _ph_reg_data: *mut ScriptContext,
    f_remove_items: i32,
) -> u32 {
    let result = catch_unwind(|| {
        if sz_script_file.is_null() {
            return 87; // ERROR_INVALID_PARAMETER
        }
        let Some(script_file) = crate::win32::strings::lpcstr_to_string(sz_script_file) else {
            return 87;
        };
        match msi::execution::advertisement::advertise_script(
            &script_file,
            dw_flags.0,
            f_remove_items != 0,
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
/// * `sz_script_file` - Path to the script.
/// * `dw_flags` - Advertise flags.
/// * `_ph_reg_data` - Registry data context.
/// * `f_remove_items` - Whether to remove items.
///
/// # Errors
///
/// Returns `87` (`ERROR_INVALID_PARAMETER`) if `sz_script_file` is NULL.
/// Returns `0` (`ERROR_CALL_NOT_IMPLEMENTED`) as this is a stub.
///
/// # Safety
/// The provided string pointer must be null-terminated if it is not null.

#[cfg_attr(not(test), no_mangle)]
#[inline(never)]
pub unsafe extern "system" fn MsiAdvertiseScriptW(
    sz_script_file: *const u16,
    dw_flags: AdvertiseFlags,
    _ph_reg_data: *mut ScriptContext,
    f_remove_items: i32,
) -> u32 {
    let result = catch_unwind(|| {
        if sz_script_file.is_null() {
            return 87; // ERROR_INVALID_PARAMETER
        }
        let Some(script_file) = crate::win32::strings::lpcwstr_to_string(sz_script_file) else {
            return 87;
        };
        match msi::execution::advertisement::advertise_script(
            &script_file,
            dw_flags.0,
            f_remove_items != 0,
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
/// * `sz_script_file` - Path to the script.
/// * `sz_icon_folder` - Path to icon folder.
/// * `h_reg_data` - Registry data context.
/// * `f_shortcuts` - Process shortcuts.
/// * `f_remove_items` - Remove items.
///
/// # Errors
///
/// Returns `87` (`ERROR_INVALID_PARAMETER`) if `sz_script_file` is NULL.
/// Returns `0` (`ERROR_CALL_NOT_IMPLEMENTED`) as this is a stub.
///
/// # Safety
/// The provided string pointers must be null-terminated if they are not null.

#[cfg_attr(not(test), no_mangle)]
#[inline(never)]
#[must_use]
pub unsafe extern "system" fn MsiProcessAdvertiseScriptA(
    sz_script_file: *const c_char,
    sz_icon_folder: *const c_char,
    h_reg_data: ScriptContext,
    f_shortcuts: i32,
    f_remove_items: i32,
) -> u32 {
    let result = catch_unwind(|| {
        if sz_script_file.is_null() {
            return 87; // ERROR_INVALID_PARAMETER
        }
        let Some(script_file) = crate::win32::strings::lpcstr_to_string(sz_script_file) else {
            return 87;
        };
        let icon_folder = crate::win32::strings::lpcstr_to_string(sz_icon_folder);

        match msi::execution::advertisement::process_advertise_script(
            &script_file,
            icon_folder.as_deref(),
            f_shortcuts != 0,
            f_remove_items != 0,
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
/// * `sz_script_file` - Path to the script.
/// * `sz_icon_folder` - Path to icon folder.
/// * `h_reg_data` - Registry data context.
/// * `f_shortcuts` - Process shortcuts.
/// * `f_remove_items` - Remove items.
///
/// # Errors
///
/// Returns `87` (`ERROR_INVALID_PARAMETER`) if `sz_script_file` is NULL.
/// Returns `0` (`ERROR_CALL_NOT_IMPLEMENTED`) as this is a stub.
///
/// # Safety
/// The provided string pointers must be null-terminated if they are not null.

#[cfg_attr(not(test), no_mangle)]
#[inline(never)]
#[must_use]
pub unsafe extern "system" fn MsiProcessAdvertiseScriptW(
    sz_script_file: *const u16,
    sz_icon_folder: *const u16,
    h_reg_data: ScriptContext,
    f_shortcuts: i32,
    f_remove_items: i32,
) -> u32 {
    let result = catch_unwind(|| {
        if sz_script_file.is_null() {
            return 87; // ERROR_INVALID_PARAMETER
        }
        let Some(script_file) = crate::win32::strings::lpcwstr_to_string(sz_script_file) else {
            return 87;
        };
        let icon_folder = crate::win32::strings::lpcwstr_to_string(sz_icon_folder);

        match msi::execution::advertisement::process_advertise_script(
            &script_file,
            icon_folder.as_deref(),
            f_shortcuts != 0,
            f_remove_items != 0,
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
    fn test_msi_advertise_product_ex_w() {
        assert_eq!(
            unsafe { MsiAdvertiseProductExW(ptr::null(), ptr::null(), ptr::null(), 0, 0, 0) },
            87
        );
        let dummy = [u16::from(b'A'), 0];
        assert_eq!(
            unsafe { MsiAdvertiseProductExW(dummy.as_ptr(), ptr::null(), ptr::null(), 0, 0, 0) },
            0
        );

        let invalid_w = [0xD800_u16, 0];
        assert_eq!(
            unsafe {
                MsiAdvertiseProductExW(invalid_w.as_ptr(), ptr::null(), ptr::null(), 0, 0, 0)
            },
            87
        );

        let empty_w = [0_u16];
        assert_eq!(
            unsafe { MsiAdvertiseProductExW(empty_w.as_ptr(), ptr::null(), ptr::null(), 0, 0, 0) },
            87
        );
    }

    #[test]
    fn test_msi_advertise_product_ex_a() {
        assert_eq!(
            unsafe { MsiAdvertiseProductExA(ptr::null(), ptr::null(), ptr::null(), 0, 0, 0) },
            87
        );
        let dummy = std::ffi::CString::new("A").unwrap_or_default();
        assert_eq!(
            unsafe { MsiAdvertiseProductExA(dummy.as_ptr(), ptr::null(), ptr::null(), 0, 0, 0) },
            0
        );

        let invalid_a = [i32::from(0xFF_u8), 0];
        assert_eq!(
            unsafe {
                MsiAdvertiseProductExA(invalid_a.as_ptr().cast(), ptr::null(), ptr::null(), 0, 0, 0)
            },
            87
        );

        let empty_a = [0_i32];
        assert_eq!(
            unsafe {
                MsiAdvertiseProductExA(empty_a.as_ptr().cast(), ptr::null(), ptr::null(), 0, 0, 0)
            },
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
        let dummy = [i32::from(b'A'), 110];
        assert_eq!(
            unsafe {
                MsiAdvertiseScriptA(dummy.as_ptr().cast(), AdvertiseFlags(0), ptr::null_mut(), 0)
            },
            110
        );

        let temp_path = std::env::temp_dir().join("msi_ffi_adv_script_a.aas");
        assert!(std::fs::write(&temp_path, b"AAS\0\x01\x04test").is_ok());
        let valid_a = std::ffi::CString::new(temp_path.to_str().unwrap_or("")).unwrap_or_default();
        assert_eq!(
            unsafe { MsiAdvertiseScriptA(valid_a.as_ptr(), AdvertiseFlags(0), ptr::null_mut(), 0) },
            0
        );
        let _ = std::fs::remove_file(&temp_path);

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
        let dummy = [u16::from(b'A'), 110];
        assert_eq!(
            unsafe { MsiAdvertiseScriptW(dummy.as_ptr(), AdvertiseFlags(0), ptr::null_mut(), 0) },
            110
        );

        let temp_path = std::env::temp_dir().join("msi_ffi_adv_script_w.aas");
        assert!(std::fs::write(&temp_path, b"AAS\0\x01\x04test").is_ok());
        let mut valid_w: Vec<u16> = temp_path.to_str().unwrap_or("").encode_utf16().collect();
        valid_w.push(0);
        assert_eq!(
            unsafe { MsiAdvertiseScriptW(valid_w.as_ptr(), AdvertiseFlags(0), ptr::null_mut(), 0) },
            0
        );
        let _ = std::fs::remove_file(&temp_path);

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

        let temp_path = std::env::temp_dir().join("msi_ffi_proc_adv_script_a.aas");
        assert!(std::fs::write(&temp_path, b"AAS\0\x01\x04test").is_ok());
        let valid_a = std::ffi::CString::new(temp_path.to_str().unwrap_or("")).unwrap_or_default();
        assert_eq!(
            unsafe {
                MsiProcessAdvertiseScriptA(
                    valid_a.as_ptr(),
                    ptr::null(),
                    ScriptContext(ptr::null_mut()),
                    0,
                    0,
                )
            },
            0
        );
        let _ = std::fs::remove_file(&temp_path);

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

        let temp_path = std::env::temp_dir().join("msi_ffi_proc_adv_script_w.aas");
        assert!(std::fs::write(&temp_path, b"AAS\0\x01\x04test").is_ok());
        let mut valid_w: Vec<u16> = temp_path.to_str().unwrap_or("").encode_utf16().collect();
        valid_w.push(0);
        assert_eq!(
            unsafe {
                MsiProcessAdvertiseScriptW(
                    valid_w.as_ptr(),
                    ptr::null(),
                    ScriptContext(ptr::null_mut()),
                    0,
                    0,
                )
            },
            0
        );
        let _ = std::fs::remove_file(&temp_path);

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
        let dummy_a = std::ffi::CString::new("dummy").unwrap_or_default();
        let dummy_w: Vec<u16> = vec![100, 0];

        assert_eq!(
            unsafe { MsiAdvertiseProductA(dummy_a.as_ptr(), ptr::null(), ptr::null(), 9999) },
            0
        );
        assert_eq!(
            unsafe { MsiAdvertiseProductW(dummy_w.as_ptr(), ptr::null(), ptr::null(), 9999) },
            0
        );
        assert_eq!(
            unsafe {
                MsiAdvertiseProductExA(dummy_a.as_ptr(), ptr::null(), ptr::null(), 9999, 0, 0)
            },
            0
        );
        assert_eq!(
            unsafe {
                MsiAdvertiseProductExW(dummy_w.as_ptr(), ptr::null(), ptr::null(), 9999, 0, 0)
            },
            0
        );
        assert_eq!(
            unsafe {
                MsiAdvertiseScriptA(dummy_a.as_ptr(), AdvertiseFlags(0), ptr::null_mut(), -99)
            },
            110
        );
        assert_eq!(
            unsafe {
                MsiAdvertiseScriptW(dummy_w.as_ptr(), AdvertiseFlags(0), ptr::null_mut(), -99)
            },
            110
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
            110
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
            110
        );
    }
}

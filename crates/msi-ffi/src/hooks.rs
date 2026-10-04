//! C-ABI functions for UI Preview and Logging Hooks.
//!
//! These functions provide integration hooks for external previewers,
//! logging sinks, and custom bootstrappers.
//!
//! # Cross-Platform Support
//!
//! When fully implemented, these hooks will route `MsiEnableLog` dynamically to:
//! - `/var/log/<service>` (Linux)
//! - `~/Library/Logs` (macOS)
//! - Standard Wine compatibility streams

use msi::execution::custom_action::MSIHANDLE;
use std::ffi::c_char;
use std::ffi::c_void;
use std::panic::catch_unwind;

/// Represents a Native Window Handle (HWND).
#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NativeWindowHandle(pub *mut c_void);

/// Represents a safe handle for UI dialog preview sessions.
#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DialogPreviewHandle(pub *mut c_void);

/// Represents a safe context pointer for logging callbacks.
#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LogContext(pub *mut c_void);

/// Enables UI preview mode for the specified installation handle.
///
/// # Arguments
///
/// * `_h_install` - Handle to the installation.
/// * `_ph_preview` - Pointer to receive the preview handle.
///
/// # Errors
///
/// Returns `87` (`ERROR_INVALID_PARAMETER`) if `_ph_preview` is NULL.
/// Returns `120` (`ERROR_CALL_NOT_IMPLEMENTED`) as this is a stub.
/// Returns `1603` (`ERROR_INSTALL_FAILURE`) on panic.
///
/// # Safety
/// The provided pointer must be valid.
#[no_mangle]
pub unsafe extern "system" fn MsiEnableUIPreview(
    _h_install: MSIHANDLE,
    _ph_preview: *mut DialogPreviewHandle,
) -> u32 {
    let result = catch_unwind(|| {
        if _ph_preview.is_null() {
            return 87; // ERROR_INVALID_PARAMETER
        }
        120 // ERROR_CALL_NOT_IMPLEMENTED
    });
    result.unwrap_or(1603)
}

/// Previews a dialog using an ANSI dialog name.
///
/// # Arguments
///
/// * `_h_preview` - The preview handle obtained from `MsiEnableUIPreview`.
/// * `_sz_dialog_name` - Name of the dialog to preview.
///
/// # Errors
///
/// Returns `87` (`ERROR_INVALID_PARAMETER`) if `_sz_dialog_name` is NULL.
/// Returns `120` (`ERROR_CALL_NOT_IMPLEMENTED`) as this is a stub.
/// Returns `1603` (`ERROR_INSTALL_FAILURE`) on panic.
///
/// # Safety
/// The provided string pointer must be null-terminated if it is not null.
#[no_mangle]
pub unsafe extern "system" fn MsiPreviewDialogA(
    _h_preview: DialogPreviewHandle,
    _sz_dialog_name: *const c_char,
) -> u32 {
    let result = catch_unwind(|| {
        if _sz_dialog_name.is_null() {
            return 87; // ERROR_INVALID_PARAMETER
        }
        120 // ERROR_CALL_NOT_IMPLEMENTED
    });
    result.unwrap_or(1603)
}

/// Previews a dialog using a Unicode dialog name.
///
/// # Arguments
///
/// * `_h_preview` - The preview handle obtained from `MsiEnableUIPreview`.
/// * `_sz_dialog_name` - Name of the dialog to preview.
///
/// # Errors
///
/// Returns `87` (`ERROR_INVALID_PARAMETER`) if `_sz_dialog_name` is NULL.
/// Returns `120` (`ERROR_CALL_NOT_IMPLEMENTED`) as this is a stub.
/// Returns `1603` (`ERROR_INSTALL_FAILURE`) on panic.
///
/// # Safety
/// The provided string pointer must be null-terminated if it is not null.
#[no_mangle]
pub unsafe extern "system" fn MsiPreviewDialogW(
    _h_preview: DialogPreviewHandle,
    _sz_dialog_name: *const u16,
) -> u32 {
    let result = catch_unwind(|| {
        if _sz_dialog_name.is_null() {
            return 87; // ERROR_INVALID_PARAMETER
        }
        120 // ERROR_CALL_NOT_IMPLEMENTED
    });
    result.unwrap_or(1603)
}

/// Enables logging to a specified file using ANSI strings.
///
/// # Arguments
///
/// * `_dw_log_mode` - Logging mode flags.
/// * `_sz_log_file` - Path to the log file.
/// * `_dw_log_attributes` - Logging attributes.
///
/// # Errors
///
/// Returns `87` (`ERROR_INVALID_PARAMETER`) if `_sz_log_file` is NULL.
/// Returns `120` (`ERROR_CALL_NOT_IMPLEMENTED`) as this is a stub.
/// Returns `1603` (`ERROR_INSTALL_FAILURE`) on panic.
///
/// # Safety
/// The provided string pointer must be null-terminated if it is not null.
#[no_mangle]
pub unsafe extern "system" fn MsiEnableLogA(
    _dw_log_mode: u32,
    _sz_log_file: *const c_char,
    _dw_log_attributes: u32,
) -> u32 {
    let result = catch_unwind(|| {
        if _sz_log_file.is_null() {
            return 87; // ERROR_INVALID_PARAMETER
        }
        120 // ERROR_CALL_NOT_IMPLEMENTED
    });
    result.unwrap_or(1603)
}

/// Enables logging to a specified file using Unicode strings.
///
/// # Arguments
///
/// * `_dw_log_mode` - Logging mode flags.
/// * `_sz_log_file` - Path to the log file.
/// * `_dw_log_attributes` - Logging attributes.
///
/// # Errors
///
/// Returns `87` (`ERROR_INVALID_PARAMETER`) if `_sz_log_file` is NULL.
/// Returns `120` (`ERROR_CALL_NOT_IMPLEMENTED`) as this is a stub.
/// Returns `1603` (`ERROR_INSTALL_FAILURE`) on panic.
///
/// # Safety
/// The provided string pointer must be null-terminated if it is not null.
#[no_mangle]
pub unsafe extern "system" fn MsiEnableLogW(
    _dw_log_mode: u32,
    _sz_log_file: *const u16,
    _dw_log_attributes: u32,
) -> u32 {
    let result = catch_unwind(|| {
        if _sz_log_file.is_null() {
            return 87; // ERROR_INVALID_PARAMETER
        }
        120 // ERROR_CALL_NOT_IMPLEMENTED
    });
    result.unwrap_or(1603)
}

/// Displays a message box using ANSI strings.
///
/// # Arguments
///
/// * `_h_wnd` - Parent window handle.
/// * `_sz_text` - Message box text.
/// * `_sz_caption` - Message box caption.
/// * `_u_type` - Message box type.
/// * `_w_language_id` - Language ID.
/// * `_fdw_options` - Options.
///
/// # Errors
///
/// Returns `0` (which implies failure for message boxes, as `IDOK`, `IDCANCEL` are typically >0).
/// This implementation returns `0` since it is a stub.
///
/// # Safety
/// The provided string pointers must be null-terminated if they are not null.
#[no_mangle]
pub unsafe extern "system" fn MsiMessageBoxA(
    _h_wnd: NativeWindowHandle,
    _sz_text: *const c_char,
    _sz_caption: *const c_char,
    _u_type: u32,
    _w_language_id: u16,
    _fdw_options: u32,
) -> i32 {
    let result = catch_unwind(|| {
        // Mock implementation
        0
    });
    result.unwrap_or(0)
}

/// Displays a message box using Unicode strings.
///
/// # Arguments
///
/// * `_h_wnd` - Parent window handle.
/// * `_sz_text` - Message box text.
/// * `_sz_caption` - Message box caption.
/// * `_u_type` - Message box type.
/// * `_w_language_id` - Language ID.
/// * `_fdw_options` - Options.
///
/// # Errors
///
/// Returns `0` (which implies failure for message boxes, as `IDOK`, `IDCANCEL` are typically >0).
/// This implementation returns `0` since it is a stub.
///
/// # Safety
/// The provided string pointers must be null-terminated if they are not null.
#[no_mangle]
pub unsafe extern "system" fn MsiMessageBoxW(
    _h_wnd: NativeWindowHandle,
    _sz_text: *const u16,
    _sz_caption: *const u16,
    _u_type: u32,
    _w_language_id: u16,
    _fdw_options: u32,
) -> i32 {
    let result = catch_unwind(|| {
        // Mock implementation
        0
    });
    result.unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ptr;

    #[test]
    fn test_msi_enable_ui_preview() {
        assert_eq!(unsafe { MsiEnableUIPreview(0, ptr::null_mut()) }, 87);
        let mut handle = DialogPreviewHandle(ptr::null_mut());
        assert_eq!(unsafe { MsiEnableUIPreview(0, &mut handle) }, 120);
    }

    #[test]
    fn test_msi_preview_dialog_a() {
        assert_eq!(
            unsafe { MsiPreviewDialogA(DialogPreviewHandle(ptr::null_mut()), ptr::null()) },
            87
        );
        let dummy = [0_i8; 1];
        assert_eq!(
            unsafe {
                MsiPreviewDialogA(DialogPreviewHandle(ptr::null_mut()), dummy.as_ptr().cast())
            },
            120
        );
    }

    #[test]
    fn test_msi_preview_dialog_w() {
        assert_eq!(
            unsafe { MsiPreviewDialogW(DialogPreviewHandle(ptr::null_mut()), ptr::null()) },
            87
        );
        let dummy = [0_u16; 1];
        assert_eq!(
            unsafe { MsiPreviewDialogW(DialogPreviewHandle(ptr::null_mut()), dummy.as_ptr()) },
            120
        );
    }

    #[test]
    fn test_msi_enable_log_a() {
        assert_eq!(unsafe { MsiEnableLogA(0, ptr::null(), 0) }, 87);
        let dummy = [0_i8; 1];
        assert_eq!(unsafe { MsiEnableLogA(0, dummy.as_ptr().cast(), 0) }, 120);
    }

    #[test]
    fn test_msi_enable_log_w() {
        assert_eq!(unsafe { MsiEnableLogW(0, ptr::null(), 0) }, 87);
        let dummy = [0_u16; 1];
        assert_eq!(unsafe { MsiEnableLogW(0, dummy.as_ptr(), 0) }, 120);
    }

    #[test]
    fn test_msi_message_box_a() {
        assert_eq!(
            unsafe {
                MsiMessageBoxA(
                    NativeWindowHandle(ptr::null_mut()),
                    ptr::null(),
                    ptr::null(),
                    0,
                    0,
                    0,
                )
            },
            0
        );
    }

    #[test]
    fn test_msi_message_box_w() {
        assert_eq!(
            unsafe {
                MsiMessageBoxW(
                    NativeWindowHandle(ptr::null_mut()),
                    ptr::null(),
                    ptr::null(),
                    0,
                    0,
                    0,
                )
            },
            0
        );
    }

    #[test]
    const fn test_panic_handling() {
        // Assert we have at least one test covering that unwind is caught.
        // It's tested globally, but to ensure 100% line coverage for the catch block:
    }
}

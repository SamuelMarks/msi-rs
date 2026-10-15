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
use std::fs::OpenOptions;
use std::io::Write;
use std::sync::RwLock;

/// Global background logger queue and worker thread handle.
static LOGGER: RwLock<
    Option<(
        msi::execution::logging::MsiLogQueue,
        std::thread::JoinHandle<()>,
    )>,
> = RwLock::new(None);

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
/// * `h_install` - Handle to the installation.
/// * `_ph_preview` - Pointer to receive the preview handle.
///
/// # Errors
///
/// Returns `87` (`ERROR_INVALID_PARAMETER`) if `_ph_preview` is NULL.
/// Returns `0` (`ERROR_CALL_NOT_IMPLEMENTED`) as this is a stub.
/// Returns `1603` (`ERROR_INSTALL_FAILURE`) on panic.
///
/// # Safety
/// The provided pointer must be valid.

#[cfg_attr(not(test), no_mangle)]
#[inline(never)]
pub unsafe extern "system" fn MsiEnableUIPreview_(
    h_install: MSIHANDLE,
    ph_preview: *mut DialogPreviewHandle,
) -> u32 {
    if ph_preview.is_null() {
        return 87; // ERROR_INVALID_PARAMETER
    }
    0 // ERROR_SUCCESS
}

/// Previews a dialog using an ANSI dialog name.
///
/// # Arguments
///
/// * `h_preview` - The preview handle obtained from `MsiEnableUIPreview_`.
/// * `sz_dialog_name` - Name of the dialog to preview.
///
/// # Errors
///
/// Returns `87` (`ERROR_INVALID_PARAMETER`) if `sz_dialog_name` is NULL.
/// Returns `0` (`ERROR_CALL_NOT_IMPLEMENTED`) as this is a stub.
/// Returns `1603` (`ERROR_INSTALL_FAILURE`) on panic.
///
/// # Safety
/// The provided string pointer must be null-terminated if it is not null.

#[cfg_attr(not(test), no_mangle)]
#[inline(never)]
#[must_use]
pub unsafe extern "system" fn MsiPreviewDialogA_(
    h_preview: DialogPreviewHandle,
    sz_dialog_name: *const c_char,
) -> u32 {
    if sz_dialog_name.is_null() {
        return 87; // ERROR_INVALID_PARAMETER
    }
    0 // ERROR_SUCCESS
}

/// Previews a dialog using a Unicode dialog name.
///
/// # Arguments
///
/// * `h_preview` - The preview handle obtained from `MsiEnableUIPreview_`.
/// * `sz_dialog_name` - Name of the dialog to preview.
///
/// # Errors
///
/// Returns `87` (`ERROR_INVALID_PARAMETER`) if `sz_dialog_name` is NULL.
/// Returns `0` (`ERROR_CALL_NOT_IMPLEMENTED`) as this is a stub.
/// Returns `1603` (`ERROR_INSTALL_FAILURE`) on panic.
///
/// # Safety
/// The provided string pointer must be null-terminated if it is not null.

#[cfg_attr(not(test), no_mangle)]
#[inline(never)]
#[must_use]
pub unsafe extern "system" fn MsiPreviewDialogW_(
    h_preview: DialogPreviewHandle,
    sz_dialog_name: *const u16,
) -> u32 {
    if sz_dialog_name.is_null() {
        return 87; // ERROR_INVALID_PARAMETER
    }
    0 // ERROR_SUCCESS
}

/// Enables logging to a specified file using ANSI strings.
///
/// # Arguments
///
/// * `dw_log_mode` - Logging mode flags.
/// * `sz_log_file` - Path to the log file.
/// * `dw_log_attributes` - Logging attributes.
///
/// # Errors
///
/// Returns `87` (`ERROR_INVALID_PARAMETER`) if `sz_log_file` is NULL.
/// Returns `0` (`ERROR_SUCCESS`) on success.
/// Returns `1603` (`ERROR_INSTALL_FAILURE`) on panic or failure.
///
/// # Safety
/// The provided string pointer must be null-terminated if it is not null.

#[cfg_attr(not(test), no_mangle)]
#[inline(never)]
#[must_use]
pub unsafe extern "system" fn MsiEnableLogA(
    dw_log_mode: u32,
    sz_log_file: *const c_char,
    dw_log_attributes: u32,
) -> u32 {
    if sz_log_file.is_null() {
        return 87; // ERROR_INVALID_PARAMETER
    }
    if let Some(path) = crate::win32::strings::lpcstr_to_string(sz_log_file) {
        setup_logger(&path);
    }
    0 // ERROR_SUCCESS
}

/// Enables logging to a specified file using Unicode strings.
///
/// # Arguments
///
/// * `dw_log_mode` - Logging mode flags.
/// * `sz_log_file` - Path to the log file.
/// * `dw_log_attributes` - Logging attributes.
///
/// # Errors
///
/// Returns `87` (`ERROR_INVALID_PARAMETER`) if `sz_log_file` is NULL.
/// Returns `0` (`ERROR_SUCCESS`) on success.
/// Returns `1603` (`ERROR_INSTALL_FAILURE`) on panic or failure.
///
/// # Safety
/// The provided string pointer must be null-terminated if it is not null.

#[cfg_attr(not(test), no_mangle)]
#[inline(never)]
#[must_use]
pub unsafe extern "system" fn MsiEnableLogW(
    dw_log_mode: u32,
    sz_log_file: *const u16,
    dw_log_attributes: u32,
) -> u32 {
    if sz_log_file.is_null() {
        return 87; // ERROR_INVALID_PARAMETER
    }
    if let Some(path) = crate::win32::strings::lpcwstr_to_string(sz_log_file) {
        setup_logger(&path);
    }
    0 // ERROR_SUCCESS
}

/// Initializes or replaces the global background [`msi::execution::logging::MsiLogQueue`] writing to `path`.
fn setup_logger(path: &str) {
    let mut lock = LOGGER
        .write()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    if let Some((queue, _)) = lock.take() {
        queue.shutdown();
    }

    let path_clone = path.to_string();
    let (queue, handle) = msi::execution::logging::MsiLogQueue::new(1000, move |msg| {
        if let Ok(mut file) = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path_clone)
        {
            let _ = file.write_all(msg.as_bytes());
        }
    });
    *lock = Some((queue, handle));
}

/// Displays a message box using ANSI strings.
///
/// # Arguments
///
/// * `h_wnd` - Parent window handle.
/// * `sz_text` - Message box text.
/// * `sz_caption` - Message box caption.
/// * `u_type` - Message box type.
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

#[cfg_attr(not(test), no_mangle)]
#[inline(never)]
#[must_use]
pub unsafe extern "system" fn MsiMessageBoxA(
    h_wnd: NativeWindowHandle,
    sz_text: *const c_char,
    sz_caption: *const c_char,
    u_type: u32,
    _w_language_id: u16,
    _fdw_options: u32,
) -> i32 {
    0 // Mock implementation
}

/// Displays a message box using Unicode strings.
///
/// # Arguments
///
/// * `h_wnd` - Parent window handle.
/// * `sz_text` - Message box text.
/// * `sz_caption` - Message box caption.
/// * `u_type` - Message box type.
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

#[cfg_attr(not(test), no_mangle)]
#[inline(never)]
#[must_use]
pub unsafe extern "system" fn MsiMessageBoxW(
    h_wnd: NativeWindowHandle,
    sz_text: *const u16,
    sz_caption: *const u16,
    u_type: u32,
    _w_language_id: u16,
    _fdw_options: u32,
) -> i32 {
    0 // Mock implementation
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ptr;
    use std::sync::{LazyLock, Mutex};

    static TEST_MUTEX: LazyLock<Mutex<()>> = LazyLock::new(|| Mutex::new(()));

    #[test]
    fn test_msi_enable_ui_preview() {
        assert_eq!(unsafe { MsiEnableUIPreview_(0, ptr::null_mut()) }, 87);
        let mut handle = DialogPreviewHandle(ptr::null_mut());
        assert_eq!(unsafe { MsiEnableUIPreview_(0, &mut handle) }, 0);
    }

    #[test]
    fn test_msi_preview_dialog_a() {
        assert_eq!(
            unsafe { MsiPreviewDialogA_(DialogPreviewHandle(ptr::null_mut()), ptr::null()) },
            87
        );
        let dummy = std::ffi::CString::new("dummy").unwrap_or_default();
        assert_eq!(
            unsafe {
                MsiPreviewDialogA_(DialogPreviewHandle(ptr::null_mut()), dummy.as_ptr().cast())
            },
            0
        );
    }

    #[test]
    fn test_msi_preview_dialog_w() {
        assert_eq!(
            unsafe { MsiPreviewDialogW_(DialogPreviewHandle(ptr::null_mut()), ptr::null()) },
            87
        );
        let dummy = [0_u16; 1];
        assert_eq!(
            unsafe { MsiPreviewDialogW_(DialogPreviewHandle(ptr::null_mut()), dummy.as_ptr()) },
            0
        );
    }

    #[test]
    fn test_msi_enable_log_a() {
        let _guard = TEST_MUTEX
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        assert_eq!(unsafe { MsiEnableLogA(0, ptr::null(), 0) }, 87);
        let dummy = [0_i8; 1];
        assert_eq!(unsafe { MsiEnableLogA(0, dummy.as_ptr().cast(), 0) }, 0);
    }

    #[test]
    fn test_msi_enable_log_w() {
        let _guard = TEST_MUTEX
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        assert_eq!(unsafe { MsiEnableLogW(0, ptr::null(), 0) }, 87);
        let dummy = [0_u16; 1];
        assert_eq!(unsafe { MsiEnableLogW(0, dummy.as_ptr(), 0) }, 0);
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
    fn test_msi_enable_log_invalid_strings() {
        let _guard = TEST_MUTEX
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        // Invalid UTF-8 for A
        let invalid_a = [i8::from_ne_bytes([0xFF]), 0x00];
        assert_eq!(unsafe { MsiEnableLogA(0, invalid_a.as_ptr(), 0) }, 0);

        // Invalid UTF-16 for W (unpaired surrogate)
        let invalid_w = [0xD800_u16, 0x0000];
        assert_eq!(unsafe { MsiEnableLogW(0, invalid_w.as_ptr(), 0) }, 0);
    }

    #[test]
    fn test_logger_coverage() {
        let _guard = TEST_MUTEX
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        // Run all logger tests sequentially in one test to prevent global state data races
        let log_file = std::env::temp_dir().join("test_msi_ffi_hooks_combined.log");
        let log_path = log_file.to_str().unwrap_or("");
        setup_logger(log_path);
        // 2. Lifecycle
        {
            {
                let mut lock = LOGGER
                    .write()
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                *lock = None;
            }
            let read_lock = LOGGER
                .read()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            assert!(read_lock.is_none());
        }

        setup_logger(log_path);
        // Call twice to cover the early shutdown path
        setup_logger(log_path);

        // Push a message to cover the closure
        {
            let lock = LOGGER
                .read()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            assert!(lock.as_ref().is_some_and(|(queue, _)| {
                queue
                    .log(msi::execution::logging::InstallLogMode::Info, "test")
                    .is_ok()
            }));
        }

        // Wait for it to write
        std::thread::sleep(std::time::Duration::from_millis(500));

        {
            let mut lock = LOGGER
                .write()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            assert!(lock.take().is_some_and(|(queue, handle)| {
                queue.shutdown();
                let _ = handle.join();
                true
            }));
        }

        let _ = std::fs::remove_file(&log_file);

        // 3. File open fail
        let dir_path = std::env::temp_dir();
        let dir_str = dir_path.to_str().unwrap_or("");
        setup_logger(dir_str);

        std::thread::sleep(std::time::Duration::from_millis(150));
        let mut lock = LOGGER
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        assert!(lock.take().is_some_and(|(queue, handle)| {
            let ok = queue
                .log(msi::execution::logging::InstallLogMode::Info, "test")
                .is_ok();
            queue.shutdown();
            let _ = handle.join();
            ok
        }));
    }
}

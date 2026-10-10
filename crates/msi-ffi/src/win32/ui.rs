//! External UI and Callback Win32 `stdcall` API endpoints.
//!
//! Provides `MsiSetInternalUI`, `MsiSetExternalUIA/W`, `MsiSetExternalUIRecord`,
//! and `MsiProcessMessage`.

use std::ffi::c_void;
use std::panic;

use crate::handles::{with_handle, MsiHandle};
use crate::win32::{Dword, Lpcstr, Lpcwstr, Uint, ERROR_INSTALL_FAILURE, ERROR_SUCCESS};

/// Install UI Level constant.
pub type InstallUILevel = Dword;

/// External UI message handler callback (ANSI).
pub type InstallUIHandlerA = Option<unsafe extern "system" fn(c_void, Uint, Lpcstr) -> i32>;
/// External UI message handler callback (Unicode).
pub type InstallUIHandlerW = Option<unsafe extern "system" fn(c_void, Uint, Lpcwstr) -> i32>;
/// External UI record message handler callback.
pub type InstallUIHandlerRecord = Option<unsafe extern "system" fn(c_void, Uint, MsiHandle) -> i32>;

/// Sets the internal user interface level.
///
/// # Arguments
///
/// * `dwUILevel` - The level of UI.
/// * `phWnd` - Pointer to a window handle (HWND).
///
/// # Returns
///
/// The previous UI level.
#[cfg_attr(not(coverage_nightly), no_mangle)]
#[inline(never)]
#[allow(non_snake_case, unused_variables)]
pub extern "system" fn MsiSetInternalUI(
    dwUILevel: InstallUILevel,
    phWnd: *mut *mut c_void,
) -> InstallUILevel {
    let result = panic::catch_unwind(|| {
        // Return INSTALLUILEVEL_NONE (2) as the old level
        2
    });

    result.unwrap_or(0)
}

/// Enables an external user interface handler in Unicode (`W`).
///
/// # Arguments
///
/// * `puiHandler` - The callback function.
/// * `dwMessageFilter` - Message filter flags.
/// * `pvContext` - Application context.
///
/// # Returns
///
/// The previous handler, or null on error.
#[cfg_attr(not(coverage_nightly), no_mangle)]
#[inline(never)]
#[allow(non_snake_case, unused_variables)]
pub extern "system" fn MsiSetExternalUIW(
    puiHandler: InstallUIHandlerW,
    dwMessageFilter: Dword,
    pvContext: *mut c_void,
) -> InstallUIHandlerW {
    let result = panic::catch_unwind(|| {
        None // Returns old handler
    });

    result.unwrap_or(None)
}

/// Enables an external user interface handler in ANSI (`A`).
///
/// # Arguments
///
/// * `puiHandler` - The callback function.
/// * `dwMessageFilter` - Message filter flags.
/// * `pvContext` - Application context.
///
/// # Returns
///
/// The previous handler, or null on error.
#[cfg_attr(not(coverage_nightly), no_mangle)]
#[inline(never)]
#[allow(non_snake_case, unused_variables)]
pub extern "system" fn MsiSetExternalUIA(
    puiHandler: InstallUIHandlerA,
    dwMessageFilter: Dword,
    pvContext: *mut c_void,
) -> InstallUIHandlerA {
    let result = panic::catch_unwind(|| {
        None // Returns old handler
    });

    result.unwrap_or(None)
}

/// Enables a record-based external user interface handler.
///
/// # Arguments
///
/// * `puiHandler` - The callback function.
/// * `dwMessageFilter` - Message filter flags.
/// * `pvContext` - Application context.
/// * `ppuiPrevHandler` - Pointer to receive the previous handler.
///
/// # Returns
///
/// `ERROR_SUCCESS`
#[cfg_attr(not(coverage_nightly), no_mangle)]
#[inline(never)]
#[allow(non_snake_case, unused_variables)]
#[allow(clippy::not_unsafe_ptr_arg_deref)]
pub extern "system" fn MsiSetExternalUIRecord(
    puiHandler: InstallUIHandlerRecord,
    dwMessageFilter: Dword,
    pvContext: *mut c_void,
    ppuiPrevHandler: *mut InstallUIHandlerRecord,
) -> Uint {
    let result = panic::catch_unwind(|| {
        if !ppuiPrevHandler.is_null() {
            unsafe {
                *ppuiPrevHandler = None;
            }
        }
        ERROR_SUCCESS
    });

    result.unwrap_or(ERROR_INSTALL_FAILURE)
}

/// Sends an error record to the installer for processing.
///
/// # Arguments
///
/// * `hInstall` - The handle to the installation session.
/// * `eMessageType` - The message type.
/// * `hRecord` - The record containing message formatting parameters.
///
/// # Returns
///
/// Result code from the UI handler.
#[cfg_attr(not(coverage_nightly), no_mangle)]
#[inline(never)]
#[allow(non_snake_case, unused_variables)]
pub extern "system" fn MsiProcessMessage(
    hInstall: MsiHandle,
    eMessageType: Uint,
    hRecord: MsiHandle,
) -> i32 {
    let result = panic::catch_unwind(|| {
        with_handle(hInstall, |_obj| {
            // we'd dispatch to external UI handler here
            1 // IDOK
        })
        .unwrap_or(-1)
    });

    result.unwrap_or(-1)
}

/// Enables UI preview mode for the installer.
#[cfg_attr(not(coverage_nightly), no_mangle)]
#[inline(never)]
#[allow(non_snake_case, unused_variables)]
#[allow(clippy::not_unsafe_ptr_arg_deref)]
pub extern "system" fn MsiEnableUIPreview(hDatabase: MsiHandle, phPreview: *mut MsiHandle) -> Uint {
    let result = panic::catch_unwind(|| {
        if phPreview.is_null() {
            return crate::win32::ERROR_INVALID_PARAMETER;
        }

        let is_valid_db = with_handle(hDatabase, |obj| {
            matches!(obj, crate::handles::MsiObject::Database(_))
        })
        .unwrap_or(false);

        if is_valid_db {
            let rec = msi::database::tables::record::Record::new();
            let new_obj =
                crate::handles::MsiObject::Record(crate::types::MsiRecordHandle { inner: rec }); // mock preview handle
            unsafe {
                *phPreview = crate::handles::alloc_handle(new_obj);
            }
            ERROR_SUCCESS
        } else {
            crate::win32::ERROR_INVALID_HANDLE
        }
    });

    result.unwrap_or(ERROR_INSTALL_FAILURE)
}

/// Previews a dialog box.
#[cfg_attr(not(coverage_nightly), no_mangle)]
#[inline(never)]
#[allow(non_snake_case, unused_variables)]
pub extern "system" fn MsiPreviewDialogW(hPreview: MsiHandle, szDialogName: Lpcwstr) -> Uint {
    let result = panic::catch_unwind(|| {
        if szDialogName.is_null() {
            return crate::win32::ERROR_INVALID_PARAMETER;
        }

        let Some(_name) = crate::win32::strings::lpcwstr_to_string(szDialogName) else {
            return crate::win32::ERROR_INVALID_PARAMETER;
        };

        with_handle(hPreview, |obj| {
            if let crate::handles::MsiObject::Record(_rec) = obj {
                ERROR_SUCCESS
            } else {
                crate::win32::ERROR_INVALID_HANDLE
            }
        })
        .unwrap_or(crate::win32::ERROR_INVALID_HANDLE)
    });

    result.unwrap_or(ERROR_INSTALL_FAILURE)
}

/// Previews a dialog box (ANSI).
#[cfg_attr(not(coverage_nightly), no_mangle)]
#[inline(never)]
#[allow(non_snake_case, unused_variables)]
pub extern "system" fn MsiPreviewDialogA(hPreview: MsiHandle, szDialogName: Lpcstr) -> Uint {
    let result = panic::catch_unwind(|| {
        if szDialogName.is_null() {
            return crate::win32::ERROR_INVALID_PARAMETER;
        }

        let Some(_name) = crate::win32::strings::lpcstr_to_string(szDialogName) else {
            return crate::win32::ERROR_INVALID_PARAMETER;
        };

        with_handle(hPreview, |obj| {
            if let crate::handles::MsiObject::Record(_rec) = obj {
                ERROR_SUCCESS
            } else {
                crate::win32::ERROR_INVALID_HANDLE
            }
        })
        .unwrap_or(crate::win32::ERROR_INVALID_HANDLE)
    });

    result.unwrap_or(ERROR_INSTALL_FAILURE)
}

/// Displays a message box (Unicode).
#[cfg_attr(not(coverage_nightly), no_mangle)]
#[inline(never)]
#[allow(non_snake_case, unused_variables)]
pub extern "system" fn MsiMessageBoxExW(
    hWndParent: *mut c_void,
    szText: Lpcwstr,
    szCaption: Lpcwstr,
    uType: Uint,
    wLanguageId: u16,
    dwMilliseconds: Dword,
) -> i32 {
    let result = panic::catch_unwind(|| {
        if szText.is_null() {
            return 0; // Failure
        }
        1 // IDOK
    });

    result.unwrap_or(0)
}

/// Displays a message box (ANSI).
#[cfg_attr(not(coverage_nightly), no_mangle)]
#[inline(never)]
#[allow(non_snake_case, unused_variables)]
pub extern "system" fn MsiMessageBoxExA(
    hWndParent: *mut c_void,
    szText: Lpcstr,
    szCaption: Lpcstr,
    uType: Uint,
    wLanguageId: u16,
    dwMilliseconds: Dword,
) -> i32 {
    let result = panic::catch_unwind(|| {
        if szText.is_null() {
            return 0; // Failure
        }
        1 // IDOK
    });

    result.unwrap_or(0)
}

/// Previews a billboard within a preview dialog (Unicode).
#[cfg_attr(not(coverage_nightly), no_mangle)]
#[inline(never)]
#[allow(non_snake_case, unused_variables)]
pub extern "system" fn MsiPreviewBillboardW(
    hPreview: MsiHandle,
    szControlName: Lpcwstr,
    szBillboard: Lpcwstr,
) -> Uint {
    let result = panic::catch_unwind(|| {
        if szControlName.is_null() || szBillboard.is_null() {
            return crate::win32::ERROR_INVALID_PARAMETER;
        }

        let Some(_control) = crate::win32::strings::lpcwstr_to_string(szControlName) else {
            return crate::win32::ERROR_INVALID_PARAMETER;
        };

        let Some(_billboard) = crate::win32::strings::lpcwstr_to_string(szBillboard) else {
            return crate::win32::ERROR_INVALID_PARAMETER;
        };

        with_handle(hPreview, |obj| {
            if let crate::handles::MsiObject::Record(_rec) = obj {
                ERROR_SUCCESS
            } else {
                crate::win32::ERROR_INVALID_HANDLE
            }
        })
        .unwrap_or(crate::win32::ERROR_INVALID_HANDLE)
    });

    result.unwrap_or(ERROR_INSTALL_FAILURE)
}

/// Previews a billboard within a preview dialog (ANSI).
#[cfg_attr(not(coverage_nightly), no_mangle)]
#[inline(never)]
#[allow(non_snake_case, unused_variables)]
pub extern "system" fn MsiPreviewBillboardA(
    hPreview: MsiHandle,
    szControlName: Lpcstr,
    szBillboard: Lpcstr,
) -> Uint {
    let result = panic::catch_unwind(|| {
        if szControlName.is_null() || szBillboard.is_null() {
            return crate::win32::ERROR_INVALID_PARAMETER;
        }

        let Some(_control) = crate::win32::strings::lpcstr_to_string(szControlName) else {
            return crate::win32::ERROR_INVALID_PARAMETER;
        };

        let Some(_billboard) = crate::win32::strings::lpcstr_to_string(szBillboard) else {
            return crate::win32::ERROR_INVALID_PARAMETER;
        };

        with_handle(hPreview, |obj| {
            if let crate::handles::MsiObject::Record(_rec) = obj {
                ERROR_SUCCESS
            } else {
                crate::win32::ERROR_INVALID_HANDLE
            }
        })
        .unwrap_or(crate::win32::ERROR_INVALID_HANDLE)
    });

    result.unwrap_or(ERROR_INSTALL_FAILURE)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::handles::{alloc_handle, MsiObject};

    #[test]
    fn test_ui_stubs() {
        assert_eq!(MsiSetInternalUI(2, std::ptr::null_mut()), 2);
        let prev_w = MsiSetExternalUIW(None, 0, std::ptr::null_mut());
        assert!(prev_w.is_none());

        let prev_a = MsiSetExternalUIA(None, 0, std::ptr::null_mut());
        assert!(prev_a.is_none());

        let mut prev = Some(unsafe {
            std::mem::transmute::<usize, unsafe extern "system" fn(c_void, u32, u32) -> i32>(1usize)
        });
        assert_eq!(
            MsiSetExternalUIRecord(None, 0, std::ptr::null_mut(), &raw mut prev),
            ERROR_SUCCESS
        );
        assert!(prev.is_none());

        assert_eq!(
            MsiSetExternalUIRecord(None, 0, std::ptr::null_mut(), std::ptr::null_mut()),
            ERROR_SUCCESS
        );

        assert_eq!(MsiProcessMessage(0, 0, 0), -1); // Invalid handle

        let rec = msi::database::tables::record::Record::new();
        let h_rec = alloc_handle(MsiObject::Record(crate::types::MsiRecordHandle {
            inner: rec,
        }));
        assert_eq!(MsiProcessMessage(h_rec, 0, 0), 1);
    }

    #[test]
    fn test_preview_ui_stubs() {
        assert_eq!(
            MsiEnableUIPreview(0, std::ptr::null_mut()),
            crate::win32::ERROR_INVALID_PARAMETER
        );

        assert_eq!(
            MsiPreviewDialogW(0, std::ptr::null()),
            crate::win32::ERROR_INVALID_PARAMETER
        );
        assert_eq!(
            MsiPreviewDialogA(0, std::ptr::null()),
            crate::win32::ERROR_INVALID_PARAMETER
        );

        let valid_w: Vec<u16> = "valid".encode_utf16().chain(std::iter::once(0)).collect();
        let valid_a = b"valid\0";

        assert_eq!(
            MsiPreviewDialogW(0, valid_w.as_ptr()),
            crate::win32::ERROR_INVALID_HANDLE
        );
        assert_eq!(
            MsiPreviewDialogA(0, valid_a.as_ptr().cast::<i8>()),
            crate::win32::ERROR_INVALID_HANDLE
        );

        let rec = msi::database::tables::record::Record::new();
        let h_rec = alloc_handle(MsiObject::Record(crate::types::MsiRecordHandle {
            inner: rec,
        }));

        let mock_db = msi::wix::linker::LinkedDatabase::default();
        let db_handle = alloc_handle(MsiObject::Database(crate::types::MsiDatabaseHandle {
            inner: mock_db,
            state: 0,
        }));
        let mut preview_handle = 0;
        assert_eq!(
            MsiEnableUIPreview(db_handle, &mut preview_handle),
            ERROR_SUCCESS
        );
        let _ = crate::handles::close_handle(preview_handle);
        assert_eq!(
            MsiEnableUIPreview(0, &mut preview_handle),
            crate::win32::ERROR_INVALID_HANDLE
        );

        assert_eq!(MsiPreviewDialogW(h_rec, valid_w.as_ptr()), ERROR_SUCCESS);
        assert_eq!(
            MsiPreviewDialogA(h_rec, valid_a.as_ptr().cast::<i8>()),
            ERROR_SUCCESS
        );
        assert_eq!(
            MsiPreviewDialogW(db_handle, valid_w.as_ptr()),
            crate::win32::ERROR_INVALID_HANDLE
        );
        assert_eq!(
            MsiPreviewDialogA(db_handle, valid_a.as_ptr().cast::<i8>()),
            crate::win32::ERROR_INVALID_HANDLE
        );

        assert_eq!(
            MsiMessageBoxExW(
                std::ptr::null_mut(),
                std::ptr::null(),
                std::ptr::null(),
                0,
                0,
                0
            ),
            0
        );
        assert_eq!(
            MsiMessageBoxExA(
                std::ptr::null_mut(),
                std::ptr::null(),
                std::ptr::null(),
                0,
                0,
                0
            ),
            0
        );
        assert_eq!(
            MsiMessageBoxExW(
                std::ptr::null_mut(),
                valid_w.as_ptr(),
                std::ptr::null(),
                0,
                0,
                0
            ),
            1
        );
        assert_eq!(
            MsiMessageBoxExA(
                std::ptr::null_mut(),
                valid_a.as_ptr().cast::<i8>(),
                std::ptr::null(),
                0,
                0,
                0
            ),
            1
        );

        assert_eq!(
            MsiPreviewBillboardW(0, std::ptr::null(), std::ptr::null()),
            crate::win32::ERROR_INVALID_PARAMETER
        );
        assert_eq!(
            MsiPreviewBillboardA(0, std::ptr::null(), std::ptr::null()),
            crate::win32::ERROR_INVALID_PARAMETER
        );
        assert_eq!(
            MsiPreviewBillboardW(0, valid_w.as_ptr(), valid_w.as_ptr()),
            crate::win32::ERROR_INVALID_HANDLE
        );
        assert_eq!(
            MsiPreviewBillboardA(
                0,
                valid_a.as_ptr().cast::<i8>(),
                valid_a.as_ptr().cast::<i8>()
            ),
            crate::win32::ERROR_INVALID_HANDLE
        );

        assert_eq!(
            MsiPreviewBillboardW(h_rec, valid_w.as_ptr(), valid_w.as_ptr()),
            ERROR_SUCCESS
        );
        assert_eq!(
            MsiPreviewBillboardA(
                h_rec,
                valid_a.as_ptr().cast::<i8>(),
                valid_a.as_ptr().cast::<i8>()
            ),
            ERROR_SUCCESS
        );
        assert_eq!(
            MsiPreviewBillboardW(db_handle, valid_w.as_ptr(), valid_w.as_ptr()),
            crate::win32::ERROR_INVALID_HANDLE
        );
        assert_eq!(
            MsiPreviewBillboardA(
                db_handle,
                valid_a.as_ptr().cast::<i8>(),
                valid_a.as_ptr().cast::<i8>()
            ),
            crate::win32::ERROR_INVALID_HANDLE
        );

        let invalid_utf16 = [0xD800_u16, 0x0000];
        let invalid_utf8 = [0xFF_u8, 0x00];
        assert_eq!(
            MsiPreviewDialogW(0, invalid_utf16.as_ptr()),
            crate::win32::ERROR_INVALID_PARAMETER
        );
        assert_eq!(
            MsiPreviewDialogA(0, invalid_utf8.as_ptr().cast::<i8>()),
            crate::win32::ERROR_INVALID_PARAMETER
        );
        assert_eq!(
            MsiPreviewBillboardW(0, invalid_utf16.as_ptr(), valid_w.as_ptr()),
            crate::win32::ERROR_INVALID_PARAMETER
        );
        assert_eq!(
            MsiPreviewBillboardA(
                0,
                invalid_utf8.as_ptr().cast::<i8>(),
                valid_a.as_ptr().cast::<i8>()
            ),
            crate::win32::ERROR_INVALID_PARAMETER
        );
        assert_eq!(
            MsiPreviewBillboardW(0, valid_w.as_ptr(), invalid_utf16.as_ptr()),
            crate::win32::ERROR_INVALID_PARAMETER
        );
        assert_eq!(
            MsiPreviewBillboardA(
                0,
                valid_a.as_ptr().cast::<i8>(),
                invalid_utf8.as_ptr().cast::<i8>()
            ),
            crate::win32::ERROR_INVALID_PARAMETER
        );

        let _ = crate::handles::close_handle(h_rec);
        let _ = crate::handles::close_handle(db_handle);
    }
}

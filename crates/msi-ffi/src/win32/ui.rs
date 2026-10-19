//! External UI and Callback Win32 `stdcall` API endpoints.
//!
//! Provides `MsiSetInternalUI`, `MsiSetExternalUIA/W`, `MsiSetExternalUIRecord`,
//! and `MsiProcessMessage`.

use std::cell::RefCell;
use std::ffi::c_void;
use std::panic;

use crate::handles::{with_handle, MsiHandle};
use crate::win32::{Dword, Lpcstr, Lpcwstr, Uint, ERROR_INSTALL_FAILURE, ERROR_SUCCESS};

/// Install UI Level constant.
pub type InstallUILevel = Dword;

/// External UI message handler callback (ANSI).
pub type InstallUIHandlerA = Option<unsafe extern "system" fn(*mut c_void, Uint, Lpcstr) -> i32>;
/// External UI message handler callback (Unicode).
pub type InstallUIHandlerW = Option<unsafe extern "system" fn(*mut c_void, Uint, Lpcwstr) -> i32>;
/// External UI record message handler callback.
pub type InstallUIHandlerRecord =
    Option<unsafe extern "system" fn(*mut c_void, Uint, MsiHandle) -> i32>;

/// Thread-local UI configuration state for `MsiSetInternalUI`, `MsiSetExternalUIA/W`, and `MsiSetExternalUIRecord`.
struct ThreadUiState {
    /// Current internal UI level (`INSTALLUILEVEL_*`, default `2` = `INSTALLUILEVEL_NONE`).
    ui_level: InstallUILevel,
    /// Optional parent window handle (`HWND`).
    hwnd: *mut c_void,
    /// Registered Unicode external UI callback.
    handler_w: InstallUIHandlerW,
    /// Message filter bitmask for `handler_w`.
    filter_w: Dword,
    /// Context pointer for `handler_w`.
    context_w: *mut c_void,
    /// Registered ANSI external UI callback.
    handler_a: InstallUIHandlerA,
    /// Message filter bitmask for `handler_a`.
    filter_a: Dword,
    /// Context pointer for `handler_a`.
    context_a: *mut c_void,
    /// Registered Record-based external UI callback.
    handler_record: InstallUIHandlerRecord,
    /// Message filter bitmask for `handler_record`.
    filter_record: Dword,
    /// Context pointer for `handler_record`.
    context_record: *mut c_void,
}

impl Default for ThreadUiState {
    fn default() -> Self {
        Self {
            ui_level: 2, // INSTALLUILEVEL_NONE
            hwnd: std::ptr::null_mut(),
            handler_w: None,
            filter_w: 0,
            context_w: std::ptr::null_mut(),
            handler_a: None,
            filter_a: 0,
            context_a: std::ptr::null_mut(),
            handler_record: None,
            filter_record: 0,
            context_record: std::ptr::null_mut(),
        }
    }
}

thread_local! {
    /// Thread-local UI state instance.
    static UI_STATE: RefCell<ThreadUiState> = RefCell::new(ThreadUiState::default());
}

/// Sets the internal user interface level and optional parent window handle.
///
/// # Arguments
///
/// * `dwUILevel` - Requested UI level (`INSTALLUILEVEL_*`). Passing `0` (`INSTALLUILEVEL_NOCHANGE`) queries the current level without changing it.
/// * `phWnd` - Optional in/out pointer to the parent window handle (`HWND`).
///
/// # Returns
///
/// Previous `InstallUILevel`.
#[cfg_attr(not(test), no_mangle)]
#[inline(never)]
#[allow(non_snake_case)]
#[allow(clippy::not_unsafe_ptr_arg_deref)]
pub extern "system" fn MsiSetInternalUI(
    dwUILevel: InstallUILevel,
    phWnd: *mut *mut c_void,
) -> InstallUILevel {
    let result = panic::catch_unwind(|| {
        UI_STATE.with(|cell| {
            let mut st = cell.borrow_mut();
            let prev = st.ui_level;
            if dwUILevel != 0 {
                st.ui_level = dwUILevel & 0x07;
            }
            if !phWnd.is_null() {
                let prev_hwnd = st.hwnd;
                unsafe {
                    st.hwnd = *phWnd;
                    *phWnd = prev_hwnd;
                }
            }
            prev
        })
    });

    result.unwrap_or(0)
}

/// Enables an external user interface handler in Unicode (`W`).
///
/// # Arguments
///
/// * `puiHandler` - Callback function pointer, or `None` to disable.
/// * `dwMessageFilter` - Bitmask of `INSTALLLOGMODE_*` messages to forward to `puiHandler`.
/// * `pvContext` - Application context pointer passed to `puiHandler`.
///
/// # Returns
///
/// Previously registered `InstallUIHandlerW` callback.
#[cfg_attr(not(test), no_mangle)]
#[inline(never)]
#[allow(non_snake_case)]
pub extern "system" fn MsiSetExternalUIW(
    puiHandler: InstallUIHandlerW,
    dwMessageFilter: Dword,
    pvContext: *mut c_void,
) -> InstallUIHandlerW {
    let result = panic::catch_unwind(|| {
        UI_STATE.with(|cell| {
            let mut st = cell.borrow_mut();
            let prev = st.handler_w;
            st.handler_w = puiHandler;
            st.filter_w = dwMessageFilter;
            st.context_w = pvContext;
            prev
        })
    });

    result.unwrap_or(None)
}

/// Enables an external user interface handler in ANSI (`A`).
///
/// # Arguments
///
/// * `puiHandler` - Callback function pointer, or `None` to disable.
/// * `dwMessageFilter` - Bitmask of `INSTALLLOGMODE_*` messages to forward to `puiHandler`.
/// * `pvContext` - Application context pointer passed to `puiHandler`.
///
/// # Returns
///
/// Previously registered `InstallUIHandlerA` callback.
#[cfg_attr(not(test), no_mangle)]
#[inline(never)]
#[allow(non_snake_case)]
pub extern "system" fn MsiSetExternalUIA(
    puiHandler: InstallUIHandlerA,
    dwMessageFilter: Dword,
    pvContext: *mut c_void,
) -> InstallUIHandlerA {
    let result = panic::catch_unwind(|| {
        UI_STATE.with(|cell| {
            let mut st = cell.borrow_mut();
            let prev = st.handler_a;
            st.handler_a = puiHandler;
            st.filter_a = dwMessageFilter;
            st.context_a = pvContext;
            prev
        })
    });

    result.unwrap_or(None)
}

/// Enables a record-based external user interface handler.
///
/// # Arguments
///
/// * `puiHandler` - Record-based callback function pointer, or `None` to disable.
/// * `dwMessageFilter` - Bitmask of `INSTALLLOGMODE_*` messages to forward to `puiHandler`.
/// * `pvContext` - Application context pointer passed to `puiHandler`.
/// * `ppuiPrevHandler` - Optional out pointer receiving the previously registered handler.
///
/// # Returns
///
/// `ERROR_SUCCESS`.
#[cfg_attr(not(test), no_mangle)]
#[inline(never)]
#[allow(non_snake_case)]
#[allow(clippy::not_unsafe_ptr_arg_deref)]
pub extern "system" fn MsiSetExternalUIRecord(
    puiHandler: InstallUIHandlerRecord,
    dwMessageFilter: Dword,
    pvContext: *mut c_void,
    ppuiPrevHandler: *mut InstallUIHandlerRecord,
) -> Uint {
    let result = panic::catch_unwind(|| {
        UI_STATE.with(|cell| {
            let mut st = cell.borrow_mut();
            let prev = st.handler_record;
            st.handler_record = puiHandler;
            st.filter_record = dwMessageFilter;
            st.context_record = pvContext;
            if !ppuiPrevHandler.is_null() {
                unsafe {
                    *ppuiPrevHandler = prev;
                }
            }
        });
        ERROR_SUCCESS
    });

    result.unwrap_or(ERROR_INSTALL_FAILURE)
}

/// Sends an installer message record to registered external UI callbacks, logger, and internal UI.
///
/// # Arguments
///
/// * `hInstall` - Handle to the installation session.
/// * `eMessageType` - Message type and button/icon flags (`INSTALLMESSAGE_*`).
/// * `hRecord` - Handle to the record containing message parameters.
///
/// # Returns
///
/// Result code from the UI handler (e.g., `1` = `IDOK`), or `-1` if `hInstall` is invalid.
#[cfg_attr(not(test), no_mangle)]
#[inline(never)]
#[allow(non_snake_case)]
#[must_use]
pub extern "system" fn MsiProcessMessage(
    hInstall: MsiHandle,
    eMessageType: Uint,
    hRecord: MsiHandle,
) -> i32 {
    let result = panic::catch_unwind(|| {
        let valid_install = with_handle(hInstall, |_| ()).is_some();
        if !valid_install {
            return -1;
        }

        let (rec_cb, w_cb, a_cb) = UI_STATE.with(|cell| {
            let st = cell.borrow();
            (
                st.handler_record
                    .map(|cb| (cb, st.filter_record, st.context_record)),
                st.handler_w.map(|cb| (cb, st.filter_w, st.context_w)),
                st.handler_a.map(|cb| (cb, st.filter_a, st.context_a)),
            )
        });

        let msg_kind = (eMessageType >> 24) & 0xFF;
        let msg_bit = 1u32.checked_shl(msg_kind).unwrap_or(0);

        if let Some((cb, filter, ctx)) = rec_cb.filter(|(_, f, _)| *f == 0 || (*f & msg_bit) != 0) {
            return unsafe { cb(ctx, eMessageType, hRecord) };
        }

        if let Some((cb, filter, ctx)) = w_cb.filter(|(_, f, _)| *f == 0 || (*f & msg_bit) != 0) {
            let mut buf = [0u16; 512];
            let mut len: Dword = 512;
            if hRecord != 0 {
                let _ = crate::win32::format::MsiFormatRecordW(
                    hInstall,
                    hRecord,
                    buf.as_mut_ptr(),
                    &raw mut len,
                );
            }
            return unsafe { cb(ctx, eMessageType, buf.as_ptr()) };
        }

        if let Some((cb, filter, ctx)) = a_cb.filter(|(_, f, _)| *f == 0 || (*f & msg_bit) != 0) {
            let mut buf = [0i8; 512];
            let mut len: Dword = 512;
            if hRecord != 0 {
                let _ = crate::win32::format::MsiFormatRecordA(
                    hInstall,
                    hRecord,
                    buf.as_mut_ptr(),
                    &raw mut len,
                );
            }
            return unsafe { cb(ctx, eMessageType, buf.as_ptr()) };
        }

        1 // IDOK
    });

    result.unwrap_or(-1)
}

/// Enables UI preview mode for the installer.
#[cfg_attr(not(test), no_mangle)]
#[inline(never)]
#[allow(non_snake_case)]
#[allow(unused_variables)]
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
#[cfg_attr(not(test), no_mangle)]
#[inline(never)]
#[allow(non_snake_case)]
#[allow(unused_variables)]
#[must_use]
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
#[cfg_attr(not(test), no_mangle)]
#[inline(never)]
#[allow(non_snake_case)]
#[allow(unused_variables)]
#[must_use]
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
#[cfg_attr(not(test), no_mangle)]
#[inline(never)]
#[allow(non_snake_case)]
#[allow(unused_variables)]
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
#[cfg_attr(not(test), no_mangle)]
#[inline(never)]
#[allow(non_snake_case)]
#[allow(unused_variables)]
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
#[cfg_attr(not(test), no_mangle)]
#[inline(never)]
#[allow(non_snake_case)]
#[allow(unused_variables)]
#[must_use]
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
#[cfg_attr(not(test), no_mangle)]
#[inline(never)]
#[allow(non_snake_case)]
#[allow(unused_variables)]
#[must_use]
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
#[allow(clippy::too_many_lines)]
#[allow(clippy::cognitive_complexity)]
mod tests {
    use super::*;
    use crate::handles::{alloc_handle, MsiObject};

    unsafe extern "system" fn dummy_ui_record_cb(
        _ctx: *mut c_void,
        _msg_type: Uint,
        _h_rec: MsiHandle,
    ) -> i32 {
        42
    }

    unsafe extern "system" fn dummy_ui_w_cb(
        _ctx: *mut c_void,
        _msg_type: Uint,
        _msg: Lpcwstr,
    ) -> i32 {
        43
    }

    unsafe extern "system" fn dummy_ui_a_cb(
        _ctx: *mut c_void,
        _msg_type: Uint,
        _msg: Lpcstr,
    ) -> i32 {
        44
    }

    #[test]
    fn test_ui_stubs() {
        let mut hwnd: *mut c_void = std::ptr::dangling_mut::<u8>().cast();
        assert_eq!(MsiSetInternalUI(5, &raw mut hwnd), 2);
        assert!(hwnd.is_null());
        assert_eq!(MsiSetInternalUI(0, &raw mut hwnd), 5);
        assert!(!hwnd.is_null());
        assert_eq!(MsiSetInternalUI(2, std::ptr::null_mut()), 5);

        let prev_w = MsiSetExternalUIW(None, 0, std::ptr::null_mut());
        assert!(prev_w.is_none());

        let prev_a = MsiSetExternalUIA(None, 0, std::ptr::null_mut());
        assert!(prev_a.is_none());

        let mut prev: InstallUIHandlerRecord = Some(dummy_ui_record_cb);
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

        // Test record, W, and A callbacks in MsiProcessMessage
        assert_eq!(
            MsiSetExternalUIRecord(
                Some(dummy_ui_record_cb),
                0,
                std::ptr::null_mut(),
                std::ptr::null_mut()
            ),
            ERROR_SUCCESS
        );
        assert_eq!(MsiProcessMessage(h_rec, 0, h_rec), 42);
        let _ = MsiSetExternalUIRecord(None, 0, std::ptr::null_mut(), std::ptr::null_mut());

        let _ = MsiSetExternalUIW(Some(dummy_ui_w_cb), 0, std::ptr::null_mut());
        assert_eq!(MsiProcessMessage(h_rec, 0, h_rec), 43);
        let _ = MsiSetExternalUIW(None, 0, std::ptr::null_mut());

        let _ = MsiSetExternalUIA(Some(dummy_ui_a_cb), 0, std::ptr::null_mut());
        assert_eq!(MsiProcessMessage(h_rec, 0, h_rec), 44);
        let _ = MsiSetExternalUIA(None, 0, std::ptr::null_mut());

        let _ = crate::handles::close_handle(h_rec);
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
            MsiEnableUIPreview(db_handle, &raw mut preview_handle),
            ERROR_SUCCESS
        );
        let _ = crate::handles::close_handle(preview_handle);
        assert_eq!(
            MsiEnableUIPreview(0, &raw mut preview_handle),
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
            MsiPreviewBillboardW(0, valid_w.as_ptr(), std::ptr::null()),
            crate::win32::ERROR_INVALID_PARAMETER
        );
        assert_eq!(
            MsiPreviewBillboardA(0, std::ptr::null(), std::ptr::null()),
            crate::win32::ERROR_INVALID_PARAMETER
        );
        assert_eq!(
            MsiPreviewBillboardA(0, valid_a.as_ptr().cast::<i8>(), std::ptr::null()),
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

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
#[no_mangle]
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
#[no_mangle]
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
#[no_mangle]
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
#[no_mangle]
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
#[no_mangle]
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
}

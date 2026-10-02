//! Win32 `stdcall` Drop-In Replacement Export Surface.
//!
//! This module exposes functions with the exact signatures and calling conventions
//! (`stdcall` on 32-bit Windows, `system` elsewhere) expected by applications
//! linking against `msi.dll`.

pub mod database;
pub mod installer;
pub mod properties;
pub mod record;
pub mod registry;
pub mod strings;
pub mod summary;
pub mod ui;
pub mod view;

use std::panic;

use crate::handles::{close_all_handles, close_handle, MsiHandle, MSI_NULL_HANDLE};

/// A Win32 `DWORD`.
pub type Dword = u32;

/// A Win32 `UINT`.
pub type Uint = u32;

/// A Win32 `LPWSTR` (Long Pointer to Wide String).
pub type Lpwstr = *mut u16;

/// A Win32 `LPCWSTR` (Long Pointer to Constant Wide String).
pub type Lpcwstr = *const u16;

/// A Win32 `LPSTR` (Long Pointer to String).
pub type Lpstr = *mut i8;

/// A Win32 `LPCSTR` (Long Pointer to Constant String).
pub type Lpcstr = *const i8;

// Standard MSI Error return codes (normally defined in winerror.h / msi.h)
/// Success.
pub const ERROR_SUCCESS: Uint = 0;
/// Invalid data.
pub const ERROR_INVALID_DATA: Uint = 13;
/// Invalid parameter.
pub const ERROR_INVALID_PARAMETER: Uint = 87;
/// More data is available; buffer too small.
pub const ERROR_MORE_DATA: Uint = 234;
/// Invalid handle.
pub const ERROR_INVALID_HANDLE: Uint = 6;
/// General installation failure.
pub const ERROR_INSTALL_FAILURE: Uint = 1603;

/// Closes an open installation handle.
///
/// # Arguments
///
/// * `handle` - The handle to close.
///
/// # Returns
///
/// `ERROR_SUCCESS` if successful, `ERROR_INVALID_HANDLE` if the handle was invalid.
#[no_mangle]
pub extern "system" fn MsiCloseHandle(handle: MsiHandle) -> Uint {
    let result = panic::catch_unwind(|| {
        if handle == MSI_NULL_HANDLE {
            return ERROR_SUCCESS; // Closing a null handle is a no-op that succeeds in Win32
        }
        if close_handle(handle) {
            ERROR_SUCCESS
        } else {
            ERROR_INVALID_HANDLE
        }
    });

    result.unwrap_or(ERROR_INSTALL_FAILURE)
}

/// Closes all open installation handles for the current process.
///
/// # Returns
///
/// `ERROR_SUCCESS`.
#[no_mangle]
pub extern "system" fn MsiCloseAllHandles() -> Uint {
    let result = panic::catch_unwind(|| {
        close_all_handles();
        ERROR_SUCCESS
    });

    result.unwrap_or(ERROR_INSTALL_FAILURE)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::handles::{alloc_handle, MsiObject};
    use crate::types::MsiRecordHandle;
    use msi::database::tables::record::Record;

    #[test]
    fn test_msi_close_handle() {
        assert_eq!(MsiCloseHandle(MSI_NULL_HANDLE), ERROR_SUCCESS);
        assert_eq!(MsiCloseHandle(9999), ERROR_INVALID_HANDLE);

        let rec = Record::new();
        let obj = MsiObject::Record(MsiRecordHandle { inner: rec });
        let handle = alloc_handle(obj);

        assert_eq!(MsiCloseHandle(handle), ERROR_SUCCESS);
        assert_eq!(MsiCloseHandle(handle), ERROR_INVALID_HANDLE); // Already closed
    }

    #[test]
    fn test_msi_close_all_handles() {
        let rec1 = Record::new();
        let obj1 = MsiObject::Record(MsiRecordHandle { inner: rec1 });
        let handle1 = alloc_handle(obj1);

        assert_eq!(MsiCloseAllHandles(), ERROR_SUCCESS);
        assert_eq!(MsiCloseHandle(handle1), ERROR_INVALID_HANDLE); // Should be closed by close_all_handles
    }
}

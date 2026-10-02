//! SQL View Win32 `stdcall` API endpoints.
//!
//! Provides `MsiViewExecute`, `MsiViewFetch`, `MsiViewClose`, `MsiViewGetColumnInfo`.

use std::panic;

use crate::handles::MsiHandle;
use crate::win32::{Uint, ERROR_INSTALL_FAILURE, ERROR_INVALID_HANDLE, ERROR_INVALID_PARAMETER};

// We don't have a View variant in MsiObject yet, but we define the signatures.

/// Executes a SQL view query.
///
/// # Arguments
///
/// * `hView` - The view handle.
/// * `hRecord` - Optional record handle providing parameters.
///
/// # Returns
///
/// `ERROR_SUCCESS`, `ERROR_INVALID_HANDLE`
#[no_mangle]
#[allow(non_snake_case, unused_variables)]
pub extern "system" fn MsiViewExecute(
    hView: MsiHandle,
    hRecord: MsiHandle, // 0 if no params
) -> Uint {
    let result = panic::catch_unwind(|| {
        // If we had a view we would execute it here.
        // For now, return ERROR_INVALID_HANDLE since there's no View handle yet.
        ERROR_INVALID_HANDLE
    });

    result.unwrap_or(ERROR_INSTALL_FAILURE)
}

/// Fetches the next sequential record from the view.
///
/// # Arguments
///
/// * `hView` - The view handle.
/// * `phRecord` - Pointer to receive the fetched record handle.
///
/// # Returns
///
/// `ERROR_SUCCESS`, `ERROR_INVALID_HANDLE`, `ERROR_NO_MORE_ITEMS`
#[no_mangle]
#[allow(non_snake_case, unused_variables)]
pub extern "system" fn MsiViewFetch(hView: MsiHandle, phRecord: *mut MsiHandle) -> Uint {
    let result = panic::catch_unwind(|| {
        if phRecord.is_null() {
            return ERROR_INVALID_PARAMETER;
        }
        ERROR_INVALID_HANDLE
    });

    result.unwrap_or(ERROR_INSTALL_FAILURE)
}

/// Releases the result set for an executed view.
///
/// # Arguments
///
/// * `hView` - The view handle.
///
/// # Returns
///
/// `ERROR_SUCCESS`, `ERROR_INVALID_HANDLE`
#[no_mangle]
#[allow(non_snake_case, unused_variables)]
pub extern "system" fn MsiViewClose(hView: MsiHandle) -> Uint {
    let result = panic::catch_unwind(|| ERROR_INVALID_HANDLE);

    result.unwrap_or(ERROR_INSTALL_FAILURE)
}

/// Returns a record containing column names or definitions.
///
/// # Arguments
///
/// * `hView` - The view handle.
/// * `eColumnInfo` - Information type (`MSICOLINFO_NAMES` or `MSICOLINFO_TYPES`).
/// * `phRecord` - Pointer to receive the record handle.
///
/// # Returns
///
/// `ERROR_SUCCESS`, `ERROR_INVALID_HANDLE`, `ERROR_INVALID_PARAMETER`
#[no_mangle]
#[allow(non_snake_case, unused_variables)]
pub extern "system" fn MsiViewGetColumnInfo(
    hView: MsiHandle,
    eColumnInfo: Uint,
    phRecord: *mut MsiHandle,
) -> Uint {
    let result = panic::catch_unwind(|| {
        if phRecord.is_null() {
            return ERROR_INVALID_PARAMETER;
        }
        ERROR_INVALID_HANDLE
    });

    result.unwrap_or(ERROR_INSTALL_FAILURE)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_view_stubs() {
        assert_eq!(MsiViewExecute(0, 0), ERROR_INVALID_HANDLE);
        assert_eq!(
            MsiViewFetch(0, std::ptr::null_mut()),
            ERROR_INVALID_PARAMETER
        );
        let mut h = 0;
        assert_eq!(MsiViewFetch(0, &raw mut h), ERROR_INVALID_HANDLE);
        assert_eq!(MsiViewClose(0), ERROR_INVALID_HANDLE);
        assert_eq!(
            MsiViewGetColumnInfo(0, 0, std::ptr::null_mut()),
            ERROR_INVALID_PARAMETER
        );
        assert_eq!(MsiViewGetColumnInfo(0, 0, &raw mut h), ERROR_INVALID_HANDLE);
    }
}

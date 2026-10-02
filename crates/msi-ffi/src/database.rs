//! C-ABI implementation of the Windows Installer Database API.
use msi::error::MsiError;
use msi::execution::custom_action::{global_handles, MSIHANDLE};
use std::ffi::c_char;

/// Error state returned by `MsiGetDatabaseState`.
pub const MSIDBSTATE_ERROR: i32 = -1;
/// Read-only state returned by `MsiGetDatabaseState`.
pub const MSIDBSTATE_READ: i32 = 0;
/// Read-write state returned by `MsiGetDatabaseState`.
pub const MSIDBSTATE_WRITE: i32 = 1;

/// Opens a database for reading or writing (UTF-16).
///
/// # Arguments
///
/// * `sz_database_path` - Path to the database file (UTF-16, null-terminated).
/// * `sz_persist` - Persist mode string or integer.
/// * `ph_database` - Pointer to receive the allocated database handle.
///
/// # Returns
///
/// Win32 status code (e.g., `ERROR_SUCCESS`, `ERROR_INVALID_PARAMETER`).
///
/// # Safety
///
/// Pointers must be valid or null per the MSI API contract.
#[no_mangle]
pub unsafe extern "system" fn MsiOpenDatabaseW(
    sz_database_path: *const u16,
    _sz_persist: *const u16,
    ph_database: *mut MSIHANDLE,
) -> u32 {
    unsafe {
        std::panic::catch_unwind(|| {
            if ph_database.is_null() || sz_database_path.is_null() {
                return MsiError::ERROR_INVALID_PARAMETER;
            }

            // Convert path
            let path_len = {
                let mut len = 0;
                while *sz_database_path.add(len) != 0 {
                    len += 1;
                }
                len
            };
            let path_slice = std::slice::from_raw_parts(sz_database_path, path_len);
            let Ok(path_str) = String::from_utf16(path_slice) else {
                return MsiError::ERROR_INVALID_PARAMETER;
            };

            // MsiOpenDatabaseW simply opens it. We'll load LinkedDatabase via Package.
            match msi::package::Package::open(&path_str) {
                Ok(pkg) => {
                    let mut hm = global_handles().lock().unwrap_or_else(|e| e.into_inner());
                    *ph_database = hm.register_database(pkg.database().clone());
                    MsiError::ERROR_SUCCESS
                }
                Err(e) => u32::from(e),
            }
        })
        .unwrap_or(MsiError::ERROR_INSTALL_FAILURE)
    }
}

/// Opens a database for reading or writing (ANSI).
///
/// # Arguments
///
/// * `sz_database_path` - Path to the database file (ANSI, null-terminated).
/// * `sz_persist` - Persist mode string or integer.
/// * `ph_database` - Pointer to receive the allocated database handle.
///
/// # Returns
///
/// Win32 status code (e.g., `ERROR_SUCCESS`, `ERROR_INVALID_PARAMETER`).
///
/// # Safety
///
/// Pointers must be valid or null per the MSI API contract.
#[no_mangle]
pub unsafe extern "system" fn MsiOpenDatabaseA(
    sz_database_path: *const c_char,
    _sz_persist: *const c_char,
    ph_database: *mut MSIHANDLE,
) -> u32 {
    unsafe {
        std::panic::catch_unwind(|| {
            if ph_database.is_null() || sz_database_path.is_null() {
                return MsiError::ERROR_INVALID_PARAMETER;
            }

            let Ok(path_str) = std::ffi::CStr::from_ptr(sz_database_path).to_str() else {
                return MsiError::ERROR_INVALID_PARAMETER;
            };

            match msi::package::Package::open(path_str) {
                Ok(pkg) => {
                    let mut hm = global_handles().lock().unwrap_or_else(|e| e.into_inner());
                    *ph_database = hm.register_database(pkg.database().clone());
                    MsiError::ERROR_SUCCESS
                }
                Err(e) => u32::from(e),
            }
        })
        .unwrap_or(MsiError::ERROR_INSTALL_FAILURE)
    }
}

/// Commits changes to a database.
///
/// # Arguments
///
/// * `h_database` - Handle to the open database.
///
/// # Returns
///
/// Win32 status code (e.g., `ERROR_SUCCESS`, `ERROR_INVALID_HANDLE`).
#[no_mangle]
pub unsafe extern "system" fn MsiDatabaseCommit(h_database: MSIHANDLE) -> u32 {
    std::panic::catch_unwind(|| {
        let hm = global_handles().lock().unwrap_or_else(|e| e.into_inner());
        if hm.get_database(h_database).is_none() {
            return MsiError::ERROR_INVALID_HANDLE;
        }

        // msi-rs databases are largely in-memory during custom action execution.
        // We'll treat commit as a no-op success unless saving is implemented on `LinkedDatabase`.
        MsiError::ERROR_SUCCESS
    })
    .unwrap_or(MsiError::ERROR_INSTALL_FAILURE)
}

/// Returns the state of the database.
///
/// # Arguments
///
/// * `h_database` - Handle to the open database.
///
/// # Returns
///
/// `MSIDBSTATE_READ`, `MSIDBSTATE_WRITE`, or `MSIDBSTATE_ERROR` on failure.
#[no_mangle]
pub unsafe extern "system" fn MsiGetDatabaseState(h_database: MSIHANDLE) -> i32 {
    std::panic::catch_unwind(|| {
        let hm = global_handles().lock().unwrap_or_else(|e| e.into_inner());
        if hm.get_database(h_database).is_some() {
            MSIDBSTATE_READ
        } else {
            MSIDBSTATE_ERROR
        }
    })
    .unwrap_or(MSIDBSTATE_ERROR)
}

/// Merges two databases together (UTF-16).
///
/// # Arguments
///
/// * `h_database` - Handle to the target database.
/// * `h_database_merge` - Handle to the database being merged.
/// * `sz_table_name` - Optional table name to merge into.
///
/// # Returns
///
/// Win32 status code (e.g., `ERROR_SUCCESS`, `ERROR_CALL_NOT_IMPLEMENTED`).
///
/// # Safety
///
/// Pointers must be valid or null.
#[no_mangle]
pub unsafe extern "system" fn MsiDatabaseMergeW(
    _h_database: MSIHANDLE,
    _h_database_merge: MSIHANDLE,
    _sz_table_name: *const u16,
) -> u32 {
    std::panic::catch_unwind(|| {
        // Not implemented in `msi-rs` yet for direct db merging at runtime
        MsiError::ERROR_CALL_NOT_IMPLEMENTED
    })
    .unwrap_or(MsiError::ERROR_INSTALL_FAILURE)
}

/// Merges two databases together (ANSI).
///
/// # Arguments
///
/// * `h_database` - Handle to the target database.
/// * `h_database_merge` - Handle to the database being merged.
/// * `sz_table_name` - Optional table name to merge into.
///
/// # Returns
///
/// Win32 status code (e.g., `ERROR_SUCCESS`, `ERROR_CALL_NOT_IMPLEMENTED`).
///
/// # Safety
///
/// Pointers must be valid or null.
#[no_mangle]
pub unsafe extern "system" fn MsiDatabaseMergeA(
    _h_database: MSIHANDLE,
    _h_database_merge: MSIHANDLE,
    _sz_table_name: *const c_char,
) -> u32 {
    std::panic::catch_unwind(|| MsiError::ERROR_CALL_NOT_IMPLEMENTED)
        .unwrap_or(MsiError::ERROR_INSTALL_FAILURE)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ptr;

    #[test]
    fn test_msi_database_null_pointers() {
        let mut handle = 0;
        assert_eq!(
            unsafe { MsiOpenDatabaseA(ptr::null(), ptr::null(), &mut handle) },
            MsiError::ERROR_INVALID_PARAMETER
        );
        assert_eq!(
            unsafe { MsiOpenDatabaseW(ptr::null(), ptr::null(), &mut handle) },
            MsiError::ERROR_INVALID_PARAMETER
        );
        assert_eq!(
            unsafe { MsiOpenDatabaseA(b"test\0".as_ptr().cast(), ptr::null(), ptr::null_mut()) },
            MsiError::ERROR_INVALID_PARAMETER
        );
    }

    #[test]
    fn test_msi_database_state_and_commit() {
        assert_eq!(unsafe { MsiGetDatabaseState(9999) }, MSIDBSTATE_ERROR);
        assert_eq!(
            unsafe { MsiDatabaseCommit(9999) },
            MsiError::ERROR_INVALID_HANDLE
        );

        // Cannot test a real database here without writing a file to disk,
        // but we can test merge unimplemented.
        assert_eq!(
            unsafe { MsiDatabaseMergeA(1, 2, ptr::null()) },
            MsiError::ERROR_CALL_NOT_IMPLEMENTED
        );
        assert_eq!(
            unsafe { MsiDatabaseMergeW(1, 2, ptr::null()) },
            MsiError::ERROR_CALL_NOT_IMPLEMENTED
        );
    }
}

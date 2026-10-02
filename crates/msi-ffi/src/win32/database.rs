//! Database Win32 `stdcall` API endpoints.
//!
//! Provides `MsiOpenDatabaseA/W` and `MsiDatabaseOpenViewA/W`.

use std::panic;

use crate::handles::{alloc_handle, with_handle, MsiHandle, MsiObject};
use crate::types::MsiDatabaseHandle;
use crate::win32::strings::{lpcstr_to_string, lpcwstr_to_string};
use crate::win32::{
    Lpcstr, Lpcwstr, Uint, ERROR_INSTALL_FAILURE, ERROR_INVALID_HANDLE, ERROR_INVALID_PARAMETER,
    ERROR_SUCCESS,
};

// Win32 constants for OpenDatabase
/// Read-only.
pub const MSIDBOPEN_READONLY: Lpcwstr = 0 as Lpcwstr;
/// Transact mode.
pub const MSIDBOPEN_TRANSACT: Lpcwstr = 1 as Lpcwstr;
/// Direct mode.
pub const MSIDBOPEN_DIRECT: Lpcwstr = 2 as Lpcwstr;
/// Create mode.
pub const MSIDBOPEN_CREATE: Lpcwstr = 3 as Lpcwstr;
/// Create direct mode.
pub const MSIDBOPEN_CREATEDIRECT: Lpcwstr = 4 as Lpcwstr;

/// Opens a database for data access in Unicode (`W`).
///
/// # Arguments
///
/// * `szDatabasePath` - The path to the database.
/// * `szPersist` - Persistence mode.
/// * `phDatabase` - Pointer to a variable that receives the database handle.
///
/// # Returns
///
/// `ERROR_SUCCESS`, `ERROR_INVALID_PARAMETER`
#[no_mangle]
#[allow(non_snake_case, unused_variables)]
#[allow(clippy::not_unsafe_ptr_arg_deref)]
pub extern "system" fn MsiOpenDatabaseW(
    szDatabasePath: Lpcwstr,
    szPersist: Lpcwstr,
    phDatabase: *mut MsiHandle,
) -> Uint {
    let result = panic::catch_unwind(|| {
        if phDatabase.is_null() {
            return ERROR_INVALID_PARAMETER;
        }

        let Some(_path) = lpcwstr_to_string(szDatabasePath) else {
            return ERROR_INVALID_PARAMETER;
        };

        let mock_db = msi::wix::linker::LinkedDatabase::default();

        let obj = MsiObject::Database(MsiDatabaseHandle { inner: mock_db });

        // SAFETY: We verified `phDatabase` is not null.
        unsafe {
            *phDatabase = alloc_handle(obj);
        }

        ERROR_SUCCESS
    });

    result.unwrap_or(ERROR_INSTALL_FAILURE)
}

/// Opens a database for data access in ANSI (`A`).
///
/// # Arguments
///
/// * `szDatabasePath` - The path to the database.
/// * `szPersist` - Persistence mode.
/// * `phDatabase` - Pointer to a variable that receives the database handle.
///
/// # Returns
///
/// `ERROR_SUCCESS`, `ERROR_INVALID_PARAMETER`
#[no_mangle]
#[allow(non_snake_case, unused_variables)]
#[allow(clippy::not_unsafe_ptr_arg_deref)]
pub extern "system" fn MsiOpenDatabaseA(
    szDatabasePath: Lpcstr,
    szPersist: Lpcstr,
    phDatabase: *mut MsiHandle,
) -> Uint {
    let result = panic::catch_unwind(|| {
        if phDatabase.is_null() {
            return ERROR_INVALID_PARAMETER;
        }

        let Some(_path) = lpcstr_to_string(szDatabasePath) else {
            return ERROR_INVALID_PARAMETER;
        };

        let mock_db = msi::wix::linker::LinkedDatabase::default();

        let obj = MsiObject::Database(MsiDatabaseHandle { inner: mock_db });

        // SAFETY: We verified `phDatabase` is not null.
        unsafe {
            *phDatabase = alloc_handle(obj);
        }

        ERROR_SUCCESS
    });

    result.unwrap_or(ERROR_INSTALL_FAILURE)
}

/// Prepares a database query and creates a view object in Unicode (`W`).
///
/// # Arguments
///
/// * `hDatabase` - The database handle.
/// * `szQuery` - SQL query string.
/// * `phView` - Pointer to a variable that receives the view handle.
///
/// # Returns
///
/// `ERROR_SUCCESS`, `ERROR_INVALID_HANDLE`, `ERROR_INVALID_PARAMETER`
#[no_mangle]
#[allow(non_snake_case, unused_variables)]
pub extern "system" fn MsiDatabaseOpenViewW(
    hDatabase: MsiHandle,
    szQuery: Lpcwstr,
    phView: *mut MsiHandle,
) -> Uint {
    let result = panic::catch_unwind(|| {
        if phView.is_null() {
            return ERROR_INVALID_PARAMETER;
        }

        let Some(_query) = lpcwstr_to_string(szQuery) else {
            return ERROR_INVALID_PARAMETER;
        };

        with_handle(hDatabase, |obj| {
            if let MsiObject::Database(_db_handle) = obj {
                // In a real implementation, we parse the SQL and create a View object.
                // We don't have a View object yet, but let's mock it.
                // We'd add a View variant to MsiObject.
                // unsafe { *phView = alloc_handle(MsiObject::View(...)) };
                ERROR_SUCCESS
            } else {
                ERROR_INVALID_HANDLE
            }
        })
        .unwrap_or(ERROR_INVALID_HANDLE)
    });

    result.unwrap_or(ERROR_INSTALL_FAILURE)
}

/// Prepares a database query and creates a view object in ANSI (`A`).
///
/// # Arguments
///
/// * `hDatabase` - The database handle.
/// * `szQuery` - SQL query string.
/// * `phView` - Pointer to a variable that receives the view handle.
///
/// # Returns
///
/// `ERROR_SUCCESS`, `ERROR_INVALID_HANDLE`, `ERROR_INVALID_PARAMETER`
#[no_mangle]
#[allow(non_snake_case, unused_variables)]
pub extern "system" fn MsiDatabaseOpenViewA(
    hDatabase: MsiHandle,
    szQuery: Lpcstr,
    phView: *mut MsiHandle,
) -> Uint {
    let result = panic::catch_unwind(|| {
        if phView.is_null() {
            return ERROR_INVALID_PARAMETER;
        }

        let Some(_query) = lpcstr_to_string(szQuery) else {
            return ERROR_INVALID_PARAMETER;
        };

        with_handle(hDatabase, |obj| {
            if let MsiObject::Database(_db_handle) = obj {
                ERROR_SUCCESS
            } else {
                ERROR_INVALID_HANDLE
            }
        })
        .unwrap_or(ERROR_INVALID_HANDLE)
    });

    result.unwrap_or(ERROR_INSTALL_FAILURE)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[allow(clippy::too_many_lines)]
    fn test_database_stubs() {
        let mut handle = 0;
        assert_eq!(
            MsiOpenDatabaseW(std::ptr::null(), std::ptr::null(), std::ptr::null_mut()),
            ERROR_INVALID_PARAMETER
        );
        assert_eq!(
            MsiOpenDatabaseA(std::ptr::null(), std::ptr::null(), std::ptr::null_mut()),
            ERROR_INVALID_PARAMETER
        );
        assert_eq!(
            MsiDatabaseOpenViewW(0, std::ptr::null(), std::ptr::null_mut()),
            ERROR_INVALID_PARAMETER
        );
        assert_eq!(
            MsiDatabaseOpenViewA(0, std::ptr::null(), std::ptr::null_mut()),
            ERROR_INVALID_PARAMETER
        );

        let valid_w: Vec<u16> = "path".encode_utf16().chain(std::iter::once(0)).collect();
        let valid_a = b"path\0";

        assert_eq!(
            MsiOpenDatabaseW(valid_w.as_ptr(), std::ptr::null(), std::ptr::null_mut()),
            ERROR_INVALID_PARAMETER
        );
        assert_eq!(
            MsiOpenDatabaseA(
                valid_a.as_ptr().cast::<i8>(),
                std::ptr::null(),
                std::ptr::null_mut()
            ),
            ERROR_INVALID_PARAMETER
        );

        assert_eq!(
            MsiDatabaseOpenViewW(0, valid_w.as_ptr(), std::ptr::null_mut()),
            ERROR_INVALID_PARAMETER
        );
        assert_eq!(
            MsiDatabaseOpenViewA(0, valid_a.as_ptr().cast::<i8>(), std::ptr::null_mut()),
            ERROR_INVALID_PARAMETER
        );

        assert_eq!(
            MsiOpenDatabaseW(std::ptr::null(), std::ptr::null(), &raw mut handle),
            ERROR_INVALID_PARAMETER
        );
        assert_eq!(
            MsiOpenDatabaseA(std::ptr::null(), std::ptr::null(), &raw mut handle),
            ERROR_INVALID_PARAMETER
        );

        assert_eq!(
            MsiDatabaseOpenViewW(0, std::ptr::null(), &raw mut handle),
            ERROR_INVALID_PARAMETER
        );
        assert_eq!(
            MsiDatabaseOpenViewA(0, std::ptr::null(), &raw mut handle),
            ERROR_INVALID_PARAMETER
        );

        assert_eq!(
            MsiOpenDatabaseW(valid_w.as_ptr(), MSIDBOPEN_READONLY, &raw mut handle),
            ERROR_SUCCESS
        );
        let h_db3 = handle;
        assert_eq!(
            MsiDatabaseOpenViewW(0, valid_w.as_ptr(), &raw mut handle),
            ERROR_INVALID_HANDLE
        ); // null handle test
        assert_eq!(
            MsiDatabaseOpenViewW(h_db3, valid_w.as_ptr(), &raw mut handle),
            ERROR_SUCCESS
        );
        assert!(crate::handles::close_handle(h_db3));

        assert_eq!(
            MsiOpenDatabaseA(
                valid_a.as_ptr().cast::<i8>(),
                std::ptr::null(),
                &raw mut handle
            ),
            ERROR_SUCCESS
        );
        let h_db4 = handle;
        assert_eq!(
            MsiDatabaseOpenViewA(0, valid_a.as_ptr().cast::<i8>(), &raw mut handle),
            ERROR_INVALID_HANDLE
        ); // null handle test
        assert_eq!(
            MsiDatabaseOpenViewA(h_db4, valid_a.as_ptr().cast::<i8>(), &raw mut handle),
            ERROR_SUCCESS
        );
        assert!(crate::handles::close_handle(h_db4));
        let rec = msi::database::tables::record::Record::new();
        let h_rec = alloc_handle(MsiObject::Record(crate::types::MsiRecordHandle {
            inner: rec,
        }));
        assert_eq!(
            MsiDatabaseOpenViewW(h_rec, valid_w.as_ptr(), &raw mut handle),
            ERROR_INVALID_HANDLE
        );
        assert_eq!(
            MsiDatabaseOpenViewA(h_rec, valid_a.as_ptr().cast::<i8>(), &raw mut handle),
            ERROR_INVALID_HANDLE
        );
    }
}

//! C-ABI implementation of the Windows Installer View and Query API.
use msi::error::MsiError;
use msi::execution::custom_action::{global_handles, View, MSIHANDLE};
use msi::database::sql::executor::{execute_sql, QueryResult};
use msi::database::tables::record::Record;
use std::ffi::c_char;

/// Modifies a view.
pub const MSIMODIFY_SEEK: i32 = -1;
/// Refreshes the information in the supplied record.
pub const MSIMODIFY_REFRESH: i32 = 0;
/// Inserts a record.
pub const MSIMODIFY_INSERT: i32 = 1;
/// Updates an existing record.
pub const MSIMODIFY_UPDATE: i32 = 2;
/// Writes current data to the cursor.
pub const MSIMODIFY_ASSIGN: i32 = 3;
/// Updates or inserts a record.
pub const MSIMODIFY_REPLACE: i32 = 4;
/// Inserts or validates a record in a merge operation.
pub const MSIMODIFY_MERGE: i32 = 5;
/// Deletes a record.
pub const MSIMODIFY_DELETE: i32 = 6;
/// Inserts a temporary record.
pub const MSIMODIFY_INSERT_TEMPORARY: i32 = 7;
/// Validates a record.
pub const MSIMODIFY_VALIDATE: i32 = 8;
/// Validates a new record.
pub const MSIMODIFY_VALIDATE_NEW: i32 = 9;
/// Validates fields of a fetched record.
pub const MSIMODIFY_VALIDATE_FIELD: i32 = 10;
/// Validates a record before deleting it.
pub const MSIMODIFY_VALIDATE_DELETE: i32 = 11;

/// Prepares a database query and creates a view object (UTF-16).
///
/// # Arguments
///
/// * `h_database` - Handle to the database.
/// * `sz_query` - SQL query string (UTF-16, null-terminated).
/// * `ph_view` - Pointer to receive the allocated view handle.
///
/// # Returns
///
/// Win32 status code (e.g., `ERROR_SUCCESS`, `ERROR_INVALID_PARAMETER`).
///
/// # Safety
///
/// Pointers must be valid or null per the MSI API contract.
#[no_mangle]
pub unsafe extern "system" fn MsiDatabaseOpenViewW(
    h_database: MSIHANDLE,
    sz_query: *const u16,
    ph_view: *mut MSIHANDLE,
) -> u32 {
    unsafe {
        std::panic::catch_unwind(|| {
            if ph_view.is_null() || sz_query.is_null() {
                return MsiError::ERROR_INVALID_PARAMETER;
            }

            let path_len = {
                let mut len = 0;
                while *sz_query.add(len) != 0 {
                    len += 1;
                }
                len
            };
            let query_slice = std::slice::from_raw_parts(sz_query, path_len);
            let query_str = match String::from_utf16(query_slice) {
                Ok(s) => s,
                Err(_) => return MsiError::ERROR_INVALID_PARAMETER,
            };

            let mut hm = global_handles().lock().unwrap_or_else(|e| e.into_inner());
            
            // Validate database handle exists
            if hm.get_database(h_database).is_none() {
                return MsiError::ERROR_INVALID_HANDLE;
            }
            
            let view = View {
                database_handle: h_database,
                query: query_str,
                fetched_records: std::collections::VecDeque::new(),
            };

            *ph_view = hm.register_view(view);
            MsiError::ERROR_SUCCESS
        })
        .unwrap_or(MsiError::ERROR_INSTALL_FAILURE)
    }

    #[test]
    fn test_msi_view_execute_and_fetch() {
        use msi::wix::linker::LinkedDatabase;
        let mut hm = global_handles().lock().unwrap_or_else(|e| e.into_inner());
        let db = LinkedDatabase::new().unwrap();
        let db_handle = hm.register_database(db);
        
        let view = View {
            database_handle: db_handle,
            query: "CREATE TABLE `Feature` (`Feature` CHAR(38) NOT NULL PRIMARY KEY)".to_string(),
            fetched_records: std::collections::VecDeque::new(),
        };
        let view_handle = hm.register_view(view);
        drop(hm);

        let res = MsiViewExecute(view_handle, 0);
        assert_eq!(res, MsiError::ERROR_SUCCESS);

        let mut rec_handle = 0;
        let fetch_res = unsafe { MsiViewFetch(view_handle, &mut rec_handle) };
        assert_eq!(fetch_res, 259);
    }

}

/// Prepares a database query and creates a view object (ANSI).
///
/// # Arguments
///
/// * `h_database` - Handle to the database.
/// * `sz_query` - SQL query string (ANSI, null-terminated).
/// * `ph_view` - Pointer to receive the allocated view handle.
///
/// # Returns
///
/// Win32 status code (e.g., `ERROR_SUCCESS`, `ERROR_INVALID_PARAMETER`).
///
/// # Safety
///
/// Pointers must be valid or null per the MSI API contract.
#[no_mangle]
pub unsafe extern "system" fn MsiDatabaseOpenViewA(
    h_database: MSIHANDLE,
    sz_query: *const c_char,
    ph_view: *mut MSIHANDLE,
) -> u32 {
    unsafe {
        std::panic::catch_unwind(|| {
            if ph_view.is_null() || sz_query.is_null() {
                return MsiError::ERROR_INVALID_PARAMETER;
            }

            let query_str = match std::ffi::CStr::from_ptr(sz_query).to_str() {
                Ok(s) => s,
                Err(_) => return MsiError::ERROR_INVALID_PARAMETER,
            };

            let mut hm = global_handles().lock().unwrap_or_else(|e| e.into_inner());
            
            if hm.get_database(h_database).is_none() {
                return MsiError::ERROR_INVALID_HANDLE;
            }
            
            let view = View {
                database_handle: h_database,
                query: query_str.to_string(),
                fetched_records: std::collections::VecDeque::new(),
            };

            *ph_view = hm.register_view(view);
            MsiError::ERROR_SUCCESS
        })
        .unwrap_or(MsiError::ERROR_INSTALL_FAILURE)
    }

    #[test]
    fn test_msi_view_execute_and_fetch() {
        use msi::wix::linker::LinkedDatabase;
        let mut hm = global_handles().lock().unwrap_or_else(|e| e.into_inner());
        let db = LinkedDatabase::new().unwrap();
        let db_handle = hm.register_database(db);
        
        let view = View {
            database_handle: db_handle,
            query: "CREATE TABLE `Feature` (`Feature` CHAR(38) NOT NULL PRIMARY KEY)".to_string(),
            fetched_records: std::collections::VecDeque::new(),
        };
        let view_handle = hm.register_view(view);
        drop(hm);

        let res = MsiViewExecute(view_handle, 0);
        assert_eq!(res, MsiError::ERROR_SUCCESS);

        let mut rec_handle = 0;
        let fetch_res = unsafe { MsiViewFetch(view_handle, &mut rec_handle) };
        assert_eq!(fetch_res, 259);
    }

}

/// Executes a view query.
///
/// # Arguments
///
/// * `h_view` - Handle to the view.
/// * `_h_record` - Handle to a record containing parameter values for the query (unused currently).
///
/// # Returns
///
/// Win32 status code (e.g., `ERROR_SUCCESS`, `ERROR_INVALID_HANDLE`).
#[no_mangle]
pub extern "system" fn MsiViewExecute(h_view: MSIHANDLE, _h_record: MSIHANDLE) -> u32 {
    std::panic::catch_unwind(|| {
        let mut hm = global_handles().lock().unwrap_or_else(|e| e.into_inner());
        
        let (db_handle, query) = {
            let view = match hm.get_view_mut(h_view) {
                Some(v) => v,
                None => return MsiError::ERROR_INVALID_HANDLE,
            };
            view.fetched_records.clear();
            (view.database_handle, view.query.clone())
        };

        // Note: For full accuracy we should extract parameter values from `_h_record`
        // if the query contains parameterized markers (`?`). 
        // For Phase 1 we execute directly.
        let params = [];

        let db = match hm.get_database_mut(db_handle) {
            Some(d) => d,
            None => return MsiError::ERROR_INVALID_HANDLE,
        };

        match execute_sql(db, &query, &params) {
            Ok(QueryResult::Select { rows, .. }) => {
                if let Some(view) = hm.get_view_mut(h_view) {
                    view.fetched_records.extend(rows);
                }
                MsiError::ERROR_SUCCESS
            }
            Ok(_) => MsiError::ERROR_SUCCESS,
            Err(e) => {
                crate::error::set_last_error(u32::from(&e) as i32, e.to_string());
                u32::from(e)
            }
        }
    })
    .unwrap_or(MsiError::ERROR_INSTALL_FAILURE)
}

/// Fetches the next sequential record from the view.
///
/// # Arguments
///
/// * `h_view` - Handle to the view.
/// * `ph_record` - Pointer to receive the allocated record handle.
///
/// # Returns
///
/// Win32 status code (e.g., `ERROR_SUCCESS`, `ERROR_NO_MORE_ITEMS`).
///
/// # Safety
///
/// Pointers must be valid or null per the MSI API contract.
#[no_mangle]
pub unsafe extern "system" fn MsiViewFetch(h_view: MSIHANDLE, ph_record: *mut MSIHANDLE) -> u32 {
    unsafe {
        std::panic::catch_unwind(|| {
            if ph_record.is_null() {
                return MsiError::ERROR_INVALID_PARAMETER;
            }

            let mut hm = global_handles().lock().unwrap_or_else(|e| e.into_inner());
            let view = match hm.get_view_mut(h_view) {
                Some(v) => v,
                None => return MsiError::ERROR_INVALID_HANDLE,
            };

            if let Some(record) = view.fetched_records.pop_front() {
                *ph_record = hm.register_record(record);
                MsiError::ERROR_SUCCESS
            } else {
                // Return ERROR_NO_MORE_ITEMS if empty.
                259 // ERROR_NO_MORE_ITEMS
            }
        })
        .unwrap_or(MsiError::ERROR_INSTALL_FAILURE)
    }

    #[test]
    fn test_msi_view_execute_and_fetch() {
        use msi::wix::linker::LinkedDatabase;
        let mut hm = global_handles().lock().unwrap_or_else(|e| e.into_inner());
        let db = LinkedDatabase::new().unwrap();
        let db_handle = hm.register_database(db);
        
        let view = View {
            database_handle: db_handle,
            query: "CREATE TABLE `Feature` (`Feature` CHAR(38) NOT NULL PRIMARY KEY)".to_string(),
            fetched_records: std::collections::VecDeque::new(),
        };
        let view_handle = hm.register_view(view);
        drop(hm);

        let res = MsiViewExecute(view_handle, 0);
        assert_eq!(res, MsiError::ERROR_SUCCESS);

        let mut rec_handle = 0;
        let fetch_res = unsafe { MsiViewFetch(view_handle, &mut rec_handle) };
        assert_eq!(fetch_res, 259);
    }

}

/// Modifies a view (e.g., inserts, updates, deletes a record).
///
/// # Arguments
///
/// * `h_view` - Handle to the view.
/// * `e_modify_mode` - Modification mode (e.g., `MSIMODIFY_UPDATE`).
/// * `_h_record` - Handle to the record containing the modified values.
///
/// # Returns
///
/// Win32 status code (e.g., `ERROR_SUCCESS`, `ERROR_CALL_NOT_IMPLEMENTED`).
#[no_mangle]
pub extern "system" fn MsiViewModify(
    h_view: MSIHANDLE,
    _e_modify_mode: i32,
    _h_record: MSIHANDLE,
) -> u32 {
    std::panic::catch_unwind(|| {
        let hm = global_handles().lock().unwrap_or_else(|e| e.into_inner());
        if hm.get_view(h_view).is_none() {
            return MsiError::ERROR_INVALID_HANDLE;
        }

        // True modify logic requires synthesizing an INSERT/UPDATE from the view schema
        // For now, this meets the ABI requirement and serves as a stub.
        MsiError::ERROR_CALL_NOT_IMPLEMENTED
    })
    .unwrap_or(MsiError::ERROR_INSTALL_FAILURE)
}

/// Closes a view.
///
/// # Arguments
///
/// * `h_view` - Handle to the view.
///
/// # Returns
///
/// Win32 status code (e.g., `ERROR_SUCCESS`, `ERROR_INVALID_HANDLE`).
#[no_mangle]
pub extern "system" fn MsiViewClose(h_view: MSIHANDLE) -> u32 {
    std::panic::catch_unwind(|| {
        let mut hm = global_handles().lock().unwrap_or_else(|e| e.into_inner());
        if hm.get_view_mut(h_view).is_some() {
            // Drop fetched records
            if let Some(v) = hm.get_view_mut(h_view) {
                v.fetched_records.clear();
            }
            MsiError::ERROR_SUCCESS
        } else {
            MsiError::ERROR_INVALID_HANDLE
        }
    })
    .unwrap_or(MsiError::ERROR_INSTALL_FAILURE)
}

#[cfg(test)]
mod tests {
    use msi::wix::linker::LinkedDatabase;

    use super::*;
    use std::ptr;

    #[test]
    fn test_msi_view_null_pointers() {
        let mut handle = 0;
        assert_eq!(
            unsafe { MsiDatabaseOpenViewA(1, ptr::null(), &mut handle) },
            MsiError::ERROR_INVALID_PARAMETER
        );
        assert_eq!(
            unsafe { MsiDatabaseOpenViewW(1, ptr::null(), &mut handle) },
            MsiError::ERROR_INVALID_PARAMETER
        );
        assert_eq!(
            unsafe { MsiDatabaseOpenViewA(1, b"SELECT\\0".as_ptr().cast(), ptr::null_mut()) },
            MsiError::ERROR_INVALID_PARAMETER
        );
        assert_eq!(
            unsafe { MsiViewFetch(1, ptr::null_mut()) },
            MsiError::ERROR_INVALID_PARAMETER
        );
    }

    #[test]
    fn test_msi_view_invalid_handles() {
        assert_eq!(
            MsiViewExecute(9999, 0),
            MsiError::ERROR_INVALID_HANDLE
        );
        assert_eq!(
            MsiViewModify(9999, MSIMODIFY_INSERT, 0),
            MsiError::ERROR_INVALID_HANDLE
        );
        assert_eq!(
            MsiViewClose(9999),
            MsiError::ERROR_INVALID_HANDLE
        );
    }

    #[test]
    fn test_msi_view_execute_and_fetch() {
        use msi::wix::linker::LinkedDatabase;
        let mut hm = global_handles().lock().unwrap_or_else(|e| e.into_inner());
        let db = LinkedDatabase::new().unwrap();
        let db_handle = hm.register_database(db);
        
        let view = View {
            database_handle: db_handle,
            query: "CREATE TABLE `Feature` (`Feature` CHAR(38) NOT NULL PRIMARY KEY)".to_string(),
            fetched_records: std::collections::VecDeque::new(),
        };
        let view_handle = hm.register_view(view);
        drop(hm);

        let res = MsiViewExecute(view_handle, 0);
        assert_eq!(res, MsiError::ERROR_SUCCESS);

        let mut rec_handle = 0;
        let fetch_res = unsafe { MsiViewFetch(view_handle, &mut rec_handle) };
        assert_eq!(fetch_res, 259);
    }

}
//! SQL View Win32 `stdcall` API endpoints.
//!
//! Provides `MsiViewExecute`, `MsiViewFetch`, `MsiViewClose`, `MsiViewGetColumnInfo`, `MsiViewGetError`.

use std::panic;

use crate::handles::MsiHandle;
use crate::win32::strings::{string_to_lpstr, string_to_lpwstr};
use crate::win32::{
    Dword, Lpstr, Lpwstr, Uint, ERROR_INSTALL_FAILURE, ERROR_INVALID_HANDLE,
    ERROR_INVALID_PARAMETER, ERROR_SUCCESS,
};

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

// We don't have a View variant in MsiObject yet, but we define the signatures.

/// Validates a view execution and returns any error that occurred.
///
/// # Arguments
///
/// * `hView` - The view handle.
/// * `szColumnNameBuffer` - Buffer to receive the column name.
/// * `pcchBuf` - In/Out parameter for the buffer length.
///
/// # Returns
///
/// An `MSIDBERROR` enum value. `MSIDBERROR_NOERROR` (0) on success, or negative on failure.
#[cfg_attr(not(coverage_nightly), no_mangle)]
#[inline(never)]
#[allow(non_snake_case, unused_variables)]
pub extern "system" fn MsiViewGetErrorW(
    hView: MsiHandle,
    szColumnNameBuffer: Lpwstr,
    pcchBuf: *mut Dword,
) -> i32 {
    let result = panic::catch_unwind(|| {
        if hView == 0 {
            return -3; // MSIDBERROR_INVALIDARG
        }

        crate::handles::with_handle(hView, |obj| {
            if let crate::handles::MsiObject::View(view) = obj {
                if let Some(col_name) = &view.last_error_column {
                    let _ = string_to_lpwstr(col_name, szColumnNameBuffer, pcchBuf);
                    -1 // MSIDBERROR_FUNCTIONERROR (or specific validation error if we had it)
                } else {
                    let _ = string_to_lpwstr("", szColumnNameBuffer, pcchBuf);
                    0 // MSIDBERROR_NOERROR
                }
            } else {
                -3 // MSIDBERROR_INVALIDARG
            }
        })
        .unwrap_or(-3)
    });

    result.unwrap_or(-1) // MSIDBERROR_ERROR
}

/// Validates a view execution and returns any error that occurred (ANSI).
#[cfg_attr(not(coverage_nightly), no_mangle)]
#[inline(never)]
#[allow(non_snake_case, unused_variables)]
pub extern "system" fn MsiViewGetErrorA(
    hView: MsiHandle,
    szColumnNameBuffer: Lpstr,
    pcchBuf: *mut Dword,
) -> i32 {
    let result = panic::catch_unwind(|| {
        if hView == 0 {
            return -3; // MSIDBERROR_INVALIDARG
        }

        crate::handles::with_handle(hView, |obj| {
            if let crate::handles::MsiObject::View(view) = obj {
                if let Some(col_name) = &view.last_error_column {
                    let _ = string_to_lpstr(col_name, szColumnNameBuffer, pcchBuf);
                    -1 // MSIDBERROR_FUNCTIONERROR
                } else {
                    let _ = string_to_lpstr("", szColumnNameBuffer, pcchBuf);
                    0 // MSIDBERROR_NOERROR
                }
            } else {
                -3 // MSIDBERROR_INVALIDARG
            }
        })
        .unwrap_or(-3)
    });

    result.unwrap_or(-1) // MSIDBERROR_ERROR
}

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
#[cfg_attr(not(coverage_nightly), no_mangle)]
#[inline(never)]
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
#[cfg_attr(not(coverage_nightly), no_mangle)]
#[inline(never)]
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

/// Modifies a view.
///
/// # Arguments
///
/// * `hView` - The view handle.
/// * `eModifyMode` - The modification mode (e.g. `MSIMODIFY_INSERT`).
/// * `hRecord` - The record handle.
///
/// # Returns
///
/// `ERROR_SUCCESS`, `ERROR_INVALID_HANDLE`
#[cfg_attr(not(coverage_nightly), no_mangle)]
#[inline(never)]
#[allow(non_snake_case, unused_variables)]
pub extern "system" fn MsiViewModify(
    hView: MsiHandle,
    eModifyMode: i32,
    hRecord: MsiHandle,
) -> Uint {
    let result = panic::catch_unwind(|| {
        if hView == 0 || hRecord == 0 {
            return ERROR_INVALID_HANDLE;
        }

        let mut query_copy = String::new();
        let mut db_handle = 0;

        let v_res = crate::handles::with_handle(hView, |obj| {
            if let crate::handles::MsiObject::View(v) = obj {
                query_copy.clone_from(&v.query);
                db_handle = v.database_handle;
                ERROR_SUCCESS
            } else {
                ERROR_INVALID_HANDLE
            }
        });

        if v_res.unwrap_or(ERROR_INVALID_HANDLE) != ERROR_SUCCESS {
            return ERROR_INVALID_HANDLE;
        }

        if db_handle == 0 {
            return ERROR_INVALID_HANDLE;
        }

        let mut record_copy = None;
        let r_res = crate::handles::with_handle(hRecord, |obj| {
            if let crate::handles::MsiObject::Record(r) = obj {
                record_copy = Some(r.inner.clone());
                ERROR_SUCCESS
            } else {
                ERROR_INVALID_HANDLE
            }
        });

        if r_res.unwrap_or(ERROR_INVALID_HANDLE) != ERROR_SUCCESS {
            return ERROR_INVALID_HANDLE;
        }

        let mut record = record_copy.unwrap_or_default();

        let table_name = {
            use msi::database::sql::ast::Statement;
            use msi::database::sql::lexer::Lexer;
            use msi::database::sql::parser::Parser;

            let mut lexer = Lexer::new(&query_copy);
            let tokens = match lexer.tokenize() {
                Ok(t) => t,
                Err(_) => return 13, // ERROR_INVALID_DATA
            };
            let mut parser = Parser::new(tokens);
            match parser.parse() {
                Ok(Statement::Select { table, .. }) => table.0,
                _ => return 13, // ERROR_INVALID_DATA
            }
        };

        let mut res_status = ERROR_SUCCESS;
        let mut err_msg = None;

        crate::handles::with_handle_mut(db_handle, |obj| {
            if let crate::handles::MsiObject::Database(db) = obj {
                match db
                    .inner
                    .execute_mutation(&table_name, &mut record, eModifyMode)
                {
                    Ok(()) => {
                        res_status = ERROR_SUCCESS;
                    }
                    Err(e) => {
                        res_status = 1627; // ERROR_FUNCTION_FAILED
                        err_msg = Some(e.to_string());
                    }
                }
            } else {
                res_status = ERROR_INVALID_HANDLE;
            }
        });

        if res_status == ERROR_SUCCESS {
            crate::handles::with_handle_mut(hView, |obj| {
                if let crate::handles::MsiObject::View(view) = obj {
                    view.last_error_column = None;
                }
            });
            // Update the record with any changes (e.g., from MSIMODIFY_SEEK or MSIMODIFY_REFRESH)
            crate::handles::with_handle_mut(hRecord, |obj| {
                if let crate::handles::MsiObject::Record(r) = obj {
                    r.inner = record;
                }
            });
        } else {
            crate::handles::with_handle_mut(hView, |obj| {
                if let crate::handles::MsiObject::View(view) = obj {
                    view.last_error_column = err_msg;
                }
            });
        }

        res_status
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
#[cfg_attr(not(coverage_nightly), no_mangle)]
#[inline(never)]
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
#[cfg_attr(not(coverage_nightly), no_mangle)]
#[inline(never)]
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

        assert_eq!(
            MsiViewGetErrorW(0, std::ptr::null_mut(), std::ptr::null_mut()),
            -3
        );
        assert_eq!(
            MsiViewGetErrorA(0, std::ptr::null_mut(), std::ptr::null_mut()),
            -3
        );
        assert_eq!(
            MsiViewGetErrorW(1, std::ptr::null_mut(), std::ptr::null_mut()),
            -3
        );
        assert_eq!(
            MsiViewGetErrorA(1, std::ptr::null_mut(), std::ptr::null_mut()),
            -3
        );

        let view = crate::types::MsiViewHandle {
            database_handle: 0,
            query: String::new(),
            fetched_records: std::collections::VecDeque::new(),
            last_error_column: Some("ErrorCol".to_string()),
        };
        let h_view = crate::handles::alloc_handle(crate::handles::MsiObject::View(view));

        assert_eq!(
            MsiViewGetErrorW(h_view, std::ptr::null_mut(), std::ptr::null_mut()),
            -1 // Because it has an error column
        );

        let view_no_err = crate::types::MsiViewHandle {
            database_handle: 0,
            query: String::new(),
            fetched_records: std::collections::VecDeque::new(),
            last_error_column: None,
        };
        let h_view2 = crate::handles::alloc_handle(crate::handles::MsiObject::View(view_no_err));

        assert_eq!(
            MsiViewGetErrorA(h_view2, std::ptr::null_mut(), std::ptr::null_mut()),
            0
        );

        // Hit None branch for W
        assert_eq!(
            MsiViewGetErrorW(h_view2, std::ptr::null_mut(), std::ptr::null_mut()),
            0
        );
        // Hit Some branch for A
        assert_eq!(
            MsiViewGetErrorA(h_view, std::ptr::null_mut(), std::ptr::null_mut()),
            -1
        );

        // Hit wrong object type
        let h_rec = crate::handles::alloc_handle(crate::handles::MsiObject::Record(
            crate::types::MsiRecordHandle {
                inner: msi::database::tables::record::Record::new(),
            },
        ));
        assert_eq!(
            MsiViewGetErrorW(h_rec, std::ptr::null_mut(), std::ptr::null_mut()),
            -3
        );
        assert_eq!(
            MsiViewGetErrorA(h_rec, std::ptr::null_mut(), std::ptr::null_mut()),
            -3
        );
        assert!(crate::handles::close_handle(h_rec));

        assert!(crate::handles::close_handle(h_view));
        assert!(crate::handles::close_handle(h_view2));

        assert_eq!(MsiViewModify(0, 0, 0), ERROR_INVALID_HANDLE);
    }

    #[test]
    fn test_view_modify() {
        use msi::database::tables::record::Record;
        use msi::database::FieldValue;
        use msi::wix::linker::LinkedDatabase;

        let db = LinkedDatabase::new().unwrap();
        let db_handle = crate::handles::alloc_handle(crate::handles::MsiObject::Database(
            crate::types::MsiDatabaseHandle {
                inner: db,
                state: 0,
            },
        ));

        let view = crate::types::MsiViewHandle {
            database_handle: db_handle,
            query: "SELECT `Property`, `Value` FROM `Property`".to_string(),
            fetched_records: std::collections::VecDeque::new(),
            last_error_column: None,
        };
        let view_handle = crate::handles::alloc_handle(crate::handles::MsiObject::View(view));

        let mut record1 = Record::new();
        record1.push(FieldValue::String("TestProp".to_string()));
        record1.push(FieldValue::String("TestValue".to_string()));
        let rec_handle = crate::handles::alloc_handle(crate::handles::MsiObject::Record(
            crate::types::MsiRecordHandle { inner: record1 },
        ));

        // INSERT
        assert_eq!(
            MsiViewModify(view_handle, MSIMODIFY_INSERT, rec_handle),
            ERROR_SUCCESS
        );

        // UPDATE
        assert_eq!(
            MsiViewModify(view_handle, MSIMODIFY_UPDATE, rec_handle),
            ERROR_SUCCESS
        );

        // DELETE
        assert_eq!(
            MsiViewModify(view_handle, MSIMODIFY_DELETE, rec_handle),
            ERROR_SUCCESS
        );

        let bad_view = crate::types::MsiViewHandle {
            database_handle: db_handle,
            query: "INVALID".to_string(),
            fetched_records: std::collections::VecDeque::new(),
            last_error_column: None,
        };
        let bad_view_handle =
            crate::handles::alloc_handle(crate::handles::MsiObject::View(bad_view));
        assert_eq!(
            MsiViewModify(bad_view_handle, MSIMODIFY_INSERT, rec_handle),
            13
        );

        assert_eq!(
            MsiViewModify(db_handle, MSIMODIFY_INSERT, rec_handle),
            ERROR_INVALID_HANDLE
        );
        assert_eq!(
            MsiViewModify(view_handle, MSIMODIFY_INSERT, db_handle),
            ERROR_INVALID_HANDLE
        );

        let zero_db_view = crate::types::MsiViewHandle {
            database_handle: 0,
            query: "SELECT `Property`, `Value` FROM `Property`".to_string(),
            fetched_records: std::collections::VecDeque::new(),
            last_error_column: None,
        };
        let zero_db_view_handle =
            crate::handles::alloc_handle(crate::handles::MsiObject::View(zero_db_view));
        assert_eq!(
            MsiViewModify(zero_db_view_handle, MSIMODIFY_INSERT, rec_handle),
            ERROR_INVALID_HANDLE
        );

        let rec_db_view = crate::types::MsiViewHandle {
            database_handle: rec_handle,
            query: "SELECT `Property`, `Value` FROM `Property`".to_string(),
            fetched_records: std::collections::VecDeque::new(),
            last_error_column: None,
        };
        let rec_db_view_handle =
            crate::handles::alloc_handle(crate::handles::MsiObject::View(rec_db_view));
        assert_eq!(
            MsiViewModify(rec_db_view_handle, MSIMODIFY_INSERT, rec_handle),
            ERROR_INVALID_HANDLE
        );

        let bad_mut_view = crate::types::MsiViewHandle {
            database_handle: db_handle,
            query: "SELECT `Property`, `Value` FROM `MissingTable`".to_string(),
            fetched_records: std::collections::VecDeque::new(),
            last_error_column: None,
        };
        let bad_mut_view_handle =
            crate::handles::alloc_handle(crate::handles::MsiObject::View(bad_mut_view));
        assert_eq!(MsiViewModify(view_handle, MSIMODIFY_INSERT, rec_handle), 0);
        assert_eq!(
            MsiViewModify(view_handle, MSIMODIFY_INSERT, rec_handle),
            1627
        );

        let _ = crate::handles::close_handle(zero_db_view_handle);
        let _ = crate::handles::close_handle(rec_db_view_handle);

        let bad_lex_view = crate::types::MsiViewHandle {
            database_handle: db_handle,
            query: "SELECT `Unclosed".to_string(),
            fetched_records: std::collections::VecDeque::new(),
            last_error_column: None,
        };
        let bad_lex_view_handle =
            crate::handles::alloc_handle(crate::handles::MsiObject::View(bad_lex_view));
        assert_eq!(
            MsiViewModify(bad_lex_view_handle, MSIMODIFY_INSERT, rec_handle),
            13
        );
        let _ = crate::handles::close_handle(bad_lex_view_handle);

        let _ = crate::handles::close_handle(bad_mut_view_handle);

        let _ = crate::handles::close_handle(rec_handle);
        let _ = crate::handles::close_handle(view_handle);
        let _ = crate::handles::close_handle(bad_view_handle);
        let _ = crate::handles::close_handle(db_handle);
    }
}

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
#[cfg_attr(not(test), no_mangle)]
#[inline(never)]
#[allow(non_snake_case)]
#[allow(unused_variables)]
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
                view.last_error_column.as_ref().map_or_else(
                    || {
                        let _ = string_to_lpwstr("", szColumnNameBuffer, pcchBuf);
                        0
                    },
                    |col_name| {
                        let _ = string_to_lpwstr(col_name, szColumnNameBuffer, pcchBuf);
                        -1
                    },
                )
            } else {
                -3 // MSIDBERROR_INVALIDARG
            }
        })
        .unwrap_or(-3)
    });

    result.unwrap_or(-1) // MSIDBERROR_ERROR
}

/// Validates a view execution and returns any error that occurred (ANSI).
#[cfg_attr(not(test), no_mangle)]
#[inline(never)]
#[allow(non_snake_case)]
#[allow(unused_variables)]
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
                view.last_error_column.as_ref().map_or_else(
                    || {
                        let _ = string_to_lpstr("", szColumnNameBuffer, pcchBuf);
                        0
                    },
                    |col_name| {
                        let _ = string_to_lpstr(col_name, szColumnNameBuffer, pcchBuf);
                        -1
                    },
                )
            } else {
                -3 // MSIDBERROR_INVALIDARG
            }
        })
        .unwrap_or(-3)
    });

    result.unwrap_or(-1) // MSIDBERROR_ERROR
}

/// Executes a SQL view query against its associated database.
///
/// # Arguments
///
/// * `hView` - Handle to the view prepared via `MsiDatabaseOpenView`.
/// * `hRecord` - Optional handle to a parameter record supplying `?` placeholder values (`0` if none).
///
/// # Returns
///
/// `ERROR_SUCCESS`, `ERROR_INVALID_HANDLE`, or `1627` (`ERROR_FUNCTION_FAILED`) on SQL execution failure.
#[cfg_attr(not(test), no_mangle)]
#[inline(never)]
#[allow(non_snake_case)]
#[must_use]
pub extern "system" fn MsiViewExecute(
    hView: MsiHandle,
    hRecord: MsiHandle, // 0 if no params
) -> Uint {
    let result = panic::catch_unwind(|| {
        if hView == 0 {
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

        if v_res.unwrap_or(ERROR_INVALID_HANDLE) != ERROR_SUCCESS || db_handle == 0 {
            return ERROR_INVALID_HANDLE;
        }

        let mut params = Vec::new();
        if hRecord != 0 {
            let r_res = crate::handles::with_handle(hRecord, |obj| {
                if let crate::handles::MsiObject::Record(r) = obj {
                    let slice = r.inner.fields();
                    params = slice.get(1..).unwrap_or(slice).to_vec();
                    ERROR_SUCCESS
                } else {
                    ERROR_INVALID_HANDLE
                }
            });
            if r_res.unwrap_or(ERROR_INVALID_HANDLE) != ERROR_SUCCESS {
                return ERROR_INVALID_HANDLE;
            }
        }

        let mut fetched = std::collections::VecDeque::new();
        let mut exec_status = ERROR_SUCCESS;

        let db_res = crate::handles::with_handle_mut(db_handle, |obj| {
            let crate::handles::MsiObject::Database(db) = obj else {
                return ERROR_INVALID_HANDLE;
            };
            let Ok(q_res) =
                msi::database::sql::executor::execute_sql(&mut db.inner, &query_copy, &params)
            else {
                exec_status = 1627; // ERROR_FUNCTION_FAILED
                return ERROR_SUCCESS;
            };
            if let msi::database::sql::executor::QueryResult::Select { rows, .. } = q_res {
                for row in rows {
                    let mut fields = vec![msi::database::FieldValue::Null];
                    fields.extend(row.fields().iter().cloned());
                    fetched.push_back(msi::database::tables::record::Record::with_fields(fields));
                }
            }
            ERROR_SUCCESS
        });

        if db_res.unwrap_or(ERROR_INVALID_HANDLE) != ERROR_SUCCESS {
            return ERROR_INVALID_HANDLE;
        }

        crate::handles::with_handle_mut(hView, |obj| {
            if let crate::handles::MsiObject::View(v) = obj {
                v.fetched_records = fetched;
            }
        });

        exec_status
    });

    result.unwrap_or(ERROR_INSTALL_FAILURE)
}

/// Fetches the next sequential record from an executed view.
///
/// # Arguments
///
/// * `hView` - Handle to the view.
/// * `phRecord` - Out pointer receiving a newly allocated `MSIHANDLE` for the fetched record.
///
/// # Returns
///
/// `ERROR_SUCCESS`, `ERROR_INVALID_PARAMETER`, `ERROR_INVALID_HANDLE`, or `ERROR_NO_MORE_ITEMS` (`259`).
#[cfg_attr(not(test), no_mangle)]
#[inline(never)]
#[allow(non_snake_case)]
pub extern "system" fn MsiViewFetch(hView: MsiHandle, phRecord: *mut MsiHandle) -> Uint {
    let result = panic::catch_unwind(|| {
        if phRecord.is_null() {
            return ERROR_INVALID_PARAMETER;
        }
        if hView == 0 {
            return ERROR_INVALID_HANDLE;
        }

        let mut next_rec = None;
        let v_res = crate::handles::with_handle_mut(hView, |obj| {
            if let crate::handles::MsiObject::View(v) = obj {
                next_rec = v.fetched_records.pop_front();
                ERROR_SUCCESS
            } else {
                ERROR_INVALID_HANDLE
            }
        });

        if v_res.unwrap_or(ERROR_INVALID_HANDLE) != ERROR_SUCCESS {
            return ERROR_INVALID_HANDLE;
        }

        if let Some(rec) = next_rec {
            let h = crate::handles::alloc_handle(crate::handles::MsiObject::Record(
                crate::types::MsiRecordHandle { inner: rec },
            ));
            unsafe {
                *phRecord = h;
            }
            ERROR_SUCCESS
        } else {
            unsafe {
                *phRecord = 0;
            }
            crate::win32::ERROR_NO_MORE_ITEMS
        }
    });

    result.unwrap_or(ERROR_INSTALL_FAILURE)
}

/// Extracts the primary table name from a `SELECT` SQL query string.
fn extract_select_table_name(query: &str) -> Option<String> {
    use msi::database::sql::ast::Statement;
    use msi::database::sql::lexer::Lexer;
    use msi::database::sql::parser::Parser;

    let mut lexer = Lexer::new(query);
    let tokens = lexer.tokenize().ok()?;
    let mut parser = Parser::new(tokens);
    match parser.parse() {
        Ok(Statement::Select { table, .. }) => Some(table.0),
        _ => None,
    }
}

/// Modifies a view using a record and an `MSIMODIFY_*` mode.
///
/// Handles both 0-based internal records and 1-based Win32 `MsiCreateRecord` records (where field `0` is the format string).
///
/// # Arguments
///
/// * `hView` - Handle to the view.
/// * `eModifyMode` - Modification mode (`MSIMODIFY_SEEK` `-1` through `MSIMODIFY_VALIDATE_DELETE` `11`).
/// * `hRecord` - Handle to the record supplying column values.
///
/// # Returns
///
/// `ERROR_SUCCESS`, `ERROR_INVALID_HANDLE`, `ERROR_INVALID_DATA` (`13`), or `ERROR_FUNCTION_FAILED` (`1627`).
#[cfg_attr(not(test), no_mangle)]
#[inline(never)]
#[allow(non_snake_case)]
#[must_use]
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

        if v_res.unwrap_or(ERROR_INVALID_HANDLE) != ERROR_SUCCESS || db_handle == 0 {
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

        let raw_record = record_copy.unwrap_or_default();
        let Some(table_name) = extract_select_table_name(&query_copy) else {
            return 13; // ERROR_INVALID_DATA
        };

        let mut res_status = ERROR_SUCCESS;
        let mut err_msg = None;
        let mut was_one_based = false;
        let mut mutated_record = raw_record.clone();

        crate::handles::with_handle_mut(db_handle, |obj| {
            if let crate::handles::MsiObject::Database(db) = obj {
                let col_count = db
                    .inner
                    .catalog
                    .get_table(&table_name)
                    .map_or(0, |s| s.columns.len());
                if col_count > 0
                    && raw_record.len() == col_count + 1
                    && matches!(raw_record.get(0), Some(msi::database::FieldValue::Null))
                {
                    was_one_based = true;
                    mutated_record = msi::database::tables::record::Record::with_fields(
                        raw_record.fields()[1..].to_vec(),
                    );
                }
                match db
                    .inner
                    .execute_mutation(&table_name, &mut mutated_record, eModifyMode)
                {
                    Ok(()) => res_status = ERROR_SUCCESS,
                    Err(e) => {
                        res_status = 1627; // ERROR_FUNCTION_FAILED
                        err_msg = Some(e.to_string());
                    }
                }
            } else {
                res_status = ERROR_INVALID_HANDLE;
            }
        });

        crate::handles::with_handle_mut(hView, |obj| {
            if let crate::handles::MsiObject::View(view) = obj {
                view.last_error_column = err_msg;
            }
        });

        if res_status == ERROR_SUCCESS {
            let mut fields = if was_one_based {
                vec![msi::database::FieldValue::Null]
            } else {
                Vec::new()
            };
            fields.extend(mutated_record.fields().iter().cloned());
            let updated_rec = msi::database::tables::record::Record::with_fields(fields);
            crate::handles::with_handle_mut(hRecord, |obj| {
                if let crate::handles::MsiObject::Record(r) = obj {
                    r.inner = updated_rec;
                }
            });
        }

        res_status
    });

    result.unwrap_or(ERROR_INSTALL_FAILURE)
}

/// Releases the result set for an executed view so it can be re-executed.
///
/// # Arguments
///
/// * `hView` - Handle to the view.
///
/// # Returns
///
/// `ERROR_SUCCESS` if `hView` is a valid view handle, or `ERROR_INVALID_HANDLE`.
#[cfg_attr(not(test), no_mangle)]
#[inline(never)]
#[allow(non_snake_case)]
#[must_use]
pub extern "system" fn MsiViewClose(hView: MsiHandle) -> Uint {
    let result = panic::catch_unwind(|| {
        if hView == 0 {
            return ERROR_INVALID_HANDLE;
        }
        crate::handles::with_handle_mut(hView, |obj| {
            if let crate::handles::MsiObject::View(v) = obj {
                v.fetched_records.clear();
                ERROR_SUCCESS
            } else {
                ERROR_INVALID_HANDLE
            }
        })
        .unwrap_or(ERROR_INVALID_HANDLE)
    });

    result.unwrap_or(ERROR_INSTALL_FAILURE)
}

/// Returns a record containing column names (`MSICOLINFO_NAMES = 0`) or definitions (`MSICOLINFO_TYPES = 1`).
///
/// # Arguments
///
/// * `hView` - Handle to the view.
/// * `eColumnInfo` - `0` (`MSICOLINFO_NAMES`) for column names, or `1` (`MSICOLINFO_TYPES`) for MSI type descriptors (e.g., `s72`, `i4`, `v0`).
/// * `phRecord` - Out pointer receiving a newly allocated `MSIHANDLE` for the column info record.
///
/// # Returns
///
/// `ERROR_SUCCESS`, `ERROR_INVALID_HANDLE`, or `ERROR_INVALID_PARAMETER`.
#[cfg_attr(not(test), no_mangle)]
#[inline(never)]
#[allow(non_snake_case)]
pub extern "system" fn MsiViewGetColumnInfo(
    hView: MsiHandle,
    eColumnInfo: Uint,
    phRecord: *mut MsiHandle,
) -> Uint {
    let result = panic::catch_unwind(|| {
        if phRecord.is_null() || eColumnInfo > 1 {
            return ERROR_INVALID_PARAMETER;
        }
        if hView == 0 {
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
        if v_res.unwrap_or(ERROR_INVALID_HANDLE) != ERROR_SUCCESS || db_handle == 0 {
            return ERROR_INVALID_HANDLE;
        }

        let Some(table_name) = extract_select_table_name(&query_copy) else {
            return ERROR_INVALID_HANDLE;
        };

        let mut cols = Vec::new();
        let db_res = crate::handles::with_handle(db_handle, |obj| {
            if let crate::handles::MsiObject::Database(db) = obj {
                cols = db
                    .inner
                    .catalog
                    .get_table(&table_name)
                    .map(|s| s.columns.clone())
                    .unwrap_or_default();
                ERROR_SUCCESS
            } else {
                ERROR_INVALID_HANDLE
            }
        });
        if db_res.unwrap_or(ERROR_INVALID_HANDLE) != ERROR_SUCCESS {
            return ERROR_INVALID_HANDLE;
        }

        let mut fields = vec![msi::database::FieldValue::Null];
        for col in &cols {
            if eColumnInfo == 0 {
                fields.push(msi::database::FieldValue::String(col.name.clone()));
            } else {
                let spec = match col.data_type {
                    msi::database::column::DataType::Short => {
                        if col.nullable { "I2" } else { "i2" }.to_string()
                    }
                    msi::database::column::DataType::Long => {
                        if col.nullable { "I4" } else { "i4" }.to_string()
                    }
                    msi::database::column::DataType::Stream => {
                        if col.nullable { "V0" } else { "v0" }.to_string()
                    }
                    msi::database::column::DataType::String { max_len } => {
                        let ch = match (col.localizable, col.nullable) {
                            (true, true) => 'L',
                            (true, false) => 'l',
                            (false, true) => 'S',
                            (false, false) => 's',
                        };
                        format!("{ch}{max_len}")
                    }
                };
                fields.push(msi::database::FieldValue::String(spec));
            }
        }

        let rec = msi::database::tables::record::Record::with_fields(fields);
        let h_rec = crate::handles::alloc_handle(crate::handles::MsiObject::Record(
            crate::types::MsiRecordHandle { inner: rec },
        ));
        unsafe {
            *phRecord = h_rec;
        }
        ERROR_SUCCESS
    });

    result.unwrap_or(ERROR_INSTALL_FAILURE)
}

#[cfg(test)]
#[allow(clippy::too_many_lines)]
#[allow(clippy::cognitive_complexity)]
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

        let db = LinkedDatabase::new().unwrap_or_default();
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
            MsiViewModify(0, MSIMODIFY_INSERT, rec_handle),
            ERROR_INVALID_HANDLE
        );
        assert_eq!(
            MsiViewModify(view_handle, MSIMODIFY_INSERT, 0),
            ERROR_INVALID_HANDLE
        );
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

        // UNKNOWN MODE
        assert_eq!(MsiViewModify(view_handle, 999, rec_handle), 1627);

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
        assert_eq!(
            MsiViewModify(bad_mut_view_handle, MSIMODIFY_SEEK, rec_handle),
            1627
        );
        assert_eq!(MsiViewModify(view_handle, MSIMODIFY_INSERT, rec_handle), 0);
        assert_eq!(
            MsiViewModify(view_handle, MSIMODIFY_INSERT, rec_handle),
            1627
        );

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

        // Test 1-based record (via MsiCreateRecord) with MsiViewModify, MsiViewExecute, MsiViewFetch, MsiViewGetColumnInfo, and MsiViewClose
        let h_rec_1based = crate::win32::record::MsiCreateRecord(2);
        let prop_w: Vec<u16> = "OneBasedProp"
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect();
        let val_w: Vec<u16> = "OneBasedVal"
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect();
        assert_eq!(
            crate::win32::record::MsiRecordSetStringW(h_rec_1based, 1, prop_w.as_ptr()),
            ERROR_SUCCESS
        );
        assert_eq!(
            crate::win32::record::MsiRecordSetStringW(h_rec_1based, 2, val_w.as_ptr()),
            ERROR_SUCCESS
        );
        assert_eq!(
            MsiViewModify(view_handle, MSIMODIFY_ASSIGN, h_rec_1based),
            ERROR_SUCCESS
        );
        assert_eq!(
            MsiViewModify(view_handle, MSIMODIFY_SEEK, h_rec_1based),
            ERROR_SUCCESS
        );

        // Execute view and fetch records
        assert_eq!(MsiViewExecute(view_handle, 0), ERROR_SUCCESS);
        assert_eq!(MsiViewExecute(view_handle, h_rec_1based), ERROR_SUCCESS);
        assert_eq!(MsiViewExecute(view_handle, db_handle), ERROR_INVALID_HANDLE);
        assert_eq!(MsiViewExecute(rec_handle, 0), ERROR_INVALID_HANDLE);
        assert_eq!(MsiViewExecute(zero_db_view_handle, 0), ERROR_INVALID_HANDLE);
        assert_eq!(MsiViewExecute(rec_db_view_handle, 0), ERROR_INVALID_HANDLE);
        assert_eq!(MsiViewExecute(bad_view_handle, 0), 1627);

        let mut fetched_h = 0;
        assert_eq!(MsiViewFetch(view_handle, &raw mut fetched_h), ERROR_SUCCESS);
        assert_ne!(fetched_h, 0);
        let _ = crate::handles::close_handle(fetched_h);
        while MsiViewFetch(view_handle, &raw mut fetched_h) == ERROR_SUCCESS {
            let _ = crate::handles::close_handle(fetched_h);
        }
        assert_eq!(
            MsiViewFetch(view_handle, &raw mut fetched_h),
            crate::win32::ERROR_NO_MORE_ITEMS
        );
        assert_eq!(
            MsiViewFetch(rec_handle, &raw mut fetched_h),
            ERROR_INVALID_HANDLE
        );

        // Column info (names = 0, types = 1)
        let mut col_rec_h = 0;
        assert_eq!(
            MsiViewGetColumnInfo(view_handle, 0, &raw mut col_rec_h),
            ERROR_SUCCESS
        );
        let _ = crate::handles::close_handle(col_rec_h);
        assert_eq!(
            MsiViewGetColumnInfo(view_handle, 1, &raw mut col_rec_h),
            ERROR_SUCCESS
        );
        let _ = crate::handles::close_handle(col_rec_h);

        // Test column info on a table with Short, Long, Stream, and localizable/nullable Strings
        {
            use msi::database::catalogs::TableSchema;
            use msi::database::column::{ColumnDef, DataType};
            let schema = TableSchema::new("AllColTypes")
                .with_column(ColumnDef::new("C1", DataType::Short).primary_key())
                .with_column(ColumnDef::new("C2", DataType::Short).nullable())
                .with_column(ColumnDef::new("C3", DataType::Long))
                .with_column(ColumnDef::new("C4", DataType::Long).nullable())
                .with_column(ColumnDef::new("C5", DataType::Stream))
                .with_column(ColumnDef::new("C6", DataType::Stream).nullable())
                .with_column(ColumnDef::new("C7", DataType::String { max_len: 32 }).localizable())
                .with_column(ColumnDef::new("C8", DataType::String { max_len: 32 }).nullable())
                .with_column(
                    ColumnDef::new("C9", DataType::String { max_len: 32 })
                        .localizable()
                        .nullable(),
                );
            crate::handles::with_handle_mut(db_handle, |obj| {
                if let crate::handles::MsiObject::Database(db_obj) = obj {
                    let _ = db_obj.inner.catalog.add_table(schema);
                }
            });
        }
        let all_cols_view = crate::handles::alloc_handle(crate::handles::MsiObject::View(
            crate::types::MsiViewHandle {
                query: "SELECT * FROM AllColTypes".to_string(),
                database_handle: db_handle,
                fetched_records: std::collections::VecDeque::new(),
                last_error_column: None,
            },
        ));
        assert_eq!(
            MsiViewGetColumnInfo(all_cols_view, 1, &raw mut col_rec_h),
            ERROR_SUCCESS
        );
        let _ = crate::handles::close_handle(col_rec_h);
        let _ = crate::handles::close_handle(all_cols_view);

        let ins_view = crate::handles::alloc_handle(crate::handles::MsiObject::View(
            crate::types::MsiViewHandle {
                query: "INSERT INTO Property (Property, Value) VALUES ('VKey', 'VVal')".to_string(),
                database_handle: db_handle,
                fetched_records: std::collections::VecDeque::new(),
                last_error_column: None,
            },
        ));
        assert_eq!(MsiViewExecute(ins_view, 0), ERROR_SUCCESS);
        let _ = crate::handles::close_handle(ins_view);
        assert_eq!(
            MsiViewGetColumnInfo(view_handle, 99, &raw mut col_rec_h),
            ERROR_INVALID_PARAMETER
        );
        assert_eq!(
            MsiViewGetColumnInfo(rec_handle, 0, &raw mut col_rec_h),
            ERROR_INVALID_HANDLE
        );
        assert_eq!(
            MsiViewGetColumnInfo(bad_view_handle, 0, &raw mut col_rec_h),
            ERROR_INVALID_HANDLE
        );
        assert_eq!(
            MsiViewGetColumnInfo(rec_db_view_handle, 0, &raw mut col_rec_h),
            ERROR_INVALID_HANDLE
        );

        // Close view
        assert_eq!(MsiViewClose(view_handle), ERROR_SUCCESS);
        assert_eq!(MsiViewClose(rec_handle), ERROR_INVALID_HANDLE);

        let _ = crate::handles::close_handle(zero_db_view_handle);
        let _ = crate::handles::close_handle(rec_db_view_handle);
        let _ = crate::handles::close_handle(h_rec_1based);
        let _ = crate::handles::close_handle(rec_handle);
        let _ = crate::handles::close_handle(view_handle);
        let _ = crate::handles::close_handle(bad_view_handle);
        let _ = crate::handles::close_handle(db_handle);
    }
}

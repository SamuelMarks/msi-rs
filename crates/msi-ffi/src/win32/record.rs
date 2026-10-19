//! Record management Win32 `stdcall` API endpoints.
//!
//! Provides `MsiCreateRecord`, `MsiRecordSetInteger`, `MsiRecordSetString`,
//! `MsiRecordGetInteger`, `MsiRecordGetString`, `MsiRecordDataSize`,
//! `MsiRecordClearData`, and `MsiGetLastErrorRecord`.

use std::cell::RefCell;
use std::panic;

use crate::handles::{
    alloc_handle, with_handle, with_handle_mut, MsiHandle, MsiObject, MSI_NULL_HANDLE,
};
use crate::types::MsiRecordHandle;
use crate::win32::strings::{
    lpcstr_to_string, lpcwstr_to_string, string_to_lpstr, string_to_lpwstr,
};
use crate::win32::{
    Dword, Lpcstr, Lpcwstr, Lpstr, Lpwstr, Uint, ERROR_INSTALL_FAILURE, ERROR_INVALID_HANDLE,
    ERROR_SUCCESS,
};

use msi::database::tables::record::{FieldValue, Record, MSI_NULL_INTEGER_32};

thread_local! {
    /// Thread-local storage holding the most recent installer error [`Record`] for `MsiGetLastErrorRecord`.
    static LAST_ERROR_RECORD: RefCell<Option<Record>> = const { RefCell::new(None) };
}

/// Sets the thread-local installer error [`Record`] to be retrieved by [`MsiGetLastErrorRecord`].
///
/// # Arguments
///
/// * `record` - Structured [`Record`] containing the error code and diagnostic fields.
pub fn set_last_error_record(record: Record) {
    LAST_ERROR_RECORD.with(|cell| {
        *cell.borrow_mut() = Some(record);
    });
}

/// Clears the thread-local installer error [`Record`].
pub fn clear_last_error_record() {
    LAST_ERROR_RECORD.with(|cell| {
        *cell.borrow_mut() = None;
    });
}

/// Helper setting a 1-based (or 0-based format) field in a [`Record`], extending with `FieldValue::Null` as needed.
fn set_record_field(rec: &mut Record, i_field: Uint, value: FieldValue) {
    let idx = i_field as usize;
    while rec.len() <= idx {
        rec.push(FieldValue::Null);
    }
    rec.set(idx, value);
}

/// Creates a new record object with the specified number of fields.
///
/// Allocates `cParams + 1` slots (field `0` for the format string plus `1..=cParams` data fields),
/// matching Wine's `MSI_CreateRecord`. Returns `MSI_NULL_HANDLE` if `cParams > 65535`.
///
/// # Arguments
///
/// * `cParams` - Number of data fields the record will hold (maximum `65535`).
///
/// # Returns
///
/// A new `MSIHANDLE` for the created record, or `MSI_NULL_HANDLE` (`0`) if `cParams` exceeds `65535`.
#[cfg_attr(not(test), no_mangle)]
#[inline(never)]
#[allow(non_snake_case)]
#[must_use]
pub extern "system" fn MsiCreateRecord(cParams: Uint) -> MsiHandle {
    let result = panic::catch_unwind(|| {
        if cParams > 65535 {
            return MSI_NULL_HANDLE;
        }
        let rec = Record::with_fields(vec![FieldValue::Null; (cParams as usize).saturating_add(1)]);
        let obj = MsiObject::Record(MsiRecordHandle { inner: rec });
        alloc_handle(obj)
    });

    result.unwrap_or(MSI_NULL_HANDLE)
}

/// Sets an integer field in a record.
///
/// Setting `iValue` to `MSI_NULL_INTEGER` (`0x80000000` / `i32::MIN`) clears the field to `Null`.
///
/// # Arguments
///
/// * `hRecord` - Handle to the record.
/// * `iField` - 1-based field index (`0` is the format string field).
/// * `iValue` - Integer value to store, or `MSI_NULL_INTEGER` to set the field to null.
///
/// # Returns
///
/// `ERROR_SUCCESS` on success, or `ERROR_INVALID_HANDLE` if `hRecord` is not a valid record handle.
#[cfg_attr(not(test), no_mangle)]
#[inline(never)]
#[allow(non_snake_case)]
#[must_use]
pub extern "system" fn MsiRecordSetInteger(hRecord: MsiHandle, iField: Uint, iValue: i32) -> Uint {
    let result = panic::catch_unwind(|| {
        with_handle_mut(hRecord, |obj| {
            if let MsiObject::Record(rec_handle) = obj {
                let val = if iValue == MSI_NULL_INTEGER_32 {
                    FieldValue::Null
                } else {
                    FieldValue::Long(iValue)
                };
                set_record_field(&mut rec_handle.inner, iField, val);
                ERROR_SUCCESS
            } else {
                ERROR_INVALID_HANDLE
            }
        })
        .unwrap_or(ERROR_INVALID_HANDLE)
    });

    result.unwrap_or(ERROR_INSTALL_FAILURE)
}

/// Shared helper setting a string field on `h_record` or clearing it to `Null` if empty.
fn record_set_string_inner(h_record: MsiHandle, i_field: Uint, val: String) -> Uint {
    with_handle_mut(h_record, |obj| {
        if let MsiObject::Record(rec_handle) = obj {
            let field_val = if val.is_empty() {
                FieldValue::Null
            } else {
                FieldValue::String(val)
            };
            set_record_field(&mut rec_handle.inner, i_field, field_val);
            ERROR_SUCCESS
        } else {
            ERROR_INVALID_HANDLE
        }
    })
    .unwrap_or(ERROR_INVALID_HANDLE)
}

/// Sets a string field in a record in Unicode (`W`).
///
/// Passing a null pointer or an empty string clears the field to `Null`, matching Wine's `msi_record_set_string`.
///
/// # Arguments
///
/// * `hRecord` - Handle to the record.
/// * `iField` - 1-based field index (`0` is the format string field).
/// * `szValue` - Null-terminated UTF-16 string pointer, or `NULL` to clear the field.
///
/// # Returns
///
/// `ERROR_SUCCESS` on success, or `ERROR_INVALID_HANDLE` if `hRecord` is not a valid record handle.
#[cfg_attr(not(test), no_mangle)]
#[inline(never)]
#[allow(non_snake_case)]
#[must_use]
pub extern "system" fn MsiRecordSetStringW(
    hRecord: MsiHandle,
    iField: Uint,
    szValue: Lpcwstr,
) -> Uint {
    panic::catch_unwind(|| {
        let val = lpcwstr_to_string(szValue).unwrap_or_default();
        record_set_string_inner(hRecord, iField, val)
    })
    .unwrap_or(ERROR_INSTALL_FAILURE)
}

/// Sets a string field in a record in ANSI (`A`).
///
/// Passing a null pointer or an empty string clears the field to `Null`.
///
/// # Arguments
///
/// * `hRecord` - Handle to the record.
/// * `iField` - 1-based field index (`0` is the format string field).
/// * `szValue` - Null-terminated ANSI string pointer, or `NULL` to clear the field.
///
/// # Returns
///
/// `ERROR_SUCCESS` on success, or `ERROR_INVALID_HANDLE` if `hRecord` is not a valid record handle.
#[cfg_attr(not(test), no_mangle)]
#[inline(never)]
#[allow(non_snake_case)]
#[must_use]
pub extern "system" fn MsiRecordSetStringA(
    hRecord: MsiHandle,
    iField: Uint,
    szValue: Lpcstr,
) -> Uint {
    panic::catch_unwind(|| {
        let val = lpcstr_to_string(szValue).unwrap_or_default();
        record_set_string_inner(hRecord, iField, val)
    })
    .unwrap_or(ERROR_INSTALL_FAILURE)
}

/// Gets an integer field from a record.
///
/// If the field contains a string that represents a valid base-10 integer, it is parsed and returned,
/// matching Wine's `MSI_RecordGetInteger`.
///
/// # Arguments
///
/// * `hRecord` - Handle to the record.
/// * `iField` - 1-based field index.
///
/// # Returns
///
/// The integer value of the field, `0` for null/out-of-bounds fields, or `MSI_NULL_INTEGER` (`i32::MIN`) if `hRecord` is invalid or a string field is not a valid integer.
#[cfg_attr(not(test), no_mangle)]
#[inline(never)]
#[allow(non_snake_case)]
#[must_use]
pub extern "system" fn MsiRecordGetInteger(hRecord: MsiHandle, iField: Uint) -> i32 {
    let result = panic::catch_unwind(|| {
        with_handle(hRecord, |obj| {
            if let MsiObject::Record(rec_handle) = obj {
                match rec_handle.inner.fields().get(iField as usize) {
                    Some(FieldValue::Long(v)) => *v,
                    Some(FieldValue::Short(v)) => i32::from(*v),
                    Some(FieldValue::String(s)) => s.parse::<i32>().unwrap_or(MSI_NULL_INTEGER_32),
                    _ => 0,
                }
            } else {
                MSI_NULL_INTEGER_32
            }
        })
        .unwrap_or(MSI_NULL_INTEGER_32)
    });

    result.unwrap_or(MSI_NULL_INTEGER_32)
}

/// Formats a record field as a string for `MsiRecordGetStringA` and `MsiRecordGetStringW`.
///
/// Integer fields (`FieldValue::Short` and `FieldValue::Long`) are formatted as base-10 decimal strings,
/// matching Wine's `MSI_RecordGetStringW` / `MSI_RecordGetStringA`.
fn format_record_field_as_string(rec: &Record, i_field: Uint) -> String {
    match rec.fields().get(i_field as usize) {
        Some(FieldValue::String(s)) => s.clone(),
        Some(FieldValue::Short(n)) => n.to_string(),
        Some(FieldValue::Long(n)) => n.to_string(),
        _ => String::new(),
    }
}

/// Gets a string field from a record in Unicode (`W`).
///
/// Automatically formats integer fields as decimal strings in accordance with the Windows Installer specification.
///
/// # Arguments
///
/// * `hRecord` - Handle to the record.
/// * `iField` - 1-based field index (`0` is the format string field).
/// * `szValueBuf` - Destination UTF-16 character buffer, or `NULL` to query required length.
/// * `pcchValueBuf` - In/out character count (excluding null terminator).
///
/// # Returns
///
/// `ERROR_SUCCESS`, `ERROR_MORE_DATA`, `ERROR_INVALID_PARAMETER`, or `ERROR_INVALID_HANDLE`.
#[cfg_attr(not(test), no_mangle)]
#[inline(never)]
#[allow(non_snake_case)]
pub extern "system" fn MsiRecordGetStringW(
    hRecord: MsiHandle,
    iField: Uint,
    szValueBuf: Lpwstr,
    pcchValueBuf: *mut Dword,
) -> Uint {
    let result = panic::catch_unwind(|| {
        with_handle(hRecord, |obj| {
            if let MsiObject::Record(rec_handle) = obj {
                let value = format_record_field_as_string(&rec_handle.inner, iField);
                string_to_lpwstr(&value, szValueBuf, pcchValueBuf)
            } else {
                ERROR_INVALID_HANDLE
            }
        })
        .unwrap_or(ERROR_INVALID_HANDLE)
    });

    result.unwrap_or(ERROR_INSTALL_FAILURE)
}

/// Gets a string field from a record in ANSI (`A`).
///
/// Automatically formats integer fields as decimal strings in accordance with the Windows Installer specification.
///
/// # Arguments
///
/// * `hRecord` - Handle to the record.
/// * `iField` - 1-based field index (`0` is the format string field).
/// * `szValueBuf` - Destination ANSI character buffer, or `NULL` to query required length.
/// * `pcchValueBuf` - In/out character count (excluding null terminator).
///
/// # Returns
///
/// `ERROR_SUCCESS`, `ERROR_MORE_DATA`, `ERROR_INVALID_PARAMETER`, or `ERROR_INVALID_HANDLE`.
#[cfg_attr(not(test), no_mangle)]
#[inline(never)]
#[allow(non_snake_case)]
pub extern "system" fn MsiRecordGetStringA(
    hRecord: MsiHandle,
    iField: Uint,
    szValueBuf: Lpstr,
    pcchValueBuf: *mut Dword,
) -> Uint {
    let result = panic::catch_unwind(|| {
        with_handle(hRecord, |obj| {
            if let MsiObject::Record(rec_handle) = obj {
                let value = format_record_field_as_string(&rec_handle.inner, iField);
                string_to_lpstr(&value, szValueBuf, pcchValueBuf)
            } else {
                ERROR_INVALID_HANDLE
            }
        })
        .unwrap_or(ERROR_INVALID_HANDLE)
    });

    result.unwrap_or(ERROR_INSTALL_FAILURE)
}

/// Returns the length of a record field (`msi.spec` Ordinal 114).
///
/// Matches Wine's `MsiRecordDataSize`:
/// - Returns `0` if `hRecord` is invalid, `uiField` is out of bounds, or the field is `Null`.
/// - Returns `4` (`sizeof(INT)`) if the field is an integer (`Short` or `Long`).
/// - Returns the character count (excluding the null terminator) if the field is a string.
/// - Returns the byte count if the field is a binary stream.
///
/// # Arguments
///
/// * `hRecord` - Handle to the record.
/// * `uiField` - 1-based field index.
///
/// # Returns
///
/// Data size of the specified field, or `0` if null or invalid.
#[cfg_attr(not(test), no_mangle)]
#[inline(never)]
#[allow(non_snake_case)]
#[must_use]
pub extern "system" fn MsiRecordDataSize(hRecord: MsiHandle, uiField: Uint) -> Uint {
    let result = panic::catch_unwind(|| {
        with_handle(hRecord, |obj| {
            if let MsiObject::Record(rec_handle) = obj {
                let idx = uiField as usize;
                match rec_handle.inner.fields().get(idx) {
                    Some(FieldValue::Short(_) | FieldValue::Long(_)) => 4,
                    Some(FieldValue::String(s)) => {
                        Uint::try_from(s.encode_utf16().count()).unwrap_or(0)
                    }
                    Some(FieldValue::Stream(_)) => rec_handle
                        .inner
                        .stream_len(idx)
                        .and_then(|len| Uint::try_from(len).ok())
                        .unwrap_or(0),
                    Some(FieldValue::Null) | None => 0,
                }
            } else {
                0
            }
        })
        .unwrap_or(0)
    });

    result.unwrap_or(0)
}

/// Clears all fields in a record to `Null` while preserving the record's field count.
///
/// # Arguments
///
/// * `hRecord` - Handle to the record.
///
/// # Returns
///
/// `ERROR_SUCCESS` on success, or `ERROR_INVALID_HANDLE` if `hRecord` is invalid.
#[cfg_attr(not(test), no_mangle)]
#[inline(never)]
#[allow(non_snake_case)]
#[must_use]
pub extern "system" fn MsiRecordClearData(hRecord: MsiHandle) -> Uint {
    let result = panic::catch_unwind(|| {
        with_handle_mut(hRecord, |obj| {
            if let MsiObject::Record(rec_handle) = obj {
                rec_handle.inner.clear_data();
                ERROR_SUCCESS
            } else {
                ERROR_INVALID_HANDLE
            }
        })
        .unwrap_or(ERROR_INVALID_HANDLE)
    });

    result.unwrap_or(ERROR_INSTALL_FAILURE)
}

/// Returns the error record that was last recorded on the calling thread, and clears the stored error record.
///
/// # Returns
///
/// Newly allocated `MSIHANDLE` to the error record, or `MSI_NULL_HANDLE` (`0`) if no error record is pending.
#[cfg_attr(not(test), no_mangle)]
#[inline(never)]
#[allow(non_snake_case)]
#[must_use]
pub extern "system" fn MsiGetLastErrorRecord() -> MsiHandle {
    let result = panic::catch_unwind(|| {
        let taken = LAST_ERROR_RECORD.with(|cell| cell.borrow_mut().take());
        taken.map_or(MSI_NULL_HANDLE, |rec| {
            alloc_handle(MsiObject::Record(MsiRecordHandle { inner: rec }))
        })
    });

    result.unwrap_or(MSI_NULL_HANDLE)
}

#[cfg(test)]
#[allow(clippy::too_many_lines)]
#[allow(clippy::cognitive_complexity)]
mod tests {
    use super::*;
    use crate::win32::ERROR_MORE_DATA;

    #[test]
    fn test_record_stubs() {
        let h = MsiCreateRecord(1);
        assert_ne!(h, MSI_NULL_HANDLE);

        assert_eq!(MsiRecordSetInteger(0, 1, 1), ERROR_INVALID_HANDLE);
        assert_eq!(MsiRecordSetInteger(h, 1, 42), ERROR_SUCCESS);
        assert_eq!(MsiRecordGetInteger(h, 1), 42);
        assert_eq!(MsiRecordGetInteger(h, 99), 0);

        let mut pcch = 0;
        assert_eq!(
            MsiRecordSetStringW(0, 1, std::ptr::null()),
            ERROR_INVALID_HANDLE
        );
        assert_eq!(
            MsiRecordSetStringA(0, 1, std::ptr::null()),
            ERROR_INVALID_HANDLE
        );
        assert_eq!(MsiRecordSetStringW(h, 1, std::ptr::null()), ERROR_SUCCESS);
        assert_eq!(MsiRecordSetStringA(h, 1, std::ptr::null()), ERROR_SUCCESS);

        assert_eq!(MsiRecordGetInteger(0, 1), -2_147_483_648);
        assert_eq!(MsiRecordGetInteger(h, 1), 0);

        // Also cover FieldValue::Short branch in MsiRecordGetInteger
        let short_rec = Record::with_fields(vec![FieldValue::Null, FieldValue::Short(7)]);
        let h_short = alloc_handle(MsiObject::Record(MsiRecordHandle { inner: short_rec }));
        assert_eq!(MsiRecordGetInteger(h_short, 1), 7);
        assert_eq!(
            MsiRecordGetStringW(h_short, 1, std::ptr::null_mut(), &raw mut pcch),
            ERROR_MORE_DATA
        );
        assert_eq!(
            MsiRecordGetStringA(h_short, 1, std::ptr::null_mut(), &raw mut pcch),
            ERROR_MORE_DATA
        );
        let _ = crate::handles::close_handle(h_short);

        assert_eq!(
            MsiRecordGetStringW(0, 1, std::ptr::null_mut(), std::ptr::null_mut()),
            ERROR_INVALID_HANDLE
        );
        assert_eq!(
            MsiRecordGetStringA(0, 1, std::ptr::null_mut(), std::ptr::null_mut()),
            ERROR_INVALID_HANDLE
        );
        assert_eq!(
            MsiRecordGetStringW(h, 1, std::ptr::null_mut(), &raw mut pcch),
            ERROR_MORE_DATA
        );
        assert_eq!(
            MsiRecordGetStringA(h, 1, std::ptr::null_mut(), &raw mut pcch),
            ERROR_MORE_DATA
        );

        assert_eq!(MsiRecordClearData(0), ERROR_INVALID_HANDLE);
        assert_eq!(MsiRecordClearData(h), ERROR_SUCCESS);
        let db = msi::wix::linker::LinkedDatabase::new().unwrap_or_default();
        let h_db = alloc_handle(MsiObject::Database(crate::types::MsiDatabaseHandle {
            inner: db,
            state: 0,
        }));
        assert_eq!(MsiRecordSetInteger(h_db, 1, 1), ERROR_INVALID_HANDLE);
        assert_eq!(
            MsiRecordSetStringW(h_db, 1, std::ptr::null()),
            ERROR_INVALID_HANDLE
        );
        assert_eq!(
            MsiRecordSetStringA(h_db, 1, std::ptr::null()),
            ERROR_INVALID_HANDLE
        );
        assert_eq!(MsiRecordGetInteger(h_db, 1), -2_147_483_648);
        assert_eq!(
            MsiRecordGetStringW(h_db, 1, std::ptr::null_mut(), &raw mut pcch),
            ERROR_INVALID_HANDLE
        );
        assert_eq!(
            MsiRecordGetStringA(h_db, 1, std::ptr::null_mut(), &raw mut pcch),
            ERROR_INVALID_HANDLE
        );
        assert_eq!(MsiRecordClearData(h_db), ERROR_INVALID_HANDLE);
        assert_eq!(MsiRecordDataSize(h_db, 1), 0);
        assert_eq!(MsiRecordDataSize(0, 1), 0);
        assert_eq!(MsiCreateRecord(70_000), MSI_NULL_HANDLE);

        // Test integer formatting in MsiRecordGetStringW/A and MsiRecordDataSize
        assert_eq!(MsiRecordSetInteger(h, 1, 12345), ERROR_SUCCESS);
        assert_eq!(MsiRecordDataSize(h, 1), 4);
        assert_eq!(MsiRecordDataSize(h, 99), 0);
        let mut str_buf_w = [0u16; 16];
        let mut str_len_w: Dword = 16;
        assert_eq!(
            MsiRecordGetStringW(h, 1, str_buf_w.as_mut_ptr(), &raw mut str_len_w),
            ERROR_SUCCESS
        );
        assert_eq!(str_len_w, 5);
        assert_eq!(
            String::from_utf16_lossy(&str_buf_w[..5]),
            "12345".to_string()
        );

        let mut str_buf_a = [0i8; 16];
        let mut str_len_a: Dword = 16;
        assert_eq!(
            MsiRecordGetStringA(h, 1, str_buf_a.as_mut_ptr(), &raw mut str_len_a),
            ERROR_SUCCESS
        );
        assert_eq!(str_len_a, 5);

        // Setting MSI_NULL_INTEGER_32 clears field to Null
        assert_eq!(
            MsiRecordSetInteger(h, 1, MSI_NULL_INTEGER_32),
            ERROR_SUCCESS
        );
        assert_eq!(MsiRecordDataSize(h, 1), 0);

        // Setting non-empty string and parsing as integer
        let num_w: Vec<u16> = "-987".encode_utf16().chain(std::iter::once(0)).collect();
        assert_eq!(MsiRecordSetStringW(h, 1, num_w.as_ptr()), ERROR_SUCCESS);
        assert_eq!(MsiRecordDataSize(h, 1), 4);
        assert_eq!(MsiRecordGetInteger(h, 1), -987);

        let non_num_a = b"not_a_number\0";
        assert_eq!(
            MsiRecordSetStringA(h, 2, non_num_a.as_ptr().cast()),
            ERROR_SUCCESS
        );
        assert_eq!(MsiRecordDataSize(h, 2), 12);
        assert_eq!(MsiRecordGetInteger(h, 2), MSI_NULL_INTEGER_32);

        // Stream data size
        with_handle_mut(h, |obj| {
            if let MsiObject::Record(r) = obj {
                r.inner.set_stream_bytes(1, vec![1, 2, 3, 4, 5, 6]);
            }
        });
        assert_eq!(MsiRecordDataSize(h, 1), 6);

        // MsiGetLastErrorRecord lifecycle
        clear_last_error_record();
        assert_eq!(MsiGetLastErrorRecord(), MSI_NULL_HANDLE);
        set_last_error_record(Record::with_fields(vec![
            FieldValue::Null,
            FieldValue::Long(2203),
        ]));
        let err_h = MsiGetLastErrorRecord();
        assert_ne!(err_h, MSI_NULL_HANDLE);
        assert_eq!(MsiRecordGetInteger(err_h, 1), 2203);
        assert_eq!(MsiGetLastErrorRecord(), MSI_NULL_HANDLE);

        let _ = crate::handles::close_handle(err_h);
        let _ = crate::handles::close_handle(h_db);
        let _ = crate::handles::close_handle(h);
    }
}

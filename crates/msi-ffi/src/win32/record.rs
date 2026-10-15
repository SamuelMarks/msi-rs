//! Record management Win32 `stdcall` API endpoints.
//!
//! Provides `MsiCreateRecord`, `MsiRecordSetInteger`, `MsiRecordSetString`,
//! `MsiRecordGetInteger`, `MsiRecordGetString`, `MsiRecordClearData`.

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

use msi::database::tables::record::{FieldValue, Record};

/// Helper setting a 1-based (or 0-based format) field in a [`Record`], extending with `FieldValue::Null` as needed.
fn set_record_field(rec: &mut Record, i_field: Uint, value: FieldValue) {
    let idx = i_field as usize;
    let mut fields = rec.fields().to_vec();
    if fields.len() <= idx {
        fields.resize(idx + 1, FieldValue::Null);
    }
    fields[idx] = value;
    *rec = Record::with_fields(fields);
}

/// Creates a new record object with a specified number of fields.
///
/// # Arguments
///
/// * `cParams` - Number of fields (0-based, plus field 0 for format).
///
/// # Returns
///
/// The new record handle, or `MSI_NULL_HANDLE` on error.
#[cfg_attr(not(test), no_mangle)]
#[inline(never)]
#[allow(non_snake_case, unused_variables)]
#[must_use]
pub extern "system" fn MsiCreateRecord(cParams: Uint) -> MsiHandle {
    let result = panic::catch_unwind(|| {
        let rec = Record::new();
        let obj = MsiObject::Record(MsiRecordHandle { inner: rec });
        alloc_handle(obj)
    });

    result.unwrap_or(MSI_NULL_HANDLE)
}

/// Sets an integer field in a record.
///
/// # Arguments
///
/// * `hRecord` - The record handle.
/// * `iField` - Field index (1-based, 0 is format).
/// * `iValue` - The integer value to set.
///
/// # Returns
///
/// `ERROR_SUCCESS`, `ERROR_INVALID_HANDLE`
#[cfg_attr(not(test), no_mangle)]
#[inline(never)]
#[allow(non_snake_case, unused_variables)]
#[must_use]
pub extern "system" fn MsiRecordSetInteger(hRecord: MsiHandle, iField: Uint, iValue: i32) -> Uint {
    let result = panic::catch_unwind(|| {
        with_handle_mut(hRecord, |obj| {
            if let MsiObject::Record(rec_handle) = obj {
                set_record_field(&mut rec_handle.inner, iField, FieldValue::Long(iValue));
                ERROR_SUCCESS
            } else {
                ERROR_INVALID_HANDLE
            }
        })
        .unwrap_or(ERROR_INVALID_HANDLE)
    });

    result.unwrap_or(ERROR_INSTALL_FAILURE)
}

/// Sets a string field in a record in Unicode (`W`).
///
/// # Arguments
///
/// * `hRecord` - The record handle.
/// * `iField` - Field index (1-based, 0 is format).
/// * `szValue` - The string value to set.
///
/// # Returns
///
/// `ERROR_SUCCESS`, `ERROR_INVALID_HANDLE`, `ERROR_INVALID_PARAMETER`
#[cfg_attr(not(test), no_mangle)]
#[inline(never)]
#[allow(non_snake_case, unused_variables)]
#[must_use]
pub extern "system" fn MsiRecordSetStringW(
    hRecord: MsiHandle,
    iField: Uint,
    szValue: Lpcwstr,
) -> Uint {
    let result = panic::catch_unwind(|| {
        let val = lpcwstr_to_string(szValue).unwrap_or_default();

        with_handle_mut(hRecord, |obj| {
            if let MsiObject::Record(rec_handle) = obj {
                set_record_field(&mut rec_handle.inner, iField, FieldValue::String(val));
                ERROR_SUCCESS
            } else {
                ERROR_INVALID_HANDLE
            }
        })
        .unwrap_or(ERROR_INVALID_HANDLE)
    });

    result.unwrap_or(ERROR_INSTALL_FAILURE)
}

/// Sets a string field in a record in ANSI (`A`).
#[cfg_attr(not(test), no_mangle)]
#[inline(never)]
#[allow(non_snake_case, unused_variables)]
#[must_use]
pub extern "system" fn MsiRecordSetStringA(
    hRecord: MsiHandle,
    iField: Uint,
    szValue: Lpcstr,
) -> Uint {
    let result = panic::catch_unwind(|| {
        let val = lpcstr_to_string(szValue).unwrap_or_default();

        with_handle_mut(hRecord, |obj| {
            if let MsiObject::Record(rec_handle) = obj {
                set_record_field(&mut rec_handle.inner, iField, FieldValue::String(val));
                ERROR_SUCCESS
            } else {
                ERROR_INVALID_HANDLE
            }
        })
        .unwrap_or(ERROR_INVALID_HANDLE)
    });

    result.unwrap_or(ERROR_INSTALL_FAILURE)
}

/// Gets an integer field from a record.
///
/// # Returns
///
/// The integer value, or `MSI_NULL_INTEGER` (0x80000000) on error.
#[cfg_attr(not(test), no_mangle)]
#[inline(never)]
#[allow(non_snake_case, unused_variables)]
#[must_use]
pub extern "system" fn MsiRecordGetInteger(hRecord: MsiHandle, iField: Uint) -> i32 {
    let result = panic::catch_unwind(|| {
        with_handle(hRecord, |obj| {
            if let MsiObject::Record(rec_handle) = obj {
                match rec_handle.inner.fields().get(iField as usize) {
                    Some(FieldValue::Long(v)) => *v,
                    Some(FieldValue::Short(v)) => i32::from(*v),
                    _ => 0,
                }
            } else {
                -2_147_483_648 // MSI_NULL_INTEGER
            }
        })
        .unwrap_or(-2_147_483_648)
    });

    result.unwrap_or(-2_147_483_648)
}

/// Gets a string field from a record in Unicode (`W`).
#[cfg_attr(not(test), no_mangle)]
#[inline(never)]
#[allow(non_snake_case, unused_variables)]
pub extern "system" fn MsiRecordGetStringW(
    hRecord: MsiHandle,
    iField: Uint,
    szValueBuf: Lpwstr,
    pcchValueBuf: *mut Dword,
) -> Uint {
    let result = panic::catch_unwind(|| {
        with_handle(hRecord, |obj| {
            if let MsiObject::Record(rec_handle) = obj {
                let value = match rec_handle.inner.fields().get(iField as usize) {
                    Some(FieldValue::String(s)) => s.as_str(),
                    _ => "",
                };
                string_to_lpwstr(value, szValueBuf, pcchValueBuf)
            } else {
                ERROR_INVALID_HANDLE
            }
        })
        .unwrap_or(ERROR_INVALID_HANDLE)
    });

    result.unwrap_or(ERROR_INSTALL_FAILURE)
}

/// Gets a string field from a record in ANSI (`A`).
#[cfg_attr(not(test), no_mangle)]
#[inline(never)]
#[allow(non_snake_case, unused_variables)]
pub extern "system" fn MsiRecordGetStringA(
    hRecord: MsiHandle,
    iField: Uint,
    szValueBuf: Lpstr,
    pcchValueBuf: *mut Dword,
) -> Uint {
    let result = panic::catch_unwind(|| {
        with_handle(hRecord, |obj| {
            if let MsiObject::Record(rec_handle) = obj {
                let value = match rec_handle.inner.fields().get(iField as usize) {
                    Some(FieldValue::String(s)) => s.as_str(),
                    _ => "",
                };
                string_to_lpstr(value, szValueBuf, pcchValueBuf)
            } else {
                ERROR_INVALID_HANDLE
            }
        })
        .unwrap_or(ERROR_INVALID_HANDLE)
    });

    result.unwrap_or(ERROR_INSTALL_FAILURE)
}

/// Clears all fields in a record.
#[cfg_attr(not(test), no_mangle)]
#[inline(never)]
#[allow(non_snake_case, unused_variables)]
#[must_use]
pub extern "system" fn MsiRecordClearData(hRecord: MsiHandle) -> Uint {
    let result = panic::catch_unwind(|| {
        with_handle_mut(hRecord, |obj| {
            if let MsiObject::Record(rec_handle) = obj {
                rec_handle.inner = Record::new();
                ERROR_SUCCESS
            } else {
                ERROR_INVALID_HANDLE
            }
        })
        .unwrap_or(ERROR_INVALID_HANDLE)
    });

    result.unwrap_or(ERROR_INSTALL_FAILURE)
}

/// Returns the error record that was last returned for the calling thread.
///
/// # Returns
///
/// The handle to the error record, or `MSI_NULL_HANDLE` if no error record exists.
#[cfg_attr(not(test), no_mangle)]
#[inline(never)]
#[allow(non_snake_case)]
#[must_use]
pub extern "system" fn MsiGetLastErrorRecord() -> MsiHandle {
    let result = panic::catch_unwind(|| {
        // TODO: Map to thread-local MSI error records, currently we only expose FFI text errors.
        MSI_NULL_HANDLE
    });

    result.unwrap_or(MSI_NULL_HANDLE)
}

#[cfg(test)]
#[allow(clippy::too_many_lines, clippy::cognitive_complexity)]
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

        assert_eq!(MsiGetLastErrorRecord(), MSI_NULL_HANDLE);
    }
}

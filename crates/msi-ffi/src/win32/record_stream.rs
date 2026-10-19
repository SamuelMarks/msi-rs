//! Record streaming and field inspection Win32 endpoints.
//!
//! Provides `MsiRecordGetFieldCount`, `MsiRecordIsNull`, `MsiRecordReadStream`,
//! `MsiRecordSetStreamW`, and `MsiRecordSetStreamA`.

use crate::handles::{with_handle, with_handle_mut, MsiHandle, MsiObject};
use crate::win32::strings::{lpcstr_to_string, lpcwstr_to_string};
use crate::win32::{
    Dword, Lpcstr, Lpcwstr, Uint, ERROR_INSTALL_FAILURE, ERROR_INVALID_HANDLE,
    ERROR_INVALID_PARAMETER, ERROR_SUCCESS,
};
use msi::database::tables::record::FieldValue;

/// Gets the number of fields in a record (`cParams`, excluding field `0`).
///
/// # Arguments
///
/// * `hRecord` - Handle to the record.
///
/// # Returns
///
/// Number of data fields in the record, or `u32::MAX` (`-1`) if the handle is invalid.
#[cfg_attr(not(test), no_mangle)]
#[inline(never)]
#[allow(non_snake_case)]
#[must_use]
pub extern "system" fn MsiRecordGetFieldCount(hRecord: MsiHandle) -> Uint {
    let result = std::panic::catch_unwind(|| {
        with_handle(hRecord, |obj| {
            if let MsiObject::Record(rec_handle) = obj {
                Uint::try_from(rec_handle.inner.len().saturating_sub(1)).unwrap_or(0)
            } else {
                u32::MAX
            }
        })
        .unwrap_or(u32::MAX)
    });

    result.unwrap_or(u32::MAX)
}

/// Checks whether a field in a record is null or out of bounds.
///
/// # Arguments
///
/// * `hRecord` - Handle to the record.
/// * `iField` - 1-based field index (`0` is the format string field).
///
/// # Returns
///
/// `1` (`TRUE`) if the field is null or out of bounds, or `0` (`FALSE`) if the field is non-null or `hRecord` is invalid.
#[cfg_attr(not(test), no_mangle)]
#[inline(never)]
#[allow(non_snake_case)]
#[must_use]
pub extern "system" fn MsiRecordIsNull(hRecord: MsiHandle, iField: Uint) -> std::ffi::c_int {
    let result = std::panic::catch_unwind(|| {
        with_handle(hRecord, |obj| {
            if let MsiObject::Record(rec_handle) = obj {
                let is_null = rec_handle
                    .inner
                    .get(iField as usize)
                    .is_none_or(FieldValue::is_null);
                std::ffi::c_int::from(is_null)
            } else {
                0 // FALSE
            }
        })
        .unwrap_or(0)
    });

    result.unwrap_or(0)
}

/// Reads bytes from a record binary stream field and advances its internal cursor.
///
/// If `szDataBuf` is `NULL`, sets `*pcbDataBuf` to the remaining unread byte count in the stream
/// without advancing the read cursor, matching Wine's `MSI_RecordReadStream`.
///
/// # Arguments
///
/// * `hRecord` - Handle to the record.
/// * `iField` - 1-based index of the stream field.
/// * `szDataBuf` - Destination buffer to receive stream bytes, or `NULL` to query remaining byte count.
/// * `pcbDataBuf` - In/out pointer specifying buffer capacity on entry and bytes read/remaining on return.
///
/// # Returns
///
/// `ERROR_SUCCESS`, `ERROR_INVALID_PARAMETER`, `ERROR_INVALID_HANDLE`, `ERROR_INVALID_DATA` (`13`), or `ERROR_INVALID_DATATYPE` (`1804`).
#[cfg_attr(not(test), no_mangle)]
#[inline(never)]
#[allow(non_snake_case)]
#[allow(clippy::not_unsafe_ptr_arg_deref)]
pub extern "system" fn MsiRecordReadStream(
    hRecord: MsiHandle,
    iField: Uint,
    szDataBuf: *mut std::ffi::c_char,
    pcbDataBuf: *mut Dword,
) -> Uint {
    let result = std::panic::catch_unwind(|| {
        if pcbDataBuf.is_null() || iField == 0 {
            return ERROR_INVALID_PARAMETER;
        }

        with_handle_mut(hRecord, |obj| {
            let MsiObject::Record(rec_handle) = obj else {
                return ERROR_INVALID_HANDLE;
            };
            let idx = iField as usize;
            match rec_handle.inner.get(idx) {
                None | Some(FieldValue::Null) => {
                    unsafe {
                        *pcbDataBuf = 0;
                    }
                    ERROR_SUCCESS
                }
                Some(FieldValue::Stream(_)) => {
                    if szDataBuf.is_null() {
                        let remaining = rec_handle.inner.stream_remaining(idx).unwrap_or(0);
                        unsafe {
                            *pcbDataBuf = Dword::try_from(remaining).unwrap_or(0);
                        }
                        return ERROR_SUCCESS;
                    }
                    let requested = unsafe { *pcbDataBuf as usize };
                    if requested == 0 {
                        return ERROR_SUCCESS;
                    }
                    let out_slice = unsafe {
                        std::slice::from_raw_parts_mut(szDataBuf.cast::<u8>(), requested)
                    };
                    let read_count = rec_handle
                        .inner
                        .read_stream_bytes(idx, out_slice)
                        .unwrap_or(0);
                    unsafe {
                        *pcbDataBuf = Dword::try_from(read_count).unwrap_or(0);
                    }
                    ERROR_SUCCESS
                }
                Some(_) => 1804, // ERROR_INVALID_DATATYPE
            }
        })
        .unwrap_or(ERROR_INVALID_HANDLE)
    });

    result.unwrap_or(ERROR_INSTALL_FAILURE)
}

/// Shared implementation for loading a file into a record stream field or resetting its read cursor.
fn record_set_stream_path(h_record: MsiHandle, i_field: Uint, file_path: Option<String>) -> Uint {
    if i_field == 0 {
        return ERROR_INVALID_PARAMETER;
    }

    with_handle_mut(h_record, |obj| {
        let MsiObject::Record(rec_handle) = obj else {
            return ERROR_INVALID_HANDLE;
        };
        let idx = i_field as usize;
        if let Some(path) = file_path {
            while rec_handle.inner.len() <= idx {
                rec_handle.inner.push(FieldValue::Null);
            }
            let bytes = std::fs::read(&path).unwrap_or_default();
            rec_handle.inner.set_stream_bytes(idx, bytes);
            ERROR_SUCCESS
        } else if rec_handle.inner.reset_stream_cursor(idx) {
            ERROR_SUCCESS
        } else {
            ERROR_INVALID_PARAMETER
        }
    })
    .unwrap_or(ERROR_INVALID_HANDLE)
}

/// Sets a record stream field from a file on disk, or resets the stream read cursor if `szFilePath` is `NULL` (Unicode).
///
/// # Arguments
///
/// * `hRecord` - Handle to the record.
/// * `iField` - 1-based field index (`0` is invalid for streams).
/// * `szFilePath` - Path to the file to load into the stream, or `NULL` to reset the stream cursor to `0`.
///
/// # Returns
///
/// `ERROR_SUCCESS`, `ERROR_INVALID_PARAMETER`, or `ERROR_INVALID_HANDLE`.
#[cfg_attr(not(test), no_mangle)]
#[inline(never)]
#[allow(non_snake_case)]
#[must_use]
pub extern "system" fn MsiRecordSetStreamW(
    hRecord: MsiHandle,
    iField: Uint,
    szFilePath: Lpcwstr,
) -> Uint {
    let result = std::panic::catch_unwind(|| {
        if szFilePath.is_null() {
            return record_set_stream_path(hRecord, iField, None);
        }
        let Some(path) = lpcwstr_to_string(szFilePath) else {
            return ERROR_INVALID_PARAMETER;
        };
        record_set_stream_path(hRecord, iField, Some(path))
    });

    result.unwrap_or(ERROR_INSTALL_FAILURE)
}

/// Sets a record stream field from a file on disk, or resets the stream read cursor if `szFilePath` is `NULL` (ANSI).
///
/// # Arguments
///
/// * `hRecord` - Handle to the record.
/// * `iField` - 1-based field index (`0` is invalid for streams).
/// * `szFilePath` - Path to the file to load into the stream, or `NULL` to reset the stream cursor to `0`.
///
/// # Returns
///
/// `ERROR_SUCCESS`, `ERROR_INVALID_PARAMETER`, or `ERROR_INVALID_HANDLE`.
#[cfg_attr(not(test), no_mangle)]
#[inline(never)]
#[allow(non_snake_case)]
#[must_use]
pub extern "system" fn MsiRecordSetStreamA(
    hRecord: MsiHandle,
    iField: Uint,
    szFilePath: Lpcstr,
) -> Uint {
    let result = std::panic::catch_unwind(|| {
        if szFilePath.is_null() {
            return record_set_stream_path(hRecord, iField, None);
        }
        let Some(path) = lpcstr_to_string(szFilePath) else {
            return ERROR_INVALID_PARAMETER;
        };
        record_set_stream_path(hRecord, iField, Some(path))
    });

    result.unwrap_or(ERROR_INSTALL_FAILURE)
}

#[cfg(test)]
mod more_record_tests {
    use super::*;
    use crate::win32::record::{MsiCreateRecord, MsiRecordSetInteger};

    #[test]
    fn test_record_stream_validation() {
        let h = MsiCreateRecord(2);
        let mut pcb = 0;

        assert_eq!(MsiRecordGetFieldCount(0), u32::MAX);
        assert_eq!(MsiRecordGetFieldCount(h), 2);

        assert_eq!(MsiRecordIsNull(0, 1), 0);
        assert_eq!(MsiRecordIsNull(h, 1), 1);
        assert_eq!(MsiRecordSetInteger(h, 2, 77), ERROR_SUCCESS);
        assert_eq!(MsiRecordIsNull(h, 2), 0);

        assert_eq!(
            MsiRecordReadStream(0, 1, std::ptr::null_mut(), std::ptr::null_mut()),
            ERROR_INVALID_PARAMETER
        );
        assert_eq!(
            MsiRecordReadStream(h, 0, std::ptr::null_mut(), &raw mut pcb),
            ERROR_INVALID_PARAMETER
        );
        assert_eq!(
            MsiRecordReadStream(0, 1, std::ptr::null_mut(), &raw mut pcb),
            ERROR_INVALID_HANDLE
        );
        assert_eq!(
            MsiRecordReadStream(h, 1, std::ptr::null_mut(), &raw mut pcb),
            ERROR_SUCCESS
        );
        assert_eq!(pcb, 0);

        // Reading stream on an integer field returns ERROR_INVALID_DATATYPE (1804)
        assert_eq!(
            MsiRecordReadStream(h, 2, std::ptr::null_mut(), &raw mut pcb),
            1804
        );

        assert_eq!(
            MsiRecordSetStreamW(0, 1, std::ptr::null()),
            ERROR_INVALID_HANDLE
        );
        assert_eq!(
            MsiRecordSetStreamA(0, 1, std::ptr::null()),
            ERROR_INVALID_HANDLE
        );
        assert_eq!(
            MsiRecordSetStreamW(h, 0, std::ptr::null()),
            ERROR_INVALID_PARAMETER
        );
        // Resetting cursor when field 1 is not yet a stream returns ERROR_INVALID_PARAMETER
        assert_eq!(
            MsiRecordSetStreamW(h, 1, std::ptr::null()),
            ERROR_INVALID_PARAMETER
        );
        assert_eq!(
            MsiRecordSetStreamA(h, 1, std::ptr::null()),
            ERROR_INVALID_PARAMETER
        );

        let invalid_utf16 = [0xD800_u16, 0x0000];
        let invalid_utf8 = [0xFF_u8, 0x00];
        assert_eq!(
            MsiRecordSetStreamW(h, 1, invalid_utf16.as_ptr()),
            ERROR_INVALID_PARAMETER
        );
        assert_eq!(
            MsiRecordSetStreamA(h, 1, invalid_utf8.as_ptr().cast()),
            ERROR_INVALID_PARAMETER
        );
        let _ = crate::handles::close_handle(h);
    }

    #[test]
    fn test_record_stream_io() {
        let h = MsiCreateRecord(2);
        let mut pcb = 0;

        // Write a real temp file and stream it through MsiRecordSetStreamW & MsiRecordReadStream
        let tmp_path = std::env::temp_dir().join("test_msi_record_stream_payload.bin");
        let _ = std::fs::write(&tmp_path, b"ABCDEF");
        let path_str = tmp_path.to_string_lossy().into_owned();
        let valid_w: Vec<u16> = path_str.encode_utf16().chain(std::iter::once(0)).collect();
        let valid_a = std::ffi::CString::new(path_str).unwrap_or_default();

        assert_eq!(
            MsiRecordSetStreamW(0, 1, valid_w.as_ptr()),
            ERROR_INVALID_HANDLE
        );
        assert_eq!(
            MsiRecordSetStreamA(0, 1, valid_a.as_ptr()),
            ERROR_INVALID_HANDLE
        );

        assert_eq!(MsiRecordSetStreamW(h, 1, valid_w.as_ptr()), ERROR_SUCCESS);
        assert_eq!(MsiRecordIsNull(h, 1), 0);

        // Probe remaining length with null buffer
        assert_eq!(
            MsiRecordReadStream(h, 1, std::ptr::null_mut(), &raw mut pcb),
            ERROR_SUCCESS
        );
        assert_eq!(pcb, 6);

        // Read first 4 bytes
        let mut buf = [0i8; 4];
        pcb = 4;
        assert_eq!(
            MsiRecordReadStream(h, 1, buf.as_mut_ptr(), &raw mut pcb),
            ERROR_SUCCESS
        );
        assert_eq!(pcb, 4);
        assert_eq!(
            buf,
            [
                i8::from_ne_bytes([b'A']),
                i8::from_ne_bytes([b'B']),
                i8::from_ne_bytes([b'C']),
                i8::from_ne_bytes([b'D']),
            ]
        );

        // Probe remaining length (should be 2)
        assert_eq!(
            MsiRecordReadStream(h, 1, std::ptr::null_mut(), &raw mut pcb),
            ERROR_SUCCESS
        );
        assert_eq!(pcb, 2);

        // Read with pcb == 0
        pcb = 0;
        assert_eq!(
            MsiRecordReadStream(h, 1, buf.as_mut_ptr(), &raw mut pcb),
            ERROR_SUCCESS
        );
        assert_eq!(pcb, 0);

        // Reset stream cursor via NULL szFilePath
        assert_eq!(MsiRecordSetStreamW(h, 1, std::ptr::null()), ERROR_SUCCESS);
        assert_eq!(
            MsiRecordReadStream(h, 1, std::ptr::null_mut(), &raw mut pcb),
            ERROR_SUCCESS
        );
        assert_eq!(pcb, 6);

        // Also test MsiRecordSetStreamA on field 3 (extending record fields dynamically)
        assert_eq!(MsiRecordSetStreamA(h, 3, valid_a.as_ptr()), ERROR_SUCCESS);
        assert_eq!(MsiRecordSetStreamA(h, 3, std::ptr::null()), ERROR_SUCCESS);
        let _ = std::fs::remove_file(&tmp_path);
        let _ = crate::handles::close_handle(h);
    }

    #[test]
    fn test_record_stream_invalid_handles() {
        let mut pcb = 0;
        let valid_w: Vec<u16> = "path".encode_utf16().chain(std::iter::once(0)).collect();
        let valid_a = b"path\0";
        let mock_db = msi::wix::linker::LinkedDatabase::default();
        let db_handle =
            crate::handles::alloc_handle(MsiObject::Database(crate::types::MsiDatabaseHandle {
                inner: mock_db,
                state: 0,
            }));
        assert_eq!(MsiRecordGetFieldCount(db_handle), u32::MAX);
        assert_eq!(MsiRecordIsNull(db_handle, 1), crate::win32::FALSE);
        assert_eq!(
            MsiRecordReadStream(db_handle, 1, std::ptr::null_mut(), std::ptr::null_mut()),
            ERROR_INVALID_PARAMETER
        );
        assert_eq!(
            MsiRecordReadStream(db_handle, 1, std::ptr::null_mut(), &raw mut pcb),
            ERROR_INVALID_HANDLE
        );
        assert_eq!(
            MsiRecordSetStreamW(db_handle, 1, valid_w.as_ptr()),
            ERROR_INVALID_HANDLE
        );
        assert_eq!(
            MsiRecordSetStreamA(db_handle, 1, valid_a.as_ptr().cast()),
            ERROR_INVALID_HANDLE
        );

        let _ = crate::handles::close_handle(db_handle);
    }
}

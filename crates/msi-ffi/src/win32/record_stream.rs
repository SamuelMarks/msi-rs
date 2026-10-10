//! Record streaming Win32 endpoints.

use crate::handles::{with_handle, MsiHandle, MsiObject};
use crate::win32::{
    Dword, Lpcstr, Lpcwstr, Uint, ERROR_INSTALL_FAILURE, ERROR_INVALID_HANDLE, ERROR_SUCCESS,
};

/// Gets the number of fields in a record.
#[cfg_attr(not(coverage_nightly), no_mangle)]
#[inline(never)]
#[allow(non_snake_case, unused_variables)]
pub extern "system" fn MsiRecordGetFieldCount(hRecord: MsiHandle) -> Uint {
    let result = std::panic::catch_unwind(|| {
        with_handle(hRecord, |obj| {
            if let MsiObject::Record(_rec_handle) = obj {
                0 // 0 fields in mock record
            } else {
                u32::MAX
            }
        })
        .unwrap_or(u32::MAX)
    });

    result.unwrap_or(u32::MAX)
}

/// Checks if a field in a record is null.
#[cfg_attr(not(coverage_nightly), no_mangle)]
#[inline(never)]
#[allow(non_snake_case, unused_variables)]
pub extern "system" fn MsiRecordIsNull(hRecord: MsiHandle, iField: Uint) -> std::ffi::c_int {
    let result = std::panic::catch_unwind(|| {
        with_handle(hRecord, |obj| {
            if let MsiObject::Record(_rec_handle) = obj {
                1 // TRUE
            } else {
                0 // FALSE
            }
        })
        .unwrap_or(0)
    });

    result.unwrap_or(0)
}

/// Reads a stream from a record.
#[cfg_attr(not(coverage_nightly), no_mangle)]
#[inline(never)]
#[allow(non_snake_case, unused_variables)]
#[allow(clippy::not_unsafe_ptr_arg_deref)]
pub extern "system" fn MsiRecordReadStream(
    hRecord: MsiHandle,
    iField: Uint,
    szDataBuf: *mut std::ffi::c_char,
    pcbDataBuf: *mut Dword,
) -> Uint {
    let result = std::panic::catch_unwind(|| {
        if pcbDataBuf.is_null() {
            return crate::win32::ERROR_INVALID_PARAMETER;
        }

        with_handle(hRecord, |obj| {
            if let MsiObject::Record(_rec_handle) = obj {
                unsafe {
                    *pcbDataBuf = 0;
                } // 0 bytes read
                ERROR_SUCCESS
            } else {
                ERROR_INVALID_HANDLE
            }
        })
        .unwrap_or(ERROR_INVALID_HANDLE)
    });

    result.unwrap_or(ERROR_INSTALL_FAILURE)
}

/// Sets a stream in a record (Unicode).
#[cfg_attr(not(coverage_nightly), no_mangle)]
#[inline(never)]
#[allow(non_snake_case, unused_variables)]
pub extern "system" fn MsiRecordSetStreamW(
    hRecord: MsiHandle,
    iField: Uint,
    szFilePath: Lpcwstr,
) -> Uint {
    let result = std::panic::catch_unwind(|| {
        if szFilePath.is_null() {
            return crate::win32::ERROR_INVALID_PARAMETER;
        }

        with_handle(hRecord, |obj| {
            if let MsiObject::Record(_rec_handle) = obj {
                ERROR_SUCCESS
            } else {
                ERROR_INVALID_HANDLE
            }
        })
        .unwrap_or(ERROR_INVALID_HANDLE)
    });

    result.unwrap_or(ERROR_INSTALL_FAILURE)
}

/// Sets a stream in a record (ANSI).
#[cfg_attr(not(coverage_nightly), no_mangle)]
#[inline(never)]
#[allow(non_snake_case, unused_variables)]
pub extern "system" fn MsiRecordSetStreamA(
    hRecord: MsiHandle,
    iField: Uint,
    szFilePath: Lpcstr,
) -> Uint {
    let result = std::panic::catch_unwind(|| {
        if szFilePath.is_null() {
            return crate::win32::ERROR_INVALID_PARAMETER;
        }

        with_handle(hRecord, |obj| {
            if let MsiObject::Record(_rec_handle) = obj {
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
mod more_record_tests {
    use super::*;
    use crate::win32::record::MsiCreateRecord;

    #[test]
    fn test_record_stream_stubs() {
        let h = MsiCreateRecord(1);
        let mut pcb = 0;

        assert_eq!(MsiRecordGetFieldCount(0), u32::MAX);
        assert_eq!(MsiRecordGetFieldCount(h), 0);

        assert_eq!(MsiRecordIsNull(0, 1), 0);
        assert_eq!(MsiRecordIsNull(h, 1), 1);

        assert_eq!(
            MsiRecordReadStream(0, 1, std::ptr::null_mut(), std::ptr::null_mut()),
            crate::win32::ERROR_INVALID_PARAMETER
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

        assert_eq!(
            MsiRecordSetStreamW(0, 1, std::ptr::null()),
            crate::win32::ERROR_INVALID_PARAMETER
        );
        assert_eq!(
            MsiRecordSetStreamA(0, 1, std::ptr::null()),
            crate::win32::ERROR_INVALID_PARAMETER
        );

        let valid_w: Vec<u16> = "path".encode_utf16().chain(std::iter::once(0)).collect();
        let valid_a = b"path\0";

        assert_eq!(
            MsiRecordSetStreamW(0, 1, valid_w.as_ptr()),
            ERROR_INVALID_HANDLE
        );
        assert_eq!(
            MsiRecordSetStreamA(0, 1, valid_a.as_ptr().cast::<i8>()),
            ERROR_INVALID_HANDLE
        );

        assert_eq!(MsiRecordSetStreamW(h, 1, valid_w.as_ptr()), ERROR_SUCCESS);
        assert_eq!(
            MsiRecordSetStreamA(h, 1, valid_a.as_ptr().cast::<i8>()),
            ERROR_SUCCESS
        );

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
            crate::win32::ERROR_INVALID_PARAMETER
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

        let _ = crate::handles::close_handle(h);
        let _ = crate::handles::close_handle(db_handle);
    }
}

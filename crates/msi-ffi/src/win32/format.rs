//! Format Record Win32 endpoints.

use crate::handles::{with_handle, MsiHandle, MsiObject};
use crate::win32::strings::{string_to_lpstr, string_to_lpwstr};
use crate::win32::{Dword, Lpstr, Lpwstr, Uint, ERROR_INSTALL_FAILURE, ERROR_INVALID_HANDLE};

/// Formats a record using a format string (Unicode).
#[cfg_attr(not(coverage_nightly), no_mangle)]
#[inline(never)]
#[allow(non_snake_case, unused_variables)]
pub extern "system" fn MsiFormatRecordW(
    hInstall: MsiHandle,
    hRecord: MsiHandle,
    szResultBuf: Lpwstr,
    pcchResultBuf: *mut Dword,
) -> Uint {
    let result = std::panic::catch_unwind(|| {
        let ctx = msi::execution::FormatContext::default();

        let formatted_res = with_handle(hRecord, |obj| {
            if let MsiObject::Record(rec_handle) = obj {
                let rec = &rec_handle.inner;
                let template = match rec.get(0) {
                    Some(msi::database::tables::FieldValue::String(s)) => s.clone(),
                    _ => String::new(),
                };
                Ok(msi::execution::format_record(&template, rec, &ctx).unwrap_or_default())
            } else {
                Err(ERROR_INVALID_HANDLE)
            }
        });

        let formatted = match formatted_res {
            Some(Ok(f)) => f,
            _ => return ERROR_INVALID_HANDLE,
        };

        string_to_lpwstr(&formatted, szResultBuf, pcchResultBuf)
    });

    result.unwrap_or(ERROR_INSTALL_FAILURE)
}

/// Formats a record using a format string (ANSI).
#[cfg_attr(not(coverage_nightly), no_mangle)]
#[inline(never)]
#[allow(non_snake_case, unused_variables)]
pub extern "system" fn MsiFormatRecordA(
    hInstall: MsiHandle,
    hRecord: MsiHandle,
    szResultBuf: Lpstr,
    pcchResultBuf: *mut Dword,
) -> Uint {
    let result = std::panic::catch_unwind(|| {
        let ctx = msi::execution::FormatContext::default();

        let formatted_res = with_handle(hRecord, |obj| {
            if let MsiObject::Record(rec_handle) = obj {
                let rec = &rec_handle.inner;
                let template = match rec.get(0) {
                    Some(msi::database::tables::FieldValue::String(s)) => s.clone(),
                    _ => String::new(),
                };
                Ok(msi::execution::format_record(&template, rec, &ctx).unwrap_or_default())
            } else {
                Err(ERROR_INVALID_HANDLE)
            }
        });

        let formatted = match formatted_res {
            Some(Ok(f)) => f,
            _ => return ERROR_INVALID_HANDLE,
        };

        string_to_lpstr(&formatted, szResultBuf, pcchResultBuf)
    });

    result.unwrap_or(ERROR_INSTALL_FAILURE)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::win32::record::MsiCreateRecord;

    #[test]
    fn test_format_record_stubs() {
        let h = MsiCreateRecord(1);
        let mut pcch = 0;

        assert_eq!(
            MsiFormatRecordW(0, 0, std::ptr::null_mut(), std::ptr::null_mut()),
            ERROR_INVALID_HANDLE
        );
        assert_eq!(
            MsiFormatRecordA(0, 0, std::ptr::null_mut(), std::ptr::null_mut()),
            ERROR_INVALID_HANDLE
        );

        assert_eq!(
            MsiFormatRecordW(0, h, std::ptr::null_mut(), &raw mut pcch),
            crate::win32::ERROR_MORE_DATA
        );
        assert_eq!(
            MsiFormatRecordA(0, h, std::ptr::null_mut(), &raw mut pcch),
            crate::win32::ERROR_MORE_DATA
        );

        let db = msi::wix::linker::LinkedDatabase::default();
        let db_handle =
            crate::handles::alloc_handle(MsiObject::Database(crate::types::MsiDatabaseHandle {
                inner: db,
                state: 0,
            }));

        assert_eq!(
            MsiFormatRecordW(0, db_handle, std::ptr::null_mut(), std::ptr::null_mut()),
            ERROR_INVALID_HANDLE
        );
        assert_eq!(
            MsiFormatRecordA(0, db_handle, std::ptr::null_mut(), std::ptr::null_mut()),
            ERROR_INVALID_HANDLE
        );

        let _ = crate::handles::close_handle(h);
        let _ = crate::handles::close_handle(db_handle);
    }
}
#[cfg(test)]
mod extra_tests {
    use super::*;

    #[test]
    fn test_format_with_string() {
        let mut rec = msi::database::tables::record::Record::default();
        rec.push(msi::database::tables::FieldValue::String(
            "test".to_string(),
        ));
        let h = crate::handles::alloc_handle(MsiObject::Record(crate::types::MsiRecordHandle {
            inner: rec,
        }));

        let mut pcch = 0;
        assert_eq!(
            MsiFormatRecordA(0, h, std::ptr::null_mut(), &raw mut pcch),
            crate::win32::ERROR_MORE_DATA
        );
        assert_eq!(pcch, 4);
        assert_eq!(
            MsiFormatRecordW(0, h, std::ptr::null_mut(), &raw mut pcch),
            crate::win32::ERROR_MORE_DATA
        );
        let _ = crate::handles::close_handle(h);
    }
}

//! Format Record Win32 endpoints.

use crate::handles::{with_handle, MsiHandle, MsiObject};
use crate::win32::strings::{string_to_lpstr, string_to_lpwstr};
use crate::win32::{Dword, Lpstr, Lpwstr, Uint, ERROR_INSTALL_FAILURE, ERROR_INVALID_HANDLE};

/// Builds a [`msi::execution::FormatContext`] from `h_install` if it refers to an active database/session handle.
fn build_format_context(h_install: MsiHandle) -> msi::execution::FormatContext {
    let mut ctx = msi::execution::FormatContext::default();
    if h_install != 0 {
        with_handle(h_install, |obj| {
            if let MsiObject::Database(db) = obj {
                for rec in db.inner.get_records("Property") {
                    if let (
                        Some(msi::database::FieldValue::String(k)),
                        Some(msi::database::FieldValue::String(v)),
                    ) = (rec.get(0), rec.get(1))
                    {
                        ctx.properties.insert(k.clone(), v.clone());
                    }
                }
            }
        });
    }
    ctx
}

/// Formats a record string using `h_install` context and `h_record` template/fields.
fn format_record_inner(h_install: MsiHandle, h_record: MsiHandle) -> Result<String, Uint> {
    let ctx = build_format_context(h_install);
    let formatted_res = with_handle(h_record, |obj| {
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
    formatted_res.unwrap_or(Err(ERROR_INVALID_HANDLE))
}

/// Formats a record using a format string and session properties (Unicode).
///
/// # Arguments
///
/// * `hInstall` - Optional handle to the installation session supplying property values (`0` if none).
/// * `hRecord` - Handle to the record to format (field `0` holds the template string).
/// * `szResultBuf` - Destination UTF-16 buffer, or `NULL` to query length.
/// * `pcchResultBuf` - In/out buffer length in characters.
///
/// # Returns
///
/// `ERROR_SUCCESS`, `ERROR_MORE_DATA`, `ERROR_INVALID_PARAMETER`, or `ERROR_INVALID_HANDLE`.
#[cfg_attr(not(test), no_mangle)]
#[inline(never)]
#[allow(non_snake_case)]
pub extern "system" fn MsiFormatRecordW(
    hInstall: MsiHandle,
    hRecord: MsiHandle,
    szResultBuf: Lpwstr,
    pcchResultBuf: *mut Dword,
) -> Uint {
    std::panic::catch_unwind(|| match format_record_inner(hInstall, hRecord) {
        Ok(formatted) => string_to_lpwstr(&formatted, szResultBuf, pcchResultBuf),
        Err(code) => code,
    })
    .unwrap_or(ERROR_INSTALL_FAILURE)
}

/// Formats a record using a format string and session properties (ANSI).
///
/// # Arguments
///
/// * `hInstall` - Optional handle to the installation session supplying property values (`0` if none).
/// * `hRecord` - Handle to the record to format (field `0` holds the template string).
/// * `szResultBuf` - Destination ANSI buffer, or `NULL` to query length.
/// * `pcchResultBuf` - In/out buffer length in characters.
///
/// # Returns
///
/// `ERROR_SUCCESS`, `ERROR_MORE_DATA`, `ERROR_INVALID_PARAMETER`, or `ERROR_INVALID_HANDLE`.
#[cfg_attr(not(test), no_mangle)]
#[inline(never)]
#[allow(non_snake_case)]
pub extern "system" fn MsiFormatRecordA(
    hInstall: MsiHandle,
    hRecord: MsiHandle,
    szResultBuf: Lpstr,
    pcchResultBuf: *mut Dword,
) -> Uint {
    std::panic::catch_unwind(|| match format_record_inner(hInstall, hRecord) {
        Ok(formatted) => string_to_lpstr(&formatted, szResultBuf, pcchResultBuf),
        Err(code) => code,
    })
    .unwrap_or(ERROR_INSTALL_FAILURE)
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

        // Test property resolution via hInstall
        let mut db = msi::wix::linker::LinkedDatabase::default();
        db.set_property_value("ProductName", "AwesomeApp");
        let h_install =
            crate::handles::alloc_handle(MsiObject::Database(crate::types::MsiDatabaseHandle {
                inner: db,
                state: 0,
            }));
        let mut prop_rec = msi::database::tables::record::Record::default();
        prop_rec.push(msi::database::tables::FieldValue::String(
            "Installing [ProductName]".to_string(),
        ));
        let h_prop_rec =
            crate::handles::alloc_handle(MsiObject::Record(crate::types::MsiRecordHandle {
                inner: prop_rec,
            }));
        let mut out_a = [0i8; 64];
        let mut len_a: Dword = 64;
        assert_eq!(
            MsiFormatRecordA(h_install, h_prop_rec, out_a.as_mut_ptr(), &raw mut len_a),
            crate::win32::ERROR_SUCCESS
        );
        assert_eq!(len_a, 21);

        let _ = crate::handles::close_handle(h_prop_rec);
        let _ = crate::handles::close_handle(h_install);
        let _ = crate::handles::close_handle(h);
    }
}

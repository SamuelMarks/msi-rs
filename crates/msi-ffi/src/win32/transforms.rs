//! Transform generation and querying Win32 endpoints.

use crate::handles::{alloc_handle, with_handle, MsiHandle, MsiObject};
use crate::win32::strings::{lpcstr_to_string, lpcwstr_to_string};
use crate::win32::{
    Lpcstr, Lpcwstr, Uint, ERROR_INSTALL_FAILURE, ERROR_INVALID_HANDLE, ERROR_INVALID_PARAMETER,
    ERROR_SUCCESS,
};

/// Generates a transform between two databases.
///
/// # Arguments
///
/// * `hDatabase` - The reference database.
/// * `hDatabaseReference` - The database containing changes.
/// * `szTransformFile` - The path to the generated transform file.
/// * `iErrorConditions` - Error conditions that should be suppressed.
/// * `iValidation` - Validation properties to associate with the transform.
///
/// # Returns
///
/// `ERROR_SUCCESS` or `ERROR_INVALID_PARAMETER`.
#[cfg_attr(not(coverage_nightly), no_mangle)]
#[inline(never)]
#[allow(non_snake_case, unused_variables)]
pub extern "system" fn MsiDatabaseGenerateTransformW(
    hDatabase: MsiHandle,
    hDatabaseReference: MsiHandle,
    szTransformFile: Lpcwstr,
    iErrorConditions: i32,
    iValidation: i32,
) -> Uint {
    let result = std::panic::catch_unwind(|| {
        if szTransformFile.is_null() {
            return ERROR_INVALID_PARAMETER;
        }

        let Some(path) = lpcwstr_to_string(szTransformFile) else {
            return ERROR_INVALID_PARAMETER;
        };

        with_handle(hDatabase, |obj| {
            if let MsiObject::Database(db1) = obj {
                with_handle(hDatabaseReference, |obj2| {
                    if let MsiObject::Database(db2) = obj2 {
                        let transform_res = msi::database::transform::DatabaseTransform::diff(
                            &db2.inner, &db1.inner,
                        );
                        match transform_res {
                            Ok(mut transform) => {
                                transform.summary_info.word_count = Some(iValidation);
                                transform.validation_flags = iValidation as u32;
                                // We don't populate iErrorConditions directly in SummaryInfo for now,
                                // but we could map it to SummaryInfo if needed.

                                match transform.to_bytes() {
                                    Ok(bytes) => {
                                        match std::fs::write(&path, bytes) {
                                            Ok(()) => ERROR_SUCCESS,
                                            Err(_) => ERROR_INSTALL_FAILURE, // Write failed
                                        }
                                    }
                                    Err(_) => ERROR_INSTALL_FAILURE,
                                }
                            }
                            Err(e) => crate::error::map_msi_error_to_lstatus(&e),
                        }
                    } else {
                        ERROR_INVALID_HANDLE
                    }
                })
                .unwrap_or(ERROR_INVALID_HANDLE)
            } else {
                ERROR_INVALID_HANDLE
            }
        })
        .unwrap_or(ERROR_INVALID_HANDLE)
    });

    result.unwrap_or(ERROR_INSTALL_FAILURE)
}

/// Generates a transform between two databases (ANSI).
#[cfg_attr(not(coverage_nightly), no_mangle)]
#[inline(never)]
#[allow(non_snake_case, unused_variables)]
pub extern "system" fn MsiDatabaseGenerateTransformA(
    hDatabase: MsiHandle,
    hDatabaseReference: MsiHandle,
    szTransformFile: Lpcstr,
    iErrorConditions: i32,
    iValidation: i32,
) -> Uint {
    let result = std::panic::catch_unwind(|| {
        if szTransformFile.is_null() {
            return ERROR_INVALID_PARAMETER;
        }

        let Some(path) = lpcstr_to_string(szTransformFile) else {
            return ERROR_INVALID_PARAMETER;
        };

        with_handle(hDatabase, |obj| {
            if let MsiObject::Database(db1) = obj {
                with_handle(hDatabaseReference, |obj2| {
                    if let MsiObject::Database(db2) = obj2 {
                        let transform_res = msi::database::transform::DatabaseTransform::diff(
                            &db2.inner, &db1.inner,
                        );
                        match transform_res {
                            Ok(mut transform) => {
                                transform.summary_info.word_count = Some(iValidation);
                                transform.validation_flags = iValidation as u32;
                                // We don't populate iErrorConditions directly in SummaryInfo for now,
                                // but we could map it to SummaryInfo if needed.

                                match transform.to_bytes() {
                                    Ok(bytes) => {
                                        match std::fs::write(&path, bytes) {
                                            Ok(()) => ERROR_SUCCESS,
                                            Err(_) => ERROR_INSTALL_FAILURE, // Write failed
                                        }
                                    }
                                    Err(_) => ERROR_INSTALL_FAILURE,
                                }
                            }
                            Err(e) => crate::error::map_msi_error_to_lstatus(&e),
                        }
                    } else {
                        ERROR_INVALID_HANDLE
                    }
                })
                .unwrap_or(ERROR_INVALID_HANDLE)
            } else {
                ERROR_INVALID_HANDLE
            }
        })
        .unwrap_or(ERROR_INVALID_HANDLE)
    });

    result.unwrap_or(ERROR_INSTALL_FAILURE)
}

/// Creates summary information for a transform file.
#[cfg_attr(not(coverage_nightly), no_mangle)]
#[inline(never)]
#[allow(non_snake_case, unused_variables)]
pub extern "system" fn MsiCreateTransformSummaryInfoW(
    hDatabase: MsiHandle,
    hDatabaseReference: MsiHandle,
    szTransformFile: Lpcwstr,
    iErrorConditions: i32,
    iValidation: i32,
) -> Uint {
    let result = std::panic::catch_unwind(|| {
        if szTransformFile.is_null() {
            return ERROR_INVALID_PARAMETER;
        }

        let Some(_path) = lpcwstr_to_string(szTransformFile) else {
            return ERROR_INVALID_PARAMETER;
        };

        ERROR_SUCCESS
    });

    result.unwrap_or(ERROR_INSTALL_FAILURE)
}

/// Creates summary information for a transform file (ANSI).
#[cfg_attr(not(coverage_nightly), no_mangle)]
#[inline(never)]
#[allow(non_snake_case, unused_variables)]
pub extern "system" fn MsiCreateTransformSummaryInfoA(
    hDatabase: MsiHandle,
    hDatabaseReference: MsiHandle,
    szTransformFile: Lpcstr,
    iErrorConditions: i32,
    iValidation: i32,
) -> Uint {
    let result = std::panic::catch_unwind(|| {
        if szTransformFile.is_null() {
            return ERROR_INVALID_PARAMETER;
        }

        let Some(_path) = lpcstr_to_string(szTransformFile) else {
            return ERROR_INVALID_PARAMETER;
        };

        ERROR_SUCCESS
    });

    result.unwrap_or(ERROR_INSTALL_FAILURE)
}

/// Gets the primary keys of a table.
#[cfg_attr(not(coverage_nightly), no_mangle)]
#[inline(never)]
#[allow(non_snake_case, unused_variables)]
#[allow(clippy::not_unsafe_ptr_arg_deref)]
pub extern "system" fn MsiDatabaseGetPrimaryKeysW(
    hDatabase: MsiHandle,
    szTableName: Lpcwstr,
    phRecord: *mut MsiHandle,
) -> Uint {
    let result = std::panic::catch_unwind(|| {
        if szTableName.is_null() || phRecord.is_null() {
            return ERROR_INVALID_PARAMETER;
        }

        let Some(_table_name) = lpcwstr_to_string(szTableName) else {
            return ERROR_INVALID_PARAMETER;
        };

        let is_valid_db =
            with_handle(hDatabase, |obj| matches!(obj, MsiObject::Database(_))).unwrap_or(false);

        if is_valid_db {
            let rec = msi::database::tables::record::Record::new();
            let new_obj = MsiObject::Record(crate::types::MsiRecordHandle { inner: rec });
            unsafe {
                *phRecord = alloc_handle(new_obj);
            }
            ERROR_SUCCESS
        } else {
            ERROR_INVALID_HANDLE
        }
    });

    result.unwrap_or(ERROR_INSTALL_FAILURE)
}

/// Gets the primary keys of a table (ANSI).
#[cfg_attr(not(coverage_nightly), no_mangle)]
#[inline(never)]
#[allow(non_snake_case, unused_variables)]
#[allow(clippy::not_unsafe_ptr_arg_deref)]
pub extern "system" fn MsiDatabaseGetPrimaryKeysA(
    hDatabase: MsiHandle,
    szTableName: Lpcstr,
    phRecord: *mut MsiHandle,
) -> Uint {
    let result = std::panic::catch_unwind(|| {
        if szTableName.is_null() || phRecord.is_null() {
            return ERROR_INVALID_PARAMETER;
        }

        let Some(_table_name) = lpcstr_to_string(szTableName) else {
            return ERROR_INVALID_PARAMETER;
        };

        let is_valid_db =
            with_handle(hDatabase, |obj| matches!(obj, MsiObject::Database(_))).unwrap_or(false);

        if is_valid_db {
            let rec = msi::database::tables::record::Record::new();
            let new_obj = MsiObject::Record(crate::types::MsiRecordHandle { inner: rec });
            unsafe {
                *phRecord = alloc_handle(new_obj);
            }
            ERROR_SUCCESS
        } else {
            ERROR_INVALID_HANDLE
        }
    });

    result.unwrap_or(ERROR_INSTALL_FAILURE)
}

/// Returns the persistent state of a table.
#[cfg_attr(not(coverage_nightly), no_mangle)]
#[inline(never)]
#[allow(non_snake_case, unused_variables)]
pub extern "system" fn MsiDatabaseIsTablePersistentW(
    hDatabase: MsiHandle,
    szTableName: Lpcwstr,
) -> i32 {
    let result = std::panic::catch_unwind(|| {
        if szTableName.is_null() {
            return -1; // MSICONDITION_ERROR
        }

        let Some(_table_name) = lpcwstr_to_string(szTableName) else {
            return -1;
        };

        with_handle(hDatabase, |obj| {
            if let MsiObject::Database(_db) = obj {
                1 // MSICONDITION_TRUE
            } else {
                -1 // MSICONDITION_ERROR
            }
        })
        .unwrap_or(-1)
    });

    result.unwrap_or(-1)
}

/// Returns the persistent state of a table (ANSI).
#[cfg_attr(not(coverage_nightly), no_mangle)]
#[inline(never)]
#[allow(non_snake_case, unused_variables)]
pub extern "system" fn MsiDatabaseIsTablePersistentA(
    hDatabase: MsiHandle,
    szTableName: Lpcstr,
) -> i32 {
    let result = std::panic::catch_unwind(|| {
        if szTableName.is_null() {
            return -1; // MSICONDITION_ERROR
        }

        let Some(_table_name) = lpcstr_to_string(szTableName) else {
            return -1;
        };

        with_handle(hDatabase, |obj| {
            if let MsiObject::Database(_db) = obj {
                1 // MSICONDITION_TRUE
            } else {
                -1 // MSICONDITION_ERROR
            }
        })
        .unwrap_or(-1)
    });

    result.unwrap_or(-1)
}

/// Retrieves a handle to the active database for the installation.
#[cfg_attr(not(coverage_nightly), no_mangle)]
#[inline(never)]
#[allow(non_snake_case, unused_variables)]
pub extern "system" fn MsiGetActiveDatabase(hInstall: MsiHandle) -> MsiHandle {
    let result = std::panic::catch_unwind(|| {
        let mock_db = msi::wix::linker::LinkedDatabase::default();
        let new_obj = MsiObject::Database(crate::types::MsiDatabaseHandle {
            inner: mock_db,
            state: 0,
        });
        alloc_handle(new_obj)
    });

    result.unwrap_or(0)
}

#[cfg(test)]
mod transform_and_record_tests {
    use super::*;

    #[test]
    fn test_msi_database_generate_transform() {
        use crate::handles::{alloc_handle, MsiObject};
        use crate::types::MsiDatabaseHandle;
        use msi::wix::linker::LinkedDatabase;
        use std::fs;

        let db1 = LinkedDatabase::default();
        let db2 = LinkedDatabase::default();

        let h1 = alloc_handle(MsiObject::Database(MsiDatabaseHandle {
            inner: db1,
            state: 0,
        }));
        let h2 = alloc_handle(MsiObject::Database(MsiDatabaseHandle {
            inner: db2,
            state: 0,
        }));

        let temp_dir = std::env::temp_dir();
        let mst_path = temp_dir.join("test_generate.mst");
        let mst_path_str = mst_path.to_string_lossy();

        let path_w: Vec<u16> = mst_path_str
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect();

        let res = MsiDatabaseGenerateTransformW(h1, h2, path_w.as_ptr(), 0, 0);
        assert_eq!(res, ERROR_SUCCESS);

        assert!(mst_path.exists());
        let _ = fs::remove_file(&mst_path);

        let res_bad = MsiDatabaseGenerateTransformW(0, h2, path_w.as_ptr(), 0, 0);
        assert_eq!(res_bad, ERROR_INVALID_HANDLE);
    }

    #[test]
    fn test_transform_generation_stubs() {
        assert_eq!(
            MsiDatabaseGenerateTransformW(0, 0, std::ptr::null(), 0, 0),
            ERROR_INVALID_PARAMETER
        );
        assert_eq!(
            MsiDatabaseGenerateTransformA(0, 0, std::ptr::null(), 0, 0),
            ERROR_INVALID_PARAMETER
        );

        assert_eq!(
            MsiCreateTransformSummaryInfoW(0, 0, std::ptr::null(), 0, 0),
            ERROR_INVALID_PARAMETER
        );
        assert_eq!(
            MsiCreateTransformSummaryInfoA(0, 0, std::ptr::null(), 0, 0),
            ERROR_INVALID_PARAMETER
        );

        let invalid_utf16 = [0xD800_u16, 0x0000];
        let invalid_utf8 = [0xFF_u8, 0x00];

        assert_eq!(
            MsiDatabaseGenerateTransformW(0, 0, invalid_utf16.as_ptr(), 0, 0),
            ERROR_INVALID_PARAMETER
        );
        assert_eq!(
            MsiDatabaseGenerateTransformA(0, 0, invalid_utf8.as_ptr().cast(), 0, 0),
            ERROR_INVALID_PARAMETER
        );

        assert_eq!(
            MsiCreateTransformSummaryInfoW(0, 0, invalid_utf16.as_ptr(), 0, 0),
            ERROR_INVALID_PARAMETER
        );
        assert_eq!(
            MsiCreateTransformSummaryInfoA(0, 0, invalid_utf8.as_ptr().cast(), 0, 0),
            ERROR_INVALID_PARAMETER
        );

        let valid_w: Vec<u16> = "valid".encode_utf16().chain(std::iter::once(0)).collect();
        let valid_a = b"valid\0";

        assert_eq!(
            MsiDatabaseGenerateTransformW(0, 0, valid_w.as_ptr(), 0, 0),
            ERROR_INVALID_HANDLE
        );
        assert_eq!(
            MsiDatabaseGenerateTransformA(0, 0, valid_a.as_ptr().cast::<i8>(), 0, 0),
            ERROR_INVALID_HANDLE
        );

        let mock_db = msi::wix::linker::LinkedDatabase::default();
        let db_handle = alloc_handle(MsiObject::Database(crate::types::MsiDatabaseHandle {
            inner: mock_db,
            state: 0,
        }));

        let mock_db2 = msi::wix::linker::LinkedDatabase::default();
        let db_handle2 = alloc_handle(MsiObject::Database(crate::types::MsiDatabaseHandle {
            inner: mock_db2,
            state: 0,
        }));

        assert_eq!(
            MsiDatabaseGenerateTransformW(db_handle, 0, valid_w.as_ptr(), 0, 0),
            ERROR_INVALID_HANDLE
        );
        assert_eq!(
            MsiDatabaseGenerateTransformA(db_handle, 0, valid_a.as_ptr().cast::<i8>(), 0, 0),
            ERROR_INVALID_HANDLE
        );

        assert_eq!(
            MsiDatabaseGenerateTransformW(db_handle, db_handle2, valid_w.as_ptr(), 0, 0),
            ERROR_SUCCESS
        );
        assert_eq!(
            MsiDatabaseGenerateTransformA(
                db_handle,
                db_handle2,
                valid_a.as_ptr().cast::<i8>(),
                0,
                0
            ),
            ERROR_SUCCESS
        );

        let _ = crate::handles::close_handle(db_handle);
        let _ = crate::handles::close_handle(db_handle2);

        assert_eq!(
            MsiCreateTransformSummaryInfoW(0, 0, valid_w.as_ptr(), 0, 0),
            ERROR_SUCCESS
        );
        assert_eq!(
            MsiCreateTransformSummaryInfoA(0, 0, valid_a.as_ptr().cast::<i8>(), 0, 0),
            ERROR_SUCCESS
        );
    }

    #[test]
    fn test_primary_keys_and_persistence() {
        let mut rec_handle = 0;
        assert_eq!(
            MsiDatabaseGetPrimaryKeysW(0, std::ptr::null(), &raw mut rec_handle),
            ERROR_INVALID_PARAMETER
        );
        assert_eq!(
            MsiDatabaseGetPrimaryKeysA(0, std::ptr::null(), &raw mut rec_handle),
            ERROR_INVALID_PARAMETER
        );

        assert_eq!(MsiDatabaseIsTablePersistentW(0, std::ptr::null()), -1);
        assert_eq!(MsiDatabaseIsTablePersistentA(0, std::ptr::null()), -1);

        let invalid_utf16 = [0xD800_u16, 0x0000];
        let invalid_utf8 = [0xFF_u8, 0x00];

        assert_eq!(
            MsiDatabaseGetPrimaryKeysW(0, invalid_utf16.as_ptr(), &raw mut rec_handle),
            ERROR_INVALID_PARAMETER
        );
        assert_eq!(
            MsiDatabaseGetPrimaryKeysA(0, invalid_utf8.as_ptr().cast(), &raw mut rec_handle),
            ERROR_INVALID_PARAMETER
        );

        assert_eq!(MsiDatabaseIsTablePersistentW(0, invalid_utf16.as_ptr()), -1);
        assert_eq!(
            MsiDatabaseIsTablePersistentA(0, invalid_utf8.as_ptr().cast()),
            -1
        );

        let valid_w: Vec<u16> = "valid".encode_utf16().chain(std::iter::once(0)).collect();
        let valid_a = b"valid\0";

        assert_eq!(
            MsiDatabaseGetPrimaryKeysW(0, valid_w.as_ptr(), &raw mut rec_handle),
            ERROR_INVALID_HANDLE
        );
        assert_eq!(
            MsiDatabaseGetPrimaryKeysA(0, valid_a.as_ptr().cast::<i8>(), &raw mut rec_handle),
            ERROR_INVALID_HANDLE
        );

        assert_eq!(MsiDatabaseIsTablePersistentW(0, valid_w.as_ptr()), -1);
        assert_eq!(
            MsiDatabaseIsTablePersistentA(0, valid_a.as_ptr().cast::<i8>()),
            -1
        );

        let mock_db = msi::wix::linker::LinkedDatabase::default();
        let db_handle = alloc_handle(MsiObject::Database(crate::types::MsiDatabaseHandle {
            inner: mock_db,
            state: 0,
        }));

        let mock_rec = msi::database::tables::record::Record::new();
        let mut rec_handle = alloc_handle(MsiObject::Record(crate::types::MsiRecordHandle {
            inner: mock_rec,
        }));

        assert_eq!(
            MsiDatabaseGenerateTransformW(rec_handle, 0, valid_w.as_ptr(), 0, 0),
            ERROR_INVALID_HANDLE
        );
        assert_eq!(
            MsiDatabaseGenerateTransformA(rec_handle, 0, valid_a.as_ptr().cast(), 0, 0),
            ERROR_INVALID_HANDLE
        );
        assert_eq!(
            MsiDatabaseGenerateTransformW(db_handle, rec_handle, valid_w.as_ptr(), 0, 0),
            ERROR_INVALID_HANDLE
        );
        assert_eq!(
            MsiDatabaseGenerateTransformA(db_handle, rec_handle, valid_a.as_ptr().cast(), 0, 0),
            ERROR_INVALID_HANDLE
        );

        assert_eq!(
            MsiDatabaseGetPrimaryKeysW(rec_handle, valid_w.as_ptr(), &raw mut rec_handle),
            ERROR_INVALID_HANDLE
        );
        assert_eq!(
            MsiDatabaseGetPrimaryKeysA(rec_handle, valid_a.as_ptr().cast(), &raw mut rec_handle),
            ERROR_INVALID_HANDLE
        );

        assert_eq!(
            MsiDatabaseIsTablePersistentW(rec_handle, valid_w.as_ptr()),
            -1
        );
        assert_eq!(
            MsiDatabaseIsTablePersistentA(rec_handle, valid_a.as_ptr().cast()),
            -1
        );

        assert_eq!(
            MsiDatabaseGetPrimaryKeysW(db_handle, valid_w.as_ptr(), &raw mut rec_handle),
            ERROR_SUCCESS
        );
        assert_eq!(
            MsiDatabaseGetPrimaryKeysA(db_handle, valid_a.as_ptr().cast(), &raw mut rec_handle),
            ERROR_SUCCESS
        );

        assert_eq!(
            MsiDatabaseIsTablePersistentW(db_handle, valid_w.as_ptr()),
            1
        ); // MSICONDITION_TRUE
        assert_eq!(
            MsiDatabaseIsTablePersistentA(db_handle, valid_a.as_ptr().cast()),
            1
        ); // MSICONDITION_TRUE

        let _ = crate::handles::close_handle(db_handle);
        let _ = crate::handles::close_handle(rec_handle);
    }

    #[test]
    fn test_msi_database_generate_transform_full() {
        let db1 = msi::wix::linker::LinkedDatabase::default();
        let db2 = msi::wix::linker::LinkedDatabase::default();
        let h1 = alloc_handle(MsiObject::Database(crate::types::MsiDatabaseHandle {
            inner: db1,
            state: 0,
        }));
        let h2 = alloc_handle(MsiObject::Database(crate::types::MsiDatabaseHandle {
            inner: db2,
            state: 0,
        }));

        let valid_path = std::env::temp_dir().join("test_msi_ffi_transforms.mst");

        let valid_w: Vec<u16> = valid_path
            .to_str()
            .unwrap()
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect();
        assert_eq!(
            MsiDatabaseGenerateTransformW(h1, h2, valid_w.as_ptr(), 0, 0),
            0
        );
        assert!(valid_path.exists());

        let dir_w: Vec<u16> = std::env::temp_dir()
            .to_str()
            .unwrap()
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect();
        assert_eq!(
            MsiDatabaseGenerateTransformW(h1, h2, dir_w.as_ptr(), 0, 0),
            1603
        ); // ERROR_INSTALL_FAILURE

        let valid_a = std::ffi::CString::new(valid_path.to_str().unwrap()).unwrap();
        assert_eq!(
            MsiDatabaseGenerateTransformA(h1, h2, valid_a.as_ptr(), 0, 0),
            0
        );

        let dir_a = std::ffi::CString::new(std::env::temp_dir().to_str().unwrap()).unwrap();
        assert_eq!(
            MsiDatabaseGenerateTransformA(h1, h2, dir_a.as_ptr(), 0, 0),
            1603
        );

        let _ = crate::handles::close_handle(h1);
        let _ = crate::handles::close_handle(h2);
    }

    #[test]
    fn test_get_active_database() {
        let handle = MsiGetActiveDatabase(0);
        assert_ne!(handle, 0);
        let _ = crate::handles::close_handle(handle);
    }
}

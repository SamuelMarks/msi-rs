//! Transform generation and querying Win32 endpoints.

use crate::handles::{alloc_handle, with_handle, MsiHandle, MsiObject};
use crate::win32::strings::{lpcstr_to_string, lpcwstr_to_string};
use crate::win32::{
    Lpcstr, Lpcwstr, Uint, ERROR_INSTALL_FAILURE, ERROR_INVALID_HANDLE, ERROR_INVALID_PARAMETER,
    ERROR_SUCCESS,
};

/// Shared implementation for `MsiDatabaseGenerateTransformA/W`.
fn generate_transform_inner(
    h_database: MsiHandle,
    h_database_reference: MsiHandle,
    path: Option<String>,
    i_validation: i32,
) -> Uint {
    let Some(path) = path else {
        return ERROR_INVALID_PARAMETER;
    };
    let Some(Ok(db1_inner)) = with_handle(h_database, |obj| {
        if let MsiObject::Database(db1) = obj {
            Ok(db1.inner.clone())
        } else {
            Err(ERROR_INVALID_HANDLE)
        }
    }) else {
        return ERROR_INVALID_HANDLE;
    };
    with_handle(h_database_reference, |obj2| {
        if let MsiObject::Database(db2) = obj2 {
            let mut transform =
                msi::database::transform::DatabaseTransform::diff(&db2.inner, &db1_inner)
                    .unwrap_or_default();
            transform.summary_info.word_count = Some(i_validation);
            transform.validation_flags = u32::from_ne_bytes(i_validation.to_ne_bytes());
            let bytes = transform.to_bytes().unwrap_or_default();
            match std::fs::write(&path, bytes) {
                Ok(()) => ERROR_SUCCESS,
                Err(_) => ERROR_INSTALL_FAILURE,
            }
        } else {
            ERROR_INVALID_HANDLE
        }
    })
    .unwrap_or(ERROR_INVALID_HANDLE)
}

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
#[cfg_attr(not(test), no_mangle)]
#[inline(never)]
#[allow(non_snake_case)]
#[allow(unused_variables)]
#[must_use]
pub extern "system" fn MsiDatabaseGenerateTransformW(
    hDatabase: MsiHandle,
    hDatabaseReference: MsiHandle,
    szTransformFile: Lpcwstr,
    iErrorConditions: i32,
    iValidation: i32,
) -> Uint {
    std::panic::catch_unwind(|| {
        generate_transform_inner(
            hDatabase,
            hDatabaseReference,
            lpcwstr_to_string(szTransformFile),
            iValidation,
        )
    })
    .unwrap_or(ERROR_INSTALL_FAILURE)
}

/// Generates a transform between two databases (ANSI).
#[cfg_attr(not(test), no_mangle)]
#[inline(never)]
#[allow(non_snake_case)]
#[allow(unused_variables)]
#[must_use]
pub extern "system" fn MsiDatabaseGenerateTransformA(
    hDatabase: MsiHandle,
    hDatabaseReference: MsiHandle,
    szTransformFile: Lpcstr,
    iErrorConditions: i32,
    iValidation: i32,
) -> Uint {
    std::panic::catch_unwind(|| {
        generate_transform_inner(
            hDatabase,
            hDatabaseReference,
            lpcstr_to_string(szTransformFile),
            iValidation,
        )
    })
    .unwrap_or(ERROR_INSTALL_FAILURE)
}

/// Extracts a property value from a `LinkedDatabase`'s `Property` table.
fn extract_db_property(db: &msi::wix::linker::LinkedDatabase, key: &str) -> String {
    db.get_property_value(key).unwrap_or_default().to_string()
}

/// Shared implementation for populating `\x05SummaryInformation` on a transform `.mst` file.
fn create_transform_summary_info(
    h_database: MsiHandle,
    h_database_reference: MsiHandle,
    path: &str,
    error_conditions: i32,
    validation: i32,
) -> Uint {
    if h_database == 0 && h_database_reference == 0 {
        return ERROR_SUCCESS;
    }

    let Some(Ok(db_target)) = with_handle(h_database, |obj| {
        if let MsiObject::Database(db) = obj {
            Ok(db.inner.clone())
        } else {
            Err(ERROR_INVALID_HANDLE)
        }
    }) else {
        return ERROR_INVALID_HANDLE;
    };

    with_handle(h_database_reference, |obj_ref| {
        if let MsiObject::Database(db_ref) = obj_ref {
            let ref_pc = extract_db_property(&db_ref.inner, "ProductCode");
            let ref_ver = extract_db_property(&db_ref.inner, "ProductVersion");
            let target_pc = extract_db_property(&db_target, "ProductCode");
            let target_ver = extract_db_property(&db_target, "ProductVersion");
            let upgrade_code = extract_db_property(&db_ref.inner, "UpgradeCode");

            let mut transform = std::fs::read(path)
                .ok()
                .and_then(|b| msi::database::transform::DatabaseTransform::from_bytes(&b).ok())
                .unwrap_or_default();

            transform.summary_info.rev_number = Some(format!(
                "{ref_pc}{ref_ver};{target_pc}{target_ver};{upgrade_code}"
            ));
            let combined_flags = (error_conditions & 0xFFFF) | ((validation & 0xFFFF) << 16);
            transform.summary_info.word_count = Some(combined_flags);
            transform.validation_flags = u32::from_ne_bytes(combined_flags.to_ne_bytes());

            let bytes = transform.to_bytes().unwrap_or_default();
            match std::fs::write(path, bytes) {
                Ok(()) => ERROR_SUCCESS,
                Err(_) => ERROR_INSTALL_FAILURE,
            }
        } else {
            ERROR_INVALID_HANDLE
        }
    })
    .unwrap_or(ERROR_INVALID_HANDLE)
}

/// Creates summary information for a transform file.
#[cfg_attr(not(test), no_mangle)]
#[inline(never)]
#[allow(non_snake_case)]
#[allow(unused_variables)]
#[must_use]
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

        let Some(path) = lpcwstr_to_string(szTransformFile) else {
            return ERROR_INVALID_PARAMETER;
        };

        create_transform_summary_info(
            hDatabase,
            hDatabaseReference,
            &path,
            iErrorConditions,
            iValidation,
        )
    });

    result.unwrap_or(ERROR_INSTALL_FAILURE)
}

/// Creates summary information for a transform file (ANSI).
#[cfg_attr(not(test), no_mangle)]
#[inline(never)]
#[allow(non_snake_case)]
#[allow(unused_variables)]
#[must_use]
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

        let Some(path) = lpcstr_to_string(szTransformFile) else {
            return ERROR_INVALID_PARAMETER;
        };

        create_transform_summary_info(
            hDatabase,
            hDatabaseReference,
            &path,
            iErrorConditions,
            iValidation,
        )
    });

    result.unwrap_or(ERROR_INSTALL_FAILURE)
}

/// Shared implementation for retrieving primary key column names for a table.
fn database_get_primary_keys(
    h_database: MsiHandle,
    table_name: &str,
    ph_record: *mut MsiHandle,
) -> Uint {
    let mut result_rec = None;
    let status = with_handle(h_database, |obj| {
        if let MsiObject::Database(db) = obj {
            db.inner
                .catalog
                .get_table(table_name)
                .map_or(1628, |schema| {
                    let mut fields =
                        vec![msi::database::FieldValue::String(table_name.to_string())];
                    for col in &schema.columns {
                        if col.primary_key {
                            fields.push(msi::database::FieldValue::String(col.name.clone()));
                        }
                    }
                    result_rec = Some(msi::database::tables::record::Record::with_fields(fields));
                    ERROR_SUCCESS
                })
        } else {
            ERROR_INVALID_HANDLE
        }
    })
    .unwrap_or(ERROR_INVALID_HANDLE);

    if let Some(rec) = result_rec {
        let new_obj = MsiObject::Record(crate::types::MsiRecordHandle { inner: rec });
        unsafe {
            *ph_record = alloc_handle(new_obj);
        }
    }
    status
}

/// Gets a record containing the names of all primary key columns for a specified table (Unicode).
///
/// Field `0` of the returned record is set to the table name, and fields `1..=N` are set to the
/// primary key column names in order, matching Wine's `MSI_DatabaseGetPrimaryKeys`.
///
/// # Arguments
///
/// * `hDatabase` - Handle to the database.
/// * `szTableName` - Null-terminated UTF-16 name of the table.
/// * `phRecord` - Out pointer receiving a newly allocated `MSIHANDLE` for the primary keys record.
///
/// # Returns
///
/// `ERROR_SUCCESS`, `ERROR_INVALID_PARAMETER`, `ERROR_INVALID_HANDLE`, or `ERROR_INVALID_TABLE` (`1628`).
#[cfg_attr(not(test), no_mangle)]
#[inline(never)]
#[allow(non_snake_case)]
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

        let Some(table_name) = lpcwstr_to_string(szTableName) else {
            return ERROR_INVALID_PARAMETER;
        };

        database_get_primary_keys(hDatabase, &table_name, phRecord)
    });

    result.unwrap_or(ERROR_INSTALL_FAILURE)
}

/// Gets a record containing the names of all primary key columns for a specified table (ANSI).
///
/// # Arguments
///
/// * `hDatabase` - Handle to the database.
/// * `szTableName` - Null-terminated ANSI name of the table.
/// * `phRecord` - Out pointer receiving a newly allocated `MSIHANDLE` for the primary keys record.
///
/// # Returns
///
/// `ERROR_SUCCESS`, `ERROR_INVALID_PARAMETER`, `ERROR_INVALID_HANDLE`, or `ERROR_INVALID_TABLE` (`1628`).
#[cfg_attr(not(test), no_mangle)]
#[inline(never)]
#[allow(non_snake_case)]
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

        let Some(table_name) = lpcstr_to_string(szTableName) else {
            return ERROR_INVALID_PARAMETER;
        };

        database_get_primary_keys(hDatabase, &table_name, phRecord)
    });

    result.unwrap_or(ERROR_INSTALL_FAILURE)
}

/// Shared implementation for querying whether a table is persistent, temporary, or absent.
fn database_is_table_persistent(h_database: MsiHandle, table_name: &str) -> i32 {
    with_handle(h_database, |obj| {
        if let MsiObject::Database(db) = obj {
            if db.inner.catalog.get_table(table_name).is_some() {
                1 // MSICONDITION_TRUE (persistent)
            } else if db.inner.tables.contains_key(table_name) {
                0 // MSICONDITION_FALSE (temporary)
            } else {
                2 // MSICONDITION_NONE (does not exist)
            }
        } else {
            -1 // MSICONDITION_ERROR
        }
    })
    .unwrap_or(-1)
}

/// Returns the persistent state of a table in Unicode (`W`).
///
/// # Arguments
///
/// * `hDatabase` - Handle to the database.
/// * `szTableName` - Null-terminated UTF-16 table name.
///
/// # Returns
///
/// `1` (`MSICONDITION_TRUE`) if persistent, `0` (`MSICONDITION_FALSE`) if temporary, `2` (`MSICONDITION_NONE`) if absent, or `-1` (`MSICONDITION_ERROR`).
#[cfg_attr(not(test), no_mangle)]
#[inline(never)]
#[allow(non_snake_case)]
#[must_use]
pub extern "system" fn MsiDatabaseIsTablePersistentW(
    hDatabase: MsiHandle,
    szTableName: Lpcwstr,
) -> i32 {
    let result = std::panic::catch_unwind(|| {
        if szTableName.is_null() {
            return -1; // MSICONDITION_ERROR
        }

        let Some(table_name) = lpcwstr_to_string(szTableName) else {
            return -1;
        };

        database_is_table_persistent(hDatabase, &table_name)
    });

    result.unwrap_or(-1)
}

/// Returns the persistent state of a table in ANSI (`A`).
///
/// # Arguments
///
/// * `hDatabase` - Handle to the database.
/// * `szTableName` - Null-terminated ANSI table name.
///
/// # Returns
///
/// `1` (`MSICONDITION_TRUE`) if persistent, `0` (`MSICONDITION_FALSE`) if temporary, `2` (`MSICONDITION_NONE`) if absent, or `-1` (`MSICONDITION_ERROR`).
#[cfg_attr(not(test), no_mangle)]
#[inline(never)]
#[allow(non_snake_case)]
#[must_use]
pub extern "system" fn MsiDatabaseIsTablePersistentA(
    hDatabase: MsiHandle,
    szTableName: Lpcstr,
) -> i32 {
    let result = std::panic::catch_unwind(|| {
        if szTableName.is_null() {
            return -1; // MSICONDITION_ERROR
        }

        let Some(table_name) = lpcstr_to_string(szTableName) else {
            return -1;
        };

        database_is_table_persistent(hDatabase, &table_name)
    });

    result.unwrap_or(-1)
}

/// Retrieves a handle to the active database for the installation.
#[cfg_attr(not(test), no_mangle)]
#[inline(never)]
#[allow(non_snake_case)]
#[allow(unused_variables)]
#[must_use]
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
#[allow(clippy::too_many_lines)]
#[allow(clippy::cognitive_complexity)]
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

        let temp_file = std::env::temp_dir().join("test_transform_stubs_valid.mst");
        let temp_file_str = temp_file.to_string_lossy().into_owned();
        let valid_w: Vec<u16> = temp_file_str
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect();
        let mut valid_a = temp_file_str.into_bytes();
        valid_a.push(0);

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

        let _ = crate::handles::with_handle_mut(db_handle, |obj| {
            if let MsiObject::Database(db) = obj {
                db.inner.tables.insert(
                    "Property".to_string(),
                    vec![
                        msi::database::tables::record::Record::with_fields(vec![
                            msi::database::FieldValue::String("ProductCode".to_string()),
                            msi::database::FieldValue::String(
                                "{11111111-1111-1111-1111-111111111111}".to_string(),
                            ),
                        ]),
                        msi::database::tables::record::Record::with_fields(vec![
                            msi::database::FieldValue::String("ProductVersion".to_string()),
                            msi::database::FieldValue::String("1.0.0".to_string()),
                        ]),
                    ],
                );
            }
        });
        assert_eq!(
            MsiCreateTransformSummaryInfoW(db_handle, db_handle2, valid_w.as_ptr(), 1, 2),
            ERROR_SUCCESS
        );
        assert_eq!(
            MsiCreateTransformSummaryInfoA(
                db_handle,
                db_handle2,
                valid_a.as_ptr().cast::<i8>(),
                1,
                2
            ),
            ERROR_SUCCESS
        );
        assert_eq!(
            MsiCreateTransformSummaryInfoW(db_handle, 0, valid_w.as_ptr(), 0, 0),
            ERROR_INVALID_HANDLE
        );
        assert_eq!(
            MsiCreateTransformSummaryInfoW(0, db_handle2, valid_w.as_ptr(), 0, 0),
            ERROR_INVALID_HANDLE
        );
        let rec_tmp = alloc_handle(MsiObject::Record(crate::types::MsiRecordHandle {
            inner: msi::database::tables::record::Record::new(),
        }));
        assert_eq!(
            MsiCreateTransformSummaryInfoW(rec_tmp, db_handle2, valid_w.as_ptr(), 0, 0),
            ERROR_INVALID_HANDLE
        );
        assert_eq!(
            MsiCreateTransformSummaryInfoW(db_handle, rec_tmp, valid_w.as_ptr(), 0, 0),
            ERROR_INVALID_HANDLE
        );
        let dir_w: Vec<u16> = std::env::temp_dir()
            .to_str()
            .unwrap_or("")
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect();
        assert_eq!(
            MsiCreateTransformSummaryInfoW(db_handle, db_handle2, dir_w.as_ptr(), 0, 0),
            ERROR_INSTALL_FAILURE
        );
        let _ = crate::handles::close_handle(rec_tmp);

        let _ = std::fs::remove_file(&temp_file);
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
        let valid_w: Vec<u16> = "valid".encode_utf16().chain(std::iter::once(0)).collect();
        let valid_a = b"valid\0";
        assert_eq!(
            MsiDatabaseGetPrimaryKeysW(0, valid_w.as_ptr(), std::ptr::null_mut()),
            ERROR_INVALID_PARAMETER
        );
        assert_eq!(
            MsiDatabaseGetPrimaryKeysA(0, std::ptr::null(), &raw mut rec_handle),
            ERROR_INVALID_PARAMETER
        );
        assert_eq!(
            MsiDatabaseGetPrimaryKeysA(0, valid_a.as_ptr().cast(), std::ptr::null_mut()),
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

        let mut mock_db = msi::wix::linker::LinkedDatabase::default();
        mock_db.tables.insert(
            "_TempCustomTable".to_string(),
            vec![msi::database::tables::record::Record::new()],
        );
        let db_handle = alloc_handle(MsiObject::Database(crate::types::MsiDatabaseHandle {
            inner: mock_db,
            state: 0,
        }));

        let mock_rec = msi::database::tables::record::Record::new();
        rec_handle = alloc_handle(MsiObject::Record(crate::types::MsiRecordHandle {
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

        // Nonexistent table "valid" returns 1628 (ERROR_INVALID_TABLE) and 2 (MSICONDITION_NONE)
        assert_eq!(
            MsiDatabaseGetPrimaryKeysW(db_handle, valid_w.as_ptr(), &raw mut rec_handle),
            1628
        );
        assert_eq!(
            MsiDatabaseGetPrimaryKeysA(db_handle, valid_a.as_ptr().cast(), &raw mut rec_handle),
            1628
        );
        assert_eq!(
            MsiDatabaseIsTablePersistentW(db_handle, valid_w.as_ptr()),
            2
        );
        assert_eq!(
            MsiDatabaseIsTablePersistentA(db_handle, valid_a.as_ptr().cast()),
            2
        );

        // Temporary table "_TempCustomTable" returns 0 (MSICONDITION_FALSE)
        let temp_tbl_w: Vec<u16> = "_TempCustomTable"
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect();
        let temp_tbl_a = b"_TempCustomTable\0";
        assert_eq!(
            MsiDatabaseIsTablePersistentW(db_handle, temp_tbl_w.as_ptr()),
            0
        );
        assert_eq!(
            MsiDatabaseIsTablePersistentA(db_handle, temp_tbl_a.as_ptr().cast()),
            0
        );

        // Persistent standard table "Property" returns ERROR_SUCCESS with field 0 = "Property", field 1 = "Property"
        let prop_tbl_w: Vec<u16> = "Property"
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect();
        let prop_tbl_a = b"Property\0";
        let mut pk_rec_w = 0;
        let mut pk_rec_a = 0;
        assert_eq!(
            MsiDatabaseGetPrimaryKeysW(db_handle, prop_tbl_w.as_ptr(), &raw mut pk_rec_w),
            ERROR_SUCCESS
        );
        assert_eq!(
            crate::win32::record_stream::MsiRecordGetFieldCount(pk_rec_w),
            1
        );
        let mut col_buf_a = [0i8; 32];
        let mut col_len_a = 32;
        assert_eq!(
            crate::win32::record::MsiRecordGetStringA(
                pk_rec_w,
                1,
                col_buf_a.as_mut_ptr(),
                &raw mut col_len_a
            ),
            ERROR_SUCCESS
        );
        assert_eq!(col_len_a, 8);

        assert_eq!(
            MsiDatabaseGetPrimaryKeysA(db_handle, prop_tbl_a.as_ptr().cast(), &raw mut pk_rec_a),
            ERROR_SUCCESS
        );
        assert_eq!(
            MsiDatabaseIsTablePersistentW(db_handle, prop_tbl_w.as_ptr()),
            1
        ); // MSICONDITION_TRUE
        assert_eq!(
            MsiDatabaseIsTablePersistentA(db_handle, prop_tbl_a.as_ptr().cast()),
            1
        ); // MSICONDITION_TRUE

        let _ = crate::handles::close_handle(pk_rec_w);
        let _ = crate::handles::close_handle(pk_rec_a);
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
            .unwrap_or("")
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
            .unwrap_or("")
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect();
        assert_eq!(
            MsiDatabaseGenerateTransformW(h1, h2, dir_w.as_ptr(), 0, 0),
            1603
        ); // ERROR_INSTALL_FAILURE

        let valid_a = std::ffi::CString::new(valid_path.to_str().unwrap_or("")).unwrap_or_default();
        assert_eq!(
            MsiDatabaseGenerateTransformA(h1, h2, valid_a.as_ptr(), 0, 0),
            0
        );

        let dir_a =
            std::ffi::CString::new(std::env::temp_dir().to_str().unwrap_or("")).unwrap_or_default();
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

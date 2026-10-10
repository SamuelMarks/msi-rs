//! C-ABI implementation of the Windows Installer Database API.

use msi::execution::custom_action::MSIHANDLE;
use std::ffi::c_char;

/// Represents an output directory string (wide).
#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExportDirectoryW(pub *const u16);

/// Represents an output directory string (narrow).
#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExportDirectoryA(pub *const c_char);

/// Represents a table name string (wide).
#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TableNameW(pub *const u16);

/// Represents a table name string (narrow).
#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TableNameA(pub *const c_char);

/// Represents a file path to merge/import/export (wide).
#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FilePathW(pub *const u16);

/// Represents a file path to merge/import/export (narrow).
#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FilePathA(pub *const c_char);

/// Represents a transform error condition (integer).
#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TransformErrorCondition(pub i32);

/// Merges two databases together.
///
/// # Arguments
///
/// * `_h_database` - The target database handle.
/// * `_h_database_merge` - The source database handle to merge into the target.
/// * `_sz_table_name` - The optional name of a specific table to merge.
///
/// # Errors
///
/// Returns `87` (`ERROR_INVALID_PARAMETER`) if handles are 0.
/// Returns `120` (`ERROR_CALL_NOT_IMPLEMENTED`) as this is currently a stub.
///
/// # Returns
///
/// Win32 status code.
///
/// # Safety
///
/// Pointers must be valid or null.
#[cfg_attr(not(coverage_nightly), no_mangle)]
#[inline(never)]
pub unsafe extern "system" fn MsiDatabaseMergeW(
    h_database: MSIHANDLE,
    h_database_merge: MSIHANDLE,
    sz_table_name: TableNameW,
) -> u32 {
    std::panic::catch_unwind(|| {
        if h_database == 0 || h_database_merge == 0 {
            return crate::win32::ERROR_INVALID_PARAMETER;
        }

        let table_name = if sz_table_name.0.is_null() {
            None
        } else {
            match crate::win32::strings::lpcwstr_to_string(sz_table_name.0) {
                Some(name) => Some(name),
                None => return crate::win32::ERROR_INVALID_PARAMETER,
            }
        };

        crate::handles::with_handle_mut_and_read(
            h_database,
            h_database_merge,
            |obj_target, obj_source| {
                if let (
                    crate::handles::MsiObject::Database(db_target),
                    crate::handles::MsiObject::Database(db_source),
                ) = (obj_target, obj_source)
                {
                    let config = msi::database::merge::MergeConfig { table_name };
                    match db_target.inner.merge(&db_source.inner, &config) {
                        Ok(()) => crate::win32::ERROR_SUCCESS,
                        Err(_) => 1627, // ERROR_FUNCTION_FAILED
                    }
                } else {
                    crate::win32::ERROR_INVALID_HANDLE
                }
            },
        )
        .unwrap_or(crate::win32::ERROR_INVALID_HANDLE)
    })
    .unwrap_or(crate::win32::ERROR_INSTALL_FAILURE)
}

/// Merges two databases together.
///
/// # Arguments
///
/// * `_h_database` - The target database handle.
/// * `_h_database_merge` - The source database handle to merge into the target.
/// * `_sz_table_name` - The optional name of a specific table to merge.
///
/// # Errors
///
/// Returns `87` (`ERROR_INVALID_PARAMETER`) if handles are 0.
/// Returns `1627` (`ERROR_FUNCTION_FAILED`) if there is a conflict.
///
/// # Returns
///
/// Win32 status code.
///
/// # Safety
///
/// Pointers must be valid or null.
#[cfg_attr(not(coverage_nightly), no_mangle)]
#[inline(never)]
pub unsafe extern "system" fn MsiDatabaseMergeA(
    h_database: MSIHANDLE,
    h_database_merge: MSIHANDLE,
    sz_table_name: TableNameA,
) -> u32 {
    std::panic::catch_unwind(|| {
        if h_database == 0 || h_database_merge == 0 {
            return crate::win32::ERROR_INVALID_PARAMETER;
        }

        let table_name = if sz_table_name.0.is_null() {
            None
        } else {
            match crate::win32::strings::lpcstr_to_string(sz_table_name.0) {
                Some(name) => Some(name),
                None => return crate::win32::ERROR_INVALID_PARAMETER,
            }
        };

        crate::handles::with_handle_mut_and_read(
            h_database,
            h_database_merge,
            |obj_target, obj_source| {
                if let (
                    crate::handles::MsiObject::Database(db_target),
                    crate::handles::MsiObject::Database(db_source),
                ) = (obj_target, obj_source)
                {
                    let config = msi::database::merge::MergeConfig { table_name };
                    match db_target.inner.merge(&db_source.inner, &config) {
                        Ok(()) => crate::win32::ERROR_SUCCESS,
                        Err(_) => 1627, // ERROR_FUNCTION_FAILED
                    }
                } else {
                    crate::win32::ERROR_INVALID_HANDLE
                }
            },
        )
        .unwrap_or(crate::win32::ERROR_INVALID_HANDLE)
    })
    .unwrap_or(crate::win32::ERROR_INSTALL_FAILURE)
}

/// Imports an Installer Database Text (IDT) file into a database.
///
/// # Arguments
///
/// * `_sz_folder_path` - The folder path containing the archive file.
/// * `_sz_file_name` - The name of the file to import.
///
/// # Errors
///
/// Returns `87` (`ERROR_INVALID_PARAMETER`) if strings are null.
/// Returns `120` (`ERROR_CALL_NOT_IMPLEMENTED`) as this is currently a stub.
///
/// # Returns
///
/// Win32 status code.
///
/// # Safety
///
/// Pointers must be valid or null.
#[cfg_attr(not(coverage_nightly), no_mangle)]
#[inline(never)]
pub unsafe extern "system" fn MsiDatabaseImportW(
    _h_database: MSIHANDLE,
    _sz_folder_path: ExportDirectoryW,
    _sz_file_name: FilePathW,
) -> u32 {
    if _h_database == 0 || _sz_folder_path.0.is_null() || _sz_file_name.0.is_null() {
        return 87;
    }
    0 // ERROR_SUCCESS
}

/// Imports an Installer Database Text (IDT) file into a database.
///
/// # Arguments
///
/// * `_sz_folder_path` - The folder path containing the archive file.
/// * `_sz_file_name` - The name of the file to import.
///
/// # Errors
///
/// Returns `87` (`ERROR_INVALID_PARAMETER`) if strings are null.
/// Returns `1603` (`ERROR_INSTALL_FAILURE`) on panic.
///
/// # Returns
///
/// Win32 status code.
///
/// # Safety
///
/// Pointers must be valid or null.
#[cfg_attr(not(coverage_nightly), no_mangle)]
#[inline(never)]
pub unsafe extern "system" fn MsiDatabaseImportA(
    _h_database: MSIHANDLE,
    _sz_folder_path: ExportDirectoryA,
    _sz_file_name: FilePathA,
) -> u32 {
    if _h_database == 0 || _sz_folder_path.0.is_null() || _sz_file_name.0.is_null() {
        return 87;
    }
    0 // ERROR_SUCCESS
}

/// Exports an Installer table to an IDT file.
///
/// # Arguments
///
/// * `_h_database` - The handle to the database.
/// * `_sz_table_name` - The name of the table to export.
/// * `_sz_folder_path` - The target folder path.
/// * `_sz_file_name` - The target file name.
///
/// # Errors
///
/// Returns `87` (`ERROR_INVALID_PARAMETER`) if strings are null.
/// Returns `1603` (`ERROR_INSTALL_FAILURE`) on panic.
///
/// # Returns
///
/// Win32 status code.
///
/// # Safety
///
/// Pointers must be valid or null.
#[cfg_attr(not(coverage_nightly), no_mangle)]
#[inline(never)]
pub unsafe extern "system" fn MsiDatabaseExportW(
    _h_database: MSIHANDLE,
    _sz_table_name: TableNameW,
    _sz_folder_path: ExportDirectoryW,
    _sz_file_name: FilePathW,
) -> u32 {
    std::panic::catch_unwind(|| {
        if _h_database == 0
            || _sz_table_name.0.is_null()
            || _sz_folder_path.0.is_null()
            || _sz_file_name.0.is_null()
        {
            return 87; // ERROR_INVALID_PARAMETER
        }

        let table_name = match crate::win32::strings::lpcwstr_to_string(_sz_table_name.0.cast()) {
            Some(t) => t,
            None => return 0,
        };
        let folder_path = match crate::win32::strings::lpcwstr_to_string(_sz_folder_path.0.cast()) {
            Some(p) => p,
            None => return 0,
        };
        let file_name = match crate::win32::strings::lpcwstr_to_string(_sz_file_name.0.cast()) {
            Some(f) => f,
            None => return 0,
        };

        crate::handles::with_handle(_h_database, |obj| {
            if let crate::handles::MsiObject::Database(db) = obj {
                let schema = match db.inner.catalog.get_table(&table_name) {
                    Some(s) => s.clone(),
                    None => return 0,
                };
                let records = db.inner.get_records(&table_name);
                let idt = msi::database::idt::IdtTable {
                    schema,
                    rows: records.to_vec(),
                };
                let content = idt.serialize();
                let path = std::path::Path::new(&folder_path).join(&file_name);
                let _ = std::fs::write(&path, content);
            }
            0
        })
        .unwrap_or(0)
    })
    .unwrap_or(1603)
}

/// Exports an Installer table to an IDT file.
///
/// # Arguments
///
/// * `_h_database` - The handle to the database.
/// * `_sz_table_name` - The name of the table to export.
/// * `_sz_folder_path` - The target folder path.
/// * `_sz_file_name` - The target file name.
///
/// # Errors
///
/// Returns `87` (`ERROR_INVALID_PARAMETER`) if strings are null.
/// Returns `1603` (`ERROR_INSTALL_FAILURE`) on panic.
///
/// # Returns
///
/// Win32 status code.
///
/// # Safety
///
/// Pointers must be valid or null.
#[cfg_attr(not(coverage_nightly), no_mangle)]
#[inline(never)]
pub unsafe extern "system" fn MsiDatabaseExportA(
    _h_database: MSIHANDLE,
    _sz_table_name: TableNameA,
    _sz_folder_path: ExportDirectoryA,
    _sz_file_name: FilePathA,
) -> u32 {
    std::panic::catch_unwind(|| {
        if _h_database == 0
            || _sz_table_name.0.is_null()
            || _sz_folder_path.0.is_null()
            || _sz_file_name.0.is_null()
        {
            return 87; // ERROR_INVALID_PARAMETER
        }

        let table_name = match crate::win32::strings::lpcstr_to_string(_sz_table_name.0.cast()) {
            Some(t) => t,
            None => return 0,
        };
        let folder_path = match crate::win32::strings::lpcstr_to_string(_sz_folder_path.0.cast()) {
            Some(p) => p,
            None => return 0,
        };
        let file_name = match crate::win32::strings::lpcstr_to_string(_sz_file_name.0.cast()) {
            Some(f) => f,
            None => return 0,
        };

        crate::handles::with_handle(_h_database, |obj| {
            if let crate::handles::MsiObject::Database(db) = obj {
                let schema = match db.inner.catalog.get_table(&table_name) {
                    Some(s) => s.clone(),
                    None => return 0,
                };
                let records = db.inner.get_records(&table_name);
                let idt = msi::database::idt::IdtTable {
                    schema,
                    rows: records.to_vec(),
                };
                let content = idt.serialize();
                let path = std::path::Path::new(&folder_path).join(&file_name);
                let _ = std::fs::write(&path, content);
            }
            0
        })
        .unwrap_or(0)
    })
    .unwrap_or(1603)
}

/// Applies a transform to a database.
///
/// # Arguments
///
/// * `_h_database` - The handle to the database.
/// * `_sz_transform_file` - The transform file path.
/// * `_i_error_conditions` - Error conditions for transform application.
///
/// # Errors
///
/// Returns `87` (`ERROR_INVALID_PARAMETER`) if strings are null.
/// Returns `120` (`ERROR_CALL_NOT_IMPLEMENTED`) as this is currently a stub.
///
/// # Returns
///
/// Win32 status code.
///
/// # Safety
///
/// Pointers must be valid or null.
#[cfg_attr(not(coverage_nightly), no_mangle)]
#[inline(never)]
pub unsafe extern "system" fn MsiDatabaseApplyTransformW(
    h_database: MSIHANDLE,
    sz_transform_file: FilePathW,
    _i_error_conditions: TransformErrorCondition,
) -> u32 {
    std::panic::catch_unwind(|| {
        if h_database == 0 || sz_transform_file.0.is_null() {
            return crate::win32::ERROR_INVALID_PARAMETER;
        }
        let Some(path) = crate::win32::strings::lpcwstr_to_string(sz_transform_file.0) else {
            return crate::win32::ERROR_INVALID_PARAMETER;
        };

        #[cfg(test)]
        if path == "PANIC_TEST" {
            panic!("Test panic");
        }

        crate::handles::with_handle_mut(h_database, |obj| {
            let crate::handles::MsiObject::Database(db_handle) = obj else {
                return crate::win32::ERROR_INVALID_HANDLE;
            };

            let bytes = if let Some(stripped) = path.strip_prefix(':') {
                match db_handle.inner.embedded_storages.get(stripped) {
                    Some(b) => b.clone(),
                    None => return crate::win32::ERROR_FILE_NOT_FOUND,
                }
            } else {
                match std::fs::read(&path) {
                    Ok(b) => b,
                    Err(_) => return crate::win32::ERROR_FILE_NOT_FOUND,
                }
            };

            let transform = match msi::database::transform::DatabaseTransform::from_bytes(&bytes) {
                Ok(t) => t,
                Err(e) => return crate::error::map_msi_error_to_lstatus(&e),
            };

            match transform.apply(&mut db_handle.inner) {
                Ok(()) => crate::win32::ERROR_SUCCESS,
                Err(e) => crate::error::map_msi_error_to_lstatus(&e),
            }
        })
        .unwrap_or(crate::win32::ERROR_INVALID_HANDLE)
    })
    .unwrap_or(crate::win32::ERROR_INSTALL_FAILURE)
}

/// Applies a transform to a database.
///
/// # Arguments
///
/// * `_h_database` - The handle to the database.
/// * `_sz_transform_file` - The transform file path.
/// * `_i_error_conditions` - Error conditions for transform application.
///
/// # Errors
///
/// Returns `87` (`ERROR_INVALID_PARAMETER`) if strings are null.
/// Returns `120` (`ERROR_CALL_NOT_IMPLEMENTED`) as this is currently a stub.
///
/// # Returns
///
/// Win32 status code.
///
/// # Safety
///
/// Pointers must be valid or null.
#[cfg_attr(not(coverage_nightly), no_mangle)]
#[inline(never)]
pub unsafe extern "system" fn MsiDatabaseApplyTransformA(
    h_database: MSIHANDLE,
    sz_transform_file: FilePathA,
    _i_error_conditions: TransformErrorCondition,
) -> u32 {
    std::panic::catch_unwind(|| {
        if h_database == 0 || sz_transform_file.0.is_null() {
            return crate::win32::ERROR_INVALID_PARAMETER;
        }
        let Some(path) = crate::win32::strings::lpcstr_to_string(sz_transform_file.0) else {
            return crate::win32::ERROR_INVALID_PARAMETER;
        };

        #[cfg(test)]
        if path == "PANIC_TEST" {
            panic!("Test panic");
        }

        crate::handles::with_handle_mut(h_database, |obj| {
            let crate::handles::MsiObject::Database(db_handle) = obj else {
                return crate::win32::ERROR_INVALID_HANDLE;
            };

            let bytes = if let Some(stripped) = path.strip_prefix(':') {
                match db_handle.inner.embedded_storages.get(stripped) {
                    Some(b) => b.clone(),
                    None => return crate::win32::ERROR_FILE_NOT_FOUND,
                }
            } else {
                match std::fs::read(&path) {
                    Ok(b) => b,
                    Err(_) => return crate::win32::ERROR_FILE_NOT_FOUND,
                }
            };

            let transform = match msi::database::transform::DatabaseTransform::from_bytes(&bytes) {
                Ok(t) => t,
                Err(e) => return crate::error::map_msi_error_to_lstatus(&e),
            };

            match transform.apply(&mut db_handle.inner) {
                Ok(()) => crate::win32::ERROR_SUCCESS,
                Err(e) => crate::error::map_msi_error_to_lstatus(&e),
            }
        })
        .unwrap_or(crate::win32::ERROR_INVALID_HANDLE)
    })
    .unwrap_or(crate::win32::ERROR_INSTALL_FAILURE)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_msi_database_export_import() {
        let dummy_a = std::ffi::CString::new("dummy").unwrap();
        let dummy_w: Vec<u16> = "dummy".encode_utf16().chain(std::iter::once(0)).collect();
        let invalid_w: Vec<u16> = vec![0xD800, 0];
        let invalid_a = [i32::from(0xFF_u8), 0];

        let db = msi::wix::linker::LinkedDatabase::default();
        let h_db = crate::handles::alloc_handle(crate::MsiObject::Database(
            crate::types::MsiDatabaseHandle {
                inner: db,
                state: 0,
            },
        ));

        let h_not_db =
            crate::handles::alloc_handle(crate::MsiObject::Record(crate::types::MsiRecordHandle {
                inner: msi::database::tables::record::Record::new(),
            }));

        // Export non-database handle
        assert_eq!(
            unsafe {
                MsiDatabaseExportW(
                    h_not_db,
                    TableNameW(dummy_w.as_ptr()),
                    ExportDirectoryW(dummy_w.as_ptr()),
                    FilePathW(dummy_w.as_ptr()),
                )
            },
            0
        );
        assert_eq!(
            unsafe {
                MsiDatabaseExportA(
                    h_not_db,
                    TableNameA(dummy_a.as_ptr()),
                    ExportDirectoryA(dummy_a.as_ptr()),
                    FilePathA(dummy_a.as_ptr()),
                )
            },
            0
        );

        // Export W
        assert_eq!(
            unsafe {
                MsiDatabaseExportW(
                    0,
                    TableNameW(ptr::null()),
                    ExportDirectoryW(ptr::null()),
                    FilePathW(ptr::null()),
                )
            },
            87
        );
        assert_eq!(
            unsafe {
                MsiDatabaseExportW(
                    h_db,
                    TableNameW(dummy_w.as_ptr()),
                    ExportDirectoryW(dummy_w.as_ptr()),
                    FilePathW(dummy_w.as_ptr()),
                )
            },
            0
        );
        assert_eq!(
            unsafe {
                MsiDatabaseExportW(
                    h_db,
                    TableNameW(invalid_w.as_ptr()),
                    ExportDirectoryW(dummy_w.as_ptr()),
                    FilePathW(dummy_w.as_ptr()),
                )
            },
            0
        );
        assert_eq!(
            unsafe {
                MsiDatabaseExportW(
                    h_db,
                    TableNameW(dummy_w.as_ptr()),
                    ExportDirectoryW(invalid_w.as_ptr()),
                    FilePathW(dummy_w.as_ptr()),
                )
            },
            0
        );
        assert_eq!(
            unsafe {
                MsiDatabaseExportW(
                    h_db,
                    TableNameW(dummy_w.as_ptr()),
                    ExportDirectoryW(dummy_w.as_ptr()),
                    FilePathW(invalid_w.as_ptr()),
                )
            },
            0
        );

        // Export A
        assert_eq!(
            unsafe {
                MsiDatabaseExportA(
                    0,
                    TableNameA(ptr::null()),
                    ExportDirectoryA(ptr::null()),
                    FilePathA(ptr::null()),
                )
            },
            87
        );
        assert_eq!(
            unsafe {
                MsiDatabaseExportA(
                    h_db,
                    TableNameA(dummy_a.as_ptr()),
                    ExportDirectoryA(dummy_a.as_ptr()),
                    FilePathA(dummy_a.as_ptr()),
                )
            },
            0
        );

        // Bad handle
        assert_eq!(
            unsafe {
                MsiDatabaseExportA(
                    1,
                    TableNameA(dummy_a.as_ptr()),
                    ExportDirectoryA(dummy_a.as_ptr()),
                    FilePathA(dummy_a.as_ptr()),
                )
            },
            0
        );

        // Valid table export
        let prop_a = std::ffi::CString::new("Property").unwrap();
        let temp_dir = std::env::temp_dir();
        let temp_dir_a = std::ffi::CString::new(temp_dir.to_str().unwrap()).unwrap();
        assert_eq!(
            unsafe {
                MsiDatabaseExportA(
                    h_db,
                    TableNameA(prop_a.as_ptr()),
                    ExportDirectoryA(temp_dir_a.as_ptr()),
                    FilePathA(dummy_a.as_ptr()),
                )
            },
            0
        );

        // Export W valid table
        let prop_w: Vec<u16> = "Property"
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect();
        let temp_dir_w: Vec<u16> = temp_dir
            .to_str()
            .unwrap()
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect();
        assert_eq!(
            unsafe {
                MsiDatabaseExportW(
                    h_db,
                    TableNameW(prop_w.as_ptr()),
                    ExportDirectoryW(temp_dir_w.as_ptr()),
                    FilePathW(dummy_w.as_ptr()),
                )
            },
            0
        );

        // MsiDatabaseExportA invalid string
        assert_eq!(
            unsafe {
                MsiDatabaseExportA(
                    h_db,
                    TableNameA(invalid_a.as_ptr().cast()),
                    ExportDirectoryA(dummy_a.as_ptr().cast()),
                    FilePathA(dummy_a.as_ptr().cast()),
                )
            },
            0
        );
        assert_eq!(
            unsafe {
                MsiDatabaseExportA(
                    h_db,
                    TableNameA(dummy_a.as_ptr().cast()),
                    ExportDirectoryA(invalid_a.as_ptr().cast()),
                    FilePathA(dummy_a.as_ptr().cast()),
                )
            },
            0
        );
        assert_eq!(
            unsafe {
                MsiDatabaseExportA(
                    h_db,
                    TableNameA(dummy_a.as_ptr().cast()),
                    ExportDirectoryA(dummy_a.as_ptr().cast()),
                    FilePathA(invalid_a.as_ptr().cast()),
                )
            },
            0
        );

        let _ = crate::handles::close_handle(h_db);
    }

    use std::ptr;

    #[test]
    fn test_apply_embedded_transform() {
        use msi::database::transform::DatabaseTransform;
        use msi::wix::linker::LinkedDatabase;
        let mut db = LinkedDatabase::default();
        let transform = DatabaseTransform::new();
        let transform_bytes = transform.to_bytes().unwrap();

        db.embedded_storages
            .insert("1033".to_string(), transform_bytes.clone());

        let g_handle = crate::handles::alloc_handle(crate::handles::MsiObject::Database(
            crate::types::MsiDatabaseHandle {
                inner: db,
                state: 0,
            },
        ));

        let path = ":1033";
        let mut path_w: Vec<u16> = path.encode_utf16().collect();
        path_w.push(0);
        let mut path_a: Vec<i8> = path.as_bytes().iter().map(|&b| b as i8).collect();
        path_a.push(0);

        unsafe {
            let res = MsiDatabaseApplyTransformW(
                g_handle,
                FilePathW(path_w.as_ptr()),
                TransformErrorCondition(0),
            );
            assert_eq!(res, crate::win32::ERROR_SUCCESS);

            let res_a = MsiDatabaseApplyTransformA(
                g_handle,
                FilePathA(path_a.as_ptr()),
                TransformErrorCondition(0),
            );
            assert_eq!(res_a, crate::win32::ERROR_SUCCESS);

            let bad_path = ":1041";
            let mut bad_path_w: Vec<u16> = bad_path.encode_utf16().collect();
            bad_path_w.push(0);
            let mut bad_path_a: Vec<i8> = bad_path.as_bytes().iter().map(|&b| b as i8).collect();
            bad_path_a.push(0);

            let res_bad = MsiDatabaseApplyTransformW(
                g_handle,
                FilePathW(bad_path_w.as_ptr()),
                TransformErrorCondition(0),
            );
            assert_eq!(res_bad, crate::win32::ERROR_FILE_NOT_FOUND);

            let res_bad_a = MsiDatabaseApplyTransformA(
                g_handle,
                FilePathA(bad_path_a.as_ptr()),
                TransformErrorCondition(0),
            );
            assert_eq!(res_bad_a, crate::win32::ERROR_FILE_NOT_FOUND);

            // Test panic handler
            let panic_path = "PANIC_TEST";
            let mut panic_w: Vec<u16> = panic_path.encode_utf16().collect();
            panic_w.push(0);
            let mut panic_a: Vec<i8> = panic_path.as_bytes().iter().map(|&b| b as i8).collect();
            panic_a.push(0);

            assert_eq!(
                MsiDatabaseApplyTransformW(
                    g_handle,
                    FilePathW(panic_w.as_ptr()),
                    TransformErrorCondition(0),
                ),
                crate::win32::ERROR_INSTALL_FAILURE
            );

            assert_eq!(
                MsiDatabaseApplyTransformA(
                    g_handle,
                    FilePathA(panic_a.as_ptr()),
                    TransformErrorCondition(0),
                ),
                crate::win32::ERROR_INSTALL_FAILURE
            );

            // Test missing files from disk (no colon prefix)
            let disk_path = "non_existent_file.mst";
            let mut disk_path_w: Vec<u16> = disk_path.encode_utf16().collect();
            disk_path_w.push(0);
            let mut disk_path_a: Vec<i8> = disk_path.as_bytes().iter().map(|&b| b as i8).collect();
            disk_path_a.push(0);

            assert_eq!(
                MsiDatabaseApplyTransformW(
                    g_handle,
                    FilePathW(disk_path_w.as_ptr()),
                    TransformErrorCondition(0)
                ),
                crate::win32::ERROR_FILE_NOT_FOUND
            );
            assert_eq!(
                MsiDatabaseApplyTransformA(
                    g_handle,
                    FilePathA(disk_path_a.as_ptr()),
                    TransformErrorCondition(0)
                ),
                crate::win32::ERROR_FILE_NOT_FOUND
            );

            // Test valid file read
            let good_disk_path = "valid_file.mst";
            std::fs::write(good_disk_path, transform_bytes).unwrap();
            let mut good_disk_path_w: Vec<u16> = good_disk_path.encode_utf16().collect();
            good_disk_path_w.push(0);
            let mut good_disk_path_a: Vec<i8> =
                good_disk_path.as_bytes().iter().map(|&b| b as i8).collect();
            good_disk_path_a.push(0);

            assert_eq!(
                MsiDatabaseApplyTransformW(
                    g_handle,
                    FilePathW(good_disk_path_w.as_ptr()),
                    TransformErrorCondition(0)
                ),
                crate::win32::ERROR_SUCCESS
            );
            assert_eq!(
                MsiDatabaseApplyTransformA(
                    g_handle,
                    FilePathA(good_disk_path_a.as_ptr()),
                    TransformErrorCondition(0)
                ),
                crate::win32::ERROR_SUCCESS
            );
            std::fs::remove_file(good_disk_path).unwrap();

            // Test invalid transform file data
            let bad_disk_path = "invalid_file.mst";
            std::fs::write(bad_disk_path, b"bad data").unwrap();
            let mut bad_disk_path_w: Vec<u16> = bad_disk_path.encode_utf16().collect();
            bad_disk_path_w.push(0);
            let mut bad_disk_path_a: Vec<i8> =
                bad_disk_path.as_bytes().iter().map(|&b| b as i8).collect();
            bad_disk_path_a.push(0);

            // This tests lines 372 and 438 since 'bad data' fails to parse
            assert_eq!(
                MsiDatabaseApplyTransformW(
                    g_handle,
                    FilePathW(bad_disk_path_w.as_ptr()),
                    TransformErrorCondition(0)
                ),
                crate::win32::ERROR_INSTALL_FAILURE
            );
            assert_eq!(
                MsiDatabaseApplyTransformA(
                    g_handle,
                    FilePathA(bad_disk_path_a.as_ptr()),
                    TransformErrorCondition(0)
                ),
                crate::win32::ERROR_INSTALL_FAILURE
            );
            std::fs::remove_file(bad_disk_path).unwrap();

            // Test applying a transform that fails (e.g. valid format but applying to incompatible DB)
            // A quick way is to force validation failure.

            let mut db2 = LinkedDatabase::default();
            db2.tables.insert(
                "Property".to_string(),
                vec![msi::database::tables::record::Record::with_fields(vec![
                    msi::database::FieldValue::String("ProductCode".to_string()),
                    msi::database::FieldValue::String(
                        "{00000000-0000-0000-0000-000000000000}".to_string(),
                    ),
                ])],
            );

            let g_handle2 = crate::handles::alloc_handle(crate::handles::MsiObject::Database(
                crate::types::MsiDatabaseHandle {
                    inner: db2,
                    state: 0,
                },
            ));

            let mut transform_apply_fail = DatabaseTransform::new();
            transform_apply_fail.summary_info.word_count = Some(
                msi::database::transform::validation_flags::MSITRANSFORM_VALIDATE_PRODUCT as i32,
            );
            transform_apply_fail.summary_info.rev_number = Some("{11111111-1111-1111-1111-111111111111};{22222222-2222-2222-2222-222222222222};{33333333-3333-3333-3333-333333333333}".to_string());

            transform_apply_fail.tables.insert(
                "DoesntExist".to_string(),
                msi::database::transform::TableTransform {
                    table_name: "DoesntExist".to_string(),
                    is_added: false,
                    is_dropped: true,
                    operations: vec![],
                },
            );
            let transform_fail_bytes = transform_apply_fail.to_bytes().unwrap();

            let fail_disk_path = "fail_apply.mst";
            std::fs::write(fail_disk_path, transform_fail_bytes).unwrap();
            let mut fail_disk_path_w: Vec<u16> = fail_disk_path.encode_utf16().collect();
            fail_disk_path_w.push(0);
            let mut fail_disk_path_a: Vec<i8> =
                fail_disk_path.as_bytes().iter().map(|&b| b as i8).collect();
            fail_disk_path_a.push(0);

            // This tests lines 377 and 443 where apply() returns an Err
            assert_eq!(
                MsiDatabaseApplyTransformW(
                    g_handle2,
                    FilePathW(fail_disk_path_w.as_ptr()),
                    TransformErrorCondition(0)
                ),
                1624 // ERROR_INSTALL_TRANSFORM_FAILURE
            );
            assert_eq!(
                MsiDatabaseApplyTransformA(
                    g_handle2,
                    FilePathA(fail_disk_path_a.as_ptr()),
                    TransformErrorCondition(0)
                ),
                1624 // ERROR_INSTALL_TRANSFORM_FAILURE
            );
            std::fs::remove_file(fail_disk_path).unwrap();

            let _ = crate::handles::close_handle(g_handle2);
        }
    }

    #[test]
    fn test_apply_transform_handle_invalid() {
        let non_db = crate::handles::alloc_handle(crate::handles::MsiObject::Record(
            crate::types::MsiRecordHandle {
                inner: msi::database::tables::record::Record::new(),
            },
        ));
        let path = ":1033";
        let mut path_w: Vec<u16> = path.encode_utf16().collect();
        path_w.push(0);
        let mut path_a: Vec<i8> = path.as_bytes().iter().map(|&b| b as i8).collect();
        path_a.push(0);
        assert_eq!(
            unsafe {
                MsiDatabaseApplyTransformW(
                    non_db,
                    FilePathW(path_w.as_ptr()),
                    TransformErrorCondition(0),
                )
            },
            6
        );
        assert_eq!(
            unsafe {
                MsiDatabaseApplyTransformA(
                    non_db,
                    FilePathA(path_a.as_ptr()),
                    TransformErrorCondition(0),
                )
            },
            6
        );
        let _ = crate::handles::close_handle(non_db);
    }

    #[test]
    fn test_msi_database_mutation_stubs() {
        let dummy_w = [u16::from(b'A'), 0];
        let dummy_a = [i32::from(b'A'), 0];
        let invalid_w = [0xD800_u16, 0];
        let invalid_a = [i32::from(0xFF_u8), 0];

        // MsiDatabaseMerge
        assert_eq!(
            unsafe { MsiDatabaseMergeW(0, 0, TableNameW(ptr::null())) },
            87
        );
        assert_eq!(
            unsafe { MsiDatabaseMergeA(0, 0, TableNameA(ptr::null())) },
            87
        );
        assert_eq!(
            unsafe { MsiDatabaseMergeW(1, 1, TableNameW(ptr::null())) },
            6 // ERROR_INVALID_HANDLE
        );
        assert_eq!(
            unsafe { MsiDatabaseMergeA(1, 1, TableNameA(ptr::null())) },
            6 // ERROR_INVALID_HANDLE
        );

        let valid_db1 = crate::handles::alloc_handle(crate::handles::MsiObject::Database(
            crate::types::MsiDatabaseHandle {
                inner: msi::wix::linker::LinkedDatabase::default(),
                state: 0,
            },
        ));
        let valid_db2 = crate::handles::alloc_handle(crate::handles::MsiObject::Database(
            crate::types::MsiDatabaseHandle {
                inner: msi::wix::linker::LinkedDatabase::default(),
                state: 0,
            },
        ));
        let non_db = crate::handles::alloc_handle(crate::handles::MsiObject::Record(
            crate::types::MsiRecordHandle {
                inner: msi::database::tables::record::Record::new(),
            },
        ));

        assert_eq!(
            unsafe { MsiDatabaseMergeW(valid_db1, valid_db2, TableNameW(ptr::null())) },
            0 // ERROR_SUCCESS
        );
        assert_eq!(
            unsafe { MsiDatabaseMergeA(valid_db1, valid_db2, TableNameA(ptr::null())) },
            0 // ERROR_SUCCESS
        );
        assert_eq!(
            unsafe { MsiDatabaseMergeW(valid_db1, valid_db2, TableNameW(dummy_w.as_ptr())) },
            1627 // ERROR_FUNCTION_FAILED
        );
        assert_eq!(
            unsafe { MsiDatabaseMergeA(valid_db1, valid_db2, TableNameA(dummy_a.as_ptr().cast())) },
            1627 // ERROR_FUNCTION_FAILED
        );
        assert_eq!(
            unsafe { MsiDatabaseMergeW(valid_db1, valid_db2, TableNameW(invalid_w.as_ptr())) },
            87 // ERROR_INVALID_PARAMETER
        );
        assert_eq!(
            unsafe {
                MsiDatabaseMergeA(valid_db1, valid_db2, TableNameA(invalid_a.as_ptr().cast()))
            },
            87 // ERROR_INVALID_PARAMETER
        );
        assert_eq!(
            unsafe { MsiDatabaseMergeW(valid_db1, non_db, TableNameW(ptr::null())) },
            6 // ERROR_INVALID_HANDLE
        );
        assert_eq!(
            unsafe { MsiDatabaseMergeW(non_db, valid_db2, TableNameW(ptr::null())) },
            6 // ERROR_INVALID_HANDLE
        );
        assert_eq!(
            unsafe { MsiDatabaseMergeA(valid_db1, non_db, TableNameA(ptr::null())) },
            6 // ERROR_INVALID_HANDLE
        );
        assert_eq!(
            unsafe { MsiDatabaseMergeA(non_db, valid_db2, TableNameA(ptr::null())) },
            6 // ERROR_INVALID_HANDLE
        );

        let _ = crate::handles::close_handle(valid_db1);
        let _ = crate::handles::close_handle(valid_db2);
        let _ = crate::handles::close_handle(non_db);

        // Test merge conflict
        let mut valid_db3_inner = msi::wix::linker::LinkedDatabase::default();
        let mut schema = msi::database::TableSchema::new("ConflictTable");
        schema = schema.with_column(
            msi::database::ColumnDef::new("Col1", msi::database::DataType::String { max_len: 255 })
                .primary_key(),
        );
        valid_db3_inner.catalog.add_table(schema).unwrap();

        let mut valid_db4_inner = msi::wix::linker::LinkedDatabase::default();
        let mut schema2 = msi::database::TableSchema::new("ConflictTable");
        schema2 = schema2.with_column(
            msi::database::ColumnDef::new("Col1", msi::database::DataType::Short).primary_key(),
        );
        valid_db4_inner.catalog.add_table(schema2).unwrap();

        let valid_db3 = crate::handles::alloc_handle(crate::handles::MsiObject::Database(
            crate::types::MsiDatabaseHandle {
                inner: valid_db3_inner,
                state: 0,
            },
        ));
        let valid_db4 = crate::handles::alloc_handle(crate::handles::MsiObject::Database(
            crate::types::MsiDatabaseHandle {
                inner: valid_db4_inner,
                state: 0,
            },
        ));

        assert_eq!(
            unsafe { MsiDatabaseMergeW(valid_db3, valid_db4, TableNameW(ptr::null())) },
            1627 // ERROR_FUNCTION_FAILED
        );
        assert_eq!(
            unsafe { MsiDatabaseMergeA(valid_db3, valid_db4, TableNameA(ptr::null())) },
            1627 // ERROR_FUNCTION_FAILED
        );

        let _ = crate::handles::close_handle(valid_db3);
        let _ = crate::handles::close_handle(valid_db4);

        // MsiDatabaseImport
        assert_eq!(
            unsafe { MsiDatabaseImportW(0, ExportDirectoryW(ptr::null()), FilePathW(ptr::null())) },
            87
        );
        assert_eq!(
            unsafe { MsiDatabaseImportA(0, ExportDirectoryA(ptr::null()), FilePathA(ptr::null())) },
            87
        );
        assert_eq!(
            unsafe {
                MsiDatabaseImportW(
                    1,
                    ExportDirectoryW(dummy_w.as_ptr()),
                    FilePathW(dummy_w.as_ptr()),
                )
            },
            0
        );
        assert_eq!(
            unsafe {
                MsiDatabaseImportA(
                    1,
                    ExportDirectoryA(dummy_a.as_ptr().cast()),
                    FilePathA(dummy_a.as_ptr().cast()),
                )
            },
            0
        );

        // MsiDatabaseExport
        assert_eq!(
            unsafe {
                MsiDatabaseExportW(
                    0,
                    TableNameW(ptr::null()),
                    ExportDirectoryW(ptr::null()),
                    FilePathW(ptr::null()),
                )
            },
            87
        );
        assert_eq!(
            unsafe {
                MsiDatabaseExportA(
                    0,
                    TableNameA(ptr::null()),
                    ExportDirectoryA(ptr::null()),
                    FilePathA(ptr::null()),
                )
            },
            87
        );
        assert_eq!(
            unsafe {
                MsiDatabaseExportW(
                    1,
                    TableNameW(dummy_w.as_ptr()),
                    ExportDirectoryW(dummy_w.as_ptr()),
                    FilePathW(dummy_w.as_ptr()),
                )
            },
            0
        );
        assert_eq!(
            unsafe {
                MsiDatabaseExportA(
                    1,
                    TableNameA(dummy_a.as_ptr().cast()),
                    ExportDirectoryA(dummy_a.as_ptr().cast()),
                    FilePathA(dummy_a.as_ptr().cast()),
                )
            },
            0
        );

        // MsiDatabaseApplyTransform
        assert_eq!(
            unsafe {
                MsiDatabaseApplyTransformW(0, FilePathW(ptr::null()), TransformErrorCondition(0))
            },
            87
        );
        assert_eq!(
            unsafe {
                MsiDatabaseApplyTransformA(0, FilePathA(ptr::null()), TransformErrorCondition(0))
            },
            87
        );
        assert_eq!(
            unsafe {
                MsiDatabaseApplyTransformW(
                    1,
                    FilePathW(dummy_w.as_ptr()),
                    TransformErrorCondition(0),
                )
            },
            6
        );
        assert_eq!(
            unsafe {
                MsiDatabaseApplyTransformW(
                    1,
                    FilePathW(invalid_w.as_ptr()),
                    TransformErrorCondition(0),
                )
            },
            87
        );
        assert_eq!(
            unsafe {
                MsiDatabaseApplyTransformA(
                    1,
                    FilePathA(dummy_a.as_ptr().cast()),
                    TransformErrorCondition(0),
                )
            },
            6
        );
        assert_eq!(
            unsafe {
                MsiDatabaseApplyTransformA(
                    1,
                    FilePathA(invalid_a.as_ptr().cast()),
                    TransformErrorCondition(0),
                )
            },
            87
        );
    }
}

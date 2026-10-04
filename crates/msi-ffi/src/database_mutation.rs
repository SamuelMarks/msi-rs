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
#[no_mangle]
pub unsafe extern "system" fn MsiDatabaseMergeW(
    _h_database: MSIHANDLE,
    _h_database_merge: MSIHANDLE,
    _sz_table_name: TableNameW,
) -> u32 {
    std::panic::catch_unwind(|| {
        if _h_database == 0 || _h_database_merge == 0 {
            return 87; // ERROR_INVALID_PARAMETER
        }
        120 // ERROR_CALL_NOT_IMPLEMENTED
    })
    .unwrap_or(1603)
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
/// Returns `120` (`ERROR_CALL_NOT_IMPLEMENTED`) as this is currently a stub.
///
/// # Returns
///
/// Win32 status code.
///
/// # Safety
///
/// Pointers must be valid or null.
#[no_mangle]
pub unsafe extern "system" fn MsiDatabaseMergeA(
    _h_database: MSIHANDLE,
    _h_database_merge: MSIHANDLE,
    _sz_table_name: TableNameA,
) -> u32 {
    std::panic::catch_unwind(|| {
        if _h_database == 0 || _h_database_merge == 0 {
            return 87;
        }
        120
    })
    .unwrap_or(1603)
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
#[no_mangle]
pub unsafe extern "system" fn MsiDatabaseImportW(
    _h_database: MSIHANDLE,
    _sz_folder_path: ExportDirectoryW,
    _sz_file_name: FilePathW,
) -> u32 {
    std::panic::catch_unwind(|| {
        if _h_database == 0 || _sz_folder_path.0.is_null() || _sz_file_name.0.is_null() {
            return 87;
        }
        120
    })
    .unwrap_or(1603)
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
#[no_mangle]
pub unsafe extern "system" fn MsiDatabaseImportA(
    _h_database: MSIHANDLE,
    _sz_folder_path: ExportDirectoryA,
    _sz_file_name: FilePathA,
) -> u32 {
    std::panic::catch_unwind(|| {
        if _h_database == 0 || _sz_folder_path.0.is_null() || _sz_file_name.0.is_null() {
            return 87;
        }
        120
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
/// Returns `120` (`ERROR_CALL_NOT_IMPLEMENTED`) as this is currently a stub.
///
/// # Returns
///
/// Win32 status code.
///
/// # Safety
///
/// Pointers must be valid or null.
#[no_mangle]
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
            return 87;
        }
        120
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
/// Returns `120` (`ERROR_CALL_NOT_IMPLEMENTED`) as this is currently a stub.
///
/// # Returns
///
/// Win32 status code.
///
/// # Safety
///
/// Pointers must be valid or null.
#[no_mangle]
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
            return 87;
        }
        120
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
#[no_mangle]
pub unsafe extern "system" fn MsiDatabaseApplyTransformW(
    _h_database: MSIHANDLE,
    _sz_transform_file: FilePathW,
    _i_error_conditions: TransformErrorCondition,
) -> u32 {
    std::panic::catch_unwind(|| {
        if _h_database == 0 || _sz_transform_file.0.is_null() {
            return 87;
        }
        120
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
#[no_mangle]
pub unsafe extern "system" fn MsiDatabaseApplyTransformA(
    _h_database: MSIHANDLE,
    _sz_transform_file: FilePathA,
    _i_error_conditions: TransformErrorCondition,
) -> u32 {
    std::panic::catch_unwind(|| {
        if _h_database == 0 || _sz_transform_file.0.is_null() {
            return 87;
        }
        120
    })
    .unwrap_or(1603)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ptr;

    #[test]
    fn test_msi_database_mutation_stubs() {
        let dummy_w = [0_u16; 1];
        let dummy_a = [0_i8; 1];

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
            120
        );
        assert_eq!(
            unsafe { MsiDatabaseMergeA(1, 1, TableNameA(ptr::null())) },
            120
        );

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
            120
        );
        assert_eq!(
            unsafe {
                MsiDatabaseImportA(
                    1,
                    ExportDirectoryA(dummy_a.as_ptr().cast()),
                    FilePathA(dummy_a.as_ptr().cast()),
                )
            },
            120
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
            120
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
            120
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
            120
        );
        assert_eq!(
            unsafe {
                MsiDatabaseApplyTransformA(
                    1,
                    FilePathA(dummy_a.as_ptr().cast()),
                    TransformErrorCondition(0),
                )
            },
            120
        );
    }
}

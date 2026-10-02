//! Summary Information and Database Commit Win32 `stdcall` API endpoints.
//!
//! Provides `MsiGetSummaryInformation`, `MsiSummaryInfoGetProperty`,
//! `MsiSummaryInfoSetProperty`, and `MsiDatabaseCommit`.

use std::panic;

use crate::handles::{with_handle_mut, MsiHandle, MsiObject};
use crate::win32::{
    Dword, Lpcstr, Lpcwstr, Lpstr, Lpwstr, Uint, ERROR_INSTALL_FAILURE, ERROR_INVALID_HANDLE,
    ERROR_INVALID_PARAMETER, ERROR_SUCCESS,
};

// We don't have a SummaryInfo handle type yet, but we stub the endpoints.

/// Obtains a handle for the _`SummaryInformation` stream for a Windows Installer database (Unicode).
#[no_mangle]
#[allow(non_snake_case, unused_variables)]
#[allow(clippy::not_unsafe_ptr_arg_deref)]
pub extern "system" fn MsiGetSummaryInformationW(
    hDatabase: MsiHandle,
    szDatabasePath: Lpcwstr,
    uiUpdateCount: Uint,
    phSummaryInfo: *mut MsiHandle,
) -> Uint {
    let result = panic::catch_unwind(|| {
        if phSummaryInfo.is_null() {
            return ERROR_INVALID_PARAMETER;
        }

        unsafe {
            *phSummaryInfo = 0;
        } // stub
        ERROR_SUCCESS
    });

    result.unwrap_or(ERROR_INSTALL_FAILURE)
}

/// Obtains a handle for the _`SummaryInformation` stream for a Windows Installer database (ANSI).
#[no_mangle]
#[allow(non_snake_case, unused_variables)]
#[allow(clippy::not_unsafe_ptr_arg_deref)]
pub extern "system" fn MsiGetSummaryInformationA(
    hDatabase: MsiHandle,
    szDatabasePath: Lpcstr,
    uiUpdateCount: Uint,
    phSummaryInfo: *mut MsiHandle,
) -> Uint {
    let result = panic::catch_unwind(|| {
        if phSummaryInfo.is_null() {
            return ERROR_INVALID_PARAMETER;
        }

        unsafe {
            *phSummaryInfo = 0;
        } // stub
        ERROR_SUCCESS
    });

    result.unwrap_or(ERROR_INSTALL_FAILURE)
}

/// Gets a single property from the summary information stream (Unicode).
#[no_mangle]
#[allow(non_snake_case, unused_variables)]
pub extern "system" fn MsiSummaryInfoGetPropertyW(
    hSummaryInfo: MsiHandle,
    uiProperty: Uint,
    puiDataType: *mut Uint,
    piValue: *mut i32,
    pftValue: *mut std::ffi::c_void, // FILETIME*
    szValueBuf: Lpwstr,
    pcchValueBuf: *mut Dword,
) -> Uint {
    let result = panic::catch_unwind(|| ERROR_INVALID_HANDLE);

    result.unwrap_or(ERROR_INSTALL_FAILURE)
}

/// Gets a single property from the summary information stream (ANSI).
#[no_mangle]
#[allow(non_snake_case, unused_variables)]
pub extern "system" fn MsiSummaryInfoGetPropertyA(
    hSummaryInfo: MsiHandle,
    uiProperty: Uint,
    puiDataType: *mut Uint,
    piValue: *mut i32,
    pftValue: *mut std::ffi::c_void, // FILETIME*
    szValueBuf: Lpstr,
    pcchValueBuf: *mut Dword,
) -> Uint {
    let result = panic::catch_unwind(|| ERROR_INVALID_HANDLE);

    result.unwrap_or(ERROR_INSTALL_FAILURE)
}

/// Sets a single property in the summary information stream (Unicode).
#[no_mangle]
#[allow(non_snake_case, unused_variables)]
pub extern "system" fn MsiSummaryInfoSetPropertyW(
    hSummaryInfo: MsiHandle,
    uiProperty: Uint,
    uiDataType: Uint,
    iValue: i32,
    pftValue: *mut std::ffi::c_void, // FILETIME*
    szValue: Lpcwstr,
) -> Uint {
    let result = panic::catch_unwind(|| ERROR_INVALID_HANDLE);

    result.unwrap_or(ERROR_INSTALL_FAILURE)
}

/// Sets a single property in the summary information stream (ANSI).
#[no_mangle]
#[allow(non_snake_case, unused_variables)]
pub extern "system" fn MsiSummaryInfoSetPropertyA(
    hSummaryInfo: MsiHandle,
    uiProperty: Uint,
    uiDataType: Uint,
    iValue: i32,
    pftValue: *mut std::ffi::c_void, // FILETIME*
    szValue: Lpcstr,
) -> Uint {
    let result = panic::catch_unwind(|| ERROR_INVALID_HANDLE);

    result.unwrap_or(ERROR_INSTALL_FAILURE)
}

/// Commits changes to a database.
///
/// # Arguments
///
/// * `hDatabase` - The database handle.
///
/// # Returns
///
/// `ERROR_SUCCESS`, `ERROR_INVALID_HANDLE`
#[no_mangle]
#[allow(non_snake_case, unused_variables)]
pub extern "system" fn MsiDatabaseCommit(hDatabase: MsiHandle) -> Uint {
    let result = panic::catch_unwind(|| {
        with_handle_mut(hDatabase, |obj| {
            if let MsiObject::Database(_db_handle) = obj {
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
mod tests {
    use super::*;
    use crate::handles::{alloc_handle, MsiObject};

    #[test]
    fn test_summary_stubs() {
        let mut h = 1;
        assert_eq!(
            MsiGetSummaryInformationW(0, std::ptr::null(), 0, &raw mut h),
            ERROR_SUCCESS
        );
        assert_eq!(
            MsiGetSummaryInformationA(0, std::ptr::null(), 0, &raw mut h),
            ERROR_SUCCESS
        );
        assert_eq!(MsiDatabaseCommit(0), ERROR_INVALID_HANDLE);

        assert_eq!(
            MsiSummaryInfoGetPropertyW(
                0,
                0,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut()
            ),
            ERROR_INVALID_HANDLE
        );
        assert_eq!(
            MsiSummaryInfoGetPropertyA(
                0,
                0,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut()
            ),
            ERROR_INVALID_HANDLE
        );

        assert_eq!(
            MsiGetSummaryInformationW(0, std::ptr::null(), 0, std::ptr::null_mut()),
            ERROR_INVALID_PARAMETER
        );
        assert_eq!(
            MsiGetSummaryInformationA(0, std::ptr::null(), 0, std::ptr::null_mut()),
            ERROR_INVALID_PARAMETER
        );
        assert_eq!(
            MsiSummaryInfoSetPropertyW(0, 0, 0, 0, std::ptr::null_mut(), std::ptr::null()),
            ERROR_INVALID_HANDLE
        );
        assert_eq!(
            MsiSummaryInfoSetPropertyA(0, 0, 0, 0, std::ptr::null_mut(), std::ptr::null()),
            ERROR_INVALID_HANDLE
        );

        let db = msi::wix::linker::LinkedDatabase::new().unwrap();
        let h_db = alloc_handle(MsiObject::Database(crate::types::MsiDatabaseHandle {
            inner: db,
        }));
        assert_eq!(MsiDatabaseCommit(h_db), ERROR_SUCCESS);

        let rec = msi::database::tables::record::Record::new();
        let h_rec = alloc_handle(MsiObject::Record(crate::types::MsiRecordHandle {
            inner: rec,
        }));
        assert_eq!(MsiDatabaseCommit(h_rec), ERROR_INVALID_HANDLE);
    }
}

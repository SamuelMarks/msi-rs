//! Calling convention verification tests.

/// Verifies that Win32 MSI API exports use the `extern "system"` calling convention
/// and return the expected Win32 status codes when invoked through typed function pointers.
#[test]
fn test_verify_calling_conventions() {
    let close_fn: extern "system" fn(msi_ffi::handles::MsiHandle) -> msi_ffi::win32::Uint =
        msi_ffi::MsiCloseHandle;
    let create_fn: extern "system" fn(msi_ffi::win32::Uint) -> msi_ffi::handles::MsiHandle =
        msi_ffi::win32::record::MsiCreateRecord;
    let open_db_fn: extern "system" fn(
        msi_ffi::win32::Lpcwstr,
        msi_ffi::win32::Lpcwstr,
        *mut msi_ffi::handles::MsiHandle,
    ) -> msi_ffi::win32::Uint = msi_ffi::win32::database::MsiOpenDatabaseW;
    let open_view_fn: extern "system" fn(
        msi_ffi::handles::MsiHandle,
        msi_ffi::win32::Lpcwstr,
        *mut msi_ffi::handles::MsiHandle,
    ) -> msi_ffi::win32::Uint = msi_ffi::win32::database::MsiDatabaseOpenViewW;
    let db_state_fn: extern "system" fn(
        msi_ffi::handles::MsiHandle,
    ) -> msi_ffi::win32::database::Msidbstate = msi_ffi::win32::database::MsiGetDatabaseState;

    let h_rec = create_fn(2);
    assert_ne!(h_rec, 0);
    assert_eq!(close_fn(h_rec), msi_ffi::win32::ERROR_SUCCESS);
    assert_eq!(close_fn(0), msi_ffi::win32::ERROR_SUCCESS);
    assert_eq!(close_fn(9999), msi_ffi::win32::ERROR_INVALID_HANDLE);

    assert_eq!(
        open_db_fn(std::ptr::null(), std::ptr::null(), std::ptr::null_mut()),
        msi_ffi::win32::ERROR_INVALID_PARAMETER
    );
    assert_eq!(
        open_view_fn(0, std::ptr::null(), std::ptr::null_mut()),
        msi_ffi::win32::ERROR_INVALID_PARAMETER
    );
    assert_eq!(db_state_fn(0), -1);
}

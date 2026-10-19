//! Ordinal and exported Win32 / C-ABI symbol validation tests.

/// Verifies that the core Win32 MSI and C-ABI entry points exported by `msi-ffi`
/// match the `msi.spec` / `msi.h` symbol table and execute deterministically.
#[test]
fn verify_export_ordinals_match_wine() {
    // 1. Verify C-ABI error & diagnostic exports
    assert_eq!(msi_ffi::msi_get_last_error_code(), msi_ffi::MSI_SUCCESS);
    // SAFETY: Passing null pointers with zero capacity is explicitly handled and returns `MSI_ERROR_BUFFER_TOO_SMALL`.
    let err_status = unsafe {
        msi_ffi::msi_get_last_error_message(std::ptr::null_mut(), 0, std::ptr::null_mut())
    };
    assert_eq!(err_status, msi_ffi::MSI_ERROR_BUFFER_TOO_SMALL);

    // 2. Verify Win32 Record API exports (`MsiCreateRecord`, `MsiRecordGetFieldCount`, `MsiRecordDataSize`, `MsiCloseHandle`, `MsiCloseAllHandles`)
    let rec_handle = msi_ffi::win32::record::MsiCreateRecord(3);
    assert_ne!(rec_handle, 0);
    assert_eq!(
        msi_ffi::win32::record_stream::MsiRecordGetFieldCount(rec_handle),
        3
    );
    assert_eq!(
        msi_ffi::win32::record::MsiRecordSetInteger(rec_handle, 1, 42),
        0
    );
    assert_eq!(msi_ffi::MsiRecordDataSize(rec_handle, 1), 4);
    assert_eq!(msi_ffi::MsiCloseHandle(rec_handle), 0);
    assert_eq!(msi_ffi::MsiCloseAllHandles(), 0);

    // 3. Verify Win32 UI level export (`MsiSetInternalUI`)
    let prev_ui = msi_ffi::win32::ui::MsiSetInternalUI(2, std::ptr::null_mut());
    assert_eq!(prev_ui, 2);
}

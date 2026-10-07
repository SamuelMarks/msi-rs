//! Integration tests for FFI handles allocation
#[test]
fn test_ffi_handle_allocation() {
    let result = msi_ffi::MsiCloseAllHandles();
    assert_eq!(result, 0); // ERROR_SUCCESS is 0
}

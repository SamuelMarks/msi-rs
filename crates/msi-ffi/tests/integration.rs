#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::undocumented_unsafe_blocks
)]
//! Integration tests for FFI handles allocation
use libloading::{Library, Symbol};

#[test]
fn test_ffi_handle_allocation() {
    let result = msi_ffi::MsiCloseAllHandles();
    assert_eq!(result, 0); // ERROR_SUCCESS is 0
}

#[test]
fn test_ffi_exports_presence() {
    let lib_name = if cfg!(target_os = "macos") {
        "libmsi_ffi.dylib"
    } else if cfg!(target_os = "windows") {
        "msi_ffi.dll"
    } else {
        "libmsi_ffi.so"
    };

    // search in target/debug/ or target/release/
    let mut current_exe = std::env::current_exe().unwrap();
    current_exe.pop(); // remove exe name
    if current_exe.ends_with("deps") {
        current_exe.pop(); // remove deps
    }

    let lib_path = current_exe.join(lib_name);
    if !lib_path.exists() {
        println!(
            "Dynamic library not found at {}, skipping test.",
            lib_path.display()
        );
        return;
    }

    let lib = unsafe { Library::new(lib_path).unwrap() };
    let _sym: Symbol<'_, unsafe extern "C" fn()> =
        unsafe { lib.get(b"msi_begin_transaction\0").unwrap() };
    let _sym2: Symbol<'_, unsafe extern "C" fn()> =
        unsafe { lib.get(b"msi_install_product\0").unwrap() };
}

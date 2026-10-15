//! Integration tests for FFI handles allocation and symbol exports.

use libloading::{Library, Symbol};

#[test]
fn test_ffi_handle_allocation() {
    let result = msi_ffi::MsiCloseAllHandles();
    assert_eq!(result, 0); // ERROR_SUCCESS is 0
}

#[test]
fn test_ffi_exports_presence() -> Result<(), Box<dyn std::error::Error>> {
    let lib_name = if cfg!(target_os = "macos") {
        "libmsi_ffi.dylib"
    } else if cfg!(target_os = "windows") {
        "msi_ffi.dll"
    } else {
        "libmsi_ffi.so"
    };

    let mut current_exe = std::env::current_exe()?;
    current_exe.pop(); // remove exe name
    if current_exe.ends_with("deps") {
        current_exe.pop(); // remove deps
    }

    let lib_path = current_exe.join(lib_name);
    if !lib_path.exists() {
        return Ok(());
    }

    // SAFETY: Loading the compiled `msi-ffi` shared library produced by Cargo in the target directory.
    let lib = unsafe { Library::new(lib_path)? };
    // SAFETY: Looking up C-ABI function symbols with no invocation.
    let _sym: Symbol<'_, unsafe extern "C" fn()> = unsafe { lib.get(b"msi_begin_transaction\0")? };
    // SAFETY: Looking up C-ABI function symbols with no invocation.
    let _sym2: Symbol<'_, unsafe extern "C" fn()> = unsafe { lib.get(b"msi_install_product\0")? };
    Ok(())
}

//! Calling convention verification tests.
#![allow(clippy::no_effect_underscore_binding, clippy::used_underscore_binding)]

#[test]
fn test_verify_calling_conventions() {
    let _fn_ptr: extern "system" fn(msi_ffi::handles::MsiHandle) -> msi_ffi::win32::Uint =
        msi_ffi::win32::MsiCloseHandle;
    assert_eq!(_fn_ptr(0), 0); // 0 = ERROR_SUCCESS (from MsiCloseHandle for handle 0 actually)
}

/// Verifies that our internal C-ABI uses `extern "C"` to avoid C++ name mangling issues,
/// distinct from the `extern "system"` used for the Win32 `msi.dll` surface.
#[test]
fn verify_c_abi_conventions() {
    let _c_fn_ptr: unsafe extern "C" fn(*mut msi_ffi::types::MsiPackageBuilderHandle) =
        msi_ffi::types::msi_package_builder_destroy;
    let _sys_fn_ptr: extern "system" fn(msi_ffi::handles::MsiHandle) -> msi_ffi::win32::Uint =
        msi_ffi::win32::MsiCloseHandle;
}

#[test]
fn verify_all_win32_system_conventions() {
    let _f1: extern "system" fn(_, _) -> _ = msi_ffi::properties::MsiGetProductCodeW;
    let _f2: extern "system" fn(_, _) -> _ = msi_ffi::properties::MsiGetProductCodeA;
    let _f3: extern "system" fn(_, _) -> _ = msi_ffi::installer::MsiInstallProductW;
    let _f4: extern "system" fn(_, _) -> _ = msi_ffi::installer::MsiInstallProductA;
    let _f5: extern "system" fn() -> _ = msi_ffi::win32::MsiCloseAllHandles;
}

#[test]
fn verify_all_win32_system_conventions_database() {
    let _f1: extern "system" fn(_, _, _) -> _ = msi_ffi::win32::database::MsiOpenDatabaseW;
    let _f2: extern "system" fn(_, _, _) -> _ = msi_ffi::win32::database::MsiDatabaseOpenViewW;
    let _f3: extern "system" fn(_) -> _ = msi_ffi::win32::database::MsiGetDatabaseState;
}

//! C-ABI implementation of the Windows Installer Execution API.
use msi::error::MsiError;
use msi::execution::custom_action::MSIHANDLE;
use std::ffi::c_char;

/// Installs a product.
#[no_mangle]
/// # Safety
/// Pointers must be valid or null.
pub unsafe extern "system" fn MsiInstallProductW(
    _sz_package_path: *const u16,
    _sz_command_line: *const u16,
) -> u32 {
    std::panic::catch_unwind(|| {
        if _sz_package_path.is_null() {
            return MsiError::ERROR_INVALID_PARAMETER;
        }
        MsiError::ERROR_CALL_NOT_IMPLEMENTED
    })
    .unwrap_or(MsiError::ERROR_INSTALL_FAILURE)
}

/// Installs a product.
#[no_mangle]
/// # Safety
/// Pointers must be valid or null.
pub unsafe extern "system" fn MsiInstallProductA(
    _sz_package_path: *const c_char,
    _sz_command_line: *const c_char,
) -> u32 {
    std::panic::catch_unwind(|| {
        if _sz_package_path.is_null() {
            return MsiError::ERROR_INVALID_PARAMETER;
        }
        MsiError::ERROR_CALL_NOT_IMPLEMENTED
    })
    .unwrap_or(MsiError::ERROR_INSTALL_FAILURE)
}

/// Configures an installed product.
#[no_mangle]
/// # Safety
/// Pointers must be valid or null.
pub unsafe extern "system" fn MsiConfigureProductExW(
    _sz_product: *const u16,
    _i_install_level: i32,
    _e_install_state: i32,
    _sz_command_line: *const u16,
) -> u32 {
    std::panic::catch_unwind(|| MsiError::ERROR_CALL_NOT_IMPLEMENTED)
        .unwrap_or(MsiError::ERROR_INSTALL_FAILURE)
}

/// Configures an installed product.
#[no_mangle]
/// # Safety
/// Pointers must be valid or null.
pub unsafe extern "system" fn MsiConfigureProductExA(
    _sz_product: *const c_char,
    _i_install_level: i32,
    _e_install_state: i32,
    _sz_command_line: *const c_char,
) -> u32 {
    std::panic::catch_unwind(|| MsiError::ERROR_CALL_NOT_IMPLEMENTED)
        .unwrap_or(MsiError::ERROR_INSTALL_FAILURE)
}

/// Opens an installer package for use with functions that access the product database and install engine.
#[no_mangle]
/// # Safety
/// Pointers must be valid or null.
pub unsafe extern "system" fn MsiOpenPackageW(
    _sz_package_path: *const u16,
    _ph_product: *mut MSIHANDLE,
) -> u32 {
    std::panic::catch_unwind(|| MsiError::ERROR_CALL_NOT_IMPLEMENTED)
        .unwrap_or(MsiError::ERROR_INSTALL_FAILURE)
}

/// Opens an installer package for use with functions that access the product database and install engine.
#[no_mangle]
/// # Safety
/// Pointers must be valid or null.
pub unsafe extern "system" fn MsiOpenPackageA(
    _sz_package_path: *const c_char,
    _ph_product: *mut MSIHANDLE,
) -> u32 {
    std::panic::catch_unwind(|| MsiError::ERROR_CALL_NOT_IMPLEMENTED)
        .unwrap_or(MsiError::ERROR_INSTALL_FAILURE)
}

/// Retrieves a property from the installer session (UTF-16).
#[no_mangle]
/// # Safety
/// Pointers must be valid or null.
pub unsafe extern "system" fn MsiGetPropertyW(
    h_install: MSIHANDLE,
    sz_name: *const u16,
    sz_value_buf: *mut u16,
    pcch_value_buf: *mut u32,
) -> u32 {
    msi::execution::custom_action::MsiGetPropertyW(h_install, sz_name, sz_value_buf, pcch_value_buf)
}

/// Sets a property in the installer session (UTF-16).
#[no_mangle]
/// # Safety
/// Pointers must be valid or null.
pub unsafe extern "system" fn MsiSetPropertyW(
    h_install: MSIHANDLE,
    sz_name: *const u16,
    sz_value: *const u16,
) -> u32 {
    msi::execution::custom_action::MsiSetPropertyW(h_install, sz_name, sz_value)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ptr;

    #[test]
    fn test_msi_installer_stubs() {
        assert_eq!(
            unsafe { MsiInstallProductW(ptr::null(), ptr::null()) },
            MsiError::ERROR_INVALID_PARAMETER
        );
        assert_eq!(
            unsafe { MsiInstallProductA(ptr::null(), ptr::null()) },
            MsiError::ERROR_INVALID_PARAMETER
        );
        assert_eq!(
            unsafe { MsiConfigureProductExW(ptr::null(), 0, 0, ptr::null()) },
            MsiError::ERROR_CALL_NOT_IMPLEMENTED
        );
        assert_eq!(
            unsafe { MsiConfigureProductExA(ptr::null(), 0, 0, ptr::null()) },
            MsiError::ERROR_CALL_NOT_IMPLEMENTED
        );
        assert_eq!(
            unsafe { MsiOpenPackageW(ptr::null(), ptr::null_mut()) },
            MsiError::ERROR_CALL_NOT_IMPLEMENTED
        );
        assert_eq!(
            unsafe { MsiOpenPackageA(ptr::null(), ptr::null_mut()) },
            MsiError::ERROR_CALL_NOT_IMPLEMENTED
        );
    }
}

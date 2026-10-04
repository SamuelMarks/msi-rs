//! Installer and package state Win32 `stdcall` API endpoints.
//!
//! Provides `MsiOpenPackageA/W` and `MsiOpenProductA/W`.

use std::panic;

use crate::handles::{alloc_handle, MsiHandle, MsiObject};
use crate::types::MsiDatabaseHandle;
use crate::win32::strings::{lpcstr_to_string, lpcwstr_to_string};
use crate::win32::{
    Dword, Lpcstr, Lpcwstr, Uint, ERROR_INSTALL_FAILURE, ERROR_INVALID_PARAMETER, ERROR_SUCCESS,
};

/// Opens a Windows Installer package for use with functions that access the product database in Unicode (`W`).
///
/// # Arguments
///
/// * `szPackagePath` - The path to the package.
/// * `hProduct` - Pointer to a variable that receives the product handle.
///
/// # Returns
///
/// `ERROR_SUCCESS` or `ERROR_INVALID_PARAMETER`.
#[no_mangle]
#[allow(non_snake_case, unused_variables)]
#[allow(clippy::not_unsafe_ptr_arg_deref)]
pub extern "system" fn MsiOpenPackageW(szPackagePath: Lpcwstr, hProduct: *mut MsiHandle) -> Uint {
    let result = panic::catch_unwind(|| {
        if hProduct.is_null() {
            return ERROR_INVALID_PARAMETER;
        }

        let Some(_path) = lpcwstr_to_string(szPackagePath) else {
            return ERROR_INVALID_PARAMETER;
        };

        // In a complete implementation, this would parse the MS-CFB container at `_path`,
        // load the database, and create a handle.
        // For now, we return a mock database object to complete the ABI surface.
        let mock_db = msi::wix::linker::LinkedDatabase::default();
        let obj = MsiObject::Database(MsiDatabaseHandle { inner: mock_db });

        // SAFETY: We verified `hProduct` is not null.
        unsafe {
            *hProduct = alloc_handle(obj);
        }

        ERROR_SUCCESS
    });

    result.unwrap_or(ERROR_INSTALL_FAILURE)
}

/// Opens a Windows Installer package for use with functions that access the product database in ANSI (`A`).
///
/// # Arguments
///
/// * `szPackagePath` - The path to the package.
/// * `hProduct` - Pointer to a variable that receives the product handle.
///
/// # Returns
///
/// `ERROR_SUCCESS` or `ERROR_INVALID_PARAMETER`.
#[no_mangle]
#[allow(non_snake_case, unused_variables)]
#[allow(clippy::not_unsafe_ptr_arg_deref)]
pub extern "system" fn MsiOpenPackageA(szPackagePath: Lpcstr, hProduct: *mut MsiHandle) -> Uint {
    let result = panic::catch_unwind(|| {
        if hProduct.is_null() {
            return ERROR_INVALID_PARAMETER;
        }

        let Some(_path) = lpcstr_to_string(szPackagePath) else {
            return ERROR_INVALID_PARAMETER;
        };

        let mock_db = msi::wix::linker::LinkedDatabase::default();
        let obj = MsiObject::Database(MsiDatabaseHandle { inner: mock_db });

        // SAFETY: We verified `hProduct` is not null.
        unsafe {
            *hProduct = alloc_handle(obj);
        }

        ERROR_SUCCESS
    });

    result.unwrap_or(ERROR_INSTALL_FAILURE)
}

/// Opens a product for use with functions that access the product database in Unicode (`W`).
///
/// # Arguments
///
/// * `szProduct` - The `ProductCode` of the product to open.
/// * `hProduct` - Pointer to a variable that receives the product handle.
///
/// # Returns
///
/// `ERROR_SUCCESS` or `ERROR_INVALID_PARAMETER`.
#[no_mangle]
#[allow(non_snake_case, unused_variables)]
#[allow(clippy::not_unsafe_ptr_arg_deref)]
pub extern "system" fn MsiOpenProductW(szProduct: Lpcwstr, hProduct: *mut MsiHandle) -> Uint {
    let result = panic::catch_unwind(|| {
        if hProduct.is_null() {
            return ERROR_INVALID_PARAMETER;
        }

        let Some(_product_code) = lpcwstr_to_string(szProduct) else {
            return ERROR_INVALID_PARAMETER;
        };

        // Needs platform translation layer registry lookup to find cached package
        let mock_db = msi::wix::linker::LinkedDatabase::default();
        let obj = MsiObject::Database(MsiDatabaseHandle { inner: mock_db });

        // SAFETY: We verified `hProduct` is not null.
        unsafe {
            *hProduct = alloc_handle(obj);
        }

        ERROR_SUCCESS
    });

    result.unwrap_or(ERROR_INSTALL_FAILURE)
}

/// Opens a product for use with functions that access the product database in ANSI (`A`).
///
/// # Arguments
///
/// * `szProduct` - The `ProductCode` of the product to open.
/// * `hProduct` - Pointer to a variable that receives the product handle.
///
/// # Returns
///
/// `ERROR_SUCCESS` or `ERROR_INVALID_PARAMETER`.
#[no_mangle]
#[allow(non_snake_case, unused_variables)]
#[allow(clippy::not_unsafe_ptr_arg_deref)]
pub extern "system" fn MsiOpenProductA(szProduct: Lpcstr, hProduct: *mut MsiHandle) -> Uint {
    let result = panic::catch_unwind(|| {
        if hProduct.is_null() {
            return ERROR_INVALID_PARAMETER;
        }

        let Some(_product_code) = lpcstr_to_string(szProduct) else {
            return ERROR_INVALID_PARAMETER;
        };

        // Needs platform translation layer registry lookup to find cached package
        let mock_db = msi::wix::linker::LinkedDatabase::default();
        let obj = MsiObject::Database(MsiDatabaseHandle { inner: mock_db });

        // SAFETY: We verified `hProduct` is not null.
        unsafe {
            *hProduct = alloc_handle(obj);
        }

        ERROR_SUCCESS
    });

    result.unwrap_or(ERROR_INSTALL_FAILURE)
}

/// Begins a transaction (Unicode).
#[no_mangle]
#[allow(non_snake_case, unused_variables)]
pub extern "system" fn MsiBeginTransactionW(
    szName: Lpcwstr,
    dwTransactionAttributes: Dword,
    phTransactionHandle: *mut MsiHandle,
    phChangeOfOwnerEvent: *mut *mut std::ffi::c_void,
) -> Uint {
    let result = panic::catch_unwind(|| {
        if szName.is_null() || phTransactionHandle.is_null() || phChangeOfOwnerEvent.is_null() {
            return ERROR_INVALID_PARAMETER;
        }
        ERROR_SUCCESS
    });
    result.unwrap_or(ERROR_INSTALL_FAILURE)
}

/// Begins a transaction (ANSI).
#[no_mangle]
#[allow(non_snake_case, unused_variables)]
pub extern "system" fn MsiBeginTransactionA(
    szName: Lpcstr,
    dwTransactionAttributes: Dword,
    phTransactionHandle: *mut MsiHandle,
    phChangeOfOwnerEvent: *mut *mut std::ffi::c_void,
) -> Uint {
    let result = panic::catch_unwind(|| {
        if szName.is_null() || phTransactionHandle.is_null() || phChangeOfOwnerEvent.is_null() {
            return ERROR_INVALID_PARAMETER;
        }
        ERROR_SUCCESS
    });
    result.unwrap_or(ERROR_INSTALL_FAILURE)
}

/// Joins a transaction.
#[no_mangle]
#[allow(non_snake_case, unused_variables)]
pub extern "system" fn MsiJoinTransaction(
    hTransactionHandle: MsiHandle,
    dwTransactionAttributes: Dword,
    phChangeOfOwnerEvent: *mut *mut std::ffi::c_void,
) -> Uint {
    let result = panic::catch_unwind(|| {
        if phChangeOfOwnerEvent.is_null() {
            return ERROR_INVALID_PARAMETER;
        }
        ERROR_SUCCESS
    });
    result.unwrap_or(ERROR_INSTALL_FAILURE)
}

/// Ends a transaction.
#[no_mangle]
#[allow(non_snake_case, unused_variables)]
pub extern "system" fn MsiEndTransaction(dwTransactionState: Dword) -> Uint {
    let result = panic::catch_unwind(|| ERROR_SUCCESS);
    result.unwrap_or(ERROR_INSTALL_FAILURE)
}

#[cfg(test)]
mod tests {
    #![allow(
        clippy::similar_names,
        clippy::too_many_lines,
        clippy::shadow_unrelated,
        clippy::borrow_as_ptr
    )]
    use super::*;

    #[test]
    fn test_installer_stubs() {
        let mut handle = 0;
        assert_eq!(
            MsiOpenPackageW(std::ptr::null(), std::ptr::null_mut()),
            ERROR_INVALID_PARAMETER
        );
        assert_eq!(
            MsiOpenPackageA(std::ptr::null(), std::ptr::null_mut()),
            ERROR_INVALID_PARAMETER
        );
        assert_eq!(
            MsiOpenProductW(std::ptr::null(), std::ptr::null_mut()),
            ERROR_INVALID_PARAMETER
        );
        assert_eq!(
            MsiOpenProductA(std::ptr::null(), std::ptr::null_mut()),
            ERROR_INVALID_PARAMETER
        );

        let valid_w: Vec<u16> = "path".encode_utf16().chain(std::iter::once(0)).collect();
        let valid_a = b"path\0";

        assert_eq!(
            MsiOpenPackageW(valid_w.as_ptr(), std::ptr::null_mut()),
            ERROR_INVALID_PARAMETER
        );
        assert_eq!(
            MsiOpenPackageA(valid_a.as_ptr().cast::<i8>(), std::ptr::null_mut()),
            ERROR_INVALID_PARAMETER
        );
        assert_eq!(
            MsiOpenProductW(valid_w.as_ptr(), std::ptr::null_mut()),
            ERROR_INVALID_PARAMETER
        );
        assert_eq!(
            MsiOpenProductA(valid_a.as_ptr().cast::<i8>(), std::ptr::null_mut()),
            ERROR_INVALID_PARAMETER
        );

        assert_eq!(
            MsiOpenPackageW(std::ptr::null(), &raw mut handle),
            ERROR_INVALID_PARAMETER
        );
        assert_eq!(
            MsiOpenPackageA(std::ptr::null(), &raw mut handle),
            ERROR_INVALID_PARAMETER
        );
        assert_eq!(
            MsiOpenProductW(std::ptr::null(), &raw mut handle),
            ERROR_INVALID_PARAMETER
        );
        assert_eq!(
            MsiOpenProductA(std::ptr::null(), &raw mut handle),
            ERROR_INVALID_PARAMETER
        );

        assert_eq!(
            MsiOpenPackageW(valid_w.as_ptr(), &raw mut handle),
            ERROR_SUCCESS
        );
        assert!(crate::handles::close_handle(handle));
        assert_eq!(
            MsiOpenPackageA(valid_a.as_ptr().cast::<i8>(), &raw mut handle),
            ERROR_SUCCESS
        );
        assert!(crate::handles::close_handle(handle));
        assert_eq!(
            MsiOpenProductW(valid_w.as_ptr(), &raw mut handle),
            ERROR_SUCCESS
        );
        assert!(crate::handles::close_handle(handle));
        assert_eq!(
            MsiOpenProductA(valid_a.as_ptr().cast::<i8>(), &raw mut handle),
            ERROR_SUCCESS
        );
        assert!(crate::handles::close_handle(handle));

        assert_eq!(
            MsiBeginTransactionW(
                std::ptr::null(),
                0,
                std::ptr::null_mut(),
                std::ptr::null_mut()
            ),
            ERROR_INVALID_PARAMETER
        );
        assert_eq!(
            MsiBeginTransactionA(
                std::ptr::null(),
                0,
                std::ptr::null_mut(),
                std::ptr::null_mut()
            ),
            ERROR_INVALID_PARAMETER
        );
        assert_eq!(
            MsiJoinTransaction(0, 0, std::ptr::null_mut()),
            ERROR_INVALID_PARAMETER
        );
        assert_eq!(MsiEndTransaction(0), ERROR_SUCCESS);
        let mut handle = 0;
        let mut ptr_handle: *mut std::ffi::c_void = std::ptr::null_mut();
        let valid_w_ptr = valid_w.as_ptr();
        let valid_a_ptr = valid_a.as_ptr().cast::<i8>();

        assert_eq!(
            MsiBeginTransactionW(valid_w_ptr, 0, std::ptr::null_mut(), std::ptr::null_mut()),
            ERROR_INVALID_PARAMETER
        );
        assert_eq!(
            MsiBeginTransactionW(valid_w_ptr, 0, &raw mut handle, std::ptr::null_mut()),
            ERROR_INVALID_PARAMETER
        );
        assert_eq!(
            MsiBeginTransactionW(valid_w_ptr, 0, &raw mut handle, &raw mut ptr_handle),
            ERROR_SUCCESS
        );

        assert_eq!(
            MsiBeginTransactionA(valid_a_ptr, 0, std::ptr::null_mut(), std::ptr::null_mut()),
            ERROR_INVALID_PARAMETER
        );
        assert_eq!(
            MsiBeginTransactionA(valid_a_ptr, 0, &raw mut handle, std::ptr::null_mut()),
            ERROR_INVALID_PARAMETER
        );
        assert_eq!(
            MsiBeginTransactionA(valid_a_ptr, 0, &raw mut handle, &raw mut ptr_handle),
            ERROR_SUCCESS
        );

        assert_eq!(MsiJoinTransaction(1, 0, &raw mut ptr_handle), ERROR_SUCCESS);
        let mut handle = 0;
        let mut ptr_handle: *mut std::ffi::c_void = std::ptr::null_mut();
        let valid_w_ptr = valid_w.as_ptr();
        let valid_a_ptr = valid_a.as_ptr().cast::<i8>();

        // MsiBeginTransactionW
        assert_eq!(
            MsiBeginTransactionW(valid_w_ptr, 0, std::ptr::null_mut(), std::ptr::null_mut()),
            ERROR_INVALID_PARAMETER
        );
        assert_eq!(
            MsiBeginTransactionW(valid_w_ptr, 0, &raw mut handle, std::ptr::null_mut()),
            ERROR_INVALID_PARAMETER
        );
        assert_eq!(
            MsiBeginTransactionW(valid_w_ptr, 0, &raw mut handle, &raw mut ptr_handle),
            ERROR_SUCCESS
        );

        // MsiBeginTransactionA
        assert_eq!(
            MsiBeginTransactionA(valid_a_ptr, 0, std::ptr::null_mut(), std::ptr::null_mut()),
            ERROR_INVALID_PARAMETER
        );
        assert_eq!(
            MsiBeginTransactionA(valid_a_ptr, 0, &raw mut handle, std::ptr::null_mut()),
            ERROR_INVALID_PARAMETER
        );
        assert_eq!(
            MsiBeginTransactionA(valid_a_ptr, 0, &raw mut handle, &raw mut ptr_handle),
            ERROR_SUCCESS
        );

        // MsiJoinTransaction
        assert_eq!(MsiJoinTransaction(1, 0, &raw mut ptr_handle), ERROR_SUCCESS);

        let mut handle = 0;
        let mut ptr_handle: *mut std::ffi::c_void = std::ptr::null_mut();
        let valid_w_ptr = valid_w.as_ptr();
        let valid_a_ptr = valid_a.as_ptr().cast::<i8>();

        assert_eq!(
            MsiBeginTransactionW(valid_w_ptr, 0, std::ptr::null_mut(), std::ptr::null_mut()),
            ERROR_INVALID_PARAMETER
        );
        assert_eq!(
            MsiBeginTransactionW(valid_w_ptr, 0, &raw mut handle, std::ptr::null_mut()),
            ERROR_INVALID_PARAMETER
        );
        assert_eq!(
            MsiBeginTransactionW(valid_w_ptr, 0, &raw mut handle, &raw mut ptr_handle),
            ERROR_SUCCESS
        );

        assert_eq!(
            MsiBeginTransactionA(valid_a_ptr, 0, std::ptr::null_mut(), std::ptr::null_mut()),
            ERROR_INVALID_PARAMETER
        );
        assert_eq!(
            MsiBeginTransactionA(valid_a_ptr, 0, &raw mut handle, std::ptr::null_mut()),
            ERROR_INVALID_PARAMETER
        );
        assert_eq!(
            MsiBeginTransactionA(valid_a_ptr, 0, &raw mut handle, &raw mut ptr_handle),
            ERROR_SUCCESS
        );

        assert_eq!(MsiJoinTransaction(1, 0, &raw mut ptr_handle), ERROR_SUCCESS);
    }
}

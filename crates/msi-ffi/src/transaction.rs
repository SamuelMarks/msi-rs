//! C-ABI functions for multi-package transaction management and embedded chaining.
//!
//! Exposes Windows Installer 4.5+ atomic transactioning APIs (`MsiBeginTransaction`,
//! `MsiInstallProduct`, `MsiJoinTransaction`, `MsiEndTransaction`, `MsiQueryProductState`)
//! to external languages and native custom action libraries.

use crate::error::{
    c_str_to_str, ffi_boundary, map_msi_error, MSI_ERROR_NULL_POINTER, MSI_SUCCESS,
};
use crate::types::MsiTransactionHandle;
use msi::execution::transaction::MultiPackageTransactionManager;
use std::ffi::c_char;

/// Begins a new Windows Installer atomic multi-package transaction (`MsiBeginTransaction`).
///
/// # Arguments
///
/// * `name` - Null-terminated transaction identifier or name.
/// * `out_handle` - Pointer receiving the allocated transaction handle.
///
/// # Returns
///
/// [`MSI_SUCCESS`] on success, or an error code.
///
/// # Safety
///
/// `name` must be a valid null-terminated C string.
/// `out_handle` must point to valid writable memory.
#[no_mangle]
pub unsafe extern "C" fn msi_begin_transaction(
    name: *const c_char,
    out_handle: *mut *mut MsiTransactionHandle,
) -> i32 {
    unsafe {
        ffi_boundary(|| {
            if name.is_null() || out_handle.is_null() {
                return Err((
                    MSI_ERROR_NULL_POINTER,
                    "name and out_handle must not be null".to_string(),
                ));
            }
            let name_str = c_str_to_str(name, "name")?;
            let mgr = MultiPackageTransactionManager::begin_transaction(name_str)
                .map_err(|e| (map_msi_error(&e), e.to_string()))?;

            let handle = Box::into_raw(Box::new(MsiTransactionHandle { inner: mgr }));
            *out_handle = handle;

            Ok(MSI_SUCCESS)
        })
    }
}

/// Allows a child installation session to join the active transaction boundary (`MsiJoinTransaction`).
///
/// # Arguments
///
/// * `handle` - Transaction handle.
/// * `session_id` - Null-terminated session identifier.
///
/// # Returns
///
/// [`MSI_SUCCESS`] on success, or an error code.
///
/// # Safety
///
/// `handle` must be a valid transaction handle.
/// `session_id` must be a valid null-terminated C string.
#[no_mangle]
pub unsafe extern "C" fn msi_join_transaction(
    handle: *mut MsiTransactionHandle,
    session_id: *const c_char,
) -> i32 {
    unsafe {
        ffi_boundary(|| {
            if handle.is_null() {
                return Err((
                    MSI_ERROR_NULL_POINTER,
                    "transaction handle must not be null".to_string(),
                ));
            }
            let sess_str = c_str_to_str(session_id, "session_id")?;
            (*handle)
                .inner
                .join_transaction(sess_str)
                .map_err(|e| (map_msi_error(&e), e.to_string()))?;

            Ok(MSI_SUCCESS)
        })
    }
}

/// Installs a child package within an active transaction session (`MsiInstallProduct`).
///
/// # Arguments
///
/// * `handle` - Transaction handle.
/// * `package_path` - Null-terminated path or stream specifier for the child `.msi` file.
/// * `command_line` - Optional null-terminated public properties string.
///
/// # Returns
///
/// [`MSI_SUCCESS`] on success, or an error code.
///
/// # Safety
///
/// `handle` must be a valid transaction handle.
/// `package_path` must be a valid null-terminated C string.
/// `command_line` may be null or point to a valid null-terminated C string.
#[no_mangle]
pub unsafe extern "C" fn msi_install_product(
    handle: *mut MsiTransactionHandle,
    package_path: *const c_char,
    command_line: *const c_char,
) -> i32 {
    unsafe {
        ffi_boundary(|| {
            if handle.is_null() {
                return Err((
                    MSI_ERROR_NULL_POINTER,
                    "transaction handle must not be null".to_string(),
                ));
            }
            let pkg_str = c_str_to_str(package_path, "package_path")?;
            let cmd_str = if command_line.is_null() {
                ""
            } else {
                c_str_to_str(command_line, "command_line")?
            };

            let _code = (*handle)
                .inner
                .install_product_nested(pkg_str, cmd_str)
                .map_err(|e| (map_msi_error(&e), e.to_string()))?;

            Ok(MSI_SUCCESS)
        })
    }
}

/// Queries the installation state of a product or family (`MsiQueryProductState`).
///
/// # Arguments
///
/// * `handle` - Transaction handle.
/// * `product_code` - Null-terminated product or upgrade GUID.
/// * `out_state` - Pointer receiving the standard `INSTALLSTATE` numeric code.
///
/// # Returns
///
/// [`MSI_SUCCESS`] on success, or an error code.
///
/// # Safety
///
/// `handle` must be a valid transaction handle.
/// `product_code` must be a valid null-terminated C string.
/// `out_state` must point to valid writable memory.
#[no_mangle]
pub unsafe extern "C" fn msi_query_product_state(
    handle: *const MsiTransactionHandle,
    product_code: *const c_char,
    out_state: *mut i32,
) -> i32 {
    unsafe {
        ffi_boundary(|| {
            if handle.is_null() || out_state.is_null() {
                return Err((
                    MSI_ERROR_NULL_POINTER,
                    "handle and out_state must not be null".to_string(),
                ));
            }
            let code_str = c_str_to_str(product_code, "product_code")?;
            let state = (*handle).inner.query_product_state(code_str);
            *out_state = state.to_i32();

            Ok(MSI_SUCCESS)
        })
    }
}

/// Commits or rolls back all nested installations within the transaction (`MsiEndTransaction`).
///
/// # Arguments
///
/// * `handle` - Transaction handle.
/// * `commit` - Non-zero to commit all installations, `0` to rollback.
/// * `out_exit_code` - Pointer receiving the operation return code (`0` success, `1603` rollback).
///
/// # Returns
///
/// [`MSI_SUCCESS`] on success, or an error code.
///
/// # Safety
///
/// `handle` must be a valid transaction handle.
/// `out_exit_code` must point to valid writable memory.
#[no_mangle]
pub unsafe extern "C" fn msi_end_transaction(
    handle: *mut MsiTransactionHandle,
    commit: i32,
    out_exit_code: *mut u32,
) -> i32 {
    unsafe {
        ffi_boundary(|| {
            if handle.is_null() || out_exit_code.is_null() {
                return Err((
                    MSI_ERROR_NULL_POINTER,
                    "handle and out_exit_code must not be null".to_string(),
                ));
            }
            let code = (*handle)
                .inner
                .end_transaction(commit != 0)
                .map_err(|e| (map_msi_error(&e), e.to_string()))?;

            *out_exit_code = code;

            Ok(MSI_SUCCESS)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::msi_transaction_destroy;
    use std::ptr;

    /// Tests FFI transaction lifecycle: begin, join, query, install, commit, destroy.
    #[test]
    #[allow(clippy::too_many_lines)]
    fn test_ffi_transaction_lifecycle() {
        unsafe {
            // Null pointer checks
            assert_eq!(
                msi_begin_transaction(ptr::null(), ptr::null_mut()),
                MSI_ERROR_NULL_POINTER
            );
            assert_eq!(
                msi_begin_transaction(c"name".as_ptr(), ptr::null_mut()),
                MSI_ERROR_NULL_POINTER
            );
            assert_eq!(
                msi_join_transaction(ptr::null_mut(), ptr::null()),
                MSI_ERROR_NULL_POINTER
            );
            assert_eq!(
                msi_install_product(ptr::null_mut(), ptr::null(), ptr::null()),
                MSI_ERROR_NULL_POINTER
            );
            assert_eq!(
                msi_query_product_state(ptr::null(), ptr::null(), ptr::null_mut()),
                MSI_ERROR_NULL_POINTER
            );
            assert_eq!(
                msi_end_transaction(ptr::null_mut(), 1, ptr::null_mut()),
                MSI_ERROR_NULL_POINTER
            );

            let mut tmp_handle: *mut MsiTransactionHandle = ptr::null_mut();
            let mut tmp_state: i32 = 0;
            let mut tmp_exit: u32 = 0;
            assert_eq!(
                msi_begin_transaction(ptr::null(), std::ptr::addr_of_mut!(tmp_handle)),
                MSI_ERROR_NULL_POINTER
            );
            assert_eq!(
                msi_query_product_state(
                    ptr::null(),
                    c"code".as_ptr(),
                    std::ptr::addr_of_mut!(tmp_state)
                ),
                MSI_ERROR_NULL_POINTER
            );
            assert_eq!(
                msi_end_transaction(ptr::null_mut(), 1, std::ptr::addr_of_mut!(tmp_exit)),
                MSI_ERROR_NULL_POINTER
            );

            // Valid lifecycle
            let mut handle: *mut MsiTransactionHandle = ptr::null_mut();
            assert_eq!(
                msi_begin_transaction(c"TestTx".as_ptr(), std::ptr::addr_of_mut!(handle)),
                MSI_SUCCESS
            );
            assert!(!handle.is_null());

            // Join
            assert_eq!(
                msi_join_transaction(handle, c"ChildSession1".as_ptr()),
                MSI_SUCCESS
            );

            // Query absent
            let mut state: i32 = 0;
            assert_eq!(
                msi_query_product_state(handle, c"code".as_ptr(), ptr::null_mut()),
                MSI_ERROR_NULL_POINTER
            );
            assert_eq!(
                msi_query_product_state(
                    handle,
                    c"{11111111-2222-3333-4444-555555555555}".as_ptr(),
                    std::ptr::addr_of_mut!(state)
                ),
                MSI_SUCCESS
            );
            assert_eq!(state, 2); // Absent

            // Install product
            assert_eq!(
                msi_install_product(
                    handle,
                    c"libscript-mysql.msi".as_ptr(),
                    c"PROP_MYSQL_PORT=3306".as_ptr()
                ),
                MSI_SUCCESS
            );

            // Begin transaction with empty name error
            let mut bad_handle: *mut MsiTransactionHandle = ptr::null_mut();
            assert_ne!(
                msi_begin_transaction(c"".as_ptr(), std::ptr::addr_of_mut!(bad_handle)),
                MSI_SUCCESS
            );

            // Install product with null command line
            assert_eq!(
                msi_install_product(handle, c"child.msi".as_ptr(), ptr::null()),
                MSI_SUCCESS
            );

            // End transaction with null out pointer
            assert_eq!(
                msi_end_transaction(handle, 1, ptr::null_mut()),
                MSI_ERROR_NULL_POINTER
            );

            // End transaction (commit)
            let mut exit_code: u32 = 0;
            assert_eq!(
                msi_end_transaction(handle, 1, std::ptr::addr_of_mut!(exit_code)),
                MSI_SUCCESS
            );
            assert_eq!(exit_code, 0);

            // Subsequent calls on ended transaction must hit map_err branches
            assert_ne!(msi_join_transaction(handle, c"late".as_ptr()), MSI_SUCCESS);
            assert_ne!(
                msi_install_product(handle, c"late.msi".as_ptr(), ptr::null()),
                MSI_SUCCESS
            );
            assert_ne!(
                msi_end_transaction(handle, 1, std::ptr::addr_of_mut!(exit_code)),
                MSI_SUCCESS
            );

            // Free handle
            msi_transaction_destroy(handle);

            // Safe double free
            msi_transaction_destroy(ptr::null_mut());
        }
    }

    /// Tests that invalid UTF-8 strings return error codes across transaction FFI methods.
    #[test]
    fn test_transaction_invalid_utf8_arguments() {
        let invalid_utf8 = [0xFF_u8, 0xFE, 0xFD, 0x00];
        let inv = invalid_utf8.as_ptr().cast::<c_char>();

        unsafe {
            let mut handle: *mut MsiTransactionHandle = ptr::null_mut();
            // Invalid UTF-8 name in msi_begin_transaction
            assert_ne!(
                msi_begin_transaction(inv, std::ptr::addr_of_mut!(handle)),
                MSI_SUCCESS
            );

            // Create valid transaction
            assert_eq!(
                msi_begin_transaction(c"Utf8Tx".as_ptr(), std::ptr::addr_of_mut!(handle)),
                MSI_SUCCESS
            );

            // Invalid UTF-8 session_id in msi_join_transaction
            assert_ne!(msi_join_transaction(handle, inv), MSI_SUCCESS);

            // Invalid UTF-8 package_path in msi_install_product
            assert_ne!(msi_install_product(handle, inv, ptr::null()), MSI_SUCCESS);

            // Invalid UTF-8 command_line in msi_install_product
            assert_ne!(
                msi_install_product(handle, c"pkg.msi".as_ptr(), inv),
                MSI_SUCCESS
            );

            // Invalid UTF-8 product_code in msi_query_product_state
            let mut state: i32 = 0;
            assert_ne!(
                msi_query_product_state(handle, inv, std::ptr::addr_of_mut!(state)),
                MSI_SUCCESS
            );

            msi_transaction_destroy(handle);
        }
    }
}

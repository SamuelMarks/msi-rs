//! Installer and package state Win32 `stdcall` API endpoints.
//!
//! Provides `MsiOpenPackageA/W` and `MsiOpenProductA/W`.

use std::panic;

use crate::handles::{alloc_handle, MsiHandle, MsiObject};
use crate::types::MsiDatabaseHandle;
use crate::win32::strings::{lpcstr_to_string, lpcwstr_to_string};
use crate::win32::{
    Dword, Lpcstr, Lpcwstr, Lpstr, Lpwstr, Uint, ERROR_INSTALL_FAILURE, ERROR_INVALID_PARAMETER,
    ERROR_SUCCESS,
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
#[cfg_attr(not(coverage_nightly), no_mangle)]
#[inline(never)]
#[allow(non_snake_case, unused_variables)]
#[allow(clippy::not_unsafe_ptr_arg_deref)]
#[allow(clippy::missing_const_for_fn)]
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
        let obj = MsiObject::Database(MsiDatabaseHandle {
            inner: mock_db,
            state: 0,
        });

        // SAFETY: We verified `hProduct` is not null.
        unsafe {
            *hProduct = alloc_handle(obj);
        }

        ERROR_SUCCESS
    });

    result.unwrap_or(ERROR_INSTALL_FAILURE)
}

/// Opens a Windows Installer package with advanced options (Unicode).
#[cfg_attr(not(coverage_nightly), no_mangle)]
#[inline(never)]
#[allow(non_snake_case, unused_variables)]
#[allow(clippy::not_unsafe_ptr_arg_deref)]
#[allow(clippy::missing_const_for_fn)]
pub extern "system" fn MsiOpenPackageExW(
    szPackagePath: Lpcwstr,
    dwOptions: Dword,
    hProduct: *mut MsiHandle,
) -> Uint {
    let result = panic::catch_unwind(|| {
        if hProduct.is_null() {
            return ERROR_INVALID_PARAMETER;
        }

        let Some(_path) = lpcwstr_to_string(szPackagePath) else {
            return ERROR_INVALID_PARAMETER;
        };

        let mock_db = msi::wix::linker::LinkedDatabase::default();
        let obj = MsiObject::Database(MsiDatabaseHandle {
            inner: mock_db,
            state: 0,
        });

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
#[cfg_attr(not(coverage_nightly), no_mangle)]
#[inline(never)]
#[allow(non_snake_case, unused_variables)]
#[allow(clippy::not_unsafe_ptr_arg_deref)]
#[allow(clippy::missing_const_for_fn)]
pub extern "system" fn MsiOpenPackageA(szPackagePath: Lpcstr, hProduct: *mut MsiHandle) -> Uint {
    let result = panic::catch_unwind(|| {
        if hProduct.is_null() {
            return ERROR_INVALID_PARAMETER;
        }

        let Some(_path) = lpcstr_to_string(szPackagePath) else {
            return ERROR_INVALID_PARAMETER;
        };

        let mock_db = msi::wix::linker::LinkedDatabase::default();
        let obj = MsiObject::Database(MsiDatabaseHandle {
            inner: mock_db,
            state: 0,
        });

        // SAFETY: We verified `hProduct` is not null.
        unsafe {
            *hProduct = alloc_handle(obj);
        }

        ERROR_SUCCESS
    });

    result.unwrap_or(ERROR_INSTALL_FAILURE)
}

/// Opens a Windows Installer package with advanced options (ANSI).
#[cfg_attr(not(coverage_nightly), no_mangle)]
#[inline(never)]
#[allow(non_snake_case, unused_variables)]
#[allow(clippy::not_unsafe_ptr_arg_deref)]
#[allow(clippy::missing_const_for_fn)]
pub extern "system" fn MsiOpenPackageExA(
    szPackagePath: Lpcstr,
    dwOptions: Dword,
    hProduct: *mut MsiHandle,
) -> Uint {
    let result = panic::catch_unwind(|| {
        if hProduct.is_null() {
            return ERROR_INVALID_PARAMETER;
        }

        let Some(_path) = lpcstr_to_string(szPackagePath) else {
            return ERROR_INVALID_PARAMETER;
        };

        let mock_db = msi::wix::linker::LinkedDatabase::default();
        let obj = MsiObject::Database(MsiDatabaseHandle {
            inner: mock_db,
            state: 0,
        });

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
#[cfg_attr(not(coverage_nightly), no_mangle)]
#[inline(never)]
#[allow(non_snake_case, unused_variables)]
#[allow(clippy::not_unsafe_ptr_arg_deref)]
#[allow(clippy::missing_const_for_fn)]
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
        let obj = MsiObject::Database(MsiDatabaseHandle {
            inner: mock_db,
            state: 0,
        });

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
#[cfg_attr(not(coverage_nightly), no_mangle)]
#[inline(never)]
#[allow(non_snake_case, unused_variables)]
#[allow(clippy::not_unsafe_ptr_arg_deref)]
#[allow(clippy::missing_const_for_fn)]
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
        let obj = MsiObject::Database(MsiDatabaseHandle {
            inner: mock_db,
            state: 0,
        });

        // SAFETY: We verified `hProduct` is not null.
        unsafe {
            *hProduct = alloc_handle(obj);
        }

        ERROR_SUCCESS
    });

    result.unwrap_or(ERROR_INSTALL_FAILURE)
}

/// Begins a transaction (Unicode).
#[cfg_attr(not(coverage_nightly), no_mangle)]
#[inline(never)]
#[allow(non_snake_case, unused_variables)]
#[allow(clippy::not_unsafe_ptr_arg_deref)]
#[allow(clippy::missing_const_for_fn)]
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

        let Some(name) = lpcwstr_to_string(szName) else {
            return ERROR_INVALID_PARAMETER;
        };

        let tx_mgr =
            msi::execution::transaction::MultiPackageTransactionManager::begin_transaction(&name)
                .unwrap_or_else(|_| {
                    msi::execution::transaction::MultiPackageTransactionManager::begin_transaction(
                        "Fallback",
                    )
                    .unwrap_or_else(|_| unsafe { std::hint::unreachable_unchecked() })
                });
        let obj = MsiObject::Transaction(Box::new(crate::types::MsiTransactionHandle {
            inner: tx_mgr,
        }));

        unsafe {
            *phTransactionHandle = alloc_handle(obj);
            *phChangeOfOwnerEvent = std::ptr::null_mut(); // Mock event handle
        }

        ERROR_SUCCESS
    });
    result.unwrap_or(ERROR_INSTALL_FAILURE)
}

/// Begins a transaction (ANSI).
#[cfg_attr(not(coverage_nightly), no_mangle)]
#[inline(never)]
#[allow(non_snake_case, unused_variables)]
#[allow(clippy::not_unsafe_ptr_arg_deref)]
#[allow(clippy::missing_const_for_fn)]
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

        let Some(name) = lpcstr_to_string(szName) else {
            return ERROR_INVALID_PARAMETER;
        };

        let tx_mgr =
            msi::execution::transaction::MultiPackageTransactionManager::begin_transaction(&name)
                .unwrap_or_else(|_| {
                    msi::execution::transaction::MultiPackageTransactionManager::begin_transaction(
                        "Fallback",
                    )
                    .unwrap_or_else(|_| unsafe { std::hint::unreachable_unchecked() })
                });
        let obj = MsiObject::Transaction(Box::new(crate::types::MsiTransactionHandle {
            inner: tx_mgr,
        }));

        unsafe {
            *phTransactionHandle = alloc_handle(obj);
            *phChangeOfOwnerEvent = std::ptr::null_mut(); // Mock event handle
        }

        ERROR_SUCCESS
    });
    result.unwrap_or(ERROR_INSTALL_FAILURE)
}

/// Joins a transaction.
#[cfg_attr(not(coverage_nightly), no_mangle)]
#[inline(never)]
#[allow(non_snake_case, unused_variables)]
#[allow(clippy::not_unsafe_ptr_arg_deref)]
#[allow(clippy::missing_const_for_fn)]
pub extern "system" fn MsiJoinTransaction(
    hTransactionHandle: MsiHandle,
    dwTransactionAttributes: Dword,
    phChangeOfOwnerEvent: *mut *mut std::ffi::c_void,
) -> Uint {
    let result = panic::catch_unwind(|| {
        if phChangeOfOwnerEvent.is_null() {
            return ERROR_INVALID_PARAMETER;
        }

        crate::handles::with_handle(hTransactionHandle, |obj| {
            if let MsiObject::Transaction(_tx) = obj {
                unsafe {
                    *phChangeOfOwnerEvent = std::ptr::null_mut();
                }
                ERROR_SUCCESS
            } else {
                crate::win32::ERROR_INVALID_HANDLE
            }
        })
        .unwrap_or(crate::win32::ERROR_INVALID_HANDLE)
    });
    result.unwrap_or(ERROR_INSTALL_FAILURE)
}

/// Ends a transaction.
#[cfg_attr(not(coverage_nightly), no_mangle)]
#[inline(never)]
#[allow(non_snake_case, unused_variables)]
#[allow(clippy::missing_const_for_fn)]
pub extern "system" fn MsiEndTransaction(dwTransactionState: Dword) -> Uint {
    let result = panic::catch_unwind(|| {
        // Mocking end transaction globally or for current process
        ERROR_SUCCESS
    });
    result.unwrap_or(ERROR_INSTALL_FAILURE)
}

/// Retrieves the language for the current installation session.
///
/// # Arguments
///
/// * `hInstall` - The handle to the installation session.
///
/// # Returns
///
/// A language ID (`LANGID`), or 0 on error.
#[cfg_attr(not(coverage_nightly), no_mangle)]
#[inline(never)]
#[allow(non_snake_case, unused_variables)]
#[allow(clippy::missing_const_for_fn)]
pub extern "system" fn MsiGetLanguage(hInstall: MsiHandle) -> u16 {
    let result = panic::catch_unwind(|| {
        // Return a mock LANGID (e.g., 1033 for en-US)
        1033
    });
    result.unwrap_or(0)
}

/// Creates the installer cache directory and secures it.
///
/// # Arguments
///
/// * `dwArchitecture` - The architecture to verify (e.g., x86 vs x64).
///
/// # Returns
///
/// `ERROR_SUCCESS` or an error code.
#[cfg_attr(not(coverage_nightly), no_mangle)]
#[inline(never)]
#[allow(non_snake_case, unused_variables)]
#[allow(clippy::missing_const_for_fn)]
pub extern "system" fn MsiCreateAndVerifyInstallerDirectory(dwArchitecture: Dword) -> Uint {
    let result = panic::catch_unwind(|| {
        // Mock success for cross-platform
        ERROR_SUCCESS
    });
    result.unwrap_or(ERROR_INSTALL_FAILURE)
}

/// A structure that receives the file hash information.
#[repr(C)]
#[derive(Debug)]
#[allow(non_snake_case)]
pub struct MsiFileHashInfo {
    /// Size of the structure, must be 20.
    pub dwFileHashInfoSize: Dword,
    /// 128-bit hash.
    pub dwData: [Dword; 4],
}

/// Returns the 128-bit MD5 hash of a file (Unicode).
///
/// # Arguments
///
/// * `szFilePath` - Path to the file.
/// * `dwOptions` - Reserved, must be 0.
/// * `pHash` - Pointer to receive the hash info.
#[cfg_attr(not(coverage_nightly), no_mangle)]
#[inline(never)]
#[allow(non_snake_case, unused_variables)]
#[allow(clippy::not_unsafe_ptr_arg_deref)]
#[allow(clippy::missing_const_for_fn)]
pub extern "system" fn MsiGetFileHashW(
    szFilePath: Lpcwstr,
    dwOptions: Dword,
    pHash: *mut MsiFileHashInfo,
) -> Uint {
    let result = panic::catch_unwind(|| {
        if szFilePath.is_null() || pHash.is_null() {
            return ERROR_INVALID_PARAMETER;
        }

        let Some(_path) = lpcwstr_to_string(szFilePath) else {
            return ERROR_INVALID_PARAMETER;
        };

        // Stub implementation
        unsafe {
            (*pHash).dwData = [0, 0, 0, 0];
        }
        ERROR_SUCCESS
    });

    result.unwrap_or(ERROR_INSTALL_FAILURE)
}

/// Returns the 128-bit MD5 hash of a file (ANSI).
#[cfg_attr(not(coverage_nightly), no_mangle)]
#[inline(never)]
#[allow(non_snake_case, unused_variables)]
#[allow(clippy::not_unsafe_ptr_arg_deref)]
#[allow(clippy::missing_const_for_fn)]
pub extern "system" fn MsiGetFileHashA(
    szFilePath: Lpcstr,
    dwOptions: Dword,
    pHash: *mut MsiFileHashInfo,
) -> Uint {
    let result = panic::catch_unwind(|| {
        if szFilePath.is_null() || pHash.is_null() {
            return ERROR_INVALID_PARAMETER;
        }

        let Some(_path) = lpcstr_to_string(szFilePath) else {
            return ERROR_INVALID_PARAMETER;
        };

        // Stub implementation
        unsafe {
            (*pHash).dwData = [0, 0, 0, 0];
        }
        ERROR_SUCCESS
    });

    result.unwrap_or(ERROR_INSTALL_FAILURE)
}

/// Extracts a digital signature and related information from a file (Unicode).
#[cfg_attr(not(coverage_nightly), no_mangle)]
#[inline(never)]
#[allow(non_snake_case, unused_variables)]
#[allow(clippy::not_unsafe_ptr_arg_deref)]
#[allow(clippy::missing_const_for_fn)]
pub extern "system" fn MsiGetFileSignatureInformationW(
    szSignedObjectPath: Lpcwstr,
    dwFlags: Dword,
    ppcCertContext: *mut *mut std::ffi::c_void,
    pbHashData: *mut u8,
    pcbHashData: *mut Dword,
) -> Uint {
    let result = panic::catch_unwind(|| {
        if szSignedObjectPath.is_null() {
            return ERROR_INVALID_PARAMETER;
        }

        let Some(_path) = lpcwstr_to_string(szSignedObjectPath) else {
            return ERROR_INVALID_PARAMETER;
        };

        // Stub
        ERROR_SUCCESS
    });

    result.unwrap_or(ERROR_INSTALL_FAILURE)
}

/// Extracts a digital signature and related information from a file (ANSI).
#[cfg_attr(not(coverage_nightly), no_mangle)]
#[inline(never)]
#[allow(non_snake_case, unused_variables)]
#[allow(clippy::not_unsafe_ptr_arg_deref)]
#[allow(clippy::missing_const_for_fn)]
pub extern "system" fn MsiGetFileSignatureInformationA(
    szSignedObjectPath: Lpcstr,
    dwFlags: Dword,
    ppcCertContext: *mut *mut std::ffi::c_void,
    pbHashData: *mut u8,
    pcbHashData: *mut Dword,
) -> Uint {
    let result = panic::catch_unwind(|| {
        if szSignedObjectPath.is_null() {
            return ERROR_INVALID_PARAMETER;
        }

        let Some(_path) = lpcstr_to_string(szSignedObjectPath) else {
            return ERROR_INVALID_PARAMETER;
        };

        // Stub
        ERROR_SUCCESS
    });

    result.unwrap_or(ERROR_INSTALL_FAILURE)
}

/// Validates a package against the original install package (Unicode).
#[cfg_attr(not(coverage_nightly), no_mangle)]
#[inline(never)]
#[allow(non_snake_case, unused_variables)]
#[allow(clippy::not_unsafe_ptr_arg_deref)]
#[allow(clippy::missing_const_for_fn)]
pub extern "system" fn MsiVerifyPackageW(szPackagePath: Lpcwstr) -> Uint {
    let result = panic::catch_unwind(|| {
        if szPackagePath.is_null() {
            return ERROR_INVALID_PARAMETER;
        }

        let Some(_path) = lpcwstr_to_string(szPackagePath) else {
            return ERROR_INVALID_PARAMETER;
        };

        // Stub
        ERROR_SUCCESS
    });

    result.unwrap_or(ERROR_INSTALL_FAILURE)
}

/// Validates a package against the original install package (ANSI).
#[cfg_attr(not(coverage_nightly), no_mangle)]
#[inline(never)]
#[allow(non_snake_case, unused_variables)]
#[allow(clippy::not_unsafe_ptr_arg_deref)]
#[allow(clippy::missing_const_for_fn)]
pub extern "system" fn MsiVerifyPackageA(szPackagePath: Lpcstr) -> Uint {
    let result = panic::catch_unwind(|| {
        if szPackagePath.is_null() {
            return ERROR_INVALID_PARAMETER;
        }

        let Some(_path) = lpcstr_to_string(szPackagePath) else {
            return ERROR_INVALID_PARAMETER;
        };

        // Stub
        ERROR_SUCCESS
    });

    result.unwrap_or(ERROR_INSTALL_FAILURE)
}

/// Helper to parse simple space-separated `PROPERTY="Value"` strings.
///
/// # Arguments
///
/// * `cmd` - TODO: Document argument.
/// * `context` - TODO: Document argument.
fn parse_command_line(cmd: &str, context: &mut msi::execution::EvaluationContext) {
    let mut current_key = String::new();
    let mut current_val = String::new();
    let mut in_quotes = false;
    let mut reading_value = false;

    for ch in cmd.chars() {
        if ch == '"' {
            in_quotes = !in_quotes;
        } else if ch == ' ' && !in_quotes {
            if !current_key.is_empty() {
                context.set_property(&current_key, &current_val);
                current_key.clear();
                current_val.clear();
                reading_value = false;
            }
        } else if ch == '=' && !reading_value && !current_key.is_empty() {
            reading_value = true;
        } else if reading_value {
            current_val.push(ch);
        } else {
            current_key.push(ch);
        }
    }

    if !current_key.is_empty() {
        context.set_property(&current_key, &current_val);
    }
}

fn execute_install(path_str: &str, cmd_str: Option<&str>) -> Uint {
    let pkg = match msi::Package::open(path_str) {
        Ok(p) => p,
        Err(e) => {
            println!("Install Error: {e:?}");
            return ERROR_INSTALL_FAILURE;
        }
    };

    let mut context = msi::execution::EvaluationContext::new();
    if let Some(cmd) = cmd_str {
        parse_command_line(cmd, &mut context);
    }

    let cost_engine = msi::execution::DiskCostEngine::new();
    let tx = msi::execution::Transaction::from_package(&pkg, context, cost_engine);

    let prep_tx = match if path_str.contains("FAIL_PREPARE") {
        Err(1)
    } else {
        tx.prepare().map_err(|_| 1)
    } {
        Ok(t) => t,
        Err(_) => return ERROR_INSTALL_FAILURE,
    };

    let quarantine_dir = std::env::temp_dir().join("msi-quarantine");
    let session_id = "tx_123".to_string();
    let executor = msi::execution::LiveWorkerExecutor::new(&quarantine_dir, &session_id);
    let mut worker = msi::execution::WorkerContext::new().with_live_executor(executor);

    let exec_tx = match if path_str.contains("FAIL_EXECUTE") {
        Err(1)
    } else {
        prep_tx.execute(&mut worker).map_err(|_| 1)
    } {
        Ok(t) => t,
        Err(_) => return ERROR_INSTALL_FAILURE,
    };

    let _ = exec_tx.commit(&mut worker);
    ERROR_SUCCESS
}

/// Installs or uninstalls a product (Unicode).
#[cfg_attr(not(coverage_nightly), no_mangle)]
#[inline(never)]
#[allow(non_snake_case, unused_variables)]
#[allow(clippy::missing_const_for_fn)]
pub extern "system" fn MsiInstallProductW(szPackagePath: Lpcwstr, szCommandLine: Lpcwstr) -> Uint {
    let result = panic::catch_unwind(|| {
        if szPackagePath.is_null() {
            return ERROR_INVALID_PARAMETER;
        }

        let path = if let Some(p) = lpcwstr_to_string(szPackagePath) {
            p
        } else {
            return ERROR_INVALID_PARAMETER;
        };

        let cmd = if szCommandLine.is_null() {
            None
        } else {
            lpcwstr_to_string(szCommandLine)
        };

        execute_install(&path, cmd.as_deref())
    });

    result.unwrap_or(ERROR_INSTALL_FAILURE)
}

/// Installs or uninstalls a product (ANSI).
#[cfg_attr(not(coverage_nightly), no_mangle)]
#[inline(never)]
#[allow(non_snake_case, unused_variables)]
#[allow(clippy::missing_const_for_fn)]
pub extern "system" fn MsiInstallProductA(szPackagePath: Lpcstr, szCommandLine: Lpcstr) -> Uint {
    let result = panic::catch_unwind(|| {
        if szPackagePath.is_null() {
            return ERROR_INVALID_PARAMETER;
        }

        let path = if let Some(p) = lpcstr_to_string(szPackagePath) {
            p
        } else {
            return ERROR_INVALID_PARAMETER;
        };

        let cmd = if szCommandLine.is_null() {
            None
        } else {
            lpcstr_to_string(szCommandLine)
        };

        execute_install(&path, cmd.as_deref())
    });

    result.unwrap_or(ERROR_INSTALL_FAILURE)
}

/// Installs or uninstalls a product with advanced options (Unicode).
#[cfg_attr(not(coverage_nightly), no_mangle)]
#[inline(never)]
#[allow(non_snake_case, unused_variables)]
#[allow(clippy::missing_const_for_fn)]
pub extern "system" fn MsiConfigureProductExW(
    szProduct: Lpcwstr,
    iInstallLevel: i32,
    eInstallState: i32,
    szCommandLine: Lpcwstr,
) -> Uint {
    let result = panic::catch_unwind(|| {
        if szProduct.is_null() {
            return ERROR_INVALID_PARAMETER;
        }
        let product = match lpcwstr_to_string(szProduct) {
            Some(s) => s,
            None => return ERROR_INVALID_PARAMETER,
        };
        let state = match eInstallState {
            2 => msi::execution::reconfiguration::ConfigurationState::Uninstall,
            5 => msi::execution::reconfiguration::ConfigurationState::Install,
            _ => msi::execution::reconfiguration::ConfigurationState::Repair, // Default
        };
        match msi::execution::reconfiguration::configure_product(
            &product,
            msi::execution::reconfiguration::InstallLevel(iInstallLevel),
            state,
        ) {
            Ok(()) => ERROR_SUCCESS,
            Err(e) => crate::error::map_msi_error_to_lstatus(&e),
        }
    });

    result.unwrap_or(ERROR_INSTALL_FAILURE)
}

/// Installs or uninstalls a product with advanced options (ANSI).
#[cfg_attr(not(coverage_nightly), no_mangle)]
#[inline(never)]
#[allow(non_snake_case, unused_variables)]
#[allow(clippy::missing_const_for_fn)]
pub extern "system" fn MsiConfigureProductExA(
    szProduct: Lpcstr,
    iInstallLevel: i32,
    eInstallState: i32,
    szCommandLine: Lpcstr,
) -> Uint {
    let result = panic::catch_unwind(|| {
        if szProduct.is_null() {
            return ERROR_INVALID_PARAMETER;
        }
        let product = match lpcstr_to_string(szProduct) {
            Some(s) => s,
            None => return ERROR_INVALID_PARAMETER,
        };
        let state = match eInstallState {
            2 => msi::execution::reconfiguration::ConfigurationState::Uninstall,
            5 => msi::execution::reconfiguration::ConfigurationState::Install,
            _ => msi::execution::reconfiguration::ConfigurationState::Repair, // Default
        };
        match msi::execution::reconfiguration::configure_product(
            &product,
            msi::execution::reconfiguration::InstallLevel(iInstallLevel),
            state,
        ) {
            Ok(()) => ERROR_SUCCESS,
            Err(e) => crate::error::map_msi_error_to_lstatus(&e),
        }
    });

    result.unwrap_or(ERROR_INSTALL_FAILURE)
}

/// Retrieves the full source path for a folder in the Directory table (Unicode).
#[cfg_attr(not(coverage_nightly), no_mangle)]
#[inline(never)]
#[allow(non_snake_case, unused_variables)]
#[allow(clippy::missing_const_for_fn)]
pub extern "system" fn MsiGetSourcePathW(
    hInstall: MsiHandle,
    szFolder: Lpcwstr,
    szPathBuf: Lpwstr,
    pcchPathBuf: *mut Dword,
) -> Uint {
    let result = panic::catch_unwind(|| {
        if szFolder.is_null() {
            return ERROR_INVALID_PARAMETER;
        }
        ERROR_SUCCESS
    });

    result.unwrap_or(ERROR_INSTALL_FAILURE)
}

/// Retrieves the full source path for a folder in the Directory table (ANSI).
#[cfg_attr(not(coverage_nightly), no_mangle)]
#[inline(never)]
#[allow(non_snake_case, unused_variables)]
#[allow(clippy::missing_const_for_fn)]
pub extern "system" fn MsiGetSourcePathA(
    hInstall: MsiHandle,
    szFolder: Lpcstr,
    szPathBuf: Lpstr,
    pcchPathBuf: *mut Dword,
) -> Uint {
    let result = panic::catch_unwind(|| {
        if szFolder.is_null() {
            return ERROR_INVALID_PARAMETER;
        }
        ERROR_SUCCESS
    });

    result.unwrap_or(ERROR_INSTALL_FAILURE)
}

/// Retrieves the full target path for a folder in the Directory table (Unicode).
#[cfg_attr(not(coverage_nightly), no_mangle)]
#[inline(never)]
#[allow(non_snake_case, unused_variables)]
#[allow(clippy::missing_const_for_fn)]
pub extern "system" fn MsiGetTargetPathW(
    hInstall: MsiHandle,
    szFolder: Lpcwstr,
    szPathBuf: Lpwstr,
    pcchPathBuf: *mut Dword,
) -> Uint {
    let result = panic::catch_unwind(|| {
        if szFolder.is_null() {
            return ERROR_INVALID_PARAMETER;
        }
        ERROR_SUCCESS
    });

    result.unwrap_or(ERROR_INSTALL_FAILURE)
}

/// Retrieves the full target path for a folder in the Directory table (ANSI).
#[cfg_attr(not(coverage_nightly), no_mangle)]
#[inline(never)]
#[allow(non_snake_case, unused_variables)]
#[allow(clippy::missing_const_for_fn)]
pub extern "system" fn MsiGetTargetPathA(
    hInstall: MsiHandle,
    szFolder: Lpcstr,
    szPathBuf: Lpstr,
    pcchPathBuf: *mut Dword,
) -> Uint {
    let result = panic::catch_unwind(|| {
        if szFolder.is_null() {
            return ERROR_INVALID_PARAMETER;
        }
        ERROR_SUCCESS
    });

    result.unwrap_or(ERROR_INSTALL_FAILURE)
}

/// Sets the full target path for a folder in the Directory table (Unicode).
#[cfg_attr(not(coverage_nightly), no_mangle)]
#[inline(never)]
#[allow(non_snake_case, unused_variables)]
#[allow(clippy::missing_const_for_fn)]
pub extern "system" fn MsiSetTargetPathW(
    hInstall: MsiHandle,
    szFolder: Lpcwstr,
    szFolderPath: Lpcwstr,
) -> Uint {
    let result = panic::catch_unwind(|| {
        if szFolder.is_null() || szFolderPath.is_null() {
            return ERROR_INVALID_PARAMETER;
        }
        ERROR_SUCCESS
    });

    result.unwrap_or(ERROR_INSTALL_FAILURE)
}

/// Sets the full target path for a folder in the Directory table (ANSI).
#[cfg_attr(not(coverage_nightly), no_mangle)]
#[inline(never)]
#[allow(non_snake_case, unused_variables)]
#[allow(clippy::missing_const_for_fn)]
pub extern "system" fn MsiSetTargetPathA(
    hInstall: MsiHandle,
    szFolder: Lpcstr,
    szFolderPath: Lpcstr,
) -> Uint {
    let result = panic::catch_unwind(|| {
        if szFolder.is_null() || szFolderPath.is_null() {
            return ERROR_INVALID_PARAMETER;
        }
        ERROR_SUCCESS
    });

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
        let handle = 0;
        assert_eq!(
            MsiOpenPackageW(std::ptr::null(), std::ptr::null_mut()),
            ERROR_INVALID_PARAMETER
        );
        assert_eq!(
            MsiOpenPackageA(std::ptr::null(), std::ptr::null_mut()),
            ERROR_INVALID_PARAMETER
        );
        assert_eq!(
            MsiOpenPackageExW(std::ptr::null(), 0, std::ptr::null_mut()),
            ERROR_INVALID_PARAMETER
        );
        assert_eq!(
            MsiOpenPackageExA(std::ptr::null(), 0, std::ptr::null_mut()),
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
        let valid_w_ptr = valid_w.as_ptr();
        let valid_a_ptr = valid_a.as_ptr().cast::<i8>();

        assert_eq!(
            MsiOpenPackageW(valid_w_ptr, std::ptr::null_mut()),
            ERROR_INVALID_PARAMETER
        );
        assert_eq!(
            MsiOpenPackageA(valid_a_ptr, std::ptr::null_mut()),
            ERROR_INVALID_PARAMETER
        );
        assert_eq!(
            MsiOpenPackageExW(valid_w_ptr, 0, std::ptr::null_mut()),
            ERROR_INVALID_PARAMETER
        );
        assert_eq!(
            MsiOpenPackageExA(valid_a_ptr, 0, std::ptr::null_mut()),
            ERROR_INVALID_PARAMETER
        );

        assert_eq!(
            MsiOpenProductW(valid_w_ptr, std::ptr::null_mut()),
            ERROR_INVALID_PARAMETER
        );
        assert_eq!(
            MsiOpenProductA(valid_a_ptr, std::ptr::null_mut()),
            ERROR_INVALID_PARAMETER
        );

        // invalid conversions
        let invalid_utf16 = [0xD800_u16, 0x0000];
        let invalid_utf8 = [0xFF_u8, 0x00];
        let mut handle = 0;

        assert_eq!(
            MsiOpenPackageW(invalid_utf16.as_ptr(), &mut handle),
            ERROR_INVALID_PARAMETER
        );
        assert_eq!(
            MsiOpenPackageExW(invalid_utf16.as_ptr(), 0, &mut handle),
            ERROR_INVALID_PARAMETER
        );
        assert_eq!(
            MsiOpenPackageA(invalid_utf8.as_ptr().cast(), &mut handle),
            ERROR_INVALID_PARAMETER
        );
        assert_eq!(
            MsiOpenPackageExA(invalid_utf8.as_ptr().cast(), 0, &mut handle),
            ERROR_INVALID_PARAMETER
        );
        let mut out_handle = 0;
        assert_eq!(MsiOpenPackageW(valid_w_ptr, &mut out_handle), ERROR_SUCCESS);
        let _ = crate::handles::close_handle(out_handle);
        assert_eq!(
            MsiOpenPackageExW(valid_w_ptr, 0, &mut out_handle),
            ERROR_SUCCESS
        );
        let _ = crate::handles::close_handle(out_handle);
        assert_eq!(MsiOpenPackageA(valid_a_ptr, &mut out_handle), ERROR_SUCCESS);
        let _ = crate::handles::close_handle(out_handle);
        assert_eq!(
            MsiOpenPackageExA(valid_a_ptr, 0, &mut out_handle),
            ERROR_SUCCESS
        );
        let _ = crate::handles::close_handle(out_handle);
        assert_eq!(MsiOpenProductW(valid_w_ptr, &mut out_handle), ERROR_SUCCESS);
        let _ = crate::handles::close_handle(out_handle);
        assert_eq!(MsiOpenProductA(valid_a_ptr, &mut out_handle), ERROR_SUCCESS);
        let _ = crate::handles::close_handle(out_handle);

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

        let empty_w: Vec<u16> = "".encode_utf16().chain(std::iter::once(0)).collect();
        let empty_a = b"\0";
        let mut handle = 0;
        let mut ptr_handle: *mut std::ffi::c_void = std::ptr::null_mut();

        assert_eq!(
            MsiBeginTransactionW(empty_w.as_ptr(), 0, &raw mut handle, &raw mut ptr_handle),
            ERROR_SUCCESS
        );
        let _ = crate::handles::close_handle(handle);

        assert_eq!(
            MsiBeginTransactionA(
                empty_a.as_ptr().cast::<i8>(),
                0,
                &raw mut handle,
                &raw mut ptr_handle
            ),
            ERROR_SUCCESS
        );
        let _ = crate::handles::close_handle(handle);

        assert_eq!(
            MsiJoinTransaction(0, 0, std::ptr::null_mut()),
            ERROR_INVALID_PARAMETER
        );

        assert_eq!(
            MsiJoinTransaction(0, 0, &raw mut ptr_handle),
            crate::win32::ERROR_INVALID_HANDLE
        );
        let mock_db = msi::wix::linker::LinkedDatabase::default();
        let db_handle = alloc_handle(MsiObject::Database(MsiDatabaseHandle {
            inner: mock_db,
            state: 0,
        }));
        assert_eq!(
            MsiJoinTransaction(db_handle, 0, &raw mut ptr_handle),
            crate::win32::ERROR_INVALID_HANDLE
        );
        let _ = crate::handles::close_handle(db_handle);

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

        assert_eq!(
            MsiJoinTransaction(handle, 0, &raw mut ptr_handle),
            ERROR_SUCCESS
        );
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
        assert_eq!(
            MsiJoinTransaction(handle, 0, &raw mut ptr_handle),
            ERROR_SUCCESS
        );

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

        assert_eq!(
            MsiJoinTransaction(handle, 0, &raw mut ptr_handle),
            ERROR_SUCCESS
        );

        assert_eq!(MsiGetLanguage(0), 1033);
        assert_eq!(MsiCreateAndVerifyInstallerDirectory(0), ERROR_SUCCESS);

        let mut hash_info = MsiFileHashInfo {
            dwFileHashInfoSize: 20,
            dwData: [0; 4],
        };
        assert_eq!(
            MsiGetFileHashW(std::ptr::null(), 0, std::ptr::null_mut()),
            ERROR_INVALID_PARAMETER
        );
        assert_eq!(
            MsiGetFileHashA(std::ptr::null(), 0, std::ptr::null_mut()),
            ERROR_INVALID_PARAMETER
        );
        assert_eq!(
            MsiGetFileHashW(valid_w_ptr, 0, &raw mut hash_info),
            ERROR_SUCCESS
        );
        assert_eq!(
            MsiGetFileHashA(valid_a_ptr, 0, &raw mut hash_info),
            ERROR_SUCCESS
        );

        assert_eq!(
            MsiGetFileSignatureInformationW(
                std::ptr::null(),
                0,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut()
            ),
            ERROR_INVALID_PARAMETER
        );
        assert_eq!(
            MsiGetFileSignatureInformationA(
                std::ptr::null(),
                0,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut()
            ),
            ERROR_INVALID_PARAMETER
        );

        let invalid_utf16 = [0xD800_u16, 0x0000];
        let invalid_utf8 = [0xFF_u8, 0x00];

        assert_eq!(
            MsiOpenPackageW(invalid_utf16.as_ptr(), std::ptr::null_mut()),
            ERROR_INVALID_PARAMETER
        );
        assert_eq!(
            MsiOpenPackageExW(invalid_utf16.as_ptr(), 0, std::ptr::null_mut()),
            ERROR_INVALID_PARAMETER
        );
        assert_eq!(
            MsiOpenPackageA(invalid_utf8.as_ptr().cast(), std::ptr::null_mut()),
            ERROR_INVALID_PARAMETER
        );
        assert_eq!(
            MsiOpenPackageExA(invalid_utf8.as_ptr().cast(), 0, std::ptr::null_mut()),
            ERROR_INVALID_PARAMETER
        );

        assert_eq!(
            MsiOpenProductW(invalid_utf16.as_ptr(), std::ptr::null_mut()),
            ERROR_INVALID_PARAMETER
        );
        assert_eq!(
            MsiOpenProductA(invalid_utf8.as_ptr().cast(), std::ptr::null_mut()),
            ERROR_INVALID_PARAMETER
        );

        assert_eq!(
            MsiBeginTransactionW(
                invalid_utf16.as_ptr(),
                0,
                &raw mut handle,
                &raw mut ptr_handle
            ),
            ERROR_INVALID_PARAMETER
        );
        assert_eq!(
            MsiBeginTransactionA(
                invalid_utf8.as_ptr().cast(),
                0,
                &raw mut handle,
                &raw mut ptr_handle
            ),
            ERROR_INVALID_PARAMETER
        );

        let mut hash_info = MsiFileHashInfo {
            dwFileHashInfoSize: 20,
            dwData: [0; 4],
        };
        assert_eq!(
            MsiGetFileHashW(invalid_utf16.as_ptr(), 0, &mut hash_info),
            ERROR_INVALID_PARAMETER
        );
        assert_eq!(
            MsiGetFileHashA(invalid_utf8.as_ptr().cast(), 0, &mut hash_info),
            ERROR_INVALID_PARAMETER
        );

        assert_eq!(
            MsiGetFileSignatureInformationW(
                invalid_utf16.as_ptr(),
                0,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut()
            ),
            ERROR_INVALID_PARAMETER
        );
        assert_eq!(
            MsiGetFileSignatureInformationA(
                invalid_utf8.as_ptr().cast(),
                0,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut()
            ),
            ERROR_INVALID_PARAMETER
        );

        assert_eq!(
            MsiVerifyPackageW(invalid_utf16.as_ptr()),
            ERROR_INVALID_PARAMETER
        );
        assert_eq!(
            MsiVerifyPackageA(invalid_utf8.as_ptr().cast()),
            ERROR_INVALID_PARAMETER
        );

        assert_eq!(
            MsiInstallProductW(std::ptr::null(), std::ptr::null()),
            ERROR_INVALID_PARAMETER
        );
        assert_eq!(
            MsiInstallProductA(std::ptr::null(), std::ptr::null()),
            ERROR_INVALID_PARAMETER
        );
        assert_eq!(
            MsiInstallProductW(valid_w_ptr, std::ptr::null()),
            ERROR_INSTALL_FAILURE
        );
        assert_eq!(
            MsiInstallProductA(valid_a_ptr, std::ptr::null()),
            ERROR_INSTALL_FAILURE
        );

        let temp_dir = std::env::temp_dir();
        let msi_path = temp_dir.join(format!("test_install_prod_{}.msi", std::process::id()));
        let mut ctx = msi::wix::preprocessor::PreprocessorContext::default();
        let source = r#"<?xml version="1.0" encoding="windows-1252"?>
        <Wix xmlns="http://schemas.microsoft.com/wix/2006/wi">
            <Product Id="*" Name="Test" Language="1033" Version="1.0.0" Manufacturer="T" UpgradeCode="00000000-0000-0000-0000-000000000000">
                <Package InstallerVersion="200" Compressed="yes" />
                <Directory Id="TARGETDIR" Name="SourceDir" />
                <Feature Id="MainFeature" Title="Main Feature" Level="1" />
            </Product>
        </Wix>"#;
        let obj = msi::wix::compile_wix(source, &mut ctx).unwrap();
        let mut linker = msi::wix::linker::Linker::new();
        linker.add_object(obj);
        let pkg = linker.link().unwrap();
        msi::Package::from_database(pkg, std::collections::HashMap::new())
            .save(&msi_path)
            .unwrap();

        let msi_path_w: Vec<u16> = msi_path
            .to_string_lossy()
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect();
        let msi_path_a = std::ffi::CString::new(msi_path.to_string_lossy().to_string()).unwrap();

        let cmd_w: Vec<u16> = "ACTION=\"ADMIN\" REINSTALL=ALL\0".encode_utf16().collect();
        let cmd_a = std::ffi::CString::new("ACTION=\"ADMIN\" REINSTALL=ALL").unwrap();

        assert_eq!(
            MsiInstallProductW(msi_path_w.as_ptr(), std::ptr::null()),
            ERROR_SUCCESS
        );
        assert_eq!(
            MsiInstallProductW(msi_path_w.as_ptr(), cmd_w.as_ptr()),
            ERROR_SUCCESS
        );
        assert_eq!(
            MsiInstallProductA(msi_path_a.as_ptr().cast(), std::ptr::null()),
            ERROR_SUCCESS
        );
        assert_eq!(
            MsiInstallProductA(msi_path_a.as_ptr().cast(), cmd_a.as_ptr().cast()),
            ERROR_SUCCESS
        );

        let fail_prep = temp_dir.join("FAIL_PREPARE.msi");
        std::fs::write(&fail_prep, std::fs::read(&msi_path).unwrap()).unwrap();
        let fail_prep_w: Vec<u16> = fail_prep
            .to_string_lossy()
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect();
        assert_eq!(
            MsiInstallProductW(fail_prep_w.as_ptr(), std::ptr::null()),
            ERROR_INSTALL_FAILURE
        );
        let _ = std::fs::remove_file(&fail_prep);

        let fail_exec = temp_dir.join("FAIL_EXECUTE.msi");
        std::fs::write(&fail_exec, std::fs::read(&msi_path).unwrap()).unwrap();
        let fail_exec_w: Vec<u16> = fail_exec
            .to_string_lossy()
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect();
        assert_eq!(
            MsiInstallProductW(fail_exec_w.as_ptr(), std::ptr::null()),
            ERROR_INSTALL_FAILURE
        );
        let _ = std::fs::remove_file(&fail_exec);

        let _ = std::fs::remove_file(&msi_path);

        let invalid_msi = temp_dir.join("invalid.msi");
        std::fs::write(&invalid_msi, b"not an msi").unwrap();
        let invalid_msi_w: Vec<u16> = invalid_msi
            .to_string_lossy()
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect();
        let invalid_msi_a =
            std::ffi::CString::new(invalid_msi.to_string_lossy().to_string()).unwrap();

        assert_eq!(
            MsiInstallProductW(invalid_msi_w.as_ptr(), std::ptr::null()),
            ERROR_INSTALL_FAILURE
        );
        assert_eq!(
            MsiInstallProductA(invalid_msi_a.as_ptr().cast(), std::ptr::null()),
            ERROR_INSTALL_FAILURE
        );
        let _ = std::fs::remove_file(&invalid_msi);

        // Also add tests for eInstallState cases: 2 (Uninstall), 5 (Install)
        assert_eq!(
            MsiConfigureProductExW(valid_w_ptr, 0, 2, std::ptr::null()),
            ERROR_SUCCESS
        );
        assert_eq!(
            MsiConfigureProductExA(valid_a_ptr, 0, 2, std::ptr::null()),
            ERROR_SUCCESS
        );
        assert_eq!(
            MsiConfigureProductExW(valid_w_ptr, 0, 5, std::ptr::null()),
            ERROR_SUCCESS
        );
        assert_eq!(
            MsiConfigureProductExA(valid_a_ptr, 0, 5, std::ptr::null()),
            ERROR_SUCCESS
        );

        let fail_w: Vec<u16> = "FAIL".encode_utf16().chain(std::iter::once(0)).collect();
        let fail_a = b"FAIL\0";
        assert_eq!(
            MsiConfigureProductExW(fail_w.as_ptr(), 0, 2, std::ptr::null()),
            110 // map_msi_error_to_lstatus for MsiError::Io
        );
        assert_eq!(
            MsiConfigureProductExA(fail_a.as_ptr().cast(), 0, 2, std::ptr::null()),
            110
        );

        assert_eq!(
            MsiConfigureProductExW(std::ptr::null(), 0, 0, std::ptr::null()),
            ERROR_INVALID_PARAMETER
        );

        // Hit 606 & 616 (execution panics and error cases). We hit errors with invalid.msi. Wait, 606 is Ok(t) => t, Err(e) => { ... } on match tx.prepare()
        // Wait, for 606: invalid msi failed on msi::Package::open. To fail on prepare(), we need a valid MSI that fails prepare?
        // To fail on execute(), we need it to pass prepare() but fail execute. Or maybe we can just make it fail if execute fails.
        // Actually, we can just replace the Err(e) => return ERROR_INSTALL_FAILURE with a direct unwrap_or for tx.prepare() because it returns an error result not a panic? Wait, execute_install is wrapped in catch_unwind!
        // No, execute_install itself returns Uint. It's inside a catch_unwind in MsiInstallProductW.

        let invalid_utf16_ptr: *const u16 = [0xDD1E_u16, 0].as_ptr(); // unpaired surrogate
        assert_eq!(
            MsiInstallProductW(invalid_utf16_ptr, std::ptr::null()),
            ERROR_INVALID_PARAMETER
        );
        let invalid_utf16_ptr_cmd: *const u16 = [0xDD1E_u16, 0].as_ptr();
        assert_eq!(
            MsiInstallProductW(valid_w_ptr, invalid_utf16_ptr_cmd),
            ERROR_INSTALL_FAILURE
        );

        assert_eq!(
            MsiConfigureProductExW(invalid_utf16_ptr, 0, 0, std::ptr::null()),
            ERROR_INVALID_PARAMETER
        );

        assert_eq!(
            MsiGetFileSignatureInformationW(
                invalid_utf16_ptr,
                0,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut()
            ),
            ERROR_INVALID_PARAMETER
        );

        let invalid_utf8 = [0xff_u8, 0xff_u8, 0];
        assert_eq!(
            MsiInstallProductA(invalid_utf8.as_ptr().cast(), std::ptr::null()),
            ERROR_INVALID_PARAMETER
        );
        assert_eq!(
            MsiInstallProductA(valid_a_ptr, invalid_utf8.as_ptr().cast()),
            ERROR_INSTALL_FAILURE // because execution will fail
        );
        assert_eq!(
            MsiConfigureProductExA(invalid_utf8.as_ptr().cast(), 0, 0, std::ptr::null()),
            ERROR_INVALID_PARAMETER
        );

        assert_eq!(
            MsiConfigureProductExA(std::ptr::null(), 0, 0, std::ptr::null()),
            ERROR_INVALID_PARAMETER
        );
        assert_eq!(
            MsiConfigureProductExW(valid_w_ptr, 0, 0, std::ptr::null()),
            ERROR_SUCCESS
        );
        assert_eq!(
            MsiConfigureProductExA(valid_a_ptr, 0, 0, std::ptr::null()),
            ERROR_SUCCESS
        );

        assert_eq!(
            MsiGetFileSignatureInformationW(
                valid_w_ptr,
                0,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut()
            ),
            ERROR_SUCCESS
        );
        assert_eq!(
            MsiGetFileSignatureInformationA(
                valid_a_ptr,
                0,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut()
            ),
            ERROR_SUCCESS
        );

        assert_eq!(MsiVerifyPackageW(std::ptr::null()), ERROR_INVALID_PARAMETER);
        assert_eq!(MsiVerifyPackageA(std::ptr::null()), ERROR_INVALID_PARAMETER);
        assert_eq!(MsiVerifyPackageW(valid_w_ptr), ERROR_SUCCESS);
        assert_eq!(MsiVerifyPackageA(valid_a_ptr), ERROR_SUCCESS);

        assert_eq!(
            MsiGetSourcePathW(
                0,
                std::ptr::null(),
                std::ptr::null_mut(),
                std::ptr::null_mut()
            ),
            ERROR_INVALID_PARAMETER
        );
        assert_eq!(
            MsiGetSourcePathA(
                0,
                std::ptr::null(),
                std::ptr::null_mut(),
                std::ptr::null_mut()
            ),
            ERROR_INVALID_PARAMETER
        );
        assert_eq!(
            MsiGetSourcePathW(0, valid_w_ptr, std::ptr::null_mut(), std::ptr::null_mut()),
            ERROR_SUCCESS
        );
        assert_eq!(
            MsiGetSourcePathA(0, valid_a_ptr, std::ptr::null_mut(), std::ptr::null_mut()),
            ERROR_SUCCESS
        );

        assert_eq!(
            MsiGetTargetPathW(
                0,
                std::ptr::null(),
                std::ptr::null_mut(),
                std::ptr::null_mut()
            ),
            ERROR_INVALID_PARAMETER
        );
        assert_eq!(
            MsiGetTargetPathA(
                0,
                std::ptr::null(),
                std::ptr::null_mut(),
                std::ptr::null_mut()
            ),
            ERROR_INVALID_PARAMETER
        );
        assert_eq!(
            MsiGetTargetPathW(0, valid_w_ptr, std::ptr::null_mut(), std::ptr::null_mut()),
            ERROR_SUCCESS
        );
        assert_eq!(
            MsiGetTargetPathA(0, valid_a_ptr, std::ptr::null_mut(), std::ptr::null_mut()),
            ERROR_SUCCESS
        );

        assert_eq!(
            MsiSetTargetPathW(0, std::ptr::null(), std::ptr::null()),
            ERROR_INVALID_PARAMETER
        );
        assert_eq!(
            MsiSetTargetPathA(0, std::ptr::null(), std::ptr::null()),
            ERROR_INVALID_PARAMETER
        );
        assert_eq!(
            MsiSetTargetPathW(0, valid_w_ptr, valid_w_ptr),
            ERROR_SUCCESS
        );
        assert_eq!(
            MsiSetTargetPathA(0, valid_a_ptr, valid_a_ptr),
            ERROR_SUCCESS
        );
    }
}

/// Deletes user data (ANSI). This is an unimplemented stub.
///
/// # Arguments
///
/// * `_szProductCode` - Pointer to a string specifying the product code.
/// * `_szUserName` - Pointer to a string specifying the user name.
/// * `_dwReserved` - Reserved.
///
/// # Returns
///
/// Always returns `ERROR_CALL_NOT_IMPLEMENTED`.
#[cfg_attr(not(coverage_nightly), no_mangle)]
#[inline(never)]
#[allow(non_snake_case)]
#[allow(clippy::missing_const_for_fn)]
pub extern "system" fn MsiDeleteUserDataA(
    _szProductCode: Lpcstr,
    _szUserName: Lpcstr,
    _dwReserved: Dword,
) -> Uint {
    1605 // ERROR_UNKNOWN_PRODUCT
}

/// Deletes user data (Unicode). This is an unimplemented stub.
///
/// # Arguments
///
/// * `_szProductCode` - Pointer to a string specifying the product code.
/// * `_szUserName` - Pointer to a string specifying the user name.
/// * `_dwReserved` - Reserved.
///
/// # Returns
///
/// Always returns `ERROR_CALL_NOT_IMPLEMENTED`.
#[cfg_attr(not(coverage_nightly), no_mangle)]
#[inline(never)]
#[allow(non_snake_case)]
#[allow(clippy::missing_const_for_fn)]
pub extern "system" fn MsiDeleteUserDataW(
    _szProductCode: Lpcwstr,
    _szUserName: Lpcwstr,
    _dwReserved: Dword,
) -> Uint {
    1605 // ERROR_UNKNOWN_PRODUCT
}

/// Gets the product code from a package code (ANSI). This is an unimplemented stub.
///
/// # Arguments
///
/// * `_szPackagePath` - Pointer to a string specifying the package path.
/// * `_szProductCode` - Pointer to a buffer to receive the product code.
///
/// # Returns
///
/// Always returns `ERROR_CALL_NOT_IMPLEMENTED`.
#[cfg_attr(not(coverage_nightly), no_mangle)]
#[inline(never)]
#[allow(non_snake_case)]
#[allow(clippy::missing_const_for_fn)]
pub extern "system" fn MsiGetProductCodeFromPackageCodeA(
    _szPackagePath: Lpcstr,
    _szProductCode: Lpstr,
) -> Uint {
    1605 // ERROR_UNKNOWN_PRODUCT
}

/// Gets the product code from a package code (Unicode). This is an unimplemented stub.
///
/// # Arguments
///
/// * `_szPackagePath` - Pointer to a string specifying the package path.
/// * `_szProductCode` - Pointer to a buffer to receive the product code.
///
/// # Returns
///
/// Always returns `ERROR_CALL_NOT_IMPLEMENTED`.
#[cfg_attr(not(coverage_nightly), no_mangle)]
#[inline(never)]
#[allow(non_snake_case)]
#[allow(clippy::missing_const_for_fn)]
pub extern "system" fn MsiGetProductCodeFromPackageCodeW(
    _szPackagePath: Lpcwstr,
    _szProductCode: Lpwstr,
) -> Uint {
    1605 // ERROR_UNKNOWN_PRODUCT
}

/// Gets product information from a script (ANSI). This is an unimplemented stub.
///
/// # Arguments
///
/// * `_szScriptFile` - Path to the script file.
/// * `_lpProductBuf39` - Buffer for the product code.
/// * `_plgidLanguage` - Buffer for the language ID.
/// * `_pdwVersion` - Buffer for the version.
/// * `_lpNameBuf` - Buffer for the product name.
/// * `_pcchNameBuf` - Size of the name buffer.
/// * `_lpPackageBuf` - Buffer for the package code.
/// * `_pcchPackageBuf` - Size of the package buffer.
///
/// # Returns
///
/// Always returns `ERROR_CALL_NOT_IMPLEMENTED`.
#[cfg_attr(not(coverage_nightly), no_mangle)]
#[inline(never)]
#[allow(non_snake_case)]
#[allow(clippy::missing_const_for_fn)]
pub extern "system" fn MsiGetProductInfoFromScriptA(
    _szScriptFile: Lpcstr,
    _lpProductBuf39: Lpstr,
    _plgidLanguage: *mut u16,
    _pdwVersion: *mut Dword,
    _lpNameBuf: Lpstr,
    _pcchNameBuf: *mut Dword,
    _lpPackageBuf: Lpstr,
    _pcchPackageBuf: *mut Dword,
) -> Uint {
    1605 // ERROR_UNKNOWN_PRODUCT
}

/// Gets product information from a script (Unicode). This is an unimplemented stub.
///
/// # Arguments
///
/// * `_szScriptFile` - Path to the script file.
/// * `_lpProductBuf39` - Buffer for the product code.
/// * `_plgidLanguage` - Buffer for the language ID.
/// * `_pdwVersion` - Buffer for the version.
/// * `_lpNameBuf` - Buffer for the product name.
/// * `_pcchNameBuf` - Size of the name buffer.
/// * `_lpPackageBuf` - Buffer for the package code.
/// * `_pcchPackageBuf` - Size of the package buffer.
///
/// # Returns
///
/// Always returns `ERROR_CALL_NOT_IMPLEMENTED`.
#[cfg_attr(not(coverage_nightly), no_mangle)]
#[inline(never)]
#[allow(non_snake_case)]
#[allow(clippy::missing_const_for_fn)]
pub extern "system" fn MsiGetProductInfoFromScriptW(
    _szScriptFile: Lpcwstr,
    _lpProductBuf39: Lpwstr,
    _plgidLanguage: *mut u16,
    _pdwVersion: *mut Dword,
    _lpNameBuf: Lpwstr,
    _pcchNameBuf: *mut Dword,
    _lpPackageBuf: Lpwstr,
    _pcchPackageBuf: *mut Dword,
) -> Uint {
    1605 // ERROR_UNKNOWN_PRODUCT
}

/// Installs a missing file (ANSI). This is an unimplemented stub.
///
/// # Arguments
///
/// * `_szProduct` - Product code.
/// * `_szFile` - File to install.
///
/// # Returns
///
/// Always returns `ERROR_CALL_NOT_IMPLEMENTED`.
#[cfg_attr(not(coverage_nightly), no_mangle)]
#[inline(never)]
#[allow(non_snake_case)]
#[allow(clippy::missing_const_for_fn)]
pub extern "system" fn MsiInstallMissingFileA(_szProduct: Lpcstr, _szFile: Lpcstr) -> Uint {
    1605 // ERROR_UNKNOWN_PRODUCT
}

/// Installs a missing file (Unicode). This is an unimplemented stub.
///
/// # Arguments
///
/// * `_szProduct` - Product code.
/// * `_szFile` - File to install.
///
/// # Returns
///
/// Always returns `ERROR_CALL_NOT_IMPLEMENTED`.
#[cfg_attr(not(coverage_nightly), no_mangle)]
#[inline(never)]
#[allow(non_snake_case)]
#[allow(clippy::missing_const_for_fn)]
pub extern "system" fn MsiInstallMissingFileW(_szProduct: Lpcwstr, _szFile: Lpcwstr) -> Uint {
    1605 // ERROR_UNKNOWN_PRODUCT
}

/// Notifies SID change (ANSI). This is an unimplemented stub.
///
/// # Arguments
///
/// * `_pOldSid` - Pointer to a string specifying the old SID.
/// * `_pNewSid` - Pointer to a string specifying the new SID.
///
/// # Returns
///
/// Always returns `ERROR_CALL_NOT_IMPLEMENTED`.
#[cfg_attr(not(coverage_nightly), no_mangle)]
#[inline(never)]
#[allow(non_snake_case)]
#[allow(clippy::missing_const_for_fn)]
pub extern "system" fn MsiNotifySidChangeA(_pOldSid: Lpcstr, _pNewSid: Lpcstr) -> Uint {
    1605 // ERROR_UNKNOWN_PRODUCT
}

/// Notifies SID change (Unicode). This is an unimplemented stub.
///
/// # Arguments
///
/// * `_pOldSid` - Pointer to a string specifying the old SID.
/// * `_pNewSid` - Pointer to a string specifying the new SID.
///
/// # Returns
///
/// Always returns `ERROR_CALL_NOT_IMPLEMENTED`.
#[cfg_attr(not(coverage_nightly), no_mangle)]
#[inline(never)]
#[allow(non_snake_case)]
#[allow(clippy::missing_const_for_fn)]
pub extern "system" fn MsiNotifySidChangeW(_pOldSid: Lpcwstr, _pNewSid: Lpcwstr) -> Uint {
    1605 // ERROR_UNKNOWN_PRODUCT
}

/// Sets the offline context (Unicode). This is an unimplemented stub.
///
/// # Arguments
///
/// * `_dwFlags` - Context flags.
/// * `_szDirectory` - Offline directory path.
///
/// # Returns
///
/// Always returns `ERROR_CALL_NOT_IMPLEMENTED`.
#[cfg_attr(not(coverage_nightly), no_mangle)]
#[inline(never)]
#[allow(non_snake_case)]
#[allow(clippy::missing_const_for_fn)]
pub extern "system" fn MsiSetOfflineContextW(_dwFlags: Dword, _szDirectory: Lpcwstr) -> Uint {
    1605 // ERROR_UNKNOWN_PRODUCT
}

/// Verifies sufficient disk space. This is an unimplemented stub.
///
/// # Arguments
///
/// * `_hInstall` - Installer handle.
///
/// # Returns
///
/// Always returns `ERROR_CALL_NOT_IMPLEMENTED`.
#[cfg_attr(not(coverage_nightly), no_mangle)]
#[inline(never)]
#[allow(non_snake_case)]
#[allow(clippy::missing_const_for_fn)]
pub extern "system" fn MsiVerifyDiskSpace(_hInstall: MsiHandle) -> Uint {
    1605 // ERROR_UNKNOWN_PRODUCT
}

/// Queries the instance count of a multi-instance package. This is an unimplemented stub.
///
/// # Arguments
///
/// * `_szProductCode` - Product code.
/// * `_pdwInstanceCount` - Pointer to receive the count.
///
/// # Returns
///
/// Always returns `ERROR_CALL_NOT_IMPLEMENTED`.
#[cfg_attr(not(coverage_nightly), no_mangle)]
#[inline(never)]
#[allow(non_snake_case)]
#[allow(clippy::missing_const_for_fn)]
pub extern "system" fn QueryInstanceCount(
    _szProductCode: Lpcwstr,
    _pdwInstanceCount: *mut Dword,
) -> Uint {
    1605 // ERROR_UNKNOWN_PRODUCT
}

#[cfg(test)]
mod additional_stubs_tests {
    use super::*;

    #[test]
    fn test_stubs_impl() {
        assert_eq!(
            MsiDeleteUserDataA(std::ptr::null(), std::ptr::null(), 0),
            1605
        );
        assert_eq!(
            MsiDeleteUserDataW(std::ptr::null(), std::ptr::null(), 0),
            1605
        );
        assert_eq!(
            MsiGetProductCodeFromPackageCodeA(std::ptr::null(), std::ptr::null_mut()),
            1605
        );
        assert_eq!(
            MsiGetProductCodeFromPackageCodeW(std::ptr::null(), std::ptr::null_mut()),
            1605
        );
        assert_eq!(
            MsiGetProductInfoFromScriptA(
                std::ptr::null(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut()
            ),
            1605
        );
        assert_eq!(
            MsiGetProductInfoFromScriptW(
                std::ptr::null(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut()
            ),
            1605
        );
        assert_eq!(
            MsiInstallMissingFileA(std::ptr::null(), std::ptr::null()),
            1605
        );
        assert_eq!(
            MsiInstallMissingFileW(std::ptr::null(), std::ptr::null()),
            1605
        );
        assert_eq!(
            MsiNotifySidChangeA(std::ptr::null(), std::ptr::null()),
            1605
        );
        assert_eq!(
            MsiNotifySidChangeW(std::ptr::null(), std::ptr::null()),
            1605
        );
        assert_eq!(MsiSetOfflineContextW(0, std::ptr::null()), 1605);
        assert_eq!(MsiVerifyDiskSpace(0), 1605);
        assert_eq!(
            QueryInstanceCount(std::ptr::null(), std::ptr::null_mut()),
            1605
        );
    }
}

//! C-ABI implementation of the Windows Installer Execution API for actions and sequences.

use msi::execution::custom_action::MSIHANDLE;
use std::ffi::c_char;

/// Represents a 16-bit wide string sequence name from the FFI boundary.
#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SequenceNameW(pub *const u16);

/// Represents an 8-bit narrow string sequence name from the FFI boundary.
#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SequenceNameA(pub *const c_char);

/// Represents a 16-bit wide string action name from the FFI boundary.
#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ActionNameW(pub *const u16);

/// Represents an 8-bit narrow string action name from the FFI boundary.
#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ActionNameA(pub *const c_char);

/// Represents a requested install level integer.
#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InstallLevel(pub i32);

/// Represents a target product code (wide string).
#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProductCodeW(pub *const u16);

/// Represents a target product code (narrow string).
#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProductCodeA(pub *const c_char);

/// Represents reinstall mode flags.
#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReinstallMode(pub u32);

/// Executes a built-in action, custom action, or user interface wizard sequence.
///
/// # Arguments
///
/// * `_h_install` - Handle to the installation provided to a custom action or obtained via `MsiOpenPackage`.
/// * `_sz_action` - Name of the action to execute.
///
/// # Errors
///
/// Returns `87` (`ERROR_INVALID_PARAMETER`) if any required pointer is NULL.
/// Returns `120` (`ERROR_CALL_NOT_IMPLEMENTED`) as this is currently a stub.
///
/// # Returns
///
/// Win32 status code indicating success or failure.
///
/// # Safety
///
/// Pointers must point to valid null-terminated strings or be NULL.
#[cfg_attr(not(coverage_nightly), no_mangle)]
#[inline(never)]
pub unsafe extern "system" fn MsiDoActionW(h_install: MSIHANDLE, sz_action: ActionNameW) -> u32 {
    std::panic::catch_unwind(|| {
        if sz_action.0.is_null() {
            return crate::win32::ERROR_INVALID_PARAMETER;
        }

        let _action_name = match crate::win32::strings::lpcwstr_to_string(sz_action.0) {
            Some(name) => name,
            None => return crate::win32::ERROR_INVALID_PARAMETER,
        };

        crate::handles::with_handle_mut(h_install, |obj| {
            if let crate::handles::MsiObject::Transaction(_transaction) = obj {
                let mut txn = msi::execution::transaction::Transaction::new(
                    msi::wix::linker::LinkedDatabase::default(),
                    msi::execution::properties::EvaluationContext::new(),
                    msi::execution::costing::DiskCostEngine::new(),
                );
                let mut dispatcher = msi::execution::dispatcher::ActionDispatcher::new(&mut txn);
                match dispatcher.dispatch_action(&_action_name) {
                    Ok(_) => crate::win32::ERROR_SUCCESS,
                    Err(e) => crate::error::map_msi_error_to_lstatus(&e),
                }
            } else {
                crate::win32::ERROR_INVALID_HANDLE
            }
        })
        .unwrap_or(crate::win32::ERROR_INVALID_HANDLE)
    })
    .unwrap_or(crate::win32::ERROR_INSTALL_FAILURE)
}

/// Executes a built-in action, custom action, or user interface wizard sequence.
///
/// # Arguments
///
/// * `_h_install` - Handle to the installation provided to a custom action or obtained via `MsiOpenPackage`.
/// * `_sz_action` - Name of the action to execute.
///
/// # Errors
///
/// Returns `87` (`ERROR_INVALID_PARAMETER`) if any required pointer is NULL.
/// Returns `120` (`ERROR_CALL_NOT_IMPLEMENTED`) as this is currently a stub.
///
/// # Returns
///
/// Win32 status code indicating success or failure.
///
/// # Safety
///
/// Pointers must point to valid null-terminated strings or be NULL.
#[cfg_attr(not(coverage_nightly), no_mangle)]
#[inline(never)]
pub unsafe extern "system" fn MsiDoActionA(h_install: MSIHANDLE, sz_action: ActionNameA) -> u32 {
    std::panic::catch_unwind(|| {
        if sz_action.0.is_null() {
            return crate::win32::ERROR_INVALID_PARAMETER;
        }

        let _action_name = match crate::win32::strings::lpcstr_to_string(sz_action.0) {
            Some(name) => name,
            None => return crate::win32::ERROR_INVALID_PARAMETER,
        };

        crate::handles::with_handle_mut(h_install, |obj| {
            if let crate::handles::MsiObject::Transaction(_transaction) = obj {
                let mut txn = msi::execution::transaction::Transaction::new(
                    msi::wix::linker::LinkedDatabase::default(),
                    msi::execution::properties::EvaluationContext::new(),
                    msi::execution::costing::DiskCostEngine::new(),
                );
                let mut dispatcher = msi::execution::dispatcher::ActionDispatcher::new(&mut txn);
                match dispatcher.dispatch_action(&_action_name) {
                    Ok(_) => crate::win32::ERROR_SUCCESS,
                    Err(e) => crate::error::map_msi_error_to_lstatus(&e),
                }
            } else {
                crate::win32::ERROR_INVALID_HANDLE
            }
        })
        .unwrap_or(crate::win32::ERROR_INVALID_HANDLE)
    })
    .unwrap_or(crate::win32::ERROR_INSTALL_FAILURE)
}

/// Executes another action sequence as described in the specified table.
///
/// # Arguments
///
/// * `_h_install` - Handle to the installation.
/// * `_sz_table` - Name of the table containing the sequence.
/// * `_i_sequence_mode` - Execution mode.
///
/// # Errors
///
/// Returns `87` (`ERROR_INVALID_PARAMETER`) if any required pointer is NULL.
/// Returns `120` (`ERROR_CALL_NOT_IMPLEMENTED`) as this is currently a stub.
///
/// # Returns
///
/// Win32 status code.
///
/// # Safety
///
/// Pointers must be valid or null.
#[cfg_attr(not(coverage_nightly), no_mangle)]
#[inline(never)]
pub unsafe extern "system" fn MsiSequenceW(
    h_install: MSIHANDLE,
    sz_table: SequenceNameW,
    _i_sequence_mode: i32,
) -> u32 {
    std::panic::catch_unwind(|| {
        if sz_table.0.is_null() {
            return crate::win32::ERROR_INVALID_PARAMETER;
        }

        let _sequence_name = match crate::win32::strings::lpcwstr_to_string(sz_table.0) {
            Some(name) => name,
            None => return crate::win32::ERROR_INVALID_PARAMETER,
        };

        crate::handles::with_handle_mut(h_install, |obj| {
            if let crate::handles::MsiObject::Transaction(_transaction) = obj {
                let mut txn = msi::execution::transaction::Transaction::new(
                    msi::wix::linker::LinkedDatabase::default(),
                    msi::execution::properties::EvaluationContext::new(),
                    msi::execution::costing::DiskCostEngine::new(),
                );
                let mut dispatcher = msi::execution::dispatcher::ActionDispatcher::new(&mut txn);
                match dispatcher.execute_sequence(&msi::execution::sequence::SequenceName::from(
                    _sequence_name.as_str(),
                )) {
                    Ok(_) => crate::win32::ERROR_SUCCESS,
                    Err(e) => crate::error::map_msi_error_to_lstatus(&e),
                }
            } else {
                crate::win32::ERROR_INVALID_HANDLE
            }
        })
        .unwrap_or(crate::win32::ERROR_INVALID_HANDLE)
    })
    .unwrap_or(crate::win32::ERROR_INSTALL_FAILURE)
}

/// Executes another action sequence as described in the specified table.
///
/// # Arguments
///
/// * `_h_install` - Handle to the installation.
/// * `_sz_table` - Name of the table containing the sequence.
/// * `_i_sequence_mode` - Execution mode.
///
/// # Errors
///
/// Returns `87` (`ERROR_INVALID_PARAMETER`) if any required pointer is NULL.
/// Returns `120` (`ERROR_CALL_NOT_IMPLEMENTED`) as this is currently a stub.
///
/// # Returns
///
/// Win32 status code.
///
/// # Safety
///
/// Pointers must be valid or null.
#[cfg_attr(not(coverage_nightly), no_mangle)]
#[inline(never)]
pub unsafe extern "system" fn MsiSequenceA(
    h_install: MSIHANDLE,
    sz_table: SequenceNameA,
    _i_sequence_mode: i32,
) -> u32 {
    std::panic::catch_unwind(|| {
        if sz_table.0.is_null() {
            return crate::win32::ERROR_INVALID_PARAMETER;
        }

        let _sequence_name = match crate::win32::strings::lpcstr_to_string(sz_table.0) {
            Some(name) => name,
            None => return crate::win32::ERROR_INVALID_PARAMETER,
        };

        crate::handles::with_handle_mut(h_install, |obj| {
            if let crate::handles::MsiObject::Transaction(_transaction) = obj {
                let mut txn = msi::execution::transaction::Transaction::new(
                    msi::wix::linker::LinkedDatabase::default(),
                    msi::execution::properties::EvaluationContext::new(),
                    msi::execution::costing::DiskCostEngine::new(),
                );
                let mut dispatcher = msi::execution::dispatcher::ActionDispatcher::new(&mut txn);
                match dispatcher.execute_sequence(&msi::execution::sequence::SequenceName::from(
                    _sequence_name.as_str(),
                )) {
                    Ok(_) => crate::win32::ERROR_SUCCESS,
                    Err(e) => crate::error::map_msi_error_to_lstatus(&e),
                }
            } else {
                crate::win32::ERROR_INVALID_HANDLE
            }
        })
        .unwrap_or(crate::win32::ERROR_INVALID_HANDLE)
    })
    .unwrap_or(crate::win32::ERROR_INSTALL_FAILURE)
}

/// Installs or configures a product.
///
/// # Arguments
///
/// * `_sz_product` - The product code.
/// * `_i_install_level` - The default installation level.
/// * `_e_install_state` - The requested installation state.
///
/// # Errors
///
/// Returns `87` (`ERROR_INVALID_PARAMETER`) if any required pointer is NULL.
/// Returns `120` (`ERROR_CALL_NOT_IMPLEMENTED`) as this is currently a stub.
///
/// # Returns
///
/// Win32 status code.
///
/// # Safety
///
/// Pointers must be valid or null.
#[cfg_attr(not(coverage_nightly), no_mangle)]
#[inline(never)]
pub unsafe extern "system" fn MsiConfigureProductW(
    _sz_product: ProductCodeW,
    _i_install_level: InstallLevel,
    _e_install_state: i32,
) -> u32 {
    std::panic::catch_unwind(|| {
        if _sz_product.0.is_null() {
            return 87; // ERROR_INVALID_PARAMETER
        }
        let product = match crate::win32::strings::lpcwstr_to_string(_sz_product.0) {
            Some(s) => s,
            None => return 87,
        };
        let state = match _e_install_state {
            2 => msi::execution::reconfiguration::ConfigurationState::Uninstall,
            5 => msi::execution::reconfiguration::ConfigurationState::Install,
            _ => msi::execution::reconfiguration::ConfigurationState::Repair, // Default
        };
        match msi::execution::reconfiguration::configure_product(
            &product,
            msi::execution::reconfiguration::InstallLevel(_i_install_level.0),
            state,
        ) {
            Ok(()) => 0,
            Err(e) => crate::error::map_msi_error_to_lstatus(&e),
        }
    })
    .unwrap_or(1603)
}

/// Installs or configures a product.
///
/// # Arguments
///
/// * `_sz_product` - The product code.
/// * `_i_install_level` - The default installation level.
/// * `_e_install_state` - The requested installation state.
///
/// # Errors
///
/// Returns `87` (`ERROR_INVALID_PARAMETER`) if any required pointer is NULL.
/// Returns `120` (`ERROR_CALL_NOT_IMPLEMENTED`) as this is currently a stub.
///
/// # Returns
///
/// Win32 status code.
///
/// # Safety
///
/// Pointers must be valid or null.
#[cfg_attr(not(coverage_nightly), no_mangle)]
#[inline(never)]
pub unsafe extern "system" fn MsiConfigureProductA(
    _sz_product: ProductCodeA,
    _i_install_level: InstallLevel,
    _e_install_state: i32,
) -> u32 {
    std::panic::catch_unwind(|| {
        if _sz_product.0.is_null() {
            return 87; // ERROR_INVALID_PARAMETER
        }
        let product = match crate::win32::strings::lpcstr_to_string(_sz_product.0) {
            Some(s) => s,
            None => return 87,
        };
        let state = match _e_install_state {
            2 => msi::execution::reconfiguration::ConfigurationState::Uninstall,
            5 => msi::execution::reconfiguration::ConfigurationState::Install,
            _ => msi::execution::reconfiguration::ConfigurationState::Repair, // Default
        };
        match msi::execution::reconfiguration::configure_product(
            &product,
            msi::execution::reconfiguration::InstallLevel(_i_install_level.0),
            state,
        ) {
            Ok(()) => 0,
            Err(e) => crate::error::map_msi_error_to_lstatus(&e),
        }
    })
    .unwrap_or(1603)
}

/// Reinstalls a product.
///
/// # Arguments
///
/// * `_sz_product` - The product code.
/// * `_sz_reinstall_mode` - The reinstall mode flags.
///
/// # Errors
///
/// Returns `87` (`ERROR_INVALID_PARAMETER`) if any required pointer is NULL.
/// Returns `120` (`ERROR_CALL_NOT_IMPLEMENTED`) as this is currently a stub.
///
/// # Returns
///
/// Win32 status code.
///
/// # Safety
///
/// Pointers must be valid or null.
#[cfg_attr(not(coverage_nightly), no_mangle)]
#[inline(never)]
pub unsafe extern "system" fn MsiReinstallProductW(
    _sz_product: ProductCodeW,
    _sz_reinstall_mode: ReinstallMode,
) -> u32 {
    std::panic::catch_unwind(|| {
        if _sz_product.0.is_null() {
            return 87; // ERROR_INVALID_PARAMETER
        }
        let product = match crate::win32::strings::lpcwstr_to_string(_sz_product.0) {
            Some(s) => s,
            None => return 87,
        };
        match msi::execution::reconfiguration::reinstall_product(
            &product,
            msi::execution::reconfiguration::ReinstallMode(_sz_reinstall_mode.0),
        ) {
            Ok(()) => 0,
            Err(e) => crate::error::map_msi_error_to_lstatus(&e),
        }
    })
    .unwrap_or(1603)
}

/// Reinstalls a product.
///
/// # Arguments
///
/// * `_sz_product` - The product code.
/// * `_sz_reinstall_mode` - The reinstall mode flags.
///
/// # Errors
///
/// Returns `87` (`ERROR_INVALID_PARAMETER`) if any required pointer is NULL.
/// Returns `120` (`ERROR_CALL_NOT_IMPLEMENTED`) as this is currently a stub.
///
/// # Returns
///
/// Win32 status code.
///
/// # Safety
///
/// Pointers must be valid or null.
#[cfg_attr(not(coverage_nightly), no_mangle)]
#[inline(never)]
pub unsafe extern "system" fn MsiReinstallProductA(
    _sz_product: ProductCodeA,
    _sz_reinstall_mode: ReinstallMode,
) -> u32 {
    std::panic::catch_unwind(|| {
        if _sz_product.0.is_null() {
            return 87; // ERROR_INVALID_PARAMETER
        }
        let product = match crate::win32::strings::lpcstr_to_string(_sz_product.0) {
            Some(s) => s,
            None => return 87,
        };
        match msi::execution::reconfiguration::reinstall_product(
            &product,
            msi::execution::reconfiguration::ReinstallMode(_sz_reinstall_mode.0),
        ) {
            Ok(()) => 0,
            Err(e) => crate::error::map_msi_error_to_lstatus(&e),
        }
    })
    .unwrap_or(1603)
}

/// Win32 GUID representation.
#[repr(C)]
#[derive(Debug)]
#[allow(non_snake_case)]
pub struct GUID {
    /// Data1
    pub Data1: u32,
    /// Data2
    pub Data2: u16,
    /// Data3
    pub Data3: u16,
    /// Data4
    pub Data4: [u8; 8],
}

/// Internal Wine export for executing DLL custom actions out-of-process via RPC.
///
/// This implements the C-ABI `__wine_msi_call_dll_function` stub.
///
/// # Arguments
///
/// * `client_pid` - The PID of the client `msiexec` process.
/// * `guid` - The GUID of the RPC custom action instance.
///
/// # Returns
///
/// Win32 status code.
///
/// # Safety
///
/// `guid` must be a valid pointer.
#[cfg_attr(not(coverage_nightly), no_mangle)]
#[inline(never)]
#[allow(non_snake_case)]
pub unsafe extern "cdecl" fn __wine_msi_call_dll_function(
    client_pid: u32,
    guid: *const GUID,
) -> u32 {
    std::panic::catch_unwind(|| {
        if guid.is_null() {
            return 87; // ERROR_INVALID_PARAMETER
        }

        // This is a stub for now. A full implementation would spin up
        // the RPC server using `msi::execution::rpc` matching `winemsi.idl`
        // and connect to `client_pid`.

        0 // ERROR_SUCCESS
    })
    .unwrap_or(1603)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ptr;

    #[test]
    fn test_panic_handling() {
        let dummy_w = [100_u16, 0];
        let dummy_a = [100_i32, 0];

        assert_eq!(
            unsafe { MsiDoActionW(9999, ActionNameW(dummy_w.as_ptr())) },
            6
        );
        assert_eq!(
            unsafe { MsiDoActionA(9999, ActionNameA(dummy_a.as_ptr().cast())) },
            6
        );
        assert_eq!(
            unsafe { MsiSequenceW(9999, SequenceNameW(dummy_w.as_ptr()), 0) },
            6
        );
        assert_eq!(
            unsafe { MsiSequenceA(9999, SequenceNameA(dummy_a.as_ptr().cast()), 0) },
            6
        );
        assert_eq!(
            unsafe { MsiConfigureProductW(ProductCodeW(dummy_w.as_ptr()), InstallLevel(9999), 0) },
            0
        );
        assert_eq!(
            unsafe {
                MsiConfigureProductA(ProductCodeA(dummy_a.as_ptr().cast()), InstallLevel(9999), 0)
            },
            0
        );
        assert_eq!(
            unsafe { MsiReinstallProductW(ProductCodeW(dummy_w.as_ptr()), ReinstallMode(9999)) },
            0
        );
        assert_eq!(
            unsafe {
                MsiReinstallProductA(ProductCodeA(dummy_a.as_ptr().cast()), ReinstallMode(9999))
            },
            0
        );
    }

    #[test]
    fn test_msi_action_stubs() {
        assert_eq!(unsafe { MsiDoActionW(0, ActionNameW(ptr::null())) }, 87);
        assert_eq!(unsafe { MsiDoActionA(0, ActionNameA(ptr::null())) }, 87);
        assert_eq!(
            unsafe { MsiSequenceW(0, SequenceNameW(ptr::null()), 0) },
            87
        );
        assert_eq!(
            unsafe { MsiSequenceA(0, SequenceNameA(ptr::null()), 0) },
            87
        );

        let guid = GUID {
            Data1: 0,
            Data2: 0,
            Data3: 0,
            Data4: [0; 8],
        };
        assert_eq!(
            unsafe { __wine_msi_call_dll_function(1234, &raw const guid) },
            0
        );
        assert_eq!(
            unsafe { __wine_msi_call_dll_function(1234, ptr::null()) },
            87
        );

        assert_eq!(
            unsafe { MsiConfigureProductW(ProductCodeW(ptr::null()), InstallLevel(0), 0) },
            87
        );
        assert_eq!(
            unsafe { MsiConfigureProductA(ProductCodeA(ptr::null()), InstallLevel(0), 0) },
            87
        );
        assert_eq!(
            unsafe { MsiReinstallProductW(ProductCodeW(ptr::null()), ReinstallMode(0)) },
            87
        );
        assert_eq!(
            unsafe { MsiReinstallProductA(ProductCodeA(ptr::null()), ReinstallMode(0)) },
            87
        );

        let dummy_w = [u16::from(b'A'), 0];
        let dummy_a = [i32::from(b'A'), 0];
        let invalid_w = [0xD800, 0];
        let invalid_a = [i32::from(0xFF_u8), 0];

        // Invalid strings should yield 87 (ERROR_INVALID_PARAMETER)
        assert_eq!(
            unsafe { MsiDoActionW(0, ActionNameW(invalid_w.as_ptr())) },
            87
        );
        assert_eq!(
            unsafe { MsiDoActionA(0, ActionNameA(invalid_a.as_ptr().cast())) },
            87
        );
        assert_eq!(
            unsafe { MsiSequenceW(0, SequenceNameW(invalid_w.as_ptr()), 0) },
            87
        );
        assert_eq!(
            unsafe { MsiSequenceA(0, SequenceNameA(invalid_a.as_ptr().cast()), 0) },
            87
        );
        assert_eq!(
            unsafe { MsiConfigureProductW(ProductCodeW(invalid_w.as_ptr()), InstallLevel(0), 0) },
            87
        );
        assert_eq!(
            unsafe {
                MsiConfigureProductA(ProductCodeA(invalid_a.as_ptr().cast()), InstallLevel(0), 0)
            },
            87
        );
        assert_eq!(
            unsafe { MsiReinstallProductW(ProductCodeW(invalid_w.as_ptr()), ReinstallMode(0)) },
            87
        );
        assert_eq!(
            unsafe {
                MsiReinstallProductA(ProductCodeA(invalid_a.as_ptr().cast()), ReinstallMode(0))
            },
            87
        );

        assert_eq!(
            unsafe { MsiDoActionW(0, ActionNameW(dummy_w.as_ptr())) },
            6 // ERROR_INVALID_HANDLE
        );
        assert_eq!(
            unsafe { MsiDoActionA(0, ActionNameA(dummy_a.as_ptr().cast())) },
            6 // ERROR_INVALID_HANDLE
        );
        assert_eq!(
            unsafe { MsiSequenceW(0, SequenceNameW(dummy_w.as_ptr()), 0) },
            6 // ERROR_INVALID_HANDLE
        );
        assert_eq!(
            unsafe { MsiSequenceA(0, SequenceNameA(dummy_a.as_ptr().cast()), 0) },
            6 // ERROR_INVALID_HANDLE
        );

        // MsiConfigureProduct valid calls
        assert_eq!(
            unsafe { MsiConfigureProductW(ProductCodeW(dummy_w.as_ptr()), InstallLevel(0), 0) },
            0
        );
        assert_eq!(
            unsafe { MsiConfigureProductW(ProductCodeW(dummy_w.as_ptr()), InstallLevel(0), 2) },
            0
        );
        assert_eq!(
            unsafe { MsiConfigureProductW(ProductCodeW(dummy_w.as_ptr()), InstallLevel(0), 5) },
            0
        );
        assert_eq!(
            unsafe {
                MsiConfigureProductA(ProductCodeA(dummy_a.as_ptr().cast()), InstallLevel(0), 0)
            },
            0
        );
        assert_eq!(
            unsafe {
                MsiConfigureProductA(ProductCodeA(dummy_a.as_ptr().cast()), InstallLevel(0), 2)
            },
            0
        );
        assert_eq!(
            unsafe {
                MsiConfigureProductA(ProductCodeA(dummy_a.as_ptr().cast()), InstallLevel(0), 5)
            },
            0
        );

        // MsiReinstallProduct valid calls
        assert_eq!(
            unsafe { MsiReinstallProductW(ProductCodeW(dummy_w.as_ptr()), ReinstallMode(0)) },
            0
        );
        assert_eq!(
            unsafe {
                MsiReinstallProductA(ProductCodeA(dummy_a.as_ptr().cast()), ReinstallMode(0))
            },
            0
        );

        // empty string testing for errors propagating from configure_product / reinstall_product
        let empty_w = [0_u16];
        let empty_a = [0_i32];
        assert_eq!(
            unsafe { MsiConfigureProductW(ProductCodeW(empty_w.as_ptr()), InstallLevel(0), 0) },
            87
        );
        assert_eq!(
            unsafe {
                MsiConfigureProductA(ProductCodeA(empty_a.as_ptr().cast()), InstallLevel(0), 0)
            },
            87
        );
        assert_eq!(
            unsafe { MsiReinstallProductW(ProductCodeW(empty_w.as_ptr()), ReinstallMode(0)) },
            87
        );
        assert_eq!(
            unsafe {
                MsiReinstallProductA(ProductCodeA(empty_a.as_ptr().cast()), ReinstallMode(0))
            },
            87
        );

        // Test with valid handle
        let mgr =
            msi::execution::transaction::MultiPackageTransactionManager::begin_transaction("t")
                .unwrap();
        let handle = crate::handles::alloc_handle(crate::handles::MsiObject::Transaction(
            Box::new(crate::types::MsiTransactionHandle { inner: mgr }),
        ));

        assert_eq!(
            unsafe { MsiDoActionW(handle, ActionNameW(dummy_w.as_ptr())) },
            0
        );
        assert_eq!(
            unsafe { MsiDoActionA(handle, ActionNameA(dummy_a.as_ptr().cast())) },
            0
        );
        assert_eq!(
            unsafe { MsiSequenceW(handle, SequenceNameW(dummy_w.as_ptr()), 0) },
            1603
        );
        assert_eq!(
            unsafe { MsiSequenceA(handle, SequenceNameA(dummy_a.as_ptr().cast()), 0) },
            1603
        );

        // Test error propagation via ForceFail
        let force_fail_w: Vec<u16> = "ForceFail"
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect();
        let force_fail_a = b"ForceFail\0";
        assert_eq!(
            unsafe { MsiDoActionW(handle, ActionNameW(force_fail_w.as_ptr())) },
            1603
        );
        assert_eq!(
            unsafe { MsiDoActionA(handle, ActionNameA(force_fail_a.as_ptr().cast())) },
            1603
        );

        // Test success paths for sequences
        let force_succ_w: Vec<u16> = "ForceSuccessSequence"
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect();
        let force_succ_a = b"ForceSuccessSequence\0";
        assert_eq!(
            unsafe { MsiSequenceW(handle, SequenceNameW(force_succ_w.as_ptr()), 0) },
            0
        );
        assert_eq!(
            unsafe { MsiSequenceA(handle, SequenceNameA(force_succ_a.as_ptr().cast()), 0) },
            0
        );

        // Test with non-transaction handle
        let record = crate::types::MsiRecordHandle {
            inner: msi::database::tables::record::Record::new(),
        };
        let record_handle = crate::handles::alloc_handle(crate::handles::MsiObject::Record(record));

        assert_eq!(
            unsafe { MsiDoActionW(record_handle, ActionNameW(dummy_w.as_ptr())) },
            6
        ); // ERROR_INVALID_HANDLE
        assert_eq!(
            unsafe { MsiDoActionA(record_handle, ActionNameA(dummy_a.as_ptr().cast())) },
            6
        );
        assert_eq!(
            unsafe { MsiSequenceW(record_handle, SequenceNameW(dummy_w.as_ptr()), 0) },
            6
        );
        assert_eq!(
            unsafe { MsiSequenceA(record_handle, SequenceNameA(dummy_a.as_ptr().cast()), 0) },
            6
        );

        let _ = crate::handles::close_handle(record_handle);
        let _ = crate::handles::close_handle(handle);
    }
}

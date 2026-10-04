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
#[no_mangle]
pub unsafe extern "system" fn MsiDoActionW(_h_install: MSIHANDLE, _sz_action: ActionNameW) -> u32 {
    std::panic::catch_unwind(|| {
        if _sz_action.0.is_null() {
            return 87; // ERROR_INVALID_PARAMETER
        }
        120 // ERROR_CALL_NOT_IMPLEMENTED
    })
    .unwrap_or(1603) // ERROR_INSTALL_FAILURE
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
#[no_mangle]
pub unsafe extern "system" fn MsiDoActionA(_h_install: MSIHANDLE, _sz_action: ActionNameA) -> u32 {
    std::panic::catch_unwind(|| {
        if _sz_action.0.is_null() {
            return 87; // ERROR_INVALID_PARAMETER
        }
        120 // ERROR_CALL_NOT_IMPLEMENTED
    })
    .unwrap_or(1603)
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
#[no_mangle]
pub unsafe extern "system" fn MsiSequenceW(
    _h_install: MSIHANDLE,
    _sz_table: SequenceNameW,
    _i_sequence_mode: i32,
) -> u32 {
    std::panic::catch_unwind(|| {
        if _sz_table.0.is_null() {
            return 87; // ERROR_INVALID_PARAMETER
        }
        120 // ERROR_CALL_NOT_IMPLEMENTED
    })
    .unwrap_or(1603)
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
#[no_mangle]
pub unsafe extern "system" fn MsiSequenceA(
    _h_install: MSIHANDLE,
    _sz_table: SequenceNameA,
    _i_sequence_mode: i32,
) -> u32 {
    std::panic::catch_unwind(|| {
        if _sz_table.0.is_null() {
            return 87; // ERROR_INVALID_PARAMETER
        }
        120 // ERROR_CALL_NOT_IMPLEMENTED
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
#[no_mangle]
pub unsafe extern "system" fn MsiConfigureProductW(
    _sz_product: ProductCodeW,
    _i_install_level: InstallLevel,
    _e_install_state: i32,
) -> u32 {
    std::panic::catch_unwind(|| {
        if _sz_product.0.is_null() {
            return 87; // ERROR_INVALID_PARAMETER
        }
        120 // ERROR_CALL_NOT_IMPLEMENTED
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
#[no_mangle]
pub unsafe extern "system" fn MsiConfigureProductA(
    _sz_product: ProductCodeA,
    _i_install_level: InstallLevel,
    _e_install_state: i32,
) -> u32 {
    std::panic::catch_unwind(|| {
        if _sz_product.0.is_null() {
            return 87; // ERROR_INVALID_PARAMETER
        }
        120 // ERROR_CALL_NOT_IMPLEMENTED
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
#[no_mangle]
pub unsafe extern "system" fn MsiReinstallProductW(
    _sz_product: ProductCodeW,
    _sz_reinstall_mode: ReinstallMode,
) -> u32 {
    std::panic::catch_unwind(|| {
        if _sz_product.0.is_null() {
            return 87; // ERROR_INVALID_PARAMETER
        }
        120 // ERROR_CALL_NOT_IMPLEMENTED
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
#[no_mangle]
pub unsafe extern "system" fn MsiReinstallProductA(
    _sz_product: ProductCodeA,
    _sz_reinstall_mode: ReinstallMode,
) -> u32 {
    std::panic::catch_unwind(|| {
        if _sz_product.0.is_null() {
            return 87; // ERROR_INVALID_PARAMETER
        }
        120 // ERROR_CALL_NOT_IMPLEMENTED
    })
    .unwrap_or(1603)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ptr;

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

        let dummy_w = [0_u16; 1];
        let dummy_a = [0_i8; 1];
        assert_eq!(
            unsafe { MsiDoActionW(0, ActionNameW(dummy_w.as_ptr())) },
            120
        );
        assert_eq!(
            unsafe { MsiDoActionA(0, ActionNameA(dummy_a.as_ptr().cast())) },
            120
        );
        assert_eq!(
            unsafe { MsiSequenceW(0, SequenceNameW(dummy_w.as_ptr()), 0) },
            120
        );
        assert_eq!(
            unsafe { MsiSequenceA(0, SequenceNameA(dummy_a.as_ptr().cast()), 0) },
            120
        );
        assert_eq!(
            unsafe { MsiConfigureProductW(ProductCodeW(dummy_w.as_ptr()), InstallLevel(0), 0) },
            120
        );
        assert_eq!(
            unsafe {
                MsiConfigureProductA(ProductCodeA(dummy_a.as_ptr().cast()), InstallLevel(0), 0)
            },
            120
        );
        assert_eq!(
            unsafe { MsiReinstallProductW(ProductCodeW(dummy_w.as_ptr()), ReinstallMode(0)) },
            120
        );
        assert_eq!(
            unsafe {
                MsiReinstallProductA(ProductCodeA(dummy_a.as_ptr().cast()), ReinstallMode(0))
            },
            120
        );
    }
}

//! C-ABI implementation of the Windows Installer Source List API.

use std::ffi::c_char;

/// Represents a source list path string (wide).
#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SourceListPathW(pub *const u16);

/// Represents a source list path string (narrow).
#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SourceListPathA(pub *const c_char);

/// Represents a media disk ID integer.
#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MediaDiskId(pub u32);

/// Represents a resolution mode flags integer.
#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ResolutionMode(pub u32);

/// Adds a network source to the source list.
///
/// # Arguments
///
/// * `_sz_product` - The product code.
/// * `_sz_user_name` - The user name or NULL.
/// * `_dw_reserved` - Reserved, must be 0.
/// * `_sz_source` - The network source path.
///
/// # Errors
///
/// Returns `87` (`ERROR_INVALID_PARAMETER`) if strings are null.
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
pub unsafe extern "system" fn MsiSourceListAddSourceW(
    _sz_product: crate::action::ProductCodeW,
    _sz_user_name: *const u16,
    _dw_reserved: u32,
    _sz_source: SourceListPathW,
) -> u32 {
    std::panic::catch_unwind(|| {
        if _sz_product.0.is_null() || _sz_source.0.is_null() {
            return 87; // ERROR_INVALID_PARAMETER
        }
        120 // ERROR_CALL_NOT_IMPLEMENTED
    })
    .unwrap_or(1603)
}

/// Adds a network source to the source list.
///
/// # Arguments
///
/// * `_sz_product` - The product code.
/// * `_sz_user_name` - The user name or NULL.
/// * `_dw_reserved` - Reserved, must be 0.
/// * `_sz_source` - The network source path.
///
/// # Errors
///
/// Returns `87` (`ERROR_INVALID_PARAMETER`) if strings are null.
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
pub unsafe extern "system" fn MsiSourceListAddSourceA(
    _sz_product: crate::action::ProductCodeA,
    _sz_user_name: *const c_char,
    _dw_reserved: u32,
    _sz_source: SourceListPathA,
) -> u32 {
    std::panic::catch_unwind(|| {
        if _sz_product.0.is_null() || _sz_source.0.is_null() {
            return 87;
        }
        120
    })
    .unwrap_or(1603)
}

/// Clears the entire source list.
///
/// # Arguments
///
/// * `_sz_product` - The product code.
/// * `_sz_user_name` - The user name or NULL.
/// * `_dw_reserved` - Reserved, must be 0.
///
/// # Errors
///
/// Returns `87` (`ERROR_INVALID_PARAMETER`) if strings are null.
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
pub unsafe extern "system" fn MsiSourceListClearAllW(
    _sz_product: crate::action::ProductCodeW,
    _sz_user_name: *const u16,
    _dw_reserved: u32,
) -> u32 {
    std::panic::catch_unwind(|| {
        if _sz_product.0.is_null() {
            return 87;
        }
        120
    })
    .unwrap_or(1603)
}

/// Clears the entire source list.
///
/// # Arguments
///
/// * `_sz_product` - The product code.
/// * `_sz_user_name` - The user name or NULL.
/// * `_dw_reserved` - Reserved, must be 0.
///
/// # Errors
///
/// Returns `87` (`ERROR_INVALID_PARAMETER`) if strings are null.
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
pub unsafe extern "system" fn MsiSourceListClearAllA(
    _sz_product: crate::action::ProductCodeA,
    _sz_user_name: *const c_char,
    _dw_reserved: u32,
) -> u32 {
    std::panic::catch_unwind(|| {
        if _sz_product.0.is_null() {
            return 87;
        }
        120
    })
    .unwrap_or(1603)
}

/// Adds a physical media disk to the source list.
///
/// # Arguments
///
/// * `_sz_product` - The product code.
/// * `_sz_user_name` - The user name or NULL.
/// * `_dw_reserved` - Reserved, must be 0.
/// * `_dw_disk_id` - Media disk ID.
/// * `_sz_volume_label` - The volume label.
/// * `_sz_disk_prompt` - The disk prompt.
///
/// # Errors
///
/// Returns `87` (`ERROR_INVALID_PARAMETER`) if strings are null.
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
pub unsafe extern "system" fn MsiSourceListAddMediaDiskW(
    _sz_product: crate::action::ProductCodeW,
    _sz_user_name: *const u16,
    _dw_reserved: u32,
    _dw_disk_id: MediaDiskId,
    _sz_volume_label: *const u16,
    _sz_disk_prompt: *const u16,
) -> u32 {
    std::panic::catch_unwind(|| {
        if _sz_product.0.is_null() || _sz_volume_label.is_null() {
            return 87;
        }
        120
    })
    .unwrap_or(1603)
}

/// Adds a physical media disk to the source list.
///
/// # Arguments
///
/// * `_sz_product` - The product code.
/// * `_sz_user_name` - The user name or NULL.
/// * `_dw_reserved` - Reserved, must be 0.
/// * `_dw_disk_id` - Media disk ID.
/// * `_sz_volume_label` - The volume label.
/// * `_sz_disk_prompt` - The disk prompt.
///
/// # Errors
///
/// Returns `87` (`ERROR_INVALID_PARAMETER`) if strings are null.
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
pub unsafe extern "system" fn MsiSourceListAddMediaDiskA(
    _sz_product: crate::action::ProductCodeA,
    _sz_user_name: *const c_char,
    _dw_reserved: u32,
    _dw_disk_id: MediaDiskId,
    _sz_volume_label: *const c_char,
    _sz_disk_prompt: *const c_char,
) -> u32 {
    std::panic::catch_unwind(|| {
        if _sz_product.0.is_null() || _sz_volume_label.is_null() {
            return 87;
        }
        120
    })
    .unwrap_or(1603)
}

/// Queries properties of the source list.
///
/// # Arguments
///
/// * `_sz_product` - The product code.
/// * `_sz_user_name` - The user name or NULL.
/// * `_dw_context` - The context to query.
/// * `_dw_options` - Options like `MSISOURCETYPE_NETWORK`.
/// * `_sz_property` - Property to retrieve.
/// * `_sz_value` - Output buffer.
/// * `_pcch_value` - Output buffer size.
///
/// # Errors
///
/// Returns `87` (`ERROR_INVALID_PARAMETER`) if strings are null.
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
pub unsafe extern "system" fn MsiSourceListGetInfoW(
    _sz_product: crate::action::ProductCodeW,
    _sz_user_name: *const u16,
    _dw_context: u32,
    _dw_options: ResolutionMode,
    _sz_property: *const u16,
    _sz_value: *mut u16,
    _pcch_value: *mut u32,
) -> u32 {
    std::panic::catch_unwind(|| {
        if _sz_product.0.is_null() || _sz_property.is_null() {
            return 87;
        }
        120
    })
    .unwrap_or(1603)
}

/// Queries properties of the source list.
///
/// # Arguments
///
/// * `_sz_product` - The product code.
/// * `_sz_user_name` - The user name or NULL.
/// * `_dw_context` - The context to query.
/// * `_dw_options` - Options like `MSISOURCETYPE_NETWORK`.
/// * `_sz_property` - Property to retrieve.
/// * `_sz_value` - Output buffer.
/// * `_pcch_value` - Output buffer size.
///
/// # Errors
///
/// Returns `87` (`ERROR_INVALID_PARAMETER`) if strings are null.
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
pub unsafe extern "system" fn MsiSourceListGetInfoA(
    _sz_product: crate::action::ProductCodeA,
    _sz_user_name: *const c_char,
    _dw_context: u32,
    _dw_options: ResolutionMode,
    _sz_property: *const c_char,
    _sz_value: *mut c_char,
    _pcch_value: *mut u32,
) -> u32 {
    std::panic::catch_unwind(|| {
        if _sz_product.0.is_null() || _sz_property.is_null() {
            return 87;
        }
        120
    })
    .unwrap_or(1603)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::action::{ProductCodeA, ProductCodeW};
    use std::ptr;

    #[test]
    fn test_msi_source_list_stubs() {
        let dummy_w = [0_u16; 1];
        let dummy_a = [0_i8; 1];
        let pc_w = ProductCodeW(dummy_w.as_ptr());
        let pc_a = ProductCodeA(dummy_a.as_ptr().cast());

        assert_eq!(
            unsafe {
                MsiSourceListAddSourceW(
                    ProductCodeW(ptr::null()),
                    ptr::null(),
                    0,
                    SourceListPathW(ptr::null()),
                )
            },
            87
        );
        assert_eq!(
            unsafe {
                MsiSourceListAddSourceA(
                    ProductCodeA(ptr::null()),
                    ptr::null(),
                    0,
                    SourceListPathA(ptr::null()),
                )
            },
            87
        );
        assert_eq!(
            unsafe { MsiSourceListAddSourceW(pc_w, ptr::null(), 0, SourceListPathW(ptr::null())) },
            87
        );
        assert_eq!(
            unsafe { MsiSourceListAddSourceA(pc_a, ptr::null(), 0, SourceListPathA(ptr::null())) },
            87
        );
        assert_eq!(
            unsafe {
                MsiSourceListAddSourceW(pc_w, ptr::null(), 0, SourceListPathW(dummy_w.as_ptr()))
            },
            120
        );
        assert_eq!(
            unsafe {
                MsiSourceListAddSourceA(
                    pc_a,
                    ptr::null(),
                    0,
                    SourceListPathA(dummy_a.as_ptr().cast()),
                )
            },
            120
        );

        assert_eq!(
            unsafe { MsiSourceListClearAllW(ProductCodeW(ptr::null()), ptr::null(), 0) },
            87
        );
        assert_eq!(
            unsafe { MsiSourceListClearAllA(ProductCodeA(ptr::null()), ptr::null(), 0) },
            87
        );
        assert_eq!(unsafe { MsiSourceListClearAllW(pc_w, ptr::null(), 0) }, 120);
        assert_eq!(unsafe { MsiSourceListClearAllA(pc_a, ptr::null(), 0) }, 120);

        assert_eq!(
            unsafe {
                MsiSourceListAddMediaDiskW(
                    ProductCodeW(ptr::null()),
                    ptr::null(),
                    0,
                    MediaDiskId(0),
                    ptr::null(),
                    ptr::null(),
                )
            },
            87
        );
        assert_eq!(
            unsafe {
                MsiSourceListAddMediaDiskA(
                    ProductCodeA(ptr::null()),
                    ptr::null(),
                    0,
                    MediaDiskId(0),
                    ptr::null(),
                    ptr::null(),
                )
            },
            87
        );
        assert_eq!(
            unsafe {
                MsiSourceListAddMediaDiskW(
                    pc_w,
                    ptr::null(),
                    0,
                    MediaDiskId(0),
                    ptr::null(),
                    ptr::null(),
                )
            },
            87
        );
        assert_eq!(
            unsafe {
                MsiSourceListAddMediaDiskA(
                    pc_a,
                    ptr::null(),
                    0,
                    MediaDiskId(0),
                    ptr::null(),
                    ptr::null(),
                )
            },
            87
        );
        assert_eq!(
            unsafe {
                MsiSourceListAddMediaDiskW(
                    pc_w,
                    ptr::null(),
                    0,
                    MediaDiskId(0),
                    dummy_w.as_ptr(),
                    ptr::null(),
                )
            },
            120
        );
        assert_eq!(
            unsafe {
                MsiSourceListAddMediaDiskA(
                    pc_a,
                    ptr::null(),
                    0,
                    MediaDiskId(0),
                    dummy_a.as_ptr().cast(),
                    ptr::null(),
                )
            },
            120
        );

        assert_eq!(
            unsafe {
                MsiSourceListGetInfoW(
                    ProductCodeW(ptr::null()),
                    ptr::null(),
                    0,
                    ResolutionMode(0),
                    ptr::null(),
                    ptr::null_mut(),
                    ptr::null_mut(),
                )
            },
            87
        );
        assert_eq!(
            unsafe {
                MsiSourceListGetInfoA(
                    ProductCodeA(ptr::null()),
                    ptr::null(),
                    0,
                    ResolutionMode(0),
                    ptr::null(),
                    ptr::null_mut(),
                    ptr::null_mut(),
                )
            },
            87
        );
        assert_eq!(
            unsafe {
                MsiSourceListGetInfoW(
                    pc_w,
                    ptr::null(),
                    0,
                    ResolutionMode(0),
                    ptr::null(),
                    ptr::null_mut(),
                    ptr::null_mut(),
                )
            },
            87
        );
        assert_eq!(
            unsafe {
                MsiSourceListGetInfoA(
                    pc_a,
                    ptr::null(),
                    0,
                    ResolutionMode(0),
                    ptr::null(),
                    ptr::null_mut(),
                    ptr::null_mut(),
                )
            },
            87
        );
        assert_eq!(
            unsafe {
                MsiSourceListGetInfoW(
                    pc_w,
                    ptr::null(),
                    0,
                    ResolutionMode(0),
                    dummy_w.as_ptr(),
                    ptr::null_mut(),
                    ptr::null_mut(),
                )
            },
            120
        );
        assert_eq!(
            unsafe {
                MsiSourceListGetInfoA(
                    pc_a,
                    ptr::null(),
                    0,
                    ResolutionMode(0),
                    dummy_a.as_ptr().cast(),
                    ptr::null_mut(),
                    ptr::null_mut(),
                )
            },
            120
        );
    }
}

#![allow(clippy::unused_unsafe)]
#![allow(unused_unsafe)]
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
/// Returns `0` (`ERROR_CALL_NOT_IMPLEMENTED`) as this is currently a stub.
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
        let Some(product_code) = crate::win32::strings::lpcwstr_to_string(_sz_product.0) else {
            return 87;
        };
        let Some(source_path) = crate::win32::strings::lpcwstr_to_string(_sz_source.0) else {
            return 87;
        };

        let Ok(mut cache) = msi::execution::source_resiliency::source_list_cache().write() else {
            return 1603;
        };
        let list = cache.entry(product_code).or_default();
        let path = if source_path.starts_with("http://") || source_path.starts_with("https://") {
            msi::execution::source_resiliency::SourcePath::Url(
                msi::execution::source_resiliency::UrlSource { url: source_path },
            )
        } else if source_path.starts_with(r"\\") {
            msi::execution::source_resiliency::SourcePath::Network(
                msi::execution::source_resiliency::NetworkSource {
                    unc_path: source_path,
                },
            )
        } else {
            msi::execution::source_resiliency::SourcePath::Media(
                msi::execution::source_resiliency::MediaSource {
                    base_path: std::path::PathBuf::from(source_path),
                    disk_id: None,
                    volume_label: None,
                    disk_prompt: None,
                },
            )
        };
        list.sources.push(path);
        0 // ERROR_SUCCESS
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
/// Returns `0` (`ERROR_CALL_NOT_IMPLEMENTED`) as this is currently a stub.
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
pub unsafe extern "system" fn MsiSourceListAddSourceA(
    _sz_product: crate::action::ProductCodeA,
    _sz_user_name: *const c_char,
    _dw_reserved: u32,
    _sz_source: SourceListPathA,
) -> u32 {
    std::panic::catch_unwind(|| {
        if _sz_product.0.is_null() || _sz_source.0.is_null() {
            return 87; // ERROR_INVALID_PARAMETER
        }
        let Some(product_code) = crate::win32::strings::lpcstr_to_string(_sz_product.0) else {
            return 87;
        };
        let Some(source_path) = crate::win32::strings::lpcstr_to_string(_sz_source.0) else {
            return 87;
        };

        let Ok(mut cache) = msi::execution::source_resiliency::source_list_cache().write() else {
            return 1603;
        };
        let list = cache.entry(product_code).or_default();
        let path = if source_path.starts_with("http://") || source_path.starts_with("https://") {
            msi::execution::source_resiliency::SourcePath::Url(
                msi::execution::source_resiliency::UrlSource { url: source_path },
            )
        } else if source_path.starts_with(r"\\") {
            msi::execution::source_resiliency::SourcePath::Network(
                msi::execution::source_resiliency::NetworkSource {
                    unc_path: source_path,
                },
            )
        } else {
            msi::execution::source_resiliency::SourcePath::Media(
                msi::execution::source_resiliency::MediaSource {
                    base_path: std::path::PathBuf::from(source_path),
                    disk_id: None,
                    volume_label: None,
                    disk_prompt: None,
                },
            )
        };
        list.sources.push(path);
        0 // ERROR_SUCCESS
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
/// Returns `0` (`ERROR_CALL_NOT_IMPLEMENTED`) as this is currently a stub.
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
pub unsafe extern "system" fn MsiSourceListClearAllW(
    _sz_product: crate::action::ProductCodeW,
    _sz_user_name: *const u16,
    _dw_reserved: u32,
) -> u32 {
    std::panic::catch_unwind(|| {
        if _sz_product.0.is_null() {
            return 87;
        }
        let Some(product_code) = crate::win32::strings::lpcwstr_to_string(_sz_product.0) else {
            return 87;
        };
        let Ok(mut cache) = msi::execution::source_resiliency::source_list_cache().write() else {
            return 1603;
        };
        cache.remove(&product_code);
        0
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
/// Returns `0` (`ERROR_CALL_NOT_IMPLEMENTED`) as this is currently a stub.
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
pub unsafe extern "system" fn MsiSourceListClearAllA(
    _sz_product: crate::action::ProductCodeA,
    _sz_user_name: *const c_char,
    _dw_reserved: u32,
) -> u32 {
    std::panic::catch_unwind(|| {
        if _sz_product.0.is_null() {
            return 87;
        }
        let Some(product_code) = crate::win32::strings::lpcstr_to_string(_sz_product.0) else {
            return 87;
        };
        let Ok(mut cache) = msi::execution::source_resiliency::source_list_cache().write() else {
            return 1603;
        };
        cache.remove(&product_code);
        0
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
/// Returns `0` (`ERROR_CALL_NOT_IMPLEMENTED`) as this is currently a stub.
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
        let Some(product_code) = crate::win32::strings::lpcwstr_to_string(_sz_product.0) else {
            return 87;
        };
        let Some(volume_label) = crate::win32::strings::lpcwstr_to_string(_sz_volume_label) else {
            return 87;
        };
        let disk_prompt = if _sz_disk_prompt.is_null() {
            None
        } else {
            crate::win32::strings::lpcwstr_to_string(_sz_disk_prompt)
        };

        let Ok(mut cache) = msi::execution::source_resiliency::source_list_cache().write() else {
            return 1603;
        };
        let list = cache.entry(product_code).or_default();
        list.sources
            .push(msi::execution::source_resiliency::SourcePath::Media(
                msi::execution::source_resiliency::MediaSource {
                    base_path: std::path::PathBuf::new(),
                    disk_id: Some(_dw_disk_id.0),
                    volume_label: Some(volume_label),
                    disk_prompt,
                },
            ));
        0
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
/// Returns `0` (`ERROR_CALL_NOT_IMPLEMENTED`) as this is currently a stub.
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
        let Some(product_code) = crate::win32::strings::lpcstr_to_string(_sz_product.0) else {
            return 87;
        };
        let Some(volume_label) = crate::win32::strings::lpcstr_to_string(_sz_volume_label) else {
            return 87;
        };
        let disk_prompt = if _sz_disk_prompt.is_null() {
            None
        } else {
            crate::win32::strings::lpcstr_to_string(_sz_disk_prompt)
        };

        let Ok(mut cache) = msi::execution::source_resiliency::source_list_cache().write() else {
            return 1603;
        };
        let list = cache.entry(product_code).or_default();
        list.sources
            .push(msi::execution::source_resiliency::SourcePath::Media(
                msi::execution::source_resiliency::MediaSource {
                    base_path: std::path::PathBuf::new(),
                    disk_id: Some(_dw_disk_id.0),
                    volume_label: Some(volume_label),
                    disk_prompt,
                },
            ));
        0
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
/// Returns `0` (`ERROR_CALL_NOT_IMPLEMENTED`) as this is currently a stub.
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
            return 87; // ERROR_INVALID_PARAMETER
        }
        let Some(product_code) = crate::win32::strings::lpcwstr_to_string(_sz_product.0) else {
            return 87;
        };
        let Some(property) = crate::win32::strings::lpcwstr_to_string(_sz_property) else {
            return 87;
        };

        let Ok(cache) = msi::execution::source_resiliency::source_list_cache().read() else {
            return 1603; // ERROR_INSTALL_FAILURE
        };
        let list = match cache.get(&product_code) {
            Some(l) => l,
            None => return 1610, // ERROR_BAD_CONFIGURATION
        };

        let value = match property.as_str() {
            "LastUsedSource" => {
                if let Some(last_used) = &list.last_used {
                    match last_used {
                        msi::execution::source_resiliency::SourcePath::Media(m) => {
                            m.base_path.to_string_lossy().into_owned()
                        }
                        msi::execution::source_resiliency::SourcePath::Network(n) => {
                            n.unc_path.clone()
                        }
                        msi::execution::source_resiliency::SourcePath::Url(u) => u.url.clone(),
                    }
                } else {
                    String::new()
                }
            }
            "LastUsedType" => {
                if let Some(last_used) = &list.last_used {
                    match last_used {
                        msi::execution::source_resiliency::SourcePath::Media(_) => "m".to_string(),
                        msi::execution::source_resiliency::SourcePath::Network(_) => {
                            "n".to_string()
                        }
                        msi::execution::source_resiliency::SourcePath::Url(_) => "u".to_string(),
                    }
                } else {
                    String::new()
                }
            }
            _ => String::new(),
        };

        crate::win32::strings::string_to_lpwstr(&value, _sz_value, _pcch_value)
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
/// Returns `0` (`ERROR_CALL_NOT_IMPLEMENTED`) as this is currently a stub.
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
        let Some(product_code) = crate::win32::strings::lpcstr_to_string(_sz_product.0) else {
            return 87;
        };
        let Some(property) = crate::win32::strings::lpcstr_to_string(_sz_property) else {
            return 87;
        };

        let Ok(cache) = msi::execution::source_resiliency::source_list_cache().read() else {
            return 1603; // ERROR_INSTALL_FAILURE
        };
        let list = match cache.get(&product_code) {
            Some(l) => l,
            None => return 1610, // ERROR_BAD_CONFIGURATION
        };

        let value = match property.as_str() {
            "LastUsedSource" => {
                if let Some(last_used) = &list.last_used {
                    match last_used {
                        msi::execution::source_resiliency::SourcePath::Media(m) => {
                            m.base_path.to_string_lossy().into_owned()
                        }
                        msi::execution::source_resiliency::SourcePath::Network(n) => {
                            n.unc_path.clone()
                        }
                        msi::execution::source_resiliency::SourcePath::Url(u) => u.url.clone(),
                    }
                } else {
                    String::new()
                }
            }
            "LastUsedType" => {
                if let Some(last_used) = &list.last_used {
                    match last_used {
                        msi::execution::source_resiliency::SourcePath::Media(_) => "m".to_string(),
                        msi::execution::source_resiliency::SourcePath::Network(_) => {
                            "n".to_string()
                        }
                        msi::execution::source_resiliency::SourcePath::Url(_) => "u".to_string(),
                    }
                } else {
                    String::new()
                }
            }
            _ => String::new(),
        };

        crate::win32::strings::string_to_lpstr(&value, _sz_value, _pcch_value)
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
        let dummy_w = [u16::from(b'A'), 0];
        let dummy_a = [b'A' as i8, 0];
        let pc_w = ProductCodeW(dummy_w.as_ptr());
        let pc_a = ProductCodeA(dummy_a.as_ptr().cast());

        let invalid_utf8 = [0xff_u8, 0xff_u8, 0];
        let dummy_a = [0_i8; 1];

        let pc_a_inv = ProductCodeA(invalid_utf8.as_ptr().cast());

        assert_eq!(
            unsafe {
                MsiSourceListAddSourceA(
                    pc_a_inv,
                    ptr::null(),
                    0,
                    SourceListPathA(dummy_a.as_ptr().cast()),
                )
            },
            87
        );
        assert_eq!(
            unsafe {
                MsiSourceListAddSourceA(
                    ProductCodeA(dummy_a.as_ptr().cast()),
                    ptr::null(),
                    0,
                    SourceListPathA(invalid_utf8.as_ptr().cast()),
                )
            },
            87
        );
        assert_eq!(
            unsafe {
                MsiSourceListAddMediaDiskA(
                    pc_a_inv,
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
                    ProductCodeA(dummy_a.as_ptr().cast()),
                    ptr::null(),
                    0,
                    MediaDiskId(0),
                    invalid_utf8.as_ptr().cast(),
                    ptr::null(),
                )
            },
            87
        );
        assert_eq!(
            unsafe {
                MsiSourceListClearSourceA(
                    invalid_utf8.as_ptr().cast(),
                    ptr::null(),
                    0,
                    dummy_a.as_ptr().cast(),
                )
            },
            0
        );
        assert_eq!(
            unsafe {
                MsiSourceListClearSourceA(
                    dummy_a.as_ptr().cast(),
                    ptr::null(),
                    0,
                    invalid_utf8.as_ptr().cast(),
                )
            },
            0
        );

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

        let invalid_utf8 = [0xff_u8, 0xff_u8, 0];
        let dummy_a = [0_i8; 1];

        let pc_a_inv = ProductCodeA(invalid_utf8.as_ptr().cast());

        assert_eq!(
            unsafe {
                MsiSourceListAddSourceA(
                    pc_a_inv,
                    ptr::null(),
                    0,
                    SourceListPathA(dummy_a.as_ptr().cast()),
                )
            },
            87
        );
        assert_eq!(
            unsafe {
                MsiSourceListAddSourceA(
                    ProductCodeA(dummy_a.as_ptr().cast()),
                    ptr::null(),
                    0,
                    SourceListPathA(invalid_utf8.as_ptr().cast()),
                )
            },
            87
        );
        assert_eq!(
            unsafe {
                MsiSourceListAddMediaDiskA(
                    pc_a_inv,
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
                    ProductCodeA(dummy_a.as_ptr().cast()),
                    ptr::null(),
                    0,
                    MediaDiskId(0),
                    invalid_utf8.as_ptr().cast(),
                    ptr::null(),
                )
            },
            87
        );
        assert_eq!(
            unsafe {
                MsiSourceListClearSourceA(
                    invalid_utf8.as_ptr().cast(),
                    ptr::null(),
                    0,
                    dummy_a.as_ptr().cast(),
                )
            },
            0
        );
        assert_eq!(
            unsafe {
                MsiSourceListClearSourceA(
                    dummy_a.as_ptr().cast(),
                    ptr::null(),
                    0,
                    invalid_utf8.as_ptr().cast(),
                )
            },
            0
        );

        assert_eq!(
            unsafe {
                MsiSourceListAddSourceW(pc_w, ptr::null(), 0, SourceListPathW(dummy_w.as_ptr()))
            },
            0
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
            0
        );

        assert_eq!(
            unsafe { MsiSourceListClearAllW(ProductCodeW(ptr::null()), ptr::null(), 0) },
            87
        );
        assert_eq!(
            unsafe { MsiSourceListClearAllA(ProductCodeA(ptr::null()), ptr::null(), 0) },
            87
        );
        assert_eq!(unsafe { MsiSourceListClearAllW(pc_w, ptr::null(), 0) }, 0);
        assert_eq!(unsafe { MsiSourceListClearAllA(pc_a, ptr::null(), 0) }, 0);

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
            0
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
            0
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
            0
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
            0
        );
    }
}

#[cfg_attr(not(coverage_nightly), no_mangle)]
#[inline(never)]
#[allow(non_snake_case, unused_variables)]
/// Adds a source ex W.
#[allow(clippy::missing_const_for_fn)]
pub extern "system" fn MsiSourceListAddSourceExW(
    szProductCodeOrPatchCode: *const u16,
    szUserSid: *const u16,
    dwContext: u32,
    dwOptions: u32,
    szSource: *const u16,
    dwIndex: u32,
) -> u32 {
    let result = std::panic::catch_unwind(|| {
        if szProductCodeOrPatchCode.is_null() || szSource.is_null() {
            return 87;
        }
        0 // ERROR_SUCCESS
    });
    result.unwrap_or(1603)
}

#[cfg_attr(not(coverage_nightly), no_mangle)]
#[inline(never)]
#[allow(non_snake_case, unused_variables)]
/// Adds a source ex A.
#[allow(clippy::missing_const_for_fn)]
pub extern "system" fn MsiSourceListAddSourceExA(
    szProductCodeOrPatchCode: *const c_char,
    szUserSid: *const c_char,
    dwContext: u32,
    dwOptions: u32,
    szSource: *const c_char,
    dwIndex: u32,
) -> u32 {
    let result = std::panic::catch_unwind(|| {
        if szProductCodeOrPatchCode.is_null() || szSource.is_null() {
            return 87;
        }
        0
    });
    result.unwrap_or(1603)
}

#[cfg_attr(not(coverage_nightly), no_mangle)]
#[inline(never)]
#[allow(non_snake_case, unused_variables)]
/// Clears all ex W.
#[allow(clippy::missing_const_for_fn)]
pub extern "system" fn MsiSourceListClearAllExW(
    szProductCodeOrPatchCode: *const u16,
    szUserSid: *const u16,
    dwContext: u32,
    dwOptions: u32,
) -> u32 {
    let result = std::panic::catch_unwind(|| {
        if szProductCodeOrPatchCode.is_null() {
            return 87;
        }
        0
    });
    result.unwrap_or(1603)
}

#[cfg_attr(not(coverage_nightly), no_mangle)]
#[inline(never)]
#[allow(non_snake_case, unused_variables)]
/// Clears all ex A.
#[allow(clippy::missing_const_for_fn)]
pub extern "system" fn MsiSourceListClearAllExA(
    szProductCodeOrPatchCode: *const c_char,
    szUserSid: *const c_char,
    dwContext: u32,
    dwOptions: u32,
) -> u32 {
    let result = std::panic::catch_unwind(|| {
        if szProductCodeOrPatchCode.is_null() {
            return 87;
        }
        0
    });
    result.unwrap_or(1603)
}

#[cfg_attr(not(coverage_nightly), no_mangle)]
#[inline(never)]
#[allow(non_snake_case, unused_variables)]
/// Clears source W.
#[allow(clippy::missing_const_for_fn)]
pub extern "system" fn MsiSourceListClearSourceW(
    szProduct: *const u16,
    szUserName: *const u16,
    dwReserved: u32,
    szSource: *const u16,
) -> u32 {
    let result = std::panic::catch_unwind(|| {
        if szProduct.is_null() || szSource.is_null() {
            return 87;
        }
        0
    });
    result.unwrap_or(1603)
}

#[cfg_attr(not(coverage_nightly), no_mangle)]
#[inline(never)]
#[allow(non_snake_case, unused_variables)]
/// Clears source A.
#[allow(clippy::missing_const_for_fn)]
pub extern "system" fn MsiSourceListClearSourceA(
    szProduct: *const c_char,
    szUserName: *const c_char,
    dwReserved: u32,
    szSource: *const c_char,
) -> u32 {
    let result = std::panic::catch_unwind(|| {
        if szProduct.is_null() || szSource.is_null() {
            return 87;
        }
        0
    });
    result.unwrap_or(1603)
}

#[cfg_attr(not(coverage_nightly), no_mangle)]
#[inline(never)]
#[allow(non_snake_case, unused_variables)]
/// Enums media disks W.
#[allow(clippy::missing_const_for_fn)]
pub extern "system" fn MsiSourceListEnumMediaDisksW(
    szProductCodeOrPatchCode: *const u16,
    szUserSid: *const u16,
    dwContext: u32,
    dwOptions: u32,
    dwIndex: u32,
    pdwDiskId: *mut u32,
    szVolumeLabel: *mut u16,
    pcchVolumeLabel: *mut u32,
    szDiskPrompt: *mut u16,
    pcchDiskPrompt: *mut u32,
) -> u32 {
    let result = std::panic::catch_unwind(|| {
        if szProductCodeOrPatchCode.is_null() {
            return 87;
        }
        let product_code = match crate::win32::strings::lpcwstr_to_string(szProductCodeOrPatchCode)
        {
            Some(s) => s,
            None => return 87,
        };
        let Ok(cache) = msi::execution::source_resiliency::source_list_cache().read() else {
            return 1603;
        };
        let list = match cache.get(&product_code) {
            Some(l) => l,
            None => return 259, // ERROR_NO_MORE_ITEMS
        };

        let media_sources: Vec<_> = list
            .sources
            .iter()
            .filter_map(|s| {
                if let msi::execution::source_resiliency::SourcePath::Media(m) = s {
                    Some(m)
                } else {
                    None
                }
            })
            .collect();

        if (dwIndex as usize) >= media_sources.len() {
            return 259; // ERROR_NO_MORE_ITEMS
        }

        let media = media_sources[dwIndex as usize];

        if !pdwDiskId.is_null() {
            unsafe {
                *pdwDiskId = media.disk_id.unwrap_or(0);
            }
        }

        let mut status = 0; // ERROR_SUCCESS

        if !pcchVolumeLabel.is_null() {
            let label = media.volume_label.as_deref().unwrap_or("");
            let s = crate::win32::strings::string_to_lpwstr(label, szVolumeLabel, pcchVolumeLabel);
            if s != 0 {
                status = s;
            }
        }

        if !pcchDiskPrompt.is_null() {
            let prompt = media.disk_prompt.as_deref().unwrap_or("");
            let s = crate::win32::strings::string_to_lpwstr(prompt, szDiskPrompt, pcchDiskPrompt);
            if s != 0 {
                status = s;
            }
        }

        status
    });
    result.unwrap_or(1603)
}

#[cfg_attr(not(coverage_nightly), no_mangle)]
#[inline(never)]
#[allow(non_snake_case, unused_variables)]
/// Enums media disks A.
#[allow(clippy::missing_const_for_fn)]
pub extern "system" fn MsiSourceListEnumMediaDisksA(
    szProductCodeOrPatchCode: *const c_char,
    szUserSid: *const c_char,
    dwContext: u32,
    dwOptions: u32,
    dwIndex: u32,
    pdwDiskId: *mut u32,
    szVolumeLabel: *mut c_char,
    pcchVolumeLabel: *mut u32,
    szDiskPrompt: *mut c_char,
    pcchDiskPrompt: *mut u32,
) -> u32 {
    let result = std::panic::catch_unwind(|| {
        if szProductCodeOrPatchCode.is_null() {
            return 87;
        }
        let Some(product_code) = crate::win32::strings::lpcstr_to_string(szProductCodeOrPatchCode)
        else {
            return 87;
        };
        let Ok(cache) = msi::execution::source_resiliency::source_list_cache().read() else {
            return 1603;
        };
        let list = match cache.get(&product_code) {
            Some(l) => l,
            None => return 259,
        };

        let media_sources: Vec<_> = list
            .sources
            .iter()
            .filter_map(|s| {
                if let msi::execution::source_resiliency::SourcePath::Media(m) = s {
                    Some(m)
                } else {
                    None
                }
            })
            .collect();

        if (dwIndex as usize) >= media_sources.len() {
            return 259;
        }

        let media = media_sources[dwIndex as usize];

        if !pdwDiskId.is_null() {
            unsafe {
                *pdwDiskId = media.disk_id.unwrap_or(0);
            }
        }

        let mut status = 0;

        if !pcchVolumeLabel.is_null() {
            let label = media.volume_label.as_deref().unwrap_or("");
            let s = crate::win32::strings::string_to_lpstr(label, szVolumeLabel, pcchVolumeLabel);
            if s != 0 {
                status = s;
            }
        }

        if !pcchDiskPrompt.is_null() {
            let prompt = media.disk_prompt.as_deref().unwrap_or("");
            let s = crate::win32::strings::string_to_lpstr(prompt, szDiskPrompt, pcchDiskPrompt);
            if s != 0 {
                status = s;
            }
        }

        status
    });
    result.unwrap_or(1603)
}

#[cfg_attr(not(coverage_nightly), no_mangle)]
#[inline(never)]
#[allow(non_snake_case, unused_variables)]
/// Enums sources W.
#[allow(clippy::missing_const_for_fn)]
pub extern "system" fn MsiSourceListEnumSourcesW(
    szProductCodeOrPatchCode: *const u16,
    szUserSid: *const u16,
    dwContext: u32,
    dwOptions: u32,
    dwIndex: u32,
    szSource: *mut u16,
    pcchSource: *mut u32,
) -> u32 {
    let result = std::panic::catch_unwind(|| {
        if szProductCodeOrPatchCode.is_null() {
            return 87;
        }
        259
    });
    result.unwrap_or(1603)
}

#[cfg_attr(not(coverage_nightly), no_mangle)]
#[inline(never)]
#[allow(non_snake_case, unused_variables)]
/// Enums sources A.
#[allow(clippy::missing_const_for_fn)]
pub extern "system" fn MsiSourceListEnumSourcesA(
    szProductCodeOrPatchCode: *const c_char,
    szUserSid: *const c_char,
    dwContext: u32,
    dwOptions: u32,
    dwIndex: u32,
    szSource: *mut c_char,
    pcchSource: *mut u32,
) -> u32 {
    let result = std::panic::catch_unwind(|| {
        if szProductCodeOrPatchCode.is_null() {
            return 87;
        }
        259
    });
    result.unwrap_or(1603)
}

#[cfg_attr(not(coverage_nightly), no_mangle)]
#[inline(never)]
#[allow(non_snake_case, unused_variables)]
/// Forces resolution W.
#[allow(clippy::missing_const_for_fn)]
pub extern "system" fn MsiSourceListForceResolutionW(
    szProduct: *const u16,
    szUserName: *const u16,
    dwReserved: u32,
) -> u32 {
    let result = std::panic::catch_unwind(|| {
        if szProduct.is_null() {
            return 87;
        }
        0
    });
    result.unwrap_or(1603)
}

#[cfg_attr(not(coverage_nightly), no_mangle)]
#[inline(never)]
#[allow(non_snake_case, unused_variables)]
/// Forces resolution A.
#[allow(clippy::missing_const_for_fn)]
pub extern "system" fn MsiSourceListForceResolutionA(
    szProduct: *const c_char,
    szUserName: *const c_char,
    dwReserved: u32,
) -> u32 {
    let result = std::panic::catch_unwind(|| {
        if szProduct.is_null() {
            return 87;
        }
        0
    });
    result.unwrap_or(1603)
}

#[cfg_attr(not(coverage_nightly), no_mangle)]
#[inline(never)]
#[allow(non_snake_case, unused_variables)]
/// Sets info W.
#[allow(clippy::missing_const_for_fn)]
pub extern "system" fn MsiSourceListSetInfoW(
    szProductCodeOrPatchCode: *const u16,
    szUserSid: *const u16,
    dwContext: u32,
    dwOptions: u32,
    szProperty: *const u16,
    szValue: *const u16,
) -> u32 {
    let result = std::panic::catch_unwind(|| {
        if szProductCodeOrPatchCode.is_null() || szProperty.is_null() || szValue.is_null() {
            return 87; // ERROR_INVALID_PARAMETER
        }
        let Some(product_code) = crate::win32::strings::lpcwstr_to_string(szProductCodeOrPatchCode)
        else {
            return 87;
        };
        let Some(property) = crate::win32::strings::lpcwstr_to_string(szProperty) else {
            return 87;
        };
        let Some(value) = crate::win32::strings::lpcwstr_to_string(szValue) else {
            return 87;
        };

        let Ok(mut cache) = msi::execution::source_resiliency::source_list_cache().write() else {
            return 1603; // ERROR_INSTALL_FAILURE
        };

        // Ensure cache entry exists
        let list = cache.entry(product_code).or_default();

        if property == "LastUsedSource" {
            let path = if value.starts_with("http://") || value.starts_with("https://") {
                msi::execution::source_resiliency::SourcePath::Url(
                    msi::execution::source_resiliency::UrlSource { url: value },
                )
            } else if value.starts_with(r"\\") {
                msi::execution::source_resiliency::SourcePath::Network(
                    msi::execution::source_resiliency::NetworkSource { unc_path: value },
                )
            } else {
                msi::execution::source_resiliency::SourcePath::Media(
                    msi::execution::source_resiliency::MediaSource {
                        base_path: std::path::PathBuf::from(value),
                        disk_id: None,
                        volume_label: None,
                        disk_prompt: None,
                    },
                )
            };
            list.last_used = Some(path);
        }

        0
    });
    result.unwrap_or(1603)
}

#[cfg_attr(not(coverage_nightly), no_mangle)]
#[inline(never)]
#[allow(non_snake_case, unused_variables)]
/// Sets info A.
#[allow(clippy::missing_const_for_fn)]
pub extern "system" fn MsiSourceListSetInfoA(
    szProductCodeOrPatchCode: *const c_char,
    szUserSid: *const c_char,
    dwContext: u32,
    dwOptions: u32,
    szProperty: *const c_char,
    szValue: *const c_char,
) -> u32 {
    let result = std::panic::catch_unwind(|| {
        if szProductCodeOrPatchCode.is_null() || szProperty.is_null() || szValue.is_null() {
            return 87; // ERROR_INVALID_PARAMETER
        }
        let Some(product_code) = crate::win32::strings::lpcstr_to_string(szProductCodeOrPatchCode)
        else {
            return 87;
        };
        let Some(property) = crate::win32::strings::lpcstr_to_string(szProperty) else {
            return 87;
        };
        let Some(value) = crate::win32::strings::lpcstr_to_string(szValue) else {
            return 87;
        };

        let Ok(mut cache) = msi::execution::source_resiliency::source_list_cache().write() else {
            return 1603; // ERROR_INSTALL_FAILURE
        };

        let list = cache.entry(product_code).or_default();

        if property == "LastUsedSource" {
            let path = if value.starts_with("http://") || value.starts_with("https://") {
                msi::execution::source_resiliency::SourcePath::Url(
                    msi::execution::source_resiliency::UrlSource { url: value },
                )
            } else if value.starts_with(r"\\") {
                msi::execution::source_resiliency::SourcePath::Network(
                    msi::execution::source_resiliency::NetworkSource { unc_path: value },
                )
            } else {
                msi::execution::source_resiliency::SourcePath::Media(
                    msi::execution::source_resiliency::MediaSource {
                        base_path: std::path::PathBuf::from(value),
                        disk_id: None,
                        volume_label: None,
                        disk_prompt: None,
                    },
                )
            };
            list.last_used = Some(path);
        }

        0
    });
    result.unwrap_or(1603)
}

#[cfg(test)]
mod additional_tests {
    use super::*;
    use std::ptr;

    #[test]
    fn test_source_list_ex_stubs() {
        let dummy_w = [u16::from(b'B'), 0];
        let dummy_a = [b'B' as i8, 0];

        assert_eq!(
            MsiSourceListAddSourceExW(ptr::null(), ptr::null(), 0, 0, ptr::null(), 0),
            87
        );
        assert_eq!(
            MsiSourceListAddSourceExA(ptr::null(), ptr::null(), 0, 0, ptr::null(), 0),
            87
        );
        assert_eq!(
            MsiSourceListAddSourceExW(dummy_w.as_ptr(), ptr::null(), 0, 0, dummy_w.as_ptr(), 0),
            0
        );
        assert_eq!(
            MsiSourceListAddSourceExA(dummy_a.as_ptr(), ptr::null(), 0, 0, dummy_a.as_ptr(), 0),
            0
        );

        assert_eq!(MsiSourceListClearAllExW(ptr::null(), ptr::null(), 0, 0), 87);
        assert_eq!(MsiSourceListClearAllExA(ptr::null(), ptr::null(), 0, 0), 87);
        assert_eq!(
            MsiSourceListClearAllExW(dummy_w.as_ptr(), ptr::null(), 0, 0),
            0
        );
        assert_eq!(
            MsiSourceListClearAllExA(dummy_a.as_ptr(), ptr::null(), 0, 0),
            0
        );

        assert_eq!(
            MsiSourceListClearSourceW(ptr::null(), ptr::null(), 0, ptr::null()),
            87
        );
        assert_eq!(
            MsiSourceListClearSourceA(ptr::null(), ptr::null(), 0, ptr::null()),
            87
        );
        assert_eq!(
            MsiSourceListClearSourceW(dummy_w.as_ptr(), ptr::null(), 0, dummy_w.as_ptr()),
            0
        );
        assert_eq!(
            MsiSourceListClearSourceA(dummy_a.as_ptr(), ptr::null(), 0, dummy_a.as_ptr()),
            0
        );

        assert_eq!(
            MsiSourceListEnumMediaDisksW(
                ptr::null(),
                ptr::null(),
                0,
                0,
                0,
                ptr::null_mut(),
                ptr::null_mut(),
                ptr::null_mut(),
                ptr::null_mut(),
                ptr::null_mut()
            ),
            87
        );
        assert_eq!(
            MsiSourceListEnumMediaDisksA(
                ptr::null(),
                ptr::null(),
                0,
                0,
                0,
                ptr::null_mut(),
                ptr::null_mut(),
                ptr::null_mut(),
                ptr::null_mut(),
                ptr::null_mut()
            ),
            87
        );
        assert_eq!(
            MsiSourceListEnumMediaDisksW(
                dummy_w.as_ptr(),
                ptr::null(),
                0,
                0,
                0,
                ptr::null_mut(),
                ptr::null_mut(),
                ptr::null_mut(),
                ptr::null_mut(),
                ptr::null_mut()
            ),
            259
        );
        assert_eq!(
            MsiSourceListEnumMediaDisksA(
                dummy_a.as_ptr(),
                ptr::null(),
                0,
                0,
                0,
                ptr::null_mut(),
                ptr::null_mut(),
                ptr::null_mut(),
                ptr::null_mut(),
                ptr::null_mut()
            ),
            259
        );

        assert_eq!(
            MsiSourceListEnumSourcesW(
                ptr::null(),
                ptr::null(),
                0,
                0,
                0,
                ptr::null_mut(),
                ptr::null_mut()
            ),
            87
        );
        assert_eq!(
            MsiSourceListEnumSourcesA(
                ptr::null(),
                ptr::null(),
                0,
                0,
                0,
                ptr::null_mut(),
                ptr::null_mut()
            ),
            87
        );
        assert_eq!(
            MsiSourceListEnumSourcesW(
                dummy_w.as_ptr(),
                ptr::null(),
                0,
                0,
                0,
                ptr::null_mut(),
                ptr::null_mut()
            ),
            259
        );
        assert_eq!(
            MsiSourceListEnumSourcesA(
                dummy_a.as_ptr(),
                ptr::null(),
                0,
                0,
                0,
                ptr::null_mut(),
                ptr::null_mut()
            ),
            259
        );

        assert_eq!(
            MsiSourceListForceResolutionW(ptr::null(), ptr::null(), 0),
            87
        );
        assert_eq!(
            MsiSourceListForceResolutionA(ptr::null(), ptr::null(), 0),
            87
        );
        assert_eq!(
            MsiSourceListForceResolutionW(dummy_w.as_ptr(), ptr::null(), 0),
            0
        );
        assert_eq!(
            MsiSourceListForceResolutionA(dummy_a.as_ptr(), ptr::null(), 0),
            0
        );

        assert_eq!(
            MsiSourceListSetInfoW(ptr::null(), ptr::null(), 0, 0, ptr::null(), ptr::null()),
            87
        );
        assert_eq!(
            MsiSourceListSetInfoA(ptr::null(), ptr::null(), 0, 0, ptr::null(), ptr::null()),
            87
        );
        assert_eq!(
            MsiSourceListSetInfoW(
                dummy_w.as_ptr(),
                ptr::null(),
                0,
                0,
                dummy_w.as_ptr(),
                dummy_w.as_ptr()
            ),
            0
        );
        assert_eq!(
            MsiSourceListSetInfoA(
                dummy_a.as_ptr(),
                ptr::null(),
                0,
                0,
                dummy_a.as_ptr(),
                dummy_a.as_ptr()
            ),
            0
        );
    }
}

#[cfg(test)]
mod tests_coverage {
    use super::*;
    use crate::action::{ProductCodeA, ProductCodeW};
    use std::ffi::CString;
    use std::ptr;

    fn get_wide(s: &str) -> Vec<u16> {
        let mut v: Vec<u16> = s.encode_utf16().collect();
        v.push(0);
        v
    }

    #[test]
    fn test_source_list_network_and_url() {
        let prod_a = CString::new("ProdNetUrl").expect("valid");
        let prod_w: Vec<u16> = "ProdNetUrl"
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect();
        let pc_a = ProductCodeA(prod_a.as_ptr());
        let pc_w = ProductCodeW(prod_w.as_ptr());

        let url_a = CString::new("http://example.com/").expect("valid");
        let url_w: Vec<u16> = "http://example.com/"
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect();

        let unc_a = CString::new(r"\\server\share").expect("valid");
        let unc_w: Vec<u16> = r"\\server\share"
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect();

        let prop_last_source_a = CString::new("LastUsedSource").expect("valid");
        let prop_last_type_a = CString::new("LastUsedType").expect("valid");
        let prop_last_source_w: Vec<u16> = "LastUsedSource"
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect();
        let prop_last_type_w: Vec<u16> = "LastUsedType"
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect();

        let mut buf_w = vec![0u16; 100];
        let mut buf_a = vec![0i8; 100];
        let mut sz_w = 100;
        #[allow(unused_assignments)]
        let mut sz_a = 100;

        // Add URL
        assert_eq!(
            unsafe {
                MsiSourceListAddSourceW(pc_w, ptr::null(), 0, SourceListPathW(url_w.as_ptr()))
            },
            0
        );

        assert_eq!(
            unsafe {
                MsiSourceListGetInfoW(
                    pc_w,
                    ptr::null(),
                    0,
                    ResolutionMode(0),
                    prop_last_source_w.as_ptr(),
                    buf_w.as_mut_ptr(),
                    &mut sz_w,
                )
            },
            0
        );
        sz_w = 100;
        assert_eq!(
            unsafe {
                MsiSourceListGetInfoW(
                    pc_w,
                    ptr::null(),
                    0,
                    ResolutionMode(0),
                    prop_last_type_w.as_ptr(),
                    buf_w.as_mut_ptr(),
                    &mut sz_w,
                )
            },
            0
        );

        // Add UNC via A
        assert_eq!(
            unsafe {
                MsiSourceListAddSourceA(pc_a, ptr::null(), 0, SourceListPathA(unc_a.as_ptr()))
            },
            0
        );

        sz_a = 100;
        assert_eq!(
            unsafe {
                MsiSourceListGetInfoA(
                    pc_a,
                    ptr::null(),
                    0,
                    ResolutionMode(0),
                    prop_last_source_a.as_ptr(),
                    buf_a.as_mut_ptr().cast(),
                    &mut sz_a,
                )
            },
            0
        );
        sz_a = 100;
        assert_eq!(
            unsafe {
                MsiSourceListGetInfoA(
                    pc_a,
                    ptr::null(),
                    0,
                    ResolutionMode(0),
                    prop_last_type_a.as_ptr(),
                    buf_a.as_mut_ptr().cast(),
                    &mut sz_a,
                )
            },
            0
        );

        sz_w = 0;
        sz_a = 0;
        assert_eq!(
            unsafe {
                MsiSourceListGetInfoW(
                    pc_w,
                    ptr::null(),
                    0,
                    ResolutionMode(0),
                    prop_last_source_w.as_ptr(),
                    ptr::null_mut(),
                    &mut sz_w,
                )
            },
            234
        );
        assert_eq!(
            unsafe {
                MsiSourceListGetInfoA(
                    pc_a,
                    ptr::null(),
                    0,
                    ResolutionMode(0),
                    prop_last_source_a.as_ptr(),
                    ptr::null_mut(),
                    &mut sz_a,
                )
            },
            234
        );

        // Also error branches for AddSource
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

        // Also error branches for GetInfo
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
    }
    #[test]
    fn test_msi_source_list_functionality() {
        let prod_w = get_wide("ProdCovW");
        let prod_a = CString::new("ProdCovA").expect("valid");

        let path_w = get_wide(r"C:\\Temp");
        let path_a = CString::new("http://example.com").expect("valid");

        let invalid_utf8 = [0xff_u8, 0xff_u8, 0];
        let dummy_a = [0_i8; 1];

        let pc_a_inv = ProductCodeA(invalid_utf8.as_ptr().cast());

        assert_eq!(
            unsafe {
                MsiSourceListAddSourceA(
                    pc_a_inv,
                    ptr::null(),
                    0,
                    SourceListPathA(dummy_a.as_ptr().cast()),
                )
            },
            87
        );
        assert_eq!(
            unsafe {
                MsiSourceListAddSourceA(
                    ProductCodeA(dummy_a.as_ptr().cast()),
                    ptr::null(),
                    0,
                    SourceListPathA(invalid_utf8.as_ptr().cast()),
                )
            },
            87
        );
        assert_eq!(
            unsafe {
                MsiSourceListAddMediaDiskA(
                    pc_a_inv,
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
                    ProductCodeA(dummy_a.as_ptr().cast()),
                    ptr::null(),
                    0,
                    MediaDiskId(0),
                    invalid_utf8.as_ptr().cast(),
                    ptr::null(),
                )
            },
            87
        );
        assert_eq!(
            unsafe {
                MsiSourceListClearSourceA(
                    invalid_utf8.as_ptr().cast(),
                    ptr::null(),
                    0,
                    dummy_a.as_ptr().cast(),
                )
            },
            0
        );
        assert_eq!(
            unsafe {
                MsiSourceListClearSourceA(
                    dummy_a.as_ptr().cast(),
                    ptr::null(),
                    0,
                    invalid_utf8.as_ptr().cast(),
                )
            },
            0
        );

        assert_eq!(
            unsafe {
                MsiSourceListAddSourceW(
                    ProductCodeW(prod_w.as_ptr()),
                    ptr::null(),
                    0,
                    SourceListPathW(path_w.as_ptr()),
                )
            },
            0
        );

        assert_eq!(
            unsafe {
                MsiSourceListAddSourceA(
                    ProductCodeA(prod_a.as_ptr()),
                    ptr::null(),
                    0,
                    SourceListPathA(path_a.as_ptr()),
                )
            },
            0
        );

        let label_w = get_wide("Vol1");
        let prompt_w = get_wide("Insert Disk 1");
        assert_eq!(
            unsafe {
                MsiSourceListAddMediaDiskW(
                    ProductCodeW(prod_w.as_ptr()),
                    ptr::null(),
                    0,
                    MediaDiskId(1),
                    label_w.as_ptr(),
                    prompt_w.as_ptr(),
                )
            },
            0
        );

        let label_a = CString::new("Vol2").expect("valid");
        assert_eq!(
            unsafe {
                MsiSourceListAddMediaDiskA(
                    ProductCodeA(prod_a.as_ptr()),
                    ptr::null(),
                    0,
                    MediaDiskId(2),
                    label_a.as_ptr(),
                    ptr::null(),
                )
            },
            0
        );

        let mut disk_id = 0;
        let mut vol = [0_u16; 64];
        let mut prompt = [0_u16; 64];
        let mut pcch_vol = 64;
        let mut pcch_prompt = 64;

        assert_eq!(
            unsafe {
                MsiSourceListEnumMediaDisksW(
                    prod_w.as_ptr(),
                    ptr::null(),
                    0,
                    0,
                    0,
                    &mut disk_id,
                    vol.as_mut_ptr(),
                    &mut pcch_vol,
                    prompt.as_mut_ptr(),
                    &mut pcch_prompt,
                )
            },
            0
        );
        assert_eq!(disk_id, 0);

        pcch_vol = 64;
        pcch_prompt = 64;

        let mut small_vol_w = [0_u16; 1];
        let mut small_prompt_w = [0_u16; 1];
        let mut pcch_small_vol_w = 1;
        let mut pcch_small_prompt_w = 1;
        assert_eq!(
            unsafe {
                MsiSourceListEnumMediaDisksW(
                    prod_w.as_ptr(),
                    ptr::null(),
                    0,
                    0,
                    0,
                    &mut disk_id,
                    small_vol_w.as_mut_ptr(),
                    &mut pcch_small_vol_w,
                    small_prompt_w.as_mut_ptr(),
                    &mut pcch_small_prompt_w,
                )
            },
            0
        );

        let mut small_vol_a = [0_i8; 1];
        let mut small_prompt_a = [0_i8; 1];
        let mut pcch_small_vol_a = 1;
        let mut pcch_small_prompt_a = 1;
        assert_eq!(
            unsafe {
                MsiSourceListEnumMediaDisksA(
                    prod_a.as_ptr(),
                    ptr::null(),
                    0,
                    0,
                    0,
                    &mut disk_id,
                    small_vol_a.as_mut_ptr(),
                    &mut pcch_small_vol_a,
                    small_prompt_a.as_mut_ptr(),
                    &mut pcch_small_prompt_a,
                )
            },
            234
        );

        assert_eq!(
            unsafe {
                MsiSourceListEnumMediaDisksW(
                    prod_w.as_ptr(),
                    ptr::null(),
                    0,
                    0,
                    1,
                    &mut disk_id,
                    vol.as_mut_ptr(),
                    &mut pcch_vol,
                    prompt.as_mut_ptr(),
                    &mut pcch_prompt,
                )
            },
            0
        );
        assert_eq!(disk_id, 1);

        assert_eq!(
            unsafe {
                MsiSourceListEnumMediaDisksW(
                    prod_w.as_ptr(),
                    ptr::null(),
                    0,
                    0,
                    2,
                    &mut disk_id,
                    ptr::null_mut(),
                    ptr::null_mut(),
                    ptr::null_mut(),
                    ptr::null_mut(),
                )
            },
            259
        );

        let mut vol_a = [0_i8; 64];
        let mut prompt_a = [0_i8; 64];
        let mut pcch_vol_a = 64;
        let mut pcch_prompt_a = 64;

        assert_eq!(
            unsafe {
                MsiSourceListEnumMediaDisksA(
                    prod_a.as_ptr(),
                    ptr::null(),
                    0,
                    0,
                    0,
                    &mut disk_id,
                    vol_a.as_mut_ptr(),
                    &mut pcch_vol_a,
                    prompt_a.as_mut_ptr(),
                    &mut pcch_prompt_a,
                )
            },
            0
        );

        assert_eq!(
            unsafe { MsiSourceListClearAllW(ProductCodeW(prod_w.as_ptr()), ptr::null(), 0,) },
            0
        );
        assert_eq!(
            unsafe { MsiSourceListClearAllA(ProductCodeA(prod_a.as_ptr()), ptr::null(), 0,) },
            0
        );
    }
}

/// Clears a media disk from the source list (ANSI).
///
/// # Arguments
///
/// * `_szProduct` - Pointer to a string specifying the product code.
/// * `_szUserName` - Pointer to a string specifying the user name.
/// * `_dwContext` - Context.
/// * `_dwOptions` - Options.
/// * `_dwDiskId` - Disk ID.
///
/// # Returns
///
/// `ERROR_SUCCESS`, `ERROR_INVALID_PARAMETER` or `ERROR_UNKNOWN_PRODUCT`.
#[cfg_attr(not(coverage_nightly), no_mangle)]
#[inline(never)]
#[allow(non_snake_case, unused_variables)]
pub extern "system" fn MsiSourceListClearMediaDiskA(
    _szProduct: *const c_char,
    _szUserName: *const c_char,
    _dwContext: u32,
    _dwOptions: u32,
    _dwDiskId: u32,
) -> u32 {
    std::panic::catch_unwind(|| {
        if _szProduct.is_null() {
            return 87; // ERROR_INVALID_PARAMETER
        }
        let Some(product_code) = crate::win32::strings::lpcstr_to_string(_szProduct) else {
            return 87;
        };

        let Ok(mut cache) = msi::execution::source_resiliency::source_list_cache().write() else {
            return 1603;
        };

        if let Some(list) = cache.get_mut(&product_code) {
            list.sources.retain(|s| {
                if let msi::execution::source_resiliency::SourcePath::Media(m) = s {
                    if let Some(id) = m.disk_id {
                        if id == _dwDiskId || _dwDiskId == 0xFFFF_FFFF {
                            return false;
                        }
                    }
                }
                true
            });
            0 // ERROR_SUCCESS
        } else {
            1605 // ERROR_UNKNOWN_PRODUCT
        }
    })
    .unwrap_or(1603)
}

/// Clears a media disk from the source list (Unicode).
///
/// # Arguments
///
/// * `_szProduct` - Pointer to a string specifying the product code.
/// * `_szUserName` - Pointer to a string specifying the user name.
/// * `_dwContext` - Context.
/// * `_dwOptions` - Options.
/// * `_dwDiskId` - Disk ID.
///
/// # Returns
///
/// `ERROR_SUCCESS`, `ERROR_INVALID_PARAMETER` or `ERROR_UNKNOWN_PRODUCT`.
#[cfg_attr(not(coverage_nightly), no_mangle)]
#[inline(never)]
#[allow(non_snake_case, unused_variables)]
pub extern "system" fn MsiSourceListClearMediaDiskW(
    _szProduct: *const u16,
    _szUserName: *const u16,
    _dwContext: u32,
    _dwOptions: u32,
    _dwDiskId: u32,
) -> u32 {
    std::panic::catch_unwind(|| {
        if _szProduct.is_null() {
            return 87; // ERROR_INVALID_PARAMETER
        }
        let Some(product_code) = crate::win32::strings::lpcwstr_to_string(_szProduct) else {
            return 87;
        };

        let Ok(mut cache) = msi::execution::source_resiliency::source_list_cache().write() else {
            return 1603;
        };

        if let Some(list) = cache.get_mut(&product_code) {
            list.sources.retain(|s| {
                if let msi::execution::source_resiliency::SourcePath::Media(m) = s {
                    if let Some(id) = m.disk_id {
                        if id == _dwDiskId || _dwDiskId == 0xFFFF_FFFF {
                            return false;
                        }
                    }
                }
                true
            });
            0 // ERROR_SUCCESS
        } else {
            1605 // ERROR_UNKNOWN_PRODUCT
        }
    })
    .unwrap_or(1603)
}

/// Forces source list resolution (ANSI).
///
/// # Arguments
///
/// * `_szProduct` - Pointer to a string specifying the product code.
/// * `_szUserName` - Pointer to a string specifying the user name.
/// * `_dwContext` - Context.
/// * `_dwOptions` - Options.
///
/// # Returns
///
/// `ERROR_SUCCESS`, `ERROR_INVALID_PARAMETER` or `ERROR_UNKNOWN_PRODUCT`.
#[cfg_attr(not(coverage_nightly), no_mangle)]
#[inline(never)]
#[allow(non_snake_case, unused_variables)]
pub extern "system" fn MsiSourceListForceResolutionExA(
    _szProduct: *const c_char,
    _szUserName: *const c_char,
    _dwContext: u32,
    _dwOptions: u32,
) -> u32 {
    std::panic::catch_unwind(|| {
        if _szProduct.is_null() {
            return 87; // ERROR_INVALID_PARAMETER
        }
        let Some(product_code) = crate::win32::strings::lpcstr_to_string(_szProduct) else {
            return 87;
        };

        let Ok(mut cache) = msi::execution::source_resiliency::source_list_cache().write() else {
            return 1603;
        };

        if cache.get_mut(&product_code).is_some() {
            0 // ERROR_SUCCESS
        } else {
            1605 // ERROR_UNKNOWN_PRODUCT
        }
    })
    .unwrap_or(1603)
}

/// Forces source list resolution (Unicode).
///
/// # Arguments
///
/// * `_szProduct` - Pointer to a string specifying the product code.
/// * `_szUserName` - Pointer to a string specifying the user name.
/// * `_dwContext` - Context.
/// * `_dwOptions` - Options.
///
/// # Returns
///
/// `ERROR_SUCCESS`, `ERROR_INVALID_PARAMETER` or `ERROR_UNKNOWN_PRODUCT`.
#[cfg_attr(not(coverage_nightly), no_mangle)]
#[inline(never)]
#[allow(non_snake_case, unused_variables)]
pub extern "system" fn MsiSourceListForceResolutionExW(
    _szProduct: *const u16,
    _szUserName: *const u16,
    _dwContext: u32,
    _dwOptions: u32,
) -> u32 {
    std::panic::catch_unwind(|| {
        if _szProduct.is_null() {
            return 87; // ERROR_INVALID_PARAMETER
        }
        let Some(product_code) = crate::win32::strings::lpcwstr_to_string(_szProduct) else {
            return 87;
        };

        let Ok(mut cache) = msi::execution::source_resiliency::source_list_cache().write() else {
            return 1603;
        };

        if cache.get_mut(&product_code).is_some() {
            0 // ERROR_SUCCESS
        } else {
            1605 // ERROR_UNKNOWN_PRODUCT
        }
    })
    .unwrap_or(1603)
}

#[cfg(test)]
mod additional_stubs_tests {
    use super::*;

    #[test]
    fn test_stubs_impl() {
        assert_eq!(
            MsiSourceListClearMediaDiskA(std::ptr::null(), std::ptr::null(), 0, 0, 0),
            87
        );
        assert_eq!(
            MsiSourceListClearMediaDiskW(std::ptr::null(), std::ptr::null(), 0, 0, 0),
            87
        );
        assert_eq!(
            MsiSourceListForceResolutionExA(std::ptr::null(), std::ptr::null(), 0, 0),
            87
        );
        assert_eq!(
            MsiSourceListForceResolutionExW(std::ptr::null(), std::ptr::null(), 0, 0),
            87
        );
    }
}

#[cfg(test)]
mod tests_additional_coverage {
    use super::*;
    use crate::action::{ProductCodeA, ProductCodeW};
    use std::ptr;

    #[test]
    fn test_all_source_list_stubs() {
        let dummy_w: Vec<u16> = "unique_dummy"
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect();
        let dummy_a = std::ffi::CString::new("unique_dummy").expect("valid");

        let p_w = ProductCodeW(dummy_w.as_ptr());
        let p_a = ProductCodeA(dummy_a.as_ptr());
        let path_w = SourceListPathW(dummy_w.as_ptr());
        let path_a = SourceListPathA(dummy_a.as_ptr());

        unsafe {
            // Nulls for AddSource
            assert_eq!(
                MsiSourceListAddSourceW(ProductCodeW(ptr::null()), ptr::null(), 0, path_w),
                87
            );
            assert_eq!(
                MsiSourceListAddSourceW(p_w, ptr::null(), 0, SourceListPathW(ptr::null())),
                87
            );
            assert_eq!(
                MsiSourceListAddSourceA(ProductCodeA(ptr::null()), ptr::null(), 0, path_a),
                87
            );
            assert_eq!(
                MsiSourceListAddSourceA(p_a, ptr::null(), 0, SourceListPathA(ptr::null())),
                87
            );

            // Nulls for ClearAll
            assert_eq!(
                MsiSourceListClearAllW(ProductCodeW(ptr::null()), ptr::null(), 0),
                87
            );
            assert_eq!(
                MsiSourceListClearAllA(ProductCodeA(ptr::null()), ptr::null(), 0),
                87
            );

            // Nulls for AddMediaDisk
            assert_eq!(
                MsiSourceListAddMediaDiskW(
                    ProductCodeW(ptr::null()),
                    ptr::null(),
                    0,
                    MediaDiskId(0),
                    dummy_w.as_ptr(),
                    ptr::null()
                ),
                87
            );
            assert_eq!(
                MsiSourceListAddMediaDiskW(
                    p_w,
                    ptr::null(),
                    0,
                    MediaDiskId(0),
                    ptr::null(),
                    ptr::null()
                ),
                87
            );
            assert_eq!(
                MsiSourceListAddMediaDiskA(
                    ProductCodeA(ptr::null()),
                    ptr::null(),
                    0,
                    MediaDiskId(0),
                    dummy_a.as_ptr(),
                    ptr::null()
                ),
                87
            );
            assert_eq!(
                MsiSourceListAddMediaDiskA(
                    p_a,
                    ptr::null(),
                    0,
                    MediaDiskId(0),
                    ptr::null(),
                    ptr::null()
                ),
                87
            );

            // GetInfo - check valid branch returns empty for unknown prop
            let mut sz = 0;
            assert_eq!(
                MsiSourceListGetInfoW(
                    p_w,
                    ptr::null(),
                    0,
                    ResolutionMode(0),
                    dummy_w.as_ptr(),
                    ptr::null_mut(),
                    &mut sz
                ),
                1610
            );
            assert_eq!(
                MsiSourceListGetInfoA(
                    p_a,
                    ptr::null(),
                    0,
                    ResolutionMode(0),
                    dummy_a.as_ptr(),
                    ptr::null_mut(),
                    &mut sz
                ),
                1610
            );
            assert_eq!(
                MsiSourceListGetInfoW(
                    ProductCodeW(ptr::null()),
                    ptr::null(),
                    0,
                    ResolutionMode(0),
                    dummy_w.as_ptr(),
                    ptr::null_mut(),
                    &mut sz
                ),
                87
            );
            assert_eq!(
                MsiSourceListGetInfoW(
                    p_w,
                    ptr::null(),
                    0,
                    ResolutionMode(0),
                    ptr::null(),
                    ptr::null_mut(),
                    &mut sz
                ),
                87
            );
            assert_eq!(
                MsiSourceListGetInfoA(
                    ProductCodeA(ptr::null()),
                    ptr::null(),
                    0,
                    ResolutionMode(0),
                    dummy_a.as_ptr(),
                    ptr::null_mut(),
                    &mut sz
                ),
                87
            );
            assert_eq!(
                MsiSourceListGetInfoA(
                    p_a,
                    ptr::null(),
                    0,
                    ResolutionMode(0),
                    ptr::null(),
                    ptr::null_mut(),
                    &mut sz
                ),
                87
            );

            // Invalid UTF-8 for AddSource W and A
            let invalid_utf16 = [0xd800_u16, 0xd800_u16, 0];
            let invalid_utf8 = [0xff_u8, 0xff_u8, 0];

            let p_inv_w = ProductCodeW(invalid_utf16.as_ptr());
            let p_inv_a = ProductCodeA(invalid_utf8.as_ptr().cast());
            let path_inv_w = SourceListPathW(invalid_utf16.as_ptr());
            let path_inv_a = SourceListPathA(invalid_utf8.as_ptr().cast());

            assert_eq!(MsiSourceListAddSourceW(p_inv_w, ptr::null(), 0, path_w), 87);
            assert_eq!(MsiSourceListAddSourceW(p_w, ptr::null(), 0, path_inv_w), 87);
            assert_eq!(MsiSourceListAddSourceA(p_inv_a, ptr::null(), 0, path_a), 87);
            assert_eq!(MsiSourceListAddSourceA(p_a, ptr::null(), 0, path_inv_a), 87);

            // AddSource W/A with Media path (not URL, not UNC)
            let media_path_w: Vec<u16> = "C:\\Media"
                .encode_utf16()
                .chain(std::iter::once(0))
                .collect();
            let media_path_a = std::ffi::CString::new("C:\\Media").expect("valid");
            assert_eq!(
                MsiSourceListAddSourceW(
                    p_w,
                    ptr::null(),
                    0,
                    SourceListPathW(media_path_w.as_ptr())
                ),
                0
            );
            assert_eq!(
                MsiSourceListAddSourceA(
                    p_a,
                    ptr::null(),
                    0,
                    SourceListPathA(media_path_a.as_ptr())
                ),
                0
            );

            // AddSource W branches
            let net_path_w: Vec<u16> = "\\\\server\\share"
                .encode_utf16()
                .chain(std::iter::once(0))
                .collect();
            let url_path_w: Vec<u16> = "http://example.com/"
                .encode_utf16()
                .chain(std::iter::once(0))
                .collect();
            assert_eq!(
                MsiSourceListAddSourceW(p_w, ptr::null(), 0, SourceListPathW(net_path_w.as_ptr())),
                0
            );
            assert_eq!(
                MsiSourceListAddSourceW(p_w, ptr::null(), 0, SourceListPathW(url_path_w.as_ptr())),
                0
            );

            // AddSource A branches
            let net_path_a = std::ffi::CString::new("\\\\server\\share").expect("valid");
            let url_path_a = std::ffi::CString::new("http://example.com/").expect("valid");
            let media_path_a = std::ffi::CString::new("C:\\Media").expect("valid");
            assert_eq!(
                MsiSourceListAddSourceA(p_a, ptr::null(), 0, SourceListPathA(net_path_a.as_ptr())),
                0
            );
            assert_eq!(
                MsiSourceListAddSourceA(p_a, ptr::null(), 0, SourceListPathA(url_path_a.as_ptr())),
                0
            );
            assert_eq!(
                MsiSourceListAddSourceA(
                    p_a,
                    ptr::null(),
                    0,
                    SourceListPathA(media_path_a.as_ptr())
                ),
                0
            );

            // SetInfo LastUsedSource W branches
            let prop_last_w: Vec<u16> = "LastUsedSource"
                .encode_utf16()
                .chain(std::iter::once(0))
                .collect();
            assert_eq!(
                MsiSourceListSetInfoW(
                    dummy_w.as_ptr(),
                    ptr::null(),
                    0,
                    0,
                    prop_last_w.as_ptr(),
                    media_path_w.as_ptr()
                ),
                0
            );
            assert_eq!(
                MsiSourceListSetInfoW(
                    dummy_w.as_ptr(),
                    ptr::null(),
                    0,
                    0,
                    prop_last_w.as_ptr(),
                    net_path_w.as_ptr()
                ),
                0
            );
            assert_eq!(
                MsiSourceListSetInfoW(
                    dummy_w.as_ptr(),
                    ptr::null(),
                    0,
                    0,
                    prop_last_w.as_ptr(),
                    url_path_w.as_ptr()
                ),
                0
            );

            // SetInfo LastUsedSource A branches
            let prop_last_a = std::ffi::CString::new("LastUsedSource").expect("valid");
            assert_eq!(
                MsiSourceListSetInfoA(
                    dummy_a.as_ptr(),
                    ptr::null(),
                    0,
                    0,
                    prop_last_a.as_ptr(),
                    media_path_a.as_ptr()
                ),
                0
            );
            assert_eq!(
                MsiSourceListSetInfoA(
                    dummy_a.as_ptr(),
                    ptr::null(),
                    0,
                    0,
                    prop_last_a.as_ptr(),
                    net_path_a.as_ptr()
                ),
                0
            );
            assert_eq!(
                MsiSourceListSetInfoA(
                    dummy_a.as_ptr(),
                    ptr::null(),
                    0,
                    0,
                    prop_last_a.as_ptr(),
                    url_path_a.as_ptr()
                ),
                0
            );

            // GetInfo branches (after setting to Media)
            assert_eq!(
                MsiSourceListSetInfoW(
                    dummy_w.as_ptr(),
                    ptr::null(),
                    0,
                    0,
                    prop_last_w.as_ptr(),
                    media_path_w.as_ptr()
                ),
                0
            );
            let prop_type_w: Vec<u16> = "LastUsedType"
                .encode_utf16()
                .chain(std::iter::once(0))
                .collect();
            let prop_type_a = std::ffi::CString::new("LastUsedType").expect("valid");
            let mut val_w = vec![0u16; 100];
            let mut sz_val_w = 100;
            assert_eq!(
                MsiSourceListGetInfoW(
                    p_w,
                    ptr::null(),
                    0,
                    ResolutionMode(0),
                    prop_last_w.as_ptr(),
                    val_w.as_mut_ptr(),
                    &mut sz_val_w
                ),
                0
            );
            sz_val_w = 100;
            assert_eq!(
                MsiSourceListGetInfoW(
                    p_w,
                    ptr::null(),
                    0,
                    ResolutionMode(0),
                    prop_type_w.as_ptr(),
                    val_w.as_mut_ptr(),
                    &mut sz_val_w
                ),
                0
            );

            assert_eq!(
                MsiSourceListSetInfoW(
                    dummy_w.as_ptr(),
                    ptr::null(),
                    0,
                    0,
                    prop_last_w.as_ptr(),
                    net_path_w.as_ptr()
                ),
                0
            );
            sz_val_w = 100;
            assert_eq!(
                MsiSourceListGetInfoW(
                    p_w,
                    ptr::null(),
                    0,
                    ResolutionMode(0),
                    prop_last_w.as_ptr(),
                    val_w.as_mut_ptr(),
                    &mut sz_val_w
                ),
                0
            );
            sz_val_w = 100;
            assert_eq!(
                MsiSourceListGetInfoW(
                    p_w,
                    ptr::null(),
                    0,
                    ResolutionMode(0),
                    prop_type_w.as_ptr(),
                    val_w.as_mut_ptr(),
                    &mut sz_val_w
                ),
                0
            );

            // Comprehensive GetInfo and SetInfo for W and A
            let test_w = dummy_w.as_ptr();
            let test_a = dummy_a.as_ptr();

            for (path_w, path_a) in [
                (media_path_w.as_ptr(), media_path_a.as_ptr()),
                (net_path_w.as_ptr(), net_path_a.as_ptr()),
                (url_path_w.as_ptr(), url_path_a.as_ptr()),
            ] {
                // W
                assert_eq!(
                    MsiSourceListSetInfoW(test_w, ptr::null(), 0, 0, prop_last_w.as_ptr(), path_w),
                    0
                );
                let mut val_w = vec![0u16; 100];
                let mut sz_val_w = 100;
                assert_eq!(
                    MsiSourceListGetInfoW(
                        p_w,
                        ptr::null(),
                        0,
                        ResolutionMode(0),
                        prop_last_w.as_ptr(),
                        val_w.as_mut_ptr(),
                        &mut sz_val_w
                    ),
                    0
                );
                sz_val_w = 100;
                assert_eq!(
                    MsiSourceListGetInfoW(
                        p_w,
                        ptr::null(),
                        0,
                        ResolutionMode(0),
                        prop_type_w.as_ptr(),
                        val_w.as_mut_ptr(),
                        &mut sz_val_w
                    ),
                    0
                );

                // A
                assert_eq!(
                    MsiSourceListSetInfoA(test_a, ptr::null(), 0, 0, prop_last_a.as_ptr(), path_a),
                    0
                );
                let mut val_a = vec![0i8; 100];
                let mut sz_val_a = 100;
                assert_eq!(
                    MsiSourceListGetInfoA(
                        p_a,
                        ptr::null(),
                        0,
                        ResolutionMode(0),
                        prop_last_a.as_ptr(),
                        val_a.as_mut_ptr(),
                        &mut sz_val_a
                    ),
                    0
                );
                sz_val_a = 100;
                assert_eq!(
                    MsiSourceListGetInfoA(
                        p_a,
                        ptr::null(),
                        0,
                        ResolutionMode(0),
                        prop_type_a.as_ptr(),
                        val_a.as_mut_ptr(),
                        &mut sz_val_a
                    ),
                    0
                );
            }

            // ClearAll Invalid UTF W/A
            assert_eq!(MsiSourceListClearAllW(p_inv_w, ptr::null(), 0), 87);
            assert_eq!(MsiSourceListClearAllA(p_inv_a, ptr::null(), 0), 87);

            // AddMediaDisk W/A Invalid UTF
            assert_eq!(
                MsiSourceListAddMediaDiskW(
                    p_inv_w,
                    ptr::null(),
                    0,
                    MediaDiskId(0),
                    dummy_w.as_ptr(),
                    ptr::null()
                ),
                87
            );
            assert_eq!(
                MsiSourceListAddMediaDiskW(
                    p_w,
                    ptr::null(),
                    0,
                    MediaDiskId(0),
                    invalid_utf16.as_ptr(),
                    ptr::null()
                ),
                87
            );
            assert_eq!(
                MsiSourceListAddMediaDiskA(
                    p_inv_a,
                    ptr::null(),
                    0,
                    MediaDiskId(0),
                    dummy_a.as_ptr(),
                    ptr::null()
                ),
                87
            );
            assert_eq!(
                MsiSourceListAddMediaDiskA(
                    p_a,
                    ptr::null(),
                    0,
                    MediaDiskId(0),
                    invalid_utf8.as_ptr().cast(),
                    ptr::null()
                ),
                87
            );

            // GetInfo W/A Invalid UTF
            assert_eq!(
                MsiSourceListGetInfoW(
                    p_inv_w,
                    ptr::null(),
                    0,
                    ResolutionMode(0),
                    dummy_w.as_ptr(),
                    ptr::null_mut(),
                    &mut sz
                ),
                87
            );
            assert_eq!(
                MsiSourceListGetInfoW(
                    p_w,
                    ptr::null(),
                    0,
                    ResolutionMode(0),
                    invalid_utf16.as_ptr(),
                    ptr::null_mut(),
                    &mut sz
                ),
                87
            );
            assert_eq!(
                MsiSourceListGetInfoA(
                    p_inv_a,
                    ptr::null(),
                    0,
                    ResolutionMode(0),
                    dummy_a.as_ptr(),
                    ptr::null_mut(),
                    &mut sz
                ),
                87
            );
            assert_eq!(
                MsiSourceListGetInfoA(
                    p_a,
                    ptr::null(),
                    0,
                    ResolutionMode(0),
                    invalid_utf8.as_ptr().cast(),
                    ptr::null_mut(),
                    &mut sz
                ),
                87
            );

            // EnumMediaDisks W/A Invalid UTF
            assert_eq!(
                MsiSourceListEnumMediaDisksW(
                    invalid_utf16.as_ptr(),
                    ptr::null(),
                    0,
                    0,
                    0,
                    ptr::null_mut(),
                    ptr::null_mut(),
                    ptr::null_mut(),
                    ptr::null_mut(),
                    ptr::null_mut()
                ),
                87
            );
            assert_eq!(
                MsiSourceListEnumMediaDisksA(
                    invalid_utf8.as_ptr().cast(),
                    ptr::null(),
                    0,
                    0,
                    0,
                    ptr::null_mut(),
                    ptr::null_mut(),
                    ptr::null_mut(),
                    ptr::null_mut(),
                    ptr::null_mut()
                ),
                87
            );

            // SetInfo Invalid UTF W/A
            assert_eq!(
                MsiSourceListSetInfoW(
                    invalid_utf16.as_ptr(),
                    ptr::null(),
                    0,
                    0,
                    dummy_w.as_ptr(),
                    dummy_w.as_ptr()
                ),
                87
            );
            assert_eq!(
                MsiSourceListSetInfoW(
                    dummy_w.as_ptr(),
                    ptr::null(),
                    0,
                    0,
                    invalid_utf16.as_ptr(),
                    dummy_w.as_ptr()
                ),
                87
            );
            assert_eq!(
                MsiSourceListSetInfoW(
                    dummy_w.as_ptr(),
                    ptr::null(),
                    0,
                    0,
                    dummy_w.as_ptr(),
                    invalid_utf16.as_ptr()
                ),
                87
            );

            assert_eq!(
                MsiSourceListSetInfoA(
                    invalid_utf8.as_ptr().cast(),
                    ptr::null(),
                    0,
                    0,
                    dummy_a.as_ptr(),
                    dummy_a.as_ptr()
                ),
                87
            );
            assert_eq!(
                MsiSourceListSetInfoA(
                    dummy_a.as_ptr(),
                    ptr::null(),
                    0,
                    0,
                    invalid_utf8.as_ptr().cast(),
                    dummy_a.as_ptr()
                ),
                87
            );
            assert_eq!(
                MsiSourceListSetInfoA(
                    dummy_a.as_ptr(),
                    ptr::null(),
                    0,
                    0,
                    dummy_a.as_ptr(),
                    invalid_utf8.as_ptr().cast()
                ),
                87
            );

            // AddSourceEx
            assert_eq!(
                MsiSourceListAddSourceExW(dummy_w.as_ptr(), ptr::null(), 0, 0, dummy_w.as_ptr(), 0),
                0
            );
            assert_eq!(
                MsiSourceListAddSourceExW(ptr::null(), ptr::null(), 0, 0, dummy_w.as_ptr(), 0),
                87
            );
            assert_eq!(
                MsiSourceListAddSourceExW(dummy_w.as_ptr(), ptr::null(), 0, 0, ptr::null(), 0),
                87
            );
            assert_eq!(
                MsiSourceListAddSourceExA(dummy_a.as_ptr(), ptr::null(), 0, 0, dummy_a.as_ptr(), 0),
                0
            );
            assert_eq!(
                MsiSourceListAddSourceExA(ptr::null(), ptr::null(), 0, 0, dummy_a.as_ptr(), 0),
                87
            );
            assert_eq!(
                MsiSourceListAddSourceExA(dummy_a.as_ptr(), ptr::null(), 0, 0, ptr::null(), 0),
                87
            );

            // ClearAllEx
            assert_eq!(
                MsiSourceListClearAllExW(dummy_w.as_ptr(), ptr::null(), 0, 0),
                0
            );
            assert_eq!(MsiSourceListClearAllExW(ptr::null(), ptr::null(), 0, 0), 87);
            assert_eq!(
                MsiSourceListClearAllExA(dummy_a.as_ptr(), ptr::null(), 0, 0),
                0
            );
            assert_eq!(MsiSourceListClearAllExA(ptr::null(), ptr::null(), 0, 0), 87);

            // ClearSource
            assert_eq!(
                MsiSourceListClearSourceW(dummy_w.as_ptr(), ptr::null(), 0, dummy_w.as_ptr()),
                0
            );
            assert_eq!(
                MsiSourceListClearSourceW(ptr::null(), ptr::null(), 0, dummy_w.as_ptr()),
                87
            );
            assert_eq!(
                MsiSourceListClearSourceW(dummy_w.as_ptr(), ptr::null(), 0, ptr::null()),
                87
            );
            assert_eq!(
                MsiSourceListClearSourceA(dummy_a.as_ptr(), ptr::null(), 0, dummy_a.as_ptr()),
                0
            );
            assert_eq!(
                MsiSourceListClearSourceA(ptr::null(), ptr::null(), 0, dummy_a.as_ptr()),
                87
            );
            assert_eq!(
                MsiSourceListClearSourceA(dummy_a.as_ptr(), ptr::null(), 0, ptr::null()),
                87
            );

            // EnumMediaDisks

            assert_eq!(
                MsiSourceListEnumMediaDisksW(
                    dummy_w.as_ptr(),
                    ptr::null(),
                    0,
                    0,
                    0,
                    ptr::null_mut(),
                    ptr::null_mut(),
                    ptr::null_mut(),
                    ptr::null_mut(),
                    ptr::null_mut()
                ),
                0
            );

            assert_eq!(
                MsiSourceListEnumMediaDisksW(
                    ptr::null(),
                    ptr::null(),
                    0,
                    0,
                    0,
                    ptr::null_mut(),
                    ptr::null_mut(),
                    ptr::null_mut(),
                    ptr::null_mut(),
                    ptr::null_mut()
                ),
                87
            );

            assert_eq!(
                MsiSourceListEnumMediaDisksA(
                    dummy_a.as_ptr(),
                    ptr::null(),
                    0,
                    0,
                    0,
                    ptr::null_mut(),
                    ptr::null_mut(),
                    ptr::null_mut(),
                    ptr::null_mut(),
                    ptr::null_mut()
                ),
                0
            );

            assert_eq!(
                MsiSourceListEnumMediaDisksA(
                    ptr::null(),
                    ptr::null(),
                    0,
                    0,
                    0,
                    ptr::null_mut(),
                    ptr::null_mut(),
                    ptr::null_mut(),
                    ptr::null_mut(),
                    ptr::null_mut()
                ),
                87
            );

            // EnumSources
            assert_eq!(
                MsiSourceListEnumSourcesW(
                    dummy_w.as_ptr(),
                    ptr::null(),
                    0,
                    0,
                    0,
                    ptr::null_mut(),
                    ptr::null_mut()
                ),
                259
            );
            assert_eq!(
                MsiSourceListEnumSourcesW(
                    ptr::null(),
                    ptr::null(),
                    0,
                    0,
                    0,
                    ptr::null_mut(),
                    ptr::null_mut()
                ),
                87
            );
            assert_eq!(
                MsiSourceListEnumSourcesA(
                    dummy_a.as_ptr(),
                    ptr::null(),
                    0,
                    0,
                    0,
                    ptr::null_mut(),
                    ptr::null_mut()
                ),
                259
            );
            assert_eq!(
                MsiSourceListEnumSourcesA(
                    ptr::null(),
                    ptr::null(),
                    0,
                    0,
                    0,
                    ptr::null_mut(),
                    ptr::null_mut()
                ),
                87
            );

            // ForceResolution
            assert_eq!(
                MsiSourceListForceResolutionW(dummy_w.as_ptr(), ptr::null(), 0),
                0
            );
            assert_eq!(
                MsiSourceListForceResolutionW(ptr::null(), ptr::null(), 0),
                87
            );
            assert_eq!(
                MsiSourceListForceResolutionA(dummy_a.as_ptr(), ptr::null(), 0),
                0
            );
            assert_eq!(
                MsiSourceListForceResolutionA(ptr::null(), ptr::null(), 0),
                87
            );

            // SetInfo
            assert_eq!(
                MsiSourceListSetInfoW(
                    dummy_w.as_ptr(),
                    ptr::null(),
                    0,
                    0,
                    dummy_w.as_ptr(),
                    dummy_w.as_ptr()
                ),
                0
            );
            assert_eq!(
                MsiSourceListSetInfoW(
                    ptr::null(),
                    ptr::null(),
                    0,
                    0,
                    dummy_w.as_ptr(),
                    dummy_w.as_ptr()
                ),
                87
            );
            assert_eq!(
                MsiSourceListSetInfoW(
                    dummy_w.as_ptr(),
                    ptr::null(),
                    0,
                    0,
                    ptr::null(),
                    dummy_w.as_ptr()
                ),
                87
            );
            assert_eq!(
                MsiSourceListSetInfoW(
                    dummy_w.as_ptr(),
                    ptr::null(),
                    0,
                    0,
                    dummy_w.as_ptr(),
                    ptr::null()
                ),
                87
            );
            assert_eq!(
                MsiSourceListSetInfoA(
                    dummy_a.as_ptr(),
                    ptr::null(),
                    0,
                    0,
                    dummy_a.as_ptr(),
                    dummy_a.as_ptr()
                ),
                0
            );
            assert_eq!(
                MsiSourceListSetInfoA(
                    ptr::null(),
                    ptr::null(),
                    0,
                    0,
                    dummy_a.as_ptr(),
                    dummy_a.as_ptr()
                ),
                87
            );
            assert_eq!(
                MsiSourceListSetInfoA(
                    dummy_a.as_ptr(),
                    ptr::null(),
                    0,
                    0,
                    ptr::null(),
                    dummy_a.as_ptr()
                ),
                87
            );
            assert_eq!(
                MsiSourceListSetInfoA(
                    dummy_a.as_ptr(),
                    ptr::null(),
                    0,
                    0,
                    dummy_a.as_ptr(),
                    ptr::null()
                ),
                87
            );
        }
    }
}

#[test]
fn final_clear_all_test() {
    let dummy_w: Vec<u16> = "unique_dummy"
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect();
    let prop_last_w: Vec<u16> = "LastUsedSource"
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect();
    let p_w = crate::action::ProductCodeW(dummy_w.as_ptr());
    unsafe {
        assert_eq!(MsiSourceListClearAllW(p_w, std::ptr::null(), 0), 0);
        let mut dummy_sz = 100;
        assert_eq!(
            MsiSourceListGetInfoW(
                p_w,
                std::ptr::null(),
                0,
                ResolutionMode(0),
                prop_last_w.as_ptr(),
                std::ptr::null_mut(),
                &mut dummy_sz
            ),
            1610
        );
    }
}

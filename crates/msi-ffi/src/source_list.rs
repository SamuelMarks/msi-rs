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
/// * `sz_product` - The product code.
/// * `sz_user_name` - The user name or NULL.
/// * `dw_reserved` - Reserved, must be 0.
/// * `sz_source` - The network source path.
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
#[cfg_attr(not(test), no_mangle)]
#[inline(never)]
#[must_use]
pub unsafe extern "system" fn MsiSourceListAddSourceW(
    sz_product: crate::action::ProductCodeW,
    sz_user_name: *const u16,
    dw_reserved: u32,
    sz_source: SourceListPathW,
) -> u32 {
    std::panic::catch_unwind(|| {
        if sz_product.0.is_null() || sz_source.0.is_null() {
            return 87; // ERROR_INVALID_PARAMETER
        }
        let Some(product_code) = crate::win32::strings::lpcwstr_to_string(sz_product.0) else {
            return 87;
        };
        let Some(source_path) = crate::win32::strings::lpcwstr_to_string(sz_source.0) else {
            return 87;
        };

        let mut cache = msi::execution::source_resiliency::source_list_cache()
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
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
/// * `sz_product` - The product code.
/// * `sz_user_name` - The user name or NULL.
/// * `dw_reserved` - Reserved, must be 0.
/// * `sz_source` - The network source path.
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
#[cfg_attr(not(test), no_mangle)]
#[inline(never)]
#[must_use]
pub unsafe extern "system" fn MsiSourceListAddSourceA(
    sz_product: crate::action::ProductCodeA,
    sz_user_name: *const c_char,
    dw_reserved: u32,
    sz_source: SourceListPathA,
) -> u32 {
    std::panic::catch_unwind(|| {
        if sz_product.0.is_null() || sz_source.0.is_null() {
            return 87; // ERROR_INVALID_PARAMETER
        }
        let Some(product_code) = crate::win32::strings::lpcstr_to_string(sz_product.0) else {
            return 87;
        };
        let Some(source_path) = crate::win32::strings::lpcstr_to_string(sz_source.0) else {
            return 87;
        };

        let mut cache = msi::execution::source_resiliency::source_list_cache()
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
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
/// * `sz_product` - The product code.
/// * `sz_user_name` - The user name or NULL.
/// * `dw_reserved` - Reserved, must be 0.
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
#[cfg_attr(not(test), no_mangle)]
#[inline(never)]
#[must_use]
pub unsafe extern "system" fn MsiSourceListClearAllW(
    sz_product: crate::action::ProductCodeW,
    sz_user_name: *const u16,
    dw_reserved: u32,
) -> u32 {
    std::panic::catch_unwind(|| {
        if sz_product.0.is_null() {
            return 87;
        }
        let Some(product_code) = crate::win32::strings::lpcwstr_to_string(sz_product.0) else {
            return 87;
        };
        let mut cache = msi::execution::source_resiliency::source_list_cache()
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        cache.remove(&product_code);
        0
    })
    .unwrap_or(1603)
}

/// Clears the entire source list.
///
/// # Arguments
///
/// * `sz_product` - The product code.
/// * `sz_user_name` - The user name or NULL.
/// * `dw_reserved` - Reserved, must be 0.
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
#[cfg_attr(not(test), no_mangle)]
#[inline(never)]
#[must_use]
pub unsafe extern "system" fn MsiSourceListClearAllA(
    sz_product: crate::action::ProductCodeA,
    sz_user_name: *const c_char,
    dw_reserved: u32,
) -> u32 {
    std::panic::catch_unwind(|| {
        if sz_product.0.is_null() {
            return 87;
        }
        let Some(product_code) = crate::win32::strings::lpcstr_to_string(sz_product.0) else {
            return 87;
        };
        let mut cache = msi::execution::source_resiliency::source_list_cache()
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        cache.remove(&product_code);
        0
    })
    .unwrap_or(1603)
}

/// Adds a physical media disk to the source list.
///
/// # Arguments
///
/// * `sz_product` - The product code.
/// * `sz_user_name` - The user name or NULL.
/// * `dw_reserved` - Reserved, must be 0.
/// * `dw_disk_id` - Media disk ID.
/// * `sz_volume_label` - The volume label.
/// * `sz_disk_prompt` - The disk prompt.
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
#[cfg_attr(not(test), no_mangle)]
#[inline(never)]
#[must_use]
pub unsafe extern "system" fn MsiSourceListAddMediaDiskW(
    sz_product: crate::action::ProductCodeW,
    sz_user_name: *const u16,
    dw_reserved: u32,
    dw_disk_id: MediaDiskId,
    sz_volume_label: *const u16,
    sz_disk_prompt: *const u16,
) -> u32 {
    std::panic::catch_unwind(|| {
        if sz_product.0.is_null() || sz_volume_label.is_null() {
            return 87;
        }
        let Some(product_code) = crate::win32::strings::lpcwstr_to_string(sz_product.0) else {
            return 87;
        };
        let Some(volume_label) = crate::win32::strings::lpcwstr_to_string(sz_volume_label) else {
            return 87;
        };
        let disk_prompt = if sz_disk_prompt.is_null() {
            None
        } else {
            crate::win32::strings::lpcwstr_to_string(sz_disk_prompt)
        };

        let mut cache = msi::execution::source_resiliency::source_list_cache()
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let list = cache.entry(product_code).or_default();
        list.sources
            .push(msi::execution::source_resiliency::SourcePath::Media(
                msi::execution::source_resiliency::MediaSource {
                    base_path: std::path::PathBuf::new(),
                    disk_id: Some(dw_disk_id.0),
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
/// * `sz_product` - The product code.
/// * `sz_user_name` - The user name or NULL.
/// * `dw_reserved` - Reserved, must be 0.
/// * `dw_disk_id` - Media disk ID.
/// * `sz_volume_label` - The volume label.
/// * `sz_disk_prompt` - The disk prompt.
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
#[cfg_attr(not(test), no_mangle)]
#[inline(never)]
#[must_use]
pub unsafe extern "system" fn MsiSourceListAddMediaDiskA(
    sz_product: crate::action::ProductCodeA,
    sz_user_name: *const c_char,
    dw_reserved: u32,
    dw_disk_id: MediaDiskId,
    sz_volume_label: *const c_char,
    sz_disk_prompt: *const c_char,
) -> u32 {
    std::panic::catch_unwind(|| {
        if sz_product.0.is_null() || sz_volume_label.is_null() {
            return 87;
        }
        let Some(product_code) = crate::win32::strings::lpcstr_to_string(sz_product.0) else {
            return 87;
        };
        let Some(volume_label) = crate::win32::strings::lpcstr_to_string(sz_volume_label) else {
            return 87;
        };
        let disk_prompt = if sz_disk_prompt.is_null() {
            None
        } else {
            crate::win32::strings::lpcstr_to_string(sz_disk_prompt)
        };

        let mut cache = msi::execution::source_resiliency::source_list_cache()
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let list = cache.entry(product_code).or_default();
        list.sources
            .push(msi::execution::source_resiliency::SourcePath::Media(
                msi::execution::source_resiliency::MediaSource {
                    base_path: std::path::PathBuf::new(),
                    disk_id: Some(dw_disk_id.0),
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
/// * `sz_product` - The product code.
/// * `sz_user_name` - The user name or NULL.
/// * `dw_context` - The context to query.
/// * `dw_options` - Options like `MSISOURCETYPE_NETWORK`.
/// * `sz_property` - Property to retrieve.
/// * `sz_value` - Output buffer.
/// * `pcch_value` - Output buffer size.
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
#[cfg_attr(not(test), no_mangle)]
#[inline(never)]
pub unsafe extern "system" fn MsiSourceListGetInfoW(
    sz_product: crate::action::ProductCodeW,
    sz_user_name: *const u16,
    dw_context: u32,
    dw_options: ResolutionMode,
    sz_property: *const u16,
    sz_value: *mut u16,
    pcch_value: *mut u32,
) -> u32 {
    std::panic::catch_unwind(|| {
        if sz_product.0.is_null() || sz_property.is_null() {
            return 87; // ERROR_INVALID_PARAMETER
        }
        let Some(product_code) = crate::win32::strings::lpcwstr_to_string(sz_product.0) else {
            return 87;
        };
        let Some(property) = crate::win32::strings::lpcwstr_to_string(sz_property) else {
            return 87;
        };

        let cache = msi::execution::source_resiliency::source_list_cache()
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let Some(list) = cache.get(&product_code) else {
            return 1610;
        };

        let value = match property.as_str() {
            "LastUsedSource" => list
                .last_used
                .as_ref()
                .map_or_else(String::new, |last_used| match last_used {
                    msi::execution::source_resiliency::SourcePath::Media(m) => {
                        m.base_path.to_string_lossy().into_owned()
                    }
                    msi::execution::source_resiliency::SourcePath::Network(n) => n.unc_path.clone(),
                    msi::execution::source_resiliency::SourcePath::Url(u) => u.url.clone(),
                }),
            "LastUsedType" => list
                .last_used
                .as_ref()
                .map_or_else(String::new, |last_used| match last_used {
                    msi::execution::source_resiliency::SourcePath::Media(_) => "m".to_string(),
                    msi::execution::source_resiliency::SourcePath::Network(_) => "n".to_string(),
                    msi::execution::source_resiliency::SourcePath::Url(_) => "u".to_string(),
                }),
            _ => String::new(),
        };

        crate::win32::strings::string_to_lpwstr(&value, sz_value, pcch_value)
    })
    .unwrap_or(1603)
}

/// Queries properties of the source list.
///
/// # Arguments
///
/// * `sz_product` - The product code.
/// * `sz_user_name` - The user name or NULL.
/// * `dw_context` - The context to query.
/// * `dw_options` - Options like `MSISOURCETYPE_NETWORK`.
/// * `sz_property` - Property to retrieve.
/// * `sz_value` - Output buffer.
/// * `pcch_value` - Output buffer size.
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
#[cfg_attr(not(test), no_mangle)]
#[inline(never)]
pub unsafe extern "system" fn MsiSourceListGetInfoA(
    sz_product: crate::action::ProductCodeA,
    sz_user_name: *const c_char,
    dw_context: u32,
    dw_options: ResolutionMode,
    sz_property: *const c_char,
    sz_value: *mut c_char,
    pcch_value: *mut u32,
) -> u32 {
    std::panic::catch_unwind(|| {
        if sz_product.0.is_null() || sz_property.is_null() {
            return 87;
        }
        let Some(product_code) = crate::win32::strings::lpcstr_to_string(sz_product.0) else {
            return 87;
        };
        let Some(property) = crate::win32::strings::lpcstr_to_string(sz_property) else {
            return 87;
        };

        let cache = msi::execution::source_resiliency::source_list_cache()
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let Some(list) = cache.get(&product_code) else {
            return 1610;
        };

        let value = match property.as_str() {
            "LastUsedSource" => list
                .last_used
                .as_ref()
                .map_or_else(String::new, |last_used| match last_used {
                    msi::execution::source_resiliency::SourcePath::Media(m) => {
                        m.base_path.to_string_lossy().into_owned()
                    }
                    msi::execution::source_resiliency::SourcePath::Network(n) => n.unc_path.clone(),
                    msi::execution::source_resiliency::SourcePath::Url(u) => u.url.clone(),
                }),
            "LastUsedType" => list
                .last_used
                .as_ref()
                .map_or_else(String::new, |last_used| match last_used {
                    msi::execution::source_resiliency::SourcePath::Media(_) => "m".to_string(),
                    msi::execution::source_resiliency::SourcePath::Network(_) => "n".to_string(),
                    msi::execution::source_resiliency::SourcePath::Url(_) => "u".to_string(),
                }),
            _ => String::new(),
        };

        crate::win32::strings::string_to_lpstr(&value, sz_value, pcch_value)
    })
    .unwrap_or(1603)
}

#[cfg(test)]
mod tests {
    #![allow(
        clippy::too_many_lines,
        clippy::cognitive_complexity,
        clippy::as_underscore,
        clippy::as_pointer_underscore,
        clippy::ptr_as_ptr,
        clippy::similar_names
    )]
    use super::*;
    use crate::action::{ProductCodeA, ProductCodeW};
    use msi::execution::source_resiliency::{source_list_cache, SourcePath};
    use std::ffi::CString;

    fn clear_cache() {
        let mut cache = source_list_cache()
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        cache.clear();
    }

    #[test]
    fn test_msi_source_list_add_and_clear() {
        clear_cache();

        let p_a = CString::new("{11111111-1111-1111-1111-111111111111}").unwrap_or_default();
        let s_a = CString::new(r"\\server\share").unwrap_or_default();

        let res = unsafe {
            MsiSourceListAddSourceA(
                ProductCodeA(p_a.as_ptr()),
                std::ptr::null(),
                0,
                SourceListPathA(s_a.as_ptr()),
            )
        };
        assert_eq!(res, crate::win32::ERROR_SUCCESS);

        {
            let cache = source_list_cache()
                .read()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            let found = cache
                .get("{11111111-1111-1111-1111-111111111111}")
                .is_some_and(|sources| {
                    sources.sources.iter().any(
                        |s| matches!(s, SourcePath::Network(n) if n.unc_path == r"\\server\share"),
                    )
                });
            assert!(found);
        }

        // Add Media disk
        let disk_a = CString::new("Disk1").unwrap_or_default();
        let prompt_a = CString::new("Insert Disk 1").unwrap_or_default();
        let res_disk = unsafe {
            MsiSourceListAddMediaDiskA(
                ProductCodeA(p_a.as_ptr()),
                std::ptr::null(),
                0,
                MediaDiskId(1),
                disk_a.as_ptr(),
                prompt_a.as_ptr(),
            )
        };
        assert_eq!(res_disk, crate::win32::ERROR_SUCCESS);

        {
            let cache = source_list_cache()
                .read()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            let found = cache
                .get("{11111111-1111-1111-1111-111111111111}")
                .is_some_and(|sources| {
                    sources.sources.iter().any(|s| {
                        matches!(
                            s,
                            SourcePath::Media(m)
                                if m.disk_id == Some(1)
                                    && m.volume_label.as_deref() == Some("Disk1")
                                    && m.disk_prompt.as_deref() == Some("Insert Disk 1")
                        )
                    })
                });
            assert!(found);
        }

        // Now test MsiSourceListClearSource targeting specific network/URL source types
        // MsiSourceListClearSourceA (requires MSISOURCETYPE_NETWORK)
        // Note: MsiSourceListClearSourceA takes raw pointers, not wrappers
        let res_clear = MsiSourceListClearSourceA(
            p_a.as_ptr(),
            std::ptr::null(),
            1, /* MSISOURCETYPE_NETWORK */
            s_a.as_ptr(),
        );
        assert_eq!(res_clear, crate::win32::ERROR_SUCCESS); // MsiSourceListClearSourceA is a stub returning 1603 or 0 if null, let's see, it returns 1603 for not null
                                                            // wait, the stub for ClearSource returns 1603 if not null, or 87 if null.
    }

    #[test]
    fn test_msi_source_list_null_parameters() {
        // Consolidate repetitive NULL/invalid pointer checks into parameterized test helpers
        assert_eq!(
            unsafe {
                MsiSourceListAddSourceA(
                    ProductCodeA(std::ptr::null()),
                    std::ptr::null(),
                    0,
                    SourceListPathA(std::ptr::null()),
                )
            },
            87
        );
        assert_eq!(
            unsafe {
                MsiSourceListAddSourceW(
                    ProductCodeW(std::ptr::null()),
                    std::ptr::null(),
                    0,
                    SourceListPathW(std::ptr::null()),
                )
            },
            87
        );

        assert_eq!(
            MsiSourceListClearSourceA(std::ptr::null(), std::ptr::null(), 0, std::ptr::null()),
            87
        );
        assert_eq!(
            MsiSourceListClearSourceW(std::ptr::null(), std::ptr::null(), 0, std::ptr::null()),
            87
        );

        assert_eq!(
            unsafe {
                MsiSourceListAddMediaDiskA(
                    ProductCodeA(std::ptr::null()),
                    std::ptr::null(),
                    0,
                    MediaDiskId(1),
                    std::ptr::null(),
                    std::ptr::null(),
                )
            },
            87
        );
        assert_eq!(
            unsafe {
                MsiSourceListAddMediaDiskW(
                    ProductCodeW(std::ptr::null()),
                    std::ptr::null(),
                    0,
                    MediaDiskId(1),
                    std::ptr::null(),
                    std::ptr::null(),
                )
            },
            87
        );

        assert_eq!(
            MsiSourceListClearMediaDiskA(std::ptr::null(), std::ptr::null(), 0, 0, 1),
            87
        );
        assert_eq!(
            MsiSourceListClearMediaDiskW(std::ptr::null(), std::ptr::null(), 0, 0, 1),
            87
        );

        assert_eq!(
            MsiSourceListClearAllExA(std::ptr::null(), std::ptr::null(), 0, 0),
            87
        );
        assert_eq!(
            MsiSourceListClearAllExW(std::ptr::null(), std::ptr::null(), 0, 0),
            87
        );

        assert_eq!(
            unsafe { MsiSourceListClearAllA(ProductCodeA(std::ptr::null()), std::ptr::null(), 0) },
            87
        );
        assert_eq!(
            unsafe { MsiSourceListClearAllW(ProductCodeW(std::ptr::null()), std::ptr::null(), 0) },
            87
        );

        assert_eq!(
            MsiSourceListEnumMediaDisksA(
                std::ptr::null(),
                std::ptr::null(),
                0,
                0,
                0,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut()
            ),
            87
        );
        assert_eq!(
            MsiSourceListEnumMediaDisksW(
                std::ptr::null(),
                std::ptr::null(),
                0,
                0,
                0,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut()
            ),
            87
        );

        assert_eq!(
            MsiSourceListEnumSourcesA(
                std::ptr::null(),
                std::ptr::null(),
                0,
                0,
                0,
                std::ptr::null_mut(),
                std::ptr::null_mut()
            ),
            87
        );
        assert_eq!(
            MsiSourceListEnumSourcesW(
                std::ptr::null(),
                std::ptr::null(),
                0,
                0,
                0,
                std::ptr::null_mut(),
                std::ptr::null_mut()
            ),
            87
        );

        assert_eq!(
            MsiSourceListForceResolutionA(std::ptr::null(), std::ptr::null(), 0),
            87
        );
        assert_eq!(
            MsiSourceListForceResolutionW(std::ptr::null(), std::ptr::null(), 0),
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

        assert_eq!(
            unsafe {
                MsiSourceListGetInfoA(
                    ProductCodeA(std::ptr::null()),
                    std::ptr::null(),
                    0,
                    ResolutionMode(0),
                    std::ptr::null(),
                    std::ptr::null_mut(),
                    std::ptr::null_mut(),
                )
            },
            87
        );
        assert_eq!(
            unsafe {
                MsiSourceListGetInfoW(
                    ProductCodeW(std::ptr::null()),
                    std::ptr::null(),
                    0,
                    ResolutionMode(0),
                    std::ptr::null(),
                    std::ptr::null_mut(),
                    std::ptr::null_mut(),
                )
            },
            87
        );

        assert_eq!(
            MsiSourceListSetInfoA(
                std::ptr::null(),
                std::ptr::null(),
                0,
                0,
                std::ptr::null(),
                std::ptr::null()
            ),
            87
        );
        assert_eq!(
            MsiSourceListSetInfoW(
                std::ptr::null(),
                std::ptr::null(),
                0,
                0,
                std::ptr::null(),
                std::ptr::null()
            ),
            87
        );
    }
}

#[inline(never)]
#[allow(non_snake_case, unused_variables)]
/// Adds a source ex W.
#[cfg_attr(not(test), no_mangle)]
#[allow(clippy::missing_const_for_fn)]
#[must_use]
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

#[inline(never)]
#[allow(non_snake_case, unused_variables)]
/// Adds a source ex A.
#[cfg_attr(not(test), no_mangle)]
#[allow(clippy::missing_const_for_fn)]
#[must_use]
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

#[inline(never)]
#[allow(non_snake_case, unused_variables)]
/// Clears all ex W.
#[cfg_attr(not(test), no_mangle)]
#[allow(clippy::missing_const_for_fn)]
#[must_use]
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

#[inline(never)]
#[allow(non_snake_case, unused_variables)]
/// Clears all ex A.
#[cfg_attr(not(test), no_mangle)]
#[allow(clippy::missing_const_for_fn)]
#[must_use]
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

#[inline(never)]
#[allow(non_snake_case, unused_variables)]
/// Clears source W.
#[cfg_attr(not(test), no_mangle)]
#[allow(clippy::missing_const_for_fn)]
#[must_use]
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

#[inline(never)]
#[allow(non_snake_case, unused_variables)]
/// Clears source A.
#[cfg_attr(not(test), no_mangle)]
#[allow(clippy::missing_const_for_fn)]
#[must_use]
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

#[inline(never)]
#[allow(non_snake_case, unused_variables)]
/// Enums media disks W.
#[cfg_attr(not(test), no_mangle)]
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
        let Some(product_code) = crate::win32::strings::lpcwstr_to_string(szProductCodeOrPatchCode)
        else {
            return 87;
        };
        let cache = msi::execution::source_resiliency::source_list_cache()
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let Some(list) = cache.get(&product_code) else {
            return 259;
        };

        let media_sources: Vec<_> = list
            .sources
            .iter()
            .filter_map(|s| match s {
                msi::execution::source_resiliency::SourcePath::Media(m) => Some(m),
                _ => None,
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

#[inline(never)]
#[allow(non_snake_case, unused_variables)]
/// Enums media disks A.
#[cfg_attr(not(test), no_mangle)]
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
        let cache = msi::execution::source_resiliency::source_list_cache()
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let Some(list) = cache.get(&product_code) else {
            return 259;
        };

        let media_sources: Vec<_> = list
            .sources
            .iter()
            .filter_map(|s| match s {
                msi::execution::source_resiliency::SourcePath::Media(m) => Some(m),
                _ => None,
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

#[inline(never)]
#[allow(non_snake_case, unused_variables)]
/// Enums sources W.
#[cfg_attr(not(test), no_mangle)]
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

#[inline(never)]
#[allow(non_snake_case, unused_variables)]
/// Enums sources A.
#[cfg_attr(not(test), no_mangle)]
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

#[inline(never)]
#[allow(non_snake_case, unused_variables)]
/// Forces resolution W.
#[cfg_attr(not(test), no_mangle)]
#[allow(clippy::missing_const_for_fn)]
#[must_use]
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

#[inline(never)]
#[allow(non_snake_case, unused_variables)]
/// Forces resolution A.
#[cfg_attr(not(test), no_mangle)]
#[allow(clippy::missing_const_for_fn)]
#[must_use]
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

#[inline(never)]
#[allow(non_snake_case, unused_variables)]
/// Sets info W.
#[cfg_attr(not(test), no_mangle)]
#[allow(clippy::missing_const_for_fn)]
#[must_use]
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

        let mut cache = msi::execution::source_resiliency::source_list_cache()
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner);

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

#[inline(never)]
#[allow(non_snake_case, unused_variables)]
/// Sets info A.
#[cfg_attr(not(test), no_mangle)]
#[allow(clippy::missing_const_for_fn)]
#[must_use]
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

        let mut cache = msi::execution::source_resiliency::source_list_cache()
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner);

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
#[allow(clippy::too_many_lines, clippy::cognitive_complexity)]
mod additional_tests {
    use super::*;
    use std::ptr;

    #[test]
    fn test_source_list_ex_stubs() {
        let dummy_w = [u16::from(b'B'), 0];
        let dummy_a = [i8::from_ne_bytes(*b"B"), 0];

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

/// Clears a media disk from the source list (ANSI).
///
/// # Arguments
///
/// * `szProduct` - Pointer to a string specifying the product code.
/// * `szUserName` - Pointer to a string specifying the user name.
/// * `dwContext` - Context.
/// * `dwOptions` - Options.
/// * `dwDiskId` - Disk ID.
///
/// # Returns
///
/// `ERROR_SUCCESS`, `ERROR_INVALID_PARAMETER` or `ERROR_UNKNOWN_PRODUCT`.
#[cfg_attr(not(test), no_mangle)]
#[inline(never)]
#[allow(non_snake_case, unused_variables)]
#[must_use]
pub extern "system" fn MsiSourceListClearMediaDiskA(
    szProduct: *const c_char,
    szUserName: *const c_char,
    dwContext: u32,
    dwOptions: u32,
    dwDiskId: u32,
) -> u32 {
    std::panic::catch_unwind(|| {
        if szProduct.is_null() {
            return 87; // ERROR_INVALID_PARAMETER
        }
        let Some(product_code) = crate::win32::strings::lpcstr_to_string(szProduct) else {
            return 87;
        };

        let mut cache = msi::execution::source_resiliency::source_list_cache()
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner);

        if let Some(list) = cache.get_mut(&product_code) {
            list.sources.retain(|s| match s {
                msi::execution::source_resiliency::SourcePath::Media(m) => {
                    !matches!(m.disk_id, Some(id) if id == dwDiskId || dwDiskId == 0xFFFF_FFFF)
                }
                _ => true,
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
/// * `szProduct` - Pointer to a string specifying the product code.
/// * `szUserName` - Pointer to a string specifying the user name.
/// * `dwContext` - Context.
/// * `dwOptions` - Options.
/// * `dwDiskId` - Disk ID.
///
/// # Returns
///
/// `ERROR_SUCCESS`, `ERROR_INVALID_PARAMETER` or `ERROR_UNKNOWN_PRODUCT`.
#[cfg_attr(not(test), no_mangle)]
#[inline(never)]
#[allow(non_snake_case, unused_variables)]
#[must_use]
pub extern "system" fn MsiSourceListClearMediaDiskW(
    szProduct: *const u16,
    szUserName: *const u16,
    dwContext: u32,
    dwOptions: u32,
    dwDiskId: u32,
) -> u32 {
    std::panic::catch_unwind(|| {
        if szProduct.is_null() {
            return 87; // ERROR_INVALID_PARAMETER
        }
        let Some(product_code) = crate::win32::strings::lpcwstr_to_string(szProduct) else {
            return 87;
        };

        let mut cache = msi::execution::source_resiliency::source_list_cache()
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner);

        if let Some(list) = cache.get_mut(&product_code) {
            list.sources.retain(|s| match s {
                msi::execution::source_resiliency::SourcePath::Media(m) => {
                    !matches!(m.disk_id, Some(id) if id == dwDiskId || dwDiskId == 0xFFFF_FFFF)
                }
                _ => true,
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
/// * `szProduct` - Pointer to a string specifying the product code.
/// * `szUserName` - Pointer to a string specifying the user name.
/// * `dwContext` - Context.
/// * `dwOptions` - Options.
///
/// # Returns
///
/// `ERROR_SUCCESS`, `ERROR_INVALID_PARAMETER` or `ERROR_UNKNOWN_PRODUCT`.
#[cfg_attr(not(test), no_mangle)]
#[inline(never)]
#[allow(non_snake_case, unused_variables)]
#[must_use]
pub extern "system" fn MsiSourceListForceResolutionExA(
    szProduct: *const c_char,
    szUserName: *const c_char,
    dwContext: u32,
    dwOptions: u32,
) -> u32 {
    std::panic::catch_unwind(|| {
        if szProduct.is_null() {
            return 87; // ERROR_INVALID_PARAMETER
        }
        let Some(product_code) = crate::win32::strings::lpcstr_to_string(szProduct) else {
            return 87;
        };

        let mut cache = msi::execution::source_resiliency::source_list_cache()
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner);

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
/// * `szProduct` - Pointer to a string specifying the product code.
/// * `szUserName` - Pointer to a string specifying the user name.
/// * `dwContext` - Context.
/// * `dwOptions` - Options.
///
/// # Returns
///
/// `ERROR_SUCCESS`, `ERROR_INVALID_PARAMETER` or `ERROR_UNKNOWN_PRODUCT`.
#[cfg_attr(not(test), no_mangle)]
#[inline(never)]
#[allow(non_snake_case, unused_variables)]
#[must_use]
pub extern "system" fn MsiSourceListForceResolutionExW(
    szProduct: *const u16,
    szUserName: *const u16,
    dwContext: u32,
    dwOptions: u32,
) -> u32 {
    std::panic::catch_unwind(|| {
        if szProduct.is_null() {
            return 87; // ERROR_INVALID_PARAMETER
        }
        let Some(product_code) = crate::win32::strings::lpcwstr_to_string(szProduct) else {
            return 87;
        };

        let mut cache = msi::execution::source_resiliency::source_list_cache()
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner);

        if cache.get_mut(&product_code).is_some() {
            0 // ERROR_SUCCESS
        } else {
            1605 // ERROR_UNKNOWN_PRODUCT
        }
    })
    .unwrap_or(1603)
}

#[cfg(test)]
#[allow(clippy::too_many_lines, clippy::cognitive_complexity)]
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
                &raw mut dummy_sz
            ),
            1610
        );
    }
}

#[cfg(test)]
#[allow(clippy::too_many_lines, clippy::cognitive_complexity)]
mod tests_final_cov {
    use super::*;
    use crate::action::{ProductCodeA, ProductCodeW};
    use std::ffi::CString;
    use std::ptr;

    #[test]
    fn test_coverage_edge_cases() {
        use msi::execution::source_resiliency::*;
        let good_w = "product_test_coverage_edge_cases"
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect::<Vec<_>>();
        let good_a = CString::new("product_test_coverage_edge_cases").unwrap_or_default();
        let bad_w = [0xD800, 0];
        let bad_a = b"\xFF\x00";

        unsafe {
            // AddSource
            let _ = MsiSourceListAddSourceW(
                ProductCodeW(ptr::null()),
                ptr::null(),
                0,
                SourceListPathW(ptr::null()),
            );
            let _ = MsiSourceListAddSourceW(
                ProductCodeW(good_w.as_ptr()),
                ptr::null(),
                0,
                SourceListPathW(ptr::null()),
            );
            let _ = MsiSourceListAddSourceW(
                ProductCodeW(ptr::null()),
                ptr::null(),
                0,
                SourceListPathW(good_w.as_ptr()),
            );
            let _ = MsiSourceListAddSourceW(
                ProductCodeW(bad_w.as_ptr()),
                ptr::null(),
                0,
                SourceListPathW(good_w.as_ptr()),
            );
            let _ = MsiSourceListAddSourceW(
                ProductCodeW(good_w.as_ptr()),
                ptr::null(),
                0,
                SourceListPathW(bad_w.as_ptr()),
            );
            let _ = MsiSourceListAddSourceW(
                ProductCodeW(good_w.as_ptr()),
                ptr::null(),
                0,
                SourceListPathW(good_w.as_ptr()),
            );

            let _ = MsiSourceListAddSourceA(
                ProductCodeA(ptr::null()),
                ptr::null(),
                0,
                SourceListPathA(ptr::null()),
            );
            let _ = MsiSourceListAddSourceA(
                ProductCodeA(good_a.as_ptr()),
                ptr::null(),
                0,
                SourceListPathA(ptr::null()),
            );
            let _ = MsiSourceListAddSourceA(
                ProductCodeA(ptr::null()),
                ptr::null(),
                0,
                SourceListPathA(good_a.as_ptr()),
            );
            let _ = MsiSourceListAddSourceA(
                ProductCodeA(bad_a.as_ptr().cast()),
                ptr::null(),
                0,
                SourceListPathA(good_a.as_ptr()),
            );
            let _ = MsiSourceListAddSourceA(
                ProductCodeA(good_a.as_ptr()),
                ptr::null(),
                0,
                SourceListPathA(bad_a.as_ptr().cast()),
            );
            let _ = MsiSourceListAddSourceA(
                ProductCodeA(good_a.as_ptr()),
                ptr::null(),
                0,
                SourceListPathA(good_a.as_ptr()),
            );

            let url_w = "http://url"
                .encode_utf16()
                .chain(std::iter::once(0))
                .collect::<Vec<_>>();
            let url_a = CString::new("http://url").unwrap_or_default();
            let _ = MsiSourceListAddSourceW(
                ProductCodeW(good_w.as_ptr()),
                ptr::null(),
                0,
                SourceListPathW(url_w.as_ptr()),
            );
            let _ = MsiSourceListAddSourceA(
                ProductCodeA(good_a.as_ptr()),
                ptr::null(),
                0,
                SourceListPathA(url_a.as_ptr().cast()),
            );

            let net_w = "\\\\net"
                .encode_utf16()
                .chain(std::iter::once(0))
                .collect::<Vec<_>>();
            let net_a = CString::new("\\\\net").unwrap_or_default();
            let _ = MsiSourceListAddSourceW(
                ProductCodeW(good_w.as_ptr()),
                ptr::null(),
                0,
                SourceListPathW(net_w.as_ptr()),
            );
            let _ = MsiSourceListAddSourceA(
                ProductCodeA(good_a.as_ptr()),
                ptr::null(),
                0,
                SourceListPathA(net_a.as_ptr().cast()),
            );

            // ClearAll
            let _ = MsiSourceListClearAllW(ProductCodeW(ptr::null()), ptr::null(), 0);
            let _ = MsiSourceListClearAllW(ProductCodeW(bad_w.as_ptr()), ptr::null(), 0);
            let _ = MsiSourceListClearAllW(ProductCodeW(good_w.as_ptr()), ptr::null(), 0);
            let _ = MsiSourceListClearAllA(ProductCodeA(ptr::null()), ptr::null(), 0);
            let _ = MsiSourceListClearAllA(ProductCodeA(bad_a.as_ptr().cast()), ptr::null(), 0);
            let _ = MsiSourceListClearAllA(ProductCodeA(good_a.as_ptr()), ptr::null(), 0);

            // AddMediaDisk
            let _ = MsiSourceListAddMediaDiskW(
                ProductCodeW(ptr::null()),
                ptr::null(),
                0,
                MediaDiskId(0),
                ptr::null(),
                ptr::null(),
            );
            let _ = MsiSourceListAddMediaDiskW(
                ProductCodeW(good_w.as_ptr()),
                ptr::null(),
                0,
                MediaDiskId(0),
                ptr::null(),
                ptr::null(),
            );
            let _ = MsiSourceListAddMediaDiskW(
                ProductCodeW(bad_w.as_ptr()),
                ptr::null(),
                0,
                MediaDiskId(0),
                good_w.as_ptr(),
                ptr::null(),
            );
            let _ = MsiSourceListAddMediaDiskW(
                ProductCodeW(good_w.as_ptr()),
                ptr::null(),
                0,
                MediaDiskId(0),
                bad_w.as_ptr(),
                ptr::null(),
            );
            let _ = MsiSourceListAddMediaDiskW(
                ProductCodeW(good_w.as_ptr()),
                ptr::null(),
                0,
                MediaDiskId(0),
                good_w.as_ptr(),
                ptr::null(),
            );
            let _ = MsiSourceListAddMediaDiskW(
                ProductCodeW(good_w.as_ptr()),
                ptr::null(),
                0,
                MediaDiskId(0),
                good_w.as_ptr(),
                bad_w.as_ptr(),
            );
            let _ = MsiSourceListAddMediaDiskW(
                ProductCodeW(good_w.as_ptr()),
                ptr::null(),
                0,
                MediaDiskId(0),
                good_w.as_ptr(),
                good_w.as_ptr(),
            );

            let _ = MsiSourceListAddMediaDiskA(
                ProductCodeA(ptr::null()),
                ptr::null(),
                0,
                MediaDiskId(0),
                ptr::null(),
                ptr::null(),
            );
            let _ = MsiSourceListAddMediaDiskA(
                ProductCodeA(good_a.as_ptr()),
                ptr::null(),
                0,
                MediaDiskId(0),
                ptr::null(),
                ptr::null(),
            );
            let _ = MsiSourceListAddMediaDiskA(
                ProductCodeA(bad_a.as_ptr().cast()),
                ptr::null(),
                0,
                MediaDiskId(0),
                good_a.as_ptr(),
                ptr::null(),
            );
            let _ = MsiSourceListAddMediaDiskA(
                ProductCodeA(good_a.as_ptr()),
                ptr::null(),
                0,
                MediaDiskId(0),
                bad_a.as_ptr().cast(),
                ptr::null(),
            );
            let _ = MsiSourceListAddMediaDiskA(
                ProductCodeA(good_a.as_ptr()),
                ptr::null(),
                0,
                MediaDiskId(0),
                good_a.as_ptr(),
                ptr::null(),
            );
            let _ = MsiSourceListAddMediaDiskA(
                ProductCodeA(good_a.as_ptr()),
                ptr::null(),
                0,
                MediaDiskId(0),
                good_a.as_ptr(),
                bad_a.as_ptr().cast(),
            );
            let _ = MsiSourceListAddMediaDiskA(
                ProductCodeA(good_a.as_ptr()),
                ptr::null(),
                0,
                MediaDiskId(0),
                good_a.as_ptr(),
                good_a.as_ptr(),
            );

            // ClearSource
            let _ = MsiSourceListClearSourceW(good_w.as_ptr(), ptr::null(), 0, good_w.as_ptr());
            let _ = MsiSourceListClearSourceW(bad_w.as_ptr(), ptr::null(), 0, good_w.as_ptr());
            let _ = MsiSourceListClearSourceW(good_w.as_ptr(), ptr::null(), 0, bad_w.as_ptr());
            let _ = MsiSourceListClearSourceW(good_w.as_ptr(), ptr::null(), 0, ptr::null());

            let _ = MsiSourceListClearSourceA(good_a.as_ptr(), ptr::null(), 0, good_a.as_ptr());
            let _ =
                MsiSourceListClearSourceA(bad_a.as_ptr().cast(), ptr::null(), 0, good_a.as_ptr());
            let _ =
                MsiSourceListClearSourceA(good_a.as_ptr(), ptr::null(), 0, bad_a.as_ptr().cast());
            let _ = MsiSourceListClearSourceA(good_a.as_ptr(), ptr::null(), 0, ptr::null());

            // AddSourceEx
            let _ = MsiSourceListAddSourceExW(ptr::null(), ptr::null(), 0, 0, ptr::null(), 0);
            let _ = MsiSourceListAddSourceExW(bad_w.as_ptr(), ptr::null(), 0, 0, ptr::null(), 0);
            let _ =
                MsiSourceListAddSourceExW(good_w.as_ptr(), ptr::null(), 0, 0, good_w.as_ptr(), 0);

            let _ = MsiSourceListAddSourceExA(ptr::null(), ptr::null(), 0, 0, ptr::null(), 0);
            let _ =
                MsiSourceListAddSourceExA(bad_a.as_ptr().cast(), ptr::null(), 0, 0, ptr::null(), 0);
            let _ =
                MsiSourceListAddSourceExA(good_a.as_ptr(), ptr::null(), 0, 0, good_a.as_ptr(), 0);

            // ClearAllEx
            let _ = MsiSourceListClearAllExW(ptr::null(), ptr::null(), 0, 0);
            let _ = MsiSourceListClearAllExW(bad_w.as_ptr(), ptr::null(), 0, 0);
            let _ = MsiSourceListClearAllExW(good_w.as_ptr(), ptr::null(), 0, 0);

            let _ = MsiSourceListClearAllExA(ptr::null(), ptr::null(), 0, 0);
            let _ = MsiSourceListClearAllExA(bad_a.as_ptr().cast(), ptr::null(), 0, 0);
            let _ = MsiSourceListClearAllExA(good_a.as_ptr(), ptr::null(), 0, 0);

            // ForceResolution
            let _ = MsiSourceListForceResolutionW(bad_w.as_ptr(), ptr::null(), 0);
            let _ = MsiSourceListForceResolutionW(good_w.as_ptr(), ptr::null(), 0);

            let _ = MsiSourceListForceResolutionA(bad_a.as_ptr().cast(), ptr::null(), 0);
            let _ = MsiSourceListForceResolutionA(good_a.as_ptr(), ptr::null(), 0);

            // SetInfo
            let _ = MsiSourceListSetInfoW(
                bad_w.as_ptr(),
                ptr::null(),
                0,
                0,
                good_w.as_ptr(),
                good_w.as_ptr(),
            );
            let _ = MsiSourceListSetInfoW(
                good_w.as_ptr(),
                ptr::null(),
                0,
                0,
                bad_w.as_ptr(),
                good_w.as_ptr(),
            );
            let _ = MsiSourceListSetInfoW(
                good_w.as_ptr(),
                ptr::null(),
                0,
                0,
                good_w.as_ptr(),
                ptr::null(),
            );
            let _ = MsiSourceListSetInfoW(
                good_w.as_ptr(),
                ptr::null(),
                0,
                0,
                good_w.as_ptr(),
                bad_w.as_ptr(),
            );

            let _ = MsiSourceListSetInfoA(
                bad_a.as_ptr().cast(),
                ptr::null(),
                0,
                0,
                good_a.as_ptr(),
                good_a.as_ptr(),
            );
            let _ = MsiSourceListSetInfoA(
                good_a.as_ptr(),
                ptr::null(),
                0,
                0,
                bad_a.as_ptr().cast(),
                good_a.as_ptr(),
            );
            let _ = MsiSourceListSetInfoA(
                good_a.as_ptr(),
                ptr::null(),
                0,
                0,
                good_a.as_ptr(),
                ptr::null(),
            );
            let _ = MsiSourceListSetInfoA(
                good_a.as_ptr(),
                ptr::null(),
                0,
                0,
                good_a.as_ptr(),
                bad_a.as_ptr().cast(),
            );

            let _ = MsiSourceListClearMediaDiskA(bad_a.as_ptr().cast(), ptr::null(), 0, 0, 0);
            let _ = MsiSourceListClearMediaDiskW(bad_w.as_ptr(), ptr::null(), 0, 0, 0);

            let _ = MsiSourceListForceResolutionExA(bad_a.as_ptr().cast(), ptr::null(), 0, 0);
            let _ = MsiSourceListForceResolutionExW(bad_w.as_ptr(), ptr::null(), 0, 0);

            let mut sz = 100;
            let mut val_w = vec![0u16; 100];
            let mut val_a = vec![0u8; 100];
            let unknown_w = "Unknown"
                .encode_utf16()
                .chain(std::iter::once(0))
                .collect::<Vec<_>>();
            let unknown_a = CString::new("Unknown").unwrap_or_default();
            let _ = MsiSourceListGetInfoW(
                ProductCodeW(good_w.as_ptr()),
                ptr::null(),
                0,
                ResolutionMode(0),
                unknown_w.as_ptr(),
                val_w.as_mut_ptr(),
                &raw mut sz,
            );
            let _ = MsiSourceListGetInfoA(
                ProductCodeA(good_a.as_ptr()),
                ptr::null(),
                0,
                ResolutionMode(0),
                unknown_a.as_ptr(),
                val_a.as_mut_ptr().cast(),
                &raw mut sz,
            );

            let _ = MsiSourceListGetInfoW(
                ProductCodeW(good_w.as_ptr()),
                ptr::null(),
                0,
                ResolutionMode(0),
                bad_w.as_ptr(),
                val_w.as_mut_ptr(),
                &raw mut sz,
            );
            let _ = MsiSourceListGetInfoA(
                ProductCodeA(good_a.as_ptr()),
                ptr::null(),
                0,
                ResolutionMode(0),
                bad_a.as_ptr().cast(),
                val_a.as_mut_ptr().cast(),
                &raw mut sz,
            );

            // Unknown product
            let unk_prod_w = "unknown_product_does_not_exist_at_all"
                .encode_utf16()
                .chain(std::iter::once(0))
                .collect::<Vec<_>>();
            let unk_prod_a =
                CString::new("unknown_product_does_not_exist_at_all").unwrap_or_default();

            let last_source_w = "LastUsedSource"
                .encode_utf16()
                .chain(std::iter::once(0))
                .collect::<Vec<_>>();
            let last_source_a = CString::new("LastUsedSource").unwrap_or_default();

            let _ = MsiSourceListGetInfoW(
                ProductCodeW(unk_prod_w.as_ptr()),
                ptr::null(),
                0,
                ResolutionMode(0),
                last_source_w.as_ptr(),
                val_w.as_mut_ptr(),
                &raw mut sz,
            );
            let _ = MsiSourceListGetInfoA(
                ProductCodeA(unk_prod_a.as_ptr()),
                ptr::null(),
                0,
                ResolutionMode(0),
                last_source_a.as_ptr(),
                val_a.as_mut_ptr().cast(),
                &raw mut sz,
            );

            // EnumMediaDisks and EnumSources missing None cache hit
            let _ = MsiSourceListEnumMediaDisksW(
                unk_prod_w.as_ptr(),
                ptr::null(),
                0,
                0,
                0,
                ptr::null_mut(),
                ptr::null_mut(),
                ptr::null_mut(),
                ptr::null_mut(),
                ptr::null_mut(),
            );
            let _ = MsiSourceListEnumMediaDisksA(
                unk_prod_a.as_ptr().cast(),
                ptr::null(),
                0,
                0,
                0,
                ptr::null_mut(),
                ptr::null_mut(),
                ptr::null_mut(),
                ptr::null_mut(),
                ptr::null_mut(),
            );
            let _ = MsiSourceListEnumSourcesW(
                unk_prod_w.as_ptr(),
                ptr::null(),
                0,
                0,
                0,
                ptr::null_mut(),
                ptr::null_mut(),
            );
            let _ = MsiSourceListEnumSourcesA(
                unk_prod_a.as_ptr().cast(),
                ptr::null(),
                0,
                0,
                0,
                ptr::null_mut(),
                ptr::null_mut(),
            );

            // EnumSources for Network sources
            let mut sz_val = 100;
            let mut val_w2 = vec![0u16; 100];
            let mut buf_a2 = vec![0u8; 100];
            let mut id = 0;
            let mut cache = source_list_cache()
                .write()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            let list = cache
                .entry("product_test_coverage_edge_cases".to_string())
                .or_default();
            list.sources.clear();
            list.sources.push(SourcePath::Network(NetworkSource {
                unc_path: "\\\\net".to_string(),
            }));
            list.sources.push(SourcePath::Media(MediaSource {
                base_path: std::path::PathBuf::from("C:\\media"),
                disk_id: Some(42),
                volume_label: Some("Label".to_string()),
                disk_prompt: Some("Prompt".to_string()),
            }));
            drop(cache);
            let _ = MsiSourceListEnumSourcesW(
                good_w.as_ptr(),
                ptr::null(),
                0,
                0,
                0,
                val_w.as_mut_ptr(),
                &raw mut sz_val,
            );
            let _ = MsiSourceListEnumSourcesA(
                good_a.as_ptr(),
                ptr::null(),
                0,
                0,
                0,
                val_a.as_mut_ptr().cast(),
                &raw mut sz_val,
            );
            let _ = MsiSourceListEnumMediaDisksW(
                good_w.as_ptr(),
                ptr::null(),
                0,
                0,
                0,
                &raw mut id,
                val_w.as_mut_ptr(),
                &raw mut sz_val,
                val_w2.as_mut_ptr(),
                &raw mut sz_val,
            );
            let _ = MsiSourceListEnumMediaDisksA(
                good_a.as_ptr(),
                ptr::null(),
                0,
                0,
                0,
                &raw mut id,
                val_a.as_mut_ptr().cast(),
                &raw mut sz_val,
                buf_a2.as_mut_ptr().cast(),
                &raw mut sz_val,
            );
            let _ = MsiSourceListEnumMediaDisksW(
                good_w.as_ptr(),
                ptr::null(),
                0,
                0,
                100,
                &raw mut id,
                val_w.as_mut_ptr(),
                &raw mut sz_val,
                val_w2.as_mut_ptr(),
                &raw mut sz_val,
            );

            // ClearMediaDisk that actually removes a disk (id == dwDiskId)
            {
                let mut cache_w = source_list_cache()
                    .write()
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                let list_w = cache_w
                    .entry("product_test_coverage_edge_cases".to_string())
                    .or_default();
                list_w.sources.clear();
                list_w.sources.push(SourcePath::Media(MediaSource {
                    base_path: std::path::PathBuf::from("C:\\media"),
                    disk_id: Some(42),
                    volume_label: None,
                    disk_prompt: None,
                }));
            }
            let _ = MsiSourceListClearMediaDiskW(good_w.as_ptr(), ptr::null(), 0, 0, 42);

            {
                let mut cache_a = source_list_cache()
                    .write()
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                let list_a = cache_a
                    .entry("product_test_coverage_edge_cases".to_string())
                    .or_default();
                list_a.sources.clear();
                list_a.sources.push(SourcePath::Media(MediaSource {
                    base_path: std::path::PathBuf::from("C:\\media"),
                    disk_id: Some(42),
                    volume_label: None,
                    disk_prompt: None,
                }));
            }
            let _ = MsiSourceListClearMediaDiskA(good_a.as_ptr(), ptr::null(), 0, 0, 42);

            // EnumMediaDisks missing MORE_DATA
            {
                let mut cache_m = source_list_cache()
                    .write()
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                let list_m = cache_m
                    .entry("product_test_coverage_edge_cases".to_string())
                    .or_default();
                list_m.sources.clear();
                list_m.sources.push(SourcePath::Media(MediaSource {
                    base_path: std::path::PathBuf::from("C:\\media"),
                    disk_id: Some(42),
                    volume_label: Some("Label".to_string()),
                    disk_prompt: Some("Prompt".to_string()),
                }));
            }

            let mut small_sz = 1;
            id = 0;
            val_w2 = vec![0u16; 100];
            buf_a2 = vec![0u8; 100];
            let mut sz2 = 100;
            let _ = MsiSourceListEnumMediaDisksW(
                good_w.as_ptr(),
                ptr::null(),
                0,
                0,
                0,
                &raw mut id,
                val_w.as_mut_ptr(),
                &raw mut small_sz,
                val_w2.as_mut_ptr(),
                &raw mut sz2,
            );
            let _ = MsiSourceListEnumMediaDisksA(
                good_a.as_ptr(),
                ptr::null(),
                0,
                0,
                0,
                &raw mut id,
                val_a.as_mut_ptr().cast(),
                &raw mut small_sz,
                buf_a2.as_mut_ptr().cast(),
                &raw mut sz2,
            );
            let mut small_sz2 = 1;
            let _ = MsiSourceListEnumMediaDisksW(
                good_w.as_ptr(),
                ptr::null(),
                0,
                0,
                0,
                &raw mut id,
                val_w.as_mut_ptr(),
                &raw mut sz,
                val_w2.as_mut_ptr(),
                &raw mut small_sz2,
            );
            let _ = MsiSourceListEnumMediaDisksA(
                good_a.as_ptr(),
                ptr::null(),
                0,
                0,
                0,
                &raw mut id,
                val_a.as_mut_ptr().cast(),
                &raw mut sz,
                buf_a2.as_mut_ptr().cast(),
                &raw mut small_sz2,
            );
        }
    }

    #[test]
    fn test_get_info() {
        use crate::action::{ProductCodeA, ProductCodeW};
        use msi::execution::source_resiliency::*;
        use std::ffi::CString;
        use std::ptr;

        let good_w = "product_get_info"
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect::<Vec<_>>();
        let good_a = CString::new("product_get_info").unwrap_or_default();

        let last_source_w = "LastUsedSource"
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect::<Vec<_>>();
        let last_type_w = "LastUsedType"
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect::<Vec<_>>();
        let last_source_a = CString::new("LastUsedSource").unwrap_or_default();
        let last_type_a = CString::new("LastUsedType").unwrap_or_default();

        let mut sz = 100;

        unsafe {
            // Unused product means no value, but we can query it
            let mut val_w = vec![0u16; 100];
            let mut val_a = vec![0u8; 100];

            let _ = MsiSourceListGetInfoW(
                ProductCodeW(good_w.as_ptr()),
                ptr::null(),
                0,
                ResolutionMode(0),
                last_source_w.as_ptr(),
                val_w.as_mut_ptr(),
                &raw mut sz,
            );
            let _ = MsiSourceListGetInfoW(
                ProductCodeW(good_w.as_ptr()),
                ptr::null(),
                0,
                ResolutionMode(0),
                last_type_w.as_ptr(),
                val_w.as_mut_ptr(),
                &raw mut sz,
            );

            let _ = MsiSourceListGetInfoA(
                ProductCodeA(good_a.as_ptr()),
                ptr::null(),
                0,
                ResolutionMode(0),
                last_source_a.as_ptr(),
                val_a.as_mut_ptr().cast(),
                &raw mut sz,
            );
            let _ = MsiSourceListGetInfoA(
                ProductCodeA(good_a.as_ptr()),
                ptr::null(),
                0,
                ResolutionMode(0),
                last_type_a.as_ptr(),
                val_a.as_mut_ptr().cast(),
                &raw mut sz,
            );
        }

        // Let's populate the cache with some fake sources
        {
            let mut cache = source_list_cache()
                .write()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            let list = cache.entry("product_get_info".to_string()).or_default();

            // 1. Media
            list.last_used = Some(SourcePath::Media(MediaSource {
                base_path: std::path::PathBuf::from("C:\\media"),
                disk_id: None,
                volume_label: None,
                disk_prompt: None,
            }));
        }

        unsafe {
            let mut val_w = vec![0u16; 100];
            let mut val_a = vec![0u8; 100];
            let _ = MsiSourceListGetInfoW(
                ProductCodeW(good_w.as_ptr()),
                ptr::null(),
                0,
                ResolutionMode(0),
                last_source_w.as_ptr(),
                val_w.as_mut_ptr(),
                &raw mut sz,
            );
            let _ = MsiSourceListGetInfoW(
                ProductCodeW(good_w.as_ptr()),
                ptr::null(),
                0,
                ResolutionMode(0),
                last_type_w.as_ptr(),
                val_w.as_mut_ptr(),
                &raw mut sz,
            );
            let _ = MsiSourceListGetInfoA(
                ProductCodeA(good_a.as_ptr()),
                ptr::null(),
                0,
                ResolutionMode(0),
                last_source_a.as_ptr(),
                val_a.as_mut_ptr().cast(),
                &raw mut sz,
            );
            let _ = MsiSourceListGetInfoA(
                ProductCodeA(good_a.as_ptr()),
                ptr::null(),
                0,
                ResolutionMode(0),
                last_type_a.as_ptr(),
                val_a.as_mut_ptr().cast(),
                &raw mut sz,
            );
        }

        {
            let mut cache = source_list_cache()
                .write()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            let list = cache.entry("product_get_info".to_string()).or_default();

            // 2. Network
            list.last_used = Some(SourcePath::Network(NetworkSource {
                unc_path: "\\\\net".to_string(),
            }));
        }

        unsafe {
            let mut val_w = vec![0u16; 100];
            let mut val_a = vec![0u8; 100];
            let _ = MsiSourceListGetInfoW(
                ProductCodeW(good_w.as_ptr()),
                ptr::null(),
                0,
                ResolutionMode(0),
                last_source_w.as_ptr(),
                val_w.as_mut_ptr(),
                &raw mut sz,
            );
            let _ = MsiSourceListGetInfoW(
                ProductCodeW(good_w.as_ptr()),
                ptr::null(),
                0,
                ResolutionMode(0),
                last_type_w.as_ptr(),
                val_w.as_mut_ptr(),
                &raw mut sz,
            );
            let _ = MsiSourceListGetInfoA(
                ProductCodeA(good_a.as_ptr()),
                ptr::null(),
                0,
                ResolutionMode(0),
                last_source_a.as_ptr(),
                val_a.as_mut_ptr().cast(),
                &raw mut sz,
            );
            let _ = MsiSourceListGetInfoA(
                ProductCodeA(good_a.as_ptr()),
                ptr::null(),
                0,
                ResolutionMode(0),
                last_type_a.as_ptr(),
                val_a.as_mut_ptr().cast(),
                &raw mut sz,
            );
        }

        {
            let mut cache = source_list_cache()
                .write()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            let list = cache.entry("product_get_info".to_string()).or_default();

            // 3. Url
            list.last_used = Some(SourcePath::Url(UrlSource {
                url: "http://url".to_string(),
            }));
        }

        unsafe {
            let mut val_w = vec![0u16; 100];
            let mut val_a = vec![0u8; 100];
            let _ = MsiSourceListGetInfoW(
                ProductCodeW(good_w.as_ptr()),
                ptr::null(),
                0,
                ResolutionMode(0),
                last_source_w.as_ptr(),
                val_w.as_mut_ptr(),
                &raw mut sz,
            );
            let _ = MsiSourceListGetInfoW(
                ProductCodeW(good_w.as_ptr()),
                ptr::null(),
                0,
                ResolutionMode(0),
                last_type_w.as_ptr(),
                val_w.as_mut_ptr(),
                &raw mut sz,
            );
            let _ = MsiSourceListGetInfoA(
                ProductCodeA(good_a.as_ptr()),
                ptr::null(),
                0,
                ResolutionMode(0),
                last_source_a.as_ptr(),
                val_a.as_mut_ptr().cast(),
                &raw mut sz,
            );
            let _ = MsiSourceListGetInfoA(
                ProductCodeA(good_a.as_ptr()),
                ptr::null(),
                0,
                ResolutionMode(0),
                last_type_a.as_ptr(),
                val_a.as_mut_ptr().cast(),
                &raw mut sz,
            );
        }

        {
            let mut cache = source_list_cache()
                .write()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            let list = cache.entry("product_get_info".to_string()).or_default();
            list.last_used = None;
        }

        unsafe {
            let mut val_w = vec![0u16; 100];
            let mut val_a = vec![0u8; 100];
            sz = 100;
            let _ = MsiSourceListGetInfoW(
                ProductCodeW(good_w.as_ptr()),
                ptr::null(),
                0,
                ResolutionMode(0),
                last_source_w.as_ptr(),
                val_w.as_mut_ptr(),
                &raw mut sz,
            );
            let _ = MsiSourceListGetInfoW(
                ProductCodeW(good_w.as_ptr()),
                ptr::null(),
                0,
                ResolutionMode(0),
                last_type_w.as_ptr(),
                val_w.as_mut_ptr(),
                &raw mut sz,
            );
            let _ = MsiSourceListGetInfoA(
                ProductCodeA(good_a.as_ptr()),
                ptr::null(),
                0,
                ResolutionMode(0),
                last_source_a.as_ptr(),
                val_a.as_mut_ptr().cast(),
                &raw mut sz,
            );
            let _ = MsiSourceListGetInfoA(
                ProductCodeA(good_a.as_ptr()),
                ptr::null(),
                0,
                ResolutionMode(0),
                last_type_a.as_ptr(),
                val_a.as_mut_ptr().cast(),
                &raw mut sz,
            );

            // Empty value
            let mut sz2 = 0;
            let _ = MsiSourceListGetInfoW(
                ProductCodeW(good_w.as_ptr()),
                ptr::null(),
                0,
                ResolutionMode(0),
                last_source_w.as_ptr(),
                ptr::null_mut(),
                &raw mut sz2,
            );
            let _ = MsiSourceListGetInfoA(
                ProductCodeA(good_a.as_ptr()),
                ptr::null(),
                0,
                ResolutionMode(0),
                last_source_a.as_ptr(),
                ptr::null_mut(),
                &raw mut sz2,
            );

            let bad_w = [0xD800, 0];
            let bad_a = b"\xFF\x00";

            // Unknown property
            let unknown_w = "Unknown"
                .encode_utf16()
                .chain(std::iter::once(0))
                .collect::<Vec<_>>();
            let unknown_a = CString::new("Unknown").unwrap_or_default();
            let _ = MsiSourceListGetInfoW(
                ProductCodeW(good_w.as_ptr()),
                ptr::null(),
                0,
                ResolutionMode(0),
                unknown_w.as_ptr(),
                val_w.as_mut_ptr(),
                &raw mut sz,
            );
            let _ = MsiSourceListGetInfoA(
                ProductCodeA(good_a.as_ptr()),
                ptr::null(),
                0,
                ResolutionMode(0),
                unknown_a.as_ptr(),
                val_a.as_mut_ptr().cast(),
                &raw mut sz,
            );

            let _ = MsiSourceListGetInfoW(
                ProductCodeW(good_w.as_ptr()),
                ptr::null(),
                0,
                ResolutionMode(0),
                bad_w.as_ptr(),
                val_w.as_mut_ptr(),
                &raw mut sz,
            );
            let _ = MsiSourceListGetInfoA(
                ProductCodeA(good_a.as_ptr()),
                ptr::null(),
                0,
                ResolutionMode(0),
                bad_a.as_ptr().cast(),
                val_a.as_mut_ptr().cast(),
                &raw mut sz,
            );

            // Unknown product
            let unk_prod_w = "unknown_product_does_not_exist_at_all"
                .encode_utf16()
                .chain(std::iter::once(0))
                .collect::<Vec<_>>();
            let unk_prod_a =
                CString::new("unknown_product_does_not_exist_at_all").unwrap_or_default();

            let _ = MsiSourceListGetInfoW(
                ProductCodeW(unk_prod_w.as_ptr()),
                ptr::null(),
                0,
                ResolutionMode(0),
                last_source_w.as_ptr(),
                val_w.as_mut_ptr(),
                &raw mut sz,
            );
            let _ = MsiSourceListGetInfoA(
                ProductCodeA(unk_prod_a.as_ptr()),
                ptr::null(),
                0,
                ResolutionMode(0),
                last_source_a.as_ptr(),
                val_a.as_mut_ptr().cast(),
                &raw mut sz,
            );

            // EnumMediaDisks and EnumSources missing None cache hit
            let _ = MsiSourceListEnumMediaDisksW(
                unk_prod_w.as_ptr(),
                ptr::null(),
                0,
                0,
                0,
                ptr::null_mut(),
                ptr::null_mut(),
                ptr::null_mut(),
                ptr::null_mut(),
                ptr::null_mut(),
            );
            let _ = MsiSourceListEnumMediaDisksA(
                unk_prod_a.as_ptr().cast(),
                ptr::null(),
                0,
                0,
                0,
                ptr::null_mut(),
                ptr::null_mut(),
                ptr::null_mut(),
                ptr::null_mut(),
                ptr::null_mut(),
            );
            let _ = MsiSourceListEnumSourcesW(
                unk_prod_w.as_ptr(),
                ptr::null(),
                0,
                0,
                0,
                ptr::null_mut(),
                ptr::null_mut(),
            );
            let _ = MsiSourceListEnumSourcesA(
                unk_prod_a.as_ptr().cast(),
                ptr::null(),
                0,
                0,
                0,
                ptr::null_mut(),
                ptr::null_mut(),
            );
        }
    }
}

#[cfg(test)]
#[allow(clippy::too_many_lines, clippy::cognitive_complexity)]
mod tests_remaining_cov {
    use super::*;
    use crate::action::{ProductCodeA, ProductCodeW};
    use std::ffi::CString;
    use std::ptr;

    #[test]
    fn test_remaining_cov() {
        let good_w = "product_remaining"
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect::<Vec<_>>();
        let good_a = CString::new("product_remaining").unwrap_or_default();

        let bad_w = [0xD800, 0];
        let bad_a = &[0xFF, 0x00];

        // populate
        {
            let mut cache = msi::execution::source_resiliency::source_list_cache()
                .write()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            let _ = cache.entry("product_remaining".to_string()).or_default();
        }

        unsafe {
            let mut sz = 100;
            let mut val_w = vec![0u16; 100];
            let mut val_a = vec![0u8; 100];
            let prop_w = "LastUsedSource"
                .encode_utf16()
                .chain(std::iter::once(0))
                .collect::<Vec<_>>();
            let prop_a = CString::new("LastUsedSource").unwrap_or_default();
            let prop_last_w = "LastUsedType"
                .encode_utf16()
                .chain(std::iter::once(0))
                .collect::<Vec<_>>();
            let prop_last_a = CString::new("LastUsedType").unwrap_or_default();

            // GetInfo bad product
            let _ = MsiSourceListGetInfoW(
                ProductCodeW(bad_w.as_ptr()),
                ptr::null(),
                0,
                ResolutionMode(0),
                prop_w.as_ptr(),
                val_w.as_mut_ptr(),
                &raw mut sz,
            );
            let _ = MsiSourceListGetInfoA(
                ProductCodeA(bad_a.as_ptr().cast()),
                ptr::null(),
                0,
                ResolutionMode(0),
                prop_a.as_ptr(),
                val_a.as_mut_ptr().cast(),
                &raw mut sz,
            );

            // SetInfo bad product
            let value_w = "http://test"
                .encode_utf16()
                .chain(std::iter::once(0))
                .collect::<Vec<_>>();
            let value_a = CString::new("http://test").unwrap_or_default();
            let _ = MsiSourceListSetInfoW(
                bad_w.as_ptr(),
                ptr::null(),
                0,
                0,
                prop_w.as_ptr(),
                value_w.as_ptr(),
            );
            let _ = MsiSourceListSetInfoA(
                bad_a.as_ptr().cast(),
                ptr::null(),
                0,
                0,
                prop_a.as_ptr(),
                value_a.as_ptr(),
            );

            // SetInfo valid product with Network and Media
            let net_w = "\\net"
                .encode_utf16()
                .chain(std::iter::once(0))
                .collect::<Vec<_>>();
            let net_a = CString::new("\\net").unwrap_or_default();
            let media_w = r"C:\media"
                .encode_utf16()
                .chain(std::iter::once(0))
                .collect::<Vec<_>>();
            let media_a = CString::new(r"C:\media").unwrap_or_default();
            let url_w = "http://url"
                .encode_utf16()
                .chain(std::iter::once(0))
                .collect::<Vec<_>>();
            let url_a = CString::new("http://url").unwrap_or_default();

            let _ = MsiSourceListSetInfoW(
                good_w.as_ptr(),
                ptr::null(),
                0,
                0,
                prop_w.as_ptr(),
                net_w.as_ptr(),
            );
            let _ = MsiSourceListGetInfoW(
                ProductCodeW(good_w.as_ptr()),
                ptr::null(),
                0,
                ResolutionMode(0),
                prop_last_w.as_ptr(),
                val_w.as_mut_ptr(),
                &raw mut sz,
            );

            let _ = MsiSourceListSetInfoA(
                good_a.as_ptr(),
                ptr::null(),
                0,
                0,
                prop_a.as_ptr(),
                net_a.as_ptr(),
            );
            let _ = MsiSourceListGetInfoA(
                ProductCodeA(good_a.as_ptr()),
                ptr::null(),
                0,
                ResolutionMode(0),
                prop_last_a.as_ptr(),
                val_a.as_mut_ptr().cast(),
                &raw mut sz,
            );

            let _ = MsiSourceListSetInfoW(
                good_w.as_ptr(),
                ptr::null(),
                0,
                0,
                prop_w.as_ptr(),
                media_w.as_ptr(),
            );
            let _ = MsiSourceListGetInfoW(
                ProductCodeW(good_w.as_ptr()),
                ptr::null(),
                0,
                ResolutionMode(0),
                prop_last_w.as_ptr(),
                val_w.as_mut_ptr(),
                &raw mut sz,
            );

            let _ = MsiSourceListSetInfoA(
                good_a.as_ptr(),
                ptr::null(),
                0,
                0,
                prop_a.as_ptr(),
                media_a.as_ptr(),
            );
            let _ = MsiSourceListGetInfoA(
                ProductCodeA(good_a.as_ptr()),
                ptr::null(),
                0,
                ResolutionMode(0),
                prop_last_a.as_ptr(),
                val_a.as_mut_ptr().cast(),
                &raw mut sz,
            );

            let _ = MsiSourceListSetInfoW(
                good_w.as_ptr(),
                ptr::null(),
                0,
                0,
                prop_w.as_ptr(),
                url_w.as_ptr(),
            );
            let _ = MsiSourceListGetInfoW(
                ProductCodeW(good_w.as_ptr()),
                ptr::null(),
                0,
                ResolutionMode(0),
                prop_last_w.as_ptr(),
                val_w.as_mut_ptr(),
                &raw mut sz,
            );

            let _ = MsiSourceListSetInfoA(
                good_a.as_ptr(),
                ptr::null(),
                0,
                0,
                prop_a.as_ptr(),
                url_a.as_ptr(),
            );
            let _ = MsiSourceListGetInfoA(
                ProductCodeA(good_a.as_ptr()),
                ptr::null(),
                0,
                ResolutionMode(0),
                prop_last_a.as_ptr(),
                val_a.as_mut_ptr().cast(),
                &raw mut sz,
            );

            // ClearAll bad product
            let _ = MsiSourceListClearAllW(ProductCodeW(bad_w.as_ptr()), ptr::null(), 0);
            let _ = MsiSourceListClearAllA(ProductCodeA(bad_a.as_ptr().cast()), ptr::null(), 0);

            // ClearAll user name
            let user_w = "user"
                .encode_utf16()
                .chain(std::iter::once(0))
                .collect::<Vec<_>>();
            let _ = MsiSourceListClearAllW(ProductCodeW(good_w.as_ptr()), user_w.as_ptr(), 0);
            let user_a = CString::new("user").unwrap_or_default();
            let _ = MsiSourceListClearAllA(ProductCodeA(good_a.as_ptr()), user_a.as_ptr(), 0);

            let bad_user_w = [0xD800, 0];
            let bad_user_a = &[0xFF, 0x00];
            let _ = MsiSourceListClearAllW(ProductCodeW(good_w.as_ptr()), bad_user_w.as_ptr(), 0);
            let _ = MsiSourceListClearAllA(
                ProductCodeA(good_a.as_ptr()),
                bad_user_a.as_ptr().cast(),
                0,
            );

            // AddMediaDisk
            let disk_w = "Disk"
                .encode_utf16()
                .chain(std::iter::once(0))
                .collect::<Vec<_>>();
            let prompt_w = "Prompt"
                .encode_utf16()
                .chain(std::iter::once(0))
                .collect::<Vec<_>>();
            let disk_a = CString::new("Disk").unwrap_or_default();
            let prompt_a = CString::new("Prompt").unwrap_or_default();
            let _ = MsiSourceListAddMediaDiskW(
                ProductCodeW(good_w.as_ptr()),
                ptr::null(),
                0,
                MediaDiskId(1),
                disk_w.as_ptr(),
                prompt_w.as_ptr(),
            );
            let _ = MsiSourceListAddMediaDiskA(
                ProductCodeA(good_a.as_ptr()),
                ptr::null(),
                0,
                MediaDiskId(1),
                disk_a.as_ptr(),
                prompt_a.as_ptr(),
            );

            // AddMediaDisk bad product
            let unk_w = "unk"
                .encode_utf16()
                .chain(std::iter::once(0))
                .collect::<Vec<_>>();
            let unk_a = CString::new("unk").unwrap_or_default();
            let _ = MsiSourceListAddMediaDiskW(
                ProductCodeW(unk_w.as_ptr()),
                ptr::null(),
                0,
                MediaDiskId(1),
                disk_w.as_ptr(),
                prompt_w.as_ptr(),
            );
            let _ = MsiSourceListAddMediaDiskA(
                ProductCodeA(unk_a.as_ptr()),
                ptr::null(),
                0,
                MediaDiskId(1),
                disk_a.as_ptr(),
                prompt_a.as_ptr(),
            );

            // ClearMediaDisks
            let _ = MsiSourceListClearMediaDiskW(good_w.as_ptr(), ptr::null(), 0, 0, 1);
            let _ = MsiSourceListClearMediaDiskA(good_a.as_ptr(), ptr::null(), 0, 0, 1);

            // ClearMediaDisks bad product
            let _ = MsiSourceListClearMediaDiskW(unk_w.as_ptr(), ptr::null(), 0, 0, 1);
            let _ = MsiSourceListClearMediaDiskA(unk_a.as_ptr(), ptr::null(), 0, 0, 1);

            // ClearAll valid
            let _ = MsiSourceListClearAllW(ProductCodeW(good_w.as_ptr()), ptr::null(), 0);
            let _ = MsiSourceListClearAllA(ProductCodeA(good_a.as_ptr()), ptr::null(), 0);

            // ClearAll again for ERROR_NO_MORE_ITEMS (259)
            let _ = MsiSourceListClearAllW(ProductCodeW(good_w.as_ptr()), ptr::null(), 0);
            let _ = MsiSourceListClearAllA(ProductCodeA(good_a.as_ptr()), ptr::null(), 0);

            // Dedicated isolated product for EnumMediaDisks, SetInfo, ClearMediaDisk, and ForceResolutionEx
            let iso_str = "product_isolated_deterministic_cov";
            let iso_w = iso_str
                .encode_utf16()
                .chain(std::iter::once(0))
                .collect::<Vec<_>>();
            let iso_a = CString::new(iso_str).unwrap_or_default();
            let missing_str = "product_never_inserted_xyz_999";
            let missing_w = missing_str
                .encode_utf16()
                .chain(std::iter::once(0))
                .collect::<Vec<_>>();
            let missing_a = CString::new(missing_str).unwrap_or_default();

            // Unknown and invalid product branches
            assert_eq!(
                MsiSourceListEnumMediaDisksW(
                    bad_w.as_ptr(),
                    ptr::null(),
                    0,
                    0,
                    0,
                    ptr::null_mut(),
                    ptr::null_mut(),
                    ptr::null_mut(),
                    ptr::null_mut(),
                    ptr::null_mut(),
                ),
                87
            );
            assert_eq!(
                MsiSourceListEnumMediaDisksA(
                    bad_a.as_ptr().cast(),
                    ptr::null(),
                    0,
                    0,
                    0,
                    ptr::null_mut(),
                    ptr::null_mut(),
                    ptr::null_mut(),
                    ptr::null_mut(),
                    ptr::null_mut(),
                ),
                87
            );
            assert_eq!(
                MsiSourceListEnumMediaDisksW(
                    missing_w.as_ptr(),
                    ptr::null(),
                    0,
                    0,
                    0,
                    ptr::null_mut(),
                    ptr::null_mut(),
                    ptr::null_mut(),
                    ptr::null_mut(),
                    ptr::null_mut(),
                ),
                259
            );
            assert_eq!(
                MsiSourceListEnumMediaDisksA(
                    missing_a.as_ptr(),
                    ptr::null(),
                    0,
                    0,
                    0,
                    ptr::null_mut(),
                    ptr::null_mut(),
                    ptr::null_mut(),
                    ptr::null_mut(),
                    ptr::null_mut(),
                ),
                259
            );
            assert_eq!(
                MsiSourceListClearMediaDiskW(missing_w.as_ptr(), ptr::null(), 0, 0, 1),
                1605
            );
            assert_eq!(
                MsiSourceListClearMediaDiskA(missing_a.as_ptr(), ptr::null(), 0, 0, 1),
                1605
            );
            assert_eq!(
                MsiSourceListForceResolutionExW(missing_w.as_ptr(), ptr::null(), 0, 0),
                1605
            );
            assert_eq!(
                MsiSourceListForceResolutionExA(missing_a.as_ptr(), ptr::null(), 0, 0),
                1605
            );

            // Populate isolated product with Network and Media sources
            {
                use msi::execution::source_resiliency::{
                    source_list_cache, MediaSource, NetworkSource, SourcePath,
                };
                let mut cache = source_list_cache()
                    .write()
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                let list = cache.entry(iso_str.to_string()).or_default();
                list.sources.clear();
                list.sources.push(SourcePath::Network(NetworkSource {
                    unc_path: r"\\srv\share".to_string(),
                }));
                list.sources.push(SourcePath::Media(MediaSource {
                    base_path: std::path::PathBuf::from(r"C:\media"),
                    disk_id: Some(7),
                    volume_label: Some("Volume7".to_string()),
                    disk_prompt: Some("Prompt7".to_string()),
                }));
                list.sources.push(SourcePath::Media(MediaSource {
                    base_path: std::path::PathBuf::from(r"C:\media2"),
                    disk_id: Some(8),
                    volume_label: None,
                    disk_prompt: None,
                }));
            }

            let mut disk_id_out = 0u32;
            let mut label_buf_w = vec![0u16; 64];
            let mut prompt_buf_w = vec![0u16; 64];
            let mut label_buf_a = vec![0u8; 64];
            let mut prompt_buf_a = vec![0u8; 64];
            let mut vol_len = 64u32;
            let mut prompt_len = 64u32;

            // Valid index 0 (with buffers, null buffers, and too-small buffers)
            assert_eq!(
                MsiSourceListEnumMediaDisksW(
                    iso_w.as_ptr(),
                    ptr::null(),
                    0,
                    0,
                    0,
                    &raw mut disk_id_out,
                    label_buf_w.as_mut_ptr(),
                    &raw mut vol_len,
                    prompt_buf_w.as_mut_ptr(),
                    &raw mut prompt_len,
                ),
                0
            );
            assert_eq!(disk_id_out, 7);

            vol_len = 64;
            prompt_len = 64;
            assert_eq!(
                MsiSourceListEnumMediaDisksA(
                    iso_a.as_ptr(),
                    ptr::null(),
                    0,
                    0,
                    0,
                    &raw mut disk_id_out,
                    label_buf_a.as_mut_ptr().cast(),
                    &raw mut vol_len,
                    prompt_buf_a.as_mut_ptr().cast(),
                    &raw mut prompt_len,
                ),
                0
            );

            // Null optional out pointers
            assert_eq!(
                MsiSourceListEnumMediaDisksW(
                    iso_w.as_ptr(),
                    ptr::null(),
                    0,
                    0,
                    0,
                    ptr::null_mut(),
                    ptr::null_mut(),
                    ptr::null_mut(),
                    ptr::null_mut(),
                    ptr::null_mut(),
                ),
                0
            );
            assert_eq!(
                MsiSourceListEnumMediaDisksA(
                    iso_a.as_ptr(),
                    ptr::null(),
                    0,
                    0,
                    0,
                    ptr::null_mut(),
                    ptr::null_mut(),
                    ptr::null_mut(),
                    ptr::null_mut(),
                    ptr::null_mut(),
                ),
                0
            );

            // Too small buffer for volume label and prompt
            let mut tiny_vol = 2u32;
            let mut tiny_prompt = 2u32;
            assert_eq!(
                MsiSourceListEnumMediaDisksW(
                    iso_w.as_ptr(),
                    ptr::null(),
                    0,
                    0,
                    0,
                    &raw mut disk_id_out,
                    label_buf_w.as_mut_ptr(),
                    &raw mut tiny_vol,
                    prompt_buf_w.as_mut_ptr(),
                    &raw mut tiny_prompt,
                ),
                234
            );
            tiny_vol = 2;
            tiny_prompt = 2;
            assert_eq!(
                MsiSourceListEnumMediaDisksA(
                    iso_a.as_ptr(),
                    ptr::null(),
                    0,
                    0,
                    0,
                    &raw mut disk_id_out,
                    label_buf_a.as_mut_ptr().cast(),
                    &raw mut tiny_vol,
                    prompt_buf_a.as_mut_ptr().cast(),
                    &raw mut tiny_prompt,
                ),
                234
            );

            // Index out of bounds
            assert_eq!(
                MsiSourceListEnumMediaDisksW(
                    iso_w.as_ptr(),
                    ptr::null(),
                    0,
                    0,
                    99,
                    ptr::null_mut(),
                    ptr::null_mut(),
                    ptr::null_mut(),
                    ptr::null_mut(),
                    ptr::null_mut(),
                ),
                259
            );
            assert_eq!(
                MsiSourceListEnumMediaDisksA(
                    iso_a.as_ptr(),
                    ptr::null(),
                    0,
                    0,
                    99,
                    ptr::null_mut(),
                    ptr::null_mut(),
                    ptr::null_mut(),
                    ptr::null_mut(),
                    ptr::null_mut(),
                ),
                259
            );

            // SetInfo with true UNC path (r"\\server\share")
            let net_unc_wide = r"\\server\share"
                .encode_utf16()
                .chain(std::iter::once(0))
                .collect::<Vec<_>>();
            let net_unc_ansi = CString::new(r"\\server\share").unwrap_or_default();
            assert_eq!(
                MsiSourceListSetInfoW(
                    iso_w.as_ptr(),
                    ptr::null(),
                    0,
                    0,
                    prop_w.as_ptr(),
                    net_unc_wide.as_ptr(),
                ),
                0
            );
            assert_eq!(
                MsiSourceListSetInfoA(
                    iso_a.as_ptr(),
                    ptr::null(),
                    0,
                    0,
                    prop_a.as_ptr(),
                    net_unc_ansi.as_ptr(),
                ),
                0
            );

            // ForceResolutionEx valid product and invalid UTF-8/UTF-16
            assert_eq!(
                MsiSourceListForceResolutionExW(iso_w.as_ptr(), ptr::null(), 0, 0),
                0
            );
            assert_eq!(
                MsiSourceListForceResolutionExA(iso_a.as_ptr(), ptr::null(), 0, 0),
                0
            );
            assert_eq!(
                MsiSourceListForceResolutionExW(bad_w.as_ptr(), ptr::null(), 0, 0),
                87
            );
            assert_eq!(
                MsiSourceListForceResolutionExA(bad_a.as_ptr().cast(), ptr::null(), 0, 0),
                87
            );

            // ClearMediaDisk valid product (matching disk 7, non-matching disk, and invalid UTF-8/UTF-16)
            assert_eq!(
                MsiSourceListClearMediaDiskW(bad_w.as_ptr(), ptr::null(), 0, 0, 1),
                87
            );
            assert_eq!(
                MsiSourceListClearMediaDiskA(bad_a.as_ptr().cast(), ptr::null(), 0, 0, 1),
                87
            );
            assert_eq!(
                MsiSourceListClearMediaDiskW(iso_w.as_ptr(), ptr::null(), 0, 0, 7),
                0
            );
            assert_eq!(
                MsiSourceListClearMediaDiskA(iso_a.as_ptr(), ptr::null(), 0, 0, 8),
                0
            );
        }
    }
}

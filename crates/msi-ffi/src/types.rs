//! Opaque handles and memory lifecycle destructors for the C-ABI.
//!
//! Provides thread-safe, memory-safe encapsulation of Rust data structures across the FFI.

use std::ffi::c_char;

/// Opaque wrapper for a Windows Installer package builder.
#[derive(Debug)]
pub struct MsiPackageBuilderHandle {
    /// Inner package builder instance.
    pub inner: msi::package::PackageBuilder,
}

/// Opaque wrapper for a complete Windows Installer package.
#[derive(Debug)]
pub struct MsiPackageHandle {
    /// Inner package instance.
    pub inner: msi::package::Package,
}

/// Opaque wrapper for a relational database.
#[derive(Debug)]
pub struct MsiDatabaseHandle {
    /// Inner linked database instance.
    pub inner: msi::wix::linker::LinkedDatabase,
}

/// Opaque wrapper for a relational database record.
#[derive(Debug)]
pub struct MsiRecordHandle {
    /// Inner record instance.
    pub inner: msi::database::tables::record::Record,
}

/// Opaque wrapper for Summary Information stream properties.
#[derive(Debug)]
pub struct MsiSummaryInfoHandle {
    /// Inner summary information instance.
    pub inner: msi::database::summary_info::SummaryInfo,
}

/// Opaque wrapper for a multi-package transaction manager.
#[derive(Debug)]
pub struct MsiTransactionHandle {
    /// Inner multi-package transaction manager.
    pub inner: msi::execution::transaction::MultiPackageTransactionManager,
}

/// Contiguous buffer descriptor representing allocated native memory.
#[repr(C)]
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct MsiBufferHandle {
    /// Pointer to the raw byte buffer.
    pub data: *mut u8,
    /// Length of the buffer in bytes.
    pub len: usize,
}

/// Frees an [`MsiPackageBuilderHandle`] allocated by the library.
///
/// If `handle` is NULL, this function is a safe no-op.
///
/// # Arguments
///
/// * `handle` - Pointer to the builder handle to destroy.
///
/// # Safety
///
/// `handle` must be a valid pointer obtained from `msi_package_builder_create`, or NULL.
#[no_mangle]
pub unsafe extern "C" fn msi_package_builder_destroy(handle: *mut MsiPackageBuilderHandle) {
    unsafe {
        if !handle.is_null() {
            drop(Box::from_raw(handle));
        }
    }
}

/// Frees an [`MsiPackageHandle`] allocated by the library.
///
/// If `handle` is NULL, this function is a safe no-op.
///
/// # Arguments
///
/// * `handle` - Pointer to the package handle to destroy.
///
/// # Safety
///
/// `handle` must be a valid pointer obtained from `msi_package_builder_build` or `msi_package_open`, or NULL.
#[no_mangle]
pub unsafe extern "C" fn msi_package_destroy(handle: *mut MsiPackageHandle) {
    unsafe {
        if !handle.is_null() {
            drop(Box::from_raw(handle));
        }
    }
}

/// Frees an [`MsiDatabaseHandle`] allocated by the library.
///
/// If `handle` is NULL, this function is a safe no-op.
///
/// # Arguments
///
/// * `handle` - Pointer to the database handle to destroy.
///
/// # Safety
///
/// `handle` must be a valid pointer obtained from the library, or NULL.
#[no_mangle]
pub unsafe extern "C" fn msi_database_destroy(handle: *mut MsiDatabaseHandle) {
    unsafe {
        if !handle.is_null() {
            drop(Box::from_raw(handle));
        }
    }
}

/// Frees an [`MsiRecordHandle`] allocated by the library.
///
/// If `handle` is NULL, this function is a safe no-op.
///
/// # Arguments
///
/// * `handle` - Pointer to the record handle to destroy.
///
/// # Safety
///
/// `handle` must be a valid pointer obtained from the library, or NULL.
#[no_mangle]
pub unsafe extern "C" fn msi_record_destroy(handle: *mut MsiRecordHandle) {
    unsafe {
        if !handle.is_null() {
            drop(Box::from_raw(handle));
        }
    }
}

/// Frees an [`MsiSummaryInfoHandle`] allocated by the library.
///
/// If `handle` is NULL, this function is a safe no-op.
///
/// # Arguments
///
/// * `handle` - Pointer to the summary info handle to destroy.
///
/// # Safety
///
/// `handle` must be a valid pointer obtained from the library, or NULL.
#[no_mangle]
pub unsafe extern "C" fn msi_summary_info_destroy(handle: *mut MsiSummaryInfoHandle) {
    unsafe {
        if !handle.is_null() {
            drop(Box::from_raw(handle));
        }
    }
}

/// Frees an [`MsiTransactionHandle`] allocated by the library.
///
/// If `handle` is NULL, this function is a safe no-op.
///
/// # Arguments
///
/// * `handle` - Pointer to the transaction handle to destroy.
///
/// # Safety
///
/// `handle` must be a valid pointer obtained from `msi_begin_transaction`, or NULL.
#[no_mangle]
pub unsafe extern "C" fn msi_transaction_destroy(handle: *mut MsiTransactionHandle) {
    unsafe {
        if !handle.is_null() {
            drop(Box::from_raw(handle));
        }
    }
}

/// Frees a null-terminated C string allocated by the library.
///
/// If `ptr` is NULL, this function is a safe no-op.
///
/// # Arguments
///
/// * `ptr` - Pointer to null-terminated string to free.
///
/// # Safety
///
/// `ptr` must be a pointer allocated by `CString::into_raw` or NULL.
#[no_mangle]
pub unsafe extern "C" fn msi_string_free(ptr: *mut c_char) {
    unsafe {
        if !ptr.is_null() {
            drop(std::ffi::CString::from_raw(ptr));
        }
    }
}

/// Frees a byte buffer allocated by the library.
///
/// If `ptr` is NULL or `len` is 0, this function is a safe no-op.
///
/// # Arguments
///
/// * `ptr` - Pointer to the byte buffer.
/// * `len` - Length of the buffer in bytes.
///
/// # Safety
///
/// `ptr` must be a buffer allocated via `Vec::into_raw_parts` or `Box::into_raw` of a slice of length `len`.
#[no_mangle]
pub unsafe extern "C" fn msi_buffer_free(ptr: *mut u8, len: usize) {
    unsafe {
        if !ptr.is_null() && len > 0 {
            let slice_ptr = std::ptr::slice_from_raw_parts_mut(ptr, len);
            drop(Box::from_raw(slice_ptr));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::CString;
    use std::ptr;

    #[test]
    fn test_destructors_null_tolerance() {
        // SAFETY: Passing null pointers to destructors is explicitly supported and tested as safe no-ops.
        unsafe {
            msi_package_builder_destroy(ptr::null_mut());
            msi_package_destroy(ptr::null_mut());
            msi_database_destroy(ptr::null_mut());
            msi_record_destroy(ptr::null_mut());
            msi_summary_info_destroy(ptr::null_mut());
            msi_string_free(ptr::null_mut());
            msi_buffer_free(ptr::null_mut(), 0);
            msi_buffer_free(ptr::null_mut(), 100);
        }
    }

    #[test]
    fn test_destructors_allocated_resources() {
        let builder = Box::into_raw(Box::new(MsiPackageBuilderHandle {
            inner: msi::package::PackageBuilder::default(),
        }));
        // SAFETY: builder is a valid heap allocation from Box::into_raw.
        unsafe { msi_package_builder_destroy(builder) };

        let meta = msi::package::PackageMetadata::new(
            "Test",
            "Mfr",
            msi::package::ProductVersion::new(1, 0, 0),
            "{00000000-0000-0000-0000-000000000000}",
        );
        let pkg = Box::into_raw(Box::new(MsiPackageHandle {
            inner: msi::package::Package::new(
                meta,
                msi::wix::linker::LinkedDatabase::default(),
                msi::database::summary_info::SummaryInfo::default(),
                std::collections::HashMap::new(),
            ),
        }));
        // SAFETY: pkg is a valid heap allocation from Box::into_raw.
        unsafe { msi_package_destroy(pkg) };

        let db = Box::into_raw(Box::new(MsiDatabaseHandle {
            inner: msi::wix::linker::LinkedDatabase::default(),
        }));
        // SAFETY: db is a valid heap allocation from Box::into_raw.
        unsafe { msi_database_destroy(db) };

        let rec = Box::into_raw(Box::new(MsiRecordHandle {
            inner: msi::database::tables::record::Record::new(),
        }));
        // SAFETY: rec is a valid heap allocation from Box::into_raw.
        unsafe { msi_record_destroy(rec) };

        let si = Box::into_raw(Box::new(MsiSummaryInfoHandle {
            inner: msi::database::summary_info::SummaryInfo::default(),
        }));
        // SAFETY: si is a valid heap allocation from Box::into_raw.
        unsafe { msi_summary_info_destroy(si) };

        let c_str = CString::new("test string").unwrap_or_default().into_raw();
        // SAFETY: c_str is a valid pointer from CString::into_raw.
        unsafe { msi_string_free(c_str) };

        let data = vec![1u8, 2, 3, 4, 5];
        let len = data.len();
        let buf_ptr = Box::into_raw(data.into_boxed_slice()).cast::<u8>();
        // SAFETY: buf_ptr is a valid slice pointer allocated with Box::into_raw of length len.
        unsafe { msi_buffer_free(buf_ptr, len) };

        let mut dummy = 42u8;
        // SAFETY: Non-null pointer with len 0 should be a safe no-op and not free anything.
        unsafe { msi_buffer_free(&raw mut dummy, 0) };
    }

    #[test]
    fn test_types_debug_and_traits() {
        let builder = MsiPackageBuilderHandle {
            inner: msi::package::PackageBuilder::default(),
        };
        assert_ne!(format!("{builder:?}"), "");

        let meta = msi::package::PackageMetadata::new(
            "Test",
            "Mfr",
            msi::package::ProductVersion::new(1, 0, 0),
            "{00000000-0000-0000-0000-000000000000}",
        );
        let pkg = MsiPackageHandle {
            inner: msi::package::Package::new(
                meta,
                msi::wix::linker::LinkedDatabase::default(),
                msi::database::summary_info::SummaryInfo::default(),
                std::collections::HashMap::new(),
            ),
        };
        assert_ne!(format!("{pkg:?}"), "");

        let db = MsiDatabaseHandle {
            inner: msi::wix::linker::LinkedDatabase::default(),
        };
        assert_ne!(format!("{db:?}"), "");

        let rec = MsiRecordHandle {
            inner: msi::database::tables::record::Record::new(),
        };
        assert_ne!(format!("{rec:?}"), "");

        let si = MsiSummaryInfoHandle {
            inner: msi::database::summary_info::SummaryInfo::default(),
        };
        assert_ne!(format!("{si:?}"), "");

        let mut val = 10u8;
        let buf = MsiBufferHandle {
            data: &raw mut val,
            len: 1,
        };
        assert_ne!(format!("{buf:?}"), "");
        let buf_clone = buf;
        assert_eq!(buf, buf_clone);

        let buf_other = MsiBufferHandle {
            data: ptr::null_mut(),
            len: 0,
        };
        assert_ne!(buf, buf_other);
    }
}

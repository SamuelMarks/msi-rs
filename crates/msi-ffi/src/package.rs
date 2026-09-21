//! C-ABI functions for package serialization, inspection, and cabinet extraction.
//!
//! Exposes package I/O capabilities to external languages.

use crate::error::{
    c_str_to_str, ffi_boundary, map_msi_error, MSI_ERROR_BUFFER_TOO_SMALL, MSI_ERROR_CABINET,
    MSI_ERROR_INVALID_ARGUMENT, MSI_ERROR_NULL_POINTER, MSI_SUCCESS,
};
use crate::types::MsiPackageHandle;
use msi::cab::reader::CabinetReader;
use msi::database::tables::record::FieldValue;
use msi::package::Package;
use std::ffi::c_char;
use std::fs;
use std::path::Path;
use std::ptr;

/// Saves the package to a file on disk.
///
/// # Arguments
///
/// * `package` - Package handle.
/// * `output_path` - Target `.msi` file path on disk.
///
/// # Returns
///
/// [`MSI_SUCCESS`] on success, or an error code.
///
/// # Safety
///
/// `package` must be a valid pointer obtained from `msi_package_builder_build` or `msi_package_open`.
/// `output_path` must be a valid null-terminated C string.
#[no_mangle]
pub unsafe extern "C" fn msi_package_save(
    package: *const MsiPackageHandle,
    output_path: *const c_char,
) -> i32 {
    unsafe {
        ffi_boundary(|| {
            if package.is_null() {
                return Err((
                    MSI_ERROR_NULL_POINTER,
                    "package handle must not be null".to_string(),
                ));
            }
            let path_str = c_str_to_str(output_path, "output_path")?;
            (*package)
                .inner
                .save(path_str)
                .map_err(|e| (map_msi_error(&e), e.to_string()))?;
            Ok(MSI_SUCCESS)
        })
    }
}

/// Serializes the package into an in-memory byte buffer.
///
/// The returned buffer must be freed using `msi_buffer_free`.
///
/// # Arguments
///
/// * `package` - Package handle.
/// * `out_bytes` - Pointer receiving the allocated byte array pointer.
/// * `out_len` - Pointer receiving the number of bytes written.
///
/// # Returns
///
/// [`MSI_SUCCESS`] on success, or an error code.
///
/// # Safety
///
/// `package` must be a valid pointer obtained from `msi_package_builder_build` or `msi_package_open`.
/// `out_bytes` and `out_len` must point to valid writable memory.
#[no_mangle]
pub unsafe extern "C" fn msi_package_to_bytes(
    package: *const MsiPackageHandle,
    out_bytes: *mut *mut u8,
    out_len: *mut usize,
) -> i32 {
    unsafe {
        ffi_boundary(|| {
            if package.is_null() {
                return Err((
                    MSI_ERROR_NULL_POINTER,
                    "package handle must not be null".to_string(),
                ));
            }
            if out_bytes.is_null() || out_len.is_null() {
                return Err((
                    MSI_ERROR_NULL_POINTER,
                    "out_bytes and out_len pointers must not be null".to_string(),
                ));
            }

            let bytes = (*package)
                .inner
                .to_bytes()
                .map_err(|e| (map_msi_error(&e), e.to_string()))?;

            let len = bytes.len();
            let ptr = Box::into_raw(bytes.into_boxed_slice()).cast::<u8>();

            *out_bytes = ptr;
            *out_len = len;

            Ok(MSI_SUCCESS)
        })
    }
}

/// Opens an existing `.msi` file from disk.
///
/// # Arguments
///
/// * `path` - Path to the `.msi` file.
/// * `out_package` - Pointer receiving the parsed package handle.
///
/// # Returns
///
/// [`MSI_SUCCESS`] on success, or an error code.
///
/// # Safety
///
/// `path` must be a valid null-terminated C string.
/// `out_package` must point to valid writable memory.
#[no_mangle]
pub unsafe extern "C" fn msi_package_open(
    path: *const c_char,
    out_package: *mut *mut MsiPackageHandle,
) -> i32 {
    unsafe {
        ffi_boundary(|| {
            if out_package.is_null() {
                return Err((
                    MSI_ERROR_NULL_POINTER,
                    "out_package pointer must not be null".to_string(),
                ));
            }
            let path_str = c_str_to_str(path, "path")?;
            let pkg = Package::open(path_str).map_err(|e| (map_msi_error(&e), e.to_string()))?;

            *out_package = Box::into_raw(Box::new(MsiPackageHandle { inner: pkg }));
            Ok(MSI_SUCCESS)
        })
    }
}

/// Reads a property value from the package.
///
/// If `buffer` is NULL or `capacity` is too small, writes the total required length
/// (including null terminator) to `*out_written` and returns [`MSI_ERROR_BUFFER_TOO_SMALL`].
///
/// # Arguments
///
/// * `package` - Package handle.
/// * `property_name` - Name of the property to query.
/// * `buffer` - Destination character buffer (caller allocated).
/// * `capacity` - Capacity of `buffer` in bytes.
/// * `out_written` - Optional pointer receiving the number of bytes written.
///
/// # Returns
///
/// [`MSI_SUCCESS`] on success, or an error code.
///
/// # Safety
///
/// `package` must be a valid pointer.
/// `property_name` must be a valid null-terminated C string.
/// `buffer` and `out_written` must point to valid memory if non-null.
#[no_mangle]
pub unsafe extern "C" fn msi_package_get_property(
    package: *const MsiPackageHandle,
    property_name: *const c_char,
    buffer: *mut c_char,
    capacity: usize,
    out_written: *mut usize,
) -> i32 {
    unsafe {
        ffi_boundary(|| {
            if package.is_null() {
                return Err((
                    MSI_ERROR_NULL_POINTER,
                    "package handle must not be null".to_string(),
                ));
            }

            let prop_name = c_str_to_str(property_name, "property_name")?;

            let val_opt = match prop_name {
                "ProductName" => Some((*package).inner.metadata().product_name().to_string()),
                "Manufacturer" => Some((*package).inner.metadata().manufacturer().to_string()),
                "ProductVersion" => Some((*package).inner.metadata().version().to_string()),
                "ProductCode" => Some((*package).inner.metadata().product_code().to_string()),
                other => (*package)
                    .inner
                    .database()
                    .get_records("Property")
                    .iter()
                    .find_map(|rec| match (rec.get(0), rec.get(1)) {
                        (Some(FieldValue::String(k)), Some(FieldValue::String(v)))
                            if k == other =>
                        {
                            Some(v.clone())
                        }
                        _ => None,
                    }),
            };

            let Some(val) = val_opt else {
                return Err((
                    MSI_ERROR_INVALID_ARGUMENT,
                    format!("Property '{prop_name}' not found in package"),
                ));
            };

            let val_bytes = val.as_bytes();
            let needed_len = val_bytes.len().saturating_add(1);

            if !out_written.is_null() {
                *out_written = needed_len;
            }

            if buffer.is_null() || capacity < needed_len {
                return Ok(MSI_ERROR_BUFFER_TOO_SMALL);
            }

            ptr::copy_nonoverlapping(val_bytes.as_ptr(), buffer.cast::<u8>(), val_bytes.len());
            *buffer.add(val_bytes.len()) = 0;

            Ok(MSI_SUCCESS)
        })
    }
}

/// Extracts all files from an embedded cabinet stream inside the package to a local directory.
///
/// # Arguments
///
/// * `package` - Package handle.
/// * `cabinet_name` - Cabinet stream identifier (e.g. "#cab1.cab").
/// * `dest_dir` - Destination directory path.
///
/// # Returns
///
/// [`MSI_SUCCESS`] on success, or an error code.
///
/// # Safety
///
/// `package` must be a valid pointer.
/// `cabinet_name` and `dest_dir` must be valid null-terminated C strings.
#[no_mangle]
pub unsafe extern "C" fn msi_package_extract_cabinet(
    package: *const MsiPackageHandle,
    cabinet_name: *const c_char,
    dest_dir: *const c_char,
) -> i32 {
    unsafe {
        ffi_boundary(|| {
            if package.is_null() {
                return Err((
                    MSI_ERROR_NULL_POINTER,
                    "package handle must not be null".to_string(),
                ));
            }

            let cab_name = c_str_to_str(cabinet_name, "cabinet_name")?;
            let destination = c_str_to_str(dest_dir, "dest_dir")?;

            let Some(cab_data) = (*package).inner.get_embedded_cabinet(cab_name) else {
                return Err((
                    MSI_ERROR_CABINET,
                    format!("Cabinet stream '{cab_name}' not found in package"),
                ));
            };

            let reader =
                CabinetReader::new(cab_data).map_err(|e| (map_msi_error(&e), e.to_string()))?;

            fs::create_dir_all(destination).map_err(|e| {
                (
                    map_msi_error(&msi::Error::Io(e.to_string())),
                    format!("Failed creating destination directory '{destination}'"),
                )
            })?;

            for file in reader.files() {
                let file_data = reader
                    .extract_file(&file.filename)
                    .map_err(|e| (map_msi_error(&e), e.to_string()))?;

                let file_path = Path::new(destination).join(&file.filename);
                fs::write(&file_path, file_data).map_err(|e| {
                    (
                        map_msi_error(&msi::Error::Io(e.to_string())),
                        format!("Failed writing extracted file to '{}'", file_path.display()),
                    )
                })?;
            }

            Ok(MSI_SUCCESS)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::builder::*;
    use crate::types::{
        msi_buffer_free, msi_package_builder_destroy, msi_package_destroy, MsiPackageBuilderHandle,
    };
    use std::ffi::{CStr, CString};

    #[test]
    fn test_package_null_checks() {
        let dummy_c = CString::new("test").unwrap_or_default();
        let meta = msi::package::PackageMetadata::new(
            "Test",
            "Mfr",
            msi::package::ProductVersion::new(1, 0, 0),
            "{00000000-0000-0000-0000-000000000000}",
        );
        let pkg_handle = Box::into_raw(Box::new(MsiPackageHandle {
            inner: Package::new(
                meta,
                msi::wix::linker::LinkedDatabase::default(),
                msi::database::summary_info::SummaryInfo::default(),
                std::collections::HashMap::new(),
            ),
        }));

        // SAFETY: Testing null pointer validation across all package FFI entry points.
        unsafe {
            // Null package handle
            assert_eq!(
                msi_package_save(ptr::null(), ptr::null()),
                MSI_ERROR_NULL_POINTER
            );
            assert_eq!(
                msi_package_to_bytes(ptr::null(), ptr::null_mut(), ptr::null_mut()),
                MSI_ERROR_NULL_POINTER
            );
            assert_eq!(
                msi_package_open(ptr::null(), ptr::null_mut()),
                MSI_ERROR_NULL_POINTER
            );
            assert_eq!(
                msi_package_get_property(
                    ptr::null(),
                    ptr::null(),
                    ptr::null_mut(),
                    0,
                    ptr::null_mut()
                ),
                MSI_ERROR_NULL_POINTER
            );
            assert_eq!(
                msi_package_extract_cabinet(ptr::null(), ptr::null(), ptr::null()),
                MSI_ERROR_NULL_POINTER
            );

            // Valid package handle with null arguments
            assert_eq!(
                msi_package_save(pkg_handle, ptr::null()),
                MSI_ERROR_NULL_POINTER
            );
            assert_eq!(
                msi_package_to_bytes(pkg_handle, ptr::null_mut(), ptr::null_mut()),
                MSI_ERROR_NULL_POINTER
            );
            let mut dummy_bytes_ptr: *mut u8 = ptr::null_mut();
            assert_eq!(
                msi_package_to_bytes(pkg_handle, &raw mut dummy_bytes_ptr, ptr::null_mut()),
                MSI_ERROR_NULL_POINTER
            );
            let mut dummy_len = 0usize;
            assert_eq!(
                msi_package_to_bytes(pkg_handle, ptr::null_mut(), &raw mut dummy_len),
                MSI_ERROR_NULL_POINTER
            );
            assert_eq!(
                msi_package_open(dummy_c.as_ptr(), ptr::null_mut()),
                MSI_ERROR_NULL_POINTER
            );
            let mut out_pkg_dummy: *mut MsiPackageHandle = ptr::null_mut();
            assert_eq!(
                msi_package_open(ptr::null(), &raw mut out_pkg_dummy),
                MSI_ERROR_NULL_POINTER
            );
            assert_eq!(
                msi_package_get_property(
                    pkg_handle,
                    ptr::null(),
                    ptr::null_mut(),
                    0,
                    ptr::null_mut()
                ),
                MSI_ERROR_NULL_POINTER
            );
            assert_eq!(
                msi_package_extract_cabinet(pkg_handle, ptr::null(), ptr::null()),
                MSI_ERROR_NULL_POINTER
            );
            assert_eq!(
                msi_package_extract_cabinet(pkg_handle, dummy_c.as_ptr(), ptr::null()),
                MSI_ERROR_NULL_POINTER
            );

            msi_package_destroy(pkg_handle);
        }
    }

    #[test]
    #[allow(clippy::too_many_lines)]
    fn test_package_roundtrip_save_and_open() {
        let name = CString::new("PkgTest").unwrap_or_default();
        let mfr = CString::new("PkgAcme").unwrap_or_default();
        let code = CString::new("{12345678-1234-1234-1234-1234567890AB}").unwrap_or_default();

        let mut builder: *mut MsiPackageBuilderHandle = ptr::null_mut();
        // SAFETY: Builder is created, modified, and built according to C-ABI protocol.
        unsafe {
            let create_res = msi_package_builder_create(
                name.as_ptr(),
                mfr.as_ptr(),
                1,
                0,
                0,
                code.as_ptr(),
                &raw mut builder,
            );
            assert_eq!(create_res, MSI_SUCCESS);

            // Add Custom Property
            let custom_key = CString::new("CustomKey").unwrap_or_default();
            let custom_val = CString::new("CustomValue").unwrap_or_default();
            assert_eq!(
                msi_package_builder_add_property(builder, custom_key.as_ptr(), custom_val.as_ptr()),
                MSI_SUCCESS
            );

            // Add embedded dummy cabinet to test extraction failure
            let bad_cab_name = CString::new("#bad.cab").unwrap_or_default();
            let corrupt_data = [0x00_u8, 0x01, 0x02, 0x03];
            assert_eq!(
                msi_package_builder_add_embedded_cabinet(
                    builder,
                    bad_cab_name.as_ptr(),
                    corrupt_data.as_ptr(),
                    corrupt_data.len()
                ),
                MSI_SUCCESS
            );

            let mut pkg: *mut MsiPackageHandle = ptr::null_mut();
            assert_eq!(
                msi_package_builder_build(builder, &raw mut pkg),
                MSI_SUCCESS
            );
            msi_package_builder_destroy(builder);

            // Query standard properties
            for (p_name, expected) in [
                ("ProductName", "PkgTest"),
                ("Manufacturer", "PkgAcme"),
                ("ProductVersion", "1.0.0"),
                ("ProductCode", "{12345678-1234-1234-1234-1234567890AB}"),
                ("CustomKey", "CustomValue"),
            ] {
                let p_c = CString::new(p_name).unwrap_or_default();
                let mut out_len = 0;
                let small_res = msi_package_get_property(
                    pkg,
                    p_c.as_ptr(),
                    ptr::null_mut(),
                    0,
                    &raw mut out_len,
                );
                assert_eq!(small_res, MSI_ERROR_BUFFER_TOO_SMALL);
                assert_eq!(out_len, expected.len() + 1);

                // Test non-null buffer with capacity too small
                let mut tiny_buf = [0_i8; 2];
                assert_eq!(
                    msi_package_get_property(
                        pkg,
                        p_c.as_ptr(),
                        tiny_buf.as_mut_ptr(),
                        tiny_buf.len(),
                        &raw mut out_len
                    ),
                    MSI_ERROR_BUFFER_TOO_SMALL
                );

                let mut buf = vec![0_i8; out_len];
                assert_eq!(
                    msi_package_get_property(
                        pkg,
                        p_c.as_ptr(),
                        buf.as_mut_ptr(),
                        buf.len(),
                        &raw mut out_len
                    ),
                    MSI_SUCCESS
                );
                let read = CStr::from_ptr(buf.as_ptr()).to_str().unwrap_or("");
                assert_eq!(read, expected);

                // Query with null out_written
                assert_eq!(
                    msi_package_get_property(
                        pkg,
                        p_c.as_ptr(),
                        buf.as_mut_ptr(),
                        buf.len(),
                        ptr::null_mut()
                    ),
                    MSI_SUCCESS
                );
            }

            // Query non-existent property
            let non_exist = CString::new("NonExistentProperty").unwrap_or_default();
            assert_eq!(
                msi_package_get_property(
                    pkg,
                    non_exist.as_ptr(),
                    ptr::null_mut(),
                    0,
                    ptr::null_mut()
                ),
                MSI_ERROR_INVALID_ARGUMENT
            );

            // Convert to bytes
            let mut out_bytes: *mut u8 = ptr::null_mut();
            let mut out_byte_len = 0;
            assert_eq!(
                msi_package_to_bytes(pkg, &raw mut out_bytes, &raw mut out_byte_len),
                MSI_SUCCESS
            );
            assert!(!out_bytes.is_null());
            assert!(out_byte_len > 0);
            msi_buffer_free(out_bytes, out_byte_len);

            // Save to invalid path
            let bad_save_path =
                CString::new("/nonexistent_dir_msi_package_test/file.msi").unwrap_or_default();
            assert_ne!(msi_package_save(pkg, bad_save_path.as_ptr()), MSI_SUCCESS);

            // Open nonexistent file
            let bad_open_path =
                CString::new("/nonexistent_dir_msi_package_test/nofile.msi").unwrap_or_default();
            let mut dummy_open: *mut MsiPackageHandle = ptr::null_mut();
            assert_ne!(
                msi_package_open(bad_open_path.as_ptr(), &raw mut dummy_open),
                MSI_SUCCESS
            );

            // Extract cabinet errors
            let missing_cab = CString::new("#missing.cab").unwrap_or_default();
            let temp_dest = std::env::temp_dir().join("cab_dest");
            let temp_dest_c = CString::new(temp_dest.to_str().unwrap_or("")).unwrap_or_default();
            assert_eq!(
                msi_package_extract_cabinet(pkg, missing_cab.as_ptr(), temp_dest_c.as_ptr()),
                MSI_ERROR_CABINET
            );

            // Corrupt cabinet extraction
            assert_eq!(
                msi_package_extract_cabinet(pkg, bad_cab_name.as_ptr(), temp_dest_c.as_ptr()),
                MSI_ERROR_CABINET
            );

            // Save and reopen successfully
            let temp_msi = std::env::temp_dir().join("test_save_open.msi");
            let temp_path_c = CString::new(temp_msi.to_str().unwrap_or("")).unwrap_or_default();
            assert_eq!(msi_package_save(pkg, temp_path_c.as_ptr()), MSI_SUCCESS);

            let mut loaded_pkg: *mut MsiPackageHandle = ptr::null_mut();
            assert_eq!(
                msi_package_open(temp_path_c.as_ptr(), &raw mut loaded_pkg),
                MSI_SUCCESS
            );
            assert!(!loaded_pkg.is_null());

            msi_package_destroy(pkg);
            msi_package_destroy(loaded_pkg);
            let _ = fs::remove_file(temp_msi);
        }
    }

    #[test]
    #[allow(clippy::too_many_lines)]
    fn test_package_extract_cabinet_io_errors() {
        let temp_dir = std::env::temp_dir().join("msi_ffi_extract_io_test");
        let _ = fs::create_dir_all(&temp_dir);

        let src_file = temp_dir.join("input.txt");
        let _ = fs::write(&src_file, b"content");

        let name = CString::new("CabApp").unwrap_or_default();
        let mfr = CString::new("CabMfr").unwrap_or_default();
        let code = CString::new("{11111111-2222-3333-4444-555555555555}").unwrap_or_default();
        let cab_name = CString::new("#cab1.cab").unwrap_or_default();

        let mut builder: *mut MsiPackageBuilderHandle = ptr::null_mut();
        // SAFETY: Standard C-ABI calls to prepare an MSI with an embedded packed cabinet.
        unsafe {
            assert_eq!(
                msi_package_builder_create(
                    name.as_ptr(),
                    mfr.as_ptr(),
                    1,
                    0,
                    0,
                    code.as_ptr(),
                    &raw mut builder
                ),
                MSI_SUCCESS
            );

            let s1 = CString::new(src_file.to_str().unwrap_or("")).unwrap_or_default();
            let fid = CString::new("File1").unwrap_or_default();
            let cid = CString::new("Comp1").unwrap_or_default();
            let sources = [s1.as_ptr()];
            let files = [fid.as_ptr()];
            let comps = [cid.as_ptr()];

            assert_eq!(
                msi_package_builder_pack_files_from_disk(
                    builder,
                    sources.as_ptr(),
                    files.as_ptr(),
                    comps.as_ptr(),
                    1,
                    1,
                    cab_name.as_ptr()
                ),
                MSI_SUCCESS
            );

            let mut pkg: *mut MsiPackageHandle = ptr::null_mut();
            assert_eq!(
                msi_package_builder_build(builder, &raw mut pkg),
                MSI_SUCCESS
            );
            msi_package_builder_destroy(builder);

            // 1. Dest dir cannot be created (a regular file occupies the path)
            let conflict_file = temp_dir.join("conflict_file");
            let _ = fs::write(&conflict_file, b"occupied");
            let invalid_dest = conflict_file.join("sub_path");
            let invalid_dest_c =
                CString::new(invalid_dest.to_str().unwrap_or("")).unwrap_or_default();
            assert_ne!(
                msi_package_extract_cabinet(pkg, cab_name.as_ptr(), invalid_dest_c.as_ptr()),
                MSI_SUCCESS
            );

            // 2. Extracted file path is a directory (writing fails)
            let extract_dir = temp_dir.join("extract_dir");
            let target_conflict = extract_dir.join("File1");
            let _ = fs::create_dir_all(&target_conflict);
            let extract_dir_c =
                CString::new(extract_dir.to_str().unwrap_or("")).unwrap_or_default();
            assert_ne!(
                msi_package_extract_cabinet(pkg, cab_name.as_ptr(), extract_dir_c.as_ptr()),
                MSI_SUCCESS
            );

            msi_package_destroy(pkg);

            // 3. Cabinet with corrupted data block causes extract_file to fail
            let mut cab_writer =
                msi::cab::writer::CabinetWriter::new(msi::cab::folder::CompressionType::Mszip);
            let _ = cab_writer.add_file("corrupt.txt", b"Valid payload data to corrupt");
            let mut corrupt_cab_bytes = cab_writer.build();
            let last_idx = corrupt_cab_bytes.len().saturating_sub(1);
            corrupt_cab_bytes[last_idx] ^= 0xFF;

            let mut builder2: *mut MsiPackageBuilderHandle = ptr::null_mut();
            assert_eq!(
                msi_package_builder_create(
                    name.as_ptr(),
                    mfr.as_ptr(),
                    1,
                    0,
                    0,
                    code.as_ptr(),
                    &raw mut builder2
                ),
                MSI_SUCCESS
            );
            let corrupt_cab_name = CString::new("#corrupt.cab").unwrap_or_default();
            assert_eq!(
                msi_package_builder_add_embedded_cabinet(
                    builder2,
                    corrupt_cab_name.as_ptr(),
                    corrupt_cab_bytes.as_ptr(),
                    corrupt_cab_bytes.len()
                ),
                MSI_SUCCESS
            );
            let mut pkg_corrupt: *mut MsiPackageHandle = ptr::null_mut();
            assert_eq!(
                msi_package_builder_build(builder2, &raw mut pkg_corrupt),
                MSI_SUCCESS
            );
            msi_package_builder_destroy(builder2);

            let valid_dest_dir = temp_dir.join("valid_dest_corrupt_test");
            let valid_dest_c =
                CString::new(valid_dest_dir.to_str().unwrap_or("")).unwrap_or_default();
            assert_ne!(
                msi_package_extract_cabinet(
                    pkg_corrupt,
                    corrupt_cab_name.as_ptr(),
                    valid_dest_c.as_ptr()
                ),
                MSI_SUCCESS
            );
            msi_package_destroy(pkg_corrupt);

            // 4. Test msi_package_to_bytes failure via duplicate stream collision
            let mut coll_cabinets = std::collections::HashMap::new();
            coll_cabinets.insert("\u{0005}SummaryInformation".to_string(), vec![0u8; 8]);
            let meta_coll = msi::package::PackageMetadata::new(
                "CollApp",
                "CollMfr",
                msi::package::ProductVersion::new(1, 0, 0),
                "{11111111-2222-3333-4444-555555555555}",
            );
            let coll_pkg_handle = Box::into_raw(Box::new(MsiPackageHandle {
                inner: Package::new(
                    meta_coll,
                    msi::wix::linker::LinkedDatabase::default(),
                    msi::database::summary_info::SummaryInfo::default(),
                    coll_cabinets,
                ),
            }));
            let mut out_bytes: *mut u8 = ptr::null_mut();
            let mut out_len = 0usize;
            assert_ne!(
                msi_package_to_bytes(coll_pkg_handle, &raw mut out_bytes, &raw mut out_len),
                MSI_SUCCESS
            );
            msi_package_destroy(coll_pkg_handle);
        }

        let _ = fs::remove_dir_all(temp_dir);
    }
}

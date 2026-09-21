//! Integration tests for `msi-ffi` verifying full lifecycle of C-ABI functions.

use crate::builder::*;
use crate::error::*;
use crate::package::*;
use crate::types::*;
use std::ffi::{CStr, CString};
use std::fs;
use std::ptr;

/// Tests the complete roundtrip package creation, authoring, disk packing, serialization, extraction, and inspection.
#[test]
#[allow(clippy::too_many_lines)]
fn test_integration_full_package_build_extract_roundtrip() {
    let temp_dir = std::env::temp_dir().join("msi_ffi_integration_test");
    let _ = fs::create_dir_all(&temp_dir);

    let src1_path = temp_dir.join("sample1.txt");
    let src2_path = temp_dir.join("sample2.bin");
    fs::write(&src1_path, b"Sample Text Content").unwrap_or_default();
    fs::write(&src2_path, [0xDE, 0xAD, 0xBE, 0xEF, 0x00, 0x01]).unwrap_or_default();

    let name = CString::new("IntegrationApp").unwrap_or_default();
    let mfr = CString::new("IntegrationVendor").unwrap_or_default();
    let code = CString::new("{AAAAAAAA-1111-2222-3333-444444444444}").unwrap_or_default();
    let upg = CString::new("{BBBBBBBB-5555-6666-7777-888888888888}").unwrap_or_default();

    let mut builder: *mut MsiPackageBuilderHandle = ptr::null_mut();

    // SAFETY: Valid C strings and valid out_pointer passed to C-ABI functions.
    unsafe {
        let res = msi_package_builder_create(
            name.as_ptr(),
            mfr.as_ptr(),
            2,
            5,
            10,
            code.as_ptr(),
            &raw mut builder,
        );
        assert_eq!(res, MSI_SUCCESS);
        assert!(!builder.is_null());

        assert_eq!(
            msi_package_builder_set_upgrade_code(builder, upg.as_ptr()),
            MSI_SUCCESS
        );

        // Directories
        let target_dir = CString::new("TARGETDIR").unwrap_or_default();
        let source_dir = CString::new("SourceDir").unwrap_or_default();
        assert_eq!(
            msi_package_builder_add_directory(
                builder,
                target_dir.as_ptr(),
                ptr::null(),
                source_dir.as_ptr()
            ),
            MSI_SUCCESS
        );

        let pfiles_dir = CString::new("ProgramFilesFolder").unwrap_or_default();
        let pfiles_def = CString::new("PFiles|Program Files").unwrap_or_default();
        assert_eq!(
            msi_package_builder_add_directory(
                builder,
                pfiles_dir.as_ptr(),
                target_dir.as_ptr(),
                pfiles_def.as_ptr()
            ),
            MSI_SUCCESS
        );

        let install_dir = CString::new("INSTALLDIR").unwrap_or_default();
        let app_def = CString::new("AppDir|IntegrationApp").unwrap_or_default();
        assert_eq!(
            msi_package_builder_add_directory(
                builder,
                install_dir.as_ptr(),
                pfiles_dir.as_ptr(),
                app_def.as_ptr()
            ),
            MSI_SUCCESS
        );

        // Components
        let comp1 = CString::new("MainComponent").unwrap_or_default();
        let comp1_guid = CString::new("{CCCCCCCC-9999-0000-1111-222222222222}").unwrap_or_default();
        assert_eq!(
            msi_package_builder_add_component(
                builder,
                comp1.as_ptr(),
                comp1_guid.as_ptr(),
                install_dir.as_ptr(),
                0,
                ptr::null(),
                ptr::null()
            ),
            MSI_SUCCESS
        );

        // Features
        let feat = CString::new("Complete").unwrap_or_default();
        let feat_title = CString::new("Complete Application").unwrap_or_default();
        let feat_desc = CString::new("Installs all application binaries").unwrap_or_default();
        assert_eq!(
            msi_package_builder_add_feature(
                builder,
                feat.as_ptr(),
                ptr::null(),
                feat_title.as_ptr(),
                feat_desc.as_ptr(),
                1,
                1,
                install_dir.as_ptr(),
                0
            ),
            MSI_SUCCESS
        );
        assert_eq!(
            msi_package_builder_add_feature_component(builder, feat.as_ptr(), comp1.as_ptr()),
            MSI_SUCCESS
        );

        // Automated disk packing
        let s1 = CString::new(src1_path.to_str().unwrap_or("")).unwrap_or_default();
        let s2 = CString::new(src2_path.to_str().unwrap_or("")).unwrap_or_default();
        let fid1 = CString::new("FileItem1").unwrap_or_default();
        let fid2 = CString::new("FileItem2").unwrap_or_default();

        let sources = [s1.as_ptr(), s2.as_ptr()];
        let file_id_slice = [fid1.as_ptr(), fid2.as_ptr()];
        let comp_id_slice = [comp1.as_ptr(), comp1.as_ptr()];
        let cab_stream_name = CString::new("#cab1.cab").unwrap_or_default();

        let pack_res = msi_package_builder_pack_files_from_disk(
            builder,
            sources.as_ptr(),
            file_id_slice.as_ptr(),
            comp_id_slice.as_ptr(),
            2,
            1, // MSZIP
            cab_stream_name.as_ptr(),
        );
        assert_eq!(pack_res, MSI_SUCCESS);

        // Build
        let mut pkg: *mut MsiPackageHandle = ptr::null_mut();
        assert_eq!(
            msi_package_builder_build(builder, &raw mut pkg),
            MSI_SUCCESS
        );
        assert!(!pkg.is_null());
        msi_package_builder_destroy(builder);

        // Save
        let out_msi = temp_dir.join("integration.msi");
        let out_c = CString::new(out_msi.to_str().unwrap_or("")).unwrap_or_default();
        assert_eq!(msi_package_save(pkg, out_c.as_ptr()), MSI_SUCCESS);

        // Extract cabinet files to a new directory
        let extract_dir = temp_dir.join("extracted_output");
        let extract_c = CString::new(extract_dir.to_str().unwrap_or("")).unwrap_or_default();
        assert_eq!(
            msi_package_extract_cabinet(pkg, cab_stream_name.as_ptr(), extract_c.as_ptr()),
            MSI_SUCCESS
        );

        let extracted1 = extract_dir.join("FileItem1");
        assert!(extracted1.exists());
        let content1 = fs::read(extracted1).unwrap_or_default();
        assert_eq!(content1, b"Sample Text Content");

        let extracted2 = extract_dir.join("FileItem2");
        assert!(extracted2.exists());
        let content2 = fs::read(extracted2).unwrap_or_default();
        assert_eq!(content2, &[0xDE, 0xAD, 0xBE, 0xEF, 0x00, 0x01]);

        // Re-open from disk and read properties
        let mut reopened_pkg: *mut MsiPackageHandle = ptr::null_mut();
        assert_eq!(
            msi_package_open(out_c.as_ptr(), &raw mut reopened_pkg),
            MSI_SUCCESS
        );
        assert!(!reopened_pkg.is_null());

        let prop_name = CString::new("ProductName").unwrap_or_default();
        let mut prop_len = 0;
        let mut prop_buf = vec![0_i8; 64];
        assert_eq!(
            msi_package_get_property(
                reopened_pkg,
                prop_name.as_ptr(),
                prop_buf.as_mut_ptr(),
                prop_buf.len(),
                &raw mut prop_len
            ),
            MSI_SUCCESS
        );
        let read_product = CStr::from_ptr(prop_buf.as_ptr()).to_str().unwrap_or("");
        assert_eq!(read_product, "IntegrationApp");

        msi_package_destroy(pkg);
        msi_package_destroy(reopened_pkg);
    }

    let _ = fs::remove_dir_all(temp_dir);
}

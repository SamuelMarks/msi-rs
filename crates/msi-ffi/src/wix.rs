//! C-ABI functions for `WiX` toolset source compilation and linking.
//!
//! Compiles `WiX` XML source (`.wxs`) into installable Windows Installer (`.msi`) packages.

use crate::error::{c_str_to_str, ffi_boundary, map_msi_error, MSI_SUCCESS};
use msi::database::summary_info::SummaryInfo;
use msi::database::tables::record::FieldValue;
use msi::package::{Package, PackageMetadata, ProductVersion};
use msi::wix::linker::Linker;
use msi::wix::preprocessor::PreprocessorContext;
use std::collections::HashMap;
use std::ffi::c_char;
use std::fs;

/// Compiles a `WiX` XML source string directly to an `.msi` file.
///
/// # Arguments
///
/// * `wxs_content` - Raw `WiX` XML source string (`.wxs`).
/// * `output_msi_path` - Target output `.msi` file path on disk.
///
/// # Returns
///
/// [`MSI_SUCCESS`] on success, or an error code.
///
/// # Safety
///
/// `wxs_content` and `output_msi_path` must be valid null-terminated C strings.
#[no_mangle]
pub unsafe extern "C" fn msi_compile_wix_source(
    wxs_content: *const c_char,
    output_msi_path: *const c_char,
) -> i32 {
    unsafe {
        ffi_boundary(|| {
            let source = c_str_to_str(wxs_content, "wxs_content")?;
            let out_path = c_str_to_str(output_msi_path, "output_msi_path")?;

            let mut ctx = PreprocessorContext::new();
            let obj = msi::wix::compile_wix(source, &mut ctx)
                .map_err(|e| (map_msi_error(&e), e.to_string()))?;

            let mut linker = Linker::new();
            linker.add_object(obj);
            let db = linker
                .link()
                .map_err(|e| (map_msi_error(&e), e.to_string()))?;

            let find_prop = |name: &str| {
                db.get_records("Property")
                    .iter()
                    .find_map(|r| match (r.get(0), r.get(1)) {
                        (Some(FieldValue::String(k)), Some(FieldValue::String(v))) if k == name => {
                            Some(v.clone())
                        }
                        _ => None,
                    })
            };

            let product_name =
                find_prop("ProductName").unwrap_or_else(|| "WiX Application".to_string());
            let manufacturer =
                find_prop("Manufacturer").unwrap_or_else(|| "WiX Author".to_string());
            let product_code = find_prop("ProductCode")
                .unwrap_or_else(|| "{00000000-0000-0000-0000-000000000000}".to_string());
            let version = find_prop("ProductVersion")
                .and_then(|v| ProductVersion::parse(&v).ok())
                .unwrap_or_else(|| ProductVersion::new(1, 0, 0));

            let metadata = PackageMetadata::new(product_name, manufacturer, version, product_code);
            let summary_info = SummaryInfo::default();
            let embedded_cabinets = HashMap::new();
            let package = Package::new(metadata, db, summary_info, embedded_cabinets);

            package
                .save(out_path)
                .map_err(|e| (map_msi_error(&e), e.to_string()))?;

            Ok(MSI_SUCCESS)
        })
    }
}

/// Compiles a `WiX` XML source file directly to an `.msi` file.
///
/// # Arguments
///
/// * `wxs_path` - Path to the `.wxs` source file on disk.
/// * `output_msi_path` - Target output `.msi` file path on disk.
///
/// # Returns
///
/// [`MSI_SUCCESS`] on success, or an error code.
///
/// # Safety
///
/// `wxs_path` and `output_msi_path` must be valid null-terminated C strings.
#[no_mangle]
pub unsafe extern "C" fn msi_compile_wix_file(
    wxs_path: *const c_char,
    output_msi_path: *const c_char,
) -> i32 {
    unsafe {
        ffi_boundary(|| {
            let in_path = c_str_to_str(wxs_path, "wxs_path")?;
            let source = fs::read_to_string(in_path).map_err(|e| {
                (
                    map_msi_error(&msi::Error::Io(e.to_string())),
                    format!("Failed reading WiX source file '{in_path}'"),
                )
            })?;

            let source_c = std::ffi::CString::new(source).map_err(|e| {
                (
                    crate::error::MSI_ERROR_INVALID_ARGUMENT,
                    format!("WiX source contains interior null: {e}"),
                )
            })?;

            let res = msi_compile_wix_source(source_c.as_ptr(), output_msi_path);
            if res == MSI_SUCCESS {
                Ok(MSI_SUCCESS)
            } else {
                Err((res, "WiX source compilation failed".to_string()))
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::{MSI_ERROR_INVALID_ARGUMENT, MSI_ERROR_IO, MSI_ERROR_NULL_POINTER};
    use std::ffi::CString;
    use std::ptr;

    #[test]
    fn test_wix_null_checks() {
        let valid_wxs = CString::new("<Wix />").unwrap_or_default();
        let temp_dir = std::env::temp_dir().join("msi_ffi_wix_null_test");
        let _ = fs::create_dir_all(&temp_dir);
        let temp_wxs = temp_dir.join("null_test.wxs");
        let _ = fs::write(&temp_wxs, "<Wix />");
        let temp_wxs_c = CString::new(temp_wxs.to_str().unwrap_or("")).unwrap_or_default();

        // SAFETY: Passing null pointers to C-ABI functions to verify null safety error handling.
        unsafe {
            assert_eq!(
                msi_compile_wix_source(ptr::null(), ptr::null()),
                MSI_ERROR_NULL_POINTER
            );
            assert_eq!(
                msi_compile_wix_source(valid_wxs.as_ptr(), ptr::null()),
                MSI_ERROR_NULL_POINTER
            );
            assert_eq!(
                msi_compile_wix_file(ptr::null(), ptr::null()),
                MSI_ERROR_NULL_POINTER
            );
            assert_eq!(
                msi_compile_wix_file(temp_wxs_c.as_ptr(), ptr::null()),
                MSI_ERROR_NULL_POINTER
            );
        }

        let _ = fs::remove_dir_all(temp_dir);
    }

    #[test]
    fn test_compile_wix_source_and_file_end_to_end() {
        let wxs = r#"
<Wix xmlns="http://schemas.microsoft.com/wix/2006/wi">
    <Product Id="{12345678-1234-1234-1234-1234567890AB}" Name="WixTestApp" Version="1.2.3" Manufacturer="Acme">
        <Package Description="WiX FFI Test" />
        <Directory Id="TARGETDIR" Name="SourceDir">
            <Component Id="TestComp">
                <File Id="TestFile" Source="test.txt" />
            </Component>
        </Directory>
        <Feature Id="MainFeature" Title="Main" Level="1">
            <ComponentRef Id="TestComp" />
        </Feature>
    </Product>
</Wix>
"#;
        let temp_dir = std::env::temp_dir().join("msi_ffi_wix_test_end_to_end");
        let _ = fs::create_dir_all(&temp_dir);
        let out_msi1 = temp_dir.join("wix1.msi");
        let out_msi2 = temp_dir.join("wix2.msi");
        let in_wxs = temp_dir.join("input.wxs");
        let _ = fs::write(&in_wxs, wxs);

        let wxs_c = CString::new(wxs).unwrap_or_default();
        let out1_c = CString::new(out_msi1.to_str().unwrap_or("")).unwrap_or_default();
        let in_wxs_c = CString::new(in_wxs.to_str().unwrap_or("")).unwrap_or_default();
        let out2_c = CString::new(out_msi2.to_str().unwrap_or("")).unwrap_or_default();

        // SAFETY: Pointers are valid CStrings with valid output paths.
        unsafe {
            let res = msi_compile_wix_source(wxs_c.as_ptr(), out1_c.as_ptr());
            assert_eq!(res, MSI_SUCCESS);
            assert!(out_msi1.exists());

            let res2 = msi_compile_wix_file(in_wxs_c.as_ptr(), out2_c.as_ptr());
            assert_eq!(res2, MSI_SUCCESS);
            assert!(out_msi2.exists());
        }

        let _ = fs::remove_dir_all(temp_dir);
    }

    #[test]
    fn test_compile_wix_invalid_source_and_save_failure() {
        let temp_dir = std::env::temp_dir().join("msi_ffi_wix_test_failures");
        let _ = fs::create_dir_all(&temp_dir);
        let out_msi = temp_dir.join("wix_fail.msi");
        let out_c = CString::new(out_msi.to_str().unwrap_or("")).unwrap_or_default();

        // 1. Invalid XML syntax
        let invalid_xml = CString::new("<Wix>Unclosed tag").unwrap_or_default();
        // SAFETY: Pointer is a valid null-terminated C string.
        unsafe {
            let res = msi_compile_wix_source(invalid_xml.as_ptr(), out_c.as_ptr());
            assert_ne!(res, MSI_SUCCESS);
        }

        // 2. Linker failure (Fragment without Product/Module entry point)
        let fragment_wxs = r#"
<Wix xmlns="http://schemas.microsoft.com/wix/2006/wi">
    <Fragment>
        <Component Id="OrphanComp" Directory="TARGETDIR" Guid="{11111111-1111-1111-1111-111111111111}" />
    </Fragment>
</Wix>
"#;
        let frag_c = CString::new(fragment_wxs).unwrap_or_default();
        // SAFETY: Pointer is a valid null-terminated C string.
        unsafe {
            let res = msi_compile_wix_source(frag_c.as_ptr(), out_c.as_ptr());
            assert_ne!(res, MSI_SUCCESS);
        }

        // 3. Package save failure due to invalid path
        let valid_wxs = r#"
<Wix xmlns="http://schemas.microsoft.com/wix/2006/wi">
    <Product Id="{12345678-1234-1234-1234-1234567890AB}" Name="SaveFailApp" Version="invalid.version" Manufacturer="Acme">
        <Package Description="Save Failure Test" />
        <Property Id="CustomProp" Value="CustomVal" />
        <Directory Id="TARGETDIR" Name="SourceDir" />
    </Product>
</Wix>
"#;
        let valid_c = CString::new(valid_wxs).unwrap_or_default();
        let invalid_out_c =
            CString::new("/nonexistent_dir_msi_save_test/cannot/create.msi").unwrap_or_default();
        // SAFETY: Valid C strings passed; target directory does not exist causing save failure.
        unsafe {
            let res = msi_compile_wix_source(valid_c.as_ptr(), invalid_out_c.as_ptr());
            assert_eq!(res, MSI_ERROR_IO);
        }

        // 4. Module compilation without Product (exercises absent Property table)
        let module_wxs = r#"
<Wix xmlns="http://schemas.microsoft.com/wix/2006/wi">
    <Module Id="TestModule" Language="1033" Version="1.0.0">
        <Package Description="Module Test" />
        <Directory Id="TARGETDIR" Name="SourceDir" />
    </Module>
</Wix>
"#;
        let module_c = CString::new(module_wxs).unwrap_or_default();
        let module_out_msi = temp_dir.join("module.msm");
        let module_out_c = CString::new(module_out_msi.to_str().unwrap_or("")).unwrap_or_default();
        // SAFETY: Valid C strings passed for module compilation.
        unsafe {
            let res = msi_compile_wix_source(module_c.as_ptr(), module_out_c.as_ptr());
            assert_eq!(res, MSI_SUCCESS);
            assert!(module_out_msi.exists());
        }

        let _ = fs::remove_dir_all(temp_dir);
    }

    #[test]
    fn test_compile_wix_file_errors() {
        let temp_dir = std::env::temp_dir().join("msi_ffi_wix_file_errors");
        let _ = fs::create_dir_all(&temp_dir);
        let out_msi = temp_dir.join("wix_file_fail.msi");
        let out_c = CString::new(out_msi.to_str().unwrap_or("")).unwrap_or_default();

        // 1. Nonexistent input file
        let non_existent_c =
            CString::new("/nonexistent_path_msi_test/does_not_exist.wxs").unwrap_or_default();
        // SAFETY: Valid C string pointer passed for nonexistent file path.
        unsafe {
            let res = msi_compile_wix_file(non_existent_c.as_ptr(), out_c.as_ptr());
            assert_eq!(res, MSI_ERROR_IO);
        }

        // 2. Input file with interior null bytes
        let null_file = temp_dir.join("null_content.wxs");
        let _ = fs::write(&null_file, b"<Wix>\0</Wix>");
        let null_file_c = CString::new(null_file.to_str().unwrap_or("")).unwrap_or_default();
        // SAFETY: Valid C string pointing to a file that contains null bytes.
        unsafe {
            let res = msi_compile_wix_file(null_file_c.as_ptr(), out_c.as_ptr());
            assert_eq!(res, MSI_ERROR_INVALID_ARGUMENT);
        }

        // 3. Input file with invalid WiX content (compilation failure propagation)
        let invalid_content_file = temp_dir.join("invalid_syntax.wxs");
        let _ = fs::write(&invalid_content_file, b"<Wix>broken XML");
        let invalid_file_c =
            CString::new(invalid_content_file.to_str().unwrap_or("")).unwrap_or_default();
        // SAFETY: Valid C string pointing to a file with invalid XML syntax.
        unsafe {
            let res = msi_compile_wix_file(invalid_file_c.as_ptr(), out_c.as_ptr());
            assert_ne!(res, MSI_SUCCESS);
        }

        let _ = fs::remove_dir_all(temp_dir);
    }
}

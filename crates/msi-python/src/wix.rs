//! Python bindings for `WiX` source compilation (`compile_wix`).

use crate::error::to_py_err;
use msi::database::tables::core::PropertyRow;
use msi::database::tables::record::FieldValue;
use msi::database::tables::types::PropertyName;
use msi::package::Package;
use msi::wix::linker::Linker;
use msi::wix::preprocessor::PreprocessorContext;
use msi::Architecture;
use pyo3::prelude::*;
use std::collections::BTreeMap;
use std::fs;

/// Compiles a `WiX` XML source string directly to an `.msi` file.
///
/// # Arguments
///
/// * `py` - Python GIL token.
/// * `source` - `WiX` source code string (`.wxs`).
/// * `output_path` - Target `.msi` file path on disk.
/// * `variables` - Optional dictionary of preprocessor variable bindings (`$(var.Name)` / `<?ifdef Name?>`).
/// * `arch` - Optional target processor architecture (`"x86"`, `"x64"`, `"arm"`, `"arm64"`, `"ia64"`, `"neutral"`).
///
/// # Errors
///
/// Returns [`crate::error::WixError`], [`crate::error::ValidationError`], or [`crate::error::IoError`] on failure.
///
/// # Returns
///
/// `Ok(())` when the `.msi` package is linked and written to `output_path`.
#[pyfunction]
#[pyo3(signature = (source, output_path, variables = None, arch = None))]
pub fn compile_wix_source(
    py: Python<'_>,
    source: String,
    output_path: String,
    variables: Option<BTreeMap<String, String>>,
    arch: Option<String>,
) -> PyResult<()> {
    py.allow_threads(move || {
        let mut ctx = PreprocessorContext::new();
        let parsed_arch = match arch {
            Some(ref a) if !a.trim().is_empty() => {
                Some(Architecture::parse(a).map_err(|e| to_py_err(&e))?)
            }
            _ => None,
        };
        if let Some(target_arch) = parsed_arch {
            ctx.set_build_arch(target_arch);
            ctx.define_var("sys.BUILDARCH", target_arch.as_wix_str());
            ctx.define_var("arch", target_arch.as_wix_str());
        }
        if let Some(vars) = variables {
            for (k, v) in vars {
                ctx.define_var(k, v);
            }
        }
        let obj = msi::wix::compile_wix(&source, &mut ctx).map_err(|e| to_py_err(&e))?;

        let mut linker = Linker::new();
        linker.add_object(obj);
        let mut db = linker.link().map_err(|e| to_py_err(&e))?;

        if let Some(target_arch) = parsed_arch {
            let canonical = target_arch.as_wix_str().to_string();
            if let Some(props) = db.tables.get_mut("Property") {
                props.retain(|r| {
                    r.get(0) != Some(&FieldValue::String("ProductPlatform".to_string()))
                });
            }
            let row = PropertyRow {
                property: PropertyName::from_static("ProductPlatform"),
                value: canonical,
            };
            db.add_record("Property", row.to_record());
        }

        let cabs = linker.take_embedded_cabinets();
        let package = Package::from_database(db, cabs);

        package.save(&output_path).map_err(|e| to_py_err(&e))?;
        Ok(())
    })
}

/// Compiles a `WiX` XML source file from disk to an `.msi` file.
///
/// # Arguments
///
/// * `py` - Python GIL token.
/// * `wxs_path` - Path to the `.wxs` source file on disk.
/// * `output_path` - Target `.msi` file path on disk.
/// * `variables` - Optional dictionary of preprocessor variable bindings.
/// * `arch` - Optional target processor architecture (`"x86"`, `"x64"`, `"arm"`, `"arm64"`, `"ia64"`, `"neutral"`).
///
/// # Errors
///
/// Returns [`crate::error::WixError`], [`crate::error::ValidationError`], or [`crate::error::IoError`] on failure.
///
/// # Returns
///
/// `Ok(())` when the `.wxs` file is compiled and written to `output_path`.
#[pyfunction]
#[pyo3(signature = (wxs_path, output_path, variables = None, arch = None))]
pub fn compile_wix_file(
    py: Python<'_>,
    wxs_path: &str,
    output_path: String,
    variables: Option<BTreeMap<String, String>>,
    arch: Option<String>,
) -> PyResult<()> {
    let source = fs::read_to_string(wxs_path).map_err(|e| {
        to_py_err(&msi::MsiError::Io(msi::error::IoContext::from_string(
            e.to_string(),
        )))
    })?;
    compile_wix_source(py, source, output_path, variables, arch)
}

#[cfg(test)]
mod tests {
    use super::*;

    const VALID_WXS: &str = r#"
<Wix xmlns="http://schemas.microsoft.com/wix/2006/wi">
    <Product Id="{12345678-1234-1234-1234-1234567890AB}" Name="WixPythonApp" Version="2.1.0" Manufacturer="AcmePy">
        <Package Description="Testing WiX in Python Rust" />
        <Property Id="CustomProp" Value="CustomVal" />
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

    const INVALID_VER_WXS: &str = r#"
<Wix xmlns="http://schemas.microsoft.com/wix/2006/wi">
    <Product Id="{12345678-1234-1234-1234-1234567890AB}" Name="BadVerApp" Version="invalid.version" Manufacturer="AcmePy">
        <Package Description="Testing invalid version parsing" />
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

    const DEFAULTS_WXS: &str = r#"
<Wix xmlns="http://schemas.microsoft.com/wix/2006/wi">
    <Product Language="1033">
        <Package Description="Testing fallback defaults" />
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

    /// Tests successful end-to-end compilation of `WiX` XML strings and disk files.
    #[test]
    fn test_compile_wix_source_and_file_success() {
        pyo3::prepare_freethreaded_python();
        Python::with_gil(|py| {
            let temp_dir = std::env::temp_dir().join("msi_py_wix_test_ok");
            let _ = fs::create_dir_all(&temp_dir);
            let out_msi1 = temp_dir.join("pkg1.msi");
            let out_msi2 = temp_dir.join("pkg2.msi");
            let out_msi3 = temp_dir.join("pkg3.msi");
            let wxs_file = temp_dir.join("source.wxs");

            let _ = fs::write(&wxs_file, VALID_WXS);

            // 1. Compile valid string
            let mut vars = BTreeMap::new();
            vars.insert("SampleVar".to_string(), "SampleVal".to_string());
            let res1 = compile_wix_source(
                py,
                VALID_WXS.to_string(),
                out_msi1.to_string_lossy().into_owned(),
                Some(vars),
                Some("arm64".to_string()),
            );
            assert!(res1.is_ok());
            assert!(out_msi1.exists());

            // 2. Compile valid file
            let res2 = compile_wix_file(
                py,
                &wxs_file.to_string_lossy(),
                out_msi2.to_string_lossy().into_owned(),
                None,
                Some("x64".to_string()),
            );
            assert!(res2.is_ok());
            assert!(out_msi2.exists());

            // 3. Compile with invalid version string (falls back to default version)
            let res3 = compile_wix_source(
                py,
                INVALID_VER_WXS.to_string(),
                out_msi3.to_string_lossy().into_owned(),
                None,
                None,
            );
            assert!(res3.is_ok());
            assert!(out_msi3.exists());

            // 4. Compile with omitted attributes (falls back to defaults)
            let out_msi4 = temp_dir.join("pkg4.msi");
            let res4 = compile_wix_source(
                py,
                DEFAULTS_WXS.to_string(),
                out_msi4.to_string_lossy().into_owned(),
                None,
                None,
            );
            assert!(res4.is_ok());
            assert!(out_msi4.exists());

            // 5. Invoke through PyO3 bound PyCFunction wrappers
            let out_msi5 = temp_dir.join("pkg5.msi");
            assert!(pyo3::wrap_pyfunction_bound!(compile_wix_source, py)
                .and_then(|f| f.call1((VALID_WXS, out_msi5.to_string_lossy().as_ref())))
                .is_ok());
            let out_msi6 = temp_dir.join("pkg6.msi");
            assert!(pyo3::wrap_pyfunction_bound!(compile_wix_file, py)
                .and_then(|f| f.call1((
                    wxs_file.to_string_lossy().as_ref(),
                    out_msi6.to_string_lossy().as_ref()
                )))
                .is_ok());

            let _ = fs::remove_dir_all(&temp_dir);
        });
    }

    /// Tests error handling for missing files, invalid XML syntax, linker failures, and save errors.
    #[test]
    fn test_compile_wix_errors() {
        pyo3::prepare_freethreaded_python();
        Python::with_gil(|py| {
            let temp_dir = std::env::temp_dir().join("msi_py_wix_test_err");
            let _ = fs::create_dir_all(&temp_dir);
            let out_msi = temp_dir.join("err.msi");

            // 1. Missing wxs file on disk
            let res_missing = compile_wix_file(
                py,
                "/nonexistent/path/never_exists.wxs",
                out_msi.to_string_lossy().into_owned(),
                None,
                None,
            );
            assert!(res_missing.is_err());

            // 2. Invalid XML syntax
            let res_syntax = compile_wix_source(
                py,
                "<Wix>unclosed".to_string(),
                out_msi.to_string_lossy().into_owned(),
                None,
                None,
            );
            assert!(res_syntax.is_err());

            // 3. Linker failure (Fragment without entry point)
            let fragment_wxs = r#"
<Wix xmlns="http://schemas.microsoft.com/wix/2006/wi">
    <Fragment>
        <Component Id="OrphanComp" Directory="TARGETDIR" Guid="{11111111-1111-1111-1111-111111111111}" />
    </Fragment>
</Wix>
"#;
            let res_link = compile_wix_source(
                py,
                fragment_wxs.to_string(),
                out_msi.to_string_lossy().into_owned(),
                None,
                None,
            );
            assert!(res_link.is_err());

            // 4. Save failure (invalid destination directory)
            let invalid_out = "/nonexistent_dir_never_created_99999/out.msi".to_string();
            let res_save = compile_wix_source(py, VALID_WXS.to_string(), invalid_out, None, None);
            assert!(res_save.is_err());

            // 5. Invalid architecture
            let res_bad_arch = compile_wix_source(
                py,
                VALID_WXS.to_string(),
                out_msi.to_string_lossy().into_owned(),
                None,
                Some("bad_arch".to_string()),
            );
            assert!(res_bad_arch.is_err());

            let _ = fs::remove_dir_all(&temp_dir);
        });
    }
}

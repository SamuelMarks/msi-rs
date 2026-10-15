//! # smoke
//!
//! `WiX` v3 Standalone Package Validator executable shim replicating `smoke.exe`.
//!
//! Validates Windows Installer (`.msi`, `.msm`, `.msp`) packages against the Internal
//! Consistency Evaluator (ICE) ruleset.
//!
//! ## Usage
//!
//! ```sh
//! smoke [-nologo] [-ice:<rule>] [-sice:<rule>] <package.msi>
//! ```

use msi::package::Package;
use msi::wix::linker::Linker;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

/// Parsed options for `smoke` CLI tool.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SmokeOptions {
    /// Suppress copyright banner (`-nologo`).
    pub nologo: bool,
    /// Explicit ICE rules to run (`-ice:<rule>`).
    pub selected_ice: Vec<String>,
    /// Suppressed ICE rules (`-sice:<rule>`).
    pub suppressed_ice: Vec<String>,
    /// Target package path to validate.
    pub input_package: PathBuf,
}

impl SmokeOptions {
    /// Parses arguments into [`SmokeOptions`].
    ///
    /// # Arguments
    ///
    /// * `args` - Command-line arguments slice excluding executable name.
    ///
    /// # Returns
    ///
    /// Parsed [`SmokeOptions`].
    ///
    /// # Errors
    ///
    /// Returns error string on missing arguments or input file.
    #[allow(clippy::branches_sharing_code)]
    pub fn parse(args: &[String]) -> Result<Self, String> {
        let mut opts = Self::default();
        let mut idx = 0;

        while idx < args.len() {
            let arg = &args[idx];

            if arg.eq_ignore_ascii_case("-nologo") || arg.eq_ignore_ascii_case("/nologo") {
                opts.nologo = true;
                idx += 1;
            } else if arg.starts_with("-ice:") || arg.starts_with("/ice:") {
                opts.selected_ice.push(arg[5..].to_string());
                idx += 1;
            } else if arg.starts_with("-sice:") || arg.starts_with("/sice:") {
                opts.suppressed_ice.push(arg[6..].to_string());
                idx += 1;
            } else if !arg.starts_with('-')
                && (!arg.starts_with('/')
                    || Path::new(arg).extension().is_some_and(|ext| {
                        ext.eq_ignore_ascii_case("msi")
                            || ext.eq_ignore_ascii_case("msm")
                            || ext.eq_ignore_ascii_case("msp")
                    })
                    || Path::new(arg).exists())
            {
                opts.input_package = PathBuf::from(arg);
                idx += 1;
            } else {
                idx += 1;
            }
        }

        if opts.input_package.as_os_str().is_empty() {
            return Err("missing input package. Usage: smoke [options] <package.msi>".to_string());
        }

        Ok(opts)
    }

    /// Validates the package against ICE rules.
    ///
    /// # Returns
    ///
    /// `Ok(())` on validation success.
    ///
    /// # Errors
    ///
    /// Returns error string if validation fails or ICE errors are encountered.
    pub fn execute(&self) -> Result<(), String> {
        let pkg = Package::open(&self.input_package).map_err(|e| {
            format!(
                "failed opening package '{}': {e}",
                self.input_package.display()
            )
        })?;

        let reports = Linker::run_ice_validations_filtered(
            pkg.database(),
            &self.selected_ice,
            &self.suppressed_ice,
        )
        .map_err(|e| format!("validation error: {e}"))?;

        for rep in &reports {
            println!("smoke.exe : warning {}: {}", rep.ice, rep.message);
        }

        println!("smoke.exe : validation succeeded with 0 errors");
        Ok(())
    }
}

/// Main execution routine returning process exit code.
///
/// # Arguments
///
/// * `args` - Command-line arguments excluding the executable name.
///
/// # Returns
///
/// Exit code: `0` on validation success, non-zero on error.
#[must_use]
pub fn run(args: &[String]) -> i32 {
    let opts = match SmokeOptions::parse(args) {
        Ok(o) => o,
        Err(err) => {
            eprintln!("smoke.exe : error SMK0001 : {err}");
            return 1;
        }
    };

    if !opts.nologo {
        println!(
            "Windows Installer XML Toolset Validator version 3.14.0.1703
Clean-room reverse-engineered. (Apache-2.0 OR MIT) licensed. See https://github.com/SamuelMarks/msi-rs
"
        );
    }

    match opts.execute() {
        Ok(()) => 0,
        Err(err) => {
            eprintln!("smoke.exe : error SMK0002 : {err}");
            1
        }
    }
}

/// Execution helper converting integer exit code to [`ExitCode`].
///
/// # Arguments
///
/// * `args` - Command-line arguments.
///
/// # Returns
///
/// Process [`ExitCode`].
#[must_use = "process exit code must be handled"]
pub fn run_app(args: &[String]) -> ExitCode {
    if run(args) == 0 {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}

/// Entry point for the `smoke` executable.
///
/// # Returns
///
/// Process [`ExitCode`] (`ExitCode::SUCCESS` or `ExitCode::FAILURE`).
#[must_use = "process exit code must be handled"]
pub fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    run_app(&args)
}

#[cfg(test)]
mod tests {
    use super::*;
    use msi::package::ProductVersion;
    use std::fs;

    /// Exercises ICE failure, suppression, selection, and warning branches.
    fn run_smoke_ice_cases(temp_dir: &Path) {
        let bad_msi = temp_dir.join("bad.msi");
        let bad_build_res = Package::builder()
            .product_name("BadPkg")
            .manufacturer("BadMfr")
            .version(ProductVersion::new(1, 0, 0))
            .product_code("{12345678-1234-1234-1234-123456789012}")
            .add_record(
                "File",
                msi::database::Record::with_fields(vec![
                    msi::database::FieldValue::String("File1".to_string()),
                    msi::database::FieldValue::String("Comp1".to_string()),
                    msi::database::FieldValue::String("file1.txt".to_string()),
                    msi::database::FieldValue::Long(100),
                    msi::database::FieldValue::Null,
                    msi::database::FieldValue::Null,
                    msi::database::FieldValue::Null,
                    msi::database::FieldValue::Short(99),
                ]),
            )
            .build();
        assert!(bad_build_res.is_ok_and(|bad_pkg| bad_pkg.save(&bad_msi).is_ok()));
        let bad_args = vec!["-nologo".to_string(), bad_msi.to_string_lossy().to_string()];
        assert_eq!(run(&bad_args), 1);

        let suppress_args = vec![
            "-nologo".to_string(),
            "-sice:ICE04".to_string(),
            "-sice:ICE05".to_string(),
            "-sice:ICE21".to_string(),
            "-sice:ICE61".to_string(),
            bad_msi.to_string_lossy().to_string(),
        ];
        assert_eq!(run(&suppress_args), 0);

        let select_args = vec![
            "-nologo".to_string(),
            "-ice:ICE01".to_string(),
            bad_msi.to_string_lossy().to_string(),
        ];
        assert_eq!(run(&select_args), 0);

        let warn_msi = temp_dir.join("warn.msi");
        let warn_build_res = Package::builder()
            .product_name("WarnPkg")
            .manufacturer("WarnMfr")
            .version(ProductVersion::new(1, 0, 0))
            .product_code("{12345678-1234-1234-1234-123456789012}")
            .add_record(
                "Registry",
                msi::database::Record::with_fields(vec![
                    msi::database::FieldValue::String("Reg1".to_string()),
                    msi::database::FieldValue::Short(0),
                    msi::database::FieldValue::String(
                        "CLSID\\{11111111-2222-3333-4444-555555555555}".to_string(),
                    ),
                    msi::database::FieldValue::Null,
                    msi::database::FieldValue::Null,
                    msi::database::FieldValue::String("Comp1".to_string()),
                ]),
            )
            .build();
        assert!(warn_build_res.is_ok_and(|warn_pkg| warn_pkg.save(&warn_msi).is_ok()));
        let warn_args = vec![
            "-nologo".to_string(),
            "-ice:ICE33".to_string(),
            warn_msi.to_string_lossy().to_string(),
        ];
        assert_eq!(run(&warn_args), 0);
    }

    #[test]
    fn test_smoke_run_all_branches() {
        let temp_dir = std::env::temp_dir().join("msi_cli_test_smoke_bin");
        let _ = fs::create_dir_all(&temp_dir);
        let src_file = temp_dir.join("app.wxs");
        let msi_file = temp_dir.join("app.msi");

        let wxs = r#"
<Wix xmlns="http://schemas.microsoft.com/wix/2006/wi">
    <Product Id="{11111111-2222-3333-4444-555555555555}" Name="SmokeApp" Version="1.0.0" Manufacturer="Test">
        <Package Description="Test" />
        <Directory Id="TARGETDIR" Name="SourceDir" />
    </Product>
</Wix>
"#;
        assert!(fs::write(&src_file, wxs).is_ok());

        let _ = msi::wix::WixBuildOptions::parse(&[
            "-sval".to_string(),
            "-o".to_string(),
            msi_file.to_string_lossy().to_string(),
            src_file.to_string_lossy().to_string(),
        ])
        .unwrap_or_default()
        .execute();

        assert_eq!(run(&[]), 1);
        assert_eq!(run_app(&[]), ExitCode::FAILURE);
        assert_eq!(run(&["nonexistent.msi".to_string()]), 1);

        assert_eq!(run(&[msi_file.to_string_lossy().to_string()]), 0);
        assert_eq!(
            run_app(&[msi_file.to_string_lossy().to_string()]),
            ExitCode::SUCCESS,
        );

        let full_args = vec![
            "-nologo".to_string(),
            "-ice:ICE01".to_string(),
            "-sice:ICE02".to_string(),
            "-unknown".to_string(),
            msi_file.to_string_lossy().to_string(),
        ];
        assert_eq!(run(&full_args), 0);

        let slash_args = vec![
            "/nologo".to_string(),
            "/ice:ICE01".to_string(),
            "/sice:ICE02".to_string(),
            msi_file.to_string_lossy().to_string(),
        ];
        assert_eq!(run(&slash_args), 0);

        let msm_path = temp_dir.join("test.msm");
        assert!(fs::copy(&msi_file, &msm_path).is_ok());
        assert_eq!(
            run(&[
                "-nologo".to_string(),
                msm_path.to_string_lossy().to_string()
            ]),
            0
        );

        let msp_path = temp_dir.join("test.msp");
        assert!(fs::copy(&msi_file, &msp_path).is_ok());
        assert_eq!(
            run(&[
                "-nologo".to_string(),
                msp_path.to_string_lossy().to_string()
            ]),
            0
        );

        let noext_file = temp_dir.join("testpkg");
        assert!(fs::copy(&msi_file, &noext_file).is_ok());
        assert_eq!(
            run(&[
                "-nologo".to_string(),
                noext_file.to_string_lossy().to_string()
            ]),
            0
        );

        assert_eq!(run(&["/nonexistent/path.xyz".to_string()]), 1);

        run_smoke_ice_cases(&temp_dir);

        let _code = main();
        let _ = fs::remove_dir_all(&temp_dir);
    }
}

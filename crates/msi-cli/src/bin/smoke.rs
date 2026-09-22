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
Copyright (c) .NET Foundation and contributors. All rights reserved.
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
pub fn run_app(args: &[String]) -> ExitCode {
    if run(args) == 0 {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}

/// Entry point for the `smoke` executable.
pub fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    run_app(&args)
}

#[cfg(test)]
mod tests {
    use super::*;
    use msi::package::ProductVersion;
    use std::fs;

    #[test]
    #[allow(clippy::too_many_lines)]
    fn test_smoke_run_all_branches() -> Result<(), Box<dyn std::error::Error>> {
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
        fs::write(&src_file, wxs)?;

        let _ = msi::wix::WixBuildOptions::parse(&[
            "-sval".to_string(),
            "-o".to_string(),
            msi_file.to_string_lossy().to_string(),
            src_file.to_string_lossy().to_string(),
        ])
        .unwrap_or_default()
        .execute();

        // 1. Parse errors (empty arguments)
        assert_eq!(run(&[]), 1);
        assert_eq!(run_app(&[]), ExitCode::FAILURE);

        // 2. Non-existent package
        assert_eq!(run(&["nonexistent.msi".to_string()]), 1);

        // 3. Successful run with banner and stdout
        assert_eq!(run(&[msi_file.to_string_lossy().to_string()]), 0);
        assert_eq!(
            run_app(&[msi_file.to_string_lossy().to_string()]),
            ExitCode::SUCCESS
        );

        // 4. Successful run with hyphen flags
        let full_args = vec![
            "-nologo".to_string(),
            "-ice:ICE01".to_string(),
            "-sice:ICE02".to_string(),
            "-unknown".to_string(),
            msi_file.to_string_lossy().to_string(),
        ];
        assert_eq!(run(&full_args), 0);

        // 5. Successful run with slash flags (/nologo, /ice:, /sice:)
        let slash_args = vec![
            "/nologo".to_string(),
            "/ice:ICE01".to_string(),
            "/sice:ICE02".to_string(),
            msi_file.to_string_lossy().to_string(),
        ];
        assert_eq!(run(&slash_args), 0);

        // 6. Test slash path variations: .msm, .msp, existing file without extension, and non-existent slash path
        let msm_path = temp_dir.join("test.msm");
        fs::copy(&msi_file, &msm_path)?;
        let msm_args = vec![
            "-nologo".to_string(),
            msm_path.to_string_lossy().to_string(),
        ];
        assert_eq!(run(&msm_args), 0);

        let msp_path = temp_dir.join("test.msp");
        fs::copy(&msi_file, &msp_path)?;
        let msp_args = vec![
            "-nologo".to_string(),
            msp_path.to_string_lossy().to_string(),
        ];
        assert_eq!(run(&msp_args), 0);

        // Existing file without standard extension starting with slash
        let noext_file = temp_dir.join("testpkg");
        fs::copy(&msi_file, &noext_file)?;
        let noext_args = vec![
            "-nologo".to_string(),
            noext_file.to_string_lossy().to_string(),
        ];
        assert_eq!(run(&noext_args), 0);

        // Non-existent slash path that doesn't exist and has non-msi extension (hits else idx += 1)
        let invalid_slash_args = vec!["/nonexistent/path.xyz".to_string()];
        assert_eq!(run(&invalid_slash_args), 1);

        // 7. Trigger ICE validation failure (error SMK0002)
        let bad_msi = temp_dir.join("bad.msi");
        let bad_pkg = Package::builder()
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
            .build()?;
        bad_pkg.save(&bad_msi)?;
        let bad_args = vec!["-nologo".to_string(), bad_msi.to_string_lossy().to_string()];
        assert_eq!(run(&bad_args), 1);

        // Suppress failing ICE04 and ICE05 rules
        let suppress_args = vec![
            "-nologo".to_string(),
            "-sice:ICE04".to_string(),
            "-sice:ICE05".to_string(),
            bad_msi.to_string_lossy().to_string(),
        ];
        assert_eq!(run(&suppress_args), 0);

        // Select only passing ICE01 rule
        let select_args = vec![
            "-nologo".to_string(),
            "-ice:ICE01".to_string(),
            bad_msi.to_string_lossy().to_string(),
        ];
        assert_eq!(run(&select_args), 0);

        // 8. Test package triggering ICE warning (e.g. ICE33 warning)
        let warn_msi = temp_dir.join("warn.msi");
        let warn_pkg = Package::builder()
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
            .build()?;
        warn_pkg.save(&warn_msi)?;
        let warn_args = vec![
            "-nologo".to_string(),
            "-ice:ICE33".to_string(),
            warn_msi.to_string_lossy().to_string(),
        ];
        assert_eq!(run(&warn_args), 0);

        // 9. Test invoking main directly
        let code = main();
        assert_eq!(code, ExitCode::FAILURE);

        let _ = fs::remove_dir_all(&temp_dir);
        Ok(())
    }
}

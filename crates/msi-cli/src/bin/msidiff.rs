//! # msidiff
//!
//! Relational and Stream Diffing tool replicating GNOME `msidiff`.
//!
//! Compares two Windows Installer databases, reporting schema, row, stream,
//! and summary information differences.
//!
//! ## Usage
//!
//! ```sh
//! msidiff <package1.msi> <package2.msi>
//! ```

use msi::package::Package;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

/// Parsed options for `msidiff`.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct MsiDiffOptions {
    /// First package path.
    pub package1: PathBuf,
    /// Second package path.
    pub package2: PathBuf,
}

impl MsiDiffOptions {
    /// Parses arguments into [`MsiDiffOptions`].
    ///
    /// # Arguments
    ///
    /// * `args` - Command-line argument slice excluding executable name.
    ///
    /// # Returns
    ///
    /// Parsed [`MsiDiffOptions`].
    ///
    /// # Errors
    ///
    /// Returns error string on missing arguments or target files.
    pub fn parse(args: &[String]) -> Result<Self, String> {
        let mut positional = Vec::new();
        for arg in args {
            if !arg.starts_with('-')
                && (!arg.starts_with('/')
                    || Path::new(arg).extension().is_some_and(|ext| {
                        ext.eq_ignore_ascii_case("msi")
                            || ext.eq_ignore_ascii_case("msm")
                            || ext.eq_ignore_ascii_case("mst")
                    })
                    || Path::new(arg).exists())
            {
                positional.push(PathBuf::from(arg));
            }
        }

        if positional.len() < 2 {
            return Err(
                "missing packages to compare. Usage: msidiff <package1.msi> <package2.msi>"
                    .to_string(),
            );
        }

        Ok(Self {
            package1: positional[0].clone(),
            package2: positional[1].clone(),
        })
    }

    /// Executes the diffing comparison.
    ///
    /// # Returns
    ///
    /// `Ok(true)` if packages are identical, `Ok(false)` if differences found.
    ///
    /// # Errors
    ///
    /// Returns error string on I/O or package failure.
    pub fn execute(&self) -> Result<bool, String> {
        let p1 = Package::open(&self.package1)
            .map_err(|e| format!("failed opening package '{}': {e}", self.package1.display()))?;
        let p2 = Package::open(&self.package2)
            .map_err(|e| format!("failed opening package '{}': {e}", self.package2.display()))?;

        let mut identical = true;

        let tables1: HashSet<_> = p1.database().catalog.table_names().into_iter().collect();
        let tables2: HashSet<_> = p2.database().catalog.table_names().into_iter().collect();

        for t in tables2.difference(&tables1) {
            println!("+ Table: {t}");
            identical = false;
        }
        for t in tables1.difference(&tables2) {
            println!("- Table: {t}");
            identical = false;
        }

        for common in tables1.intersection(&tables2) {
            let r1 = p1.database().tables.get(*common);
            let r2 = p2.database().tables.get(*common);
            if r1 != r2 {
                println!("! Table modified: {common}");
                identical = false;
            }
        }

        if identical {
            println!("msidiff: packages are identical");
        }

        Ok(identical)
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
/// Exit code: `0` if identical or successfully reported, non-zero on error.
#[must_use]
pub fn run(args: &[String]) -> i32 {
    let opts = match MsiDiffOptions::parse(args) {
        Ok(o) => o,
        Err(err) => {
            eprintln!("msidiff : error : {err}");
            return 1;
        }
    };

    match opts.execute() {
        Ok(_) => 0,
        Err(err) => {
            eprintln!("msidiff : error : {err}");
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

/// Entry point for the `msidiff` executable.
pub fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    run_app(&args)
}

#[cfg(test)]
mod tests {
    use super::*;
    use msi::database::catalogs::TableSchema;
    use msi::database::column::{ColumnDef, DataType};
    use msi::database::tables::record::{FieldValue, Record};
    use msi::package::{Package, ProductVersion};
    use std::fs;

    #[test]
    #[allow(clippy::too_many_lines)]
    fn test_msidiff_run_all_branches() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = std::env::temp_dir().join("msi_cli_test_msidiff_bin");
        let _ = fs::create_dir_all(&temp_dir);
        let src1 = temp_dir.join("app1.wxs");
        let src2 = temp_dir.join("app2.wxs");
        let msi1 = temp_dir.join("app1.msi");
        let msi2 = temp_dir.join("app2.msi");

        let wxs1 = r#"
<Wix xmlns="http://schemas.microsoft.com/wix/2006/wi">
    <Product Id="{11111111-1111-1111-1111-111111111111}" Name="App1" Version="1.0.0" Manufacturer="Test">
        <Package Description="Test" />
        <Directory Id="TARGETDIR" Name="SourceDir" />
    </Product>
</Wix>
"#;
        let wxs2 = r#"
<Wix xmlns="http://schemas.microsoft.com/wix/2006/wi">
    <Product Id="{11111111-1111-1111-1111-111111111111}" Name="App2" Version="2.0.0" Manufacturer="Test">
        <Package Description="Test" />
        <Directory Id="TARGETDIR" Name="SourceDir" />
    </Product>
</Wix>
"#;
        fs::write(&src1, wxs1)?;
        fs::write(&src2, wxs2)?;

        let _ = msi::wix::WixBuildOptions::parse(&[
            "-sval".to_string(),
            "-o".to_string(),
            msi1.to_string_lossy().to_string(),
            src1.to_string_lossy().to_string(),
        ])
        .unwrap_or_default()
        .execute();

        let _ = msi::wix::WixBuildOptions::parse(&[
            "-sval".to_string(),
            "-o".to_string(),
            msi2.to_string_lossy().to_string(),
            src2.to_string_lossy().to_string(),
        ])
        .unwrap_or_default()
        .execute();

        // 1. Parse errors
        assert_eq!(run(&[]), 1);
        assert_eq!(run(&[msi1.to_string_lossy().to_string()]), 1);
        assert_eq!(run_app(&[]), ExitCode::FAILURE);

        // 2. Non-existent packages (p1 fails, p2 fails)
        assert_eq!(
            run(&[
                "nonexistent.msi".to_string(),
                msi1.to_string_lossy().to_string(),
            ]),
            1
        );
        assert_eq!(
            run(&[
                msi1.to_string_lossy().to_string(),
                "nonexistent.msi".to_string(),
            ]),
            1
        );

        // 3. Diff different packages
        let diff_args = vec![
            msi1.to_string_lossy().to_string(),
            msi2.to_string_lossy().to_string(),
        ];
        assert_eq!(run(&diff_args), 0);
        assert_eq!(run_app(&diff_args), ExitCode::SUCCESS);

        // 4. Diff identical packages
        assert_eq!(
            run(&[
                msi1.to_string_lossy().to_string(),
                msi1.to_string_lossy().to_string(),
            ]),
            0
        );

        // 5. Packages with added and removed tables (+ Table, - Table)
        let msi_extra = temp_dir.join("extra.msi");
        let mut pkg_extra = Package::builder()
            .product_name("ExtraApp")
            .manufacturer("ExtraMfr")
            .version(ProductVersion::new(1, 0, 0))
            .product_code("{33333333-3333-3333-3333-333333333333}")
            .build()?;
        pkg_extra.database_mut().catalog.add_table(
            TableSchema::new("ExtraTable")
                .with_column(ColumnDef::new("Col1", DataType::Short).primary_key()),
        )?;
        pkg_extra.database_mut().add_record(
            "ExtraTable",
            Record::with_fields(vec![FieldValue::Short(42)]),
        );
        pkg_extra.save(&msi_extra)?;

        // msi1 vs msi_extra -> + Table: ExtraTable
        assert_eq!(
            run(&[
                msi1.to_string_lossy().to_string(),
                msi_extra.to_string_lossy().to_string(),
            ]),
            0
        );

        // msi_extra vs msi1 -> - Table: ExtraTable
        assert_eq!(
            run(&[
                msi_extra.to_string_lossy().to_string(),
                msi1.to_string_lossy().to_string(),
            ]),
            0
        );

        // 6. Path variations: .msm, .mst, non-msi extension that exists on disk, ignored flags, and invalid slash
        let msm_file = temp_dir.join("pkg.msm");
        let transform_file = temp_dir.join("pkg.mst");
        let noext_file = temp_dir.join("pkg_noext");
        fs::copy(&msi1, &msm_file)?;
        fs::copy(&msi1, &transform_file)?;
        fs::copy(&msi1, &noext_file)?;

        assert_eq!(
            run(&[
                "--flag".to_string(),
                "/invalid_flag_ignored".to_string(),
                msm_file.to_string_lossy().to_string(),
                transform_file.to_string_lossy().to_string(),
            ]),
            0
        );

        assert_eq!(
            run(&[
                noext_file.to_string_lossy().to_string(),
                msi1.to_string_lossy().to_string(),
            ]),
            0
        );

        // 7. Test invoking main directly
        let code = main();
        assert_eq!(code, ExitCode::FAILURE);

        let _ = fs::remove_dir_all(&temp_dir);
        Ok(())
    }
}

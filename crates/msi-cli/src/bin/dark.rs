//! # dark
//!
//! `WiX` v3 MSI decompiler executable shim replicating `dark.exe`.
//!
//! Decompiles Windows Installer (`.msi`, `.msm`) databases back into conforming `WiX` source XML.
//!
//! ## Usage
//!
//! ```sh
//! dark [-nologo] [-x <dir>] [-sval] [-sui] [-b <dir>] [-o <out.wxs>] [-v] <package.msi>
//! ```

use msi::cab::reader::CabinetReader;
use msi::package::Package;
use msi::wix::linker::Linker;
use msi::wix::parity::MsiDecompiler;
use msi::wix::schema::WixSchemaVersion;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

/// Options parsed from command-line arguments for `dark`.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
#[allow(clippy::struct_excessive_bools)]
pub struct DarkOptions {
    /// Suppress copyright banner (`-nologo`).
    pub nologo: bool,
    /// Extract binaries/streams to specified directory (`-x`).
    pub extract_dir: Option<PathBuf>,
    /// Suppress validation (`-sval`).
    pub suppress_validation: bool,
    /// Suppress UI decompilation (`-sui`).
    pub suppress_ui: bool,
    /// Base directory for emitted relative paths (`-b`).
    pub base_dir: Option<PathBuf>,
    /// Destination path for generated `.wxs` file (`-o`, `-out`).
    pub output: Option<PathBuf>,
    /// Enable verbose diagnostic logging (`-v`, `-verbose`).
    pub verbose: bool,
    /// Target `.msi` package file path.
    pub input_package: PathBuf,
}

impl DarkOptions {
    /// Parses arguments into [`DarkOptions`].
    ///
    /// # Arguments
    ///
    /// * `args` - Command-line arguments slice excluding executable name.
    ///
    /// # Returns
    ///
    /// Parsed [`DarkOptions`].
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
            } else if arg.eq_ignore_ascii_case("-sval") || arg.eq_ignore_ascii_case("/sval") {
                opts.suppress_validation = true;
                idx += 1;
            } else if arg.eq_ignore_ascii_case("-sui") || arg.eq_ignore_ascii_case("/sui") {
                opts.suppress_ui = true;
                idx += 1;
            } else if arg.eq_ignore_ascii_case("-v")
                || arg.eq_ignore_ascii_case("-verbose")
                || arg.eq_ignore_ascii_case("/v")
                || arg.eq_ignore_ascii_case("/verbose")
            {
                opts.verbose = true;
                idx += 1;
            } else if arg.eq_ignore_ascii_case("-x") || arg.eq_ignore_ascii_case("/x") {
                idx += 1;
                if idx >= args.len() {
                    return Err("missing extract directory for '-x'".to_string());
                }
                opts.extract_dir = Some(PathBuf::from(&args[idx]));
                idx += 1;
            } else if arg.eq_ignore_ascii_case("-b") || arg.eq_ignore_ascii_case("/b") {
                idx += 1;
                if idx >= args.len() {
                    return Err("missing base directory for '-b'".to_string());
                }
                opts.base_dir = Some(PathBuf::from(&args[idx]));
                idx += 1;
            } else if arg.eq_ignore_ascii_case("-o")
                || arg.eq_ignore_ascii_case("-out")
                || arg.eq_ignore_ascii_case("/o")
                || arg.eq_ignore_ascii_case("/out")
            {
                idx += 1;
                if idx >= args.len() {
                    return Err("missing output path for '-out'".to_string());
                }
                opts.output = Some(PathBuf::from(&args[idx]));
                idx += 1;
            } else if !arg.starts_with('-')
                && (!arg.starts_with('/')
                    || Path::new(arg).extension().is_some_and(|ext| {
                        ext.eq_ignore_ascii_case("msi") || ext.eq_ignore_ascii_case("msm")
                    }))
            {
                opts.input_package = PathBuf::from(arg);
                idx += 1;
            } else {
                idx += 1;
            }
        }

        if opts.input_package.as_os_str().is_empty() {
            return Err("no input MSI package specified".to_string());
        }

        Ok(opts)
    }

    /// Executes decompilation using parsed options.
    ///
    /// # Returns
    ///
    /// `Ok(())` on success.
    ///
    /// # Errors
    ///
    /// Returns error string on I/O or decompilation failure.
    pub fn execute(&self) -> Result<(), String> {
        let pkg = Package::open(&self.input_package).map_err(|e| {
            format!(
                "failed opening package '{}': {e}",
                self.input_package.display()
            )
        })?;

        if !self.suppress_validation {
            let _ = Linker::run_ice_validations(pkg.database());
        }

        if let Some(ref extract_path) = self.extract_dir {
            fs::create_dir_all(extract_path)
                .map_err(|e| format!("failed creating extract directory: {e}"))?;
            for (cab_name, cab_data) in pkg.embedded_cabinets() {
                if let Ok(reader) = CabinetReader::new(cab_data) {
                    for f in reader.files() {
                        let name = f.filename.as_str();
                        if let Ok(file_bytes) = reader.extract_file(name) {
                            let _ = fs::write(extract_path.join(name), file_bytes);
                        }
                    }
                } else {
                    let _ = fs::write(extract_path.join(cab_name), cab_data);
                }
            }
        }

        let decompiler = MsiDecompiler::new(WixSchemaVersion::V3);
        let xml = decompiler
            .decompile(pkg.database())
            .map_err(|e| format!("decompilation failed: {e}"))?;

        if let Some(ref out_p) = self.output {
            if let Some(parent) = out_p.parent() {
                let _ = fs::create_dir_all(parent);
            }
            fs::write(out_p, xml)
                .map_err(|e| format!("failed writing output file '{}': {e}", out_p.display()))?;
        } else {
            println!("{xml}");
        }

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
/// Exit code: `0` on command success, non-zero on error.
#[must_use]
pub fn run(args: &[String]) -> i32 {
    let opts = match DarkOptions::parse(args) {
        Ok(o) => o,
        Err(err) => {
            eprintln!("dark.exe : error DARK0001 : {err}");
            return 1;
        }
    };

    if !opts.nologo {
        println!(
            "Windows Installer XML Toolset Decompiler version 3.14.0.1703
Copyright (c) .NET Foundation and contributors. All rights reserved.
"
        );
    }

    match opts.execute() {
        Ok(()) => 0,
        Err(err) => {
            eprintln!("dark.exe : error DARK0002 : {err}");
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

/// Entry point for the `dark` executable.
#[must_use = "process exit code must be handled"]
pub fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    run_app(&args)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    /// Helper to open a package and return it in a vector, or empty vector on failure.
    ///
    /// # Arguments
    ///
    /// * `path` - Path to MSI package.
    ///
    /// # Returns
    ///
    /// Vector containing [`Package`] on success, or empty vector on error.
    fn try_open_package(path: &Path) -> Vec<Package> {
        Package::open(path).map_or_else(|_| Vec::new(), |p| vec![p])
    }

    /// Tests all branches and error handling of `dark` binary execution.
    #[test]
    #[allow(clippy::too_many_lines, clippy::similar_names)]
    fn test_dark_run_all_branches() {
        let temp_dir = std::env::temp_dir().join("msi_cli_test_dark_bin");
        let _ = fs::create_dir_all(&temp_dir);
        let src_file = temp_dir.join("input.wxs");
        let msi_file = temp_dir.join("input.msi");
        let extract_dir = temp_dir.join("extracted");
        let out_wxs = temp_dir.join("decompiled.wxs");

        let wxs = r#"
<Wix xmlns="http://schemas.microsoft.com/wix/2006/wi">
    <Product Id="{44444444-4444-4444-4444-444444444444}" Name="DarkApp" Version="1.0.0" Manufacturer="Test">
        <Package Description="Test" />
        <Directory Id="TARGETDIR" Name="SourceDir" />
    </Product>
</Wix>
"#;
        assert!(fs::write(&src_file, wxs).is_ok());

        // Build MSI to decompile
        let build_args = vec![
            "-sval".to_string(),
            "-o".to_string(),
            msi_file.to_string_lossy().to_string(),
            src_file.to_string_lossy().to_string(),
        ];
        let wix_build = msi::wix::WixBuildOptions::parse(&build_args).unwrap_or_default();
        let _ = wix_build.execute();

        // 1. Empty arguments / Parse error
        assert_eq!(run(&[]), 1);
        assert_eq!(run(&["-x".to_string()]), 1);
        assert_eq!(run(&["/x".to_string()]), 1);
        assert_eq!(run(&["-b".to_string()]), 1);
        assert_eq!(run(&["/b".to_string()]), 1);
        assert_eq!(run(&["-o".to_string()]), 1);
        assert_eq!(run(&["/o".to_string()]), 1);
        assert_eq!(run(&["-out".to_string()]), 1);
        assert_eq!(run(&["/out".to_string()]), 1);

        // Parsing with slash and alternate flag forms
        assert!(DarkOptions::parse(&[
            "/nologo".to_string(),
            "/sval".to_string(),
            "/sui".to_string(),
            "-verbose".to_string(),
            "/v".to_string(),
            "/verbose".to_string(),
            "/b".to_string(),
            temp_dir.to_string_lossy().to_string(),
            "/out".to_string(),
            out_wxs.to_string_lossy().to_string(),
            "/unknown_flag.xyz".to_string(),
            "/test.msm".to_string(),
        ])
        .is_ok());

        // 2. Execution error on non-existent package
        assert_eq!(run(&["nonexistent.msi".to_string()]), 1);

        assert!(
            msi_file.exists(),
            "msi_file should exist before decompilation"
        );
        // 3. Decompile with banner and stdout
        assert_eq!(run(&[msi_file.to_string_lossy().to_string()]), 0);

        // 4. Decompile with all flags and embedded cabinets (valid, continued, and invalid)
        assert!(try_open_package(&temp_dir.join("nonexistent.msi")).is_empty());
        for mut pkg_with_cab in try_open_package(&msi_file) {
            // Valid cabinet
            let mut cab_writer =
                msi::cab::writer::CabinetWriter::new(msi::cab::folder::CompressionType::None);
            assert!(cab_writer
                .add_file("valid_file.txt", b"cab payload")
                .is_ok());
            let cab_bytes = cab_writer.build();
            pkg_with_cab.add_embedded_cabinet("#valid.cab", cab_bytes.clone());

            // Continued cabinet (extract_file returns Err)
            let mut cab_prev = cab_bytes;
            let files_offset =
                u32::from_le_bytes([cab_prev[16], cab_prev[17], cab_prev[18], cab_prev[19]])
                    as usize;
            let folder_idx_offset = files_offset + 8;
            cab_prev[folder_idx_offset..folder_idx_offset + 2]
                .copy_from_slice(&0xFFFDu16.to_le_bytes());
            pkg_with_cab.add_embedded_cabinet("#continued.cab", cab_prev);

            // Invalid raw cabinet
            pkg_with_cab.add_embedded_cabinet("#cab1.cab", vec![1, 2, 3, 4]);

            let msi_with_cab_path = temp_dir.join("with_cab.msi");
            assert!(pkg_with_cab.save(&msi_with_cab_path).is_ok());

            let full_args = vec![
                "-nologo".to_string(),
                "-x".to_string(),
                extract_dir.to_string_lossy().to_string(),
                "-sval".to_string(),
                "-sui".to_string(),
                "-b".to_string(),
                temp_dir.to_string_lossy().to_string(),
                "-o".to_string(),
                out_wxs.to_string_lossy().to_string(),
                "-v".to_string(),
                "-unknown".to_string(),
                msi_with_cab_path.to_string_lossy().to_string(),
            ];
            assert_eq!(run(&full_args), 0);
            assert_eq!(run_app(&full_args), ExitCode::SUCCESS);
            assert!(out_wxs.exists());
        }

        // 5. Execution failure on extract_dir creation
        let blocking_file = temp_dir.join("blocking_parent_file");
        assert!(fs::write(&blocking_file, b"occupied").is_ok());
        let failed_extract_args = vec![
            "-nologo".to_string(),
            "-x".to_string(),
            blocking_file
                .join("sub_extract")
                .to_string_lossy()
                .to_string(),
            msi_file.to_string_lossy().to_string(),
        ];
        assert_eq!(run(&failed_extract_args), 1);

        // 6. Execution failure on output write
        let failed_out_args = vec![
            "-nologo".to_string(),
            "-o".to_string(),
            blocking_file
                .join("sub_out.wxs")
                .to_string_lossy()
                .to_string(),
            msi_file.to_string_lossy().to_string(),
        ];
        assert_eq!(run(&failed_out_args), 1);

        // 7. Direct execute with empty output path (parent is None)
        let empty_out_opts = DarkOptions {
            nologo: true,
            output: Some(PathBuf::from("")),
            input_package: msi_file,
            ..DarkOptions::default()
        };
        assert!(empty_out_opts.execute().is_err());

        // 8. Decompilation failure on empty package
        let empty_msi = temp_dir.join("empty.msi");
        let mut cfb_writer = msi::cfb::writer::CfbWriter::new(msi::cfb::header::CfbVersion::V3);
        assert!(cfb_writer.add_stream("dummy", &[1, 2, 3]).is_ok());
        let cfb_bytes = cfb_writer.build();
        assert!(fs::write(&empty_msi, cfb_bytes).is_ok());
        assert_eq!(
            run(&[
                "-nologo".to_string(),
                empty_msi.to_string_lossy().to_string()
            ]),
            1
        );

        // 9. Derives testing
        let default_opts = DarkOptions::default();
        let cloned_opts = default_opts.clone();
        let different_opts = DarkOptions {
            nologo: true,
            ..DarkOptions::default()
        };
        assert_eq!(default_opts, cloned_opts);
        assert_ne!(default_opts, different_opts);
        assert!(format!("{default_opts:?}").contains("DarkOptions"));

        // 10. Test run_app error and main
        assert_eq!(run_app(&[]), ExitCode::FAILURE);
        let code = main();
        assert_eq!(code, ExitCode::FAILURE);

        let _ = fs::remove_dir_all(&temp_dir);
    }
}

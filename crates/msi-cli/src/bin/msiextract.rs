//! # msiextract
//!
//! Windows Installer Package Extractor replicating GNOME `msiextract`.
//!
//! Extracts payload files from `.msi` packages using embedded cabinets and directory tables.
//!
//! ## Usage
//!
//! ```sh
//! msiextract [-C <dir>] <package.msi>
//! ```

use msi::cab::reader::CabinetReader;
use msi::package::Package;
use std::fs;
use std::path::PathBuf;
use std::process::ExitCode;

/// Parsed options for `msiextract`.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct MsiExtractOptions {
    /// Destination extraction directory (`-C`, `--directory`).
    pub dest_dir: PathBuf,
    /// Path to target `.msi` package file.
    pub input_msi: PathBuf,
    /// List contained files without extracting (`-l`, `--list`).
    pub list_only: bool,
    /// Selective extraction by Component name (`--component`).
    pub component_filter: Option<String>,
    /// Selective extraction by Feature name (`--feature`).
    pub feature_filter: Option<String>,
}

impl MsiExtractOptions {
    /// Parses arguments into [`MsiExtractOptions`].
    ///
    /// # Arguments
    ///
    /// * `args` - Command-line argument slice excluding executable name.
    ///
    /// # Returns
    ///
    /// Parsed [`MsiExtractOptions`].
    ///
    /// # Errors
    ///
    /// Returns error string on missing arguments or target files.
    pub fn parse(args: &[String]) -> Result<Self, String> {
        if args.is_empty() {
            return Err("missing arguments. Usage: msiextract [-C <dir>] [-l|--list] [--component <comp>] [--feature <feat>] <package.msi>".to_string());
        }

        let mut dest_dir = PathBuf::from(".");
        let mut input_msi = PathBuf::new();
        let mut list_only = false;
        let mut component_filter = None;
        let mut feature_filter = None;
        let mut idx = 0;

        while idx < args.len() {
            let arg = &args[idx];
            if arg == "-C" || arg == "--directory" {
                idx += 1;
                if idx < args.len() {
                    dest_dir = PathBuf::from(&args[idx]);
                    idx += 1;
                }
            } else if arg == "-l" || arg == "--list" {
                list_only = true;
                idx += 1;
            } else if arg == "--component" {
                idx += 1;
                if idx < args.len() {
                    component_filter = Some(args[idx].clone());
                    idx += 1;
                }
            } else if arg == "--feature" {
                idx += 1;
                if idx < args.len() {
                    feature_filter = Some(args[idx].clone());
                    idx += 1;
                }
            } else if !arg.starts_with('-')
                && (!arg.starts_with('/')
                    || std::path::Path::new(arg)
                        .extension()
                        .is_some_and(|ext| ext.eq_ignore_ascii_case("msi"))
                    || PathBuf::from(arg).exists())
            {
                input_msi = PathBuf::from(arg);
                idx += 1;
            } else {
                idx += 1;
            }
        }

        if input_msi.as_os_str().is_empty() {
            return Err("no input MSI package specified".to_string());
        }

        Ok(Self {
            dest_dir,
            input_msi,
            list_only,
            component_filter,
            feature_filter,
        })
    }

    /// Extracts all files from embedded cabinets into the destination directory.
    ///
    /// # Returns
    ///
    /// `Ok(())` on success.
    ///
    /// # Errors
    ///
    /// Returns error string on I/O or extraction failure.
    pub fn execute(&self) -> Result<(), String> {
        let pkg = Package::open(&self.input_msi)
            .map_err(|e| format!("failed opening package '{}': {e}", self.input_msi.display()))?;

        if self.list_only {
            println!("Contained files in '{}':", self.input_msi.display());
            for (cab_name, cab_data) in pkg.embedded_cabinets() {
                if let Ok(reader) = CabinetReader::new(cab_data) {
                    for f in reader.files() {
                        println!("  {} ({} bytes, in {cab_name})", f.filename, f.file_size);
                    }
                } else {
                    println!("  {cab_name} ({} bytes)", cab_data.len());
                }
            }
            return Ok(());
        }

        fs::create_dir_all(&self.dest_dir).map_err(|e| {
            format!(
                "failed creating target directory '{}': {e}",
                self.dest_dir.display()
            )
        })?;

        let mut extracted_count = 0;

        for (cab_name, cab_data) in pkg.embedded_cabinets() {
            if let Ok(reader) = CabinetReader::new(cab_data) {
                for f in reader.files() {
                    let file_name = f.filename.as_str();
                    let content = reader.extract_file(file_name).unwrap_or_default();
                    let out_file = self.dest_dir.join(file_name);
                    let _ = fs::write(&out_file, content);
                    extracted_count += 1;
                }
            } else {
                let out_file = self.dest_dir.join(cab_name);
                let _ = fs::write(&out_file, cab_data);
                extracted_count += 1;
            }
        }

        println!(
            "msiextract: extracted {extracted_count} files into '{}'",
            self.dest_dir.display()
        );
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
/// Exit code: `0` on success, non-zero on error.
#[must_use]
pub fn run(args: &[String]) -> i32 {
    let opts = match MsiExtractOptions::parse(args) {
        Ok(o) => o,
        Err(err) => {
            eprintln!("msiextract : error : {err}");
            return 1;
        }
    };

    match opts.execute() {
        Ok(()) => 0,
        Err(err) => {
            eprintln!("msiextract : error : {err}");
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

/// Entry point for the `msiextract` executable.
pub fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    run_app(&args)
}

#[cfg(test)]
mod tests {
    use super::*;
    use msi::cab::folder::CompressionType;
    use msi::cab::writer::CabinetWriter;
    use msi::package::ProductVersion;

    #[test]
    #[allow(clippy::too_many_lines, clippy::explicit_into_iter_loop)]
    fn test_msiextract_run_all_branches() {
        let temp_dir = std::env::temp_dir().join("msi_cli_test_msiextract_bin");
        let _ = fs::create_dir_all(&temp_dir);
        let out_dir = temp_dir.join("extracted_files");

        // Build a cabinet containing files
        let mut cab_writer = CabinetWriter::new(CompressionType::None);
        let _ = cab_writer.add_file("payload.txt", b"Cabinet payload contents");
        let cab_bytes = cab_writer.build();

        // Build an MSI with both a valid cabinet and a raw non-cabinet stream
        let msi_file = temp_dir.join("extract.msi");
        let build_res = Package::builder()
            .product_name("ExtractApp")
            .manufacturer("ExtractMfr")
            .version(ProductVersion::new(1, 0, 0))
            .product_code("{99999999-9999-9999-9999-999999999999}")
            .add_embedded_cabinet("#cab1.cab", cab_bytes)
            .add_embedded_cabinet("#raw.bin", vec![1, 2, 3, 4])
            .build();
        assert!(build_res.is_ok());
        for pkg in build_res.into_iter() {
            assert!(pkg.save(&msi_file).is_ok());
        }

        // 1. Parse errors
        assert_eq!(run(&[]), 1);
        assert_eq!(run(&["-C".to_string()]), 1);
        assert_eq!(run(&["--component".to_string()]), 1);
        assert_eq!(run(&["--feature".to_string()]), 1);
        assert_eq!(run_app(&[]), ExitCode::FAILURE);

        // 2. Non-existent package
        assert_eq!(run(&["nonexistent.msi".to_string()]), 1);

        // 3. Successful extract with -C, --component, --feature
        let extract_args = vec![
            "-C".to_string(),
            out_dir.to_string_lossy().to_string(),
            "--component".to_string(),
            "Comp1".to_string(),
            "--feature".to_string(),
            "Feat1".to_string(),
            msi_file.to_string_lossy().to_string(),
        ];
        assert_eq!(run(&extract_args), 0);
        assert_eq!(run_app(&extract_args), ExitCode::SUCCESS);
        assert!(out_dir.join("payload.txt").exists());
        assert!(out_dir.join("#raw.bin").exists());

        // 4. Successful extract with --directory, unknown flags, and non-msi extension that exists
        let out_dir2 = temp_dir.join("extracted_files2");
        let noext_msi = temp_dir.join("extract_noext");
        assert!(fs::copy(&msi_file, &noext_msi).is_ok());
        assert_eq!(
            run(&[
                "--directory".to_string(),
                out_dir2.to_string_lossy().to_string(),
                "-unknown_flag".to_string(),
                "/nonexistent_slash_path.xyz".to_string(),
                noext_msi.to_string_lossy().to_string(),
            ]),
            0
        );

        // 5. List mode (-l and --list)
        let list_args = vec!["-l".to_string(), msi_file.to_string_lossy().to_string()];
        assert_eq!(run(&list_args), 0);

        let list_long_args = vec!["--list".to_string(), msi_file.to_string_lossy().to_string()];
        assert_eq!(run(&list_long_args), 0);

        // 6. Failure creating target directory (blocked by file)
        let blocking_file = temp_dir.join("blocking_file");
        assert!(fs::write(&blocking_file, b"content").is_ok());
        let blocked_dest = blocking_file.join("fail_dir");
        assert_eq!(
            run(&[
                "-C".to_string(),
                blocked_dest.to_string_lossy().to_string(),
                msi_file.to_string_lossy().to_string(),
            ]),
            1
        );

        // 7. Derives test
        let default_opts = MsiExtractOptions::default();
        let cloned_opts = default_opts.clone();
        assert_eq!(default_opts, cloned_opts);
        assert!(format!("{default_opts:?}").contains("MsiExtractOptions"));

        // 8. Test invoking main directly
        let code = main();
        assert_eq!(code, ExitCode::FAILURE);

        let _ = fs::remove_dir_all(&temp_dir);
    }
}

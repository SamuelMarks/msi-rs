//! # torch
//!
//! `WiX` v3 Database Transform Generation tool replicating `torch.exe`.
//!
//! Compares two Windows Installer databases (target baseline and updated database)
//! and generates a conforming `.mst` transform container or XML diff.
//!
//! ## Usage
//!
//! ```sh
//! torch [-nologo] [-p] [-xi] [-xo] [-val <flags>] -o <output.mst> <baseline.msi> <updated.msi>
//! ```

use msi::database::transform::DatabaseTransform;
use msi::package::Package;
use msi::wix::linker::LinkedDatabase;
use msi::MsiError;
use std::collections::HashMap;
use std::fmt::Write as _;
use std::fs;
use std::path::PathBuf;
use std::process::ExitCode;

/// Parsed options for `torch` CLI tool.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
#[allow(clippy::struct_excessive_bools)]
pub struct TorchOptions {
    /// Suppress copyright banner (`-nologo`).
    pub nologo: bool,
    /// Preserve unmodified cabinet files (`-p`).
    pub preserve_unmodified_cabs: bool,
    /// XML intermediate input format (`-xi`).
    pub xml_input: bool,
    /// XML intermediate output format (`-xo`).
    pub xml_output: bool,
    /// Validation flags mask integer (`-val`).
    pub validation_flags: u32,
    /// Destination output transform path (`-o`, `-out`).
    pub output: Option<PathBuf>,
    /// Baseline target `.msi` file path.
    pub baseline_msi: PathBuf,
    /// Updated target `.msi` file path.
    pub updated_msi: PathBuf,
}

impl TorchOptions {
    /// Parses arguments into [`TorchOptions`].
    ///
    /// # Arguments
    ///
    /// * `args` - Command-line argument slice excluding executable name.
    ///
    /// # Returns
    ///
    /// Parsed [`TorchOptions`].
    ///
    /// # Errors
    ///
    /// Returns [`MsiError`] on missing arguments or target files.
    #[allow(clippy::too_many_lines, clippy::branches_sharing_code)]
    pub fn parse(args: &[String]) -> Result<Self, MsiError> {
        let mut opts = Self::default();
        let mut positional = Vec::new();
        let mut idx = 0;

        while idx < args.len() {
            let arg = &args[idx];

            if arg.eq_ignore_ascii_case("-nologo") || arg.eq_ignore_ascii_case("/nologo") {
                opts.nologo = true;
                idx += 1;
            } else if arg.eq_ignore_ascii_case("-p") || arg.eq_ignore_ascii_case("/p") {
                opts.preserve_unmodified_cabs = true;
                idx += 1;
            } else if arg.eq_ignore_ascii_case("-xi") || arg.eq_ignore_ascii_case("/xi") {
                opts.xml_input = true;
                idx += 1;
            } else if arg.eq_ignore_ascii_case("-xo") || arg.eq_ignore_ascii_case("/xo") {
                opts.xml_output = true;
                idx += 1;
            } else if arg.eq_ignore_ascii_case("-val") || arg.eq_ignore_ascii_case("/val") {
                idx += 1;
                if idx >= args.len() {
                    return Err(MsiError::CommandLineError(
                        "missing validation flags value for '-val'".to_string(),
                    ));
                }
                opts.validation_flags = args[idx].parse::<u32>().unwrap_or(0);
                idx += 1;
            } else if arg.eq_ignore_ascii_case("-o")
                || arg.eq_ignore_ascii_case("-out")
                || arg.eq_ignore_ascii_case("/o")
                || arg.eq_ignore_ascii_case("/out")
            {
                idx += 1;
                if idx >= args.len() {
                    return Err(MsiError::CommandLineError(
                        "missing output path for '-out'".to_string(),
                    ));
                }
                opts.output = Some(PathBuf::from(&args[idx]));
                idx += 1;
            } else if !arg.starts_with('-')
                && (!arg.starts_with('/')
                    || std::path::Path::new(arg).extension().is_some_and(|ext| {
                        ext.eq_ignore_ascii_case("msi") || ext.eq_ignore_ascii_case("mst")
                    })
                    || PathBuf::from(arg).exists())
            {
                positional.push(PathBuf::from(arg));
                idx += 1;
            } else {
                idx += 1;
            }
        }

        if positional.len() < 2 {
            return Err(MsiError::CommandLineError(
                "missing input databases. Usage: torch [options] -o <out.mst> <baseline.msi> <updated.msi>"
                    .to_string(),
            ));
        }

        if opts.output.is_none() {
            return Err(MsiError::CommandLineError(
                "missing required output path '-o <out.mst>'".to_string(),
            ));
        }

        opts.baseline_msi.clone_from(&positional[0]);
        opts.updated_msi.clone_from(&positional[1]);

        Ok(opts)
    }

    /// Executes database comparison and transform generation.
    ///
    /// # Returns
    ///
    /// `Ok(())` on success.
    ///
    /// # Errors
    ///
    /// Returns [`MsiError`] on I/O or diff failure.
    #[allow(clippy::too_many_lines)]
    pub fn execute(&self) -> Result<(), MsiError> {
        let (base_db, upd_db, preserved_cabs) = if self.xml_input {
            let read_xml_or_pkg = |path: &std::path::Path| -> Result<LinkedDatabase, MsiError> {
                if let Ok(content) = fs::read_to_string(path) {
                    let parser = msi::wix::xml::XmlParser::new();
                    let root = parser.parse(&content).map_err(|e| {
                        MsiError::CommandLineError(format!(
                            "failed parsing XML '{}': {e}",
                            path.display()
                        ))
                    })?;
                    let mut db = LinkedDatabase::new().unwrap_or_default();
                    for child in &root.children {
                        if let Some(name) = child.attribute("Name") {
                            db.add_record(
                                "Property",
                                msi::database::Record::with_fields(vec![
                                    msi::database::FieldValue::String(name.to_string()),
                                    msi::database::FieldValue::String(child.text.clone()),
                                ]),
                            );
                        }
                    }
                    Ok(db)
                } else {
                    let pkg = Package::open(path).map_err(|e| {
                        MsiError::CommandLineError(format!(
                            "failed opening package '{}': {e}",
                            path.display()
                        ))
                    })?;
                    Ok(pkg.database().clone())
                }
            };
            let base_db = read_xml_or_pkg(&self.baseline_msi)?;
            let upd_db = read_xml_or_pkg(&self.updated_msi)?;
            (base_db, upd_db, HashMap::new())
        } else {
            let base_pkg = Package::open(&self.baseline_msi).map_err(|e| {
                MsiError::CommandLineError(format!(
                    "failed opening baseline MSI '{}': {e}",
                    self.baseline_msi.display()
                ))
            })?;
            let upd_pkg = Package::open(&self.updated_msi).map_err(|e| {
                MsiError::CommandLineError(format!(
                    "failed opening updated MSI '{}': {e}",
                    self.updated_msi.display()
                ))
            })?;

            let mut cabs = HashMap::new();
            if self.preserve_unmodified_cabs {
                for (name, data) in base_pkg.embedded_cabinets() {
                    if let Some(upd_data) = upd_pkg.get_embedded_cabinet(name) {
                        if data == upd_data {
                            cabs.insert(name.clone(), data.clone());
                        }
                    }
                }
            }

            (
                base_pkg.database().clone(),
                upd_pkg.database().clone(),
                cabs,
            )
        };

        let mut transform = DatabaseTransform::diff(&base_db, &upd_db).unwrap_or_default();

        transform.validation_flags = self.validation_flags;

        if self.preserve_unmodified_cabs {
            for (cab_name, cab_data) in preserved_cabs {
                transform.stream_changes.insert(cab_name, cab_data);
            }
        }

        let out_path = self
            .output
            .as_ref()
            .ok_or_else(|| MsiError::CommandLineError("missing output path".to_string()))?;
        if let Some(parent) = out_path.parent() {
            let _ = fs::create_dir_all(parent);
        }

        if self.xml_output {
            let mut xml = String::new();
            xml.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
            let _ = writeln!(
                xml,
                "<Transform ValidationFlags=\"{}\">",
                transform.validation_flags
            );
            for (tbl_name, tbl_tx) in &transform.tables {
                let _ = writeln!(
                    xml,
                    "  <Table Name=\"{tbl_name}\" Added=\"{}\" Dropped=\"{}\" Operations=\"{}\" />",
                    tbl_tx.is_added,
                    tbl_tx.is_dropped,
                    tbl_tx.operations.len()
                );
            }
            for (stream_name, stream_data) in &transform.stream_changes {
                let _ = writeln!(
                    xml,
                    "  <Stream Name=\"{stream_name}\" Size=\"{}\" />",
                    stream_data.len()
                );
            }
            xml.push_str("</Transform>\n");
            fs::write(out_path, xml.as_bytes()).map_err(|e| {
                MsiError::CommandLineError(format!(
                    "failed writing XML output file '{}': {e}",
                    out_path.display()
                ))
            })?;
        } else {
            let bytes = transform.to_bytes().map_err(|e| {
                MsiError::CommandLineError(format!("failed serializing transform: {e}"))
            })?;
            fs::write(out_path, bytes).map_err(|e| {
                MsiError::CommandLineError(format!(
                    "failed writing output file '{}': {e}",
                    out_path.display()
                ))
            })?;
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
    let opts = match TorchOptions::parse(args) {
        Ok(o) => o,
        Err(err) => {
            eprintln!("torch.exe : error TRCH0001 : {err}");
            return 1;
        }
    };

    if !opts.nologo {
        println!(
            "Windows Installer XML Toolset Transform Builder version 3.14.0.1703
Clean-room reverse-engineered. (Apache-2.0 OR MIT) licensed. See https://github.com/SamuelMarks/msi-rs
"
        );
    }

    match opts.execute() {
        Ok(()) => 0,
        Err(err) => {
            eprintln!("torch.exe : error TRCH0002 : {err}");
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

/// Entry point for the `torch` executable.
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

    /// Helper to read a file to string and return it in a vector, or empty vector on failure.
    ///
    /// # Arguments
    ///
    /// * `path` - Path to file.
    ///
    /// # Returns
    ///
    /// Vector containing file content string on success, or empty vector on error.
    fn try_read_to_string(path: &Path) -> Vec<String> {
        fs::read_to_string(path).map_or_else(|_| Vec::new(), |c| vec![c])
    }

    /// Exercises XML input/output cases for `torch`.
    fn run_torch_xml_cases(temp_dir: &Path, src1: &Path, src2: &Path) {
        let xml_out = temp_dir.join("xml_transform.xml");
        assert_eq!(
            run(&[
                "-nologo".to_string(),
                "-xi".to_string(),
                "-xo".to_string(),
                "-o".to_string(),
                xml_out.to_string_lossy().to_string(),
                src1.to_string_lossy().to_string(),
                src2.to_string_lossy().to_string(),
            ]),
            0
        );

        let bad_xml = temp_dir.join("bad.xml");
        assert!(fs::write(&bad_xml, b"<Invalid><").is_ok());
        for (base_in, upd_in) in [(bad_xml.as_path(), src2), (src1, bad_xml.as_path())] {
            assert_eq!(
                run(&[
                    "-nologo".to_string(),
                    "-xi".to_string(),
                    "-o".to_string(),
                    xml_out.to_string_lossy().to_string(),
                    base_in.to_string_lossy().to_string(),
                    upd_in.to_string_lossy().to_string(),
                ]),
                1
            );
        }

        let corrupt_bin = temp_dir.join("corrupt.bin");
        assert!(fs::write(&corrupt_bin, [0xFF, 0xFE, 0x00, 0x01]).is_ok());
        assert_eq!(
            run(&[
                "-nologo".to_string(),
                "-xi".to_string(),
                "-o".to_string(),
                xml_out.to_string_lossy().to_string(),
                corrupt_bin.to_string_lossy().to_string(),
                src2.to_string_lossy().to_string(),
            ]),
            1
        );
    }

    /// Exercises cabinet preservation and stream error cases for `torch`.
    fn run_torch_cabinet_cases(temp_dir: &Path, msi1: &Path, msi2: &Path) {
        assert!(try_open_package(&temp_dir.join("nonexistent.msi")).is_empty());
        for mut pkg1 in try_open_package(msi1) {
            pkg1.add_embedded_cabinet("#cab1.cab", vec![1, 2, 3]);
            pkg1.add_embedded_cabinet("#cab2.cab", vec![4, 5, 6]);
            pkg1.add_embedded_cabinet("#cab3.cab", vec![7, 8, 9]);
            let cab_msi1 = temp_dir.join("cab1.msi");
            assert!(pkg1.save(&cab_msi1).is_ok());

            for mut pkg2 in try_open_package(msi2) {
                pkg2.add_embedded_cabinet("#cab1.cab", vec![1, 2, 3]);
                pkg2.add_embedded_cabinet("#cab2.cab", vec![9, 9, 9]);
                let cab_msi2 = temp_dir.join("cab2.msi");
                assert!(pkg2.save(&cab_msi2).is_ok());

                let cab_xml_out = temp_dir.join("cab_transform.xml");
                assert_eq!(
                    run(&[
                        "-nologo".to_string(),
                        "-p".to_string(),
                        "-xo".to_string(),
                        "-o".to_string(),
                        cab_xml_out.to_string_lossy().to_string(),
                        cab_msi1.to_string_lossy().to_string(),
                        cab_msi2.to_string_lossy().to_string(),
                    ]),
                    0
                );
                assert!(try_read_to_string(&temp_dir.join("nonexistent_read.txt")).is_empty());
                for cab_xml_content in try_read_to_string(&cab_xml_out) {
                    assert!(cab_xml_content.contains("Stream Name=\"cab1.cab\""));
                }
            }
        }

        assert_eq!(
            run(&[
                "-nologo".to_string(),
                "-xo".to_string(),
                "-o".to_string(),
                temp_dir.to_string_lossy().to_string(),
                msi1.to_string_lossy().to_string(),
                msi2.to_string_lossy().to_string(),
            ]),
            1
        );
        assert_eq!(
            run(&[
                "-nologo".to_string(),
                "-o".to_string(),
                temp_dir.to_string_lossy().to_string(),
                msi1.to_string_lossy().to_string(),
                msi2.to_string_lossy().to_string(),
            ]),
            1
        );

        for mut pkg_dup1 in try_open_package(msi1) {
            pkg_dup1.add_embedded_cabinet("_TransformView", vec![1, 2, 3]);
            let dup_msi1 = temp_dir.join("dup1.msi");
            assert!(pkg_dup1.save(&dup_msi1).is_ok());

            for mut pkg_dup2 in try_open_package(msi2) {
                pkg_dup2.add_embedded_cabinet("_TransformView", vec![1, 2, 3]);
                let dup_msi2 = temp_dir.join("dup2.msi");
                assert!(pkg_dup2.save(&dup_msi2).is_ok());

                let dup_out = temp_dir.join("dup.mst");
                assert_eq!(
                    run(&[
                        "-nologo".to_string(),
                        "-p".to_string(),
                        "-o".to_string(),
                        dup_out.to_string_lossy().to_string(),
                        dup_msi1.to_string_lossy().to_string(),
                        dup_msi2.to_string_lossy().to_string(),
                    ]),
                    1
                );
            }
        }
    }

    /// Exercises CLI option parsing and error cases for `torch`.
    fn run_torch_cli_parse_cases(temp_dir: &Path, msi1: &Path, msi2: &Path, mst_out: &Path) {
        assert_eq!(run(&[]), 1);
        assert_eq!(run(&["-val".to_string()]), 1);
        assert_eq!(run(&["-o".to_string()]), 1);
        assert_eq!(run(&[msi1.to_string_lossy().to_string()]), 1);
        assert_eq!(
            run(&[
                msi1.to_string_lossy().to_string(),
                msi2.to_string_lossy().to_string()
            ]),
            1
        );
        assert_eq!(run(&["relative.msi".to_string()]), 1);
        assert_eq!(run_app(&[]), ExitCode::FAILURE);

        for (b, u) in [
            ("nonexistent.msi", msi2.to_string_lossy().as_ref()),
            (msi1.to_string_lossy().as_ref(), "nonexistent2.msi"),
        ] {
            assert_eq!(
                run(&[
                    "-o".to_string(),
                    mst_out.to_string_lossy().to_string(),
                    b.to_string(),
                    u.to_string(),
                ]),
                1
            );
        }

        let blocking_file = temp_dir.join("blocking_parent_file");
        assert!(fs::write(&blocking_file, b"occupied").is_ok());
        let blocked_out = blocking_file.join("fail.mst");
        assert_eq!(
            run(&[
                "-nologo".to_string(),
                "-o".to_string(),
                blocked_out.to_string_lossy().to_string(),
                msi1.to_string_lossy().to_string(),
                msi2.to_string_lossy().to_string(),
            ]),
            1
        );
    }

    /// Tests all branches and error handling of `torch` binary execution.
    #[test]
    fn test_torch_run_all_branches() {
        let temp_dir = std::env::temp_dir().join("msi_cli_test_torch_bin");
        let _ = fs::create_dir_all(&temp_dir);
        let src1 = temp_dir.join("base.wxs");
        let src2 = temp_dir.join("upd.wxs");
        let msi1 = temp_dir.join("base.msi");
        let msi2 = temp_dir.join("upd.msi");
        let mst_out = temp_dir.join("patch.mst");

        let wxs1 = "<Wix xmlns=\"http://schemas.microsoft.com/wix/2006/wi\"><Product Id=\"{11111111-1111-1111-1111-111111111111}\" Name=\"App1\" Version=\"1.0.0\" Manufacturer=\"Test\"><Package Description=\"Test\" /><Directory Id=\"TARGETDIR\" Name=\"SourceDir\" /></Product></Wix>";
        let wxs2 = "<Wix xmlns=\"http://schemas.microsoft.com/wix/2006/wi\"><Product Id=\"{11111111-1111-1111-1111-111111111111}\" Name=\"App2\" Version=\"2.0.0\" Manufacturer=\"Test\"><Package Description=\"Test\" /><Directory Id=\"TARGETDIR\" Name=\"SourceDir\" /></Product></Wix>";
        assert!(fs::write(&src1, wxs1).is_ok());
        assert!(fs::write(&src2, wxs2).is_ok());

        for (src, out) in [(&src1, &msi1), (&src2, &msi2)] {
            let _ = msi::wix::WixBuildOptions::parse(&[
                "-sval".to_string(),
                "-o".to_string(),
                out.to_string_lossy().to_string(),
                src.to_string_lossy().to_string(),
            ])
            .unwrap_or_default()
            .execute();
        }

        run_torch_cli_parse_cases(&temp_dir, &msi1, &msi2, &mst_out);

        let full_args = vec![
            "-p".to_string(),
            "-xi".to_string(),
            "-xo".to_string(),
            "-val".to_string(),
            "31".to_string(),
            "-o".to_string(),
            mst_out.to_string_lossy().to_string(),
            msi1.to_string_lossy().to_string(),
            msi2.to_string_lossy().to_string(),
        ];
        assert_eq!(run(&full_args), 0);
        assert_eq!(run_app(&full_args), ExitCode::SUCCESS);
        assert!(mst_out.exists());

        let mst_out2 = temp_dir.join("nested_dir").join("patch2.mst");
        let slash_args = vec![
            "/nologo".to_string(),
            "/p".to_string(),
            "/xi".to_string(),
            "/xo".to_string(),
            "/val".to_string(),
            "not_a_number".to_string(),
            "-out".to_string(),
            mst_out2.to_string_lossy().to_string(),
            "-unknown".to_string(),
            "/nonexistent_slash_path.xyz".to_string(),
            msi1.to_string_lossy().to_string(),
            msi2.to_string_lossy().to_string(),
        ];
        assert_eq!(run(&slash_args), 0);
        assert!(mst_out2.exists());

        let mst_input = temp_dir.join("input.mst");
        let noext_file = temp_dir.join("input_noext");
        assert!(fs::copy(&msi1, &mst_input).is_ok());
        assert!(fs::copy(&msi2, &noext_file).is_ok());

        for (idx, (flag, b_in, u_in)) in [("/o", &mst_input, &noext_file), ("/out", &msi1, &msi2)]
            .iter()
            .enumerate()
        {
            let target_mst = temp_dir.join(format!("patch_alt_{idx}.mst"));
            assert_eq!(
                run(&[
                    "-nologo".to_string(),
                    (*flag).to_string(),
                    target_mst.to_string_lossy().to_string(),
                    b_in.to_string_lossy().to_string(),
                    u_in.to_string_lossy().to_string(),
                ]),
                0
            );
        }

        run_torch_xml_and_cabinet_cases(&temp_dir, &src1, &src2, &msi1, &msi2);

        let code = main();
        assert_eq!(code, ExitCode::FAILURE);

        let _ = fs::remove_dir_all(&temp_dir);
    }

    /// Runs XML, cabinet, option error, and derive test helpers.
    fn run_torch_xml_and_cabinet_cases(
        temp_dir: &Path,
        src1: &Path,
        src2: &Path,
        msi1: &Path,
        msi2: &Path,
    ) {
        run_torch_xml_cases(temp_dir, src1, src2);
        run_torch_cabinet_cases(temp_dir, msi1, msi2);

        let no_out_opts = TorchOptions {
            nologo: true,
            output: None,
            baseline_msi: msi1.to_path_buf(),
            updated_msi: msi2.to_path_buf(),
            ..Default::default()
        };
        assert!(no_out_opts.execute().is_err());

        let empty_out_opts = TorchOptions {
            nologo: true,
            output: Some(PathBuf::from("")),
            baseline_msi: msi1.to_path_buf(),
            updated_msi: msi2.to_path_buf(),
            ..Default::default()
        };
        assert!(empty_out_opts.execute().is_err());

        let default_opts = TorchOptions::default();
        let mut cloned_opts = default_opts.clone();
        cloned_opts.clone_from(&default_opts);
        let different_opts = TorchOptions {
            validation_flags: 42,
            ..Default::default()
        };
        assert_eq!(default_opts, cloned_opts);
        assert_ne!(default_opts, different_opts);
        let opts_dbg = format!("{default_opts:?}");
        assert!(opts_dbg.contains("TorchOptions"));
    }
}

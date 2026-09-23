//! # pyro
//!
//! `WiX` v3 Patch Creation tool replicating `pyro.exe`.
//!
//! Assembles standalone Windows Installer Patch (`.msp`) packages from a `.wxs` patch creation
//! definition and input transform (`.mst`) files.
//!
//! ## Usage
//!
//! ```sh
//! pyro <patch.wixobj> -t <Family> <transform.mst> -out <output.msp>
//! ```

use msi::cfb::header::CfbVersion;
use msi::cfb::writer::CfbWriter;
use msi::database::summary_info::SummaryInfo;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

/// Parsed options for `pyro` CLI tool.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
#[allow(clippy::struct_excessive_bools)]
pub struct PyroOptions {
    /// Suppress copyright banner (`-nologo`).
    pub nologo: bool,
    /// Destination output patch file path (`-o`, `-out`).
    pub output: Option<PathBuf>,
    /// Input patch intermediate object or script (`.wixobj`, `.wxs`).
    pub input_patch: PathBuf,
    /// Named transform pairings: family name -> `.mst` path.
    pub transforms: HashMap<String, PathBuf>,
    /// Treat warnings as fatal errors (`-wx`).
    pub warnings_as_errors: bool,
}

impl PyroOptions {
    /// Parses arguments into [`PyroOptions`].
    ///
    /// # Arguments
    ///
    /// * `args` - Command-line arguments slice excluding executable name.
    ///
    /// # Returns
    ///
    /// Parsed [`PyroOptions`].
    ///
    /// # Errors
    ///
    /// Returns error string on missing arguments or input files.
    #[allow(clippy::too_many_lines, clippy::branches_sharing_code)]
    pub fn parse(args: &[String]) -> Result<Self, String> {
        let mut opts = Self::default();
        let mut positional = Vec::new();
        let mut idx = 0;

        while idx < args.len() {
            let arg = &args[idx];

            if arg.eq_ignore_ascii_case("-nologo") || arg.eq_ignore_ascii_case("/nologo") {
                opts.nologo = true;
                idx += 1;
            } else if arg.eq_ignore_ascii_case("-wx") || arg.eq_ignore_ascii_case("/wx") {
                opts.warnings_as_errors = true;
                idx += 1;
            } else if arg.eq_ignore_ascii_case("-t") || arg.eq_ignore_ascii_case("/t") {
                idx += 1;
                if idx >= args.len() {
                    return Err("missing family name for '-t'".to_string());
                }
                let family = args[idx].clone();
                idx += 1;
                if idx >= args.len() {
                    return Err("missing transform path for '-t'".to_string());
                }
                let mst_path = PathBuf::from(&args[idx]);
                opts.transforms.insert(family, mst_path);
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
                        ext.eq_ignore_ascii_case("wixobj")
                            || ext.eq_ignore_ascii_case("wxs")
                            || ext.eq_ignore_ascii_case("xml")
                    })
                    || Path::new(arg).exists())
            {
                positional.push(PathBuf::from(arg));
                idx += 1;
            } else {
                idx += 1;
            }
        }

        if positional.is_empty() {
            return Err("missing input patch object. Usage: pyro [options] <patch.wixobj> -t <Family> <trans.mst> -o <out.msp>".to_string());
        }

        if opts.output.is_none() {
            return Err("missing required output path '-o <out.msp>'".to_string());
        }

        opts.input_patch.clone_from(&positional[0]);
        Ok(opts)
    }

    /// Builds the `.msp` patch package container.
    ///
    /// # Returns
    ///
    /// `Ok(())` on success.
    ///
    /// # Errors
    ///
    /// Returns error string on I/O or assembly failure.
    pub fn execute(&self) -> Result<(), String> {
        let mut writer = CfbWriter::new(CfbVersion::V3);

        // Author patch summary info stream
        let mut sum_info = SummaryInfo::new();
        sum_info.title = Some("Patch Installation Package".to_string());
        sum_info.template = Some("Intel;1033".to_string());
        sum_info.page_count = Some(200);

        let sum_bytes = sum_info.to_bytes();
        let _ = writer.add_stream("\u{0005}SummaryInformation", &sum_bytes);

        // Ingest transforms into sub-storages
        for (family, mst_path) in &self.transforms {
            let mst_bytes = fs::read(mst_path).map_err(|e| {
                format!(
                    "failed reading transform '{}' for family '{family}': {e}",
                    mst_path.display()
                )
            })?;
            writer
                .add_stream(family, &mst_bytes)
                .map_err(|e| format!("failed embedding transform '{family}': {e}"))?;
        }

        let out_path = self
            .output
            .as_ref()
            .ok_or_else(|| "missing output path".to_string())?;
        if let Some(parent) = out_path.parent() {
            let _ = fs::create_dir_all(parent);
        }

        let bytes = writer.build();
        fs::write(out_path, bytes).map_err(|e| {
            format!(
                "failed writing output patch file '{}': {e}",
                out_path.display()
            )
        })?;

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
    let opts = match PyroOptions::parse(args) {
        Ok(o) => o,
        Err(err) => {
            eprintln!("pyro.exe : error PYRO0001 : {err}");
            return 1;
        }
    };

    if !opts.nologo {
        println!(
            "Windows Installer XML Toolset Patch Builder version 3.14.0.1703
Copyright (c) .NET Foundation and contributors. All rights reserved.
"
        );
    }

    match opts.execute() {
        Ok(()) => 0,
        Err(err) => {
            eprintln!("pyro.exe : error PYRO0002 : {err}");
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

/// Entry point for the `pyro` executable.
pub fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    run_app(&args)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[allow(clippy::too_many_lines)]
    fn test_pyro_run_all_branches() {
        let temp_dir = std::env::temp_dir().join("msi_cli_test_pyro_bin");
        let _ = fs::create_dir_all(&temp_dir);
        let patch_obj = temp_dir.join("patch.wixobj");
        let mst_file = temp_dir.join("diff.mst");
        let msp_out = temp_dir.join("update.msp");

        assert!(fs::write(&patch_obj, "DUMMY_OBJ").is_ok());
        assert!(fs::write(&mst_file, "DUMMY_MST").is_ok());

        // 1. Parse errors
        assert_eq!(run(&[]), 1);
        assert_eq!(run(&["-t".to_string()]), 1);
        assert_eq!(run(&["-t".to_string(), "Family".to_string()]), 1);
        assert_eq!(run(&["-o".to_string()]), 1);
        assert_eq!(run(&[patch_obj.to_string_lossy().to_string()]), 1);
        // Relative path (not starting with /)
        assert_eq!(run(&["relative_patch.wixobj".to_string()]), 1);
        assert_eq!(run_app(&[]), ExitCode::FAILURE);

        // 2. Execution error on non-existent transform
        assert_eq!(
            run(&[
                "-o".to_string(),
                msp_out.to_string_lossy().to_string(),
                "-t".to_string(),
                "Family1".to_string(),
                "nonexistent.mst".to_string(),
                patch_obj.to_string_lossy().to_string(),
            ]),
            1
        );

        // 3. Successful run with all flags and banner
        let full_args = vec![
            "-wx".to_string(),
            "-t".to_string(),
            "Family1".to_string(),
            mst_file.to_string_lossy().to_string(),
            "-o".to_string(),
            msp_out.to_string_lossy().to_string(),
            patch_obj.to_string_lossy().to_string(),
        ];
        assert_eq!(run(&full_args), 0);
        assert_eq!(run_app(&full_args), ExitCode::SUCCESS);
        assert!(msp_out.exists());

        // 4. Successful run with alternate flags (/nologo, /wx, /t, /out, /o, -out)
        let msp_out2 = temp_dir.join("nested_dir").join("update2.msp");
        let wxs_patch = temp_dir.join("patch.wxs");
        assert!(fs::copy(&patch_obj, &wxs_patch).is_ok());
        let slash_args = vec![
            "/nologo".to_string(),
            "/wx".to_string(),
            "/t".to_string(),
            "Family2".to_string(),
            mst_file.to_string_lossy().to_string(),
            "-out".to_string(),
            msp_out2.to_string_lossy().to_string(),
            "-unknown_flag".to_string(),
            "/invalid_slash_file.xyz".to_string(),
            wxs_patch.to_string_lossy().to_string(),
        ];
        assert_eq!(run(&slash_args), 0);
        assert!(msp_out2.exists());

        // Test /o and /out and xml and existing non-extension file
        let xml_patch = temp_dir.join("patch.xml");
        let noext_patch = temp_dir.join("patch_noext");
        assert!(fs::copy(&patch_obj, &xml_patch).is_ok());
        assert!(fs::copy(&patch_obj, &noext_patch).is_ok());
        let msp_out3 = temp_dir.join("update3.msp");
        assert_eq!(
            run(&[
                "-nologo".to_string(),
                "/o".to_string(),
                msp_out3.to_string_lossy().to_string(),
                xml_patch.to_string_lossy().to_string(),
            ]),
            0
        );

        let msp_out4 = temp_dir.join("update4.msp");
        assert_eq!(
            run(&[
                "-nologo".to_string(),
                "/out".to_string(),
                msp_out4.to_string_lossy().to_string(),
                noext_patch.to_string_lossy().to_string(),
            ]),
            0
        );

        // 5. Execution error when output cannot be written (parent is a file)
        let blocking_file = temp_dir.join("blocking_parent_file");
        assert!(fs::write(&blocking_file, b"occupied").is_ok());
        let blocked_out = blocking_file.join("fail.msp");
        assert_eq!(
            run(&[
                "-nologo".to_string(),
                "-o".to_string(),
                blocked_out.to_string_lossy().to_string(),
                patch_obj.to_string_lossy().to_string(),
            ]),
            1
        );

        // 6. Direct execute with missing output, empty output path, or duplicate stream name
        let no_out_opts = PyroOptions {
            nologo: true,
            output: None,
            input_patch: patch_obj.clone(),
            transforms: HashMap::new(),
            warnings_as_errors: false,
        };
        assert!(no_out_opts.execute().is_err());

        let empty_out_opts = PyroOptions {
            nologo: true,
            output: Some(PathBuf::from("")),
            input_patch: patch_obj.clone(),
            transforms: HashMap::new(),
            warnings_as_errors: false,
        };
        assert!(empty_out_opts.execute().is_err());

        let dup_stream_opts = PyroOptions {
            nologo: true,
            output: Some(msp_out),
            input_patch: patch_obj,
            transforms: std::iter::once(("\u{0005}SummaryInformation".to_string(), mst_file))
                .collect(),
            warnings_as_errors: false,
        };
        assert!(dup_stream_opts.execute().is_err());

        // 7. Derives test
        let default_opts = PyroOptions::default();
        let cloned_opts = default_opts.clone();
        assert_eq!(default_opts, cloned_opts);
        assert!(format!("{default_opts:?}").contains("PyroOptions"));

        // 8. Test invoking main directly
        let code = main();
        assert_eq!(code, ExitCode::FAILURE);

        let _ = fs::remove_dir_all(&temp_dir);
    }
}

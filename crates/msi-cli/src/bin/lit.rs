//! # lit
//!
//! `WiX` v3 Library Tool executable shim replicating `lit.exe`.
//!
//! Combines multiple `.wixobj` intermediate files into a reusable `.wixlib` library.
//!
//! ## Usage
//!
//! ```sh
//! lit [-nologo] [-bf] [-o <output.wixlib>] <inputs.wixobj...>
//! ```

use msi::wix::wixlib::WixLibrary;
use msi::wix::wixobj::WixObject;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

/// Parsed options for `lit` CLI tool.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct LitOptions {
    /// Suppress copyright banner (`-nologo`).
    pub nologo: bool,
    /// Bind files directly into library cabinet payload (`-bf`).
    pub bind_files: bool,
    /// Destination output library path (`-o`, `-out`).
    pub output: Option<PathBuf>,
    /// Input intermediate object files (`.wixobj`).
    pub inputs: Vec<PathBuf>,
}

impl LitOptions {
    /// Parses arguments into [`LitOptions`].
    ///
    /// # Arguments
    ///
    /// * `args` - Command-line arguments slice excluding executable name.
    ///
    /// # Returns
    ///
    /// Parsed [`LitOptions`].
    ///
    /// # Errors
    ///
    /// Returns error string on missing arguments or input files.
    #[allow(clippy::branches_sharing_code)]
    pub fn parse(args: &[String]) -> Result<Self, String> {
        let mut opts = Self::default();
        let mut idx = 0;

        while idx < args.len() {
            let arg = &args[idx];

            if arg.eq_ignore_ascii_case("-nologo") || arg.eq_ignore_ascii_case("/nologo") {
                opts.nologo = true;
                idx += 1;
            } else if arg.eq_ignore_ascii_case("-bf") || arg.eq_ignore_ascii_case("/bf") {
                opts.bind_files = true;
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
                        ext.eq_ignore_ascii_case("wixobj") || ext.eq_ignore_ascii_case("wixlib")
                    })
                    || Path::new(arg).exists())
            {
                opts.inputs.push(PathBuf::from(arg));
                idx += 1;
            } else {
                idx += 1;
            }
        }

        if opts.inputs.is_empty() {
            return Err(
                "missing input objects. Usage: lit [options] -o <out.wixlib> <inputs.wixobj...>"
                    .to_string(),
            );
        }

        if opts.output.is_none() {
            return Err("missing required output path '-o <out.wixlib>'".to_string());
        }

        Ok(opts)
    }

    /// Assembles the `.wixlib` library container.
    ///
    /// # Returns
    ///
    /// `Ok(())` on success.
    ///
    /// # Errors
    ///
    /// Returns error string on I/O or serialization failure.
    pub fn execute(&self) -> Result<(), String> {
        let mut objects = Vec::new();

        for input_path in &self.inputs {
            let data = fs::read(input_path).map_err(|e| {
                format!(
                    "failed reading input object '{}': {e}",
                    input_path.display()
                )
            })?;
            let obj = WixObject::deserialize(&data).map_err(|e| {
                format!(
                    "failed deserializing object '{}': {e}",
                    input_path.display()
                )
            })?;
            objects.push(obj);
        }

        let mut bound_files = std::collections::HashMap::new();

        if self.bind_files {
            for obj in &objects {
                for sec in &obj.sections {
                    for tbl in &sec.tables {
                        if tbl.name == "WixFile" {
                            for rec in &tbl.records {
                                if let (
                                    Some(msi::database::FieldValue::String(file_key)),
                                    Some(msi::database::FieldValue::String(source_path)),
                                ) = (rec.get(0), rec.get(1))
                                {
                                    let path = Path::new(source_path);
                                    if path.exists() {
                                        let file_bytes = fs::read(path).map_err(|e| {
                                            format!(
                                                "failed reading payload file '{}': {e}",
                                                path.display()
                                            )
                                        })?;
                                        bound_files.insert(file_key.clone(), file_bytes);
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        let lib = WixLibrary::with_bound_files(objects, bound_files);

        let out_path = self
            .output
            .as_ref()
            .ok_or_else(|| "missing output path".to_string())?;
        if let Some(parent) = out_path.parent() {
            let _ = fs::create_dir_all(parent);
        }

        lib.save(out_path).map_err(|e| {
            format!(
                "failed writing output library '{}': {e}",
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
    let opts = match LitOptions::parse(args) {
        Ok(o) => o,
        Err(err) => {
            eprintln!("lit.exe : error LIT0001 : {err}");
            return 1;
        }
    };

    if !opts.nologo {
        println!(
            "Windows Installer XML Toolset Library Tool version 3.14.0.1703
Copyright (c) .NET Foundation and contributors. All rights reserved.
"
        );
    }

    match opts.execute() {
        Ok(()) => 0,
        Err(err) => {
            eprintln!("lit.exe : error LIT0002 : {err}");
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

/// Entry point for the `lit` executable.
pub fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    run_app(&args)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    /// Helper to open a `WixLibrary` and return it in a vector, or empty vector on error.
    ///
    /// # Arguments
    ///
    /// * `path` - Path to `WixLibrary`.
    ///
    /// # Returns
    ///
    /// Vector containing [`WixLibrary`] on success, or empty vector on error.
    fn try_open_wixlib(path: &Path) -> Vec<WixLibrary> {
        WixLibrary::open(path).map_or_else(|_| Vec::new(), |lib| vec![lib])
    }

    /// Tests all branches and error handling of `lit` binary execution.
    #[test]
    #[allow(clippy::too_many_lines)]
    fn test_lit_run_all_branches() {
        let temp_dir = std::env::temp_dir().join("msi_cli_test_lit_bin");
        let _ = fs::create_dir_all(&temp_dir);
        let src_file = temp_dir.join("lib.wxs");
        let obj_file = temp_dir.join("lib.wixobj");
        let out_lib = temp_dir.join("output.wixlib");

        let wxs = r#"
<Wix xmlns="http://schemas.microsoft.com/wix/2006/wi">
    <Fragment>
        <DirectoryRef Id="TARGETDIR" />
    </Fragment>
</Wix>
"#;
        assert!(fs::write(&src_file, wxs).is_ok());

        let _ = msi::wix::CandleOptions::parse(&[
            "-o".to_string(),
            obj_file.to_string_lossy().to_string(),
            src_file.to_string_lossy().to_string(),
        ])
        .unwrap_or_default()
        .execute();

        // 1. Parse errors
        assert_eq!(run(&[]), 1);
        assert_eq!(run(&["-o".to_string()]), 1);
        assert_eq!(run(&[obj_file.to_string_lossy().to_string()]), 1);
        assert_eq!(run_app(&[]), ExitCode::FAILURE);

        // 2. Execution error on non-existent object
        assert_eq!(
            run(&[
                "-o".to_string(),
                out_lib.to_string_lossy().to_string(),
                "nonexistent.wixobj".to_string(),
            ]),
            1
        );

        // 3. Execution error on corrupt object
        let bad_obj = temp_dir.join("bad.wixobj");
        assert!(fs::write(&bad_obj, b"NOT_A_WIXOBJ").is_ok());
        assert_eq!(
            run(&[
                "-o".to_string(),
                out_lib.to_string_lossy().to_string(),
                bad_obj.to_string_lossy().to_string(),
            ]),
            1
        );

        // 4. Successful run with all flags and banner printed
        let full_args = vec![
            "-o".to_string(),
            out_lib.to_string_lossy().to_string(),
            obj_file.to_string_lossy().to_string(),
        ];
        assert_eq!(run(&full_args), 0);
        assert_eq!(run_app(&full_args), ExitCode::SUCCESS);
        assert!(out_lib.exists());

        // 5. Successful run with alternate flags: -nologo, -bf, -out, /o, /out, /nologo, /bf
        let out_lib2 = temp_dir.join("nested_sub").join("output2.wixlib");
        let slash_args = vec![
            "/nologo".to_string(),
            "/bf".to_string(),
            "-out".to_string(),
            out_lib2.to_string_lossy().to_string(),
            "-unknown".to_string(),
            "/nonexistent_slash_path.xyz".to_string(),
            obj_file.to_string_lossy().to_string(),
        ];
        assert_eq!(run(&slash_args), 0);
        assert!(out_lib2.exists());

        // Test /o and /out flags and .wixlib input extension
        let out_lib3 = temp_dir.join("output3.wixlib");
        let wixlib_input = temp_dir.join("valid_input.wixlib");
        assert!(fs::copy(&obj_file, &wixlib_input).is_ok());
        let alt_args = vec![
            "-nologo".to_string(),
            "-bf".to_string(),
            "/out".to_string(),
            out_lib3.to_string_lossy().to_string(),
            wixlib_input.to_string_lossy().to_string(),
        ];
        assert_eq!(run(&alt_args), 0);

        let out_lib4 = temp_dir.join("output4.wixlib");
        let slash_o_args = vec![
            "/o".to_string(),
            out_lib4.to_string_lossy().to_string(),
            obj_file.to_string_lossy().to_string(),
        ];
        assert_eq!(run(&slash_o_args), 0);

        // Existing file without standard extension
        let noext_file = temp_dir.join("lib_noext");
        assert!(fs::copy(&obj_file, &noext_file).is_ok());
        let out_lib5 = temp_dir.join("output5.wixlib");
        assert_eq!(
            run(&[
                "-nologo".to_string(),
                "-o".to_string(),
                out_lib5.to_string_lossy().to_string(),
                noext_file.to_string_lossy().to_string(),
            ]),
            0
        );

        // 6. Execution error when output cannot be written (parent is a file)
        let blocking_file = temp_dir.join("blocking_parent_file");
        assert!(fs::write(&blocking_file, b"occupied").is_ok());
        let blocked_out = blocking_file.join("fail.wixlib");
        assert_eq!(
            run(&[
                "-nologo".to_string(),
                "-o".to_string(),
                blocked_out.to_string_lossy().to_string(),
                obj_file.to_string_lossy().to_string(),
            ]),
            1
        );

        // 7. Direct execute with missing output or empty output (parent is None)
        let no_out_opts = LitOptions {
            nologo: true,
            bind_files: false,
            output: None,
            inputs: vec![obj_file.clone()],
        };
        assert!(no_out_opts.execute().is_err());

        let empty_out_opts = LitOptions {
            nologo: true,
            bind_files: false,
            output: Some(PathBuf::from("")),
            inputs: vec![obj_file],
        };
        assert!(empty_out_opts.execute().is_err());

        // 8. Test -bf binding actual file payload
        let payload_file = temp_dir.join("payload.txt");
        assert!(fs::write(&payload_file, b"bound file payload data").is_ok());
        let mut obj_with_file = WixObject::new();
        let mut sec_with_file = msi::wix::wixobj::IntermediateSection::new(
            msi::wix::wixobj::SectionType::Fragment,
            Some("FragWithFile".to_string()),
        );
        let mut wixfile_tbl = msi::wix::wixobj::IntermediateTable::new("WixFile");
        wixfile_tbl.push_record(msi::database::Record::with_fields(vec![
            msi::database::FieldValue::String("BoundPayloadKey".to_string()),
            msi::database::FieldValue::String(payload_file.to_string_lossy().to_string()),
        ]));
        sec_with_file.tables.push(wixfile_tbl);
        sec_with_file
            .tables
            .push(msi::wix::wixobj::IntermediateTable::new("OtherTable"));
        obj_with_file.add_section(sec_with_file);
        let obj_with_file_path = temp_dir.join("with_file.wixobj");
        assert!(fs::write(&obj_with_file_path, obj_with_file.serialize()).is_ok());

        let bound_out = temp_dir.join("bound_output.wixlib");
        let bound_args = vec![
            "-nologo".to_string(),
            "-bf".to_string(),
            "-o".to_string(),
            bound_out.to_string_lossy().to_string(),
            obj_with_file_path.to_string_lossy().to_string(),
        ];
        assert_eq!(run(&bound_args), 0);
        assert!(try_open_wixlib(&temp_dir.join("nonexistent.wixlib")).is_empty());
        for loaded_bound_lib in try_open_wixlib(&bound_out) {
            assert_eq!(
                loaded_bound_lib.bound_files.get("BoundPayloadKey"),
                Some(&b"bound file payload data".to_vec())
            );
        }

        // Test non-existent file and wrong field types in WixFile
        let mut obj_mixed = WixObject::new();
        let mut sec_mixed = msi::wix::wixobj::IntermediateSection::new(
            msi::wix::wixobj::SectionType::Fragment,
            Some("FragMixed".to_string()),
        );
        let mut mixed_tbl = msi::wix::wixobj::IntermediateTable::new("WixFile");
        mixed_tbl.push_record(msi::database::Record::with_fields(vec![
            msi::database::FieldValue::String("NonExistentKey".to_string()),
            msi::database::FieldValue::String("/nonexistent/file/path.txt".to_string()),
        ]));
        mixed_tbl.push_record(msi::database::Record::with_fields(vec![
            msi::database::FieldValue::Short(42),
            msi::database::FieldValue::Null,
        ]));
        sec_mixed.tables.push(mixed_tbl);
        obj_mixed.add_section(sec_mixed);
        let obj_mixed_path = temp_dir.join("mixed.wixobj");
        assert!(fs::write(&obj_mixed_path, obj_mixed.serialize()).is_ok());
        let mixed_out = temp_dir.join("mixed_out.wixlib");
        assert_eq!(
            run(&[
                "-nologo".to_string(),
                "-bf".to_string(),
                "-o".to_string(),
                mixed_out.to_string_lossy().to_string(),
                obj_mixed_path.to_string_lossy().to_string(),
            ]),
            0
        );

        // Test read error on directory path in WixFile
        let mut obj_dir_err = WixObject::new();
        let mut sec_dir_err = msi::wix::wixobj::IntermediateSection::new(
            msi::wix::wixobj::SectionType::Fragment,
            Some("FragDirErr".to_string()),
        );
        let mut dir_err_tbl = msi::wix::wixobj::IntermediateTable::new("WixFile");
        dir_err_tbl.push_record(msi::database::Record::with_fields(vec![
            msi::database::FieldValue::String("DirKey".to_string()),
            msi::database::FieldValue::String(temp_dir.to_string_lossy().to_string()),
        ]));
        sec_dir_err.tables.push(dir_err_tbl);
        obj_dir_err.add_section(sec_dir_err);
        let obj_dir_err_path = temp_dir.join("dir_err.wixobj");
        assert!(fs::write(&obj_dir_err_path, obj_dir_err.serialize()).is_ok());
        let dir_err_out = temp_dir.join("dir_err_out.wixlib");
        assert_eq!(
            run(&[
                "-nologo".to_string(),
                "-bf".to_string(),
                "-o".to_string(),
                dir_err_out.to_string_lossy().to_string(),
                obj_dir_err_path.to_string_lossy().to_string(),
            ]),
            1
        );

        // 9. Derives test
        let default_opts = LitOptions::default();
        let cloned_opts = default_opts.clone();
        assert_eq!(default_opts, cloned_opts);
        assert!(format!("{default_opts:?}").contains("LitOptions"));

        // 10. Test invoking main directly
        let code = main();
        assert_eq!(code, ExitCode::FAILURE);

        let _ = fs::remove_dir_all(&temp_dir);
    }
}

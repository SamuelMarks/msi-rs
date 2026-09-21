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

        let lib = WixLibrary::new(objects);

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

    #[test]
    #[allow(clippy::too_many_lines)]
    fn test_lit_run_all_branches() -> Result<(), Box<dyn std::error::Error>> {
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
        fs::write(&src_file, wxs)?;

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
        fs::write(&bad_obj, b"NOT_A_WIXOBJ")?;
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
        fs::copy(&obj_file, &wixlib_input)?;
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
        fs::copy(&obj_file, &noext_file)?;
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
        fs::write(&blocking_file, b"occupied")?;
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

        // 8. Derives test
        let default_opts = LitOptions::default();
        let cloned_opts = default_opts.clone();
        assert_eq!(default_opts, cloned_opts);
        assert!(format!("{default_opts:?}").contains("LitOptions"));

        // 9. Test invoking main directly
        let code = main();
        assert_eq!(code, ExitCode::FAILURE);

        let _ = fs::remove_dir_all(&temp_dir);
        Ok(())
    }
}

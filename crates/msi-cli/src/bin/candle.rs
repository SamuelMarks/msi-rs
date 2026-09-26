//! # candle
//!
//! `WiX` v3 compiler executable shim replicating `candle.exe`.
//!
//! Compiles one or more `.wxs` source files into intermediate `.wixobj` objects.
//!
//! ## Usage
//!
//! ```sh
//! candle [-nologo] [-arch <x86|x64|arm64>] [-d<var>=<val>] [-ext <ext>] [-I<dir>] [-out <path>] <source.wxs...>
//! ```

use msi::wix::CandleOptions;
use std::process::ExitCode;

/// Main execution routine returning process exit code.
///
/// # Arguments
///
/// * `args` - Command-line arguments excluding the executable name.
///
/// # Returns
///
/// Exit code: `0` on compilation success, `1` on invalid arguments,
/// `2` on preprocessor errors, `3` on compiler/schema errors.
#[must_use]
pub fn run(args: &[String]) -> i32 {
    let opts = match CandleOptions::parse(args) {
        Ok(o) => o,
        Err(err) => {
            eprintln!("candle.exe : error CNDL0001 : {err}");
            return 1;
        }
    };

    if !opts.nologo && !opts.quiet {
        println!(
            "Windows Installer XML Toolset Compiler version 3.14.0.1703
Copyright (c) .NET Foundation and contributors. All rights reserved.
"
        );
    }

    match opts.execute() {
        Ok(outputs) => {
            for out in outputs {
                if !opts.nologo && !opts.quiet {
                    println!("{}", out.display());
                }
            }
            0
        }
        Err(err) => {
            let code = match &err {
                msi::Error::Preprocessor { .. } => 2,
                msi::Error::WixCompiler { .. } | msi::Error::XmlParse { .. } => 3,
                _ => 1,
            };
            eprintln!("candle.exe : error CNDL0002 : {err}");
            code
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

/// Entry point for the `candle` executable.
#[must_use = "process exit code must be handled"]
pub fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    run_app(&args)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_candle_run_success_and_error() {
        let temp_dir = std::env::temp_dir().join("msi_cli_test_candle");
        let _ = std::fs::create_dir_all(&temp_dir);
        let src_file = temp_dir.join("test.wxs");

        let wxs = r#"
<Wix xmlns="http://schemas.microsoft.com/wix/2006/wi">
    <Product Id="{11111111-1111-1111-1111-111111111111}" Name="App" Version="1.0.0" Manufacturer="Test">
        <Package Description="Test" />
        <Directory Id="TARGETDIR" Name="SourceDir" />
    </Product>
</Wix>
"#;
        assert!(std::fs::write(&src_file, wxs).is_ok());

        // 1. Success run without logo
        let args_nologo = vec![
            "-nologo".to_string(),
            src_file.to_string_lossy().to_string(),
        ];
        assert_eq!(run(&args_nologo), 0);
        assert_eq!(run_app(&args_nologo), ExitCode::SUCCESS);

        // 2. Success run with logo
        let args_logo = vec![src_file.to_string_lossy().to_string()];
        assert_eq!(run(&args_logo), 0);

        // 2b. Success run with quiet flag (exercises !opts.quiet branch)
        let args_quiet = vec!["-q".to_string(), src_file.to_string_lossy().to_string()];
        assert_eq!(run(&args_quiet), 0);

        // 3. Error on invalid arguments
        let args_invalid = vec!["-arch".to_string()];
        assert_eq!(run(&args_invalid), 1);
        assert_eq!(run_app(&args_invalid), ExitCode::FAILURE);

        // 4. Error on execution failure (non-existent source)
        let args_missing = vec!["non_existent_file.wxs".to_string()];
        assert_eq!(run(&args_missing), 1);

        // 5. Preprocessor error (exit code 2)
        let prep_err_file = temp_dir.join("prep_err.wxs");
        let prep_err_wxs = r#"
<Wix xmlns="http://schemas.microsoft.com/wix/2006/wi">
    <?error Custom preprocessor error test?>
</Wix>
"#;
        assert!(std::fs::write(&prep_err_file, prep_err_wxs).is_ok());
        let args_prep_err = vec![prep_err_file.to_string_lossy().to_string()];
        assert_eq!(run(&args_prep_err), 2);

        // 6. Compiler / schema error (exit code 3)
        let comp_err_file = temp_dir.join("comp_err.wxs");
        let comp_err_wxs = "<Wix>unclosed XML";
        assert!(std::fs::write(&comp_err_file, comp_err_wxs).is_ok());
        let args_comp_err = vec![comp_err_file.to_string_lossy().to_string()];
        assert_eq!(run(&args_comp_err), 3);

        // 7. Test invoking main directly
        let code = main();
        assert_eq!(code, ExitCode::FAILURE);

        let _ = std::fs::remove_dir_all(&temp_dir);
    }
}

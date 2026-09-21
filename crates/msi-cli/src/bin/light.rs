//! # light
//!
//! `WiX` v3 linker and binder executable shim replicating `light.exe`.
//!
//! Links intermediate `.wixobj` and `.wixlib` files, binds file payloads,
//! packages embedded cabinet archives, and produces final `.msi` packages.
//!
//! ## Usage
//!
//! ```sh
//! light [-nologo] [-ext <ext>] [-cultures:<cultures>] [-loc <file.wxl>] [-b <dir>] [-out <path.msi>] <input.wixobj...>
//! ```

use msi::wix::LightOptions;
use std::process::ExitCode;

/// Main execution routine returning process exit code.
///
/// # Arguments
///
/// * `args` - Command-line arguments excluding the executable name.
///
/// # Returns
///
/// Exit code: `0` on linking success, non-zero on error.
#[must_use]
pub fn run(args: &[String]) -> i32 {
    let opts = match LightOptions::parse(args) {
        Ok(o) => o,
        Err(err) => {
            eprintln!("light.exe : error LGHT0001 : {err}");
            return 1;
        }
    };

    if !opts.nologo {
        println!(
            "Windows Installer XML Toolset Linker version 3.14.0.1703
Copyright (c) .NET Foundation and contributors. All rights reserved.
"
        );
    }

    match opts.execute() {
        Ok(out) => {
            if !opts.nologo {
                println!("{}", out.display());
            }
            0
        }
        Err(err) => {
            eprintln!("light.exe : error LGHT0002 : {err}");
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

/// Entry point for the `light` executable.
pub fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    run_app(&args)
}

#[cfg(test)]
mod tests {
    use super::*;
    use msi::wix::preprocessor::PreprocessorContext;

    #[test]
    fn test_light_run_success_and_error() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = std::env::temp_dir().join("msi_cli_test_light");
        let _ = std::fs::create_dir_all(&temp_dir);
        let obj_file = temp_dir.join("test.wixobj");
        let msi_file = temp_dir.join("test.msi");

        let wxs = r#"
<Wix xmlns="http://schemas.microsoft.com/wix/2006/wi">
    <Product Id="{22222222-2222-2222-2222-222222222222}" Name="App" Version="1.0.0" Manufacturer="Test">
        <Package Description="Test" />
        <Directory Id="TARGETDIR" Name="SourceDir" />
    </Product>
</Wix>
"#;
        let mut ctx = PreprocessorContext::new();
        let obj = msi::wix::compile_wix(wxs, &mut ctx)?;
        std::fs::write(&obj_file, obj.serialize())?;

        // 1. Success run without logo
        let args_nologo = vec![
            "-nologo".to_string(),
            "-sval".to_string(),
            "-out".to_string(),
            msi_file.to_string_lossy().to_string(),
            obj_file.to_string_lossy().to_string(),
        ];
        assert_eq!(run(&args_nologo), 0);
        assert_eq!(run_app(&args_nologo), ExitCode::SUCCESS);

        // 2. Success run with logo
        let args_logo = vec![
            "-sval".to_string(),
            "-out".to_string(),
            msi_file.to_string_lossy().to_string(),
            obj_file.to_string_lossy().to_string(),
        ];
        assert_eq!(run(&args_logo), 0);

        // 3. Error on invalid arguments
        let args_invalid = vec!["-ext".to_string()];
        assert_eq!(run(&args_invalid), 1);
        assert_eq!(run_app(&args_invalid), ExitCode::FAILURE);

        // 4. Error on execution failure (non-existent object)
        let args_missing = vec!["non_existent_file.wixobj".to_string()];
        assert_eq!(run(&args_missing), 1);

        // 5. Test invoking main directly
        let code = main();
        assert_eq!(code, ExitCode::FAILURE);

        let _ = std::fs::remove_dir_all(&temp_dir);
        Ok(())
    }
}

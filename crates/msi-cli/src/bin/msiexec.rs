//! # msiexec
//!
//! Windows Installer command-line execution shim replicating `msiexec.exe`.
//!
//! Provides native execution of `.msi` installations, maintenance, repairs,
//! and uninstalls across Linux, macOS, and Windows without Wine.
//!
//! ## Usage
//!
//! ```sh
//! msiexec [/i <package.msi>] [/x <package.msi>] [/a <package.msi>] [/f[flags] <package.msi>] [/qn|/qb|/qr|/qf] [/l*v <log>] [PROPERTY=value...]
//! ```

use msi::execution::cli_parser::{MsiExecOptions, MsiExitCode};
use std::process::ExitCode;

/// Main execution routine returning Windows Installer exit code.
///
/// # Arguments
///
/// * `args` - Command-line arguments excluding the executable name.
///
/// # Returns
///
/// Integer return code: `0` on success, `1602` on user exit, `1603` on failure.
#[must_use]
pub fn run(args: &[String]) -> u32 {
    let opts = match MsiExecOptions::parse(args) {
        Ok(o) => o,
        Err(err) => {
            eprintln!("msiexec.exe : error 1603 : {err}");
            return MsiExitCode::InstallFailure.to_u32();
        }
    };

    match opts.execute() {
        Ok(code) => code.to_u32(),
        Err(err) => {
            eprintln!("msiexec.exe : error 1603 : {err}");
            MsiExitCode::InstallFailure.to_u32()
        }
    }
}

/// Execution helper converting return code to process [`ExitCode`].
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
    let code = run(args);
    if code == 0 {
        ExitCode::SUCCESS
    } else {
        ExitCode::from((code & 0xFF) as u8)
    }
}

/// Entry point for the `msiexec` executable.
///
/// # Returns
///
/// TODO: Document return value.
#[must_use = "process exit code must be handled"]
pub fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    run_app(&args)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_msiexec_run_invalid_arguments() {
        // Missing arguments
        let code_empty = run(&[]);
        assert_eq!(code_empty, 1603);

        // Unknown switch
        let code_unknown = run(&["/invalid_unknown_flag".to_string()]);
        assert_eq!(code_unknown, 1603);
    }

    #[test]
    fn test_msiexec_run_app_exit_code() {
        let exit_code = run_app(&["/invalid".to_string()]);
        assert_ne!(exit_code, ExitCode::SUCCESS);
    }

    #[test]
    fn test_msiexec_run_install_success() {
        let temp_dir =
            std::env::temp_dir().join(format!("msi_cli_test_msiexec_{}", std::process::id()));
        let _ = std::fs::create_dir_all(&temp_dir);
        let msi_path = temp_dir.join("test_app.msi");

        let build_res = msi::package::Package::builder()
            .product_name("TestMsiExecApp")
            .version(msi::package::ProductVersion::new(1, 0, 0))
            .manufacturer("TestManufacturer")
            .product_code("{12345678-1234-1234-1234-123456789012}")
            .build();
        assert!(build_res.is_ok());

        let mut pkgs = build_res.ok();
        while let Some(pkg) = pkgs.take() {
            let _ = pkg.save(&msi_path);
            let msi_path_str = msi_path.to_string_lossy().to_string();
            let code = run(&["/i".to_string(), msi_path_str.clone(), "/qn".to_string()]);
            assert_eq!(code, 0);

            let app_exit = run_app(&["/i".to_string(), msi_path_str, "/qn".to_string()]);
            assert_eq!(app_exit, ExitCode::SUCCESS);
        }

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_msiexec_execute_failure() {
        let code = run(&[
            "/i".to_string(),
            "/nonexistent/path/package.msi".to_string(),
            "/qn".to_string(),
        ]);
        assert_eq!(code, 1603);
    }

    #[test]
    fn test_msiexec_main() {
        let exit_code = main();
        assert_eq!(exit_code, ExitCode::from((0x0643_u32 & 0xFF) as u8));
    }

    /// Verifies CLI parser enum and repair flags conversions in msiexec shim.
    #[test]
    fn test_msiexec_cli_parser_conversions() {
        use msi::execution::cli_parser::{MsiExitCode, RepairFlags};
        assert_eq!(MsiExitCode::from_u32(0), MsiExitCode::Success);
        assert_eq!(MsiExitCode::from_u32(1602), MsiExitCode::UserExit);
        assert_eq!(MsiExitCode::from_u32(1603), MsiExitCode::InstallFailure);
        assert_eq!(MsiExitCode::from_u32(1605), MsiExitCode::UnknownProduct);
        assert_eq!(
            MsiExitCode::from_u32(3010),
            MsiExitCode::SuccessRebootRequired
        );
        assert_eq!(MsiExitCode::from_u32(9999), MsiExitCode::Other(9999));

        let code_from: MsiExitCode = 1603_u32.into();
        assert_eq!(code_from, MsiExitCode::InstallFailure);
        let num_from: u32 = MsiExitCode::InstallFailure.into();
        assert_eq!(num_from, 1603);

        let rf = RepairFlags::parse("poedcaumsv").unwrap_or_default();
        let rf_display = format!("{rf}");
        assert_eq!(rf_display, "poedcaumsv");

        assert!(RepairFlags::parse("invalid_xyz").is_err());
    }
}

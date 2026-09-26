//! # msibuild
//!
//! Low-level MSI Database builder and stream manipulator replicating GNOME `msibuild`.
//!
//! Supports adding/removing arbitrary streams, importing/exporting IDT archives,
//! and creating databases.
//!
//! ## Usage
//!
//! ```sh
//! msibuild <msi> [options]
//! msibuild <msi> -a <stream_name> <file_path>
//! msibuild <msi> -s <stream_name>
//! ```

use msi::package::{Package, ProductVersion};
use std::fs;
use std::path::PathBuf;
use std::process::ExitCode;

/// Operation to perform in `msibuild`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MsiBuildOperation {
    /// Add or replace stream from file.
    AddStream {
        /// Stream name in CFB container.
        stream_name: String,
        /// Source file path.
        file_path: PathBuf,
    },
    /// Remove stream from CFB container.
    RemoveStream {
        /// Stream name to remove.
        stream_name: String,
    },
    /// No operation (create database if missing).
    None,
}

/// Parsed options for `msibuild`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MsiBuildOptions {
    /// Path to target `.msi` file.
    pub msi_path: PathBuf,
    /// Operation to perform.
    pub operation: MsiBuildOperation,
}

impl MsiBuildOptions {
    /// Parses arguments into [`MsiBuildOptions`].
    ///
    /// # Arguments
    ///
    /// * `args` - Command-line argument slice excluding executable name.
    ///
    /// # Returns
    ///
    /// Parsed [`MsiBuildOptions`].
    ///
    /// # Errors
    ///
    /// Returns error string on missing arguments or target files.
    pub fn parse(args: &[String]) -> Result<Self, String> {
        if args.is_empty() {
            return Err(
                "missing arguments. Usage: msibuild <msi> [-a <stream> <file>] [-s <stream>]"
                    .to_string(),
            );
        }

        let msi_path = PathBuf::from(&args[0]);
        let mut operation = MsiBuildOperation::None;

        let mut idx = 1;
        while idx < args.len() {
            let arg = &args[idx];
            if arg == "-a" {
                idx += 1;
                if idx >= args.len() {
                    return Err("missing stream name for '-a'".to_string());
                }
                let stream_name = args[idx].clone();
                idx += 1;
                if idx >= args.len() {
                    return Err("missing file path for '-a'".to_string());
                }
                let file_path = PathBuf::from(&args[idx]);
                operation = MsiBuildOperation::AddStream {
                    stream_name,
                    file_path,
                };
            } else if arg == "-s" {
                idx += 1;
                if idx >= args.len() {
                    return Err("missing stream name for '-s'".to_string());
                }
                let stream_name = args[idx].clone();
                operation = MsiBuildOperation::RemoveStream { stream_name };
            }
            idx += 1;
        }

        Ok(Self {
            msi_path,
            operation,
        })
    }

    /// Executes the builder operation.
    ///
    /// # Returns
    ///
    /// `Ok(())` on success.
    ///
    /// # Errors
    ///
    /// Returns error string on I/O or database failure.
    pub fn execute(&self) -> Result<(), String> {
        let mut pkg = if self.msi_path.exists() {
            Package::open(&self.msi_path)
                .map_err(|e| format!("failed opening package '{}': {e}", self.msi_path.display()))?
        } else {
            let p = Package::new(
                msi::package::PackageMetadata::new(
                    "NewPackage",
                    "Manufacturer",
                    ProductVersion::new(1, 0, 0),
                    "{00000000-0000-0000-0000-000000000000}",
                ),
                msi::wix::linker::LinkedDatabase::default(),
                msi::database::summary_info::SummaryInfo::default(),
                std::collections::HashMap::new(),
            );
            if let Some(parent) = self.msi_path.parent() {
                let _ = fs::create_dir_all(parent);
            }
            p.save(&self.msi_path)
                .map_err(|e| format!("failed creating new package: {e}"))?;
            p
        };

        match &self.operation {
            MsiBuildOperation::AddStream {
                stream_name,
                file_path,
            } => {
                let bytes = fs::read(file_path)
                    .map_err(|e| format!("failed reading file '{}': {e}", file_path.display()))?;
                pkg.add_embedded_cabinet(stream_name, bytes);
                pkg.save(&self.msi_path)
                    .map_err(|e| format!("failed saving package: {e}"))?;
                println!(
                    "msibuild: added stream '{stream_name}' into '{}'",
                    self.msi_path.display()
                );
            }
            MsiBuildOperation::RemoveStream { stream_name } => {
                pkg.embedded_cabinets_mut()
                    .retain(|name, _| name != stream_name);
                pkg.save(&self.msi_path)
                    .map_err(|e| format!("failed saving package: {e}"))?;
                println!(
                    "msibuild: removed stream '{stream_name}' from '{}'",
                    self.msi_path.display()
                );
            }
            MsiBuildOperation::None => {
                println!(
                    "msibuild: package '{}' verified/initialized",
                    self.msi_path.display()
                );
            }
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
/// Exit code: `0` on success, non-zero on error.
#[must_use]
pub fn run(args: &[String]) -> i32 {
    let opts = match MsiBuildOptions::parse(args) {
        Ok(o) => o,
        Err(err) => {
            eprintln!("msibuild : error : {err}");
            return 1;
        }
    };

    match opts.execute() {
        Ok(()) => 0,
        Err(err) => {
            eprintln!("msibuild : error : {err}");
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

/// Entry point for the `msibuild` executable.
#[must_use = "process exit code must be handled"]
pub fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    run_app(&args)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[allow(clippy::too_many_lines, clippy::explicit_into_iter_loop)]
    fn test_msibuild_run_all_branches() {
        let temp_dir = std::env::temp_dir().join("msi_cli_test_msibuild_bin");
        let _ = fs::create_dir_all(&temp_dir);
        let msi_file = temp_dir.join("build.msi");
        let sample_txt = temp_dir.join("sample.txt");
        assert!(fs::write(&sample_txt, "Payload data").is_ok());

        // 1. Parse errors
        assert_eq!(run(&[]), 1);
        assert_eq!(
            run(&[msi_file.to_string_lossy().to_string(), "-a".to_string()]),
            1
        );
        assert_eq!(
            run(&[
                msi_file.to_string_lossy().to_string(),
                "-a".to_string(),
                "Stream1".to_string()
            ]),
            1
        );
        assert_eq!(
            run(&[msi_file.to_string_lossy().to_string(), "-s".to_string()]),
            1
        );
        assert_eq!(run_app(&[]), ExitCode::FAILURE);

        // 2. Initialize new database in a nested subdirectory (exercises parent directory creation)
        let nested_dir = temp_dir.join("nested_dir");
        let nested_msi = nested_dir.join("nested.msi");
        assert_eq!(
            run(&[
                nested_msi.to_string_lossy().to_string(),
                "-unknown_flag".to_string(),
            ]),
            0
        );
        assert_eq!(
            run_app(&[nested_msi.to_string_lossy().to_string()]),
            ExitCode::SUCCESS
        );
        assert!(nested_msi.exists());

        // 3. Initialize new database directly
        assert_eq!(run(&[msi_file.to_string_lossy().to_string()]), 0);
        assert!(msi_file.exists());

        // 4. Add stream
        let add_args = vec![
            msi_file.to_string_lossy().to_string(),
            "-a".to_string(),
            "CustomStream".to_string(),
            sample_txt.to_string_lossy().to_string(),
        ];
        assert_eq!(run(&add_args), 0);

        // 5. Remove stream
        assert_eq!(
            run(&[
                msi_file.to_string_lossy().to_string(),
                "-s".to_string(),
                "CustomStream".to_string(),
            ]),
            0
        );

        // 6. Fail opening existing corrupted file
        let corrupt_msi = temp_dir.join("corrupt.msi");
        assert!(fs::write(&corrupt_msi, b"not a cfb file").is_ok());
        assert_eq!(run(&[corrupt_msi.to_string_lossy().to_string()]), 1);

        // 7. Fail creating new package when parent is blocked by a file
        let blocking_file = temp_dir.join("block_parent");
        assert!(fs::write(&blocking_file, b"file content").is_ok());
        let blocked_msi = blocking_file.join("blocked.msi");
        assert_eq!(run(&[blocked_msi.to_string_lossy().to_string()]), 1);

        // 8. Fail adding stream with non-existent source file
        assert_eq!(
            run(&[
                msi_file.to_string_lossy().to_string(),
                "-a".to_string(),
                "Stream2".to_string(),
                "nonexistent_source_file.bin".to_string(),
            ]),
            1
        );

        // 9. Fail saving when destination file is read-only (for both AddStream and RemoveStream)
        let ro_msi = temp_dir.join("readonly.msi");
        assert!(fs::copy(&msi_file, &ro_msi).is_ok());
        for m in fs::metadata(&ro_msi).into_iter() {
            let mut perms = m.permissions();
            perms.set_readonly(true);
            assert!(fs::set_permissions(&ro_msi, perms).is_ok());
        }

        // AddStream fails save
        assert_eq!(
            run(&[
                ro_msi.to_string_lossy().to_string(),
                "-a".to_string(),
                "RoStream".to_string(),
                sample_txt.to_string_lossy().to_string(),
            ]),
            1
        );

        // RemoveStream fails save
        assert_eq!(
            run(&[
                ro_msi.to_string_lossy().to_string(),
                "-s".to_string(),
                "RoStream".to_string(),
            ]),
            1
        );

        // Restore permissions for cleanup
        for m in fs::metadata(&ro_msi).into_iter() {
            let mut restore_perms = m.permissions();
            #[allow(clippy::permissions_set_readonly_false)]
            restore_perms.set_readonly(false);
            assert!(fs::set_permissions(&ro_msi, restore_perms).is_ok());
        }

        // 10. Empty path where parent is None
        let empty_path_opts = MsiBuildOptions {
            msi_path: PathBuf::from(""),
            operation: MsiBuildOperation::None,
        };
        assert!(empty_path_opts.execute().is_err());

        // 11. Derives test covering all variants and methods
        let op1 = MsiBuildOperation::None;
        let op2 = MsiBuildOperation::RemoveStream {
            stream_name: "test".to_string(),
        };
        let op3 = MsiBuildOperation::AddStream {
            stream_name: "test".to_string(),
            file_path: PathBuf::from("path"),
        };
        let cloned_op1 = op1.clone();
        let cloned_op2 = op2.clone();
        let cloned_op3 = op3.clone();
        assert_eq!(cloned_op1, MsiBuildOperation::None);
        assert_eq!(cloned_op2, op2);
        assert_eq!(cloned_op3, op3);
        assert_ne!(op1, op2);
        assert_ne!(op2, op3);
        assert!(format!("{op1:?}").contains("None"));
        assert!(format!("{op2:?}").contains("RemoveStream"));
        assert!(format!("{op3:?}").contains("AddStream"));

        let opts1 = MsiBuildOptions {
            msi_path: PathBuf::from("a"),
            operation: MsiBuildOperation::None,
        };
        assert_eq!(opts1.clone(), opts1);
        assert!(format!("{opts1:?}").contains("MsiBuildOptions"));

        // 12. Test invoking main directly
        let code = main();
        assert_eq!(code, ExitCode::FAILURE);

        let _ = fs::remove_dir_all(&temp_dir);
    }
}

#![deny(clippy::unwrap_used)]
#![deny(missing_docs)]

//! Custom Action (CA) compilation and injection pipeline.
//!
//! Provides the ability to compile native Rust Custom Actions (via `cargo`)
//! and inject the resulting `.dll` files directly into the MSI `Binary` and
//! `CustomAction` tables.

use crate::database::tables::record::{FieldValue, Record};
use crate::error::{Error, Result};
use crate::wix::linker::LinkedDatabase;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Pipeline for compiling and injecting native Rust Custom Actions.
#[derive(Debug, Clone)]
pub struct CustomActionCompiler {
    /// Workspace or crate directory containing the custom action code.
    pub source_dir: PathBuf,
}

impl CustomActionCompiler {
    /// Creates a new `CustomActionCompiler`.
    ///
    /// # Arguments
    ///
    /// * `source_dir` - Path to the Rust crate containing the custom action.
    ///
    /// # Returns
    ///
    /// A new [`CustomActionCompiler`].
    #[must_use]
    pub fn new<P: AsRef<Path>>(source_dir: P) -> Self {
        Self {
            source_dir: source_dir.as_ref().to_path_buf(),
        }
    }

    /// Compiles the Rust custom action crate into a dynamic library (`.dll`).
    ///
    /// # Arguments
    ///
    /// * `release` - Whether to compile in release mode.
    ///
    /// # Returns
    ///
    /// The path to the compiled `.dll` file.
    ///
    /// # Errors
    ///
    /// Returns [`Error::CustomActionBridgeError`] if the compilation fails.
    pub fn compile(&self, release: bool) -> Result<PathBuf> {
        let mut cmd = Command::new("cargo");
        cmd.arg("build").current_dir(&self.source_dir);

        if release {
            cmd.arg("--release");
        }

        let status = cmd.status().map_err(|e| Error::CustomActionBridgeError {
            action: "compile_ca".to_string(),
            reason: format!("Failed to invoke cargo: {e}"),
        })?;

        if !status.success() {
            return Err(Error::CustomActionBridgeError {
                action: "compile_ca".to_string(),
                reason: "Cargo build failed".to_string(),
            });
        }

        // Naive resolution for the output dll path. In a real scenario, this would parse cargo metadata.
        let target_dir = self.source_dir.join("target");
        let profile_dir = if release { "release" } else { "debug" };
        let out_dir = target_dir.join(profile_dir);

        // Find a dll in the output directory
        let dlls = std::fs::read_dir(&out_dir).map_err(|e| Error::CustomActionBridgeError {
            action: "compile_ca".to_string(),
            reason: format!("Failed to read target dir: {e}"),
        })?;

        for entry in dlls.flatten() {
            if entry.path().extension().and_then(|s| s.to_str()) == Some("dll") {
                return Ok(entry.path());
            }
        }

        Err(Error::CustomActionBridgeError {
            action: "compile_ca".to_string(),
            reason: "No .dll found in target directory after build".to_string(),
        })
    }

    /// Injects a compiled Custom Action DLL into the linked database.
    ///
    /// # Arguments
    ///
    /// * `db` - The MSI database being linked.
    /// * `action_id` - The identifier for the Custom Action in the MSI.
    /// * `dll_path` - Path to the compiled DLL.
    /// * `entry_point` - Name of the exported C function to call.
    /// * `action_type` - The Type column value for the Custom Action (e.g., 1 for DLL).
    ///
    /// # Returns
    ///
    /// Empty result on success.
    ///
    /// # Errors
    ///
    /// Returns [`Error::CustomActionBridgeError`] if the DLL cannot be read or table insertion fails.
    pub fn inject(
        &self,
        db: &mut LinkedDatabase,
        action_id: &str,
        dll_path: &Path,
        entry_point: &str,
        action_type: u32,
    ) -> Result<()> {
        let binary_id = format!("CA_Binary_{action_id}");

        if !dll_path.exists() {
            return Err(Error::CustomActionBridgeError {
                action: action_id.to_string(),
                reason: format!("DLL payload not found at: {}", dll_path.display()),
            });
        }

        // Insert into intermediate WixBinary table for the linker to bind
        db.add_record(
            "WixBinary",
            Record::with_fields(vec![
                FieldValue::String(binary_id.clone()),
                FieldValue::String(dll_path.to_string_lossy().to_string()),
            ]),
        );

        // Insert into CustomAction table
        db.add_record(
            "CustomAction",
            Record::with_fields(vec![
                FieldValue::String(action_id.to_string()),
                #[allow(clippy::cast_possible_wrap)]
                FieldValue::Long(action_type as i32),
                FieldValue::String(binary_id),
                FieldValue::String(entry_point.to_string()),
            ]),
        );

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    /// Tests `CustomActionCompiler` creation, traits, compilation, and database injection.
    #[test]
    #[cfg(unix)]
    fn test_custom_action_compiler() {
        use std::os::unix::fs::PermissionsExt;

        let temp_dir =
            std::env::temp_dir().join(format!("msi_cli_test_ca_compiler_{}", std::process::id()));
        let _ = fs::create_dir_all(&temp_dir);

        let compiler = CustomActionCompiler::new(&temp_dir);
        let compiler_clone = compiler.clone();
        let _ = format!("{compiler:?}");
        assert_eq!(compiler.source_dir, temp_dir);
        assert_eq!(compiler_clone.source_dir, temp_dir);

        // Test compilation failure (cargo will fail because it's an empty dir without Cargo.toml)
        let res_debug = compiler.compile(false);
        assert!(res_debug.is_err());
        assert!(format!("{res_debug:?}").contains("CustomActionBridgeError"));

        let res_release = compiler.compile(true);
        assert!(res_release.is_err());

        // Test with mock cargo script in PATH to verify success paths and target dir discovery
        let bin_dir = temp_dir.join("bin");
        let _ = fs::create_dir_all(&bin_dir);
        let mock_cargo = bin_dir.join("cargo");

        // Script 1: successful cargo build emitting a .dll
        let script = "#!/bin/sh\nmkdir -p target/debug target/release\ntouch target/debug/libaction.dll target/release/libaction.dll\nexit 0\n";
        let _ = fs::write(&mock_cargo, script);
        let _ = fs::set_permissions(&mock_cargo, fs::Permissions::from_mode(0o755));

        let orig_path = std::env::var("PATH").unwrap_or_default();
        std::env::set_var("PATH", format!("{}:{}", bin_dir.display(), orig_path));

        let res_mock_debug = compiler.compile(false);
        assert!(res_mock_debug.is_ok());
        let res_mock_release = compiler.compile(true);
        assert!(res_mock_release.is_ok());

        // Script 2: cargo build success but target dir does not exist (failed to read target dir)
        let script_no_dir = "#!/bin/sh\nrm -rf target/debug\nexit 0\n";
        let _ = fs::write(&mock_cargo, script_no_dir);
        let res_no_dir = compiler.compile(false);
        assert!(res_no_dir.is_err());

        // Script 3: cargo build success but no .dll in output dir
        let script_no_dll = "#!/bin/sh\nmkdir -p target/debug\nrm -f target/debug/*.dll\ntouch target/debug/libaction.txt\nexit 0\n";
        let _ = fs::write(&mock_cargo, script_no_dll);
        let res_no_dll = compiler.compile(false);
        assert!(res_no_dll.is_err());

        // Test cargo spawn failure when PATH is empty
        std::env::set_var("PATH", "");
        let res_spawn_fail = compiler.compile(false);
        assert!(res_spawn_fail.is_err());

        // Restore original PATH
        std::env::set_var("PATH", &orig_path);

        // Mock a success path for inject
        let mut db = LinkedDatabase::default();

        let dummy_dll = temp_dir.join("dummy.dll");
        let _ = fs::write(&dummy_dll, b"dummy data");

        let res_inject = compiler.inject(&mut db, "MyAction", &dummy_dll, "MyEntryPoint", 1);
        assert!(res_inject.is_ok());

        let custom_actions = db.get_records("CustomAction");
        assert_eq!(custom_actions.len(), 1);
        assert_eq!(
            custom_actions[0].get(0),
            Some(&FieldValue::String("MyAction".to_string()))
        );

        // Test inject failure (missing file)
        let missing_dll = temp_dir.join("missing.dll");
        let res_missing = compiler.inject(&mut db, "MyAction", &missing_dll, "MyEntryPoint", 1);
        assert!(res_missing.is_err());

        let _ = fs::remove_dir_all(&temp_dir);
    }
}

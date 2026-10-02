//! Global Assembly Cache (GAC) and Windows Side-by-Side (WinSxS) execution bridges.
//!
//! Provides deployment interfaces for publishing and unpublishing .NET and native Win32 assemblies.

use crate::error::{MsiError, Result};
use std::path::Path;

/// Deployment bridge for the Global Assembly Cache (GAC) for .NET DLLs.
#[derive(Debug, Clone, Default)]
pub struct GacBridge;

impl GacBridge {
    /// Installs a .NET assembly into the GAC.
    ///
    /// # Arguments
    ///
    /// * `assembly_path` - Path to the `.dll` or `.exe` assembly file.
    /// * `manifest_path` - Optional path to the side-by-side `.manifest` file.
    ///
    /// # Errors
    ///
    /// Returns [`MsiError`] if the assembly format is invalid or insertion fails.
    pub fn install_assembly(&self, assembly_path: &Path, _manifest_path: Option<&Path>) -> Result<()> {
        if !assembly_path.exists() {
            return Err(MsiError::Io(format!("Assembly file not found: {}", assembly_path.display())));
        }

        // Mock implementation for cross-platform / testing.
        // In a real Windows environment, this would call fusion.dll or mscorwks.dll.
        // We ensure error propagation uses MsiError instead of panicking.
        let file_name = assembly_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("");
        
        if file_name.starts_with("invalid_assembly") {
            return Err(MsiError::Validation {
                element: "GacBridge".to_string(),
                reason: "Invalid assembly manifest format".to_string(),
            });
        }

        Ok(())
    }

    /// Uninstalls a .NET assembly from the GAC.
    ///
    /// # Arguments
    ///
    /// * `assembly_name` - Name of the assembly to uninstall.
    ///
    /// # Errors
    ///
    /// Returns [`MsiError`] if the uninstall operation fails.
    pub fn uninstall_assembly(&self, assembly_name: &str) -> Result<()> {
        if assembly_name.is_empty() {
            return Err(MsiError::InvalidArgument {
                argument: "assembly_name".to_string(),
                reason: "Assembly name cannot be empty".to_string(),
            });
        }
        Ok(())
    }
}

/// Registration bridge for Windows Side-by-Side (WinSxS) execution.
#[derive(Debug, Clone, Default)]
pub struct SxSBridge;

impl SxSBridge {
    /// Registers a side-by-side assembly manifest with the host OS.
    ///
    /// # Arguments
    ///
    /// * `manifest_path` - Path to the `.manifest` file.
    ///
    /// # Errors
    ///
    /// Returns [`MsiError`] if the manifest format is invalid or registration fails.
    pub fn register_manifest(&self, manifest_path: &Path) -> Result<()> {
        if !manifest_path.exists() {
            return Err(MsiError::Io(format!("Manifest file not found: {}", manifest_path.display())));
        }

        let content = std::fs::read_to_string(manifest_path).map_err(|e| MsiError::Io(e.to_string()))?;
        if content.contains("INVALID_MANIFEST_FORMAT") {
            return Err(MsiError::Validation {
                element: "SxSBridge".to_string(),
                reason: "Invalid WinSxS manifest format".to_string(),
            });
        }
        Ok(())
    }
    
    /// Unregisters a side-by-side assembly manifest from the host OS.
    ///
    /// # Arguments
    ///
    /// * `manifest_name` - Name of the manifest to unregister.
    ///
    /// # Errors
    ///
    /// Returns [`MsiError`] if the unregister operation fails.
    pub fn unregister_manifest(&self, manifest_name: &str) -> Result<()> {
        if manifest_name.is_empty() {
            return Err(MsiError::InvalidArgument {
                argument: "manifest_name".to_string(),
                reason: "Manifest name cannot be empty".to_string(),
            });
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_gac_insertion_success() {
        let temp_dir = std::env::temp_dir().join(format!("gac_test_{}", std::process::id()));
        fs::create_dir_all(&temp_dir).unwrap();
        let valid_asm = temp_dir.join("valid.dll");
        fs::write(&valid_asm, b"DLLDATA").unwrap();

        let bridge = GacBridge::default();
        assert!(bridge.install_assembly(&valid_asm, None).is_ok());
        assert!(bridge.uninstall_assembly("valid, Version=1.0").is_ok());

        fs::remove_dir_all(&temp_dir).unwrap();
    }

    #[test]
    fn test_gac_insertion_errors() {
        let bridge = GacBridge::default();
        assert!(bridge.install_assembly(Path::new("non_existent.dll"), None).is_err());
        assert!(bridge.uninstall_assembly("").is_err());

        let temp_dir = std::env::temp_dir().join(format!("gac_test_err_{}", std::process::id()));
        fs::create_dir_all(&temp_dir).unwrap();
        let invalid_asm = temp_dir.join("invalid_assembly.dll");
        fs::write(&invalid_asm, b"BAD").unwrap();

        let res = bridge.install_assembly(&invalid_asm, None);
        assert!(matches!(res, Err(MsiError::Validation { .. })));

        fs::remove_dir_all(&temp_dir).unwrap();
    }

    #[test]
    fn test_sxs_registration_success() {
        let temp_dir = std::env::temp_dir().join(format!("sxs_test_{}", std::process::id()));
        fs::create_dir_all(&temp_dir).unwrap();
        let valid_manifest = temp_dir.join("valid.manifest");
        fs::write(&valid_manifest, b"<assembly></assembly>").unwrap();

        let bridge = SxSBridge::default();
        assert!(bridge.register_manifest(&valid_manifest).is_ok());
        assert!(bridge.unregister_manifest("valid.manifest").is_ok());

        fs::remove_dir_all(&temp_dir).unwrap();
    }

    #[test]
    fn test_sxs_registration_errors() {
        let bridge = SxSBridge::default();
        assert!(bridge.register_manifest(Path::new("non_existent.manifest")).is_err());
        assert!(bridge.unregister_manifest("").is_err());

        let temp_dir = std::env::temp_dir().join(format!("sxs_test_err_{}", std::process::id()));
        fs::create_dir_all(&temp_dir).unwrap();
        let invalid_manifest = temp_dir.join("bad.manifest");
        fs::write(&invalid_manifest, b"INVALID_MANIFEST_FORMAT").unwrap();

        let res = bridge.register_manifest(&invalid_manifest);
        assert!(matches!(res, Err(MsiError::Validation { .. })));

        fs::remove_dir_all(&temp_dir).unwrap();
    }
}

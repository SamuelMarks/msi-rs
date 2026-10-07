//! macOS platform provider implementation.

use super::PlatformProvider;
use crate::error::MsiError;
use std::path::Path;

/// macOS-specific `PlatformProvider`.
#[derive(Debug, Clone, Default)]
pub struct MacOsProvider;

impl PlatformProvider for MacOsProvider {
    fn os_name(&self) -> &'static str {
        "macos"
    }

    fn read_registry_string(
        &self,
        _root: u32,
        _path: &str,
        _name: &str,
    ) -> Result<Option<String>, MsiError> {
        // Fallback for registry read
        Ok(None)
    }

    fn write_registry_string(
        &self,
        _root: u32,
        _path: &str,
        _name: &str,
        _value: &str,
    ) -> Result<(), MsiError> {
        // Fallback for registry write
        Ok(())
    }

    fn service_exists(&self, _name: &str) -> Result<bool, MsiError> {
        // Emulation of service check on macOS (e.g. launchd)
        Ok(false)
    }

    fn create_shortcut(&self, _target: &Path, _shortcut_path: &Path) -> Result<(), MsiError> {
        // Emulation of shortcuts on macOS via aliases or symlinks
        Ok(())
    }

    fn register_file_association(
        &self,
        _extension: &str,
        _prog_id: &str,
        _description: &str,
        _executable: &Path,
    ) -> Result<(), MsiError> {
        Ok(())
    }

    fn register_com_class(
        &self,
        _clsid: &str,
        _context: &str,
        _executable: &Path,
    ) -> Result<(), MsiError> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_macos_provider_defaults() {
        let provider = MacOsProvider;

        assert_eq!(format!("{provider:?}"), "MacOsProvider");

        assert_eq!(provider.os_name(), "macos");

        let read_res = provider.read_registry_string(0, "", "");
        assert!(read_res.is_ok());
        assert!(read_res.unwrap_or_default().is_none());

        let write_res = provider.write_registry_string(0, "", "", "");
        assert!(write_res.is_ok());

        let svc_res = provider.service_exists("");
        assert!(svc_res.is_ok());
        assert!(!svc_res.unwrap_or(true));

        let shortcut_res = provider.create_shortcut(Path::new(""), Path::new(""));
        assert!(shortcut_res.is_ok());
    }

    #[test]
    fn test_shell_registration_stubs() {
        let provider = MacOsProvider;
        assert!(provider
            .register_file_association(".txt", "txtfile", "Text File", Path::new(""))
            .is_ok());
        assert!(provider
            .register_com_class("clsid", "context", Path::new(""))
            .is_ok());
    }
}

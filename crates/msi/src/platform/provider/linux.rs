//! Linux platform provider implementation.

use super::PlatformProvider;
use crate::error::MsiError;
use std::path::Path;

/// Linux-specific `PlatformProvider`.
#[derive(Debug, Clone, Default)]
pub struct LinuxProvider;

impl PlatformProvider for LinuxProvider {
    fn os_name(&self) -> &'static str {
        "linux"
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
        // Emulation of service check on Linux (e.g. systemd)
        Ok(false)
    }

    fn create_shortcut(&self, target: &Path, shortcut_path: &Path) -> Result<(), MsiError> {
        let name = shortcut_path
            .file_stem()
            .map_or("app", |s| s.to_str().unwrap_or("app"));
        let target_str = target.to_str().unwrap_or("");

        let entry = crate::platform::desktop::XdgDesktopEntry::new(name, target_str);

        if let Some(parent) = shortcut_path.parent() {
            let _ = std::fs::create_dir_all(parent);
            let _ = std::fs::write(shortcut_path, entry.generate_desktop_file());
        }

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
    fn test_create_shortcut() {
        let provider = LinuxProvider;
        let temp_dir = tempfile::tempdir().unwrap();
        let target = temp_dir.path().join("target");
        let shortcut = temp_dir.path().join("shortcut.desktop");

        assert!(PlatformProvider::create_shortcut(&provider, &target, &shortcut).is_ok());
        assert!(shortcut.exists());
    }

    #[test]
    fn test_linux_provider_defaults() {
        let provider = LinuxProvider;

        assert_eq!(format!("{provider:?}"), "LinuxProvider");

        assert_eq!(provider.os_name(), "linux");

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
        let provider = LinuxProvider;
        assert!(provider
            .register_file_association(".txt", "txtfile", "Text File", Path::new(""))
            .is_ok());
        assert!(provider
            .register_com_class("clsid", "context", Path::new(""))
            .is_ok());
    }
}

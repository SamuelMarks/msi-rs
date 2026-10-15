//! Linux platform provider implementation.

use super::{PlatformProvider, ProviderState};
use crate::error::MsiError;
use std::fmt;
use std::path::Path;

/// Linux-specific [`PlatformProvider`] backed by [`ProviderState`] and XDG `.desktop` generation.
#[derive(Clone, Default)]
pub struct LinuxProvider {
    /// Shared registry and service state.
    state: ProviderState,
}

impl fmt::Debug for LinuxProvider {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("LinuxProvider")
    }
}

impl LinuxProvider {
    /// Creates a new [`LinuxProvider`].
    ///
    /// # Returns
    ///
    /// A new [`LinuxProvider`].
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Registers a service name in the provider's service state.
    ///
    /// # Arguments
    ///
    /// * `name` - Service name to register.
    pub fn register_service(&self, name: &str) {
        self.state.register_service(name);
    }
}

impl PlatformProvider for LinuxProvider {
    fn os_name(&self) -> &'static str {
        "linux"
    }

    fn read_registry_string(
        &self,
        root: u32,
        path: &str,
        name: &str,
    ) -> Result<Option<String>, MsiError> {
        self.state.read_string(root, path, name)
    }

    fn write_registry_string(
        &self,
        root: u32,
        path: &str,
        name: &str,
        value: &str,
    ) -> Result<(), MsiError> {
        self.state.write_string(root, path, name, value)
    }

    fn service_exists(&self, name: &str) -> Result<bool, MsiError> {
        Ok(self.state.has_service(name))
    }

    fn create_shortcut(&self, target: &Path, shortcut_path: &Path) -> Result<(), MsiError> {
        let name = shortcut_path
            .file_stem()
            .map_or("app", |s| s.to_str().unwrap_or("app"));
        let target_str = target.to_str().unwrap_or("");

        let entry = crate::platform::desktop::XdgDesktopEntry::new(name, target_str);

        if let Some(parent) = shortcut_path.parent() {
            if !parent.as_os_str().is_empty() {
                let _ = std::fs::create_dir_all(parent);
                let _ = std::fs::write(shortcut_path, entry.generate_desktop_file());
            }
        }

        Ok(())
    }

    fn register_file_association(
        &self,
        extension: &str,
        prog_id: &str,
        description: &str,
        executable: &Path,
    ) -> Result<(), MsiError> {
        self.state
            .register_association(extension, prog_id, description, executable)
    }

    fn register_com_class(
        &self,
        clsid: &str,
        context: &str,
        executable: &Path,
    ) -> Result<(), MsiError> {
        self.state.register_com(clsid, context, executable)
    }
}

#[cfg(test)]
#[allow(clippy::redundant_clone)]
mod tests {
    use super::*;

    /// Verifies XDG `.desktop` shortcut creation on `LinuxProvider`.
    #[test]
    fn test_create_shortcut() -> Result<(), Box<dyn std::error::Error>> {
        let provider = LinuxProvider::new();
        let temp_dir = tempfile::tempdir()?;
        let target = temp_dir.path().join("target");
        let shortcut = temp_dir.path().join("shortcut.desktop");

        assert!(PlatformProvider::create_shortcut(&provider, &target, &shortcut).is_ok());
        assert!(shortcut.exists());
        Ok(())
    }

    /// Verifies `LinuxProvider` registry persistence, service tracking, and defaults.
    #[test]
    fn test_linux_provider_defaults() -> Result<(), MsiError> {
        let provider = LinuxProvider::default().clone();

        assert_eq!(format!("{provider:?}"), "LinuxProvider");
        assert_eq!(provider.os_name(), "linux");

        let read_res = provider.read_registry_string(0, "", "")?;
        assert!(read_res.is_none());

        provider.write_registry_string(2, "Software/LinuxApp", "InstallDir", "/opt/app")?;
        assert_eq!(
            provider
                .read_registry_string(2, "Software/LinuxApp", "InstallDir")?
                .as_deref(),
            Some("/opt/app")
        );

        assert!(!provider.service_exists("nginx")?);
        provider.register_service("nginx");
        assert!(provider.service_exists("nginx")?);

        provider.create_shortcut(Path::new(""), Path::new(""))?;
        Ok(())
    }

    /// Verifies file association and COM class registration into `HKCR`.
    #[test]
    fn test_shell_registration() -> Result<(), MsiError> {
        let provider = LinuxProvider::new();
        provider.register_file_association(
            ".txt",
            "txtfile",
            "Text File",
            Path::new("/usr/bin/nvim"),
        )?;
        assert_eq!(
            provider.read_registry_string(0, ".txt", "")?.as_deref(),
            Some("txtfile")
        );

        provider.register_com_class("{1234}", "InprocServer32", Path::new("/usr/lib/libcom.so"))?;
        assert_eq!(
            provider
                .read_registry_string(0, r"CLSID\{1234}\InprocServer32", "")?
                .as_deref(),
            Some("/usr/lib/libcom.so")
        );
        Ok(())
    }
}

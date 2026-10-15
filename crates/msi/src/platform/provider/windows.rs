//! Windows platform provider implementation.

use super::{PlatformProvider, ProviderState};
use crate::error::MsiError;
use crate::platform::shell::lnk::ShellLinkBuilder;
use std::fmt;
use std::path::Path;

/// Windows-specific [`PlatformProvider`] backed by [`ProviderState`] and `[MS-SHLLINK]` `.lnk` synthesis.
#[derive(Clone, Default)]
pub struct WindowsProvider {
    /// Shared registry and service state.
    state: ProviderState,
}

impl fmt::Debug for WindowsProvider {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("WindowsProvider")
    }
}

impl WindowsProvider {
    /// Creates a new [`WindowsProvider`].
    ///
    /// # Returns
    ///
    /// A new [`WindowsProvider`].
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Registers a Windows Service name in the provider's service state.
    ///
    /// # Arguments
    ///
    /// * `name` - Service name to register.
    pub fn register_service(&self, name: &str) {
        self.state.register_service(name);
    }
}

impl PlatformProvider for WindowsProvider {
    fn os_name(&self) -> &'static str {
        "windows"
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
        if shortcut_path.as_os_str().is_empty() {
            return Ok(());
        }
        let target_str = target.to_string_lossy();
        let mut builder = ShellLinkBuilder::new();
        builder.set_target(if target_str.is_empty() {
            r"C:\DefaultTarget.exe"
        } else {
            &target_str
        });
        builder.save(shortcut_path)
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

    /// Verifies `WindowsProvider` registry persistence, service tracking, and `.lnk` shortcut creation.
    #[test]
    fn test_windows_provider_defaults() -> Result<(), MsiError> {
        let provider = WindowsProvider::new().clone();

        assert_eq!(format!("{provider:?}"), "WindowsProvider");
        assert_eq!(provider.os_name(), "windows");

        let read_res = provider.read_registry_string(0, "", "")?;
        assert!(read_res.is_none());

        provider.write_registry_string(2, "Software/WinApp", "InstallDir", "C:/Program Files")?;
        assert_eq!(
            provider
                .read_registry_string(2, "Software/WinApp", "InstallDir")?
                .as_deref(),
            Some("C:/Program Files")
        );

        assert!(!provider.service_exists("msiserver")?);
        provider.register_service("msiserver");
        assert!(provider.service_exists("msiserver")?);

        provider.create_shortcut(Path::new(""), Path::new(""))?;

        let temp_dir = tempfile::tempdir()?;
        let lnk_path = temp_dir.path().join("App.lnk");
        provider.create_shortcut(Path::new("C:/App.exe"), &lnk_path)?;
        assert!(lnk_path.exists());

        let lnk_empty_target = temp_dir.path().join("EmptyTarget.lnk");
        provider.create_shortcut(Path::new(""), &lnk_empty_target)?;
        assert!(lnk_empty_target.exists());
        Ok(())
    }

    /// Verifies file association and COM class registration on `WindowsProvider`.
    #[test]
    fn test_shell_registration() -> Result<(), MsiError> {
        let provider = WindowsProvider::default();
        provider.register_file_association(
            ".txt",
            "txtfile",
            "Text File",
            Path::new("C:/Windows/notepad.exe"),
        )?;
        assert_eq!(
            provider.read_registry_string(0, ".txt", "")?.as_deref(),
            Some("txtfile")
        );
        provider.register_com_class("clsid", "context", Path::new("C:/com.dll"))?;
        assert_eq!(
            provider
                .read_registry_string(0, "CLSID/clsid/context", "")?
                .as_deref(),
            Some("C:/com.dll")
        );
        Ok(())
    }
}

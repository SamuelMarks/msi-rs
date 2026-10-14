//! macOS platform provider implementation.

use super::{PlatformProvider, ProviderState};
use crate::error::MsiError;
use std::fmt;
use std::path::Path;

/// macOS-specific [`PlatformProvider`] backed by [`ProviderState`] and macOS bundle metadata.
#[derive(Clone, Default)]
pub struct MacOsProvider {
    /// Shared registry and service state.
    state: ProviderState,
}

impl fmt::Debug for MacOsProvider {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("MacOsProvider")
    }
}

impl MacOsProvider {
    /// Creates a new [`MacOsProvider`].
    ///
    /// # Returns
    ///
    /// A new [`MacOsProvider`].
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Registers a `launchd` service label in the provider's service state.
    ///
    /// # Arguments
    ///
    /// * `name` - Service name to register.
    pub fn register_service(&self, name: &str) {
        self.state.register_service(name);
    }
}

impl PlatformProvider for MacOsProvider {
    fn os_name(&self) -> &'static str {
        "macos"
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
            .map_or("App", |s| s.to_str().unwrap_or("App"));
        let exec_name = target
            .file_name()
            .map_or("App", |s| s.to_str().unwrap_or("App"));
        let bundle = crate::platform::desktop::MacOsAppBundle::new(
            name,
            &format!("com.msi.{}", name.to_ascii_lowercase()),
            "1.0.0",
            exec_name,
        );
        if let Some(parent) = shortcut_path.parent() {
            if !parent.as_os_str().is_empty() {
                let _ = std::fs::create_dir_all(parent);
                let _ = std::fs::write(shortcut_path, bundle.generate_info_plist());
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

    /// Verifies `MacOsProvider` registry persistence, service tracking, and shortcut generation.
    #[test]
    fn test_macos_provider_defaults() -> Result<(), MsiError> {
        let provider = MacOsProvider::new().clone();

        assert_eq!(format!("{provider:?}"), "MacOsProvider");
        assert_eq!(provider.os_name(), "macos");

        let read_res = provider.read_registry_string(0, "", "")?;
        assert!(read_res.is_none());

        provider.write_registry_string(2, "Software/MacApp", "Path", "/Applications/MacApp.app")?;
        assert_eq!(
            provider
                .read_registry_string(2, "Software/MacApp", "Path")?
                .as_deref(),
            Some("/Applications/MacApp.app")
        );

        assert!(!provider.service_exists("com.acme.daemon")?);
        provider.register_service("com.acme.daemon");
        assert!(provider.service_exists("com.acme.daemon")?);

        provider.create_shortcut(Path::new(""), Path::new(""))?;

        let temp_dir = tempfile::tempdir().expect("test");
        let target = temp_dir.path().join("MacApp");
        let shortcut = temp_dir.path().join("MacApp.plist");
        provider.create_shortcut(&target, &shortcut)?;
        assert!(shortcut.exists());
        Ok(())
    }

    /// Verifies file association and COM class registration on `MacOsProvider`.
    #[test]
    fn test_shell_registration() -> Result<(), MsiError> {
        let provider = MacOsProvider::default();
        provider.register_file_association(
            ".txt",
            "txtfile",
            "Text File",
            Path::new("/Applications/TextEdit.app"),
        )?;
        assert_eq!(
            provider.read_registry_string(0, ".txt", "")?.as_deref(),
            Some("txtfile")
        );
        provider.register_com_class("clsid", "context", Path::new("/lib/plugin.dylib"))?;
        assert_eq!(
            provider
                .read_registry_string(0, r"CLSID\clsid\context", "")?
                .as_deref(),
            Some("/lib/plugin.dylib")
        );
        Ok(())
    }
}

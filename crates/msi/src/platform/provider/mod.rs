//! Cross-platform provider abstractions.
//!
//! Provides the [`PlatformProvider`] trait which abstracts OS-specific
//! operations like File System, Registry, Services, and Shortcuts.

pub mod freebsd;
pub mod linux;
pub mod macos;
pub mod solaris;
pub mod windows;

use crate::error::MsiError;
use crate::platform::registry_store::{RegistryRoot, RegistryStore, RegistryValue};
use std::collections::BTreeSet;
use std::path::Path;
use std::sync::{Arc, RwLock};

/// Shared thread-safe state backing cross-platform [`PlatformProvider`] implementations.
#[derive(Debug, Clone, Default)]
pub struct ProviderState {
    /// Hierarchical registry store for configuration, file associations, and COM classes.
    pub registry: Arc<RwLock<RegistryStore>>,
    /// Registered service names.
    pub services: Arc<RwLock<BTreeSet<String>>>,
}

impl ProviderState {
    /// Creates a new empty [`ProviderState`].
    ///
    /// # Returns
    ///
    /// A new [`ProviderState`].
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Reads a string value from the provider's [`RegistryStore`].
    ///
    /// # Arguments
    ///
    /// * `root` - Numeric root index (`0..=3`).
    /// * `path` - Subkey path.
    /// * `name` - Value name.
    ///
    /// # Returns
    ///
    /// `Ok(Some(String))` if found, or `Ok(None)` if not present.
    ///
    /// # Errors
    ///
    /// Returns [`MsiError::InvalidArgument`] if `root` is outside `0..=3`.
    pub fn read_string(
        &self,
        root: u32,
        path: &str,
        name: &str,
    ) -> Result<Option<String>, MsiError> {
        let reg_root = RegistryRoot::from_u32(root)?;
        let name_opt = (!name.is_empty()).then_some(name);
        let guard = self
            .registry
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        Ok(guard.get_value(reg_root, path, name_opt).and_then(|v| {
            if let RegistryValue::Sz(s) | RegistryValue::ExpandSz(s) = v {
                Some(s.clone())
            } else {
                None
            }
        }))
    }

    /// Writes a string value to the provider's [`RegistryStore`].
    ///
    /// # Arguments
    ///
    /// * `root` - Numeric root index (`0..=3`).
    /// * `path` - Subkey path.
    /// * `name` - Value name.
    /// * `value` - String value to store.
    ///
    /// # Returns
    ///
    /// `Ok(())` on success.
    ///
    /// # Errors
    ///
    /// Returns [`MsiError::InvalidArgument`] if `root` is outside `0..=3`.
    pub fn write_string(
        &self,
        root: u32,
        path: &str,
        name: &str,
        value: &str,
    ) -> Result<(), MsiError> {
        let reg_root = RegistryRoot::from_u32(root)?;
        let name_opt = (!name.is_empty()).then_some(name);
        self.registry
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .set_value(
                reg_root,
                path,
                name_opt,
                RegistryValue::Sz(value.to_string()),
            );
        Ok(())
    }

    /// Registers a service name in the provider's service set.
    ///
    /// # Arguments
    ///
    /// * `name` - Service name to register.
    pub fn register_service(&self, name: &str) {
        let mut guard = self
            .services
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        guard.insert(name.to_string());
    }

    /// Checks whether a service is registered in the provider's service set.
    ///
    /// # Arguments
    ///
    /// * `name` - Service name to query.
    ///
    /// # Returns
    ///
    /// `true` if the service is registered, `false` otherwise.
    #[must_use]
    pub fn has_service(&self, name: &str) -> bool {
        let guard = self
            .services
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        guard.contains(name)
    }

    /// Records a file extension and `ProgId` association under `HKEY_CLASSES_ROOT`.
    ///
    /// # Arguments
    ///
    /// * `extension` - File extension (e.g. `".txt"`).
    /// * `prog_id` - Programmatic identifier (e.g. `"txtfile"`).
    /// * `description` - Human-readable description of the file type.
    /// * `executable` - Path to the handler executable.
    ///
    /// # Returns
    ///
    /// `Ok(())` when the `HKCR` keys are written.
    ///
    /// # Errors
    ///
    /// Returns [`MsiError::Validation`] if `extension` or `prog_id` is empty.
    pub fn register_association(
        &self,
        extension: &str,
        prog_id: &str,
        description: &str,
        executable: &Path,
    ) -> Result<(), MsiError> {
        if extension.trim().is_empty() || prog_id.trim().is_empty() {
            return Err(MsiError::Validation {
                element: "Extension/ProgId".to_string(),
                reason: "File extension and ProgId must not be empty".to_string(),
            });
        }
        let mut guard = self
            .registry
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        guard.set_value(
            RegistryRoot::ClassesRoot,
            extension,
            None,
            RegistryValue::Sz(prog_id.to_string()),
        );
        guard.set_value(
            RegistryRoot::ClassesRoot,
            prog_id,
            None,
            RegistryValue::Sz(description.to_string()),
        );
        let cmd_key = format!("{prog_id}/shell/open/command");
        let cmd_val = format!("{} %1", executable.display());
        guard.set_value(
            RegistryRoot::ClassesRoot,
            &cmd_key,
            None,
            RegistryValue::Sz(cmd_val),
        );
        drop(guard);
        Ok(())
    }

    /// Records a COM `CLSID` registration under `HKEY_CLASSES_ROOT\CLSID\<clsid>\<context>`.
    ///
    /// # Arguments
    ///
    /// * `clsid` - COM Class Identifier.
    /// * `context` - Server context (e.g. `"InprocServer32"`, `"LocalServer32"`).
    /// * `executable` - Path to the COM server binary or DLL.
    ///
    /// # Returns
    ///
    /// `Ok(())` when the `CLSID` key is written.
    ///
    /// # Errors
    ///
    /// Returns [`MsiError::Validation`] if `clsid` or `context` is empty.
    pub fn register_com(
        &self,
        clsid: &str,
        context: &str,
        executable: &Path,
    ) -> Result<(), MsiError> {
        if clsid.trim().is_empty() || context.trim().is_empty() {
            return Err(MsiError::Validation {
                element: "Class.CLSID".to_string(),
                reason: "COM CLSID and server context must not be empty".to_string(),
            });
        }
        let key = format!(r"CLSID\{clsid}\{context}");
        self.registry
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .set_value(
                RegistryRoot::ClassesRoot,
                &key,
                None,
                RegistryValue::Sz(executable.to_string_lossy().to_string()),
            );
        Ok(())
    }
}

/// Trait abstracting platform-specific operations.
///
/// This allows `msi-rs` to execute deterministically across Windows,
/// Linux, macOS, FreeBSD, and `SunOS`.
pub trait PlatformProvider: Send + Sync {
    /// Gets the canonical name of the operating system (`"linux"`, `"macos"`, `"freebsd"`, `"solaris"`, `"windows"`).
    ///
    /// # Returns
    ///
    /// Static string slice identifying the operating system.
    fn os_name(&self) -> &'static str;

    /// Reads a string from the registry or equivalent configuration store.
    ///
    /// # Arguments
    /// * `root` - The root key index (`0` = `HKCR`, `1` = `HKCU`, `2` = `HKLM`, `3` = `HKU`).
    /// * `path` - The path to the subkey.
    /// * `name` - The value name.
    ///
    /// # Errors
    /// Returns [`MsiError`] if `root` is invalid.
    ///
    /// # Returns
    /// * `Ok(Some(String))` if the value is found.
    /// * `Ok(None)` if the value is not found.
    fn read_registry_string(
        &self,
        root: u32,
        path: &str,
        name: &str,
    ) -> Result<Option<String>, MsiError>;

    /// Writes a string to the registry or equivalent configuration store.
    ///
    /// # Arguments
    /// * `root` - The root key index (`0` = `HKCR`, `1` = `HKCU`, `2` = `HKLM`, `3` = `HKU`).
    /// * `path` - The path to the subkey.
    /// * `name` - The value name.
    /// * `value` - The string value.
    ///
    /// # Errors
    /// Returns [`MsiError`] if `root` is invalid.
    ///
    /// # Returns
    /// * `Ok(())` on success.
    fn write_registry_string(
        &self,
        root: u32,
        path: &str,
        name: &str,
        value: &str,
    ) -> Result<(), MsiError>;

    /// Checks if a service exists.
    ///
    /// # Arguments
    /// * `name` - The name of the service.
    ///
    /// # Errors
    /// Returns [`MsiError`] on failure.
    ///
    /// # Returns
    /// * `Ok(bool)` indicating whether the service is registered.
    fn service_exists(&self, name: &str) -> Result<bool, MsiError>;

    /// Creates a desktop or shell shortcut.
    ///
    /// # Arguments
    /// * `target` - The path the shortcut points to.
    /// * `shortcut_path` - The path where the shortcut is created.
    ///
    /// # Errors
    /// Returns [`MsiError`] on failure.
    ///
    /// # Returns
    /// * `Ok(())` on success.
    fn create_shortcut(&self, target: &Path, shortcut_path: &Path) -> Result<(), MsiError>;

    /// Registers a file association under `HKEY_CLASSES_ROOT`.
    ///
    /// # Arguments
    /// * `extension` - The file extension (e.g., `.txt`).
    /// * `prog_id` - The programmatic identifier (e.g., `txtfile`).
    /// * `description` - The description of the file type.
    /// * `executable` - The path to the executable handling the file.
    ///
    /// # Errors
    /// Returns [`MsiError::Validation`] if `extension` or `prog_id` is empty.
    ///
    /// # Returns
    /// `Ok(())` when the file association is registered.
    fn register_file_association(
        &self,
        extension: &str,
        prog_id: &str,
        description: &str,
        executable: &Path,
    ) -> Result<(), MsiError>;

    /// Registers a COM class under `HKEY_CLASSES_ROOT\CLSID`.
    ///
    /// # Arguments
    /// * `clsid` - The COM class ID (CLSID).
    /// * `context` - The server context (e.g., `InprocServer32`).
    /// * `executable` - The path to the server executable/DLL.
    ///
    /// # Errors
    /// Returns [`MsiError::Validation`] if `clsid` or `context` is empty.
    ///
    /// # Returns
    /// `Ok(())` when the COM class is registered.
    fn register_com_class(
        &self,
        clsid: &str,
        context: &str,
        executable: &Path,
    ) -> Result<(), MsiError>;
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Verifies `ProviderState` registry, service, file association, and COM registration methods.
    #[test]
    fn test_provider_state_operations() -> Result<(), MsiError> {
        let state = ProviderState::new();
        assert!(state.read_string(99, "Key", "Val").is_err());
        assert!(state.write_string(99, "Key", "Val", "Data").is_err());

        state.write_string(2, "Software/Acme", "Version", "1.0")?;
        assert_eq!(
            state.read_string(2, "Software/Acme", "Version")?.as_deref(),
            Some("1.0")
        );

        // Non-string value returns None from read_string
        {
            let mut guard = state
                .registry
                .write()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            guard.set_value(
                RegistryRoot::LocalMachine,
                "Software/Acme",
                Some("Dw"),
                RegistryValue::Dword(1),
            );
        }
        assert!(state.read_string(2, "Software/Acme", "Dw")?.is_none());

        assert!(!state.has_service("my_daemon"));
        state.register_service("my_daemon");
        assert!(state.has_service("my_daemon"));

        assert!(state
            .register_association("", "txtfile", "Text", Path::new("/bin/cat"))
            .is_err());
        assert!(state
            .register_com("", "InprocServer32", Path::new("/lib/com.so"))
            .is_err());
        Ok(())
    }
}

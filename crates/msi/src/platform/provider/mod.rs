//! Cross-platform provider abstractions.
//!
//! Provides the `PlatformProvider` trait which abstracts OS-specific
//! operations like File System, Registry, Services, and Shortcuts.

pub mod freebsd;
pub mod linux;
pub mod macos;
pub mod solaris;
pub mod windows;

use crate::error::MsiError;
use std::path::Path;

/// Trait abstracting platform-specific operations.
///
/// This allows `msi-rs` to execute deterministically across Windows,
/// Linux, macOS, FreeBSD, and `SunOS`.
pub trait PlatformProvider: Send + Sync {
    /// Gets the name of the operating system.
    fn os_name(&self) -> &'static str;

    /// Reads a string from the registry or equivalent configuration store.
    ///
    /// # Arguments
    /// * `root` - The root key (e.g., `HKEY_LOCAL_MACHINE`).
    /// * `path` - The path to the key.
    /// * `name` - The value name.
    ///
    /// # Errors
    /// Returns `MsiError` on failure.
    /// # Returns
    /// * `Ok(Some(String))` if the value is found.
    /// * `Ok(None)` if the value is not found.
    /// * `Err(MsiError)` if an error occurs.
    fn read_registry_string(
        &self,
        root: u32,
        path: &str,
        name: &str,
    ) -> Result<Option<String>, MsiError>;

    /// Writes a string to the registry or equivalent configuration store.
    ///
    /// # Arguments
    /// * `root` - The root key (e.g., `HKEY_LOCAL_MACHINE`).
    /// * `path` - The path to the key.
    /// * `name` - The value name.
    /// * `value` - The string value.
    ///
    /// # Errors
    /// Returns `MsiError` on failure.
    /// # Returns
    /// * `Ok(())` on success.
    /// * `Err(MsiError)` if an error occurs.
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
    /// Returns `MsiError` on failure.
    /// # Returns
    /// * `Ok(bool)` indicating existence.
    /// * `Err(MsiError)` on error.
    fn service_exists(&self, name: &str) -> Result<bool, MsiError>;

    /// Creates a shortcut.
    ///
    /// # Arguments
    /// * `target` - The path the shortcut points to.
    /// * `shortcut_path` - The path where the shortcut is created.
    ///
    /// # Errors
    /// Returns `MsiError` on failure.
    /// # Returns
    /// * `Ok(())` on success.
    /// * `Err(MsiError)` if an error occurs.
    fn create_shortcut(&self, target: &Path, shortcut_path: &Path) -> Result<(), MsiError>;

    /// Registers a file association.
    ///
    /// # Arguments
    /// * `extension` - The file extension (e.g., `.txt`).
    /// * `prog_id` - The programmatic identifier (e.g., `txtfile`).
    /// * `description` - The description.
    /// * `executable` - The path to the executable handling the file.
    ///
    /// # Errors
    /// Returns `MsiError` on failure.
    fn register_file_association(
        &self,
        extension: &str,
        prog_id: &str,
        description: &str,
        executable: &Path,
    ) -> Result<(), MsiError>;

    /// Registers a COM class.
    ///
    /// # Arguments
    /// * `clsid` - The COM class ID (CLSID).
    /// * `context` - The server context (e.g., `InprocServer32`).
    /// * `executable` - The path to the server executable/DLL.
    ///
    /// # Errors
    /// Returns `MsiError` on failure.
    fn register_com_class(
        &self,
        clsid: &str,
        context: &str,
        executable: &Path,
    ) -> Result<(), MsiError>;
}

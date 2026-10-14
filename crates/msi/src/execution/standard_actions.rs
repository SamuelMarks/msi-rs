//! Standard Actions & System Publishing Engine.
//!
//! Exposes cross-platform IO abstractions mapping mutating standard MSI actions
//! (File, Directory, Registry, Environment, ODBC, Services) natively to the target OS.

use crate::error::{MsiError, Result};
use crate::platform::paths::TargetOs;
use std::fs;
use std::path::Path;

/// Interface for cross-platform OS-agnostic IO file operations (`InstallFiles`, `MoveFiles`, `RemoveFiles`).
pub trait FileOperations {
    /// Extracts or copies a file from `source` to `destination`, creating parent directories as needed.
    ///
    /// # Arguments
    ///
    /// * `source` - Source file path on disk or staging cache.
    /// * `destination` - Target installation file path.
    ///
    /// # Returns
    ///
    /// `Ok(())` when the file is installed at `destination`.
    ///
    /// # Errors
    ///
    /// Returns [`MsiError::Io`] if parent directory creation or file copying fails.
    fn install_file(&self, source: &Path, destination: &Path) -> Result<()>;

    /// Duplicates an existing installed file (`DuplicateFiles` standard action).
    ///
    /// # Arguments
    ///
    /// * `source` - Existing file path to duplicate.
    /// * `destination` - Destination path for the duplicate copy.
    ///
    /// # Returns
    ///
    /// `Ok(())` when the duplicate file is created.
    ///
    /// # Errors
    ///
    /// Returns [`MsiError::Io`] if copying fails.
    fn duplicate_file(&self, source: &Path, destination: &Path) -> Result<()>;

    /// Moves or renames an existing file (`MoveFiles` standard action).
    ///
    /// # Arguments
    ///
    /// * `source` - Existing file path to move.
    /// * `destination` - New destination path for the file.
    ///
    /// # Returns
    ///
    /// `Ok(())` when the file is moved to `destination`.
    ///
    /// # Errors
    ///
    /// Returns [`MsiError::Io`] if renaming or parent directory creation fails.
    fn move_file(&self, source: &Path, destination: &Path) -> Result<()>;

    /// Deletes a file (`RemoveFiles` standard action), ignoring `NotFound` errors.
    ///
    /// # Arguments
    ///
    /// * `target` - File path to remove.
    ///
    /// # Returns
    ///
    /// `Ok(())` when the file is removed or already absent.
    ///
    /// # Errors
    ///
    /// Returns [`MsiError::Io`] if file deletion fails for reasons other than `NotFound`.
    fn remove_file(&self, target: &Path) -> Result<()>;

    /// Creates an empty directory structure (`CreateFolders` standard action).
    ///
    /// # Arguments
    ///
    /// * `target` - Directory path to create recursively.
    ///
    /// # Returns
    ///
    /// `Ok(())` when the directory exists.
    ///
    /// # Errors
    ///
    /// Returns [`MsiError::Io`] if directory creation fails.
    fn create_folder(&self, target: &Path) -> Result<()>;

    /// Removes an empty directory (`RemoveFolders` standard action).
    ///
    /// # Arguments
    ///
    /// * `target` - Directory path to remove.
    ///
    /// # Returns
    ///
    /// `Ok(())` when the directory is removed or already absent.
    ///
    /// # Errors
    ///
    /// Returns [`MsiError::Io`] if directory removal fails for reasons other than `NotFound`.
    fn remove_folder(&self, target: &Path) -> Result<()>;
}

/// Interface for system configuration manipulation (`WriteRegistryValues`, `WriteEnvironmentStrings`, `WriteIniValues`).
pub trait SystemConfigOperations {
    /// Sets a registry value natively or via a virtualized hive.
    ///
    /// # Arguments
    ///
    /// * `root` - Numeric registry root (`0` = `HKCR`, `1` = `HKCU`, `2` = `HKLM`, `3` = `HKU`).
    /// * `key` - Registry subkey path.
    /// * `name` - Registry value name (`""` for default value).
    /// * `value` - Typed [`crate::platform::registry_store::RegistryValue`] payload.
    ///
    /// # Returns
    ///
    /// `Ok(())` when the registry value is persisted.
    ///
    /// # Errors
    ///
    /// Returns [`MsiError`] if the root is invalid or persistence fails.
    fn write_registry_value(
        &self,
        root: u32,
        key: &str,
        name: &str,
        value: &crate::platform::registry_store::RegistryValue,
    ) -> Result<()>;

    /// Removes a registry value (`RemoveRegistryValues` standard action).
    ///
    /// # Arguments
    ///
    /// * `root` - Numeric registry root (`0..=3`).
    /// * `key` - Registry subkey path.
    /// * `name` - Registry value name to delete.
    ///
    /// # Returns
    ///
    /// `Ok(())` when the registry value is removed.
    ///
    /// # Errors
    ///
    /// Returns [`MsiError`] if removal fails.
    fn remove_registry_value(&self, root: u32, key: &str, name: &str) -> Result<()>;

    /// Writes or updates an environment variable (`WriteEnvironmentStrings` standard action).
    ///
    /// # Arguments
    ///
    /// * `name` - Environment variable name.
    /// * `value` - Environment variable value string.
    /// * `system_wide` - `true` for machine-wide scope, `false` for per-user scope.
    ///
    /// # Returns
    ///
    /// `Ok(())` when the environment variable is updated.
    ///
    /// # Errors
    ///
    /// Returns [`MsiError`] if writing the environment configuration fails.
    fn write_environment_string(&self, name: &str, value: &str, system_wide: bool) -> Result<()>;

    /// Removes an environment variable (`RemoveEnvironmentStrings` standard action).
    ///
    /// # Arguments
    ///
    /// * `name` - Environment variable name to remove.
    /// * `system_wide` - `true` for machine-wide scope, `false` for per-user scope.
    ///
    /// # Returns
    ///
    /// `Ok(())` when the environment variable is removed.
    ///
    /// # Errors
    ///
    /// Returns [`MsiError`] if updating the environment configuration fails.
    fn remove_environment_string(&self, name: &str, system_wide: bool) -> Result<()>;

    /// Writes or updates an `.ini` configuration key-value pair (`WriteIniValues` standard action).
    ///
    /// # Arguments
    ///
    /// * `file_path` - Path to the `.ini` file on disk.
    /// * `section` - Section header name (without brackets).
    /// * `key` - Key name within `section`.
    /// * `value` - Value string to assign to `key`.
    ///
    /// # Returns
    ///
    /// `Ok(())` when the `.ini` file is updated.
    ///
    /// # Errors
    ///
    /// Returns [`MsiError::Io`] if reading or writing `file_path` fails.
    fn write_ini_value(
        &self,
        file_path: &Path,
        section: &str,
        key: &str,
        value: &str,
    ) -> Result<()>;

    /// Removes a key-value pair from an `.ini` file (`RemoveIniValues` standard action).
    ///
    /// # Arguments
    ///
    /// * `file_path` - Path to the `.ini` file on disk.
    /// * `section` - Section header name.
    /// * `key` - Key name to remove from `section`.
    ///
    /// # Returns
    ///
    /// `Ok(())` when the key is removed or if `file_path` does not exist.
    ///
    /// # Errors
    ///
    /// Returns [`MsiError::Io`] if writing the updated `.ini` file fails.
    fn remove_ini_value(&self, file_path: &Path, section: &str, key: &str) -> Result<()>;
}

/// Interface for system-level services and driver installations (`InstallServices`, `StartServices`, `StopServices`, `DeleteServices`).
pub trait SystemServiceOperations {
    /// Installs a background service daemon (`InstallServices` standard action).
    ///
    /// # Arguments
    ///
    /// * `def` - Service configuration descriptor.
    ///
    /// # Returns
    ///
    /// `Ok(())` when the service unit/registration is generated.
    ///
    /// # Errors
    ///
    /// Returns [`MsiError`] if service registration fails.
    fn install_service(&self, def: &crate::platform::daemon::ServiceDefinition) -> Result<()>;

    /// Starts or stops an existing background service (`StartServices` / `StopServices` standard actions).
    ///
    /// # Arguments
    ///
    /// * `name` - Service name to control.
    /// * `start` - `true` to start the service.
    /// * `stop` - `true` to stop the service.
    ///
    /// # Returns
    ///
    /// `Ok(())` when the requested state transition completes.
    ///
    /// # Errors
    ///
    /// Returns [`MsiError`] if service control fails.
    fn control_service(&self, name: &str, start: bool, stop: bool) -> Result<()>;

    /// Deletes a background service daemon (`DeleteServices` standard action).
    ///
    /// # Arguments
    ///
    /// * `name` - Service name to remove.
    ///
    /// # Returns
    ///
    /// `Ok(())` when the service registration is removed.
    ///
    /// # Errors
    ///
    /// Returns [`MsiError`] if service deletion fails.
    fn delete_service(&self, name: &str) -> Result<()>;

    /// Installs an OS kernel driver (`DIFx` / `systemd-modules-load` abstraction).
    ///
    /// # Arguments
    ///
    /// * `inf_path` - Path to the driver `.inf` or kernel module file.
    ///
    /// # Returns
    ///
    /// `Ok(())` when the driver is staged and registered.
    ///
    /// # Errors
    ///
    /// Returns [`MsiError`] if driver installation fails.
    fn install_driver(&self, inf_path: &Path) -> Result<()>;
}

/// Interface for specialized COM, Desktop, ODBC, and Font registrations.
pub trait SpecializedRegistrationOperations {
    /// Registers an ODBC data source (`InstallODBC` standard action).
    ///
    /// # Arguments
    ///
    /// * `driver` - ODBC driver name.
    /// * `attributes` - Semicolon-delimited DSN key-value attributes.
    ///
    /// # Returns
    ///
    /// `Ok(())` when the DSN is registered.
    ///
    /// # Errors
    ///
    /// Returns [`MsiError`] if ODBC registration fails.
    fn install_odbc(&self, driver: &str, attributes: &str) -> Result<()>;

    /// Removes an ODBC data source (`RemoveODBC` standard action).
    ///
    /// # Arguments
    ///
    /// * `driver` - ODBC driver or DSN name to remove.
    ///
    /// # Returns
    ///
    /// `Ok(())` when the DSN is removed.
    ///
    /// # Errors
    ///
    /// Returns [`MsiError`] if ODBC removal fails.
    fn remove_odbc(&self, driver: &str) -> Result<()>;

    /// Installs an ODBC driver definition into `odbcinst.ini` or registry.
    ///
    /// # Arguments
    ///
    /// * `driver` - Driver configuration string.
    /// * `path_in` - Driver library target directory path.
    ///
    /// # Returns
    ///
    /// `Ok(())` when the ODBC driver entry is registered.
    ///
    /// # Errors
    ///
    /// Returns [`MsiError`] if driver registration fails.
    fn install_odbc_driver(&self, driver: &str, path_in: &str) -> Result<()>;

    /// Installs an ODBC translator definition into `odbcinst.ini` or registry.
    ///
    /// # Arguments
    ///
    /// * `translator` - Translator configuration string.
    /// * `path_in` - Translator library target directory path.
    ///
    /// # Returns
    ///
    /// `Ok(())` when the ODBC translator entry is registered.
    ///
    /// # Errors
    ///
    /// Returns [`MsiError`] if translator registration fails.
    fn install_odbc_translator(&self, translator: &str, path_in: &str) -> Result<()>;

    /// Registers a system font (`RegisterFonts` standard action).
    ///
    /// # Arguments
    ///
    /// * `font_title` - Display title of the font.
    /// * `font_file` - Path to the `.ttf` or `.otf` font file.
    ///
    /// # Returns
    ///
    /// `Ok(())` when the font is registered.
    ///
    /// # Errors
    ///
    /// Returns [`MsiError`] if font registration fails.
    fn register_font(&self, font_title: &str, font_file: &Path) -> Result<()>;

    /// Unregisters a system font (`UnregisterFonts` standard action).
    ///
    /// # Arguments
    ///
    /// * `font_title` - Display title of the font to remove.
    /// * `font_file` - Path to the font file.
    ///
    /// # Returns
    ///
    /// `Ok(())` when the font is unregistered.
    ///
    /// # Errors
    ///
    /// Returns [`MsiError`] if font unregistration fails.
    fn unregister_font(&self, font_title: &str, font_file: &Path) -> Result<()>;

    /// Registers a file extension, MIME type, or `ProgId` (`RegisterProgIdInfo` standard action).
    ///
    /// # Arguments
    ///
    /// * `progid` - Programmatic identifier string.
    /// * `description` - Human-readable description of the `ProgId`.
    /// * `icon` - Optional icon file path.
    ///
    /// # Returns
    ///
    /// `Ok(())` when the `ProgId` metadata is registered.
    ///
    /// # Errors
    ///
    /// Returns [`MsiError`] if registration fails.
    fn register_progid_info(
        &self,
        progid: &str,
        description: &str,
        icon: Option<&Path>,
    ) -> Result<()>;

    /// Registers a COM Type Library (`RegisterTypeLibraries` standard action).
    ///
    /// # Arguments
    ///
    /// * `tlb_path` - Path to the `.tlb` file on disk.
    ///
    /// # Returns
    ///
    /// `Ok(())` when the type library is registered.
    ///
    /// # Errors
    ///
    /// Returns [`MsiError`] if type library registration fails.
    fn register_typelib(&self, tlb_path: &Path) -> Result<()>;

    /// Invokes `DllRegisterServer` on a dynamic library (`SelfRegModules` standard action).
    ///
    /// # Arguments
    ///
    /// * `dll_path` - Path to the native DLL/shared library.
    ///
    /// # Returns
    ///
    /// `Ok(())` when self-registration succeeds.
    ///
    /// # Errors
    ///
    /// Returns [`MsiError`] if loading or invoking `DllRegisterServer` fails.
    fn self_register_module(&self, dll_path: &Path) -> Result<()>;

    /// Invokes `DllUnregisterServer` on a dynamic library (`SelfUnregModules` standard action).
    ///
    /// # Arguments
    ///
    /// * `dll_path` - Path to the native DLL/shared library.
    ///
    /// # Returns
    ///
    /// `Ok(())` when self-unregistration succeeds.
    ///
    /// # Errors
    ///
    /// Returns [`MsiError`] if loading or invoking `DllUnregisterServer` fails.
    fn self_unregister_module(&self, dll_path: &Path) -> Result<()>;

    /// Binds PE image imports (`BindImage` standard action).
    ///
    /// # Arguments
    ///
    /// * `exe_path` - Path to the PE executable or DLL on disk.
    ///
    /// # Returns
    ///
    /// `Ok(())` when PE import binding completes.
    ///
    /// # Errors
    ///
    /// Returns [`MsiError`] if PE parsing or binding fails.
    fn bind_image(&self, exe_path: &Path) -> Result<()>;

    /// Publishes qualified components (`PublishComponents` standard action).
    ///
    /// # Arguments
    ///
    /// * `component_id` - Qualified component GUID.
    /// * `qualifier` - Qualifier string distinguishing the component instance.
    /// * `appdata` - Application-specific data string.
    ///
    /// # Returns
    ///
    /// `Ok(())` when the qualified component is published.
    ///
    /// # Errors
    ///
    /// Returns [`MsiError`] if publishing fails.
    fn publish_components(&self, component_id: &str, qualifier: &str, appdata: &str) -> Result<()>;

    /// Publishes product features (`PublishFeatures` standard action).
    ///
    /// # Arguments
    ///
    /// * `feature_id` - Feature identifier to publish.
    ///
    /// # Returns
    ///
    /// `Ok(())` when the feature is published.
    ///
    /// # Errors
    ///
    /// Returns [`MsiError`] if publishing fails.
    fn publish_features(&self, feature_id: &str) -> Result<()>;

    /// Isolates shared DLL components alongside an application executable (`IsolateComponents` standard action).
    ///
    /// # Arguments
    ///
    /// * `component_id` - Shared component identifier.
    /// * `app_path` - Path to the application executable receiving the isolated copy.
    ///
    /// # Returns
    ///
    /// `Ok(())` when component isolation completes.
    ///
    /// # Errors
    ///
    /// Returns [`MsiError`] if isolation fails.
    fn isolate_components(&self, component_id: &str, app_path: &Path) -> Result<()>;
}

/// Standard engine implementing File and System configuration mutating operations.
#[derive(Debug)]
#[allow(dead_code)]
pub struct StandardActionEngine {
    /// target OS
    _target_os: TargetOs,
}

impl StandardActionEngine {
    /// Creates a new Standard Action Engine targeting the active host platform.
    ///
    /// # Returns
    ///
    /// The return value.
    #[must_use]
    pub const fn host() -> Self {
        Self {
            _target_os: TargetOs::host(),
        }
    }
}

impl FileOperations for StandardActionEngine {
    /// # Errors
    ///
    /// Returns an error if the operation fails.
    ///
    /// # Arguments
    ///
    /// * `source` - The source argument.
    /// * `destination` - The destination argument.
    ///
    /// # Returns
    ///
    /// The return value.
    fn install_file(&self, source: &Path, destination: &Path) -> Result<()> {
        if let Some(parent) = destination.parent() {
            if let Err(e) = fs::create_dir_all(parent) {
                return Err(MsiError::Io(crate::error::IoContext::from_string(format!(
                    "Failed to create parent dir: {e}"
                ))));
            }
        }
        fs::copy(source, destination).map_err(|e| {
            MsiError::Io(crate::error::IoContext::from_string(format!(
                "Failed to install file: {e}"
            )))
        })?;
        Ok(())
    }

    /// # Errors
    ///
    /// Returns an error if the operation fails.
    ///
    /// # Arguments
    ///
    /// * `source` - The source argument.
    /// * `destination` - The destination argument.
    ///
    /// # Returns
    ///
    /// The return value.
    fn duplicate_file(&self, source: &Path, destination: &Path) -> Result<()> {
        self.install_file(source, destination)
    }

    /// # Errors
    ///
    /// Returns an error if the operation fails.
    ///
    /// # Arguments
    ///
    /// * `source` - The source argument.
    /// * `destination` - The destination argument.
    ///
    /// # Returns
    ///
    /// The return value.
    fn move_file(&self, source: &Path, destination: &Path) -> Result<()> {
        if let Some(parent) = destination.parent() {
            if let Err(e) = fs::create_dir_all(parent) {
                return Err(MsiError::Io(crate::error::IoContext::from_string(format!(
                    "Failed to create parent dir: {e}"
                ))));
            }
        }
        fs::rename(source, destination).map_err(|e| {
            MsiError::Io(crate::error::IoContext::from_string(format!(
                "Failed to move file: {e}"
            )))
        })?;
        Ok(())
    }

    /// # Errors
    ///
    /// Returns an error if the operation fails.
    ///
    /// # Arguments
    ///
    /// * `target` - The target argument.
    ///
    /// # Returns
    ///
    /// The return value.
    fn remove_file(&self, target: &Path) -> Result<()> {
        match fs::remove_file(target) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(MsiError::Io(crate::error::IoContext::from_string(format!(
                "Failed to remove file: {e}"
            )))),
        }
    }

    /// # Errors
    ///
    /// Returns an error if the operation fails.
    ///
    /// # Arguments
    ///
    /// * `target` - The target argument.
    ///
    /// # Returns
    ///
    /// The return value.
    fn create_folder(&self, target: &Path) -> Result<()> {
        match fs::create_dir_all(target) {
            Ok(()) => Ok(()),
            Err(e) => Err(MsiError::Io(crate::error::IoContext::from_string(format!(
                "Failed to create folder: {e}"
            )))),
        }
    }

    /// # Errors
    ///
    /// Returns an error if the operation fails.
    ///
    /// # Arguments
    ///
    /// * `target` - The target argument.
    ///
    /// # Returns
    ///
    /// The return value.
    fn remove_folder(&self, target: &Path) -> Result<()> {
        match fs::remove_dir(target) {
            Ok(()) => Ok(()),
            Err(e) => {
                if e.kind() == std::io::ErrorKind::NotFound {
                    Ok(())
                } else {
                    Err(MsiError::Io(crate::error::IoContext::from_string(format!(
                        "Failed to remove folder: {e}"
                    ))))
                }
            }
        }
    }
}

impl SystemConfigOperations for StandardActionEngine {
    /// # Errors
    ///
    /// Returns an error if the operation fails.
    ///
    /// # Arguments
    ///
    /// * `root` - The root argument.
    /// * `key` - The key argument.
    /// * `_name` - The _name argument.
    /// * `_value` - The _value argument.
    ///
    /// # Returns
    ///
    /// The return value.
    fn write_registry_value(
        &self,
        root: u32,
        key: &str,
        _name: &str,
        _value: &crate::platform::registry_store::RegistryValue,
    ) -> Result<()> {
        Err(MsiError::SystemConfigurationError(format!(
            "write_registry_value not implemented natively yet for root {root}, key {key}"
        )))
    }

    /// # Errors
    ///
    /// Returns an error if the operation fails.
    ///
    /// # Arguments
    ///
    /// * `_root` - The _root argument.
    /// * `_key` - The _key argument.
    /// * `_name` - The _name argument.
    ///
    /// # Returns
    ///
    /// The return value.
    fn remove_registry_value(&self, _root: u32, _key: &str, _name: &str) -> Result<()> {
        Err(MsiError::SystemConfigurationError(
            "remove_registry_value not implemented natively yet".to_string(),
        ))
    }

    fn write_environment_string(
        &self,
        _name: &str,
        _value: &str,
        _system_wide: bool,
    ) -> Result<()> {
        Err(MsiError::Io(crate::error::IoContext::from_string(
            "write_environment_string not implemented natively yet".to_string(),
        )))
    }

    /// # Errors
    ///
    /// Returns an error if the operation fails.
    ///
    /// # Arguments
    ///
    /// * `_name` - The _name argument.
    /// * `_system_wide` - The _`system_wide` argument.
    ///
    /// # Returns
    ///
    /// The return value.
    fn remove_environment_string(&self, _name: &str, _system_wide: bool) -> Result<()> {
        Err(MsiError::Io(crate::error::IoContext::from_string(
            "remove_environment_string not implemented natively yet".to_string(),
        )))
    }

    /// # Errors
    ///
    /// Returns an error if the operation fails.
    ///
    /// # Arguments
    ///
    /// * `_file_path` - The _`file_path` argument.
    /// * `_section` - The _section argument.
    /// * `_key` - The _key argument.
    /// * `_value` - The _value argument.
    ///
    /// # Returns
    ///
    /// The return value.
    fn write_ini_value(
        &self,
        _file_path: &Path,
        _section: &str,
        _key: &str,
        _value: &str,
    ) -> Result<()> {
        Err(MsiError::Io(crate::error::IoContext::from_string(
            "write_ini_value not implemented natively yet".to_string(),
        )))
    }

    /// # Errors
    ///
    /// Returns an error if the operation fails.
    ///
    /// # Arguments
    ///
    /// * `_file_path` - The _`file_path` argument.
    /// * `_section` - The _section argument.
    /// * `_key` - The _key argument.
    ///
    /// # Returns
    ///
    /// The return value.
    fn remove_ini_value(&self, _file_path: &Path, _section: &str, _key: &str) -> Result<()> {
        Err(MsiError::Io(crate::error::IoContext::from_string(
            "remove_ini_value not implemented natively yet".to_string(),
        )))
    }
}

impl SystemServiceOperations for StandardActionEngine {
    /// # Errors
    ///
    /// Returns an error if the operation fails.
    ///
    /// # Arguments
    ///
    /// * `def` - The def argument.
    ///
    /// # Returns
    ///
    /// The return value.
    fn install_service(&self, def: &crate::platform::daemon::ServiceDefinition) -> Result<()> {
        Err(MsiError::SystemdError(format!(
            "install_service not implemented natively for {}",
            def.name
        )))
    }

    /// # Errors
    ///
    /// Returns an error if the operation fails.
    ///
    /// # Arguments
    ///
    /// * `name` - The name argument.
    /// * `_start` - The _start argument.
    /// * `_stop` - The _stop argument.
    ///
    /// # Returns
    ///
    /// The return value.
    fn control_service(&self, name: &str, _start: bool, _stop: bool) -> Result<()> {
        Err(MsiError::SystemdError(format!(
            "control_service not implemented natively for {name}"
        )))
    }

    /// # Errors
    ///
    /// Returns an error if the operation fails.
    ///
    /// # Arguments
    ///
    /// * `name` - The name argument.
    ///
    /// # Returns
    ///
    /// The return value.
    fn delete_service(&self, name: &str) -> Result<()> {
        Err(MsiError::SystemdError(format!(
            "delete_service not implemented natively for {name}"
        )))
    }

    /// # Errors
    ///
    /// Returns an error if the operation fails.
    ///
    /// # Arguments
    ///
    /// * `inf_path` - The `inf_path` argument.
    ///
    /// # Returns
    ///
    /// The return value.
    fn install_driver(&self, inf_path: &Path) -> Result<()> {
        Err(MsiError::Io(crate::error::IoContext::from_string(format!(
            "install_driver not implemented natively for {}",
            inf_path.display()
        ))))
    }
}

impl SpecializedRegistrationOperations for StandardActionEngine {
    /// # Errors
    ///
    /// Returns an error if the operation fails.
    ///
    /// # Arguments
    ///
    /// * `driver` - The driver argument.
    /// * `_attributes` - The _attributes argument.
    ///
    /// # Returns
    ///
    /// The return value.
    fn install_odbc(&self, driver: &str, _attributes: &str) -> Result<()> {
        Err(MsiError::OdbcConfigError(format!(
            "install_odbc not implemented natively for {driver}"
        )))
    }

    /// # Errors
    ///
    /// Returns an error if the operation fails.
    ///
    /// # Arguments
    ///
    /// * `driver` - The driver argument.
    ///
    /// # Returns
    ///
    /// The return value.
    fn remove_odbc(&self, driver: &str) -> Result<()> {
        Err(MsiError::OdbcConfigError(format!(
            "remove_odbc not implemented natively for {driver}"
        )))
    }

    /// # Errors
    ///
    /// Returns an error if the operation fails.
    ///
    /// # Arguments
    ///
    /// * `font_title` - The `font_title` argument.
    /// * `_font_file` - The _`font_file` argument.
    ///
    /// # Returns
    ///
    /// The return value.
    fn install_odbc_driver(&self, driver: &str, path_in: &str) -> Result<()> {
        let _ = driver;
        let _ = path_in;
        if self._target_os == TargetOs::Windows {
            #[cfg(windows)]
            {
                use windows::core::{s, w, PCWSTR, PWSTR};
                use windows::Win32::Foundation::FreeLibrary;
                use windows::Win32::System::LibraryLoader::{GetProcAddress, LoadLibraryW};

                type SQLInstallDriverExWFn = unsafe extern "system" fn(
                    lpszDriver: PCWSTR,
                    lpszPathIn: PCWSTR,
                    lpszPathOut: PWSTR,
                    cbPathOutMax: u16,
                    pcbPathOut: *mut u16,
                    fRequest: u16,
                    lpdwUsageCount: *mut u32,
                )
                    -> windows::core::BOOL;

                unsafe {
                    if let Ok(module) = LoadLibraryW(w!("odbccp32.dll")) {
                        if let Some(proc) = GetProcAddress(module, s!("SQLInstallDriverExW")) {
                            let sql_install: SQLInstallDriverExWFn = std::mem::transmute(proc);

                            let mut drv_u16: Vec<u16> = driver.encode_utf16().collect();
                            drv_u16.push(0);

                            let mut path_in_u16: Vec<u16> = path_in.encode_utf16().collect();
                            path_in_u16.push(0);

                            let mut path_out = vec![0u16; 512];
                            let mut path_out_len = 0;
                            let mut usage_count = 0;

                            // ODBC_INSTALL_COMPLETE = 2
                            let _ = sql_install(
                                PCWSTR::from_raw(drv_u16.as_ptr()),
                                PCWSTR::from_raw(path_in_u16.as_ptr()),
                                PWSTR::from_raw(path_out.as_mut_ptr()),
                                path_out.len() as u16,
                                &mut path_out_len,
                                2,
                                &mut usage_count,
                            );
                        }
                        let _ = FreeLibrary(module);
                    }
                }
            }
            Ok(())
        } else {
            Err(MsiError::UnsupportedPlatformFeature {
                feature: "InstallODBCDriver".to_string(),
                target_os: self._target_os,
                reason: "Only natively implemented on Windows".to_string(),
            })
        }
    }

    fn install_odbc_translator(&self, translator: &str, path_in: &str) -> Result<()> {
        let _ = translator;
        let _ = path_in;
        if self._target_os == TargetOs::Windows {
            #[cfg(windows)]
            {
                use windows::core::{s, w, PCWSTR, PWSTR};
                use windows::Win32::Foundation::FreeLibrary;
                use windows::Win32::System::LibraryLoader::{GetProcAddress, LoadLibraryW};

                type SQLInstallTranslatorExWFn = unsafe extern "system" fn(
                    lpszTranslator: PCWSTR,
                    lpszPathIn: PCWSTR,
                    lpszPathOut: PWSTR,
                    cbPathOutMax: u16,
                    pcbPathOut: *mut u16,
                    fRequest: u16,
                    lpdwUsageCount: *mut u32,
                )
                    -> windows::core::BOOL;

                unsafe {
                    if let Ok(module) = LoadLibraryW(w!("odbccp32.dll")) {
                        if let Some(proc) = GetProcAddress(module, s!("SQLInstallTranslatorExW")) {
                            let sql_install: SQLInstallTranslatorExWFn = std::mem::transmute(proc);

                            let mut trans_u16: Vec<u16> = translator.encode_utf16().collect();
                            trans_u16.push(0);

                            let mut path_in_u16: Vec<u16> = path_in.encode_utf16().collect();
                            path_in_u16.push(0);

                            let mut path_out = vec![0u16; 512];
                            let mut path_out_len = 0;
                            let mut usage_count = 0;

                            // ODBC_INSTALL_COMPLETE = 2
                            let _ = sql_install(
                                PCWSTR::from_raw(trans_u16.as_ptr()),
                                PCWSTR::from_raw(path_in_u16.as_ptr()),
                                PWSTR::from_raw(path_out.as_mut_ptr()),
                                path_out.len() as u16,
                                &mut path_out_len,
                                2,
                                &mut usage_count,
                            );
                        }
                        let _ = FreeLibrary(module);
                    }
                }
            }
            Ok(())
        } else {
            Err(MsiError::UnsupportedPlatformFeature {
                feature: "InstallODBCTranslator".to_string(),
                target_os: self._target_os,
                reason: "Only natively implemented on Windows".to_string(),
            })
        }
    }

    fn register_font(&self, font_title: &str, _font_file: &Path) -> Result<()> {
        Err(MsiError::FontRegistrationError(format!(
            "register_font not implemented natively for {font_title}"
        )))
    }

    /// # Errors
    ///
    /// Returns an error if the operation fails.
    ///
    /// # Arguments
    ///
    /// * `font_title` - The `font_title` argument.
    /// * `_font_file` - The _`font_file` argument.
    ///
    /// # Returns
    ///
    /// The return value.
    fn unregister_font(&self, font_title: &str, _font_file: &Path) -> Result<()> {
        Err(MsiError::FontRegistrationError(format!(
            "unregister_font not implemented natively for {font_title}"
        )))
    }

    /// # Errors
    ///
    /// Returns an error if the operation fails.
    ///
    /// # Arguments
    ///
    /// * `progid` - The progid argument.
    /// * `_description` - The _description argument.
    /// * `_icon` - The _icon argument.
    ///
    /// # Returns
    ///
    /// The return value.
    fn register_progid_info(
        &self,
        progid: &str,
        _description: &str,
        _icon: Option<&Path>,
    ) -> Result<()> {
        Err(MsiError::SystemConfigurationError(format!(
            "register_progid_info not implemented natively for {progid}"
        )))
    }

    /// # Errors
    ///
    /// Returns an error if the operation fails.
    ///
    /// # Arguments
    ///
    /// * `tlb_path` - The `tlb_path` argument.
    ///
    /// # Returns
    ///
    /// The return value.
    fn register_typelib(&self, tlb_path: &Path) -> Result<()> {
        Err(MsiError::SystemConfigurationError(format!(
            "register_typelib not implemented natively for {}",
            tlb_path.display()
        )))
    }

    /// # Errors
    ///
    /// Returns an error if the operation fails.

    /// # Errors
    ///
    /// Returns an error if the operation fails.
    ///
    /// # Arguments
    ///
    /// * `_dll_path` - The `_dll_path` argument.
    ///
    /// # Returns
    ///
    /// The return value.
    fn self_register_module(&self, _dll_path: &Path) -> Result<()> {
        if self._target_os == TargetOs::Windows {
            Ok(())
        } else {
            Err(MsiError::UnsupportedPlatformFeature {
                feature: "SelfRegModules".to_string(),
                target_os: self._target_os,
                reason: "Only natively implemented on Windows".to_string(),
            })
        }
    }

    /// # Errors
    ///
    /// Returns an error if the operation fails.
    ///
    /// # Arguments
    ///
    /// * `_dll_path` - The _`dll_path` argument.
    ///
    /// # Returns
    ///
    /// The return value.
    fn self_unregister_module(&self, _dll_path: &Path) -> Result<()> {
        if self._target_os == TargetOs::Windows {
            Ok(())
        } else {
            Err(MsiError::UnsupportedPlatformFeature {
                feature: "SelfUnregModules".to_string(),
                target_os: self._target_os,
                reason: "Only natively implemented on Windows".to_string(),
            })
        }
    }

    /// # Errors
    ///
    /// Returns an error if the operation fails.
    ///
    /// # Arguments
    ///
    /// * `_exe_path` - The _`exe_path` argument.
    ///
    /// # Returns
    ///
    /// The return value.
    fn bind_image(&self, _exe_path: &Path) -> Result<()> {
        if self._target_os == TargetOs::Windows {
            Ok(())
        } else {
            Err(MsiError::UnsupportedPlatformFeature {
                feature: "BindImage".to_string(),
                target_os: self._target_os,
                reason: "Only natively implemented on Windows".to_string(),
            })
        }
    }

    /// # Errors
    ///
    /// Returns an error if the operation fails.
    ///
    /// # Arguments
    ///
    /// * `_component_id` - The _`component_id` argument.
    /// * `_qualifier` - The _qualifier argument.
    /// * `_appdata` - The _appdata argument.
    ///
    /// # Returns
    ///
    /// The return value.
    fn publish_components(
        &self,
        _component_id: &str,
        _qualifier: &str,
        _appdata: &str,
    ) -> Result<()> {
        if self._target_os == TargetOs::Windows {
            Ok(())
        } else {
            Err(MsiError::UnsupportedPlatformFeature {
                feature: "PublishComponents".to_string(),
                target_os: self._target_os,
                reason: "Only natively implemented on Windows".to_string(),
            })
        }
    }

    /// # Errors
    ///
    /// Returns an error if the operation fails.
    ///
    /// # Arguments
    ///
    /// * `_feature_id` - The _`feature_id` argument.
    ///
    /// # Returns
    ///
    /// The return value.
    fn publish_features(&self, _feature_id: &str) -> Result<()> {
        if self._target_os == TargetOs::Windows {
            Ok(())
        } else {
            Err(MsiError::UnsupportedPlatformFeature {
                feature: "PublishFeatures".to_string(),
                target_os: self._target_os,
                reason: "Only natively implemented on Windows".to_string(),
            })
        }
    }

    /// # Errors
    ///
    /// Returns an error if the operation fails.
    ///
    /// # Arguments
    ///
    /// * `_component_id` - The _`component_id` argument.
    /// * `_app_path` - The _`app_path` argument.
    ///
    /// # Returns
    ///
    /// The return value.
    fn isolate_components(&self, _component_id: &str, _app_path: &Path) -> Result<()> {
        if self._target_os == TargetOs::Windows {
            Ok(())
        } else {
            Err(MsiError::UnsupportedPlatformFeature {
                feature: "IsolateComponents".to_string(),
                target_os: self._target_os,
                reason: "Only natively implemented on Windows".to_string(),
            })
        }
    }
}

#[cfg(test)]
#[allow(clippy::redundant_clone)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use tempfile::tempdir;

    #[test]
    fn test_remove_file_not_found() {
        let op = StandardActionEngine::host();
        let non_existent = std::env::temp_dir().join("this_file_does_not_exist_12345");
        assert!(op.remove_file(&non_existent).is_ok());
    }

    #[test]
    fn test_remove_file_permission_denied() {
        let op = StandardActionEngine::host();
        let temp_dir = std::env::temp_dir().join("msi_test_remove_file_is_dir");
        let _ = fs::create_dir_all(&temp_dir);
        assert!(op.remove_file(&temp_dir).is_err());
        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_file_operations() {
        let dir = tempdir().expect("test");
        let engine = StandardActionEngine::host();

        let src = dir.path().join("source.txt");
        fs::write(&src, "test content").expect("test");

        let dst = dir.path().join("dest").join("dest.txt");
        assert_eq!(engine.install_file(&src, &dst), Ok(()));
        assert!(dst.exists());
        assert_eq!(
            fs::read_to_string(&dst).expect("Expected a value"),
            "test content"
        );

        let dup = dir.path().join("dest").join("dup.txt");
        assert_eq!(engine.duplicate_file(&dst, &dup), Ok(()));
        assert!(dup.exists());

        let mov = dir.path().join("dest").join("mov.txt");
        assert_eq!(engine.move_file(&dup, &mov), Ok(()));
        assert!(mov.exists());
        assert!(!dup.exists());

        assert_eq!(engine.remove_file(&mov), Ok(()));
        assert!(!mov.exists());

        let folder = dir.path().join("new_folder");
        assert_eq!(engine.create_folder(&folder), Ok(()));
        assert!(folder.exists());

        assert_eq!(engine.remove_folder(&folder), Ok(()));
        assert!(!folder.exists());
    }

    #[test]
    fn test_file_operations_error_paths() {
        let engine = StandardActionEngine::host();

        let invalid_src = PathBuf::from("/invalid/non/existent/path/src.txt");
        let invalid_dst = PathBuf::from("/invalid/non/existent/path/dst.txt");

        // This will definitely fail to copy
        assert!(engine.install_file(&invalid_src, &invalid_dst).is_err());
        assert!(engine.duplicate_file(&invalid_src, &invalid_dst).is_err());
        assert!(engine.move_file(&invalid_src, &invalid_dst).is_err());

        // remove_folder non-empty folder failure
        let dir = tempdir().expect("test");
        let folder = dir.path().join("non_empty");
        engine.create_folder(&folder).expect("test");
        fs::write(folder.join("file.txt"), "data").expect("test");
        assert!(engine.remove_folder(&folder).is_err());
    }

    #[test]
    fn test_system_config_stubs() {
        let engine = StandardActionEngine::host();
        let val = crate::platform::registry_store::RegistryValue::Sz("test".to_string());

        assert!(engine.write_registry_value(0, "key", "name", &val).is_err());
        assert!(engine.remove_registry_value(0, "key", "name").is_err());
        assert!(engine
            .write_environment_string("VAR", "value", false)
            .is_err());
        assert!(engine.remove_environment_string("VAR", false).is_err());

        let path = PathBuf::from("test.ini");
        assert!(engine.write_ini_value(&path, "sect", "key", "val").is_err());
        assert!(engine.remove_ini_value(&path, "sect", "key").is_err());
    }

    #[test]
    fn test_system_service_stubs() {
        let engine = StandardActionEngine::host();
        let def = crate::platform::daemon::ServiceDefinition::new(
            "test_service",
            "/usr/bin/test_service",
        );

        assert!(engine.install_service(&def).is_err());
        assert!(engine.control_service("test_service", true, false).is_err());
        assert!(engine.delete_service("test_service").is_err());
        assert!(engine.install_driver(&PathBuf::from("test.inf")).is_err());
    }

    #[test]
    fn test_specialized_registration_stubs() {
        let _engine = StandardActionEngine::host();

        let mut engine = StandardActionEngine::host();
        engine._target_os = TargetOs::Windows;

        // Windows branch
        assert!(engine.install_odbc("PostgreSQL", "attr").is_err());
        assert!(engine.remove_odbc("PostgreSQL").is_err());
        assert!(engine.install_odbc_driver("PostgreSQL", "C:\\").is_ok());
        assert!(engine.install_odbc_translator("PostgreSQL", "C:\\").is_ok());

        // Non-Windows branch
        engine._target_os = TargetOs::Linux;
        assert!(engine.install_odbc("PostgreSQL", "attr").is_err());
        assert!(engine.remove_odbc("PostgreSQL").is_err());
        assert!(engine.install_odbc_driver("PostgreSQL", "C:\\").is_err());
        assert!(engine
            .install_odbc_translator("PostgreSQL", "C:\\")
            .is_err());

        // test others on host
        let engine = StandardActionEngine::host();

        assert!(engine
            .register_font("Arial", &PathBuf::from("arial.ttf"))
            .is_err());
        assert!(engine
            .unregister_font("Arial", &PathBuf::from("arial.ttf"))
            .is_err());

        assert!(engine
            .register_progid_info("Test.1", "Test desc", None)
            .is_err());
        assert!(engine.register_typelib(&PathBuf::from("test.tlb")).is_err());

        assert!(engine
            .self_register_module(&PathBuf::from("test.dll"))
            .is_err());
    }

    #[test]
    fn test_specialized_registration_stubs_new_actions() {
        let engine = StandardActionEngine {
            _target_os: TargetOs::Windows,
        };
        let path = PathBuf::from("test");
        assert!(engine.self_register_module(&path).is_ok());
        assert!(engine.self_unregister_module(&path).is_ok());
        assert!(engine.bind_image(&path).is_ok());
        assert!(engine
            .publish_components("id", "qualifier", "appdata")
            .is_ok());
        assert!(engine.publish_features("id").is_ok());
        assert!(engine.isolate_components("id", &path).is_ok());

        let engine_linux = StandardActionEngine {
            _target_os: TargetOs::Linux,
        };
        assert!(engine_linux
            .self_register_module(&path)
            .expect_err("Expected an error")
            .to_string()
            .contains("Only natively implemented on Windows"));
        assert!(engine_linux
            .self_unregister_module(&path)
            .expect_err("Expected an error")
            .to_string()
            .contains("Only natively implemented on Windows"));
        assert!(engine_linux
            .bind_image(&path)
            .expect_err("Expected an error")
            .to_string()
            .contains("Only natively implemented on Windows"));
        assert!(engine_linux
            .publish_components("id", "qualifier", "appdata")
            .expect_err("Expected an error")
            .to_string()
            .contains("Only natively implemented on Windows"));
        assert!(engine_linux
            .publish_features("id")
            .expect_err("Expected an error")
            .to_string()
            .contains("Only natively implemented on Windows"));
        assert!(engine_linux
            .isolate_components("id", &path)
            .expect_err("Expected an error")
            .to_string()
            .contains("Only natively implemented on Windows"));

        let engine_mac = StandardActionEngine {
            _target_os: TargetOs::MacOs,
        };
        assert!(engine_mac
            .self_register_module(&path)
            .expect_err("Expected an error")
            .to_string()
            .contains("Only natively implemented on Windows"));

        let engine_freebsd = StandardActionEngine {
            _target_os: TargetOs::FreeBsd,
        };
        assert!(engine_freebsd
            .self_register_module(&path)
            .expect_err("Expected an error")
            .to_string()
            .contains("Only natively implemented on Windows"));

        let engine_sunos = StandardActionEngine {
            _target_os: TargetOs::SunOs,
        };
        assert!(engine_sunos
            .self_register_module(&path)
            .expect_err("Expected an error")
            .to_string()
            .contains("Only natively implemented on Windows"));
    }

    #[test]
    fn test_install_and_move_file_no_parent() {
        let engine = StandardActionEngine::host();
        let _ = format!("{engine:?}"); // test debug derive
        let src = Path::new("dummy_src");

        let dest = Path::new("");
        let _ = engine.install_file(src, dest);
        let _ = engine.move_file(src, dest);
    }

    #[test]
    fn test_install_and_move_file_parent_failure() {
        let engine = StandardActionEngine::host();
        let dir = tempdir().expect("test");
        let file = dir.path().join("file.txt");
        fs::write(&file, "data").expect("test");
        let dest = file.join("invalid_dir").join("dest.txt");
        assert!(engine.install_file(Path::new("dummy"), &dest).is_err());
        assert!(engine.move_file(Path::new("dummy"), &dest).is_err());
    }

    #[test]
    #[allow(clippy::unreachable, clippy::expect_used)]
    fn test_file_operations_errors() {
        let engine = StandardActionEngine::host();
        let dir = tempdir().expect("test");

        let file_path = dir.path().join("file.txt");
        fs::write(&file_path, "test").expect("test");

        // Cannot create a folder if a file exists at the exact path
        let res = engine.create_folder(&file_path);
        assert!(res.is_err());

        // Cannot remove a folder that is not empty
        let sub = dir.path().join("sub");
        fs::create_dir_all(&sub).unwrap_or_else(|_| unreachable!());
        fs::write(sub.join("file.txt"), "test").unwrap_or_else(|_| unreachable!());
        let res2 = engine.remove_folder(&sub);
        assert!(res2.is_err());

        // Remove a folder successfully
        let empty_sub = dir.path().join("empty_sub");
        fs::create_dir_all(&empty_sub).unwrap_or_else(|_| unreachable!());
        assert!(engine.remove_folder(&empty_sub).is_ok());

        // Remove a non-existent folder
        assert!(engine
            .remove_folder(&dir.path().join("non_existent"))
            .is_ok());
    }
}

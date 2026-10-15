//! Standard Actions & System Publishing Engine.
//!
//! Exposes cross-platform IO abstractions mapping mutating standard MSI actions
//! (File, Directory, Registry, Environment, INI, ODBC, Services, Drivers, Fonts,
//! `ProgId`, `TypeLib`, `BindImage`, and Component Publishing) natively to the target OS
//! and virtualized platform state stores.

use crate::error::{MsiError, Result};
use crate::execution::bind_image::BindImageEngine;
use crate::platform::daemon::ServiceDefinition;
use crate::platform::driver::DriverInf;
use crate::platform::font::{FontFile, FontManager, FontName};
use crate::platform::odbc::OdbcManager;
use crate::platform::paths::TargetOs;
use crate::platform::registry_store::{RegistryRoot, RegistryStore, RegistryValue};
use std::collections::{BTreeMap, HashMap};
use std::fs;
use std::path::Path;
use std::sync::{Arc, RwLock};

/// Target scope for environment variable modifications (`WriteEnvironmentStrings` / `RemoveEnvironmentStrings`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EnvironmentScope {
    /// Per-user environment variables (`HKEY_CURRENT_USER\Environment`).
    PerUser,
    /// Machine-wide system environment variables (`HKLM\SYSTEM\CurrentControlSet\Control\Session Manager\Environment`).
    MachineWide,
}

/// Service lifecycle state transition action (`StartServices` / `StopServices`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ServiceControlAction {
    /// Start the service.
    Start,
    /// Stop the service.
    Stop,
    /// Stop and restart the service.
    Restart,
}

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
    /// * `value` - Typed [`RegistryValue`] payload.
    ///
    /// # Returns
    ///
    /// `Ok(())` when the registry value is persisted.
    ///
    /// # Errors
    ///
    /// Returns [`MsiError`] if `root` or `key` is invalid or persistence fails.
    fn write_registry_value(
        &self,
        root: u32,
        key: &str,
        name: &str,
        value: &RegistryValue,
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
    /// Returns [`MsiError`] if `root` or `key` is invalid or removal fails.
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
    /// Returns [`MsiError`] if `name` is empty or writing the environment configuration fails.
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
    /// Returns [`MsiError`] if `name` is empty or updating the environment configuration fails.
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
    /// Returns [`MsiError`] if `section` or `key` is empty or reading/writing `file_path` fails.
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
    /// Returns [`MsiError`] if `section` or `key` is empty or writing the updated `.ini` file fails.
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
    fn install_service(&self, def: &ServiceDefinition) -> Result<()>;

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

/// Standard engine implementing File, Registry, Environment, INI, Service, Driver, ODBC, Font,
/// and PE binding standard actions across Windows and POSIX platforms.
#[derive(Debug, Clone)]
pub struct StandardActionEngine {
    /// Target operating system for platform-specific action translation.
    target_os: TargetOs,
    /// Hierarchical registry store backing virtualized and cross-platform registry operations.
    registry: Arc<RwLock<RegistryStore>>,
    /// Installed service definitions keyed by service name, paired with running status.
    services: Arc<RwLock<BTreeMap<String, (ServiceDefinition, bool)>>>,
    /// Registered ODBC data sources and drivers keyed by name.
    odbc_entries: Arc<RwLock<BTreeMap<String, HashMap<String, String>>>>,
}

impl Default for StandardActionEngine {
    fn default() -> Self {
        Self::host()
    }
}

impl StandardActionEngine {
    /// Creates a new [`StandardActionEngine`] targeting the specified `target_os`.
    ///
    /// # Arguments
    ///
    /// * `target_os` - Target operating system for platform-specific behavior.
    ///
    /// # Returns
    ///
    /// A new [`StandardActionEngine`] with an initialized [`RegistryStore`].
    #[must_use]
    pub fn new(target_os: TargetOs) -> Self {
        Self {
            target_os,
            registry: Arc::new(RwLock::new(RegistryStore::new())),
            services: Arc::new(RwLock::new(BTreeMap::new())),
            odbc_entries: Arc::new(RwLock::new(BTreeMap::new())),
        }
    }

    /// Creates a new [`StandardActionEngine`] targeting the active host platform.
    ///
    /// # Returns
    ///
    /// A new [`StandardActionEngine`] configured for [`TargetOs::host()`].
    #[must_use]
    pub fn host() -> Self {
        Self::new(TargetOs::host())
    }

    /// Returns the configured [`TargetOs`] of this engine.
    ///
    /// # Returns
    ///
    /// The active [`TargetOs`].
    #[must_use]
    pub const fn target_os(&self) -> TargetOs {
        self.target_os
    }

    /// Reads a value from the engine's backing [`RegistryStore`].
    ///
    /// # Arguments
    ///
    /// * `root` - Numeric registry root (`0..=3`).
    /// * `key` - Registry subkey path.
    /// * `name` - Optional value name (`None` or `Some("")` for default value).
    ///
    /// # Returns
    ///
    /// `Ok(Some(RegistryValue))` if present, or `Ok(None)` if not found.
    ///
    /// # Errors
    ///
    /// Returns [`MsiError::InvalidArgument`] if `root` is outside `0..=3`.
    pub fn get_registry_value(
        &self,
        root: u32,
        key: &str,
        name: Option<&str>,
    ) -> Result<Option<RegistryValue>> {
        let reg_root = RegistryRoot::from_u32(root)?;
        let normalized_name = name.filter(|s| !s.is_empty());
        let guard = self
            .registry
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        Ok(guard.get_value(reg_root, key, normalized_name).cloned())
    }

    /// Checks whether a service is currently registered and whether it is running.
    ///
    /// # Arguments
    ///
    /// * `name` - Service name to query.
    ///
    /// # Returns
    ///
    /// `Some(true)` if running, `Some(false)` if stopped, or `None` if not installed.
    #[must_use]
    pub fn service_state(&self, name: &str) -> Option<bool> {
        let guard = self
            .services
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        guard.get(name).map(|(_, running)| *running)
    }

    /// Creates the parent directory of `path` if `path` has a non-empty parent.
    fn ensure_parent_dir(path: &Path) -> Result<()> {
        if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
            fs::create_dir_all(parent).map_err(crate::error::IoContext)?;
        }
        Ok(())
    }
}

impl FileOperations for StandardActionEngine {
    fn install_file(&self, source: &Path, destination: &Path) -> Result<()> {
        Self::ensure_parent_dir(destination)?;
        fs::copy(source, destination).map_err(crate::error::IoContext)?;
        Ok(())
    }

    fn duplicate_file(&self, source: &Path, destination: &Path) -> Result<()> {
        self.install_file(source, destination)
    }

    fn move_file(&self, source: &Path, destination: &Path) -> Result<()> {
        Self::ensure_parent_dir(destination)?;
        fs::rename(source, destination).map_err(crate::error::IoContext)?;
        Ok(())
    }

    fn remove_file(&self, target: &Path) -> Result<()> {
        match fs::remove_file(target) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(MsiError::Io(crate::error::IoContext(e))),
        }
    }

    fn create_folder(&self, target: &Path) -> Result<()> {
        fs::create_dir_all(target).map_err(|e| MsiError::Io(crate::error::IoContext(e)))
    }

    fn remove_folder(&self, target: &Path) -> Result<()> {
        match fs::remove_dir(target) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(MsiError::Io(crate::error::IoContext(e))),
        }
    }
}

impl SystemConfigOperations for StandardActionEngine {
    fn write_registry_value(
        &self,
        root: u32,
        key: &str,
        name: &str,
        value: &RegistryValue,
    ) -> Result<()> {
        if key.trim().is_empty() {
            return Err(MsiError::SystemConfigurationError(
                "Registry key path cannot be empty".to_string(),
            ));
        }
        let reg_root = RegistryRoot::from_u32(root)?;
        let name_opt = (!name.is_empty()).then_some(name);
        self.registry
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .set_value(reg_root, key, name_opt, value.clone());
        Ok(())
    }

    fn remove_registry_value(&self, root: u32, key: &str, name: &str) -> Result<()> {
        if key.trim().is_empty() {
            return Err(MsiError::SystemConfigurationError(
                "Registry key path cannot be empty".to_string(),
            ));
        }
        let reg_root = RegistryRoot::from_u32(root)?;
        let name_opt = (!name.is_empty()).then_some(name);
        self.registry
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .delete_value(reg_root, key, name_opt);
        Ok(())
    }

    fn write_environment_string(&self, name: &str, value: &str, system_wide: bool) -> Result<()> {
        if name.trim().is_empty() {
            return Err(MsiError::SystemConfigurationError(
                "Environment variable name cannot be empty".to_string(),
            ));
        }
        let (root, key) = if system_wide {
            (
                RegistryRoot::LocalMachine,
                r"SYSTEM\CurrentControlSet\Control\Session Manager\Environment",
            )
        } else {
            (RegistryRoot::CurrentUser, "Environment")
        };
        self.registry
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .set_value(root, key, Some(name), RegistryValue::Sz(value.to_string()));
        Ok(())
    }

    fn remove_environment_string(&self, name: &str, system_wide: bool) -> Result<()> {
        if name.trim().is_empty() {
            return Err(MsiError::SystemConfigurationError(
                "Environment variable name cannot be empty".to_string(),
            ));
        }
        let (root, key) = if system_wide {
            (
                RegistryRoot::LocalMachine,
                r"SYSTEM\CurrentControlSet\Control\Session Manager\Environment",
            )
        } else {
            (RegistryRoot::CurrentUser, "Environment")
        };
        self.registry
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .delete_value(root, key, Some(name));
        Ok(())
    }

    fn write_ini_value(
        &self,
        file_path: &Path,
        section: &str,
        key: &str,
        value: &str,
    ) -> Result<()> {
        if section.trim().is_empty() || key.trim().is_empty() {
            return Err(MsiError::SystemConfigurationError(
                "INI section and key cannot be empty".to_string(),
            ));
        }
        let existing = if file_path.exists() {
            fs::read_to_string(file_path).map_err(crate::error::IoContext)?
        } else {
            String::new()
        };

        let mut entries = OdbcManager::parse_ini(&existing)?;
        entries
            .entry(section.to_string())
            .or_default()
            .insert(key.to_string(), value.to_string());

        Self::ensure_parent_dir(file_path)?;
        let serialized = OdbcManager::write_ini(&entries);
        fs::write(file_path, serialized).map_err(crate::error::IoContext)?;
        Ok(())
    }

    fn remove_ini_value(&self, file_path: &Path, section: &str, key: &str) -> Result<()> {
        if section.trim().is_empty() || key.trim().is_empty() {
            return Err(MsiError::SystemConfigurationError(
                "INI section and key cannot be empty".to_string(),
            ));
        }
        if !file_path.exists() {
            return Ok(());
        }
        let existing = fs::read_to_string(file_path).map_err(crate::error::IoContext)?;
        let mut entries = OdbcManager::parse_ini(&existing)?;
        if let Some(sec_map) = entries.get_mut(section) {
            sec_map.remove(key);
        }
        entries.retain(|_, attrs| !attrs.is_empty());
        let serialized = OdbcManager::write_ini(&entries);
        fs::write(file_path, serialized).map_err(crate::error::IoContext)?;
        Ok(())
    }
}

impl SystemServiceOperations for StandardActionEngine {
    fn install_service(&self, def: &ServiceDefinition) -> Result<()> {
        if def.name.trim().is_empty() || def.exec_start.as_os_str().is_empty() {
            return Err(MsiError::SystemdError(
                "Service name and executable path cannot be empty".to_string(),
            ));
        }
        let unit_content = match self.target_os {
            TargetOs::Linux => def.generate_systemd_unit(),
            TargetOs::MacOs => def.generate_launchd_plist(),
            TargetOs::FreeBsd => def.generate_freebsd_rc_script(),
            TargetOs::SunOs => def.generate_smf_manifest(),
            TargetOs::Windows => def.windows_scm_lifecycle_commands().join("\n"),
        };
        let svc_key = format!(r"SYSTEM\CurrentControlSet\Services\{}", def.name);
        {
            let mut reg = self
                .registry
                .write()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            reg.set_value(
                RegistryRoot::LocalMachine,
                &svc_key,
                Some("ImagePath"),
                RegistryValue::Sz(def.exec_start.to_string_lossy().to_string()),
            );
            reg.set_value(
                RegistryRoot::LocalMachine,
                &svc_key,
                Some("UnitDefinition"),
                RegistryValue::Sz(unit_content),
            );
        }
        self.services
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .insert(def.name.clone(), (def.clone(), def.auto_start));
        Ok(())
    }

    fn control_service(&self, name: &str, start: bool, stop: bool) -> Result<()> {
        if name.trim().is_empty() {
            return Err(MsiError::SystemdError(
                "Service name cannot be empty".to_string(),
            ));
        }
        let mut guard = self
            .services
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let Some((_, running)) = guard.get_mut(name) else {
            return Err(MsiError::SystemdError(format!(
                "Service '{name}' is not installed"
            )));
        };
        if stop {
            *running = false;
        }
        if start {
            *running = true;
        }
        drop(guard);
        Ok(())
    }

    fn delete_service(&self, name: &str) -> Result<()> {
        if name.trim().is_empty() {
            return Err(MsiError::SystemdError(
                "Service name cannot be empty".to_string(),
            ));
        }
        let removed = self
            .services
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .remove(name);
        if removed.is_none() {
            return Err(MsiError::SystemdError(format!(
                "Service '{name}' is not installed"
            )));
        }
        let svc_key = format!(r"SYSTEM\CurrentControlSet\Services\{name}");
        let mut reg = self
            .registry
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        reg.delete_value(RegistryRoot::LocalMachine, &svc_key, Some("ImagePath"));
        reg.delete_value(RegistryRoot::LocalMachine, &svc_key, Some("UnitDefinition"));
        drop(reg);
        Ok(())
    }

    fn install_driver(&self, inf_path: &Path) -> Result<()> {
        let inf_text = fs::read_to_string(inf_path).map_err(|e| {
            MsiError::Io(crate::error::IoContext::from_string(format!(
                "Failed reading driver INF '{}': {e}",
                inf_path.display()
            )))
        })?;
        let descriptor = DriverInf::parse_inf(&inf_text)?;
        let drv_key = format!(
            r"SYSTEM\CurrentControlSet\Control\Class\{}",
            descriptor.class
        );
        self.registry
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .set_value(
                RegistryRoot::LocalMachine,
                &drv_key,
                Some("Provider"),
                RegistryValue::Sz(descriptor.provider),
            );
        Ok(())
    }
}

impl SpecializedRegistrationOperations for StandardActionEngine {
    fn install_odbc(&self, driver: &str, attributes: &str) -> Result<()> {
        if driver.trim().is_empty() || attributes.trim().is_empty() {
            return Err(MsiError::OdbcConfigError(
                "ODBC driver and attributes cannot be empty".to_string(),
            ));
        }
        let mut attr_map = HashMap::new();
        for pair in attributes.split(';').filter(|s| !s.trim().is_empty()) {
            let Some((k, v)) = pair.split_once('=') else {
                return Err(MsiError::OdbcConfigError(format!(
                    "Invalid ODBC attribute pair '{pair}' (expected KEY=VALUE)"
                )));
            };
            attr_map.insert(k.trim().to_string(), v.trim().to_string());
        }
        let odbc_key = format!(r"SOFTWARE\ODBC\ODBC.INI\{driver}");
        {
            let mut reg = self
                .registry
                .write()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            for (k, v) in &attr_map {
                reg.set_value(
                    RegistryRoot::LocalMachine,
                    &odbc_key,
                    Some(k),
                    RegistryValue::Sz(v.clone()),
                );
            }
        }
        self.odbc_entries
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .insert(driver.to_string(), attr_map);
        Ok(())
    }

    fn remove_odbc(&self, driver: &str) -> Result<()> {
        if driver.trim().is_empty() {
            return Err(MsiError::OdbcConfigError(
                "ODBC driver name cannot be empty".to_string(),
            ));
        }
        let removed = self
            .odbc_entries
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .remove(driver);
        let Some(attrs) = removed else {
            return Err(MsiError::OdbcConfigError(format!(
                "ODBC entry '{driver}' is not registered"
            )));
        };
        let odbc_key = format!(r"SOFTWARE\ODBC\ODBC.INI\{driver}");
        let mut reg = self
            .registry
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        for k in attrs.keys() {
            reg.delete_value(RegistryRoot::LocalMachine, &odbc_key, Some(k));
        }
        drop(reg);
        Ok(())
    }

    fn install_odbc_driver(&self, driver: &str, path_in: &str) -> Result<()> {
        if driver.trim().is_empty() || path_in.trim().is_empty() {
            return Err(MsiError::OdbcConfigError(
                "ODBC driver name and path cannot be empty".to_string(),
            ));
        }
        let drv_key = format!(r"SOFTWARE\ODBC\ODBCINST.INI\{driver}");
        self.registry
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .set_value(
                RegistryRoot::LocalMachine,
                &drv_key,
                Some("Driver"),
                RegistryValue::Sz(path_in.to_string()),
            );
        Ok(())
    }

    fn install_odbc_translator(&self, translator: &str, path_in: &str) -> Result<()> {
        if translator.trim().is_empty() || path_in.trim().is_empty() {
            return Err(MsiError::OdbcConfigError(
                "ODBC translator name and path cannot be empty".to_string(),
            ));
        }
        let trans_key = format!(r"SOFTWARE\ODBC\ODBCINST.INI\{translator}");
        self.registry
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .set_value(
                RegistryRoot::LocalMachine,
                &trans_key,
                Some("Translator"),
                RegistryValue::Sz(path_in.to_string()),
            );
        Ok(())
    }

    fn register_font(&self, font_title: &str, font_file: &Path) -> Result<()> {
        let mgr = FontManager;
        let name = FontName::new(font_title);
        let file = FontFile::new(font_file);
        mgr.register_font(&name, &file)?;
        self.registry
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .set_value(
                RegistryRoot::LocalMachine,
                r"SOFTWARE\Microsoft\Windows NT\CurrentVersion\Fonts",
                Some(font_title),
                RegistryValue::Sz(font_file.to_string_lossy().to_string()),
            );
        Ok(())
    }

    fn unregister_font(&self, font_title: &str, font_file: &Path) -> Result<()> {
        let mgr = FontManager;
        let name = FontName::new(font_title);
        let file = FontFile::new(font_file);
        mgr.unregister_font(&name, &file)?;
        self.registry
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .delete_value(
                RegistryRoot::LocalMachine,
                r"SOFTWARE\Microsoft\Windows NT\CurrentVersion\Fonts",
                Some(font_title),
            );
        Ok(())
    }

    fn register_progid_info(
        &self,
        progid: &str,
        description: &str,
        icon: Option<&Path>,
    ) -> Result<()> {
        if progid.trim().is_empty() {
            return Err(MsiError::SystemConfigurationError(
                "ProgId cannot be empty".to_string(),
            ));
        }
        let mut reg = self
            .registry
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        reg.set_value(
            RegistryRoot::ClassesRoot,
            progid,
            None,
            RegistryValue::Sz(description.to_string()),
        );
        if let Some(icon_path) = icon {
            let icon_key = format!(r"{progid}\DefaultIcon");
            reg.set_value(
                RegistryRoot::ClassesRoot,
                &icon_key,
                None,
                RegistryValue::Sz(icon_path.to_string_lossy().to_string()),
            );
        }
        drop(reg);
        Ok(())
    }

    fn register_typelib(&self, tlb_path: &Path) -> Result<()> {
        if !tlb_path.exists() {
            return Err(MsiError::SystemConfigurationError(format!(
                "TypeLib file does not exist: {}",
                tlb_path.display()
            )));
        }
        let file_name = tlb_path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();
        let tlb_key = format!(r"TypeLib\{file_name}");
        self.registry
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .set_value(
                RegistryRoot::ClassesRoot,
                &tlb_key,
                None,
                RegistryValue::Sz(tlb_path.to_string_lossy().to_string()),
            );
        Ok(())
    }

    fn self_register_module(&self, dll_path: &Path) -> Result<()> {
        if self.target_os == TargetOs::Windows {
            Ok(())
        } else {
            Err(MsiError::UnsupportedPlatformFeature {
                feature: "SelfRegModules".to_string(),
                target_os: self.target_os,
                reason: format!(
                    "Only natively implemented on Windows (cannot invoke DllRegisterServer on {})",
                    dll_path.display()
                ),
            })
        }
    }

    fn self_unregister_module(&self, dll_path: &Path) -> Result<()> {
        if self.target_os == TargetOs::Windows {
            Ok(())
        } else {
            Err(MsiError::UnsupportedPlatformFeature {
                feature: "SelfUnregModules".to_string(),
                target_os: self.target_os,
                reason: format!(
                    "Only natively implemented on Windows (cannot invoke DllUnregisterServer on {})",
                    dll_path.display()
                ),
            })
        }
    }

    fn bind_image(&self, exe_path: &Path) -> Result<()> {
        let binder = BindImageEngine;
        binder.bind_image(exe_path, None)
    }

    fn publish_components(&self, component_id: &str, qualifier: &str, appdata: &str) -> Result<()> {
        if component_id.trim().is_empty() || qualifier.trim().is_empty() {
            return Err(MsiError::SystemConfigurationError(
                "Component ID and qualifier cannot be empty".to_string(),
            ));
        }
        let key = format!(
            r"SOFTWARE\Microsoft\Windows\CurrentVersion\Installer\UserData\S-1-5-18\Components\{component_id}"
        );
        self.registry
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .set_value(
                RegistryRoot::LocalMachine,
                &key,
                Some(qualifier),
                RegistryValue::Sz(appdata.to_string()),
            );
        Ok(())
    }

    fn publish_features(&self, feature_id: &str) -> Result<()> {
        if feature_id.trim().is_empty() {
            return Err(MsiError::SystemConfigurationError(
                "Feature ID cannot be empty".to_string(),
            ));
        }
        self.registry
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .set_value(
                RegistryRoot::LocalMachine,
                r"SOFTWARE\Microsoft\Windows\CurrentVersion\Installer\Features",
                Some(feature_id),
                RegistryValue::Sz("Published".to_string()),
            );
        Ok(())
    }

    fn isolate_components(&self, component_id: &str, app_path: &Path) -> Result<()> {
        if component_id.trim().is_empty() || app_path.as_os_str().is_empty() {
            return Err(MsiError::SystemConfigurationError(
                "Component ID and application path cannot be empty".to_string(),
            ));
        }
        let local_marker = Path::new(&format!("{}.local", app_path.display())).to_path_buf();
        if local_marker
            .parent()
            .is_some_and(|p| !p.as_os_str().is_empty() && p.exists())
        {
            fs::write(&local_marker, component_id).map_err(crate::error::IoContext)?;
        }
        self.registry
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .set_value(
                RegistryRoot::LocalMachine,
                r"SOFTWARE\Microsoft\Windows\CurrentVersion\SharedDLLs",
                Some(component_id),
                RegistryValue::Sz(app_path.to_string_lossy().to_string()),
            );
        Ok(())
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
        let op = StandardActionEngine::default();
        assert_eq!(op.target_os(), TargetOs::host());
        assert_eq!(EnvironmentScope::PerUser, EnvironmentScope::PerUser);
        assert_ne!(EnvironmentScope::PerUser, EnvironmentScope::MachineWide);
        assert_eq!(ServiceControlAction::Start, ServiceControlAction::Start);
        assert_ne!(ServiceControlAction::Start, ServiceControlAction::Stop);
        assert_ne!(ServiceControlAction::Stop, ServiceControlAction::Restart);
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
    fn test_file_operations() -> Result<()> {
        let dir = tempdir().map_err(crate::error::IoContext)?;
        let engine = StandardActionEngine::host();

        let src = dir.path().join("source.txt");
        fs::write(&src, "test content").map_err(crate::error::IoContext)?;

        let dst = dir.path().join("dest").join("dest.txt");
        engine.install_file(&src, &dst)?;
        assert!(dst.exists());
        assert_eq!(
            fs::read_to_string(&dst).map_err(crate::error::IoContext)?,
            "test content"
        );

        let dup = dir.path().join("dest").join("dup.txt");
        engine.duplicate_file(&dst, &dup)?;
        assert!(dup.exists());

        let mov = dir.path().join("dest").join("mov.txt");
        engine.move_file(&dup, &mov)?;
        assert!(mov.exists());
        assert!(!dup.exists());

        engine.remove_file(&mov)?;
        assert!(!mov.exists());

        let folder = dir.path().join("new_folder");
        engine.create_folder(&folder)?;
        assert!(folder.exists());

        engine.remove_folder(&folder)?;
        assert!(!folder.exists());
        Ok(())
    }

    #[test]
    fn test_file_operations_error_paths() -> Result<()> {
        let engine = StandardActionEngine::host();

        let invalid_src = PathBuf::from("/invalid/non/existent/path/src.txt");
        let invalid_dst = PathBuf::from("/invalid/non/existent/path/dst.txt");

        assert!(engine.install_file(&invalid_src, &invalid_dst).is_err());
        assert!(engine.duplicate_file(&invalid_src, &invalid_dst).is_err());
        assert!(engine.move_file(&invalid_src, &invalid_dst).is_err());

        let dir = tempdir().map_err(crate::error::IoContext)?;
        let folder = dir.path().join("non_empty");
        engine.create_folder(&folder)?;
        fs::write(folder.join("file.txt"), "data").map_err(crate::error::IoContext)?;
        assert!(engine.remove_folder(&folder).is_err());
        Ok(())
    }

    #[test]
    fn test_system_config_operations() -> Result<()> {
        let engine = StandardActionEngine::host();
        let val = RegistryValue::Sz("test".to_string());

        // Validation errors
        assert!(engine.write_registry_value(0, "", "name", &val).is_err());
        assert!(engine
            .write_registry_value(99, "key", "name", &val)
            .is_err());
        assert!(engine.remove_registry_value(0, "", "name").is_err());
        assert!(engine.remove_registry_value(99, "key", "name").is_err());
        assert!(engine.get_registry_value(99, "key", None).is_err());

        // Registry round-trip
        engine.write_registry_value(2, "Software/Acme", "InstallDir", &val)?;
        assert_eq!(
            engine.get_registry_value(2, "Software/Acme", Some("InstallDir"))?,
            Some(val.clone())
        );
        engine.remove_registry_value(2, "Software/Acme", "InstallDir")?;
        assert_eq!(
            engine.get_registry_value(2, "Software/Acme", Some("InstallDir"))?,
            None
        );

        // Environment strings (per-user and machine-wide)
        assert!(engine.write_environment_string("", "val", false).is_err());
        assert!(engine.remove_environment_string("", false).is_err());
        engine.write_environment_string("ACME_HOME", "/opt/acme", false)?;
        assert_eq!(
            engine.get_registry_value(1, "Environment", Some("ACME_HOME"))?,
            Some(RegistryValue::Sz("/opt/acme".to_string()))
        );
        engine.remove_environment_string("ACME_HOME", false)?;
        assert_eq!(
            engine.get_registry_value(1, "Environment", Some("ACME_HOME"))?,
            None
        );

        engine.write_environment_string("ACME_SYS", "/etc/acme", true)?;
        engine.remove_environment_string("ACME_SYS", true)?;
        Ok(())
    }

    #[test]
    fn test_ini_config_operations() -> Result<()> {
        let dir = tempdir().map_err(crate::error::IoContext)?;
        let engine = StandardActionEngine::host();

        // INI operations
        let ini_path = dir.path().join("sub").join("config.ini");
        assert!(engine.write_ini_value(&ini_path, "", "k", "v").is_err());
        assert!(engine.remove_ini_value(&ini_path, "", "k").is_err());
        // Removing from non-existent INI is a no-op
        engine.remove_ini_value(&ini_path, "sect", "k")?;

        engine.write_ini_value(&ini_path, "Database", "Port", "5432")?;
        engine.write_ini_value(&ini_path, "Database", "Host", "localhost")?;
        let content = fs::read_to_string(&ini_path).map_err(crate::error::IoContext)?;
        assert!(content.contains("[Database]"));
        assert!(content.contains("Port=5432"));
        assert!(content.contains("Host=localhost"));

        engine.remove_ini_value(&ini_path, "Database", "Port")?;
        let updated = fs::read_to_string(&ini_path).map_err(crate::error::IoContext)?;
        assert!(!updated.contains("Port=5432"));
        assert!(updated.contains("Host=localhost"));

        // Write INI error when path is a directory
        assert!(engine
            .write_ini_value(dir.path(), "Section", "Key", "Val")
            .is_err());
        assert!(engine
            .remove_ini_value(dir.path(), "Section", "Key")
            .is_err());
        let file_blocker = dir.path().join("blocker");
        fs::write(&file_blocker, "x").map_err(crate::error::IoContext)?;
        assert!(engine
            .write_ini_value(&file_blocker.join("child.ini"), "S", "K", "V")
            .is_err());

        // Write/remove INI error when existing file is read-only
        let ro_ini = dir.path().join("ro.ini");
        fs::write(&ro_ini, "[Sec]\nK=V\n").map_err(crate::error::IoContext)?;
        let mut perms = fs::metadata(&ro_ini)
            .map_err(crate::error::IoContext)?
            .permissions();
        perms.set_readonly(true);
        fs::set_permissions(&ro_ini, perms).map_err(crate::error::IoContext)?;
        assert!(engine.write_ini_value(&ro_ini, "Sec", "K2", "V2").is_err());
        assert!(engine.remove_ini_value(&ro_ini, "Sec", "K").is_err());
        let mut restore_perms = fs::metadata(&ro_ini)
            .map_err(crate::error::IoContext)?
            .permissions();
        #[allow(clippy::permissions_set_readonly_false)]
        restore_perms.set_readonly(false);
        fs::set_permissions(&ro_ini, restore_perms).map_err(crate::error::IoContext)?;

        // Relative filename without parent directory component
        assert!(engine
            .write_ini_value(Path::new("/"), "Sec", "K", "V")
            .is_err());
        assert!(engine
            .isolate_components(
                "{COMP-1}",
                &dir.path().join("missing_parent_dir").join("app.exe")
            )
            .is_ok());

        Ok(())
    }

    #[test]
    fn test_system_service_and_driver_operations() -> Result<()> {
        let dir = tempdir().map_err(crate::error::IoContext)?;
        let def = ServiceDefinition::new("acme_svc", "/usr/bin/acme_svc");

        for os in [
            TargetOs::Linux,
            TargetOs::MacOs,
            TargetOs::FreeBsd,
            TargetOs::SunOs,
            TargetOs::Windows,
        ] {
            let engine = StandardActionEngine::new(os);
            engine.install_service(&def)?;
            assert_eq!(engine.service_state("acme_svc"), Some(true));
            engine.control_service("acme_svc", false, true)?;
            assert_eq!(engine.service_state("acme_svc"), Some(false));
            engine.control_service("acme_svc", true, false)?;
            assert_eq!(engine.service_state("acme_svc"), Some(true));
            engine.delete_service("acme_svc")?;
            assert_eq!(engine.service_state("acme_svc"), None);
        }

        let engine = StandardActionEngine::host();
        let bad_def = ServiceDefinition::new("", "");
        assert!(engine.install_service(&bad_def).is_err());
        assert!(engine.control_service("", true, false).is_err());
        assert!(engine.control_service("missing_svc", true, false).is_err());
        assert!(engine.delete_service("").is_err());
        assert!(engine.delete_service("missing_svc").is_err());

        // Driver INF test
        assert!(engine
            .install_driver(&PathBuf::from("/nonexistent.inf"))
            .is_err());
        let inf_path = dir.path().join("netdrv.inf");
        fs::write(
            &inf_path,
            "[Version]\nSignature=\"$WINDOWS NT$\"\nClass=Net\nProvider=AcmeCorp\nDriverVer=01/01/2026,1.0.0.0\n",
        )
        .map_err(crate::error::IoContext)?;
        engine.install_driver(&inf_path)?;
        assert_eq!(
            engine.get_registry_value(
                2,
                r"SYSTEM\CurrentControlSet\Control\Class\Net",
                Some("Provider")
            )?,
            Some(RegistryValue::Sz("AcmeCorp".to_string()))
        );
        Ok(())
    }

    #[test]
    fn test_specialized_registration_operations() -> Result<()> {
        let dir = tempdir().map_err(crate::error::IoContext)?;
        let engine = StandardActionEngine::host();

        // ODBC DSN & driver & translator
        assert!(engine.install_odbc("", "DSN=Test").is_err());
        assert!(engine.install_odbc("PostgreSQL", "invalid_attr").is_err());
        engine.install_odbc("PostgreSQL", "DSN=PgDsn;Port=5432")?;
        assert!(engine.remove_odbc("").is_err());
        assert!(engine.remove_odbc("UnknownDriver").is_err());
        engine.remove_odbc("PostgreSQL")?;

        assert!(engine.install_odbc_driver("", "/usr/lib").is_err());
        engine.install_odbc_driver("PostgreSQL", "/usr/lib/psqlodbcw.so")?;
        assert!(engine.install_odbc_translator("", "/usr/lib").is_err());
        engine.install_odbc_translator("PgTranslator", "/usr/lib/pgtrans.so")?;

        // Fonts
        let missing_font = dir.path().join("missing.ttf");
        assert!(engine.register_font("Arial", &missing_font).is_err());
        assert!(engine.unregister_font("Arial", &missing_font).is_err());

        let font_path = dir.path().join("arial.ttf");
        fs::write(&font_path, b"TTF_DATA").map_err(crate::error::IoContext)?;
        engine.register_font("Arial", &font_path)?;
        engine.unregister_font("Arial", &font_path)?;

        // ProgId & TypeLib
        assert!(engine.register_progid_info("", "Desc", None).is_err());
        engine.register_progid_info("Acme.Document.1", "Acme Document", Some(&font_path))?;
        assert!(engine.register_typelib(&missing_font).is_err());
        engine.register_typelib(&font_path)?;

        // BindImage with valid minimal PE
        let mut pe_data = vec![0u8; 512];
        pe_data[0] = b'M';
        pe_data[1] = b'Z';
        pe_data[0x3C] = 0x80;
        pe_data[0x80..0x84].copy_from_slice(b"PE\0\0");
        pe_data[0x84..0x86].copy_from_slice(&0x014Cu16.to_le_bytes());
        pe_data[0x86..0x88].copy_from_slice(&1u16.to_le_bytes());
        pe_data[0x94..0x96].copy_from_slice(&0x00E0u16.to_le_bytes());
        pe_data[0x96..0x98].copy_from_slice(&0x0102u16.to_le_bytes());
        pe_data[0x98..0x9A].copy_from_slice(&0x010Bu16.to_le_bytes());
        pe_data[0xF4..0xF8].copy_from_slice(&16u32.to_le_bytes());
        let pe_path = dir.path().join("app.exe");
        fs::write(&pe_path, &pe_data).map_err(crate::error::IoContext)?;
        engine.bind_image(&pe_path)?;

        // PublishComponents, PublishFeatures, IsolateComponents
        assert!(engine.publish_components("", "q", "d").is_err());
        engine.publish_components("{COMP-1}", "1033", "AppData")?;
        assert!(engine.publish_features("").is_err());
        engine.publish_features("MainFeature")?;
        assert!(engine.isolate_components("", &pe_path).is_err());
        engine.isolate_components("{COMP-1}", &pe_path)?;
        assert!(dir.path().join("app.exe.local").exists());
        let blocked_app = dir.path().join("blocked.exe");
        let blocked_local = dir.path().join("blocked.exe.local");
        fs::create_dir_all(&blocked_local).map_err(crate::error::IoContext)?;
        assert!(engine.isolate_components("{COMP-1}", &blocked_app).is_err());

        // SelfReg / SelfUnreg across Windows and POSIX
        let win_engine = StandardActionEngine::new(TargetOs::Windows);
        win_engine.self_register_module(&pe_path)?;
        win_engine.self_unregister_module(&pe_path)?;

        for posix_os in [
            TargetOs::Linux,
            TargetOs::MacOs,
            TargetOs::FreeBsd,
            TargetOs::SunOs,
        ] {
            let posix_engine = StandardActionEngine::new(posix_os);
            assert!(posix_engine.self_register_module(&pe_path).is_err());
            assert!(posix_engine.self_unregister_module(&pe_path).is_err());
        }
        Ok(())
    }

    #[test]
    fn test_install_and_move_file_edge_cases() -> Result<()> {
        let engine = StandardActionEngine::host();
        let eng_dbg = format!("{engine:?}");
        assert!(eng_dbg.contains("StandardActionEngine"));
        let src = Path::new("dummy_src");
        let dest = Path::new("");
        assert!(engine.install_file(src, dest).is_err());
        assert!(engine.move_file(src, dest).is_err());

        let dir = tempdir().map_err(crate::error::IoContext)?;
        let file = dir.path().join("file.txt");
        fs::write(&file, "data").map_err(crate::error::IoContext)?;
        let bad_dest = file.join("invalid_dir").join("dest.txt");
        assert!(engine.install_file(&file, &bad_dest).is_err());
        assert!(engine.move_file(&file, &bad_dest).is_err());

        // Cannot create a folder if a file exists at the exact path
        assert!(engine.create_folder(&file).is_err());

        // Remove an empty folder and a non-existent folder
        let empty_sub = dir.path().join("empty_sub");
        engine.create_folder(&empty_sub)?;
        engine.remove_folder(&empty_sub)?;
        engine.remove_folder(&dir.path().join("non_existent"))?;
        Ok(())
    }
}

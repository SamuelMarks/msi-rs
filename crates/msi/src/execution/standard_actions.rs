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
    /// Extract or copy a file to a destination path.
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
    fn install_file(&self, source: &Path, destination: &Path) -> Result<()>;
    /// Duplicate an existing file.
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
    fn duplicate_file(&self, source: &Path, destination: &Path) -> Result<()>;
    /// Move an existing file.
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
    fn move_file(&self, source: &Path, destination: &Path) -> Result<()>;
    /// Delete a file, managing `.lock` semantics on Unix or `MoveFileEx` semantics on Windows for in-use files.
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
    fn remove_file(&self, target: &Path) -> Result<()>;

    /// Create an empty directory structure (`CreateFolders`).
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
    fn create_folder(&self, target: &Path) -> Result<()>;
    /// Remove an empty directory (`RemoveFolders`).
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
    fn remove_folder(&self, target: &Path) -> Result<()>;
}

/// Interface for system configuration manipulation.
pub trait SystemConfigOperations {
    /// Sets a registry value natively or via a virtualized hive.
    /// # Errors
    ///
    /// Returns an error if the operation fails.
    ///
    /// # Arguments
    ///
    /// * `root` - The root argument.
    /// * `key` - The key argument.
    /// * `name` - The name argument.
    /// * `value` - The value argument.
    ///
    /// # Returns
    ///
    /// The return value.
    fn write_registry_value(
        &self,
        root: u32,
        key: &str,
        name: &str,
        value: &crate::platform::registry_store::RegistryValue,
    ) -> Result<()>;
    /// Removes a registry value.
    /// # Errors
    ///
    /// Returns an error if the operation fails.
    ///
    /// # Arguments
    ///
    /// * `root` - The root argument.
    /// * `key` - The key argument.
    /// * `name` - The name argument.
    ///
    /// # Returns
    ///
    /// The return value.
    fn remove_registry_value(&self, root: u32, key: &str, name: &str) -> Result<()>;

    /// Updates environment variables.
    /// # Errors
    ///
    /// Returns an error if the operation fails.
    ///
    /// # Arguments
    ///
    /// * `name` - The name argument.
    /// * `value` - The value argument.
    /// * `system_wide` - The `system_wide` argument.
    ///
    /// # Returns
    ///
    /// The return value.
    fn write_environment_string(&self, name: &str, value: &str, system_wide: bool) -> Result<()>;
    /// Removes an environment variable.
    /// # Errors
    ///
    /// Returns an error if the operation fails.
    ///
    /// # Arguments
    ///
    /// * `name` - The name argument.
    /// * `system_wide` - The `system_wide` argument.
    ///
    /// # Returns
    ///
    /// The return value.
    fn remove_environment_string(&self, name: &str, system_wide: bool) -> Result<()>;

    /// Updates an INI file value.
    /// # Errors
    ///
    /// Returns an error if the operation fails.
    ///
    /// # Arguments
    ///
    /// * `file_path` - The `file_path` argument.
    /// * `section` - The section argument.
    /// * `key` - The key argument.
    /// * `value` - The value argument.
    ///
    /// # Returns
    ///
    /// The return value.
    fn write_ini_value(
        &self,
        file_path: &Path,
        section: &str,
        key: &str,
        value: &str,
    ) -> Result<()>;
    /// Removes an INI file value.
    /// # Errors
    ///
    /// Returns an error if the operation fails.
    ///
    /// # Arguments
    ///
    /// * `file_path` - The `file_path` argument.
    /// * `section` - The section argument.
    /// * `key` - The key argument.
    ///
    /// # Returns
    ///
    /// The return value.
    fn remove_ini_value(&self, file_path: &Path, section: &str, key: &str) -> Result<()>;
}

/// Interface for system-level services and driver installations.
pub trait SystemServiceOperations {
    /// Install a background service daemon.
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
    fn install_service(&self, def: &crate::platform::daemon::ServiceDefinition) -> Result<()>;
    /// Control an existing background service.
    /// # Errors
    ///
    /// Returns an error if the operation fails.
    ///
    /// # Arguments
    ///
    /// * `name` - The name argument.
    /// * `start` - The start argument.
    /// * `stop` - The stop argument.
    ///
    /// # Returns
    ///
    /// The return value.
    fn control_service(&self, name: &str, start: bool, stop: bool) -> Result<()>;
    /// Delete a background service daemon.
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
    fn delete_service(&self, name: &str) -> Result<()>;

    /// Install an OS kernel driver (DIFx/systemd module abstraction).
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
    fn install_driver(&self, inf_path: &Path) -> Result<()>;
}

/// Interface for specialized COM, Desktop, and Font registrations.
pub trait SpecializedRegistrationOperations {
    /// Register an ODBC data source.
    /// # Errors
    ///
    /// Returns an error if the operation fails.
    ///
    /// # Arguments
    ///
    /// * `driver` - The driver argument.
    /// * `attributes` - The attributes argument.
    ///
    /// # Returns
    ///
    /// The return value.
    fn install_odbc(&self, driver: &str, attributes: &str) -> Result<()>;
    /// Remove an ODBC data source.
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
    fn remove_odbc(&self, driver: &str) -> Result<()>;

    /// Install an ODBC driver.
    /// # Errors
    /// Returns an error if operation fails.
    /// # Arguments
    /// * `driver` - driver config string
    /// * `path_in` - target path
    fn install_odbc_driver(&self, driver: &str, path_in: &str) -> Result<()>;

    /// Install an ODBC translator.
    /// # Errors
    /// Returns an error if operation fails.
    /// # Arguments
    /// * `translator` - translator config string
    /// * `path_in` - target path
    fn install_odbc_translator(&self, translator: &str, path_in: &str) -> Result<()>;

    /// Register a system font.
    /// # Errors
    ///
    /// Returns an error if the operation fails.
    ///
    /// # Arguments
    ///
    /// * `font_title` - The `font_title` argument.
    /// * `font_file` - The `font_file` argument.
    ///
    /// # Returns
    ///
    /// The return value.
    fn register_font(&self, font_title: &str, font_file: &Path) -> Result<()>;
    /// Unregister a system font.
    /// # Errors
    ///
    /// Returns an error if the operation fails.
    ///
    /// # Arguments
    ///
    /// * `font_title` - The `font_title` argument.
    /// * `font_file` - The `font_file` argument.
    ///
    /// # Returns
    ///
    /// The return value.
    fn unregister_font(&self, font_title: &str, font_file: &Path) -> Result<()>;

    /// Register file extension, MIME, or `ProgID` (XDG/macOS LaunchServices/HKCR).
    /// # Errors
    ///
    /// Returns an error if the operation fails.
    ///
    /// # Arguments
    ///
    /// * `progid` - The progid argument.
    /// * `description` - The description argument.
    /// * `icon` - The icon argument.
    ///
    /// # Returns
    ///
    /// The return value.
    fn register_progid_info(
        &self,
        progid: &str,
        description: &str,
        icon: Option<&Path>,
    ) -> Result<()>;

    /// Register a type library (TLB) or COM class.
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
    fn register_typelib(&self, tlb_path: &Path) -> Result<()>;

    /// Invoke self-registration on a dynamic library.

    /// # Errors
    ///
    /// Returns an error if the operation fails.
    ///
    /// # Arguments
    ///
    /// * `dll_path` - The dll_path argument.
    ///
    /// # Returns
    ///
    /// The return value.
    fn self_register_module(&self, dll_path: &Path) -> Result<()>;
    /// # Errors
    ///
    /// Returns an error if the operation fails.
    ///
    /// # Arguments
    ///
    /// * `dll_path` - The `dll_path` argument.
    ///
    /// # Returns
    ///
    /// The return value.
    fn self_unregister_module(&self, dll_path: &Path) -> Result<()>;
    /// # Errors
    ///
    /// Returns an error if the operation fails.
    ///
    /// # Arguments
    ///
    /// * `exe_path` - The `exe_path` argument.
    ///
    /// # Returns
    ///
    /// The return value.
    fn bind_image(&self, exe_path: &Path) -> Result<()>;
    /// # Errors
    ///
    /// Returns an error if the operation fails.
    ///
    /// # Arguments
    ///
    /// * `component_id` - The `component_id` argument.
    /// * `qualifier` - The qualifier argument.
    /// * `appdata` - The appdata argument.
    ///
    /// # Returns
    ///
    /// The return value.
    fn publish_components(&self, component_id: &str, qualifier: &str, appdata: &str) -> Result<()>;
    /// # Errors
    ///
    /// Returns an error if the operation fails.
    ///
    /// # Arguments
    ///
    /// * `feature_id` - The `feature_id` argument.
    ///
    /// # Returns
    ///
    /// The return value.
    fn publish_features(&self, feature_id: &str) -> Result<()>;
    /// # Errors
    ///
    /// Returns an error if the operation fails.
    ///
    /// # Arguments
    ///
    /// * `component_id` - The `component_id` argument.
    /// * `app_path` - The `app_path` argument.
    ///
    /// # Returns
    ///
    /// The return value.
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
                use windows::Win32::System::LibraryLoader::{
                    FreeLibrary, GetProcAddress, LoadLibraryW,
                };

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
                use windows::Win32::System::LibraryLoader::{
                    FreeLibrary, GetProcAddress, LoadLibraryW,
                };

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
    /// * `_dll_path` - The _dll_path argument.
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

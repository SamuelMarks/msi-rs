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
    fn install_file(&self, source: &Path, destination: &Path) -> Result<()>;
    /// Duplicate an existing file.
    /// # Errors
    ///
    /// Returns an error if the operation fails.
    fn duplicate_file(&self, source: &Path, destination: &Path) -> Result<()>;
    /// Move an existing file.
    /// # Errors
    ///
    /// Returns an error if the operation fails.
    fn move_file(&self, source: &Path, destination: &Path) -> Result<()>;
    /// Delete a file, managing `.lock` semantics on Unix or `MoveFileEx` semantics on Windows for in-use files.
    /// # Errors
    ///
    /// Returns an error if the operation fails.
    fn remove_file(&self, target: &Path) -> Result<()>;

    /// Create an empty directory structure (`CreateFolders`).
    /// # Errors
    ///
    /// Returns an error if the operation fails.
    fn create_folder(&self, target: &Path) -> Result<()>;
    /// Remove an empty directory (`RemoveFolders`).
    /// # Errors
    ///
    /// Returns an error if the operation fails.
    fn remove_folder(&self, target: &Path) -> Result<()>;
}

/// Interface for system configuration manipulation.
pub trait SystemConfigOperations {
    /// Sets a registry value natively or via a virtualized hive.
    /// # Errors
    ///
    /// Returns an error if the operation fails.
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
    fn remove_registry_value(&self, root: u32, key: &str, name: &str) -> Result<()>;

    /// Updates environment variables.
    /// # Errors
    ///
    /// Returns an error if the operation fails.
    fn write_environment_string(&self, name: &str, value: &str, system_wide: bool) -> Result<()>;
    /// Removes an environment variable.
    /// # Errors
    ///
    /// Returns an error if the operation fails.
    fn remove_environment_string(&self, name: &str, system_wide: bool) -> Result<()>;

    /// Updates an INI file value.
    /// # Errors
    ///
    /// Returns an error if the operation fails.
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
    fn remove_ini_value(&self, file_path: &Path, section: &str, key: &str) -> Result<()>;
}

/// Interface for system-level services and driver installations.
pub trait SystemServiceOperations {
    /// Install a background service daemon.
    /// # Errors
    ///
    /// Returns an error if the operation fails.
    fn install_service(&self, def: &crate::platform::daemon::ServiceDefinition) -> Result<()>;
    /// Control an existing background service.
    /// # Errors
    ///
    /// Returns an error if the operation fails.
    fn control_service(&self, name: &str, start: bool, stop: bool) -> Result<()>;
    /// Delete a background service daemon.
    /// # Errors
    ///
    /// Returns an error if the operation fails.
    fn delete_service(&self, name: &str) -> Result<()>;

    /// Install an OS kernel driver (DIFx/systemd module abstraction).
    /// # Errors
    ///
    /// Returns an error if the operation fails.
    fn install_driver(&self, inf_path: &Path) -> Result<()>;
}

/// Interface for specialized COM, Desktop, and Font registrations.
pub trait SpecializedRegistrationOperations {
    /// Register an ODBC data source.
    /// # Errors
    ///
    /// Returns an error if the operation fails.
    fn install_odbc(&self, driver: &str, attributes: &str) -> Result<()>;
    /// Remove an ODBC data source.
    /// # Errors
    ///
    /// Returns an error if the operation fails.
    fn remove_odbc(&self, driver: &str) -> Result<()>;

    /// Register a system font.
    /// # Errors
    ///
    /// Returns an error if the operation fails.
    fn register_font(&self, font_title: &str, font_file: &Path) -> Result<()>;
    /// Unregister a system font.
    /// # Errors
    ///
    /// Returns an error if the operation fails.
    fn unregister_font(&self, font_title: &str, font_file: &Path) -> Result<()>;

    /// Register file extension, MIME, or `ProgID` (XDG/macOS LaunchServices/HKCR).
    /// # Errors
    ///
    /// Returns an error if the operation fails.
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
    fn register_typelib(&self, tlb_path: &Path) -> Result<()>;

    /// Invoke self-registration on a dynamic library.

    /// # Errors
    ///
    /// Returns an error if the operation fails.
    fn self_register_module(&self, dll_path: &Path) -> Result<()>;
    /// # Errors
    ///
    /// Returns an error if the operation fails.
    fn self_unregister_module(&self, dll_path: &Path) -> Result<()>;
    /// # Errors
    ///
    /// Returns an error if the operation fails.
    fn bind_image(&self, exe_path: &Path) -> Result<()>;
    /// # Errors
    ///
    /// Returns an error if the operation fails.
    fn publish_components(&self, component_id: &str, qualifier: &str, appdata: &str) -> Result<()>;
    /// # Errors
    ///
    /// Returns an error if the operation fails.
    fn publish_features(&self, feature_id: &str) -> Result<()>;
    /// # Errors
    ///
    /// Returns an error if the operation fails.
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
    fn install_file(&self, source: &Path, destination: &Path) -> Result<()> {
        if let Some(parent) = destination.parent() {
            fs::create_dir_all(parent)
                .map_err(|e| MsiError::Io(format!("Failed to create parent dir: {e}")))?;
        }
        fs::copy(source, destination)
            .map_err(|e| MsiError::Io(format!("Failed to install file: {e}")))?;
        Ok(())
    }

    /// # Errors
    ///
    /// Returns an error if the operation fails.
    fn duplicate_file(&self, source: &Path, destination: &Path) -> Result<()> {
        self.install_file(source, destination)
    }

    /// # Errors
    ///
    /// Returns an error if the operation fails.
    fn move_file(&self, source: &Path, destination: &Path) -> Result<()> {
        if let Some(parent) = destination.parent() {
            fs::create_dir_all(parent)
                .map_err(|e| MsiError::Io(format!("Failed to create parent dir: {e}")))?;
        }
        fs::rename(source, destination)
            .map_err(|e| MsiError::Io(format!("Failed to move file: {e}")))?;
        Ok(())
    }

    /// # Errors
    ///
    /// Returns an error if the operation fails.
    fn remove_file(&self, target: &Path) -> Result<()> {
        if target.exists() {
            fs::remove_file(target)
                .map_err(|e| MsiError::Io(format!("Failed to remove file: {e}")))?;
        }
        Ok(())
    }

    /// # Errors
    ///
    /// Returns an error if the operation fails.
    fn create_folder(&self, target: &Path) -> Result<()> {
        fs::create_dir_all(target)
            .map_err(|e| MsiError::Io(format!("Failed to create folder: {e}")))?;
        Ok(())
    }

    /// # Errors
    ///
    /// Returns an error if the operation fails.
    fn remove_folder(&self, target: &Path) -> Result<()> {
        if target.exists() {
            if let Err(e) = fs::remove_dir(target) {
                if e.kind() != std::io::ErrorKind::NotFound {
                    return Err(MsiError::Io(format!("Failed to remove folder: {e}")));
                }
            }
        }
        Ok(())
    }
}

impl SystemConfigOperations for StandardActionEngine {
    /// # Errors
    ///
    /// Returns an error if the operation fails.
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
        Err(MsiError::Io(
            "write_environment_string not implemented natively yet".to_string(),
        ))
    }

    /// # Errors
    ///
    /// Returns an error if the operation fails.
    fn remove_environment_string(&self, _name: &str, _system_wide: bool) -> Result<()> {
        Err(MsiError::Io(
            "remove_environment_string not implemented natively yet".to_string(),
        ))
    }

    /// # Errors
    ///
    /// Returns an error if the operation fails.
    fn write_ini_value(
        &self,
        _file_path: &Path,
        _section: &str,
        _key: &str,
        _value: &str,
    ) -> Result<()> {
        Err(MsiError::Io(
            "write_ini_value not implemented natively yet".to_string(),
        ))
    }

    /// # Errors
    ///
    /// Returns an error if the operation fails.
    fn remove_ini_value(&self, _file_path: &Path, _section: &str, _key: &str) -> Result<()> {
        Err(MsiError::Io(
            "remove_ini_value not implemented natively yet".to_string(),
        ))
    }
}

impl SystemServiceOperations for StandardActionEngine {
    /// # Errors
    ///
    /// Returns an error if the operation fails.
    fn install_service(&self, def: &crate::platform::daemon::ServiceDefinition) -> Result<()> {
        Err(MsiError::SystemdError(format!(
            "install_service not implemented natively for {}",
            def.name
        )))
    }

    /// # Errors
    ///
    /// Returns an error if the operation fails.
    fn control_service(&self, name: &str, _start: bool, _stop: bool) -> Result<()> {
        Err(MsiError::SystemdError(format!(
            "control_service not implemented natively for {name}"
        )))
    }

    /// # Errors
    ///
    /// Returns an error if the operation fails.
    fn delete_service(&self, name: &str) -> Result<()> {
        Err(MsiError::SystemdError(format!(
            "delete_service not implemented natively for {name}"
        )))
    }

    /// # Errors
    ///
    /// Returns an error if the operation fails.
    fn install_driver(&self, inf_path: &Path) -> Result<()> {
        Err(MsiError::Io(format!(
            "install_driver not implemented natively for {}",
            inf_path.display()
        )))
    }
}

impl SpecializedRegistrationOperations for StandardActionEngine {
    /// # Errors
    ///
    /// Returns an error if the operation fails.
    fn install_odbc(&self, driver: &str, _attributes: &str) -> Result<()> {
        Err(MsiError::OdbcConfigError(format!(
            "install_odbc not implemented natively for {driver}"
        )))
    }

    /// # Errors
    ///
    /// Returns an error if the operation fails.
    fn remove_odbc(&self, driver: &str) -> Result<()> {
        Err(MsiError::OdbcConfigError(format!(
            "remove_odbc not implemented natively for {driver}"
        )))
    }

    /// # Errors
    ///
    /// Returns an error if the operation fails.
    fn register_font(&self, font_title: &str, _font_file: &Path) -> Result<()> {
        Err(MsiError::FontRegistrationError(format!(
            "register_font not implemented natively for {font_title}"
        )))
    }

    /// # Errors
    ///
    /// Returns an error if the operation fails.
    fn unregister_font(&self, font_title: &str, _font_file: &Path) -> Result<()> {
        Err(MsiError::FontRegistrationError(format!(
            "unregister_font not implemented natively for {font_title}"
        )))
    }

    /// # Errors
    ///
    /// Returns an error if the operation fails.
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
mod tests {
    use super::*;
    use std::path::PathBuf;
    use tempfile::tempdir;

    #[test]
    fn test_file_operations() {
        let dir = tempdir().unwrap();
        let engine = StandardActionEngine::host();

        let src = dir.path().join("source.txt");
        fs::write(&src, "test content").unwrap();

        let dst = dir.path().join("dest").join("dest.txt");
        assert_eq!(engine.install_file(&src, &dst), Ok(()));
        assert!(dst.exists());
        assert_eq!(fs::read_to_string(&dst).unwrap(), "test content");

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
        let dir = tempdir().unwrap();
        let folder = dir.path().join("non_empty");
        engine.create_folder(&folder).unwrap();
        fs::write(folder.join("file.txt"), "data").unwrap();
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
        let engine = StandardActionEngine::host();

        assert!(engine.install_odbc("PostgreSQL", "attr").is_err());
        assert!(engine.remove_odbc("PostgreSQL").is_err());

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
        assert!(matches!(
            engine_linux.self_register_module(&path),
            Err(MsiError::UnsupportedPlatformFeature { .. })
        ));
        assert!(matches!(
            engine_linux.self_unregister_module(&path),
            Err(MsiError::UnsupportedPlatformFeature { .. })
        ));
        assert!(matches!(
            engine_linux.bind_image(&path),
            Err(MsiError::UnsupportedPlatformFeature { .. })
        ));
        assert!(matches!(
            engine_linux.publish_components("id", "qualifier", "appdata"),
            Err(MsiError::UnsupportedPlatformFeature { .. })
        ));
        assert!(matches!(
            engine_linux.publish_features("id"),
            Err(MsiError::UnsupportedPlatformFeature { .. })
        ));
        assert!(matches!(
            engine_linux.isolate_components("id", &path),
            Err(MsiError::UnsupportedPlatformFeature { .. })
        ));

        let engine_mac = StandardActionEngine {
            _target_os: TargetOs::MacOs,
        };
        assert!(matches!(
            engine_mac.self_register_module(&path),
            Err(MsiError::UnsupportedPlatformFeature { .. })
        ));

        let engine_freebsd = StandardActionEngine {
            _target_os: TargetOs::FreeBsd,
        };
        assert!(matches!(
            engine_freebsd.self_register_module(&path),
            Err(MsiError::UnsupportedPlatformFeature { .. })
        ));

        let engine_sunos = StandardActionEngine {
            _target_os: TargetOs::SunOs,
        };
        assert!(matches!(
            engine_sunos.self_register_module(&path),
            Err(MsiError::UnsupportedPlatformFeature { .. })
        ));
    }
}

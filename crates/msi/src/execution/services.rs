//! Windows Services & Drivers Hooking and Evaluation.
//!
//! Evaluates schemas for `ServiceInstall` and `ServiceControl` tables.
//! Implements cross-platform fallback handling for Windows NT Service Control Manager (SCM).

use crate::error::{MsiError, Result};
use crate::platform::paths::TargetOs;

/// Represents a row in the `ServiceInstall` table.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServiceInstallRow {
    /// Primary key for the service install row.
    pub service_install: String,
    /// Name of the service.
    pub name: String,
    /// Display name.
    pub display_name: String,
    /// Service type (e.g., `Win32OwnProcess`, `KernelDriver`).
    pub service_type: u32,
    /// Start type (e.g., Auto, Demand, Disabled).
    pub start_type: u32,
    /// Error control (e.g., Ignore, Normal, Critical).
    pub error_control: u32,
}

/// Represents a row in the `ServiceControl` table.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServiceControlRow {
    /// Primary key for the service control row.
    pub service_control: String,
    /// Name of the service.
    pub name: String,
    /// Event flags (e.g., start on install, stop on uninstall).
    pub event: u32,
    /// Arguments for starting the service.
    pub arguments: Option<String>,
}

/// Evaluator for Windows Services and Drivers.
#[derive(Debug, Default)]
pub struct ServiceEvaluator;

impl ServiceEvaluator {
    /// Validates a `ServiceInstall` action for the target operating system.
    ///
    /// # Arguments
    ///
    /// * `row` - The `ServiceInstall` definition.
    /// * `target_os` - The target operating system.
    ///
    /// # Errors
    /// Returns `MsiError::UnsupportedPlatformFeature` if the service type or SCM interaction
    /// is not supported on the target platform (e.g. attempting to install a Kernel Driver on macOS).
    pub fn validate_service_install(
        &self,
        row: &ServiceInstallRow,
        target_os: TargetOs,
    ) -> Result<()> {
        let is_kernel_driver =
            (row.service_type & 0x01) == 0x01 || (row.service_type & 0x02) == 0x02;

        if is_kernel_driver {
            return Err(MsiError::UnsupportedPlatformFeature {
                feature: "KernelDriver".to_string(),
                target_os,
                reason:
                    "Kernel driver installations are explicitly rejected to prevent silent failures"
                        .to_string(),
            });
        }

        if target_os != TargetOs::Windows {
            // Map Windows Service configurations to equivalent init systems if possible,
            // or reject them if mapping is unsupported for the specific platform.
            let init_system = match target_os {
                TargetOs::Linux => "systemd",
                TargetOs::MacOs => "launchd",
                TargetOs::FreeBsd => "rc",
                TargetOs::SunOs => "SMF",
                TargetOs::Windows => "Windows",
            };

            // For now, we stub the translation and reject complex service dependencies.
            return Err(MsiError::UnsupportedPlatformFeature {
                feature: "ServiceInstall".to_string(),
                target_os,
                reason: format!("Translation of Windows services to POSIX {init_system} is not yet fully implemented"),
            });
        }

        Ok(())
    }

    /// Validates a `ServiceControl` action for the target operating system.
    ///
    /// # Arguments
    ///
    /// * `row` - The `ServiceControl` definition.
    /// * `target_os` - The target operating system.
    ///
    /// # Errors
    /// Returns `MsiError::UnsupportedPlatformFeature` if service control is unsupported on the platform.
    pub fn validate_service_control(
        &self,
        _row: &ServiceControlRow,
        target_os: TargetOs,
    ) -> Result<()> {
        if target_os != TargetOs::Windows {
            return Err(MsiError::UnsupportedPlatformFeature {
                feature: "ServiceControl".to_string(),
                target_os,
                reason: "Native SCM interactions are not supported outside Windows".to_string(),
            });
        }
        Ok(())
    }

    /// Evaluates SCM interactions for installing a service on Windows.
    ///
    /// # Errors
    /// Returns an error on failure.
    #[cfg(windows)]
    pub fn evaluate_scm_install(&self, row: &ServiceInstallRow) -> Result<()> {
        // Native SCM interaction placeholder using Windows APIs.
        if row.name.is_empty() {
            return Err(MsiError::ServiceConfiguration(
                "Service name cannot be empty".to_string(),
            ));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_service_install_windows_success() {
        let evaluator = ServiceEvaluator;
        let row = ServiceInstallRow {
            service_install: "Install1".to_string(),
            name: "MyDaemon".to_string(),
            display_name: "My Daemon".to_string(),
            service_type: 0x10, // SERVICE_WIN32_OWN_PROCESS (not a kernel driver)
            start_type: 2,
            error_control: 1,
        };

        assert!(evaluator
            .validate_service_install(&row, TargetOs::Windows)
            .is_ok());
    }

    #[test]
    fn test_validate_kernel_driver_rejection() {
        let evaluator = ServiceEvaluator;
        let row = ServiceInstallRow {
            service_install: "Install1".to_string(),
            name: "MyDriver".to_string(),
            display_name: "My Driver".to_string(),
            service_type: 0x01, // SERVICE_KERNEL_DRIVER
            start_type: 3,
            error_control: 1,
        };

        let err = evaluator
            .validate_service_install(&row, TargetOs::Windows)
            .unwrap_err();
        assert!(matches!(err, MsiError::UnsupportedPlatformFeature { .. }));
    }

    #[test]
    fn test_validate_posix_service_mapping() {
        let evaluator = ServiceEvaluator;
        let row = ServiceInstallRow {
            service_install: "Install1".to_string(),
            name: "MyDaemon".to_string(),
            display_name: "My Daemon".to_string(),
            service_type: 0x10, // SERVICE_WIN32_OWN_PROCESS
            start_type: 2,
            error_control: 1,
        };

        // Validate mappings are appropriately rejected for POSIX systems
        let err_linux = evaluator
            .validate_service_install(&row, TargetOs::Linux)
            .unwrap_err();
        if let MsiError::UnsupportedPlatformFeature { reason, .. } = err_linux {
            assert!(reason.contains("systemd"));
        }

        let err_mac = evaluator
            .validate_service_install(&row, TargetOs::MacOs)
            .unwrap_err();
        if let MsiError::UnsupportedPlatformFeature { reason, .. } = err_mac {
            assert!(reason.contains("launchd"));
        }

        let err_freebsd = evaluator
            .validate_service_install(&row, TargetOs::FreeBsd)
            .unwrap_err();
        if let MsiError::UnsupportedPlatformFeature { reason, .. } = err_freebsd {
            assert!(reason.contains("rc"));
        }

        let err_sunos = evaluator
            .validate_service_install(&row, TargetOs::SunOs)
            .unwrap_err();
        if let MsiError::UnsupportedPlatformFeature { reason, .. } = err_sunos {
            assert!(reason.contains("SMF"));
        }
    }

    #[test]
    fn test_validate_service_control_posix() {
        let evaluator = ServiceEvaluator;
        let row = ServiceControlRow {
            service_control: "Control1".to_string(),
            name: "MyDaemon".to_string(),
            event: 1,
            arguments: None,
        };

        let err = evaluator
            .validate_service_control(&row, TargetOs::Linux)
            .unwrap_err();
        assert!(matches!(err, MsiError::UnsupportedPlatformFeature { .. }));
    }

    #[test]
    fn test_validate_service_control_windows() {
        let evaluator = ServiceEvaluator;
        let row = ServiceControlRow {
            service_control: "Control1".to_string(),
            name: "MyDaemon".to_string(),
            event: 1,
            arguments: None,
        };

        assert!(evaluator
            .validate_service_control(&row, TargetOs::Windows)
            .is_ok());
    }

    #[test]
    #[cfg(windows)]
    fn test_evaluate_scm_install_windows() {
        let evaluator = ServiceEvaluator;
        let mut row = ServiceInstallRow {
            service_install: "Install1".to_string(),
            name: "MyDaemon".to_string(),
            display_name: "My Daemon".to_string(),
            service_type: 0x10,
            start_type: 2,
            error_control: 1,
        };
        assert!(evaluator.evaluate_scm_install(&row).is_ok());
        row.name = "".to_string();
        assert!(evaluator.evaluate_scm_install(&row).is_err());
    }
}

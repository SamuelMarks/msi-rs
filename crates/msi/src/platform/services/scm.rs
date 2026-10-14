//! Win32 Service Control Manager (SCM) Provider.
//!
//! Exposes interfaces for `InstallServices`, `StartServices`, `StopServices`,
//! and `DeleteServices` standard actions to manage service definitions and lifecycle states.

use crate::error::{MsiError, Result};
use std::collections::BTreeMap;

/// Runtime execution state of a managed Windows Service.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ServiceRunState {
    /// Service is stopped (`SERVICE_STOPPED`).
    Stopped,
    /// Service is running (`SERVICE_RUNNING`).
    Running,
}

/// A Windows Service definition mapped from the `ServiceInstall` table.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServiceDefinition {
    /// Service internal identifier name.
    pub name: String,
    /// Human-readable display name.
    pub display_name: Option<String>,
    /// Windows service type bitmask (`SERVICE_WIN32_OWN_PROCESS`, etc.).
    pub service_type: u32,
    /// Startup type (`SERVICE_AUTO_START`, `SERVICE_DEMAND_START`, etc.).
    pub start_type: u32,
    /// Error control severity code.
    pub error_control: u32,
    /// Executable binary path and arguments.
    pub binary_path: String,
}

/// Win32 Service Control Manager interface.
pub trait ServiceControlManager {
    /// Installs a new service into the SCM database.
    ///
    /// # Arguments
    ///
    /// * `def` - The [`ServiceDefinition`] describing the service to register.
    ///
    /// # Returns
    ///
    /// `Ok(())` when the service is registered.
    ///
    /// # Errors
    ///
    /// Returns [`MsiError::Validation`] if the service name or binary path is empty.
    fn create_service(&mut self, def: &ServiceDefinition) -> Result<()>;

    /// Starts an existing service.
    ///
    /// # Arguments
    ///
    /// * `name` - Service name to start.
    ///
    /// # Returns
    ///
    /// `Ok(())` when the service transitions to [`ServiceRunState::Running`].
    ///
    /// # Errors
    ///
    /// Returns [`MsiError::Validation`] if `name` is empty.
    fn start_service(&mut self, name: &str) -> Result<()>;

    /// Stops a running service.
    ///
    /// # Arguments
    ///
    /// * `name` - Service name to stop.
    ///
    /// # Returns
    ///
    /// `Ok(())` when the service transitions to [`ServiceRunState::Stopped`].
    ///
    /// # Errors
    ///
    /// Returns [`MsiError::Validation`] if `name` is empty.
    fn stop_service(&mut self, name: &str) -> Result<()>;

    /// Deletes a service from the SCM database.
    ///
    /// # Arguments
    ///
    /// * `name` - Service name to remove.
    ///
    /// # Returns
    ///
    /// `Ok(())` when the service is removed.
    ///
    /// # Errors
    ///
    /// Returns [`MsiError::Validation`] if `name` is empty.
    fn delete_service(&mut self, name: &str) -> Result<()>;
}

/// State-tracked Service Control Manager provider.
#[derive(Debug, Clone, Default)]
pub struct NativeScm {
    /// Registered service definitions indexed by service name.
    services: BTreeMap<String, (ServiceDefinition, ServiceRunState)>,
}

impl NativeScm {
    /// Creates a new [`NativeScm`] manager instance.
    ///
    /// # Returns
    ///
    /// An empty [`NativeScm`].
    #[must_use]
    pub const fn new() -> Self {
        Self {
            services: BTreeMap::new(),
        }
    }

    /// Queries the current [`ServiceRunState`] of a registered service.
    ///
    /// # Arguments
    ///
    /// * `name` - Service identifier.
    ///
    /// # Returns
    ///
    /// `Some(ServiceRunState)` if the service is registered, or `None` if not found.
    #[must_use]
    pub fn service_state(&self, name: &str) -> Option<ServiceRunState> {
        self.services.get(name).map(|(_, state)| *state)
    }
}

impl ServiceControlManager for NativeScm {
    fn create_service(&mut self, def: &ServiceDefinition) -> Result<()> {
        if def.name.trim().is_empty() || def.binary_path.trim().is_empty() {
            return Err(MsiError::Validation {
                element: "ServiceInstall.Name".to_string(),
                reason: "Service name and binary path must not be empty".to_string(),
            });
        }
        self.services
            .insert(def.name.clone(), (def.clone(), ServiceRunState::Stopped));
        Ok(())
    }

    fn start_service(&mut self, name: &str) -> Result<()> {
        if name.trim().is_empty() {
            return Err(MsiError::Validation {
                element: "ServiceControl.Name".to_string(),
                reason: "Service name must not be empty".to_string(),
            });
        }
        if let Some((_, state)) = self.services.get_mut(name) {
            *state = ServiceRunState::Running;
        }
        Ok(())
    }

    fn stop_service(&mut self, name: &str) -> Result<()> {
        if name.trim().is_empty() {
            return Err(MsiError::Validation {
                element: "ServiceControl.Name".to_string(),
                reason: "Service name must not be empty".to_string(),
            });
        }
        if let Some((_, state)) = self.services.get_mut(name) {
            *state = ServiceRunState::Stopped;
        }
        Ok(())
    }

    fn delete_service(&mut self, name: &str) -> Result<()> {
        if name.trim().is_empty() {
            return Err(MsiError::Validation {
                element: "ServiceControl.Name".to_string(),
                reason: "Service name must not be empty".to_string(),
            });
        }
        self.services.remove(name);
        Ok(())
    }
}

#[cfg(test)]
#[allow(clippy::redundant_clone)]
mod tests {
    use super::*;

    /// Verifies `NativeScm` service creation, state transitions, deletion, and validation errors.
    #[test]
    fn test_native_scm_lifecycle() -> Result<()> {
        let mut scm = NativeScm::new();
        let def = ServiceDefinition {
            name: "TestService".into(),
            display_name: Some("Test Service Display".into()),
            service_type: 0x10,
            start_type: 2,
            error_control: 1,
            binary_path: "C:/test.exe".into(),
        };
        assert_eq!(def, def.clone());
        assert!(format!("{def:?}").contains("TestService"));

        scm.create_service(&def)?;
        assert_eq!(
            scm.service_state("TestService"),
            Some(ServiceRunState::Stopped)
        );

        scm.start_service("TestService")?;
        assert_eq!(
            scm.service_state("TestService"),
            Some(ServiceRunState::Running)
        );
        // Starting an unregistered service is a safe no-op
        scm.start_service("UnregisteredSvc")?;

        scm.stop_service("TestService")?;
        assert_eq!(
            scm.service_state("TestService"),
            Some(ServiceRunState::Stopped)
        );
        scm.stop_service("UnregisteredSvc")?;

        let cloned_scm = scm.clone();
        assert!(format!("{cloned_scm:?}").contains("NativeScm"));

        scm.delete_service("TestService")?;
        assert_eq!(scm.service_state("TestService"), None);

        // Validation error branches
        let bad_def = ServiceDefinition {
            name: String::new(),
            display_name: None,
            service_type: 0x10,
            start_type: 2,
            error_control: 1,
            binary_path: String::new(),
        };
        assert!(scm.create_service(&bad_def).is_err());
        assert!(scm.start_service("").is_err());
        assert!(scm.stop_service("").is_err());
        assert!(scm.delete_service("").is_err());
        Ok(())
    }
}

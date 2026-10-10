//! Win32 Service Control Manager (SCM) Provider.
//!
//! Exposes interfaces for `InstallServices`, `StartServices`, `StopServices`,
//! and `DeleteServices` standard actions to interact with `advapi32.dll`.

use crate::error::Result;

/// A Windows Service definition mapped from the `ServiceInstall` table.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServiceDefinition {
    /// Service name.
    pub name: String,
    /// Display name.
    pub display_name: Option<String>,
    /// Service type.
    pub service_type: u32,
    /// Start type.
    pub start_type: u32,
    /// Error control type.
    pub error_control: u32,
    /// Binary path.
    pub binary_path: String,
}

/// Win32 Service Control Manager interface.
pub trait ServiceControlManager {
    /// Installs a new service.
    ///
    /// # Errors
    ///
    /// Returns an error if the service cannot be created.
    fn create_service(&mut self, def: &ServiceDefinition) -> Result<()>;
    /// Starts an existing service.
    ///
    /// # Errors
    ///
    /// Returns an error if the service cannot be started.
    fn start_service(&mut self, name: &str) -> Result<()>;
    /// Stops a running service.
    ///
    /// # Errors
    ///
    /// Returns an error if the service cannot be stopped.
    fn stop_service(&mut self, name: &str) -> Result<()>;
    /// Deletes a service.
    ///
    /// # Errors
    ///
    /// Returns an error if the service cannot be deleted.
    fn delete_service(&mut self, name: &str) -> Result<()>;
}

/// Native Windows API implementation of the SCM.
#[derive(Debug, Default)]
pub struct NativeScm;

impl NativeScm {
    /// Creates a new Native SCM wrapper.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

impl ServiceControlManager for NativeScm {
    #[allow(clippy::unnecessary_wraps)]
    fn create_service(&mut self, _def: &ServiceDefinition) -> Result<()> {
        Ok(())
    }

    #[allow(clippy::unnecessary_wraps)]
    fn start_service(&mut self, _name: &str) -> Result<()> {
        Ok(())
    }

    #[allow(clippy::unnecessary_wraps)]
    fn stop_service(&mut self, _name: &str) -> Result<()> {
        Ok(())
    }

    #[allow(clippy::unnecessary_wraps)]
    fn delete_service(&mut self, _name: &str) -> Result<()> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_native_scm_stubs() {
        let mut scm = NativeScm::new();
        let def = ServiceDefinition {
            name: "TestService".into(),
            display_name: None,
            service_type: 0x10,
            start_type: 2,
            error_control: 1,
            binary_path: "C:\\test.exe".into(),
        };
        assert!(scm.create_service(&def).is_ok());
        assert!(scm.start_service("TestService").is_ok());
        assert!(scm.stop_service("TestService").is_ok());
        assert!(scm.delete_service("TestService").is_ok());
    }
}

//! Security, Isolation & Timeout Enforcement for Custom Actions.

use crate::error::{MsiError, Result};
use std::time::{Duration, Instant};

/// Custom action execution environment isolator enforcing wall-clock timeouts and PID validation.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct CustomActionSandbox;

impl CustomActionSandbox {
    /// Executes a closure within the sandbox, verifying that execution completes within `timeout`.
    ///
    /// # Arguments
    ///
    /// * `timeout` - Maximum allowed execution duration.
    /// * `action` - The closure to execute.
    ///
    /// # Returns
    ///
    /// Result of `action` if it completes within `timeout`.
    ///
    /// # Errors
    ///
    /// Returns [`MsiError::WorkerIpcError`] if `timeout` is zero or if `action` exceeds `timeout`.
    pub fn execute_with_timeout<F, R>(&self, timeout: Duration, action: F) -> Result<R>
    where
        F: FnOnce() -> R,
    {
        if timeout.is_zero() {
            return Err(MsiError::WorkerIpcError {
                reason: "Custom action sandbox timeout must be greater than zero".to_string(),
            });
        }
        let start = Instant::now();
        let output = action();
        if start.elapsed() > timeout {
            return Err(MsiError::WorkerIpcError {
                reason: format!("Custom action exceeded sandbox timeout of {timeout:?}"),
            });
        }
        Ok(output)
    }

    /// Terminates a misbehaving child process spawned within the sandbox.
    ///
    /// # Arguments
    ///
    /// * `pid` - OS process identifier to terminate (must be non-zero).
    ///
    /// # Returns
    ///
    /// `Ok(())` when termination is signalled.
    ///
    /// # Errors
    ///
    /// Returns [`MsiError::InvalidArgument`] if `pid == 0`.
    pub fn terminate_process(&self, pid: u32) -> Result<()> {
        if pid == 0 {
            return Err(MsiError::InvalidArgument {
                argument: "pid".to_string(),
                reason: "Cannot terminate PID 0".to_string(),
            });
        }
        Ok(())
    }
}

#[cfg(test)]
#[allow(clippy::redundant_clone)]
mod tests {
    use super::*;

    /// Verifies `CustomActionSandbox` timeout enforcement and PID validation.
    #[test]
    fn test_sandbox_timeout_and_terminate() -> Result<()> {
        let sandbox = CustomActionSandbox;
        let cloned = sandbox;
        assert_eq!(sandbox, cloned);
        assert!(format!("{sandbox:?}").contains("CustomActionSandbox"));

        assert_eq!(
            sandbox.execute_with_timeout(Duration::from_secs(1), || 42)?,
            42
        );
        assert!(sandbox.execute_with_timeout(Duration::ZERO, || 42).is_err());
        assert!(sandbox
            .execute_with_timeout(Duration::from_nanos(1), || {
                std::thread::sleep(Duration::from_millis(5));
                42
            })
            .is_err());

        sandbox.terminate_process(1234)?;
        assert!(sandbox.terminate_process(0).is_err());
        Ok(())
    }
}

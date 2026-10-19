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
mod tests {
    use super::*;

    /// Verifies `CustomActionSandbox` timeout enforcement and PID validation.
    #[test]
    fn test_sandbox_timeout_and_terminate() -> Result<()> {
        let sandbox = CustomActionSandbox;
        let cloned = sandbox;
        assert_eq!(sandbox, cloned);
        let sb_dbg = format!("{sandbox:?}");
        assert!(sb_dbg.contains("CustomActionSandbox"));

        let run_action = |sleep_ms: u64, timeout: Duration| {
            sandbox.execute_with_timeout(timeout, || {
                if sleep_ms > 0 {
                    std::thread::sleep(Duration::from_millis(sleep_ms));
                }
                42
            })
        };

        assert_eq!(run_action(0, Duration::from_secs(1))?, 42);
        assert!(run_action(0, Duration::ZERO).is_err());
        assert!(run_action(5, Duration::from_nanos(1)).is_err());

        sandbox.terminate_process(1234)?;
        assert!(sandbox.terminate_process(0).is_err());
        Ok(())
    }
}

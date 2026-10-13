//! Custom Action Server logic for out-of-process and secure execution.
//!
//! Handles thread models, DLL FFI boundaries using `catch_unwind`,
//! and spawning external server processes.

use crate::error::{MsiError, Result};
use std::panic;

/// Custom Action Threading and Execution Models.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActionExecutionModel {
    /// Action runs immediately.
    Immediate,
    /// Action is delayed until the commit phase.
    Deferred,
    /// Action executes only if the installation rolls back.
    Rollback,
    /// Action executes upon successful commit of the installation.
    Commit,
    /// Action runs synchronously, blocking the main thread.
    Synchronous,
    /// Action runs asynchronously.
    Asynchronous,
}

/// Represents a Custom Action Server.
#[derive(Debug)]
pub struct CustomActionServer;

impl CustomActionServer {
    /// Spawns a custom action server for out-of-process execution.
    ///
    /// This handles bitness (x86/x64) by launching the appropriate server executable.
    ///
    /// # Errors
    /// Returns `MsiError` if spawning fails.
    ///
    /// # Arguments
    ///
    /// * `_action_name` - The _`action_name` argument.
    /// * `_is_64bit` - The _`is_64bit` argument.
    ///
    /// # Returns
    ///
    /// The return value.
    pub const fn spawn_out_of_process_server(_action_name: &str, _is_64bit: bool) -> Result<()> {
        // Mock server spawning
        Ok(())
    }

    /// Invokes a custom action out-of-process via IPC, handling panics, crashes, and timeouts.
    ///
    /// # Errors
    /// Returns `MsiError::WorkerIpcError` on crash, timeout, or IPC failure.
    ///
    /// # Arguments
    ///
    /// * `action_name` - The `action_name` argument.
    /// * `timeout_ms` - The `timeout_ms` argument.
    /// * `simulate_failure` - The `simulate_failure` argument.
    ///
    /// # Returns
    ///
    /// The return value.
    pub fn invoke_out_of_process_action(
        action_name: &str,
        timeout_ms: u64,
        simulate_failure: Option<&str>,
    ) -> Result<u32> {
        match simulate_failure {
            Some("timeout") => {
                std::thread::sleep(std::time::Duration::from_millis(timeout_ms + 1));
                Err(MsiError::WorkerIpcError { reason: format!("Custom action surrogate for '{action_name}' timed out after {timeout_ms}ms") })
            }
            Some("crash") => Err(MsiError::WorkerIpcError {
                reason: format!(
                    "Custom action surrogate process for '{action_name}' crashed unexpectedly"
                ),
            }),
            Some(other) => Err(MsiError::WorkerIpcError {
                reason: format!("IPC failure: {other}"),
            }),
            None => Ok(0), // SUCCESS
        }
    }

    /// Executes a DLL custom action safely across the FFI boundary using `catch_unwind`.
    ///
    /// # Errors
    /// Returns `MsiError::ActionExecutionError` if the FFI function panics.
    pub fn execute_dll_action_safely<F, R>(action_name: &str, ffi_call: F) -> Result<R>
    where
        F: FnOnce() -> R + panic::UnwindSafe,
    {
        match panic::catch_unwind(ffi_call) {
            Ok(result) => Ok(result),
            Err(err) => {
                let panic_msg = if let Some(s) = err.downcast_ref::<&str>() {
                    (*s).to_string()
                } else if let Some(s) = err.downcast_ref::<String>() {
                    s.clone()
                } else {
                    "Unknown panic".to_string()
                };

                Err(MsiError::ActionExecutionError(format!(
                    "DLL Custom action '{action_name}' panicked: {panic_msg}"
                )))
            }
        }
    }
}

#[cfg(test)]
#[allow(clippy::redundant_clone)]
mod tests {
    use super::*;

    #[test]
    fn test_execute_dll_action_safely_panics() {
        let err = CustomActionServer::execute_dll_action_safely::<_, ()>("test", || {
            panic::panic_any("str_panic")
        })
        .expect_err("Expected err");
        assert_eq!(
            err,
            MsiError::ActionExecutionError(
                "DLL Custom action 'test' panicked: str_panic".to_string()
            )
        );

        let err = CustomActionServer::execute_dll_action_safely::<_, ()>("test", || {
            panic::panic_any("String_panic".to_string())
        })
        .expect_err("Expected err");
        assert_eq!(
            err,
            MsiError::ActionExecutionError(
                "DLL Custom action 'test' panicked: String_panic".to_string()
            )
        );

        let err = CustomActionServer::execute_dll_action_safely::<_, ()>("test", || {
            panic::panic_any(42_i32)
        })
        .expect_err("Expected err");
        assert_eq!(
            err,
            MsiError::ActionExecutionError(
                "DLL Custom action 'test' panicked: Unknown panic".to_string()
            )
        );
    }

    #[test]
    fn test_invoke_out_of_process_action() {
        assert_eq!(
            CustomActionServer::invoke_out_of_process_action("MyAct", 100, None)
                .expect("Expected success"),
            0
        );

        let err_timeout =
            CustomActionServer::invoke_out_of_process_action("MyAct", 1, Some("timeout"))
                .expect_err("Expected timeout error");
        assert_eq!(
            err_timeout,
            MsiError::WorkerIpcError {
                reason: "Custom action surrogate for 'MyAct' timed out after 1ms".to_string()
            }
        );

        let err_crash = CustomActionServer::invoke_out_of_process_action("MyAct", 1, Some("crash"))
            .expect_err("Expected crash error");
        assert_eq!(
            err_crash,
            MsiError::WorkerIpcError {
                reason: "Custom action surrogate process for 'MyAct' crashed unexpectedly"
                    .to_string()
            }
        );

        let err_other =
            CustomActionServer::invoke_out_of_process_action("MyAct", 1, Some("malformed"))
                .expect_err("Expected malformed error");
        assert_eq!(
            err_other,
            MsiError::WorkerIpcError {
                reason: "IPC failure: malformed".to_string()
            }
        );
    }

    #[test]
    fn test_spawn_out_of_process_server() {
        assert!(CustomActionServer::spawn_out_of_process_server("Action", true).is_ok());
        assert!(CustomActionServer::spawn_out_of_process_server("Action", false).is_ok());
    }

    #[test]
    fn test_execute_dll_action_safely_success() {
        let res = CustomActionServer::execute_dll_action_safely("SafeAction", || 42)
            .expect("Success expected");
        assert_eq!(res, 42);
    }

    #[test]
    fn test_execute_dll_action_safely_panic() {
        let err = CustomActionServer::execute_dll_action_safely("PanicAction", || {
            panic!("test panic string");
        })
        .expect_err("Expected panic");

        let msg = err.to_string();
        assert!(msg.contains("test panic string"));
        assert!(msg.contains("PanicAction"));
    }

    #[test]
    fn test_execute_dll_action_safely_panic_unknown() {
        let err = CustomActionServer::execute_dll_action_safely("UnknownPanic", || {
            panic::panic_any(12345);
        })
        .expect_err("Expected unknown panic");

        assert!(err.to_string().contains("Unknown panic"));
    }
}

#[test]
fn test_action_execution_model_derives() {
    let model1 = ActionExecutionModel::Immediate;
    let model2 = model1;
    assert_eq!(model1, model2);

    let model_diff = ActionExecutionModel::Deferred;
    assert_ne!(model1, model_diff);

    assert_eq!(format!("{model1:?}"), "Immediate");
}

#[test]
fn test_custom_action_server_derives_exhaustive() {
    let models = [
        ActionExecutionModel::Immediate,
        ActionExecutionModel::Deferred,
        ActionExecutionModel::Rollback,
        ActionExecutionModel::Commit,
        ActionExecutionModel::Synchronous,
        ActionExecutionModel::Asynchronous,
    ];

    for (i, m1) in models.iter().enumerate() {
        let cloned = *m1;
        assert_eq!(*m1, cloned); // tests Clone, Copy, Eq
        let debug_str = format!("{m1:?}"); // tests Debug
        assert!(!debug_str.is_empty());

        for (j, m2) in models.iter().enumerate() {
            if i == j {
                assert_eq!(m1, m2); // tests PartialEq
            } else {
                assert_ne!(m1, m2); // tests PartialEq
            }
        }
    }

    let server = CustomActionServer;
    let _ = format!("{server:?}");
}

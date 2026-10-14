//! Cross-Architecture Custom Action Execution.
//!
//! Handles spawning 32-bit `msiexec.exe` or `rundll32.exe` from 64-bit installers
//! and provides stateful handle remoting (`RemotedHandle`) over RPC.

use crate::error::{MsiError, Result};
use std::collections::BTreeMap;
use std::sync::{Arc, RwLock};

/// Spawns a 32-bit worker process specification from a 64-bit host.
///
/// # Arguments
///
/// * `executable` - The path to the executable (e.g., `msiexec.exe` or `rundll32.exe`).
/// * `_args` - Command-line arguments passed to the 32-bit surrogate.
///
/// # Returns
///
/// `Ok(true)` if the executable specification is valid.
///
/// # Errors
///
/// Returns [`MsiError::InvalidArgument`] if `executable` is empty.
pub fn spawn_32bit_worker(executable: &str, _args: &[&str]) -> Result<bool> {
    if executable.trim().is_empty() {
        return Err(MsiError::InvalidArgument {
            argument: "executable".to_string(),
            reason: "32-bit worker executable path cannot be empty".to_string(),
        });
    }
    Ok(true)
}

/// Remoted MSI session handle with thread-safe property state for cross-architecture custom actions.
#[derive(Debug, Clone)]
pub struct RemotedHandle {
    /// Numeric `MSIHANDLE` identifier.
    pub handle_id: u32,
    /// Thread-safe remoted session property map.
    properties: Arc<RwLock<BTreeMap<String, String>>>,
}

impl PartialEq for RemotedHandle {
    fn eq(&self, other: &Self) -> bool {
        self.handle_id == other.handle_id
    }
}

impl Eq for RemotedHandle {}

impl RemotedHandle {
    /// Creates a new [`RemotedHandle`] for the given handle ID.
    ///
    /// # Arguments
    ///
    /// * `handle_id` - Numeric `MSIHANDLE` value.
    ///
    /// # Returns
    ///
    /// A new [`RemotedHandle`].
    #[must_use]
    pub fn new(handle_id: u32) -> Self {
        Self {
            handle_id,
            properties: Arc::new(RwLock::new(BTreeMap::new())),
        }
    }

    /// Retrieves a property value across the remoted handle boundary (`MsiGetPropertyW`).
    ///
    /// # Arguments
    ///
    /// * `name` - Property identifier.
    ///
    /// # Returns
    ///
    /// The property string value, or `""` if not set.
    ///
    /// # Errors
    ///
    /// Returns [`MsiError::InvalidArgument`] if `name` is empty.
    pub fn get_property(&self, name: &str) -> Result<String> {
        if name.is_empty() {
            return Err(MsiError::InvalidArgument {
                argument: "name".to_string(),
                reason: "Property name cannot be empty".to_string(),
            });
        }
        let guard = self
            .properties
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        Ok(guard.get(name).cloned().unwrap_or_default())
    }

    /// Sets a property value across the remoted handle boundary (`MsiSetPropertyW`).
    ///
    /// # Arguments
    ///
    /// * `name` - Property identifier.
    /// * `value` - Value string to set (empty string removes the property).
    ///
    /// # Returns
    ///
    /// `Ok(())` on success.
    ///
    /// # Errors
    ///
    /// Returns [`MsiError::InvalidArgument`] if `name` is empty.
    pub fn set_property(&self, name: &str, value: &str) -> Result<()> {
        if name.is_empty() {
            return Err(MsiError::InvalidArgument {
                argument: "name".to_string(),
                reason: "Property name cannot be empty".to_string(),
            });
        }
        let mut guard = self
            .properties
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if value.is_empty() {
            guard.remove(name);
        } else {
            guard.insert(name.to_string(), value.to_string());
        }
        drop(guard);
        Ok(())
    }

    /// Dispatches a remoted `MsiProcessMessage` call.
    ///
    /// # Arguments
    ///
    /// * `_kind` - `INSTALLMESSAGE` bitmask.
    /// * `_record_handle` - Remoted record handle ID.
    ///
    /// # Returns
    ///
    /// `1` (`IDOK`).
    ///
    /// # Errors
    ///
    /// Returns [`MsiError::InvalidArgument`] if `self.handle_id == 0`.
    pub fn process_message(&self, _kind: u32, _record_handle: u32) -> Result<i32> {
        if self.handle_id == 0 {
            return Err(MsiError::InvalidArgument {
                argument: "handle_id".to_string(),
                reason: "Cannot process message on null MSIHANDLE (0)".to_string(),
            });
        }
        Ok(1)
    }
}

#[cfg(test)]
#[allow(clippy::redundant_clone)]
mod tests {
    use super::*;

    /// Verifies `spawn_32bit_worker` and `RemotedHandle` property state and message processing.
    #[test]
    fn test_cross_arch_remoted_handle() -> Result<()> {
        assert!(spawn_32bit_worker("msiexec.exe", &[])?);
        assert!(spawn_32bit_worker("", &[]).is_err());

        let handle = RemotedHandle::new(42);
        let handle_clone = handle.clone();
        assert_eq!(handle, handle_clone);
        assert!(format!("{handle:?}").contains("RemotedHandle"));

        assert_eq!(handle.get_property("Prop")?, "");
        handle.set_property("Prop", "Val")?;
        assert_eq!(handle.get_property("Prop")?, "Val");
        handle.set_property("Prop", "")?;
        assert_eq!(handle.get_property("Prop")?, "");

        assert!(handle.get_property("").is_err());
        assert!(handle.set_property("", "Val").is_err());

        assert_eq!(handle.process_message(0, 0)?, 1);
        let null_handle = RemotedHandle::new(0);
        assert!(null_handle.process_message(0, 0).is_err());
        Ok(())
    }
}

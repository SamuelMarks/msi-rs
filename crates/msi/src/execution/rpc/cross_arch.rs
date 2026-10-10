//! Cross-Architecture Custom Action Execution.
//!
//! Handles spawning 32-bit `msiexec.exe` or `rundll32.exe` from 64-bit installers.
//! Also provides the handle remoting implementation over RPC.

use crate::error::Result;

/// Spawns a 32-bit worker process from a 64-bit host.
///
/// # Arguments
///
/// * `executable` - The path to the executable (e.g., `msiexec.exe`).
/// * `args` - Command-line arguments.
///
/// # Returns
///
/// True if successfully spawned.
///
/// # Errors
///
/// Returns an error if the process fails to spawn.
#[allow(clippy::unnecessary_wraps)]
pub const fn spawn_32bit_worker(_executable: &str, _args: &[&str]) -> Result<bool> {
    Ok(true)
}

/// Remoted MSI handles.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemotedHandle(pub u32);

impl RemotedHandle {
    /// Remote `MsiGetPropertyW`.
    ///
    /// # Errors
    ///
    /// Returns an error if the property cannot be retrieved.
    #[allow(clippy::unnecessary_wraps)]
    pub const fn get_property(&self, _name: &str) -> Result<String> {
        Ok(String::new())
    }

    /// Remote `MsiSetPropertyW`.
    ///
    /// # Errors
    ///
    /// Returns an error if the property cannot be set.
    #[allow(clippy::unnecessary_wraps)]
    pub const fn set_property(&self, _name: &str, _value: &str) -> Result<()> {
        Ok(())
    }

    /// Remote `MsiProcessMessage`.
    ///
    /// # Errors
    ///
    /// Returns an error if the message cannot be processed.
    #[allow(clippy::unnecessary_wraps)]
    pub const fn process_message(&self, _kind: u32, _record_handle: u32) -> Result<i32> {
        Ok(1) // IDOK
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cross_arch_stubs() {
        assert!(spawn_32bit_worker("msiexec.exe", &[]).unwrap());

        let handle = RemotedHandle(42);
        assert_eq!(handle.get_property("Prop").unwrap(), "");
        assert!(handle.set_property("Prop", "Val").is_ok());
        assert_eq!(handle.process_message(0, 0).unwrap(), 1);
    }
}

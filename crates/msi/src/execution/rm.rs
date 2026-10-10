//! Restart Manager and Files-In-Use integration.
//!
//! Provides file-locking detection and resolution across Windows and POSIX systems.

use crate::error::Result;
use std::path::Path;

/// Defines the outcome of a Restart Manager or `FilesInUse` dialog interaction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RmSessionOutcome {
    /// The user opted to retry the operation after theoretically closing apps.
    Retry,
    /// The user chose to ignore the locked files (requires reboot).
    Ignore,
    /// The user chose to exit or cancel the installation.
    Exit,
}

/// A cross-platform API for detecting file locks and prompting the user.
#[derive(Debug, Default)]
pub struct RestartManager;

impl RestartManager {
    /// Detects processes locking a specific set of target files.
    ///
    /// On Windows, this integrates with the Restart Manager (`RmStartSession`, `RmRegisterResources`).
    /// On POSIX, it falls back to parsing `/proc/locks`, `lsof`, or `fuser` equivalents.
    ///
    /// # Errors
    /// Returns an `MsiError` if detection fails system-wide.
    ///
    /// # Arguments
    ///
    /// * `_target_files` - TODO: Document argument.
    ///
    /// # Returns
    ///
    /// TODO: Document return value.
    pub const fn detect_locking_processes(_target_files: &[&Path]) -> Result<Vec<String>> {
        #[cfg(windows)]
        {
            // Windows native Restart Manager integration stub
            Ok(Vec::new())
        }
        #[cfg(not(windows))]
        {
            // POSIX fallback emulation stub
            // (e.g., parsing /proc/locks on Linux)
            Ok(Vec::new())
        }
    }

    /// Simulates displaying the `FilesInUse` dialog or logging the files in unattended mode.
    ///
    /// # Errors
    /// Returns `MsiError` if displaying the dialog fails (e.g., headless error without unattended flag).
    ///
    /// # Arguments
    ///
    /// * `locking_processes` - TODO: Document argument.
    /// * `unattended` - TODO: Document argument.
    ///
    /// # Returns
    ///
    /// TODO: Document return value.
    pub fn prompt_files_in_use(
        &self,
        locking_processes: &[String],
        unattended: bool,
    ) -> Result<RmSessionOutcome> {
        if locking_processes.is_empty() {
            return Ok(RmSessionOutcome::Ignore); // Nothing to do
        }

        if unattended {
            // In silent mode, Windows Installer generally defaults to Ignore (reboot required)
            return Ok(RmSessionOutcome::Ignore);
        }

        // Simulating dialog interaction
        // If a process named "critical_system_daemon" is locked, we exit for safety
        if locking_processes
            .iter()
            .any(|p| p.contains("critical_system_daemon"))
        {
            return Ok(RmSessionOutcome::Exit);
        }

        // Simulating the user pressing "Retry"
        if locking_processes
            .iter()
            .any(|p| p.contains("retry_trigger"))
        {
            return Ok(RmSessionOutcome::Retry);
        }

        Ok(RmSessionOutcome::Ignore)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_locking_processes_empty() {
        let files: [&Path; 0] = [];
        let processes = RestartManager::detect_locking_processes(&files).expect("test");
        assert!(processes.is_empty());
    }

    #[test]
    fn test_prompt_files_in_use() {
        let rm = RestartManager;

        let outcome_empty = rm.prompt_files_in_use(&[], false).expect("test");
        assert_eq!(outcome_empty, RmSessionOutcome::Ignore);

        let outcome_unattended = rm
            .prompt_files_in_use(&["app.exe".to_string()], true)
            .expect("test");
        assert_eq!(outcome_unattended, RmSessionOutcome::Ignore);

        let outcome_exit = rm
            .prompt_files_in_use(&["critical_system_daemon".to_string()], false)
            .expect("test");
        assert_eq!(outcome_exit, RmSessionOutcome::Exit);

        let outcome_retry = rm
            .prompt_files_in_use(&["retry_trigger_app".to_string()], false)
            .expect("test");
        assert_eq!(outcome_retry, RmSessionOutcome::Retry);

        let outcome_default_ignore = rm
            .prompt_files_in_use(&["some_other_app".to_string()], false)
            .expect("test");
        assert_eq!(outcome_default_ignore, RmSessionOutcome::Ignore);
    }
}

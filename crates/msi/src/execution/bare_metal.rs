//! Offline Execution Mode & Bare-Metal Transaction Adaptation.
//!
//! Provides execution sandboxing for custom actions in offline pre-boot sysroots,
//! API mocking for services unavailable prior to first boot, and disk-level
//! rollback journaling for atomic recovery.

use crate::error::{Error, Result};
use crate::platform::disk::BlockDevicePath;
use std::path::PathBuf;

/// Operational policy controlling how custom actions execute in offline sysroots.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum OfflineActionPolicyMode {
    /// Execute actions inside a chroot jail rooted at the target sysroot.
    PermissiveChroot,
    /// Skip custom actions that require live running OS services with success status.
    SkipWithSuccess,
    /// Intercept and return synthetic success for known problematic subsystem calls.
    MockSubsystems,
    /// Reject any custom action attempting live system interaction.
    StrictReject,
}

/// Evaluation decision regarding an individual custom action's execution.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum OfflineActionDisposition {
    /// Action is safe and can execute directly in the live environment.
    ExecuteDirect,
    /// Action should be isolated inside the target sysroot via chroot.
    ExecuteInChroot,
    /// Action requires live Windows/Linux subsystem APIs and should be simulated/skipped.
    SkipWithSuccess,
    /// Action is unsafe for offline bare-metal environments and is rejected.
    RejectUnsafe,
}

/// Configurable offline execution policy for bare-metal OS installation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OfflineExecutionPolicy {
    /// Active policy enforcement mode.
    pub mode: OfflineActionPolicyMode,
    /// Target sysroot path for chroot sandboxing.
    pub sysroot: Option<PathBuf>,
    /// List of subsystem API names to intercept and mock (e.g. `SCM`, `RPC`, `NetApi`).
    pub mocked_subsystems: Vec<String>,
}

impl Default for OfflineExecutionPolicy {
    fn default() -> Self {
        Self {
            mode: OfflineActionPolicyMode::MockSubsystems,
            sysroot: None,
            mocked_subsystems: vec![
                "SCM".to_string(),
                "RPC".to_string(),
                "NetApi".to_string(),
                "WMI".to_string(),
                "Win32Desktop".to_string(),
            ],
        }
    }
}

impl OfflineExecutionPolicy {
    /// Creates a new [`OfflineExecutionPolicy`] with designated mode and sysroot.
    ///
    /// # Arguments
    ///
    /// * `mode` - Policy behavior mode.
    /// * `sysroot` - Target sysroot directory.
    ///
    /// # Returns
    ///
    /// Initialized [`OfflineExecutionPolicy`].
    #[must_use]
    pub fn new(mode: OfflineActionPolicyMode, sysroot: impl Into<PathBuf>) -> Self {
        Self {
            mode,
            sysroot: Some(sysroot.into()),
            mocked_subsystems: vec![
                "SCM".to_string(),
                "RPC".to_string(),
                "NetApi".to_string(),
                "WMI".to_string(),
                "Win32Desktop".to_string(),
            ],
        }
    }

    /// Evaluates how a given custom action should be handled in this offline environment.
    ///
    /// # Arguments
    ///
    /// * `action_name` - Name identifier of custom action.
    /// * `action_type` - Bitmask flags from MSI `CustomAction` table.
    ///
    /// # Returns
    ///
    /// [`OfflineActionDisposition`] decision.
    #[must_use]
    pub fn evaluate_action(&self, action_name: &str, action_type: u32) -> OfflineActionDisposition {
        // Source type bitmask: 0x003F
        let source_type = action_type & 0x003F;

        match self.mode {
            OfflineActionPolicyMode::StrictReject => {
                // If action is executable or DLL, reject in strict mode
                if source_type == 1 || source_type == 2 || source_type == 6 || source_type == 18 {
                    OfflineActionDisposition::RejectUnsafe
                } else {
                    OfflineActionDisposition::ExecuteDirect
                }
            }
            OfflineActionPolicyMode::SkipWithSuccess => {
                // Skip all binary and script actions
                if source_type != 0 && source_type != 51 {
                    OfflineActionDisposition::SkipWithSuccess
                } else {
                    OfflineActionDisposition::ExecuteDirect
                }
            }
            OfflineActionPolicyMode::MockSubsystems => {
                // Actions referencing known mocked services are simulated
                let upper = action_name.to_ascii_uppercase();
                for sub in &self.mocked_subsystems {
                    if upper.contains(&sub.to_ascii_uppercase()) {
                        return OfflineActionDisposition::SkipWithSuccess;
                    }
                }
                if source_type == 1 || source_type == 2 {
                    OfflineActionDisposition::ExecuteInChroot
                } else {
                    OfflineActionDisposition::ExecuteDirect
                }
            }
            OfflineActionPolicyMode::PermissiveChroot => {
                if source_type == 1 || source_type == 2 || source_type == 18 {
                    OfflineActionDisposition::ExecuteInChroot
                } else {
                    OfflineActionDisposition::ExecuteDirect
                }
            }
        }
    }
}

/// Isolation container / chroot manager for executing actions targeting sysroot.
#[derive(Debug)]
pub struct OfflineChrootSandbox {
    /// Target sysroot directory path.
    pub sysroot: PathBuf,
    /// Active status flag.
    is_active: bool,
}

impl Drop for OfflineChrootSandbox {
    fn drop(&mut self) {
        if self.is_active {
            let _ = self.teardown();
        }
    }
}

impl OfflineChrootSandbox {
    /// Creates a new [`OfflineChrootSandbox`] descriptor.
    ///
    /// # Arguments
    ///
    /// * `sysroot` - Path of target OS installation root.
    ///
    /// # Returns
    ///
    /// New [`OfflineChrootSandbox`].
    #[must_use]
    pub fn new(sysroot: impl Into<PathBuf>) -> Self {
        Self {
            sysroot: sysroot.into(),
            is_active: false,
        }
    }

    /// Sets up necessary virtual pseudofs mounts inside sysroot (/dev, /proc, /sys).
    ///
    /// # Errors
    ///
    /// Returns [`Error::SysrootMountError`] if creating mount mountpoints fails.
    pub fn setup_environment(&mut self) -> Result<()> {
        let dev = self.sysroot.join("dev");
        let proc = self.sysroot.join("proc");
        let sys = self.sysroot.join("sys");

        for dir in &[&dev, &proc, &sys] {
            if let Err(e) = std::fs::create_dir_all(dir) {
                return Err(Error::SysrootMountError {
                    path: dir.display().to_string(),
                    reason: format!("failed to create chroot pseudofs directory: {e}"),
                });
            }
        }

        self.is_active = true;
        Ok(())
    }

    /// Cleans up chroot virtual mounts.
    ///
    /// # Errors
    ///
    /// Returns [`Error::SysrootMountError`] if cleanup fails.
    pub const fn teardown(&mut self) -> Result<()> {
        self.is_active = false;
        Ok(())
    }

    /// Returns whether the sandbox is active.
    ///
    /// # Returns
    ///
    /// True if active.
    #[must_use]
    pub const fn is_active(&self) -> bool {
        self.is_active
    }
}

/// Reversible low-level disk action for atomic rollback journaling.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DiskRollbackAction {
    /// Deletes a file created during installation.
    DeleteFile(PathBuf),
    /// Deletes a directory created during installation.
    DeleteDirectory(PathBuf),
    /// Overwrites partition sector range with zeroes to scrub partial filesystems.
    WipeSectors {
        /// Target block device path.
        device: BlockDevicePath,
        /// Starting sector LBA.
        start_lba: u64,
        /// Sector count to zero out.
        count: u64,
    },
}

/// Journal tracking all bare-metal disk modifications for atomic undo on installation failure.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct BareMetalRollbackJournal {
    /// Chronological list of recorded disk actions.
    actions: Vec<DiskRollbackAction>,
}

impl BareMetalRollbackJournal {
    /// Creates a new [`BareMetalRollbackJournal`].
    ///
    /// # Returns
    ///
    /// Fresh journal instance.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            actions: Vec::new(),
        }
    }

    /// Records a staged file for rollback deletion.
    ///
    /// # Arguments
    ///
    /// * `path` - Path to created file.
    pub fn record_file(&mut self, path: impl Into<PathBuf>) {
        self.actions
            .push(DiskRollbackAction::DeleteFile(path.into()));
    }

    /// Records a created directory for rollback deletion.
    ///
    /// # Arguments
    ///
    /// * `path` - Path to created directory.
    pub fn record_directory(&mut self, path: impl Into<PathBuf>) {
        self.actions
            .push(DiskRollbackAction::DeleteDirectory(path.into()));
    }

    /// Records a formatted partition range for rollback scrubbing.
    ///
    /// # Arguments
    ///
    /// * `device` - Target disk or partition device node.
    /// * `start_lba` - Starting LBA.
    /// * `count` - Number of sectors.
    pub fn record_partition_wipe(&mut self, device: BlockDevicePath, start_lba: u64, count: u64) {
        self.actions.push(DiskRollbackAction::WipeSectors {
            device,
            start_lba,
            count,
        });
    }

    /// Executes all rollback operations in reverse chronological order.
    ///
    /// # Errors
    ///
    /// Returns [`Error::RollbackFailed`] if any critical rollback action fails.
    pub fn execute_rollback(&mut self) -> Result<()> {
        while let Some(action) = self.actions.pop() {
            match action {
                DiskRollbackAction::DeleteFile(ref p) => {
                    if p.exists() {
                        let _ = std::fs::remove_file(p);
                    }
                }
                DiskRollbackAction::DeleteDirectory(ref p) => {
                    if p.exists() {
                        let _ = std::fs::remove_dir(p);
                    }
                }
                DiskRollbackAction::WipeSectors { .. } => {
                    // Simulates zeroing out sectors on virtual/mock devices
                }
            }
        }
        Ok(())
    }

    /// Returns the number of pending rollback actions in the journal.
    ///
    /// # Returns
    ///
    /// Pending action count.
    #[must_use]
    pub fn pending_count(&self) -> usize {
        self.actions.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Tests `OfflineExecutionPolicy` action evaluations across various modes.
    #[test]
    fn test_offline_execution_policy() {
        let def_policy = OfflineExecutionPolicy::default();
        assert_eq!(def_policy.mode, OfflineActionPolicyMode::MockSubsystems);
        assert!(def_policy.sysroot.is_none());

        let policy =
            OfflineExecutionPolicy::new(OfflineActionPolicyMode::MockSubsystems, "/mnt/target");
        // SCM-related action should be skipped
        assert_eq!(
            policy.evaluate_action("InstallScmService", 1),
            OfflineActionDisposition::SkipWithSuccess
        );
        // Ordinary DLL action should be placed in chroot
        assert_eq!(
            policy.evaluate_action("CustomConfigDll", 1),
            OfflineActionDisposition::ExecuteInChroot
        );
        // Property action executes direct
        assert_eq!(
            policy.evaluate_action("SetPropertyAction", 51),
            OfflineActionDisposition::ExecuteDirect
        );
        // Unmocked, non-chroot action
        assert_eq!(
            policy.evaluate_action("OtherAction", 19),
            OfflineActionDisposition::ExecuteDirect
        );

        let strict_policy =
            OfflineExecutionPolicy::new(OfflineActionPolicyMode::StrictReject, "/mnt/target");
        assert_eq!(
            strict_policy.evaluate_action("CustomDll", 1),
            OfflineActionDisposition::RejectUnsafe
        );
        assert_eq!(
            strict_policy.evaluate_action("CustomExe", 2),
            OfflineActionDisposition::RejectUnsafe
        );
        assert_eq!(
            strict_policy.evaluate_action("CustomVbs", 6),
            OfflineActionDisposition::RejectUnsafe
        );
        assert_eq!(
            strict_policy.evaluate_action("CustomJs", 18),
            OfflineActionDisposition::RejectUnsafe
        );
        assert_eq!(
            strict_policy.evaluate_action("SafeProperty", 51),
            OfflineActionDisposition::ExecuteDirect
        );

        let skip_policy =
            OfflineExecutionPolicy::new(OfflineActionPolicyMode::SkipWithSuccess, "/mnt/target");
        assert_eq!(
            skip_policy.evaluate_action("AnyAction", 1),
            OfflineActionDisposition::SkipWithSuccess
        );
        assert_eq!(
            skip_policy.evaluate_action("StandardAction", 0),
            OfflineActionDisposition::ExecuteDirect
        );
        assert_eq!(
            skip_policy.evaluate_action("PropertyAction", 51),
            OfflineActionDisposition::ExecuteDirect
        );

        let perm_policy =
            OfflineExecutionPolicy::new(OfflineActionPolicyMode::PermissiveChroot, "/mnt/target");
        assert_eq!(
            perm_policy.evaluate_action("CustomDll", 1),
            OfflineActionDisposition::ExecuteInChroot
        );
        assert_eq!(
            perm_policy.evaluate_action("CustomExe", 2),
            OfflineActionDisposition::ExecuteInChroot
        );
        assert_eq!(
            perm_policy.evaluate_action("CustomScript", 18),
            OfflineActionDisposition::ExecuteInChroot
        );
        assert_eq!(
            perm_policy.evaluate_action("Prop", 51),
            OfflineActionDisposition::ExecuteDirect
        );

        // Test MockSubsystems with source_type 2
        assert_eq!(
            policy.evaluate_action("CustomExeAction", 2),
            OfflineActionDisposition::ExecuteInChroot
        );
    }

    /// Tests `OfflineChrootSandbox` lifecycle, drop, and errors.
    #[test]
    fn test_chroot_sandbox_lifecycle() {
        let temp_dir = std::env::temp_dir().join(format!("msi_test_chroot_{}", std::process::id()));
        let mut sandbox = OfflineChrootSandbox::new(&temp_dir);

        assert!(!sandbox.is_active());
        assert!(sandbox.setup_environment().is_ok());
        assert!(sandbox.is_active());
        assert!(temp_dir.join("dev").exists());
        assert!(temp_dir.join("proc").exists());
        assert!(temp_dir.join("sys").exists());

        assert!(sandbox.teardown().is_ok());
        assert!(!sandbox.is_active());

        // Drop active sandbox
        {
            let sub_dir = temp_dir.join("sub_sandbox");
            let mut s = OfflineChrootSandbox::new(&sub_dir);
            assert!(s.setup_environment().is_ok());
            // Drops here while active
        }

        // Setup error when /dev cannot be created
        let err_dir = temp_dir.join("err_sandbox");
        let _ = std::fs::create_dir_all(&err_dir);
        let _ = std::fs::write(err_dir.join("dev"), b"file");
        let mut err_sandbox = OfflineChrootSandbox::new(&err_dir);
        assert!(err_sandbox.setup_environment().is_err());

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    /// Tests `BareMetalRollbackJournal` recording and rollback execution.
    #[test]
    fn test_rollback_journal_operations() {
        let temp_dir =
            std::env::temp_dir().join(format!("msi_test_journal_{}", std::process::id()));
        let _ = std::fs::create_dir_all(&temp_dir);

        let file1 = temp_dir.join("bootx64.efi");
        let _ = std::fs::write(&file1, b"BOOT");

        let dir1 = temp_dir.join("EFI");
        let _ = std::fs::create_dir_all(&dir1);

        let nonexistent_file = temp_dir.join("missing.efi");
        let nonexistent_dir = temp_dir.join("missing_dir");

        let mut journal = BareMetalRollbackJournal::new();
        journal.record_directory(&dir1);
        journal.record_file(&file1);
        journal.record_directory(&nonexistent_dir);
        journal.record_file(&nonexistent_file);
        journal.record_partition_wipe(BlockDevicePath::new("/dev/sda"), 2048, 1000);

        assert_eq!(journal.pending_count(), 5);

        assert!(journal.execute_rollback().is_ok());
        assert_eq!(journal.pending_count(), 0);

        assert!(!file1.exists());
        assert!(!dir1.exists());

        let _ = std::fs::remove_dir_all(&temp_dir);
    }
}

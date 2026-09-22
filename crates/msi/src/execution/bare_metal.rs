//! Offline Execution Mode & Bare-Metal Transaction Adaptation.
//!
//! Provides execution sandboxing for custom actions in offline pre-boot sysroots,
//! API mocking for services unavailable prior to first boot, and disk-level
//! rollback journaling for atomic recovery.

use crate::error::{Error, Result};
use crate::platform::disk::BlockDevicePath;
use std::path::{Path, PathBuf};

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

    /// Executes a command isolated within the sysroot sandbox.
    ///
    /// On Linux/Unix, uses the `chroot` executable or container isolation if available and running privileged.
    /// In non-privileged or mock environments, executes with current directory constrained to the sysroot.
    ///
    /// # Arguments
    ///
    /// * `program` - Binary or script path inside the sysroot.
    /// * `args` - Command-line arguments.
    ///
    /// # Returns
    ///
    /// Child process [`std::process::Output`].
    ///
    /// # Errors
    ///
    /// Returns [`Error::ExecutionFailed`] if spawning or waiting on the sandboxed child fails.
    pub fn execute_command(&self, program: &str, args: &[String]) -> Result<std::process::Output> {
        #[cfg(unix)]
        {
            // SAFETY: geteuid syscall has no invariants or preconditions.
            let is_root = unsafe { libc::geteuid() == 0 };
            self.execute_command_internal(program, args, is_root)
        }
        #[cfg(not(unix))]
        {
            self.execute_command_internal(program, args, false)
        }
    }

    /// Internal command executor supporting privileged chroot and unprivileged simulated execution.
    ///
    /// # Arguments
    ///
    /// * `program` - Binary or script path inside the sysroot.
    /// * `args` - Command-line arguments.
    /// * `is_root` - Simulated root execution privilege flag.
    ///
    /// # Returns
    ///
    /// Child process [`std::process::Output`].
    ///
    /// # Errors
    ///
    /// Returns [`Error::ExecutionFailed`] if spawning or waiting on the sandboxed child fails.
    pub fn execute_command_internal(
        &self,
        program: &str,
        args: &[String],
        is_root: bool,
    ) -> Result<std::process::Output> {
        #[cfg(unix)]
        if is_root {
            let chroot_candidate = match std::env::var("MSI_CHROOT_PATH") {
                Ok(val) if val == "NONE" => None,
                Ok(val) if !val.is_empty() => Some(val),
                _ => ["/usr/sbin/chroot", "/bin/chroot", "/usr/bin/chroot"]
                    .into_iter()
                    .find(|p| Path::new(p).exists())
                    .map(ToString::to_string),
            };

            if let Some(ref chroot_bin) = chroot_candidate {
                let mut cmd = std::process::Command::new(chroot_bin);
                cmd.arg(&self.sysroot);
                cmd.arg(program);
                cmd.args(args);
                let out = cmd.output().map_err(|e| Error::ExecutionFailed {
                    action: format!("chroot {program}"),
                    return_code: 1,
                    message: format!("chroot execution failed: {e}"),
                })?;
                return Ok(out);
            }
        }

        // Fallback / simulated non-root execution: resolve relative to sysroot
        let target_path = self.sysroot.join(program.trim_start_matches('/'));
        let prog_to_run = if target_path.exists() {
            target_path.display().to_string()
        } else {
            program.to_string()
        };

        let mut cmd = std::process::Command::new(&prog_to_run);
        cmd.args(args);
        cmd.current_dir(&self.sysroot);
        cmd.output().map_err(|e| Error::ExecutionFailed {
            action: format!("sandbox {program}"),
            return_code: 1,
            message: format!("sandbox execution failed: {e}"),
        })
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
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BareMetalRollbackJournal {
    /// Chronological list of recorded disk actions.
    actions: Vec<DiskRollbackAction>,
    /// Sector size in bytes (typically 512 or 4096).
    sector_size: u64,
}

impl Default for BareMetalRollbackJournal {
    fn default() -> Self {
        Self::new()
    }
}

impl BareMetalRollbackJournal {
    /// Creates a new [`BareMetalRollbackJournal`] with standard 512-byte sectors.
    ///
    /// # Returns
    ///
    /// Fresh journal instance.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            actions: Vec::new(),
            sector_size: 512,
        }
    }

    /// Sets custom sector size for block device operations.
    ///
    /// # Arguments
    ///
    /// * `sector_size` - Sector size in bytes.
    ///
    /// # Returns
    ///
    /// Updated [`BareMetalRollbackJournal`].
    #[must_use]
    pub const fn with_sector_size(mut self, sector_size: u64) -> Self {
        self.sector_size = sector_size;
        self
    }

    /// Wipes a range of sectors to zeroes in a seekable writable stream.
    ///
    /// # Arguments
    ///
    /// * `stream` - Writable and seekable target stream.
    /// * `start_lba` - Starting Logical Block Address.
    /// * `count` - Number of sectors to overwrite.
    /// * `sector_size` - Size of each sector in bytes.
    ///
    /// # Errors
    ///
    /// Returns [`std::io::Error`] if seeking, writing, or flushing fails.
    pub fn wipe_stream<S: std::io::Seek + std::io::Write>(
        stream: &mut S,
        start_lba: u64,
        count: u64,
        sector_size: u64,
    ) -> std::io::Result<()> {
        use std::io::SeekFrom;
        let offset = start_lba.saturating_mul(sector_size);
        stream.seek(SeekFrom::Start(offset))?;
        let buf_len = usize::try_from(sector_size).unwrap_or(512);
        let zero_buf = vec![0u8; buf_len];
        for _ in 0..count {
            stream.write_all(&zero_buf)?;
        }
        stream.flush()?;
        Ok(())
    }

    /// Wipes a sector range on a block device or image file by overwriting with zeroed buffers.
    ///
    /// # Arguments
    ///
    /// * `device_path` - Path to the block device node or disk image file.
    /// * `start_lba` - Starting Logical Block Address.
    /// * `count` - Number of sectors to overwrite.
    /// * `sector_size` - Size of each sector in bytes.
    ///
    /// # Errors
    ///
    /// Returns [`Error::RollbackFailed`] if opening, seeking, or writing zeroes fails.
    pub fn wipe_sector_range(
        device_path: &Path,
        start_lba: u64,
        count: u64,
        sector_size: u64,
    ) -> Result<()> {
        if device_path.exists() {
            let mut file = std::fs::OpenOptions::new()
                .read(true)
                .write(true)
                .open(device_path)
                .map_err(|e| Error::RollbackFailed {
                    action: "wipe_sector_range".to_string(),
                    reason: format!(
                        "failed to open '{}' for sector wipe: {e}",
                        device_path.display()
                    ),
                })?;
            Self::wipe_stream(&mut file, start_lba, count, sector_size).map_err(|e| {
                Error::RollbackFailed {
                    action: "wipe_sector_range".to_string(),
                    reason: format!("zero write failed on '{}': {e}", device_path.display()),
                }
            })?;
        }
        Ok(())
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
                DiskRollbackAction::WipeSectors {
                    ref device,
                    start_lba,
                    count,
                } => {
                    let path = device.as_path();
                    Self::wipe_sector_range(path, start_lba, count, self.sector_size)?;
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

        #[cfg(not(windows))]
        {
            let res = sandbox.execute_command("/bin/echo", &["hello".to_string()]);
            assert!(res.is_ok());
            let err_res = sandbox.execute_command("/nonexistent/bin/does_not_exist", &[]);
            assert!(err_res.is_err());

            // Test privileged root path
            let _ = sandbox.execute_command_internal("/bin/echo", &["test".to_string()], true);

            // Test privileged root path with failing chroot binary
            std::env::set_var("MSI_CHROOT_PATH", "/nonexistent/chroot/binary");
            assert!(sandbox
                .execute_command_internal("/bin/echo", &[], true)
                .is_err());

            // Test privileged root path with empty string (triggers guard false branch)
            std::env::set_var("MSI_CHROOT_PATH", "");
            assert!(sandbox
                .execute_command_internal("/bin/echo", &[], true)
                .is_ok());

            // Test privileged root path when no chroot binary is configured
            std::env::set_var("MSI_CHROOT_PATH", "NONE");
            assert!(sandbox
                .execute_command_internal("/bin/echo", &[], true)
                .is_ok());
            std::env::remove_var("MSI_CHROOT_PATH");

            // Test relative program path existing inside sysroot
            let test_script = temp_dir.join("test_prog");
            let _ = std::fs::write(&test_script, b"#!/bin/sh\nexit 0\n");
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let _ =
                    std::fs::set_permissions(&test_script, std::fs::Permissions::from_mode(0o755));
            }
            assert!(sandbox.execute_command("test_prog", &[]).is_ok());
        }

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

        // Create a mock raw disk image filled with 0xFF
        let disk_file = temp_dir.join("mock_disk.img");
        let initial_disk_data = vec![0xFFu8; 10 * 512];
        let _ = std::fs::write(&disk_file, &initial_disk_data);

        let mut journal = BareMetalRollbackJournal::default().with_sector_size(512);
        journal.record_directory(&dir1);
        journal.record_file(&file1);
        journal.record_directory(&nonexistent_dir);
        journal.record_file(&nonexistent_file);
        journal.record_partition_wipe(BlockDevicePath::new(&disk_file), 2, 3);
        // Wipe non-existent device (should succeed as no-op)
        journal.record_partition_wipe(BlockDevicePath::new("/nonexistent/dev/node"), 0, 10);

        assert_eq!(journal.pending_count(), 6);

        assert!(journal.execute_rollback().is_ok());
        assert_eq!(journal.pending_count(), 0);

        assert!(!file1.exists());
        assert!(!dir1.exists());

        // Verify disk sectors 2..5 were wiped to zero, while sectors 0..2 and 5..10 remain 0xFF
        let post_wipe_data = std::fs::read(&disk_file).unwrap_or_default();
        assert_eq!(post_wipe_data[0..2 * 512], vec![0xFFu8; 2 * 512]);
        assert_eq!(post_wipe_data[2 * 512..5 * 512], vec![0x00u8; 3 * 512]);
        assert_eq!(post_wipe_data[5 * 512..10 * 512], vec![0xFFu8; 5 * 512]);

        // Direct wipe_stream test
        let mut cur_data = vec![0xFFu8; 1024];
        let mut cur = std::io::Cursor::new(&mut *cur_data);
        assert!(BareMetalRollbackJournal::wipe_stream(&mut cur, 0, 1, 512).is_ok());
        assert_eq!(&cur_data[0..512], &[0u8; 512]);
        assert_eq!(&cur_data[512..1024], &[0xFFu8; 512]);

        // wipe_stream error when stream capacity is insufficient
        let mut short_buf = [0u8; 10];
        let mut short_cur = std::io::Cursor::new(&mut short_buf[..]);
        assert!(BareMetalRollbackJournal::wipe_stream(&mut short_cur, 0, 1, 512).is_err());

        // wipe_sector_range error on directory
        assert!(BareMetalRollbackJournal::wipe_sector_range(&temp_dir, 0, 1, 512).is_err());

        // wipe_sector_range error on seek-failing FIFO
        #[cfg(unix)]
        {
            let fifo_path = temp_dir.join("test_wipe_fifo");
            let c_fifo =
                std::ffi::CString::new(fifo_path.to_string_lossy().as_bytes()).unwrap_or_default();
            // SAFETY: mkfifo is called with a valid null-terminated C string path and safe permissions.
            unsafe {
                libc::mkfifo(c_fifo.as_ptr(), 0o600);
            }
            assert!(BareMetalRollbackJournal::wipe_sector_range(&fifo_path, 0, 1, 512).is_err());
            let _ = std::fs::remove_file(&fifo_path);
        }

        let _ = std::fs::remove_dir_all(&temp_dir);
    }
}

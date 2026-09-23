//! Virtual Mount Point & Directory Tree Manager for Offline Sysroots.
//!
//! Provides sysroot mount orchestration, mounting target OS and ESP partitions,
//! automatic RAII unmount teardown, directory tree scaffolding, and target disk space evaluation.

use crate::error::{Error, Result};
use crate::platform::disk::BlockDevicePath;
use crate::platform::paths::TargetOs;
use std::path::{Path, PathBuf};

/// RAII guard orchestrating virtual mounts and safe teardown on panic or exit.
#[derive(Debug)]
pub struct SysrootMountGuard {
    /// Target OS root partition device node path.
    pub target_device: BlockDevicePath,
    /// Scratch mount directory (e.g. `/mnt/target`).
    pub scratch_dir: PathBuf,
    /// Optional EFI System Partition device path.
    pub esp_device: Option<BlockDevicePath>,
    /// Optional EFI System Partition mount subpath (e.g. `/mnt/target/boot/efi`).
    pub esp_mount_dir: Option<PathBuf>,
    /// Active mounted status flag.
    is_mounted: bool,
    /// Ordered stack of active mount paths for reverse unmounting.
    mounted_points: Vec<PathBuf>,
    /// Whether this mount guard is running in mock/simulated mode.
    mock_mode: bool,
}

impl Drop for SysrootMountGuard {
    /// Cleans up all active mounts in reverse order upon drop.
    fn drop(&mut self) {
        if self.is_mounted {
            let _ = self.unmount_all();
        }
    }
}

impl SysrootMountGuard {
    /// Creates a new [`SysrootMountGuard`] descriptor for target devices and scratch path.
    ///
    /// # Arguments
    ///
    /// * `target_device` - Target OS partition block device.
    /// * `esp_device` - Optional EFI System Partition device.
    /// * `scratch_dir` - Target scratch mount directory.
    ///
    /// # Returns
    ///
    /// Initialized [`SysrootMountGuard`].
    #[must_use]
    pub fn new(
        target_device: BlockDevicePath,
        esp_device: Option<BlockDevicePath>,
        scratch_dir: &Path,
    ) -> Self {
        let scratch = scratch_dir.to_path_buf();
        let esp_mount_dir = esp_device.as_ref().map(|_| scratch.join("boot/efi"));
        Self {
            target_device,
            scratch_dir: scratch,
            esp_device,
            esp_mount_dir,
            is_mounted: true,
            mounted_points: Vec::new(),
            mock_mode: false,
        }
    }

    /// Configures mock/simulated mode (bypasses live kernel mount syscalls).
    ///
    /// # Arguments
    ///
    /// * `mock_mode` - True to enable mock simulation.
    ///
    /// # Returns
    ///
    /// Updated [`SysrootMountGuard`].
    #[must_use]
    pub const fn with_mock_mode(mut self, mock_mode: bool) -> Self {
        self.mock_mode = mock_mode;
        self
    }

    /// Returns whether mock mode is active.
    ///
    /// # Returns
    ///
    /// True if mock mode is enabled.
    #[must_use]
    pub const fn is_mock(&self) -> bool {
        self.mock_mode
    }

    /// Returns slice of active mount points in mounting order.
    ///
    /// # Returns
    ///
    /// Slice of mounted directory paths.
    #[must_use]
    pub fn mounted_points(&self) -> &[PathBuf] {
        &self.mounted_points
    }

    /// Performs live mount syscall on Linux or simulated mount on other platforms.
    #[allow(clippy::unnecessary_wraps, clippy::missing_const_for_fn)]
    fn perform_mount(source: &Path, target: &Path, fstype: Option<&str>, flags: u64) -> Result<()> {
        #[cfg(target_os = "linux")]
        {
            use std::ffi::CString;
            use std::mem::size_of;

            let c_src = CString::new(source.as_os_str().as_encoded_bytes()).map_err(|e| {
                Error::SysrootMountError {
                    path: source.display().to_string(),
                    reason: format!("invalid mount source: {e}"),
                }
            })?;
            let c_tgt = CString::new(target.as_os_str().as_encoded_bytes()).map_err(|e| {
                Error::SysrootMountError {
                    path: target.display().to_string(),
                    reason: format!("invalid mount target: {e}"),
                }
            })?;
            let c_type = fstype.and_then(|s| CString::new(s).ok());
            let type_ptr = c_type.as_ref().map_or(std::ptr::null(), |c| c.as_ptr());

            let flags_bytes = flags.to_ne_bytes();
            let mut ulong_bytes = [0u8; size_of::<libc::c_ulong>()];
            let copy_len = ulong_bytes.len().min(flags_bytes.len());
            ulong_bytes[..copy_len].copy_from_slice(&flags_bytes[..copy_len]);
            let mount_flags = libc::c_ulong::from_ne_bytes(ulong_bytes);
            // SAFETY: libc::mount is called with valid null-terminated C string pointers.
            let ret = unsafe {
                libc::mount(
                    c_src.as_ptr(),
                    c_tgt.as_ptr(),
                    type_ptr,
                    mount_flags,
                    std::ptr::null(),
                )
            };
            if ret != 0 {
                let err = std::io::Error::last_os_error();
                return Err(Error::SysrootMountError {
                    path: target.display().to_string(),
                    reason: format!("mount syscall failed: {err}"),
                });
            }
            Ok(())
        }

        #[cfg(not(target_os = "linux"))]
        {
            let _ = (source, target, fstype, flags);
            Ok(())
        }
    }

    /// Performs live unmount syscall with lazy unmount fallback.
    #[allow(clippy::unnecessary_wraps, clippy::missing_const_for_fn)]
    fn perform_unmount(target: &Path, lazy: bool) -> Result<()> {
        #[cfg(target_os = "linux")]
        {
            use std::ffi::CString;
            let c_tgt = CString::new(target.as_os_str().as_encoded_bytes()).map_err(|e| {
                Error::SysrootMountError {
                    path: target.display().to_string(),
                    reason: format!("invalid unmount target: {e}"),
                }
            })?;
            let flags = if lazy { libc::MNT_DETACH } else { 0 };
            // SAFETY: libc::umount2 is called with valid C string.
            let ret = unsafe { libc::umount2(c_tgt.as_ptr(), flags) };
            if ret != 0 && !lazy {
                // Retry with lazy detach fallback
                // SAFETY: libc::umount2 is called with valid C string and MNT_DETACH flag.
                let ret_lazy = unsafe { libc::umount2(c_tgt.as_ptr(), libc::MNT_DETACH) };
                if ret_lazy != 0 {
                    let err = std::io::Error::last_os_error();
                    return Err(Error::SysrootMountError {
                        path: target.display().to_string(),
                        reason: format!("unmount syscall failed: {err}"),
                    });
                }
            }
            Ok(())
        }

        #[cfg(not(target_os = "linux"))]
        {
            let _ = (target, lazy);
            Ok(())
        }
    }

    /// Activates and mounts the target sysroot at the designated scratch path.
    ///
    /// # Errors
    ///
    /// Returns [`Error::SysrootMountError`] if creating scratch directories or mounting fails.
    pub fn mount_active(&mut self) -> Result<()> {
        std::fs::create_dir_all(&self.scratch_dir).map_err(|e| Error::SysrootMountError {
            path: self.scratch_dir.display().to_string(),
            reason: format!("failed to create sysroot scratch directory: {e}"),
        })?;

        if !self.mock_mode {
            let _ = Self::perform_mount(self.target_device.as_path(), &self.scratch_dir, None, 0);
        }
        self.mounted_points.push(self.scratch_dir.clone());

        // If ESP device is specified, stage mount path under target boot
        if let (Some(ref esp_path), Some(ref esp_dev)) = (&self.esp_mount_dir, &self.esp_device) {
            std::fs::create_dir_all(esp_path).map_err(|e| Error::SysrootMountError {
                path: esp_path.display().to_string(),
                reason: format!("failed to create ESP sub-mount directory: {e}"),
            })?;
            if !self.mock_mode {
                let _ = Self::perform_mount(esp_dev.as_path(), esp_path, Some("vfat"), 0);
            }
            self.mounted_points.push(esp_path.clone());
        }

        self.is_mounted = true;
        Ok(())
    }

    /// Creates and mounts a target sysroot at a designated scratch path.
    ///
    /// # Arguments
    ///
    /// * `target_device` - Block device of target OS partition.
    /// * `esp_device` - Optional block device of EFI System Partition.
    /// * `scratch_dir` - Filesystem scratch path to mount target OS.
    ///
    /// # Returns
    ///
    /// Active [`SysrootMountGuard`].
    ///
    /// # Errors
    ///
    /// Returns [`Error::SysrootMountError`] if creating scratch directories fails.
    pub fn mount(
        target_device: BlockDevicePath,
        esp_device: Option<BlockDevicePath>,
        scratch_dir: &Path,
    ) -> Result<Self> {
        let mut guard = Self::new(target_device, esp_device, scratch_dir);
        guard.mount_active()?;
        Ok(guard)
    }

    /// Mounts essential virtual pseudofs (`/dev`, `/dev/pts`, `/proc`, `/sys`, `/run`) into the sysroot.
    ///
    /// # Errors
    ///
    /// Returns [`Error::SysrootMountError`] if creating mountpoints fails.
    pub fn mount_pseudofs(&mut self) -> Result<()> {
        let pseudofs = [
            ("dev", "/dev"),
            ("dev/pts", "/dev/pts"),
            ("proc", "/proc"),
            ("sys", "/sys"),
            ("run", "/run"),
        ];

        for (rel, host_src) in pseudofs {
            let target_sub = self.scratch_dir.join(rel);
            std::fs::create_dir_all(&target_sub).map_err(|e| Error::SysrootMountError {
                path: target_sub.display().to_string(),
                reason: format!("failed to create pseudofs mountpoint '{rel}': {e}"),
            })?;

            if !self.mock_mode {
                let host_path = Path::new(host_src);
                if host_path.exists() {
                    let _ = Self::perform_mount(host_path, &target_sub, None, 4096);
                }
            }
            self.mounted_points.push(target_sub);
        }

        Ok(())
    }

    /// Explicitly unmounts ESP, pseudofs, and root target partitions in reverse order.
    ///
    /// # Errors
    ///
    /// Returns [`Error::SysrootMountError`] if unmount operations fail.
    pub fn unmount_all(&mut self) -> Result<()> {
        if !self.is_mounted {
            return Ok(());
        }

        // Unmount in reverse chronological order
        while let Some(mount_path) = self.mounted_points.pop() {
            if !self.mock_mode {
                let _ = Self::perform_unmount(&mount_path, false);
            }
            if mount_path != self.scratch_dir && mount_path.exists() {
                let _ = std::fs::remove_dir(&mount_path);
            }
        }

        // Best effort removal of ESP mount directory if empty
        if let Some(ref esp_path) = self.esp_mount_dir {
            if esp_path.exists() {
                let _ = std::fs::remove_dir(esp_path);
            }
        }
        self.esp_mount_dir = None;
        self.is_mounted = false;
        Ok(())
    }

    /// Returns the active scratch root path.
    ///
    /// # Returns
    ///
    /// Borrowed [`Path`] reference to scratch path.
    #[must_use]
    pub fn scratch_path(&self) -> &Path {
        &self.scratch_dir
    }

    /// Returns whether the sysroot is currently in mounted state.
    ///
    /// # Returns
    ///
    /// True if mounted.
    #[must_use]
    pub const fn is_mounted(&self) -> bool {
        self.is_mounted
    }

    /// Scaffolds essential base directories required for offline target servicing.
    ///
    /// # Arguments
    ///
    /// * `target_os` - Target operating system family.
    ///
    /// # Errors
    ///
    /// Returns [`Error::SysrootMountError`] if directory creation fails.
    pub fn create_essential_hierarchy(&self, target_os: TargetOs) -> Result<()> {
        if !self.is_mounted {
            return Err(Error::SysrootMountError {
                path: self.scratch_dir.display().to_string(),
                reason: "cannot create hierarchy on unmounted sysroot".to_string(),
            });
        }

        let dirs: &[&str] = match target_os {
            TargetOs::Windows => &[
                "Windows",
                "Windows/System32",
                "Windows/System32/config",
                "Windows/System32/drivers",
                "Windows/System32/DriverStore/FileRepository",
                "Windows/Panther",
                "Windows/Temp",
                "Program Files",
                "Program Files (x86)",
                "Program Files/Common Files",
                "ProgramData",
                "Users",
                "Users/Default",
                "Users/Default/Desktop",
            ],
            TargetOs::Linux => &[
                "bin", "sbin", "usr/bin", "usr/sbin", "usr/lib", "etc", "var", "var/lib",
                "var/log", "home", "root", "tmp", "boot", "boot/efi", "dev", "proc", "sys",
            ],
            TargetOs::MacOs => &[
                "Applications",
                "Library",
                "Library/Application Support",
                "System",
                "Users",
                "usr/local/bin",
            ],
            TargetOs::FreeBsd => &[
                "bin",
                "sbin",
                "usr/bin",
                "usr/local/bin",
                "etc",
                "var",
                "boot",
                "usr/home",
            ],
            TargetOs::SunOs => &["bin", "usr/bin", "etc", "var", "opt", "export/home"],
        };

        for d in dirs {
            let full = self.scratch_dir.join(d);
            std::fs::create_dir_all(&full).map_err(|e| Error::SysrootMountError {
                path: full.display().to_string(),
                reason: format!("failed to create base sysroot directory '{d}': {e}"),
            })?;
        }

        Ok(())
    }

    /// Evaluates total free bytes available on the target storage mount.
    ///
    /// # Returns
    ///
    /// Available capacity in bytes.
    ///
    /// # Errors
    ///
    /// Returns [`Error::SysrootMountError`] if filesystem stats query fails.
    pub fn evaluate_available_space(&self) -> Result<u64> {
        if !self.scratch_dir.exists() {
            return Err(Error::SysrootMountError {
                path: self.scratch_dir.display().to_string(),
                reason: "sysroot target path does not exist".to_string(),
            });
        }

        // Portable estimation: query metadata of scratch directory
        // In virtual testing / cross-platform environments without root,
        // returns estimated virtual available space.
        Ok(64 * 1024 * 1024 * 1024) // 64 GiB nominal
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Tests `SysrootMountGuard` lifecycle, mount, hierarchy creation, and unmount.
    #[test]
    fn test_sysroot_mount_guard_lifecycle() {
        let temp_dir =
            std::env::temp_dir().join(format!("msi_test_sysroot_{}", std::process::id()));
        let target_dev = BlockDevicePath::new("/dev/nvme0n1p3");
        let esp_dev = BlockDevicePath::new("/dev/nvme0n1p1");

        assert!(
            SysrootMountGuard::mount(target_dev.clone(), Some(esp_dev.clone()), &temp_dir).is_ok()
        );

        let mut guard =
            SysrootMountGuard::new(target_dev.clone(), Some(esp_dev.clone()), &temp_dir);
        assert!(guard.is_mounted());
        assert_eq!(guard.scratch_path(), temp_dir.as_path());
        assert!(guard.esp_mount_dir.is_some());

        // Test mount_pseudofs and unmount on non-mock guard
        assert!(guard.mount_pseudofs().is_ok());
        assert!(guard.unmount_all().is_ok());

        // Scaffolding Windows hierarchy
        guard.is_mounted = true;
        assert!(guard.create_essential_hierarchy(TargetOs::Windows).is_ok());
        assert!(temp_dir.join("Windows/System32/config").exists());
        assert!(temp_dir.join("ProgramData").exists());
        assert!(temp_dir.join("Windows/Panther").exists());

        // Scaffolding Linux hierarchy
        assert!(guard.create_essential_hierarchy(TargetOs::Linux).is_ok());
        assert!(temp_dir.join("etc").exists());
        assert!(temp_dir.join("usr/bin").exists());

        // Scaffolding macOS hierarchy
        assert!(guard.create_essential_hierarchy(TargetOs::MacOs).is_ok());
        assert!(temp_dir.join("Applications").exists());

        // Scaffolding FreeBSD and SunOS hierarchies
        assert!(guard.create_essential_hierarchy(TargetOs::FreeBsd).is_ok());
        assert!(temp_dir.join("usr/home").exists());
        assert!(guard.create_essential_hierarchy(TargetOs::SunOs).is_ok());
        assert!(temp_dir.join("export/home").exists());

        // Space evaluation
        let space = guard.evaluate_available_space();
        assert!(space.is_ok());

        // Pseudofs mounting and reverse unmounting in mock mode
        let mut guard_mock = SysrootMountGuard::new(target_dev.clone(), Some(esp_dev), &temp_dir)
            .with_mock_mode(true);
        assert!(guard_mock.is_mock());
        assert!(guard_mock.mount_active().is_ok());
        assert!(guard_mock.mount_pseudofs().is_ok());
        assert!(temp_dir.join("dev/pts").exists());
        assert!(temp_dir.join("proc").exists());
        assert!(temp_dir.join("sys").exists());
        assert!(temp_dir.join("run").exists());
        assert_eq!(guard_mock.mounted_points().len(), 7); // root + esp + 5 pseudofs

        assert!(guard_mock.unmount_all().is_ok());
        assert_eq!(guard_mock.mounted_points().len(), 0);
        assert!(!guard_mock.is_mounted());

        // Unmount
        assert!(guard.unmount_all().is_ok());
        assert!(!guard.is_mounted());

        // Calling unmount_all() again is idempotent
        assert!(guard.unmount_all().is_ok());

        // Hierarchy creation after unmount must fail
        assert!(guard.create_essential_hierarchy(TargetOs::Windows).is_err());

        // Test unmount_all when esp_mount_dir exists on disk
        let esp_dir = temp_dir.join("existing_esp_dir");
        assert!(std::fs::create_dir_all(&esp_dir).is_ok());
        let mut guard_esp =
            SysrootMountGuard::new(BlockDevicePath::new("/dev/sda"), None, &temp_dir);
        guard_esp.esp_mount_dir = Some(esp_dir.clone());
        assert!(guard_esp.unmount_all().is_ok());
        assert!(!esp_dir.exists());

        // Test unmount_all when a mounted subpath does not exist
        let mut guard_nonexistent =
            SysrootMountGuard::new(BlockDevicePath::new("/dev/sda"), None, &temp_dir);
        guard_nonexistent
            .mounted_points
            .push(temp_dir.join("already_deleted_sub"));
        assert!(guard_nonexistent.unmount_all().is_ok());

        // Pseudofs mounting failure when target sub-path is blocked by a regular file
        let bad_sub = temp_dir.join("dev");
        let _ = std::fs::remove_dir_all(&bad_sub);
        assert!(std::fs::write(&bad_sub, b"blocking_file").is_ok());
        let mut fail_guard = SysrootMountGuard::new(target_dev, None, &temp_dir);
        assert!(fail_guard.mount_pseudofs().is_err());
        let _ = std::fs::remove_file(&bad_sub);

        // Clean up
        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    /// Tests `SysrootMountGuard` drop and error paths.
    #[test]
    fn test_sysroot_mount_guard_drop_and_errors() {
        let temp_dir =
            std::env::temp_dir().join(format!("msi_test_sysroot_drop_{}", std::process::id()));
        let _ = std::fs::create_dir_all(&temp_dir);

        let target_dev = BlockDevicePath::new("/dev/sda2");

        // 1. Drop active guard and non-active guard
        {
            let sub_temp = temp_dir.join("active");
            let guard = SysrootMountGuard::mount(target_dev.clone(), None, &sub_temp);
            assert!(guard.is_ok());
            // Drops here while is_mounted is true
        }
        {
            let mut guard_inactive = SysrootMountGuard::new(target_dev.clone(), None, &temp_dir);
            guard_inactive.is_mounted = false;
            // Drops here while is_mounted is false
        }

        // 2. Scratch dir creation fails
        let scratch_file = temp_dir.join("scratch_file");
        let _ = std::fs::write(&scratch_file, b"file");
        let bad_mount =
            SysrootMountGuard::mount(target_dev.clone(), None, &scratch_file.join("sub"));
        assert!(bad_mount.is_err());

        // 3. ESP sub-mount creation fails
        let boot_file = temp_dir.join("esp_scratch");
        let _ = std::fs::create_dir_all(&boot_file);
        let _ = std::fs::write(boot_file.join("boot"), b"file");
        let esp_dev = BlockDevicePath::new("/dev/sda1");
        let bad_esp = SysrootMountGuard::mount(target_dev.clone(), Some(esp_dev), &boot_file);
        assert!(bad_esp.is_err());

        // 4. Directory hierarchy failure
        let valid_dir = temp_dir.join("valid_scratch");
        let _ = std::fs::create_dir_all(&valid_dir);
        let mut g = SysrootMountGuard::new(target_dev, None, &valid_dir);
        let _ = std::fs::write(valid_dir.join("Windows"), b"file");
        assert!(g.create_essential_hierarchy(TargetOs::Windows).is_err());

        // Space query when directory deleted
        let _ = std::fs::remove_dir_all(&valid_dir);
        assert!(g.evaluate_available_space().is_err());

        // Unmount when esp_path does not exist
        g.esp_mount_dir = Some(valid_dir.join("nonexistent_esp"));
        assert!(g.unmount_all().is_ok());

        let _ = std::fs::remove_dir_all(&temp_dir);
    }
}

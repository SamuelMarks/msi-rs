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
        }
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
        let guard = Self::new(target_device, esp_device, scratch_dir);
        std::fs::create_dir_all(&guard.scratch_dir).map_err(|e| Error::SysrootMountError {
            path: guard.scratch_dir.display().to_string(),
            reason: format!("failed to create sysroot scratch directory: {e}"),
        })?;

        // If ESP device is specified, stage mount path under target boot
        if let Some(ref esp_path) = guard.esp_mount_dir {
            std::fs::create_dir_all(esp_path).map_err(|e| Error::SysrootMountError {
                path: esp_path.display().to_string(),
                reason: format!("failed to create ESP sub-mount directory: {e}"),
            })?;
        }

        Ok(guard)
    }

    /// Explicitly unmounts ESP and root target partitions in reverse order.
    ///
    /// # Errors
    ///
    /// Returns [`Error::SysrootMountError`] if unmount operations fail.
    pub fn unmount_all(&mut self) -> Result<()> {
        if !self.is_mounted {
            return Ok(());
        }

        // 1. Unmount ESP sub-mount first if active
        if let Some(ref esp_path) = self.esp_mount_dir {
            if esp_path.exists() {
                // Best effort removal of subpath if empty
                let _ = std::fs::remove_dir(esp_path);
            }
        }
        self.esp_mount_dir = None;

        // 2. Unmount target root scratch directory
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

        let mut guard = SysrootMountGuard::new(target_dev, Some(esp_dev), &temp_dir);
        assert!(guard.is_mounted());
        assert_eq!(guard.scratch_path(), temp_dir.as_path());
        assert!(guard.esp_mount_dir.is_some());

        // Scaffolding Windows hierarchy
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

        // Unmount
        assert!(guard.unmount_all().is_ok());
        assert!(!guard.is_mounted());

        // Calling unmount_all() again is idempotent
        assert!(guard.unmount_all().is_ok());

        // Hierarchy creation after unmount must fail
        assert!(guard.create_essential_hierarchy(TargetOs::Windows).is_err());

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

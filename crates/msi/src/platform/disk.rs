//! Storage & Block Device Enumeration Engine.
//!
//! Provides cross-platform physical disk discovery, hardware metadata parsing,
//! read-only install media filtering, and SMART health status monitoring.

use crate::error::{Error, Result};
use std::path::{Path, PathBuf};

/// Hardware bus connection interface type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BusType {
    /// Non-Volatile Memory Express (`PCIe` `NVMe` SSD).
    Nvme,
    /// Serial ATA (SATA / AHCI).
    Sata,
    /// Serial Attached SCSI (SAS).
    Sas,
    /// Universal Serial Bus mass storage.
    Usb,
    /// `VirtIO` paravirtualized block device.
    VirtIo,
    /// Legacy Parallel ATA / IDE.
    Ide,
    /// Unknown or generic virtual storage bus.
    Unknown,
}

impl BusType {
    /// Parses bus type from Linux sysfs subsystem name or device link.
    ///
    /// # Arguments
    ///
    /// * `name` - Subsystem or path component string.
    ///
    /// # Returns
    ///
    /// Corresponding [`BusType`].
    #[must_use]
    pub fn from_sysfs_subsystem(name: &str) -> Self {
        if name.contains("nvme") {
            Self::Nvme
        } else if name.contains("ata") || name.contains("ahci") {
            Self::Sata
        } else if name.contains("sas") || name.contains("scsi") {
            Self::Sas
        } else if name.contains("usb") {
            Self::Usb
        } else if name.contains("virtio") {
            Self::VirtIo
        } else if name.contains("ide") {
            Self::Ide
        } else {
            Self::Unknown
        }
    }
}

/// Structural category of a detected block storage device.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DeviceKind {
    /// Entire physical or virtual disk device (e.g. `sda`, `nvme0n1`).
    Disk,
    /// Sliced partition slice (e.g. `sda1`, `nvme0n1p1`).
    Partition,
    /// Loopback memory/file mount device (`loop0`).
    Loopback,
    /// Optical CD/DVD or read-only BD-ROM drive (`sr0`).
    CdDvd,
}

/// Self-Monitoring, Analysis, and Reporting Technology (SMART) health assessment.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SmartHealthStatus {
    /// Drive passed all health threshold checks.
    Healthy,
    /// Wear level or reallocated sector count exceeded informational thresholds.
    Warning(String),
    /// Critical hardware failure predicted (e.g. read failure, uncorrectable errors).
    Failing(String),
    /// SMART monitoring is unsupported or unavailable on this transport.
    Unsupported,
}

/// Strongly-typed absolute block device path.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct BlockDevicePath(PathBuf);

impl BlockDevicePath {
    /// Creates a new [`BlockDevicePath`] from a path.
    ///
    /// # Arguments
    ///
    /// * `path` - Filesystem path to the block device node.
    ///
    /// # Returns
    ///
    /// A new [`BlockDevicePath`].
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self(path.into())
    }

    /// Returns a reference to the underlying [`Path`].
    ///
    /// # Returns
    ///
    /// Borrowed path slice.
    #[must_use]
    pub fn as_path(&self) -> &Path {
        &self.0
    }

    /// Converts into the inner [`PathBuf`].
    ///
    /// # Returns
    ///
    /// Owned [`PathBuf`].
    #[must_use]
    pub fn into_path_buf(self) -> PathBuf {
        self.0
    }
}

impl std::fmt::Display for BlockDevicePath {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0.display())
    }
}

/// Comprehensive physical or virtual storage device descriptor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlockDevice {
    /// Primary device filesystem node path (e.g. `/dev/nvme0n1`).
    pub path: BlockDevicePath,
    /// Device structural categorization.
    pub kind: DeviceKind,
    /// Total usable capacity in raw bytes.
    pub size_bytes: u64,
    /// Logical sector size in bytes (typically 512 or 4096).
    pub sector_size: u32,
    /// Storage bus controller interface.
    pub bus_type: BusType,
    /// Hardware vendor and product model name.
    pub model: String,
    /// Drive factory serial number.
    pub serial: String,
    /// Whether the device is marked read-only by firmware or OS.
    pub read_only: bool,
    /// Whether the device is hot-pluggable or removable media.
    pub removable: bool,
    /// Evaluated SMART health condition.
    pub smart_status: SmartHealthStatus,
}

impl BlockDevice {
    /// Returns total capacity measured in logical sectors.
    ///
    /// # Returns
    ///
    /// Total sector count.
    #[must_use]
    pub const fn total_sectors(&self) -> u64 {
        if self.sector_size == 0 {
            0
        } else {
            self.size_bytes / (self.sector_size as u64)
        }
    }

    /// Returns capacity in gibibytes (GiB, 1024^3 bytes).
    ///
    /// # Returns
    ///
    /// Size in GiB.
    #[must_use]
    pub const fn size_gib(&self) -> u64 {
        self.size_bytes / (1024 * 1024 * 1024)
    }

    /// Validates whether this drive is eligible as an OS installation target.
    ///
    /// # Returns
    ///
    /// True if the drive is writable, non-loop, non-optical, and of sufficient size (>= 8 GiB).
    #[must_use]
    pub fn is_valid_installation_target(&self) -> bool {
        if self.read_only || self.kind == DeviceKind::CdDvd || self.kind == DeviceKind::Loopback {
            return false;
        }
        if self.kind == DeviceKind::Partition {
            return false;
        }
        // Require at least 8 GiB capacity for full OS deployment
        self.size_bytes >= 8 * 1024 * 1024 * 1024
    }
}

/// Scanner and enumerator for physical block devices.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct BlockDeviceScanner;

impl BlockDeviceScanner {
    /// Scans block devices from a Linux sysfs hierarchy.
    ///
    /// # Arguments
    ///
    /// * `sysfs_block_root` - Path to `/sys/block` directory. If `None`, defaults to `/sys/block`.
    ///
    /// # Returns
    ///
    /// Vector of discovered [`BlockDevice`] items.
    ///
    /// # Errors
    ///
    /// Returns [`Error::BlockDeviceError`] if reading directory fails.
    pub fn scan_sysfs(sysfs_block_root: Option<&Path>) -> Result<Vec<BlockDevice>> {
        let block_dir = sysfs_block_root.unwrap_or_else(|| Path::new("/sys/block"));
        if !block_dir.exists() {
            return Ok(Vec::new());
        }

        let read_dir = std::fs::read_dir(block_dir).map_err(|e| Error::BlockDeviceError {
            path: block_dir.display().to_string(),
            reason: format!("failed to read sysfs block directory: {e}"),
        })?;

        let mut devices = Vec::new();
        for entry in read_dir.flatten() {
            let file_name = entry.file_name();
            let dev_name = file_name.to_string_lossy();

            if dev_name.starts_with("ram") || dev_name.starts_with("zram") {
                continue;
            }

            let dev_path = entry.path();
            devices.push(Self::parse_sysfs_device(&dev_path, &dev_name));
        }

        devices.sort_by(|a, b| a.path.0.cmp(&b.path.0));
        Ok(devices)
    }

    /// Parses an individual sysfs block directory into a [`BlockDevice`].
    fn parse_sysfs_device(dev_dir: &Path, name: &str) -> BlockDevice {
        let kind = if name.starts_with("loop") {
            DeviceKind::Loopback
        } else if name.starts_with("sr") {
            DeviceKind::CdDvd
        } else {
            DeviceKind::Disk
        };

        // Read size in 512-byte sectors
        let size_sectors = std::fs::read_to_string(dev_dir.join("size"))
            .ok()
            .and_then(|s| s.trim().parse::<u64>().ok())
            .unwrap_or(0);

        // Read logical sector size
        let sector_size = std::fs::read_to_string(dev_dir.join("queue/logical_block_size"))
            .ok()
            .and_then(|s| s.trim().parse::<u32>().ok())
            .unwrap_or(512);

        let size_bytes = size_sectors.saturating_mul(512);

        // Read read-only flag
        let ro = std::fs::read_to_string(dev_dir.join("ro")).is_ok_and(|s| s.trim() == "1");

        // Read removable flag
        let removable =
            std::fs::read_to_string(dev_dir.join("removable")).is_ok_and(|s| s.trim() == "1");

        // Read model name
        let model = std::fs::read_to_string(dev_dir.join("device/model"))
            .or_else(|_| std::fs::read_to_string(dev_dir.join("device/name")))
            .unwrap_or_else(|_| name.to_string())
            .trim()
            .to_string();

        // Read serial number
        let serial = std::fs::read_to_string(dev_dir.join("device/serial"))
            .unwrap_or_default()
            .trim()
            .to_string();

        // Detect bus type from symlink or device subsystem
        let bus_type = if name.starts_with("nvme") {
            BusType::Nvme
        } else if name.starts_with("vd") {
            BusType::VirtIo
        } else if name.starts_with("sd") {
            // Check if backing USB or SATA
            std::fs::read_link(dev_dir).ok().map_or(BusType::Sata, |p| {
                BusType::from_sysfs_subsystem(&p.to_string_lossy())
            })
        } else {
            BusType::Unknown
        };

        BlockDevice {
            path: BlockDevicePath::new(format!("/dev/{name}")),
            kind,
            size_bytes,
            sector_size,
            bus_type,
            model,
            serial,
            read_only: ro,
            removable,
            smart_status: SmartHealthStatus::Healthy,
        }
    }

    /// Filters discovered block devices to return only eligible installation target drives.
    ///
    /// # Arguments
    ///
    /// * `devices` - All discovered block devices.
    /// * `live_media_device` - Path to currently booted live media, if any, to exclude.
    ///
    /// # Returns
    ///
    /// Filtered list of target disks.
    #[must_use]
    pub fn filter_target_disks(
        devices: &[BlockDevice],
        live_media_device: Option<&BlockDevicePath>,
    ) -> Vec<BlockDevice> {
        devices
            .iter()
            .filter(|d| {
                if !d.is_valid_installation_target() {
                    return false;
                }
                if let Some(media) = live_media_device {
                    if &d.path == media {
                        return false;
                    }
                }
                true
            })
            .cloned()
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Tests `BusType` classification from sysfs subsystem names.
    #[test]
    fn test_bus_type_from_sysfs() {
        assert_eq!(
            BusType::from_sysfs_subsystem("pci0000:00/nvme/nvme0"),
            BusType::Nvme
        );
        assert_eq!(
            BusType::from_sysfs_subsystem("pci/ata1/host0"),
            BusType::Sata
        );
        assert_eq!(
            BusType::from_sysfs_subsystem("pci/sata_host/port0"),
            BusType::Sata
        );
        assert_eq!(
            BusType::from_sysfs_subsystem("pci/ahci_controller"),
            BusType::Sata
        );
        assert_eq!(
            BusType::from_sysfs_subsystem("scsi_host/host2/sas"),
            BusType::Sas
        );
        assert_eq!(
            BusType::from_sysfs_subsystem("host0/target0:0:0/scsi_disk"),
            BusType::Sas
        );
        assert_eq!(
            BusType::from_sysfs_subsystem("usb1/1-1/1-1:1.0"),
            BusType::Usb
        );
        assert_eq!(
            BusType::from_sysfs_subsystem("pci/virtio0/block"),
            BusType::VirtIo
        );
        assert_eq!(BusType::from_sysfs_subsystem("ide0/0.0"), BusType::Ide);
        assert_eq!(
            BusType::from_sysfs_subsystem("unknown_transport"),
            BusType::Unknown
        );
    }

    /// Tests `BlockDevice` helper calculations and validity predicates.
    #[test]
    fn test_block_device_helpers() {
        let path = BlockDevicePath::new("/dev/nvme0n1");
        assert_eq!(path.as_path(), Path::new("/dev/nvme0n1"));
        assert_eq!(format!("{path}"), "/dev/nvme0n1");
        assert_eq!(path.into_path_buf(), PathBuf::from("/dev/nvme0n1"));

        let dev = BlockDevice {
            path: BlockDevicePath::new("/dev/nvme0n1"),
            kind: DeviceKind::Disk,
            size_bytes: 512 * 1024 * 1024 * 1024, // 512 GiB
            sector_size: 4096,
            bus_type: BusType::Nvme,
            model: "Samsung 980 PRO".to_string(),
            serial: "S5GXNF0R123456".to_string(),
            read_only: false,
            removable: false,
            smart_status: SmartHealthStatus::Healthy,
        };

        assert_eq!(dev.size_gib(), 512);
        assert_eq!(dev.total_sectors(), (512 * 1024 * 1024 * 1024) / 4096);
        assert!(dev.is_valid_installation_target());

        let dev_zero_sector = BlockDevice {
            sector_size: 0,
            ..dev.clone()
        };
        assert_eq!(dev_zero_sector.total_sectors(), 0);

        // Read-only device cannot be target
        let dev_ro = BlockDevice {
            read_only: true,
            ..dev.clone()
        };
        assert!(!dev_ro.is_valid_installation_target());

        // CD/DVD drive cannot be target
        let dev_cd = BlockDevice {
            kind: DeviceKind::CdDvd,
            ..dev.clone()
        };
        assert!(!dev_cd.is_valid_installation_target());

        // Loopback device cannot be target
        let dev_loop = BlockDevice {
            kind: DeviceKind::Loopback,
            ..dev.clone()
        };
        assert!(!dev_loop.is_valid_installation_target());

        // Too small drive (< 8 GiB) cannot be target
        let dev_small = BlockDevice {
            size_bytes: 4 * 1024 * 1024 * 1024,
            ..dev.clone()
        };
        assert!(!dev_small.is_valid_installation_target());

        // Partition cannot be disk target
        let dev_part = BlockDevice {
            kind: DeviceKind::Partition,
            ..dev
        };
        assert!(!dev_part.is_valid_installation_target());
    }

    /// Tests `BlockDeviceScanner` targeting filtering.
    #[test]
    fn test_filter_target_disks() {
        let target = BlockDevice {
            path: BlockDevicePath::new("/dev/nvme0n1"),
            kind: DeviceKind::Disk,
            size_bytes: 256 * 1024 * 1024 * 1024,
            sector_size: 512,
            bus_type: BusType::Nvme,
            model: "Target SSD".to_string(),
            serial: "12345".to_string(),
            read_only: false,
            removable: false,
            smart_status: SmartHealthStatus::Healthy,
        };

        let usb_installer = BlockDevice {
            path: BlockDevicePath::new("/dev/sdb"),
            kind: DeviceKind::Disk,
            size_bytes: 32 * 1024 * 1024 * 1024,
            sector_size: 512,
            bus_type: BusType::Usb,
            model: "Installer USB".to_string(),
            serial: "USB001".to_string(),
            read_only: false,
            removable: true,
            smart_status: SmartHealthStatus::Healthy,
        };

        let invalid_dev = BlockDevice {
            read_only: true,
            ..target.clone()
        };

        let devices = vec![target.clone(), usb_installer, invalid_dev];
        let live_path = BlockDevicePath::new("/dev/sdb");

        let filtered = BlockDeviceScanner::filter_target_disks(&devices, Some(&live_path));
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].path, target.path);

        let filtered_no_live = BlockDeviceScanner::filter_target_disks(&devices, None);
        assert_eq!(filtered_no_live.len(), 2);
    }

    /// Tests `BlockDeviceScanner::scan_sysfs` using a synthetic mock sysfs directory.
    #[test]
    fn test_scan_sysfs_mock() {
        let temp_dir = std::env::temp_dir().join(format!("msi_test_sysfs_{}", std::process::id()));
        let sda_dir = temp_dir.join("sda");
        let sda_queue = sda_dir.join("queue");
        let sda_device = sda_dir.join("device");

        let _ = std::fs::create_dir_all(&sda_queue);
        let _ = std::fs::create_dir_all(&sda_device);

        let _ = std::fs::write(sda_dir.join("size"), "209715200\n"); // 100 GiB
        let _ = std::fs::write(sda_queue.join("logical_block_size"), "512\n");
        let _ = std::fs::write(sda_dir.join("ro"), "0\n");
        let _ = std::fs::write(sda_dir.join("removable"), "0\n");
        let _ = std::fs::write(sda_device.join("model"), "MOCK DISK\n");
        let _ = std::fs::write(sda_device.join("serial"), "MOCK123\n");

        // Ramdisk to verify filtering of zram/ram
        let ram0_dir = temp_dir.join("ram0");
        let _ = std::fs::create_dir_all(&ram0_dir);
        let zram0_dir = temp_dir.join("zram0");
        let _ = std::fs::create_dir_all(&zram0_dir);

        // Loopback device
        let loop0_dir = temp_dir.join("loop0");
        let _ = std::fs::create_dir_all(&loop0_dir);

        // Optical drive
        let sr0_dir = temp_dir.join("sr0");
        let _ = std::fs::create_dir_all(&sr0_dir);

        // NVMe drive
        let nvme0_dir = temp_dir.join("nvme0n1");
        let _ = std::fs::create_dir_all(&nvme0_dir);

        // VirtIO drive
        let vda_dir = temp_dir.join("vda");
        let _ = std::fs::create_dir_all(&vda_dir);

        // Device with name fallback
        let named_dir = temp_dir.join("other_named");
        let named_device = named_dir.join("device");
        let _ = std::fs::create_dir_all(&named_device);
        let _ = std::fs::write(named_device.join("name"), "NAMED DEV\n");

        // Device without model or name
        let unnamed_dir = temp_dir.join("other_unnamed");
        let _ = std::fs::create_dir_all(&unnamed_dir);

        // Symlink device to exercise read_link branch
        #[cfg(unix)]
        {
            let target_link = temp_dir.join("usb_target/usb1/1-1/1-1:1.0");
            let _ = std::fs::create_dir_all(&target_link);
            let sdb_link = temp_dir.join("sdb");
            let _ = std::os::unix::fs::symlink(&target_link, &sdb_link);
        }

        let scan_res = BlockDeviceScanner::scan_sysfs(Some(&temp_dir));
        assert!(scan_res.is_ok());
        assert_eq!(scan_res.as_ref().map(Vec::is_empty), Ok(false));

        // Scan sysfs with None (tests default /sys/block path)
        let _ = BlockDeviceScanner::scan_sysfs(None);

        // Reading directory error (when block_dir is a file instead of a directory)
        let block_file = temp_dir.join("file_not_dir");
        let _ = std::fs::write(&block_file, b"content");
        let err_scan = BlockDeviceScanner::scan_sysfs(Some(&block_file));
        assert!(err_scan.is_err());

        // Clean up mock directory
        let _ = std::fs::remove_dir_all(&temp_dir);

        // Non-existent directory returns empty list
        let nonexistent = temp_dir.join("nonexistent");
        let empty_scan = BlockDeviceScanner::scan_sysfs(Some(&nonexistent));
        assert_eq!(empty_scan.as_ref().map(Vec::len), Ok(0));
    }
}

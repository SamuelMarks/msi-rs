//! Automated Testing & Virtualized Emulation Harness.
//!
//! Provides in-memory mock block storage devices, mock UEFI NVRAM variable stores,
//! and automated QEMU/KVM virtual machine test runners for bare-metal OS installation validation.

use crate::error::{Error, Result};
use std::collections::BTreeMap;
use std::path::PathBuf;

/// In-memory simulated physical block storage device.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MockBlockDevice {
    /// Total virtual capacity in bytes.
    pub capacity_bytes: usize,
    /// Logical sector size (typically 512 or 4096).
    pub sector_size: u32,
    /// In-memory sector storage backing store.
    pub storage: Vec<u8>,
    /// Read-only emulation lock.
    pub read_only: bool,
}

impl MockBlockDevice {
    /// Creates a new [`MockBlockDevice`] initialized with zeroes.
    ///
    /// # Arguments
    ///
    /// * `capacity_bytes` - Virtual storage size in bytes.
    /// * `sector_size` - Sector size in bytes.
    ///
    /// # Returns
    ///
    /// Initialized in-memory mock device.
    ///
    /// # Errors
    ///
    /// Returns [`Error::BlockDeviceError`] if sector size is invalid or capacity is not sector-aligned.
    pub fn new(capacity_bytes: usize, sector_size: u32) -> Result<Self> {
        if sector_size == 0 || (sector_size != 512 && sector_size != 4096) {
            return Err(Error::BlockDeviceError {
                path: "mock://disk0".to_string(),
                reason: format!("invalid sector size {sector_size}, only 512 or 4096 supported"),
            });
        }

        let ss = sector_size as usize;
        if capacity_bytes % ss != 0 {
            return Err(Error::BlockDeviceError {
                path: "mock://disk0".to_string(),
                reason: format!(
                    "capacity ({capacity_bytes}) must be multiple of sector size ({ss})"
                ),
            });
        }

        Ok(Self {
            capacity_bytes,
            sector_size,
            storage: vec![0u8; capacity_bytes],
            read_only: false,
        })
    }

    /// Reads sectors starting at the given LBA into a newly allocated byte buffer.
    ///
    /// # Arguments
    ///
    /// * `start_lba` - 64-bit starting LBA sector index.
    /// * `sector_count` - Number of sectors to read.
    ///
    /// # Returns
    ///
    /// Read byte buffer.
    ///
    /// # Errors
    ///
    /// Returns [`Error::BlockDeviceError`] if request exceeds device capacity.
    pub fn read_sectors(&self, start_lba: u64, sector_count: u32) -> Result<Vec<u8>> {
        let ss = self.sector_size as usize;
        let Some(start_byte) = start_lba
            .checked_mul(u64::from(self.sector_size))
            .and_then(|b| usize::try_from(b).ok())
        else {
            return Err(Error::BlockDeviceError {
                path: "mock://disk0".to_string(),
                reason: "start_lba byte offset overflowed usize bounds".to_string(),
            });
        };
        let len_bytes = (sector_count as usize).saturating_mul(ss);
        let end_byte = start_byte.saturating_add(len_bytes);

        if end_byte > self.capacity_bytes {
            return Err(Error::BlockDeviceError {
                path: "mock://disk0".to_string(),
                reason: format!(
                    "read range [{}..{}] exceeds disk capacity {}",
                    start_byte, end_byte, self.capacity_bytes
                ),
            });
        }

        Ok(self.storage[start_byte..end_byte].to_vec())
    }

    /// Writes data sectors starting at the designated LBA.
    ///
    /// # Arguments
    ///
    /// * `start_lba` - Starting LBA sector index.
    /// * `data` - Byte buffer to write (must be multiple of sector size).
    ///
    /// # Errors
    ///
    /// Returns [`Error::BlockDeviceError`] if device is read-only or range exceeds capacity.
    pub fn write_sectors(&mut self, start_lba: u64, data: &[u8]) -> Result<()> {
        if self.read_only {
            return Err(Error::BlockDeviceError {
                path: "mock://disk0".to_string(),
                reason: "cannot write to read-only mock block device".to_string(),
            });
        }

        let ss = self.sector_size as usize;
        if data.len() % ss != 0 {
            return Err(Error::BlockDeviceError {
                path: "mock://disk0".to_string(),
                reason: format!(
                    "write payload len ({}) must be multiple of sector size ({ss})",
                    data.len()
                ),
            });
        }

        let Some(start_byte) = start_lba
            .checked_mul(u64::from(self.sector_size))
            .and_then(|b| usize::try_from(b).ok())
        else {
            return Err(Error::BlockDeviceError {
                path: "mock://disk0".to_string(),
                reason: "start_lba byte offset overflowed usize bounds".to_string(),
            });
        };

        let end_byte = start_byte.saturating_add(data.len());

        if end_byte > self.capacity_bytes {
            return Err(Error::BlockDeviceError {
                path: "mock://disk0".to_string(),
                reason: format!(
                    "write range [{}..{}] exceeds disk capacity {}",
                    start_byte, end_byte, self.capacity_bytes
                ),
            });
        }

        self.storage[start_byte..end_byte].copy_from_slice(data);
        Ok(())
    }

    /// Returns borrowed slice of the entire virtual disk memory buffer.
    ///
    /// # Returns
    ///
    /// Byte slice of raw virtual storage.
    #[must_use]
    pub fn as_slice(&self) -> &[u8] {
        &self.storage
    }

    /// Returns mutable slice of the virtual disk buffer.
    ///
    /// # Returns
    ///
    /// Mutable byte slice.
    pub fn as_mut_slice(&mut self) -> &mut [u8] {
        &mut self.storage
    }
}

/// In-memory mock store for simulated UEFI non-volatile variables.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct MockUefiNvram {
    /// In-memory mapping of variable keys to attributes and payloads.
    variables: BTreeMap<String, (u32, Vec<u8>)>,
}

impl MockUefiNvram {
    /// Creates a new [`MockUefiNvram`] store.
    ///
    /// # Returns
    ///
    /// Empty NVRAM variable store.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            variables: BTreeMap::new(),
        }
    }

    /// Sets or updates a UEFI variable.
    ///
    /// # Arguments
    ///
    /// * `guid` - Vendor/standard UUID string.
    /// * `name` - Variable name (e.g. `BootOrder`, `Boot0001`).
    /// * `attributes` - Attribute bitmask flags.
    /// * `data` - Variable payload bytes.
    pub fn set_variable(&mut self, guid: &str, name: &str, attributes: u32, data: &[u8]) {
        let key = format!("{name}-{guid}");
        self.variables.insert(key, (attributes, data.to_vec()));
    }

    /// Retrieves a variable by GUID and name.
    ///
    /// # Arguments
    ///
    /// * `guid` - UUID string.
    /// * `name` - Variable name.
    ///
    /// # Returns
    ///
    /// Optional tuple of `(attributes, payload_bytes)`.
    #[must_use]
    pub fn get_variable(&self, guid: &str, name: &str) -> Option<(u32, Vec<u8>)> {
        let key = format!("{name}-{guid}");
        self.variables.get(&key).cloned()
    }
}

/// Automated QEMU/KVM virtual machine test runner.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QemuTestRunner {
    /// Path to QEMU executable binary (e.g. `qemu-system-x86_64`).
    pub qemu_binary: String,
    /// Path to OVMF UEFI firmware image (e.g. `OVMF_CODE.fd`).
    pub ovmf_code: PathBuf,
    /// Target raw virtual disk image path.
    pub disk_image: PathBuf,
    /// Optional bootable ISO image path.
    pub iso_image: Option<PathBuf>,
    /// VM memory in megabytes (typically 2048).
    pub memory_mb: u32,
    /// Symmetrical Multi-Processing (SMP) CPU core count.
    pub smp_cores: u32,
}

impl Default for QemuTestRunner {
    fn default() -> Self {
        Self {
            qemu_binary: "qemu-system-x86_64".to_string(),
            ovmf_code: PathBuf::from("/usr/share/OVMF/OVMF_CODE.fd"),
            disk_image: PathBuf::from("target_disk.img"),
            iso_image: None,
            memory_mb: 2048,
            smp_cores: 2,
        }
    }
}

impl QemuTestRunner {
    /// Creates a new [`QemuTestRunner`] configuration.
    ///
    /// # Arguments
    ///
    /// * `ovmf_code` - OVMF UEFI code firmware path.
    /// * `disk_image` - Target virtual disk path.
    ///
    /// # Returns
    ///
    /// Configured [`QemuTestRunner`].
    #[must_use]
    pub fn new(ovmf_code: impl Into<PathBuf>, disk_image: impl Into<PathBuf>) -> Self {
        Self {
            qemu_binary: "qemu-system-x86_64".to_string(),
            ovmf_code: ovmf_code.into(),
            disk_image: disk_image.into(),
            iso_image: None,
            memory_mb: 2048,
            smp_cores: 2,
        }
    }

    /// Assembles the complete QEMU launch command-line argument vector.
    ///
    /// # Returns
    ///
    /// Vector of string arguments for spawning QEMU.
    #[must_use]
    pub fn build_command_args(&self) -> Vec<String> {
        let mut args = vec![
            self.qemu_binary.clone(),
            "-nodefaults".to_string(),
            "-m".to_string(),
            self.memory_mb.to_string(),
            "-smp".to_string(),
            self.smp_cores.to_string(),
            "-bios".to_string(),
            self.ovmf_code.display().to_string(),
            "-drive".to_string(),
            format!("file={},format=raw,if=virtio", self.disk_image.display()),
            "-serial".to_string(),
            "stdio".to_string(),
            "-display".to_string(),
            "none".to_string(),
        ];

        if let Some(ref iso) = self.iso_image {
            args.push("-cdrom".to_string());
            args.push(iso.display().to_string());
            args.push("-boot".to_string());
            args.push("d".to_string());
        }

        args
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::platform::format::{
        Fat32FormatOptions, Fat32Formatter, FileSystemKind, FileSystemVerifier,
    };
    use crate::platform::partition::{
        GptPartitionEntry, GptTable, Lba, PartitionTypeGuid, PartitionUuid,
    };

    /// Tests `MockBlockDevice` read and write operations.
    #[test]
    fn test_mock_block_device_read_write() -> Result<()> {
        let mut mock = MockBlockDevice::new(1024 * 1024, 512)?;

        assert_eq!(mock.capacity_bytes, 1024 * 1024);
        assert_eq!(mock.sector_size, 512);

        let data = vec![0xAB; 512];
        mock.write_sectors(10, &data)?;

        let read = mock.read_sectors(10, 1)?;
        assert_eq!(read, data);

        // Test invalid alignment/capacity/bounds
        assert!(MockBlockDevice::new(1000, 512).is_err());
        assert!(MockBlockDevice::new(1024, 100).is_err());
        assert!(MockBlockDevice::new(1024, 0).is_err());
        assert!(MockBlockDevice::new(4096, 4096).is_ok());
        assert!(mock.read_sectors(10000, 1).is_err());
        assert!(mock.write_sectors(10000, &data).is_err());
        assert!(mock.write_sectors(0, &[1, 2, 3]).is_err()); // non-sector aligned data

        // Test slice accessors
        assert_eq!(mock.as_slice().len(), 1024 * 1024);
        let mut_slice = mock.as_mut_slice();
        mut_slice[0] = 0x42;
        assert_eq!(mock.as_slice()[0], 0x42);

        // Test usize overflow error branches if start_lba is huge
        let huge_lba = u64::MAX;
        assert!(mock.read_sectors(huge_lba, 1).is_err());
        assert!(mock.write_sectors(huge_lba, &data).is_err());

        // Test len_bytes overflow error branch
        assert!(mock.read_sectors(0, u32::MAX).is_err());

        mock.read_only = true;
        assert!(mock.write_sectors(0, &data).is_err());

        Ok(())
    }

    /// Tests mock block device partitioning and filesystem formatting integration.
    #[test]
    fn test_mock_block_device_partition_and_format() -> Result<()> {
        let total_sectors_u64 = 10_000u64;
        let capacity = 10_000 * 512;
        let mut mock = MockBlockDevice::new(capacity, 512)?;

        let mut gpt = GptTable::new(total_sectors_u64, 512, [0x11; 16])?;

        let esp_part = GptPartitionEntry {
            type_guid: PartitionTypeGuid::ESP,
            unique_guid: PartitionUuid([1; 16]),
            start_lba: Lba(2048),
            end_lba: Lba(4095),
            attributes: 0,
            name: "ESP".to_string(),
        };
        gpt.add_partition(esp_part)?;

        let mbr_bytes = gpt.serialize_protective_mbr(512);
        mock.write_sectors(0, &mbr_bytes)?;

        let hdr_bytes = gpt.serialize_header(false, 512);
        mock.write_sectors(1, &hdr_bytes)?;

        let entries_bytes = gpt.serialize_partition_entries();
        mock.write_sectors(2, &entries_bytes[0..512])?;

        let read_mbr = mock.read_sectors(0, 1)?;
        assert_eq!(read_mbr[450], 0xEE);
        assert_eq!(read_mbr[510], 0x55);
        assert_eq!(read_mbr[511], 0xAA);

        let esp_opt = Fat32FormatOptions::default();
        let fs_bytes = Fat32Formatter::format_filesystem(1_000_000, 512, &esp_opt)?;
        assert!(FileSystemVerifier::verify(FileSystemKind::Fat32, &fs_bytes).is_ok());

        Ok(())
    }

    /// Tests `MockUefiNvram` variable getter and setter.
    #[test]
    fn test_mock_uefi_nvram() {
        let mut nvram = MockUefiNvram::new();
        let guid = "8be4df61-93ca-11d2-aa0d-00e098032b8c";
        nvram.set_variable(guid, "BootOrder", 7, &[1, 0, 0, 0]);

        let var = nvram.get_variable(guid, "BootOrder");
        assert_eq!(var, Some((7, vec![1, 0, 0, 0])));

        assert_eq!(nvram.get_variable(guid, "NonExistent"), None);
    }

    /// Tests `QemuTestRunner` argument assembly.
    #[test]
    fn test_qemu_test_runner_args() {
        let runner = QemuTestRunner {
            qemu_binary: "qemu-system-x86_64".to_string(),
            ovmf_code: PathBuf::from("/usr/share/ovmf/OVMF.fd"),
            disk_image: PathBuf::from("/tmp/target.img"),
            iso_image: Some(PathBuf::from("/tmp/install.iso")),
            memory_mb: 4096,
            smp_cores: 4,
        };

        let args = runner.build_command_args();
        assert!(args.contains(&"-m".to_string()));
        assert!(args.contains(&"4096".to_string()));
        assert!(args.contains(&"-smp".to_string()));
        assert!(args.contains(&"4".to_string()));
        assert!(args.contains(&"-cdrom".to_string()));
        assert!(args.contains(&"/tmp/install.iso".to_string()));
        assert!(args.contains(&"-drive".to_string()));
        assert!(args.contains(&"file=/tmp/target.img,format=raw,if=virtio".to_string()));

        let runner_no_iso = QemuTestRunner::new("/custom/ovmf.fd", "/custom/disk.img");
        assert_eq!(runner_no_iso.ovmf_code, PathBuf::from("/custom/ovmf.fd"));
        assert_eq!(runner_no_iso.disk_image, PathBuf::from("/custom/disk.img"));
        assert!(runner_no_iso.iso_image.is_none());
        let args_no_iso = runner_no_iso.build_command_args();
        assert!(!args_no_iso.contains(&"-cdrom".to_string()));

        let default_runner = QemuTestRunner::default();
        assert_eq!(default_runner.memory_mb, 2048);
        assert_eq!(default_runner.smp_cores, 2);
    }
}

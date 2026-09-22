//! GUID Partition Table (GPT) & Master Boot Record (MBR) Engine.
//!
//! Grounded directly in UEFI Specification Chapter 5 and legacy IBM PC MBR architecture:
//! - Full GPT header generation with CRC32 verification.
//! - Canonical partition type GUIDs for Windows and Linux OS installations.
//! - Automated standard partition scheme synthesis (UEFI Windows, UEFI Linux, BIOS fallback).
//! - Custom user partition layout sizing and validation.

use crate::error::{Error, Result};

/// Logical Block Address (LBA) representing a 64-bit sector index.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct Lba(pub u64);

impl std::fmt::Display for Lba {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// 128-bit Partition Unique GUID.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct PartitionUuid(pub [u8; 16]);

impl std::fmt::Display for PartitionUuid {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for b in self.0 {
            write!(f, "{b:02X}")?;
        }
        Ok(())
    }
}

/// 128-bit Partition Type GUID identifying the role and operating system compatibility.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PartitionTypeGuid(pub [u8; 16]);

impl PartitionTypeGuid {
    /// EFI System Partition (ESP) GUID: `C12A7328-F81F-11D2-BA4B-00A0C93EC93B`
    pub const ESP: Self = Self([
        0x28, 0x73, 0x2A, 0xC1, 0x1F, 0xF8, 0xD2, 0x11, 0xBA, 0x4B, 0x00, 0xA0, 0xC9, 0x3E, 0xC9,
        0x3B,
    ]);

    /// Microsoft Reserved (MSR) Partition GUID: `E3C9E310-0B5B-4DB8-817D-F92DF00215AE`
    pub const MSR: Self = Self([
        0x10, 0xE3, 0xC9, 0xE3, 0x5B, 0x0B, 0xB8, 0x4D, 0x81, 0x7D, 0xF9, 0x2D, 0xF0, 0x02, 0x15,
        0xAE,
    ]);

    /// Windows Basic Data (OS / NTFS / `ReFS`) Partition GUID: `EBD0A0A2-B9E5-4433-87C0-68B6B72699C7`
    pub const WINDOWS_BASIC_DATA: Self = Self([
        0xA2, 0xA0, 0xD0, 0xEB, 0xE5, 0xB9, 0x33, 0x44, 0x87, 0xC0, 0x68, 0xB6, 0xB7, 0x26, 0x99,
        0xC7,
    ]);

    /// Windows Recovery Environment Partition GUID: `DE94BBA4-06D1-4D40-A16A-BFD50179D6AC`
    pub const WINDOWS_RECOVERY: Self = Self([
        0xA4, 0xBB, 0x94, 0xDE, 0xD1, 0x06, 0x40, 0x4D, 0xA1, 0x6A, 0xBF, 0xD5, 0x01, 0x79, 0xD6,
        0xAC,
    ]);

    /// Linux `x86_64` Root Partition (`/`) GUID: `4F68BCE3-E8CD-4DB1-96E7-FBCAF984B709`
    pub const LINUX_ROOT_X86_64: Self = Self([
        0xE3, 0xBC, 0x68, 0x4F, 0xCD, 0xE8, 0xB1, 0x4D, 0x96, 0xE7, 0xFB, 0xCA, 0xF9, 0x84, 0xB7,
        0x09,
    ]);

    /// Linux `/boot` Partition GUID: `BC13C260-BF3B-4440-B142-EA907D213AE4`
    pub const LINUX_BOOT: Self = Self([
        0x60, 0xC2, 0x13, 0xBC, 0x3B, 0xBF, 0x40, 0x44, 0xB1, 0x42, 0xEA, 0x90, 0x7D, 0x21, 0x3A,
        0xE4,
    ]);

    /// Linux Swap Partition GUID: `0657FD6D-A4AB-43C4-84E5-0933C84B4F4F`
    pub const LINUX_SWAP: Self = Self([
        0x6D, 0xFD, 0x57, 0x06, 0xAB, 0xA4, 0xC4, 0x43, 0x84, 0xE5, 0x09, 0x33, 0xC8, 0x4B, 0x4F,
        0x4F,
    ]);
}

/// A single entry in a GUID Partition Table (128 bytes).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GptPartitionEntry {
    /// Identifies the purpose and partition type.
    pub type_guid: PartitionTypeGuid,
    /// Unique GUID assigned to this partition instance.
    pub unique_guid: PartitionUuid,
    /// Starting Logical Block Address.
    pub start_lba: Lba,
    /// Ending Logical Block Address (inclusive).
    pub end_lba: Lba,
    /// Attribute flags (e.g. read-only, system partition).
    pub attributes: u64,
    /// Human-readable partition name (up to 36 UTF-16 code units).
    pub name: String,
}

impl GptPartitionEntry {
    /// Returns the total number of sectors allocated to this partition.
    ///
    /// # Returns
    ///
    /// Number of sectors (`end_lba` - `start_lba` + 1).
    #[must_use]
    pub const fn sector_count(&self) -> u64 {
        if self.end_lba.0 >= self.start_lba.0 {
            self.end_lba.0 - self.start_lba.0 + 1
        } else {
            0
        }
    }

    /// Serializes this partition entry to the canonical 128-byte GPT structure.
    ///
    /// # Returns
    ///
    /// 128-byte array.
    #[must_use]
    pub fn serialize(&self) -> [u8; 128] {
        let mut buf = [0u8; 128];
        buf[0..16].copy_from_slice(&self.type_guid.0);
        buf[16..32].copy_from_slice(&self.unique_guid.0);
        buf[32..40].copy_from_slice(&self.start_lba.0.to_le_bytes());
        buf[40..48].copy_from_slice(&self.end_lba.0.to_le_bytes());
        buf[48..56].copy_from_slice(&self.attributes.to_le_bytes());

        // UTF-16LE partition name (bytes 56..128)
        let utf16: Vec<u16> = self.name.encode_utf16().take(36).collect();
        for (i, unit) in utf16.iter().enumerate() {
            let offset = 56 + (i * 2);
            let bytes = unit.to_le_bytes();
            buf[offset] = bytes[0];
            buf[offset + 1] = bytes[1];
        }

        buf
    }
}

/// Standard Master Boot Record (MBR) 16-byte partition entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MbrPartitionEntry {
    /// Whether this partition is marked active/bootable (0x80) or inactive (0x00).
    pub bootable: bool,
    /// MBR filesystem/partition type identifier (e.g. 0x07 for NTFS/exFAT, 0x83 for Linux, 0xEF for ESP).
    pub partition_type: u8,
    /// Starting LBA sector index.
    pub start_lba: u32,
    /// Number of sectors in partition.
    pub sector_count: u32,
}

impl MbrPartitionEntry {
    /// Serializes the partition entry to 16 bytes.
    ///
    /// # Returns
    ///
    /// 16-byte array.
    #[must_use]
    pub const fn serialize(self) -> [u8; 16] {
        let mut buf = [0u8; 16];
        buf[0] = if self.bootable { 0x80 } else { 0x00 };
        // CHS start (dummy 0x000200 for LBA)
        buf[1] = 0x00;
        buf[2] = 0x02;
        buf[3] = 0x00;
        buf[4] = self.partition_type;
        // CHS end (dummy 0xFFFFFF for LBA)
        buf[5] = 0xFF;
        buf[6] = 0xFF;
        buf[7] = 0xFF;

        let start_bytes = self.start_lba.to_le_bytes();
        buf[8] = start_bytes[0];
        buf[9] = start_bytes[1];
        buf[10] = start_bytes[2];
        buf[11] = start_bytes[3];

        let count_bytes = self.sector_count.to_le_bytes();
        buf[12] = count_bytes[0];
        buf[13] = count_bytes[1];
        buf[14] = count_bytes[2];
        buf[15] = count_bytes[3];

        buf
    }
}

/// Legacy Master Boot Record (MBR) table (512 bytes).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MbrTable {
    /// Windows/DOS 32-bit disk signature at offset 440.
    pub disk_signature: u32,
    /// Up to 4 primary partition entries.
    pub partitions: [Option<MbrPartitionEntry>; 4],
}

impl Default for MbrTable {
    fn default() -> Self {
        Self {
            disk_signature: 0x1234_5678,
            partitions: [None, None, None, None],
        }
    }
}

impl MbrTable {
    /// Creates a new [`MbrTable`].
    ///
    /// # Arguments
    ///
    /// * `disk_signature` - 32-bit unique disk identifier.
    ///
    /// # Returns
    ///
    /// A new [`MbrTable`] instance.
    #[must_use]
    pub const fn new(disk_signature: u32) -> Self {
        Self {
            disk_signature,
            partitions: [None, None, None, None],
        }
    }

    /// Serializes the MBR sector (512 bytes) with 0x55AA boot signature.
    ///
    /// # Returns
    ///
    /// 512-byte array.
    #[must_use]
    pub fn serialize(&self) -> [u8; 512] {
        let mut buf = [0u8; 512];
        // Disk signature at offset 440
        buf[440..444].copy_from_slice(&self.disk_signature.to_le_bytes());

        // Partitions at offsets 446, 462, 478, 494
        for (i, entry_opt) in self.partitions.iter().enumerate() {
            if let Some(entry) = entry_opt {
                let offset = 446 + (i * 16);
                buf[offset..offset + 16].copy_from_slice(&entry.serialize());
            }
        }

        // MBR signature
        buf[510] = 0x55;
        buf[511] = 0xAA;
        buf
    }
}

/// Full GUID Partition Table manager.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GptTable {
    /// 128-bit unique disk GUID.
    pub disk_guid: [u8; 16],
    /// Primary header LBA (typically 1).
    pub header_lba: Lba,
    /// Backup header LBA (last sector of drive).
    pub backup_header_lba: Lba,
    /// First usable LBA for partition payload data (typically 34 or 2048 for 1MB alignment).
    pub first_usable_lba: Lba,
    /// Last usable LBA for partition payload data.
    pub last_usable_lba: Lba,
    /// Sector location of the primary partition entry array (typically LBA 2).
    pub partition_entries_lba: Lba,
    /// List of configured partition entries.
    pub partitions: Vec<GptPartitionEntry>,
}

impl GptTable {
    /// Creates a new, unpartitioned [`GptTable`] initialized for a given drive geometry.
    ///
    /// # Arguments
    ///
    /// * `total_sectors` - Total logical sector count of target disk.
    /// * `sector_size` - Sector size in bytes (typically 512 or 4096).
    /// * `disk_guid` - 16-byte unique disk identifier.
    ///
    /// # Returns
    ///
    /// Initialized [`GptTable`].
    ///
    /// # Errors
    ///
    /// Returns [`Error::PartitionError`] if `total_sectors` is insufficient (< 68 sectors).
    pub fn new(total_sectors: u64, sector_size: u32, disk_guid: [u8; 16]) -> Result<Self> {
        let min_sectors = 68;
        if total_sectors < min_sectors {
            return Err(Error::PartitionError {
                reason: format!("total sectors ({total_sectors}) too small for GPT layout (minimum {min_sectors})"),
            });
        }

        let partition_entry_sectors = (128 * 128) / u64::from(sector_size);
        let first_usable = 1 + 1 + partition_entry_sectors; // MBR + Header + Entry Array
        let last_usable = total_sectors.saturating_sub(1 + partition_entry_sectors + 1);
        let first_usable_aligned = first_usable.max(2048);

        if last_usable <= first_usable_aligned {
            return Err(Error::PartitionError {
                reason: format!(
                    "total sectors ({total_sectors}) too small for usable partition space"
                ),
            });
        }

        Ok(Self {
            disk_guid,
            header_lba: Lba(1),
            backup_header_lba: Lba(total_sectors - 1),
            first_usable_lba: Lba(first_usable_aligned), // Standard 1MiB alignment
            last_usable_lba: Lba(last_usable),
            partition_entries_lba: Lba(2),
            partitions: Vec::new(),
        })
    }

    /// Adds a partition entry to the table, validating non-overlapping boundaries.
    ///
    /// # Arguments
    ///
    /// * `entry` - Partition entry to insert.
    ///
    /// # Errors
    ///
    /// Returns [`Error::PartitionError`] if boundaries exceed disk bounds or overlap existing partitions.
    pub fn add_partition(&mut self, entry: GptPartitionEntry) -> Result<()> {
        if entry.start_lba.0 < self.first_usable_lba.0 {
            return Err(Error::PartitionError {
                reason: format!(
                    "partition start LBA {} is before first usable LBA {}",
                    entry.start_lba.0, self.first_usable_lba.0
                ),
            });
        }
        if entry.end_lba.0 > self.last_usable_lba.0 {
            return Err(Error::PartitionError {
                reason: format!(
                    "partition end LBA {} exceeds last usable LBA {}",
                    entry.end_lba.0, self.last_usable_lba.0
                ),
            });
        }
        if entry.end_lba.0 < entry.start_lba.0 {
            return Err(Error::PartitionError {
                reason: format!(
                    "partition end LBA {} is less than start LBA {}",
                    entry.end_lba.0, entry.start_lba.0
                ),
            });
        }

        for existing in &self.partitions {
            if !(entry.end_lba.0 < existing.start_lba.0 || entry.start_lba.0 > existing.end_lba.0) {
                return Err(Error::PartitionError {
                    reason: format!(
                        "partition range {}..={} overlaps existing partition {}..={}",
                        entry.start_lba.0,
                        entry.end_lba.0,
                        existing.start_lba.0,
                        existing.end_lba.0
                    ),
                });
            }
        }

        self.partitions.push(entry);
        self.partitions.sort_by_key(|p| p.start_lba.0);
        Ok(())
    }

    /// Serializes the standard Protective MBR (LBA 0).
    ///
    /// # Arguments
    ///
    /// * `sector_size` - Sector size in bytes.
    ///
    /// # Returns
    ///
    /// Protective MBR byte buffer.
    #[must_use]
    pub fn serialize_protective_mbr(&self, sector_size: u32) -> Vec<u8> {
        let mut mbr = vec![0u8; sector_size as usize];
        // Partition 1 in MBR at offset 446
        mbr[446] = 0x00; // Inactive
        mbr[447] = 0x00; // CHS start
        mbr[448] = 0x02;
        mbr[449] = 0x00;
        mbr[450] = 0xEE; // Type: GPT Protective MBR
        mbr[451] = 0xFF; // CHS end
        mbr[452] = 0xFF;
        mbr[453] = 0xFF;

        // Starting LBA: 1
        mbr[454..458].copy_from_slice(&1u32.to_le_bytes());
        // Sector count: size - 1, or 0xFFFFFFFF if exceeds 32 bits
        let count = u32::try_from(self.backup_header_lba.0).unwrap_or(u32::MAX);
        mbr[458..462].copy_from_slice(&count.to_le_bytes());

        // MBR signature
        mbr[510] = 0x55;
        mbr[511] = 0xAA;
        mbr
    }

    /// Serializes the 92-byte GPT header into a sector-sized byte buffer.
    ///
    /// # Arguments
    ///
    /// * `is_backup` - True if generating the backup header at drive end.
    /// * `sector_size` - Sector size in bytes.
    ///
    /// # Returns
    ///
    /// Serialized header sector buffer with computed CRC32 checksums.
    #[must_use]
    pub fn serialize_header(&self, is_backup: bool, sector_size: u32) -> Vec<u8> {
        let mut header = vec![0u8; sector_size as usize];
        // Signature: "EFI PART" (8 bytes)
        header[0..8].copy_from_slice(b"EFI PART");
        // Revision: 0x00010000 (1.0)
        header[8..12].copy_from_slice(&0x0001_0000u32.to_le_bytes());
        // Header size: 92 bytes
        header[12..16].copy_from_slice(&92u32.to_le_bytes());

        let current_lba = if is_backup {
            self.backup_header_lba.0
        } else {
            self.header_lba.0
        };
        let backup_lba = if is_backup {
            self.header_lba.0
        } else {
            self.backup_header_lba.0
        };

        header[24..32].copy_from_slice(&current_lba.to_le_bytes());
        header[32..40].copy_from_slice(&backup_lba.to_le_bytes());
        header[40..48].copy_from_slice(&self.first_usable_lba.0.to_le_bytes());
        header[48..56].copy_from_slice(&self.last_usable_lba.0.to_le_bytes());
        header[56..72].copy_from_slice(&self.disk_guid);

        let entries_lba = if is_backup {
            let partition_entry_sectors = (128 * 128) / u64::from(sector_size);
            self.backup_header_lba
                .0
                .saturating_sub(partition_entry_sectors)
        } else {
            self.partition_entries_lba.0
        };
        header[72..80].copy_from_slice(&entries_lba.to_le_bytes());

        // Number of partition entries: 128
        header[80..84].copy_from_slice(&128u32.to_le_bytes());
        // Size of partition entry: 128 bytes
        header[84..88].copy_from_slice(&128u32.to_le_bytes());

        // Partition entries CRC32
        let entries_buf = self.serialize_partition_entries();
        let entries_crc = compute_crc32(&entries_buf);
        header[88..92].copy_from_slice(&entries_crc.to_le_bytes());

        // Header CRC32 (with CRC field zeroed)
        let header_crc = compute_crc32(&header[0..92]);
        header[16..20].copy_from_slice(&header_crc.to_le_bytes());

        header
    }

    /// Serializes the partition entry array (128 entries * 128 bytes = 16,384 bytes).
    ///
    /// # Returns
    ///
    /// 16 KiB byte buffer of serialized entries.
    #[must_use]
    pub fn serialize_partition_entries(&self) -> Vec<u8> {
        let mut buf = vec![0u8; 128 * 128];
        for (i, part) in self.partitions.iter().take(128).enumerate() {
            let offset = i * 128;
            buf[offset..offset + 128].copy_from_slice(&part.serialize());
        }
        buf
    }
}

/// Standardized operating system partition layout blueprints.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StandardPartitionScheme {
    /// UEFI Windows Standard Layout:
    /// 1. EFI System Partition (ESP) - default 260 MB
    /// 2. Microsoft Reserved (MSR) - 16 MB
    /// 3. Windows OS Partition - Remaining space
    /// 4. Windows Recovery Partition - default 1000 MB
    UefiWindows {
        /// ESP size in megabytes (typically 100 to 512).
        esp_mb: u64,
        /// MSR size in megabytes (typically 16).
        msr_mb: u64,
        /// Windows RE recovery size in megabytes (typically 500 to 1000).
        recovery_mb: u64,
    },
    /// UEFI Linux Standard Layout:
    /// 1. EFI System Partition (ESP) - default 512 MB
    /// 2. Boot Partition (`/boot`) - default 1024 MB
    /// 3. Swap Partition - default 4096 MB
    /// 4. Root Partition (`/`) - Remaining space
    UefiLinux {
        /// ESP size in megabytes.
        esp_mb: u64,
        /// Boot partition size in megabytes.
        boot_mb: u64,
        /// Swap partition size in megabytes.
        swap_mb: u64,
    },
}

impl StandardPartitionScheme {
    /// Synthesizes a ready-to-write [`GptTable`] configured according to this blueprint.
    ///
    /// # Arguments
    ///
    /// * `total_sectors` - Target disk capacity in sectors.
    /// * `sector_size` - Sector size in bytes.
    /// * `disk_guid` - 16-byte disk GUID.
    ///
    /// # Returns
    ///
    /// Configured [`GptTable`].
    ///
    /// # Errors
    ///
    /// Returns [`Error::PartitionError`] if disk capacity is insufficient for standard partitions.
    pub fn provision(
        &self,
        total_sectors: u64,
        sector_size: u32,
        disk_guid: [u8; 16],
    ) -> Result<GptTable> {
        let mut table = GptTable::new(total_sectors, sector_size, disk_guid)?;
        let sectors_per_mb = (1024 * 1024) / u64::from(sector_size);

        match *self {
            Self::UefiWindows {
                esp_mb,
                msr_mb,
                recovery_mb,
            } => {
                Self::provision_windows(&mut table, sectors_per_mb, esp_mb, msr_mb, recovery_mb)?;
            }
            Self::UefiLinux {
                esp_mb,
                boot_mb,
                swap_mb,
            } => {
                Self::provision_linux(&mut table, sectors_per_mb, esp_mb, boot_mb, swap_mb)?;
            }
        }

        Ok(table)
    }

    /// Provisions Windows partitions (ESP, MSR, OS, Recovery).
    fn provision_windows(
        table: &mut GptTable,
        sectors_per_mb: u64,
        esp_mb: u64,
        msr_mb: u64,
        recovery_mb: u64,
    ) -> Result<()> {
        let esp_sectors = esp_mb.saturating_mul(sectors_per_mb);
        let msr_sectors = msr_mb.saturating_mul(sectors_per_mb);
        let recovery_sectors = recovery_mb.saturating_mul(sectors_per_mb);

        let total_required = esp_sectors + msr_sectors + recovery_sectors + (1024 * sectors_per_mb);
        if table
            .last_usable_lba
            .0
            .saturating_sub(table.first_usable_lba.0)
            < total_required
        {
            return Err(Error::PartitionError {
                reason: "disk capacity too small for standard Windows UEFI layout".to_string(),
            });
        }

        // 1. ESP
        let esp_start = table.first_usable_lba;
        let esp_end = Lba(esp_start.0 + esp_sectors - 1);
        table.add_partition(GptPartitionEntry {
            type_guid: PartitionTypeGuid::ESP,
            unique_guid: PartitionUuid([1; 16]),
            start_lba: esp_start,
            end_lba: esp_end,
            attributes: 0,
            name: "EFI system partition".to_string(),
        })?;

        // 2. MSR
        let msr_start = Lba(esp_end.0 + 1);
        let msr_end = Lba(msr_start.0 + msr_sectors - 1);
        table.add_partition(GptPartitionEntry {
            type_guid: PartitionTypeGuid::MSR,
            unique_guid: PartitionUuid([2; 16]),
            start_lba: msr_start,
            end_lba: msr_end,
            attributes: 0,
            name: "Microsoft reserved partition".to_string(),
        })?;

        // 4. Recovery at end of disk
        let rec_end = table.last_usable_lba;
        let rec_start = Lba(rec_end.0.saturating_sub(recovery_sectors) + 1);

        // 3. OS Partition in remaining space
        let os_start = Lba(msr_end.0 + 1);
        let os_end = Lba(rec_start.0.saturating_sub(1));
        table.add_partition(GptPartitionEntry {
            type_guid: PartitionTypeGuid::WINDOWS_BASIC_DATA,
            unique_guid: PartitionUuid([3; 16]),
            start_lba: os_start,
            end_lba: os_end,
            attributes: 0,
            name: "Basic data partition".to_string(),
        })?;

        table.add_partition(GptPartitionEntry {
            type_guid: PartitionTypeGuid::WINDOWS_RECOVERY,
            unique_guid: PartitionUuid([4; 16]),
            start_lba: rec_start,
            end_lba: rec_end,
            attributes: 0x8000_0000_0000_0001,
            name: "Recovery".to_string(),
        })?;
        Ok(())
    }

    /// Provisions Linux partitions (ESP, Boot, Swap, Root).
    fn provision_linux(
        table: &mut GptTable,
        sectors_per_mb: u64,
        esp_mb: u64,
        boot_mb: u64,
        swap_mb: u64,
    ) -> Result<()> {
        let esp_sectors = esp_mb.saturating_mul(sectors_per_mb);
        let boot_sectors = boot_mb.saturating_mul(sectors_per_mb);
        let swap_sectors = swap_mb.saturating_mul(sectors_per_mb);

        let total_required = esp_sectors + boot_sectors + swap_sectors + (1024 * sectors_per_mb);
        if table
            .last_usable_lba
            .0
            .saturating_sub(table.first_usable_lba.0)
            < total_required
        {
            return Err(Error::PartitionError {
                reason: "disk capacity too small for standard Linux UEFI layout".to_string(),
            });
        }

        // 1. ESP
        let esp_start = table.first_usable_lba;
        let esp_end = Lba(esp_start.0 + esp_sectors - 1);
        table.add_partition(GptPartitionEntry {
            type_guid: PartitionTypeGuid::ESP,
            unique_guid: PartitionUuid([1; 16]),
            start_lba: esp_start,
            end_lba: esp_end,
            attributes: 0,
            name: "EFI System Partition".to_string(),
        })?;

        // 2. Boot
        let boot_start = Lba(esp_end.0 + 1);
        let boot_end = Lba(boot_start.0 + boot_sectors - 1);
        table.add_partition(GptPartitionEntry {
            type_guid: PartitionTypeGuid::LINUX_BOOT,
            unique_guid: PartitionUuid([2; 16]),
            start_lba: boot_start,
            end_lba: boot_end,
            attributes: 0,
            name: "Boot Partition".to_string(),
        })?;

        // 3. Swap
        let swap_start = Lba(boot_end.0 + 1);
        let swap_end = Lba(swap_start.0 + swap_sectors - 1);
        table.add_partition(GptPartitionEntry {
            type_guid: PartitionTypeGuid::LINUX_SWAP,
            unique_guid: PartitionUuid([3; 16]),
            start_lba: swap_start,
            end_lba: swap_end,
            attributes: 0,
            name: "Swap Partition".to_string(),
        })?;

        // 4. Root
        let root_start = Lba(swap_end.0 + 1);
        let root_end = table.last_usable_lba;
        table.add_partition(GptPartitionEntry {
            type_guid: PartitionTypeGuid::LINUX_ROOT_X86_64,
            unique_guid: PartitionUuid([4; 16]),
            start_lba: root_start,
            end_lba: root_end,
            attributes: 0,
            name: "Linux Root".to_string(),
        })?;
        Ok(())
    }
}

/// Standard IEEE 802.3 CRC32 checksum calculator for GPT header verification.
#[must_use]
pub fn compute_crc32(bytes: &[u8]) -> u32 {
    let mut crc: u32 = 0xFFFF_FFFF;
    for &b in bytes {
        crc ^= u32::from(b);
        for _ in 0..8 {
            if crc & 1 != 0 {
                crc = (crc >> 1) ^ 0xEDB8_8320;
            } else {
                crc >>= 1;
            }
        }
    }
    !crc
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Tests `Lba` and `PartitionUuid` formatting.
    #[test]
    fn test_types_formatting() {
        let lba = Lba(2048);
        assert_eq!(format!("{lba}"), "2048");

        let uuid = PartitionUuid([0x12; 16]);
        assert!(format!("{uuid}").starts_with("1212"));
    }

    /// Tests `GptPartitionEntry` serialization and sector calculation.
    #[test]
    fn test_gpt_partition_entry() {
        let entry = GptPartitionEntry {
            type_guid: PartitionTypeGuid::ESP,
            unique_guid: PartitionUuid([0xAA; 16]),
            start_lba: Lba(2048),
            end_lba: Lba(4095),
            attributes: 0,
            name: "EFI".to_string(),
        };

        assert_eq!(entry.sector_count(), 2048);
        let serialized = entry.serialize();
        assert_eq!(&serialized[0..16], &PartitionTypeGuid::ESP.0);
        assert_eq!(&serialized[16..32], &[0xAA; 16]);
        assert_eq!(&serialized[32..40], &2048u64.to_le_bytes());
        assert_eq!(&serialized[40..48], &4095u64.to_le_bytes());

        let invalid_entry = GptPartitionEntry {
            start_lba: Lba(5000),
            end_lba: Lba(1000),
            ..entry
        };
        assert_eq!(invalid_entry.sector_count(), 0);
    }

    /// Tests `MbrTable` serialization with active and inactive partitions.
    #[test]
    fn test_mbr_table() {
        let mut mbr = MbrTable::new(0xDEAD_BEEF);
        mbr.partitions[0] = Some(MbrPartitionEntry {
            bootable: true,
            partition_type: 0x07,
            start_lba: 2048,
            sector_count: 204_800,
        });

        let buf = mbr.serialize();
        assert_eq!(&buf[440..444], &0xDEAD_BEEFu32.to_le_bytes());
        assert_eq!(buf[446], 0x80); // Bootable
        assert_eq!(buf[450], 0x07); // Type
        assert_eq!(buf[510], 0x55);
        assert_eq!(buf[511], 0xAA);

        // Non-bootable MbrPartitionEntry serialization
        let nonboot = MbrPartitionEntry {
            bootable: false,
            partition_type: 0x83,
            start_lba: 2048,
            sector_count: 1000,
        };
        assert_eq!(nonboot.serialize()[0], 0x00);

        let default_mbr = MbrTable::default();
        assert_eq!(default_mbr.partitions[0], None);
    }

    /// Tests `GptTable` creation, partition addition, and overlap rejection.
    #[test]
    fn test_gpt_table_operations() {
        let mut table = GptTable::default_mock();

        let part1 = GptPartitionEntry {
            type_guid: PartitionTypeGuid::ESP,
            unique_guid: PartitionUuid([1; 16]),
            start_lba: Lba(2048),
            end_lba: Lba(4095),
            attributes: 0,
            name: "ESP".to_string(),
        };
        assert!(table.add_partition(part1).is_ok());

        // Overlapping partition must fail
        let part_overlap = GptPartitionEntry {
            type_guid: PartitionTypeGuid::WINDOWS_BASIC_DATA,
            unique_guid: PartitionUuid([2; 16]),
            start_lba: Lba(3000),
            end_lba: Lba(5000),
            attributes: 0,
            name: "Overlap".to_string(),
        };
        assert!(table.add_partition(part_overlap).is_err());

        // Non-overlapping at higher LBA
        let part2 = GptPartitionEntry {
            type_guid: PartitionTypeGuid::WINDOWS_BASIC_DATA,
            unique_guid: PartitionUuid([2; 16]),
            start_lba: Lba(10000),
            end_lba: Lba(20000),
            attributes: 0,
            name: "DataHigh".to_string(),
        };
        assert!(table.add_partition(part2).is_ok());

        // Non-overlapping between part1 and part2 (tests entry.end_lba.0 < existing.start_lba.0)
        let part3 = GptPartitionEntry {
            type_guid: PartitionTypeGuid::WINDOWS_BASIC_DATA,
            unique_guid: PartitionUuid([3; 16]),
            start_lba: Lba(5000),
            end_lba: Lba(8000),
            attributes: 0,
            name: "DataMid".to_string(),
        };
        assert!(table.add_partition(part3).is_ok());

        // Header serialization
        let primary_hdr = table.serialize_header(false, 512);
        assert_eq!(&primary_hdr[0..8], b"EFI PART");
        let backup_hdr = table.serialize_header(true, 512);
        assert_eq!(&backup_hdr[0..8], b"EFI PART");

        // Protective MBR
        let prot_mbr = table.serialize_protective_mbr(512);
        assert_eq!(prot_mbr[450], 0xEE);
        assert_eq!(prot_mbr[510], 0x55);
        assert_eq!(prot_mbr[511], 0xAA);

        // Too small drive
        assert!(GptTable::new(50, 512, [0; 16]).is_err());
    }

    /// Tests automated standard partition schemes for Windows and Linux.
    #[test]
    fn test_standard_partition_schemes() {
        let total_sectors = 209_715_200; // 100 GiB in 512-byte sectors
        let win_scheme = StandardPartitionScheme::UefiWindows {
            esp_mb: 260,
            msr_mb: 16,
            recovery_mb: 1000,
        };
        let win_gpt = win_scheme.provision(total_sectors, 512, [0xAA; 16]);
        assert_eq!(win_gpt.as_ref().map(|g| g.partitions.len()), Ok(4));
        assert_eq!(
            win_gpt.as_ref().map(|g| g.partitions[0].type_guid),
            Ok(PartitionTypeGuid::ESP)
        );
        assert_eq!(
            win_gpt.as_ref().map(|g| g.partitions[1].type_guid),
            Ok(PartitionTypeGuid::MSR)
        );
        assert_eq!(
            win_gpt.as_ref().map(|g| g.partitions[2].type_guid),
            Ok(PartitionTypeGuid::WINDOWS_BASIC_DATA)
        );
        assert_eq!(
            win_gpt.as_ref().map(|g| g.partitions[3].type_guid),
            Ok(PartitionTypeGuid::WINDOWS_RECOVERY)
        );

        let linux_scheme = StandardPartitionScheme::UefiLinux {
            esp_mb: 512,
            boot_mb: 1024,
            swap_mb: 4096,
        };
        let linux_gpt = linux_scheme.provision(total_sectors, 512, [0xBB; 16]);
        assert_eq!(linux_gpt.as_ref().map(|g| g.partitions.len()), Ok(4));
        assert_eq!(
            linux_gpt.as_ref().map(|g| g.partitions[0].type_guid),
            Ok(PartitionTypeGuid::ESP)
        );
        assert_eq!(
            linux_gpt.as_ref().map(|g| g.partitions[1].type_guid),
            Ok(PartitionTypeGuid::LINUX_BOOT)
        );
        assert_eq!(
            linux_gpt.as_ref().map(|g| g.partitions[2].type_guid),
            Ok(PartitionTypeGuid::LINUX_SWAP)
        );
        assert_eq!(
            linux_gpt.as_ref().map(|g| g.partitions[3].type_guid),
            Ok(PartitionTypeGuid::LINUX_ROOT_X86_64)
        );

        // Scheme provision on too small drive for GPT headers (new_for_drive failure)
        assert!(win_scheme.provision(1000, 512, [0; 16]).is_err());
        assert!(linux_scheme.provision(1000, 512, [0; 16]).is_err());

        // Scheme provision with enough for GPT headers but too small for layout
        assert!(win_scheme.provision(5000, 512, [0; 16]).is_err());
        assert!(linux_scheme.provision(5000, 512, [0; 16]).is_err());
    }

    /// Tests GPT partition boundary validation and errors in `add_partition`.
    #[test]
    fn test_gpt_add_partition_errors() {
        let mut table = GptTable::default_mock();

        // 1. start_lba < first_usable_lba
        let before_start = GptPartitionEntry {
            type_guid: PartitionTypeGuid::ESP,
            unique_guid: PartitionUuid([1; 16]),
            start_lba: Lba(100),
            end_lba: Lba(3000),
            attributes: 0,
            name: "BadStart".to_string(),
        };
        assert!(table.add_partition(before_start).is_err());

        // 2. end_lba > last_usable_lba
        let exceeds_end = GptPartitionEntry {
            type_guid: PartitionTypeGuid::ESP,
            unique_guid: PartitionUuid([1; 16]),
            start_lba: Lba(2048),
            end_lba: Lba(100_000),
            attributes: 0,
            name: "BadEnd".to_string(),
        };
        assert!(table.add_partition(exceeds_end).is_err());

        // 3. end_lba < start_lba
        let inverted = GptPartitionEntry {
            type_guid: PartitionTypeGuid::ESP,
            unique_guid: PartitionUuid([1; 16]),
            start_lba: Lba(5000),
            end_lba: Lba(4000),
            attributes: 0,
            name: "Inverted".to_string(),
        };
        assert!(table.add_partition(inverted).is_err());
    }

    impl GptTable {
        fn default_mock() -> Self {
            Self {
                disk_guid: [0; 16],
                header_lba: Lba(1),
                backup_header_lba: Lba(99_999),
                first_usable_lba: Lba(2048),
                last_usable_lba: Lba(99_000),
                partition_entries_lba: Lba(2),
                partitions: Vec::new(),
            }
        }
    }
}

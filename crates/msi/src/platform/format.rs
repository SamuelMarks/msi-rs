//! Filesystem Formatting & Verification Engine.
//!
//! Provides native formatting synthesis for FAT32 EFI System Partitions (ESP),
//! boot sector generators for NTFS and ext4, CLI formatting tool builders for
//! Linux/Windows bare-metal environments, and post-format integrity verification.

use crate::error::{Error, Result};

/// Target filesystem architecture category.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FileSystemKind {
    /// FAT32 filesystem (mandatory for UEFI ESP).
    Fat32,
    /// New Technology File System (Windows NT / Windows 10/11 OS partition).
    Ntfs,
    /// Fourth Extended Filesystem (standard Linux root/boot).
    Ext4,
    /// B-tree Filesystem (copy-on-write Linux).
    Btrfs,
    /// Silicon Graphics XFS (high performance 64-bit Linux).
    Xfs,
}

impl FileSystemKind {
    /// Returns the standard Linux formatting command utility binary name.
    ///
    /// # Returns
    ///
    /// Command name string (e.g. `mkfs.fat`, `mkfs.ntfs`, `mkfs.ext4`).
    #[must_use]
    pub const fn mkfs_command(self) -> &'static str {
        match self {
            Self::Fat32 => "mkfs.fat",
            Self::Ntfs => "mkfs.ntfs",
            Self::Ext4 => "mkfs.ext4",
            Self::Btrfs => "mkfs.btrfs",
            Self::Xfs => "mkfs.xfs",
        }
    }
}

/// Configuration parameters for FAT32 formatting.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Fat32FormatOptions {
    /// Volume label (up to 11 ASCII characters).
    pub volume_label: String,
    /// Sectors per allocation cluster (typically 8 for 4 KiB clusters).
    pub sectors_per_cluster: u8,
    /// Number of reserved sectors before the first FAT (standard 32).
    pub reserved_sectors: u16,
    /// Number of File Allocation Table (FAT) copies (standard 2).
    pub num_fats: u8,
    /// 32-bit unique volume serial number.
    pub volume_id: u32,
}

impl Default for Fat32FormatOptions {
    fn default() -> Self {
        Self {
            volume_label: "EFI".to_string(),
            sectors_per_cluster: 8,
            reserved_sectors: 32,
            num_fats: 2,
            volume_id: 0x1234_5678,
        }
    }
}

/// Pure-Rust FAT32 filesystem synthesis engine.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Fat32Formatter;

impl Fat32Formatter {
    /// Generates a compliant FAT32 filesystem image containing BPB, `FSInfo`, FATs, and root directory.
    ///
    /// # Arguments
    ///
    /// * `total_sectors` - Sector count of target partition.
    /// * `sector_size` - Sector size in bytes (typically 512).
    /// * `options` - FAT32 formatting configuration.
    ///
    /// # Returns
    ///
    /// Byte buffer representing the formatted filesystem head.
    ///
    /// # Errors
    ///
    /// Returns [`Error::FileSystemFormatError`] if sector count is too small (< 65536 clusters for FAT32).
    pub fn format_filesystem(
        total_sectors: u64,
        sector_size: u32,
        options: &Fat32FormatOptions,
    ) -> Result<Vec<u8>> {
        if sector_size != 512 {
            return Err(Error::FileSystemFormatError {
                fs_type: "FAT32".to_string(),
                reason: format!("unsupported sector size {sector_size}, only 512 supported"),
            });
        }

        // FAT32 requires at least 65,525 clusters
        let spc = u64::from(options.sectors_per_cluster);
        let min_sectors = 65_536 * spc + u64::from(options.reserved_sectors);
        if total_sectors < min_sectors {
            return Err(Error::FileSystemFormatError {
                fs_type: "FAT32".to_string(),
                reason: format!(
                    "total sectors ({total_sectors}) too small for FAT32 (minimum {min_sectors})"
                ),
            });
        }

        // Compute FAT size: total_clusters * 4 bytes / sector_size
        let usable_data_sectors = total_sectors.saturating_sub(u64::from(options.reserved_sectors));
        let est_clusters = usable_data_sectors / spc;
        let fat_size_bytes = est_clusters * 4;
        let fat_size_sectors = u32::try_from(fat_size_bytes.div_ceil(512)).unwrap_or(u32::MAX);

        let total_sectors_32 = u32::try_from(total_sectors).unwrap_or(u32::MAX);

        // We synthesize the first 1 MiB of filesystem (BPB + FSInfo + Backup + FATs)
        let staging_sectors = (u64::from(options.reserved_sectors)
            + (u64::from(options.num_fats) * u64::from(fat_size_sectors))
            + spc)
            .min(2048);
        let image_len = usize::try_from(staging_sectors * 512).unwrap_or(1024 * 1024);
        let mut image = vec![0u8; image_len];

        // Sector 0: Boot Sector (BPB)
        let bpb = Self::generate_bpb(total_sectors_32, fat_size_sectors, options);
        image[0..512].copy_from_slice(&bpb);

        // Sector 1: FSInfo Sector
        let fsinfo = Self::generate_fsinfo(u32::try_from(est_clusters).unwrap_or(u32::MAX));
        image[512..1024].copy_from_slice(&fsinfo);

        // Sector 6: Backup Boot Sector
        image[6 * 512..7 * 512].copy_from_slice(&bpb);
        // Sector 7: Backup FSInfo Sector
        image[7 * 512..8 * 512].copy_from_slice(&fsinfo);

        // Initialize FAT1 and FAT2
        let fat1_offset = (options.reserved_sectors as usize) * 512;
        if fat1_offset + 12 <= image.len() {
            // Cluster 0: Media descriptor
            image[fat1_offset..fat1_offset + 4].copy_from_slice(&0x0FFF_FFF8u32.to_le_bytes());
            // Cluster 1: EOF
            image[fat1_offset + 4..fat1_offset + 8].copy_from_slice(&0x0FFF_FFFFu32.to_le_bytes());
            // Cluster 2: Root directory EOF
            image[fat1_offset + 8..fat1_offset + 12].copy_from_slice(&0x0FFF_FFFFu32.to_le_bytes());
        }

        let fat2_offset = fat1_offset + ((fat_size_sectors as usize) * 512);
        if fat2_offset + 12 <= image.len() {
            image[fat2_offset..fat2_offset + 4].copy_from_slice(&0x0FFF_FFF8u32.to_le_bytes());
            image[fat2_offset + 4..fat2_offset + 8].copy_from_slice(&0x0FFF_FFFFu32.to_le_bytes());
            image[fat2_offset + 8..fat2_offset + 12].copy_from_slice(&0x0FFF_FFFFu32.to_le_bytes());
        }

        Ok(image)
    }

    /// Synthesizes the 512-byte FAT32 Boot Parameter Block (BPB).
    fn generate_bpb(
        total_sectors: u32,
        fat_size_sectors: u32,
        options: &Fat32FormatOptions,
    ) -> [u8; 512] {
        let mut bpb = [0u8; 512];
        // Jump instruction: EB 58 90
        bpb[0] = 0xEB;
        bpb[1] = 0x58;
        bpb[2] = 0x90;
        // OEM Name: MSWIN4.1
        bpb[3..11].copy_from_slice(b"MSWIN4.1");
        // Bytes per sector: 512
        bpb[11..13].copy_from_slice(&512u16.to_le_bytes());
        // Sectors per cluster
        bpb[13] = options.sectors_per_cluster;
        // Reserved sector count
        bpb[14..16].copy_from_slice(&options.reserved_sectors.to_le_bytes());
        // Number of FATs
        bpb[16] = options.num_fats;
        // Root entries (0 for FAT32)
        bpb[17..19].copy_from_slice(&0u16.to_le_bytes());
        // Total sectors 16 (0 for FAT32)
        bpb[19..21].copy_from_slice(&0u16.to_le_bytes());
        // Media descriptor: 0xF8 (fixed disk)
        bpb[21] = 0xF8;
        // Sectors per track: 63
        bpb[24..26].copy_from_slice(&63u16.to_le_bytes());
        // Number of heads: 255
        bpb[26..28].copy_from_slice(&255u16.to_le_bytes());
        // Total sectors 32
        bpb[32..36].copy_from_slice(&total_sectors.to_le_bytes());
        // Sectors per FAT 32
        bpb[36..40].copy_from_slice(&fat_size_sectors.to_le_bytes());
        // Root cluster: 2
        bpb[44..48].copy_from_slice(&2u32.to_le_bytes());
        // FSInfo sector: 1
        bpb[48..50].copy_from_slice(&1u16.to_le_bytes());
        // Backup boot sector: 6
        bpb[50..52].copy_from_slice(&6u16.to_le_bytes());
        // Drive number: 0x80
        bpb[64] = 0x80;
        // Extended boot signature: 0x29
        bpb[66] = 0x29;
        // Volume serial ID
        bpb[67..71].copy_from_slice(&options.volume_id.to_le_bytes());

        // Volume label (11 bytes space padded)
        let mut label_bytes = [b' '; 11];
        let copy_len = options.volume_label.len().min(11);
        label_bytes[..copy_len].copy_from_slice(&options.volume_label.as_bytes()[..copy_len]);
        bpb[71..82].copy_from_slice(&label_bytes);

        // Filesystem type: "FAT32   "
        bpb[82..90].copy_from_slice(b"FAT32   ");

        // Boot signature 0xAA55
        bpb[510] = 0x55;
        bpb[511] = 0xAA;

        bpb
    }

    /// Synthesizes the 512-byte FAT32 `FSInfo` sector.
    fn generate_fsinfo(free_clusters: u32) -> [u8; 512] {
        let mut fsinfo = [0u8; 512];
        // Lead signature: 0x41615252 ("RRaA")
        fsinfo[0..4].copy_from_slice(&0x4161_5252u32.to_le_bytes());
        // Struct signature: 0x61417272 ("rrAa")
        fsinfo[484..488].copy_from_slice(&0x6141_7272u32.to_le_bytes());
        // Last known free cluster count
        fsinfo[488..492].copy_from_slice(&free_clusters.to_le_bytes());
        // Next free cluster hint: 3
        fsinfo[492..496].copy_from_slice(&3u32.to_le_bytes());
        // Trail signature: 0xAA550000
        fsinfo[508..512].copy_from_slice(&0xAA55_0000u32.to_le_bytes());

        fsinfo
    }
}

/// Pure-Rust NTFS boot sector generator.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct NtfsFormatter;

impl NtfsFormatter {
    /// Generates a valid NTFS partition boot sector (512 bytes).
    ///
    /// # Arguments
    ///
    /// * `total_sectors` - Total sector count of partition.
    /// * `volume_serial` - 64-bit volume serial number.
    ///
    /// # Returns
    ///
    /// 512-byte boot sector array.
    #[must_use]
    pub fn generate_boot_sector(total_sectors: u64, volume_serial: u64) -> [u8; 512] {
        let mut bpb = [0u8; 512];
        // Jump instruction: EB 52 90
        bpb[0] = 0xEB;
        bpb[1] = 0x52;
        bpb[2] = 0x90;
        // OEM ID: "NTFS    "
        bpb[3..11].copy_from_slice(b"NTFS    ");
        // Bytes per sector: 512
        bpb[11..13].copy_from_slice(&512u16.to_le_bytes());
        // Sectors per cluster: 8 (4 KiB)
        bpb[13] = 8;
        // Media descriptor: 0xF8
        bpb[21] = 0xF8;
        // Total sectors 64
        bpb[40..48].copy_from_slice(&total_sectors.to_le_bytes());
        // Logical cluster number for $MFT (cluster 4)
        bpb[48..56].copy_from_slice(&4u64.to_le_bytes());
        // Logical cluster number for $MFTMirr (cluster 8)
        bpb[56..64].copy_from_slice(&8u64.to_le_bytes());
        // Clusters per file record: 0xF6 (-10 = 2^10 = 1024 bytes)
        bpb[64] = 0xF6;
        // Clusters per index block: 1
        bpb[68] = 1;
        // Volume serial number: 64-bit
        bpb[72..80].copy_from_slice(&volume_serial.to_le_bytes());

        // Boot signature 0xAA55
        bpb[510] = 0x55;
        bpb[511] = 0xAA;

        bpb
    }
}

/// Pure-Rust ext4 superblock generator and validator.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Ext4Formatter;

impl Ext4Formatter {
    /// Generates a valid ext4 superblock (1024 bytes).
    ///
    /// # Arguments
    ///
    /// * `block_count` - Total 4 KiB block count.
    /// * `volume_uuid` - 16-byte filesystem UUID.
    ///
    /// # Returns
    ///
    /// 1024-byte superblock array.
    #[must_use]
    pub fn generate_superblock(block_count: u64, volume_uuid: [u8; 16]) -> [u8; 1024] {
        let mut sb = [0u8; 1024];
        let block_count_32 = u32::try_from(block_count).unwrap_or(u32::MAX);

        // Inodes count: block_count / 4
        let inodes = (block_count_32 / 4).max(1024);
        sb[0..4].copy_from_slice(&inodes.to_le_bytes());
        // Blocks count 32
        sb[4..8].copy_from_slice(&block_count_32.to_le_bytes());
        // Reserved blocks count: 5% of total
        let r_blocks = block_count_32 / 20;
        sb[8..12].copy_from_slice(&r_blocks.to_le_bytes());
        // Free blocks count
        sb[12..16].copy_from_slice(&block_count_32.saturating_sub(100).to_le_bytes());
        // First data block (0 for 4KB block size)
        sb[20..24].copy_from_slice(&0u32.to_le_bytes());
        // Block size: 2 (2^(10+2) = 4096 bytes)
        sb[24..28].copy_from_slice(&2u32.to_le_bytes());
        // Blocks per group: 32768
        sb[32..36].copy_from_slice(&32768u32.to_le_bytes());
        // Magic signature: 0xEF53 at offset 56 (0x38)
        sb[56..58].copy_from_slice(&0xEF53u16.to_le_bytes());
        // State: 1 (cleanly unmounted)
        sb[58..60].copy_from_slice(&1u16.to_le_bytes());
        // Filesystem UUID at offset 104..120
        sb[104..120].copy_from_slice(&volume_uuid);

        sb
    }
}

/// Command builder for external mkfs formatting utilities.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FormatCommandBuilder;

impl FormatCommandBuilder {
    /// Builds arguments for running the appropriate formatting command on a block device.
    ///
    /// # Arguments
    ///
    /// * `fs` - Target filesystem kind.
    /// * `device_node` - Block device node path (e.g. `/dev/nvme0n1p3`).
    /// * `label` - Filesystem volume label.
    ///
    /// # Returns
    ///
    /// Vector of command and CLI arguments (e.g. `["mkfs.fat", "-F", "32", "-n", "ESP", "/dev/..."]`).
    #[must_use]
    pub fn build_command(fs: FileSystemKind, device_node: &str, label: &str) -> Vec<String> {
        let mut args = Vec::new();
        args.push(fs.mkfs_command().to_string());

        match fs {
            FileSystemKind::Fat32 => {
                args.push("-F".to_string());
                args.push("32".to_string());
                args.push("-n".to_string());
                args.push(label.to_string());
            }
            FileSystemKind::Ntfs => {
                args.push("-f".to_string()); // fast format
                args.push("-L".to_string());
                args.push(label.to_string());
            }
            FileSystemKind::Ext4 => {
                args.push("-F".to_string()); // force
                args.push("-L".to_string());
                args.push(label.to_string());
            }
            FileSystemKind::Btrfs | FileSystemKind::Xfs => {
                args.push("-f".to_string());
                args.push("-L".to_string());
                args.push(label.to_string());
            }
        }

        args.push(device_node.to_string());
        args
    }
}

/// Post-format filesystem verification engine.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct FileSystemVerifier;

impl FileSystemVerifier {
    /// Validates the boot sector and metadata integrity of a formatted filesystem.
    ///
    /// # Arguments
    ///
    /// * `fs` - Filesystem format kind expected.
    /// * `buffer` - Byte buffer of the filesystem beginning.
    ///
    /// # Errors
    ///
    /// Returns [`Error::FileSystemFormatError`] if validation signatures or headers are corrupt.
    pub fn verify(fs: FileSystemKind, buffer: &[u8]) -> Result<()> {
        match fs {
            FileSystemKind::Fat32 => Self::verify_fat32(buffer),
            FileSystemKind::Ntfs => Self::verify_ntfs(buffer),
            FileSystemKind::Ext4 => Self::verify_ext4(buffer),
            FileSystemKind::Btrfs => Self::verify_btrfs(buffer),
            FileSystemKind::Xfs => Self::verify_xfs(buffer),
        }
    }

    /// Verifies FAT32 BPB signatures, OEM ID, and `FSInfo` magic numbers.
    fn verify_fat32(buffer: &[u8]) -> Result<()> {
        if buffer.len() < 1024 {
            return Err(Error::FileSystemFormatError {
                fs_type: "FAT32".to_string(),
                reason: "buffer too small for FAT32 boot sector and FSInfo".to_string(),
            });
        }

        // Check boot signature 0xAA55
        if buffer[510] != 0x55 || buffer[511] != 0xAA {
            return Err(Error::FileSystemFormatError {
                fs_type: "FAT32".to_string(),
                reason: "invalid boot sector signature, expected 0x55AA".to_string(),
            });
        }

        // Check FAT32 string identifier at offset 82..90
        if &buffer[82..90] != b"FAT32   " {
            return Err(Error::FileSystemFormatError {
                fs_type: "FAT32".to_string(),
                reason: "missing FAT32 type string in BPB".to_string(),
            });
        }

        // Check FSInfo lead signature
        if buffer[512..516] != 0x4161_5252u32.to_le_bytes() {
            return Err(Error::FileSystemFormatError {
                fs_type: "FAT32".to_string(),
                reason: "corrupt FSInfo lead signature".to_string(),
            });
        }

        Ok(())
    }

    /// Verifies NTFS OEM signature and boot sector ending.
    fn verify_ntfs(buffer: &[u8]) -> Result<()> {
        if buffer.len() < 512 {
            return Err(Error::FileSystemFormatError {
                fs_type: "NTFS".to_string(),
                reason: "buffer too small for NTFS boot sector".to_string(),
            });
        }

        if &buffer[3..11] != b"NTFS    " {
            return Err(Error::FileSystemFormatError {
                fs_type: "NTFS".to_string(),
                reason: "invalid NTFS OEM identifier".to_string(),
            });
        }

        if buffer[510] != 0x55 || buffer[511] != 0xAA {
            return Err(Error::FileSystemFormatError {
                fs_type: "NTFS".to_string(),
                reason: "invalid boot sector signature, expected 0x55AA".to_string(),
            });
        }

        Ok(())
    }

    /// Verifies ext4 superblock magic number (0xEF53).
    fn verify_ext4(buffer: &[u8]) -> Result<()> {
        // Superblock starts at byte 1024 (offset 0x400)
        let sb_offset = 1024;
        if buffer.len() < sb_offset + 1024 {
            return Err(Error::FileSystemFormatError {
                fs_type: "ext4".to_string(),
                reason: "buffer too small for ext4 superblock".to_string(),
            });
        }

        let magic = u16::from_le_bytes([buffer[sb_offset + 56], buffer[sb_offset + 57]]);
        if magic != 0xEF53 {
            return Err(Error::FileSystemFormatError {
                fs_type: "ext4".to_string(),
                reason: format!("invalid ext4 magic 0x{magic:04X}, expected 0xEF53"),
            });
        }

        Ok(())
    }

    /// Verifies Btrfs superblock signature (`_BHRfS_M` at offset 65536).
    fn verify_btrfs(buffer: &[u8]) -> Result<()> {
        let sb_offset = 65_536;
        if buffer.len() < sb_offset + 64 {
            return Err(Error::FileSystemFormatError {
                fs_type: "Btrfs".to_string(),
                reason: "buffer too small for Btrfs primary superblock".to_string(),
            });
        }

        let magic = &buffer[sb_offset + 64..sb_offset + 72];
        if magic != b"_BHRfS_M" {
            return Err(Error::FileSystemFormatError {
                fs_type: "Btrfs".to_string(),
                reason: "invalid Btrfs magic signature".to_string(),
            });
        }

        Ok(())
    }

    /// Verifies XFS superblock magic number (`XFSB` at offset 0).
    fn verify_xfs(buffer: &[u8]) -> Result<()> {
        if buffer.len() < 512 {
            return Err(Error::FileSystemFormatError {
                fs_type: "XFS".to_string(),
                reason: "buffer too small for XFS superblock".to_string(),
            });
        }

        if &buffer[0..4] != b"XFSB" {
            return Err(Error::FileSystemFormatError {
                fs_type: "XFS".to_string(),
                reason: "invalid XFS magic signature".to_string(),
            });
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Tests FAT32 image generation and verification.
    #[test]
    fn test_fat32_format_and_verify() {
        let total_sectors = 1_000_000; // ~500 MB
        let options = Fat32FormatOptions {
            volume_label: "ESP_TEST".to_string(),
            sectors_per_cluster: 8,
            reserved_sectors: 32,
            num_fats: 2,
            volume_id: 0xAABB_CCDD,
        };

        let res = Fat32Formatter::format_filesystem(total_sectors, 512, &options);
        assert!(res.is_ok());
        let image = res.unwrap_or_default();
        assert_eq!(
            FileSystemVerifier::verify(FileSystemKind::Fat32, &image),
            Ok(())
        );

        // False branches for fat1_offset + 12 <= image.len() and fat2_offset + 12 <= image.len()
        let large_reserved_opts = Fat32FormatOptions {
            reserved_sectors: 3000,
            ..options.clone()
        };
        assert!(
            Fat32Formatter::format_filesystem(total_sectors, 512, &large_reserved_opts).is_ok()
        );
        assert!(Fat32Formatter::format_filesystem(10_000_000, 512, &options).is_ok());

        // Invalid sector size
        assert!(Fat32Formatter::format_filesystem(total_sectors, 4096, &options).is_err());
        // Too small partition
        assert!(Fat32Formatter::format_filesystem(100, 512, &options).is_err());

        // FileSystemVerifier FAT32 error branches
        assert!(FileSystemVerifier::verify(FileSystemKind::Fat32, &[0u8; 100]).is_err());

        let mut bad_sig = image.clone();
        bad_sig[510] = 0;
        assert!(FileSystemVerifier::verify(FileSystemKind::Fat32, &bad_sig).is_err());

        let mut bad_sig2 = image.clone();
        bad_sig2[510] = 0x55;
        bad_sig2[511] = 0x00;
        assert!(FileSystemVerifier::verify(FileSystemKind::Fat32, &bad_sig2).is_err());

        let mut bad_type = image.clone();
        bad_type[82..90].copy_from_slice(b"NOTFAT32");
        assert!(FileSystemVerifier::verify(FileSystemKind::Fat32, &bad_type).is_err());

        let mut bad_fsinfo = image;
        bad_fsinfo[512..516].copy_from_slice(&[0; 4]);
        assert!(FileSystemVerifier::verify(FileSystemKind::Fat32, &bad_fsinfo).is_err());
    }

    /// Tests NTFS boot sector generation and verification.
    #[test]
    fn test_ntfs_format_and_verify() {
        let boot = NtfsFormatter::generate_boot_sector(209_715_200, 0x1122_3344_5566_7788);
        assert!(FileSystemVerifier::verify(FileSystemKind::Ntfs, &boot).is_ok());

        // Buffer too small (< 512)
        assert!(FileSystemVerifier::verify(FileSystemKind::Ntfs, &[0u8; 100]).is_err());

        // Corrupt signature
        let mut bad_boot = boot;
        bad_boot[510] = 0x00;
        assert!(FileSystemVerifier::verify(FileSystemKind::Ntfs, &bad_boot).is_err());

        let mut bad_boot2 = boot;
        bad_boot2[510] = 0x55;
        bad_boot2[511] = 0x00;
        assert!(FileSystemVerifier::verify(FileSystemKind::Ntfs, &bad_boot2).is_err());

        // Bad OEM
        let mut bad_oem = boot;
        bad_oem[3..11].copy_from_slice(b"FAT32   ");
        assert!(FileSystemVerifier::verify(FileSystemKind::Ntfs, &bad_oem).is_err());
    }

    /// Tests ext4 superblock generation and verification.
    #[test]
    fn test_ext4_format_and_verify() {
        let mut buffer = vec![0u8; 2048];
        let sb = Ext4Formatter::generate_superblock(26_214_400, [0xEE; 16]);
        buffer[1024..2048].copy_from_slice(&sb);

        assert!(FileSystemVerifier::verify(FileSystemKind::Ext4, &buffer).is_ok());

        // Buffer too small (< 2048)
        assert!(FileSystemVerifier::verify(FileSystemKind::Ext4, &[0u8; 100]).is_err());

        // Corrupt magic
        buffer[1024 + 56] = 0x00;
        assert!(FileSystemVerifier::verify(FileSystemKind::Ext4, &buffer).is_err());
    }

    /// Tests Btrfs and XFS signature verification.
    #[test]
    fn test_btrfs_and_xfs_verify() {
        let mut btrfs_buf = vec![0u8; 65536 + 128];
        btrfs_buf[65536 + 64..65536 + 72].copy_from_slice(b"_BHRfS_M");
        assert!(FileSystemVerifier::verify(FileSystemKind::Btrfs, &btrfs_buf).is_ok());

        // Buffer too small (< 65536 + 64)
        assert!(FileSystemVerifier::verify(FileSystemKind::Btrfs, &[0u8; 100]).is_err());

        btrfs_buf[65536 + 64] = 0;
        assert!(FileSystemVerifier::verify(FileSystemKind::Btrfs, &btrfs_buf).is_err());

        let mut xfs_buf = vec![0u8; 512];
        xfs_buf[0..4].copy_from_slice(b"XFSB");
        assert!(FileSystemVerifier::verify(FileSystemKind::Xfs, &xfs_buf).is_ok());

        // Buffer too small (< 512)
        assert!(FileSystemVerifier::verify(FileSystemKind::Xfs, &[0u8; 100]).is_err());

        xfs_buf[0] = 0;
        assert!(FileSystemVerifier::verify(FileSystemKind::Xfs, &xfs_buf).is_err());
    }

    /// Tests format command line builders for external mkfs tools.
    #[test]
    fn test_format_command_builder() {
        let fat_cmd =
            FormatCommandBuilder::build_command(FileSystemKind::Fat32, "/dev/nvme0n1p1", "ESP");
        assert_eq!(
            fat_cmd,
            vec!["mkfs.fat", "-F", "32", "-n", "ESP", "/dev/nvme0n1p1"]
        );

        let ntfs_cmd =
            FormatCommandBuilder::build_command(FileSystemKind::Ntfs, "/dev/nvme0n1p3", "Windows");
        assert_eq!(
            ntfs_cmd,
            vec!["mkfs.ntfs", "-f", "-L", "Windows", "/dev/nvme0n1p3"]
        );

        let ext4_cmd =
            FormatCommandBuilder::build_command(FileSystemKind::Ext4, "/dev/sda2", "root");
        assert_eq!(ext4_cmd, vec!["mkfs.ext4", "-F", "-L", "root", "/dev/sda2"]);

        let btrfs_cmd =
            FormatCommandBuilder::build_command(FileSystemKind::Btrfs, "/dev/sda2", "btrfs_root");
        assert_eq!(
            btrfs_cmd,
            vec!["mkfs.btrfs", "-f", "-L", "btrfs_root", "/dev/sda2"]
        );

        let xfs_cmd =
            FormatCommandBuilder::build_command(FileSystemKind::Xfs, "/dev/sda2", "xfs_root");
        assert_eq!(
            xfs_cmd,
            vec!["mkfs.xfs", "-f", "-L", "xfs_root", "/dev/sda2"]
        );
    }
}

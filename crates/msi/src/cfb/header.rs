//! Compound File Binary Format Header parser, serializer, and validator ([MS-CFB] 2.2).

use crate::cfb::sector::SectorId;
use crate::error::{Error, Result};

/// CFB magic signature bytes (`{ 0xD0, 0xCF, 0x11, 0xE0, 0xA1, 0xB1, 0x1A, 0xE1 }`).
pub const CFB_SIGNATURE: [u8; 8] = [0xD0, 0xCF, 0x11, 0xE0, 0xA1, 0xB1, 0x1A, 0xE1];

/// CFB standard minor version (`0x003E`).
pub const CFB_MINOR_VERSION: u16 = 0x003E;

/// CFB little-endian byte order mark (`0xFFFE`).
pub const CFB_BYTE_ORDER_LE: u16 = 0xFFFE;

/// Standard mini stream cutoff size in bytes (4096 bytes / `0x00001000`).
pub const CFB_MINI_STREAM_CUTOFF_STANDARD: u32 = 0x0000_1000;

/// Standard mini sector shift (`0x0006` -> 64 bytes).
pub const CFB_MINI_SECTOR_SHIFT_STANDARD: u16 = 0x0006;

/// Number of DIFAT sector entries stored directly in the header ([MS-CFB] 2.2).
pub const CFB_HEADER_DIFAT_ENTRIES: usize = 109;

/// Exact byte length of the CFB header structure.
pub const CFB_HEADER_SIZE: usize = 512;

/// CFB Compound File major version.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CfbVersion {
    /// Version 3 format (512-byte sectors, 32-bit directory counts).
    V3,
    /// Version 4 format (4096-byte sectors, 64-bit directory counts).
    V4,
}

impl CfbVersion {
    /// Returns the required sector shift power-of-two exponent.
    ///
    /// # Returns
    ///
    /// `9` for [`CfbVersion::V3`] (512 bytes) or `12` for [`CfbVersion::V4`] (4096 bytes).
    #[must_use]
    pub const fn sector_shift(self) -> u16 {
        match self {
            Self::V3 => 9,
            Self::V4 => 12,
        }
    }

    /// Returns the sector size in bytes.
    ///
    /// # Returns
    ///
    /// `512` for [`CfbVersion::V3`] or `4096` for [`CfbVersion::V4`].
    #[must_use]
    pub const fn sector_size(self) -> usize {
        1 << self.sector_shift()
    }

    /// Returns the raw major version number (`3` or `4`).
    ///
    /// # Returns
    ///
    /// The major version integer as a [`u16`].
    #[must_use]
    pub const fn major_number(self) -> u16 {
        match self {
            Self::V3 => 3,
            Self::V4 => 4,
        }
    }
}

/// Parsed and validated Compound File Binary Format (CFB) header ([MS-CFB] 2.2).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CfbHeader {
    /// Compound file format version (v3 or v4).
    version: CfbVersion,
    /// Number of directory sectors (must be 0 in v3; valid sector count in v4).
    num_dir_sectors: u32,
    /// Total number of FAT sectors.
    num_fat_sectors: u32,
    /// Starting sector ID of the Directory stream.
    first_dir_sector: SectorId,
    /// Transaction sequence number.
    transaction_sig: u32,
    /// Maximum size of a stream stored in the Mini-Stream (standard 4096).
    mini_stream_cutoff: u32,
    /// Starting sector ID of the `MiniFAT` stream.
    first_minifat_sector: SectorId,
    /// Total number of `MiniFAT` sectors.
    num_minifat_sectors: u32,
    /// Starting sector ID of the DIFAT sector chain.
    first_difat_sector: SectorId,
    /// Total number of DIFAT sectors.
    num_difat_sectors: u32,
    /// First 109 DIFAT entries stored within the header block.
    difat_table: [SectorId; CFB_HEADER_DIFAT_ENTRIES],
}

impl CfbHeader {
    /// Creates a new, default [`CfbHeader`] configured for the specified [`CfbVersion`].
    ///
    /// All DIFAT entries are initialized to [`SectorId::FREE`].
    ///
    /// # Arguments
    ///
    /// * `version` - The target format version.
    ///
    /// # Returns
    ///
    /// A valid initialized [`CfbHeader`].
    #[must_use]
    pub const fn new(version: CfbVersion) -> Self {
        Self {
            version,
            num_dir_sectors: 0,
            num_fat_sectors: 0,
            first_dir_sector: SectorId::END_OF_CHAIN,
            transaction_sig: 0,
            mini_stream_cutoff: CFB_MINI_STREAM_CUTOFF_STANDARD,
            first_minifat_sector: SectorId::END_OF_CHAIN,
            num_minifat_sectors: 0,
            first_difat_sector: SectorId::END_OF_CHAIN,
            num_difat_sectors: 0,
            difat_table: [SectorId::FREE; CFB_HEADER_DIFAT_ENTRIES],
        }
    }

    /// Returns the CFB format version.
    ///
    /// # Returns
    ///
    /// The [`CfbVersion`].
    #[must_use]
    pub const fn version(&self) -> CfbVersion {
        self.version
    }

    /// Returns the sector size in bytes.
    ///
    /// # Returns
    ///
    /// The sector size in bytes.
    #[must_use]
    pub const fn sector_size(&self) -> usize {
        self.version.sector_size()
    }

    /// Returns the sector shift exponent.
    ///
    /// # Returns
    ///
    /// The sector shift power-of-two exponent.
    #[must_use]
    pub const fn sector_shift(&self) -> u16 {
        self.version.sector_shift()
    }

    /// Returns the mini sector size in bytes (always 64).
    ///
    /// # Returns
    ///
    /// Mini sector size in bytes (`64`).
    #[must_use]
    pub const fn mini_sector_size(&self) -> usize {
        1 << CFB_MINI_SECTOR_SHIFT_STANDARD
    }

    /// Returns the mini sector shift exponent (always 6).
    ///
    /// # Returns
    ///
    /// Mini sector shift exponent (`6`).
    #[must_use]
    pub const fn mini_sector_shift(&self) -> u16 {
        CFB_MINI_SECTOR_SHIFT_STANDARD
    }

    /// Returns the number of directory sectors.
    ///
    /// # Returns
    ///
    /// The directory sector count.
    #[must_use]
    pub const fn num_dir_sectors(&self) -> u32 {
        self.num_dir_sectors
    }

    /// Sets the number of directory sectors.
    ///
    /// # Arguments
    ///
    /// * `count` - The count of directory sectors.
    pub const fn set_num_dir_sectors(&mut self, count: u32) {
        self.num_dir_sectors = count;
    }

    /// Returns the number of FAT sectors.
    ///
    /// # Returns
    ///
    /// Total FAT sector count.
    #[must_use]
    pub const fn num_fat_sectors(&self) -> u32 {
        self.num_fat_sectors
    }

    /// Sets the number of FAT sectors.
    ///
    /// # Arguments
    ///
    /// * `count` - Total FAT sectors.
    pub const fn set_num_fat_sectors(&mut self, count: u32) {
        self.num_fat_sectors = count;
    }

    /// Returns the first directory sector ID.
    ///
    /// # Returns
    ///
    /// The starting [`SectorId`] of the directory stream.
    #[must_use]
    pub const fn first_dir_sector(&self) -> SectorId {
        self.first_dir_sector
    }

    /// Sets the first directory sector ID.
    ///
    /// # Arguments
    ///
    /// * `sector` - The starting [`SectorId`] of the directory stream.
    pub const fn set_first_dir_sector(&mut self, sector: SectorId) {
        self.first_dir_sector = sector;
    }

    /// Returns the transaction signature sequence number.
    ///
    /// # Returns
    ///
    /// Transaction sequence number as [`u32`].
    #[must_use]
    pub const fn transaction_sig(&self) -> u32 {
        self.transaction_sig
    }

    /// Returns the mini stream cutoff size in bytes.
    ///
    /// # Returns
    ///
    /// Mini stream cutoff size (typically 4096).
    #[must_use]
    pub const fn mini_stream_cutoff(&self) -> u32 {
        self.mini_stream_cutoff
    }

    /// Returns the first `MiniFAT` sector ID.
    ///
    /// # Returns
    ///
    /// The starting [`SectorId`] of the `MiniFAT` stream.
    #[must_use]
    pub const fn first_minifat_sector(&self) -> SectorId {
        self.first_minifat_sector
    }

    /// Sets the first `MiniFAT` sector ID.
    ///
    /// # Arguments
    ///
    /// * `sector` - The starting [`SectorId`] of the `MiniFAT` stream.
    pub const fn set_first_minifat_sector(&mut self, sector: SectorId) {
        self.first_minifat_sector = sector;
    }

    /// Returns the number of `MiniFAT` sectors.
    ///
    /// # Returns
    ///
    /// Total `MiniFAT` sector count.
    #[must_use]
    pub const fn num_minifat_sectors(&self) -> u32 {
        self.num_minifat_sectors
    }

    /// Sets the number of `MiniFAT` sectors.
    ///
    /// # Arguments
    ///
    /// * `count` - Total `MiniFAT` sector count.
    pub const fn set_num_minifat_sectors(&mut self, count: u32) {
        self.num_minifat_sectors = count;
    }

    /// Returns the first DIFAT sector ID.
    ///
    /// # Returns
    ///
    /// The starting [`SectorId`] of the external DIFAT chain.
    #[must_use]
    pub const fn first_difat_sector(&self) -> SectorId {
        self.first_difat_sector
    }

    /// Sets the first DIFAT sector ID.
    ///
    /// # Arguments
    ///
    /// * `sector` - The starting [`SectorId`] of the external DIFAT chain.
    pub const fn set_first_difat_sector(&mut self, sector: SectorId) {
        self.first_difat_sector = sector;
    }

    /// Returns the number of external DIFAT sectors.
    ///
    /// # Returns
    ///
    /// External DIFAT sector count.
    #[must_use]
    pub const fn num_difat_sectors(&self) -> u32 {
        self.num_difat_sectors
    }

    /// Sets the number of external DIFAT sectors.
    ///
    /// # Arguments
    ///
    /// * `count` - External DIFAT sector count.
    pub const fn set_num_difat_sectors(&mut self, count: u32) {
        self.num_difat_sectors = count;
    }

    /// Returns a reference to the 109 DIFAT entries stored in the header.
    ///
    /// # Returns
    ///
    /// Reference to the array of 109 [`SectorId`] entries.
    #[must_use]
    pub const fn difat_table(&self) -> &[SectorId; CFB_HEADER_DIFAT_ENTRIES] {
        &self.difat_table
    }

    /// Returns a mutable reference to the 109 DIFAT entries in the header.
    ///
    /// # Returns
    ///
    /// Mutable reference to the array of 109 [`SectorId`] entries.
    pub const fn difat_table_mut(&mut self) -> &mut [SectorId; CFB_HEADER_DIFAT_ENTRIES] {
        &mut self.difat_table
    }

    /// Parses and rigorously validates a CFB header from a byte slice ([MS-CFB] 2.2).
    ///
    /// # Arguments
    ///
    /// * `bytes` - Byte slice containing at least 512 bytes.
    ///
    /// # Returns
    ///
    /// A parsed and validated [`CfbHeader`].
    ///
    /// # Errors
    ///
    /// Returns:
    /// - [`Error::CfbCorrupted`] if slice is shorter than 512 bytes.
    /// - [`Error::InvalidCfbSignature`] if magic signature does not match.
    /// - [`Error::InvalidCfbClsid`] if header CLSID is not all zeroes.
    /// - [`Error::InvalidCfbMinorVersion`] if minor version is not `0x003E`.
    /// - [`Error::InvalidCfbMajorVersion`] if major version is not 3 or 4.
    /// - [`Error::InvalidCfbByteOrder`] if byte order is not little-endian (`0xFFFE`).
    /// - [`Error::InvalidCfbSectorShift`] if sector shift does not match major version.
    /// - [`Error::InvalidCfbMiniSectorShift`] if mini sector shift is not 6.
    /// - [`Error::InvalidCfbReserved`] if reserved 6 bytes are non-zero.
    /// - [`Error::InvalidCfbDirectorySectors`] if v3 directory sector count is non-zero.
    /// - [`Error::InvalidCfbMiniStreamCutoff`] if mini stream cutoff is not 4096.
    #[allow(clippy::too_many_lines)]
    pub fn parse(bytes: &[u8]) -> Result<Self> {
        if bytes.len() < CFB_HEADER_SIZE {
            return Err(Error::CfbCorrupted {
                offset: 0,
                reason: format!(
                    "Header byte slice too short: expected at least {CFB_HEADER_SIZE} bytes, got {}",
                    bytes.len()
                ),
            });
        }

        // 1. Signature (8 bytes)
        let mut sig = [0u8; 8];
        sig.copy_from_slice(&bytes[0..8]);
        if sig != CFB_SIGNATURE {
            return Err(Error::InvalidCfbSignature { found: sig });
        }

        // 2. Header CLSID (16 bytes, must be all zeroes)
        let mut clsid = [0u8; 16];
        clsid.copy_from_slice(&bytes[8..24]);
        if clsid != [0u8; 16] {
            return Err(Error::InvalidCfbClsid { found: clsid });
        }

        // 3. Minor Version (2 bytes, must be 0x003E)
        let minor_version = u16::from_le_bytes([bytes[24], bytes[25]]);
        if minor_version != CFB_MINOR_VERSION {
            return Err(Error::InvalidCfbMinorVersion {
                found: minor_version,
            });
        }

        // 4. Major Version (2 bytes, must be 3 or 4)
        let major_raw = u16::from_le_bytes([bytes[26], bytes[27]]);
        let version = match major_raw {
            3 => CfbVersion::V3,
            4 => CfbVersion::V4,
            _ => return Err(Error::InvalidCfbMajorVersion { found: major_raw }),
        };

        // 5. Byte Order (2 bytes, must be 0xFFFE)
        let byte_order = u16::from_le_bytes([bytes[28], bytes[29]]);
        if byte_order != CFB_BYTE_ORDER_LE {
            return Err(Error::InvalidCfbByteOrder { found: byte_order });
        }

        // 6. Sector Shift (2 bytes, 9 for v3, 12 for v4)
        let sector_shift = u16::from_le_bytes([bytes[30], bytes[31]]);
        if sector_shift != version.sector_shift() {
            return Err(Error::InvalidCfbSectorShift {
                major_version: version.major_number(),
                shift: sector_shift,
            });
        }

        // 7. Mini Sector Shift (2 bytes, must be 6)
        let mini_sector_shift = u16::from_le_bytes([bytes[32], bytes[33]]);
        if mini_sector_shift != CFB_MINI_SECTOR_SHIFT_STANDARD {
            return Err(Error::InvalidCfbMiniSectorShift {
                shift: mini_sector_shift,
            });
        }

        // 8. Reserved (6 bytes, must be all zeroes)
        let mut reserved = [0u8; 6];
        reserved.copy_from_slice(&bytes[34..40]);
        if reserved != [0u8; 6] {
            return Err(Error::InvalidCfbReserved { found: reserved });
        }

        // 9. Number of Directory Sectors (4 bytes; must be 0 if v3)
        let num_dir_sectors = u32::from_le_bytes([bytes[40], bytes[41], bytes[42], bytes[43]]);
        if version == CfbVersion::V3 && num_dir_sectors != 0 {
            return Err(Error::InvalidCfbDirectorySectors {
                major_version: 3,
                count: num_dir_sectors,
            });
        }

        // 10. Number of FAT Sectors (4 bytes)
        let num_fat_sectors = u32::from_le_bytes([bytes[44], bytes[45], bytes[46], bytes[47]]);

        // 11. First Directory Sector Location (4 bytes)
        let first_dir_sector = SectorId::new(u32::from_le_bytes([
            bytes[48], bytes[49], bytes[50], bytes[51],
        ]));

        // 12. Transaction Signature Number (4 bytes)
        let transaction_sig = u32::from_le_bytes([bytes[52], bytes[53], bytes[54], bytes[55]]);

        // 13. Mini Stream Cutoff Size (4 bytes, must be 4096)
        let mini_stream_cutoff = u32::from_le_bytes([bytes[56], bytes[57], bytes[58], bytes[59]]);
        if mini_stream_cutoff != CFB_MINI_STREAM_CUTOFF_STANDARD {
            return Err(Error::InvalidCfbMiniStreamCutoff {
                cutoff: mini_stream_cutoff,
            });
        }

        // 14. First MiniFAT Sector Location (4 bytes)
        let first_minifat_sector = SectorId::new(u32::from_le_bytes([
            bytes[60], bytes[61], bytes[62], bytes[63],
        ]));

        // 15. Number of MiniFAT Sectors (4 bytes)
        let num_minifat_sectors = u32::from_le_bytes([bytes[64], bytes[65], bytes[66], bytes[67]]);

        // 16. First DIFAT Sector Location (4 bytes)
        let first_difat_sector = SectorId::new(u32::from_le_bytes([
            bytes[68], bytes[69], bytes[70], bytes[71],
        ]));

        // 17. Number of DIFAT Sectors (4 bytes)
        let num_difat_sectors = u32::from_le_bytes([bytes[72], bytes[73], bytes[74], bytes[75]]);

        // 18. DIFAT Table (109 entries, 4 bytes each = 436 bytes)
        let mut difat_table = [SectorId::FREE; CFB_HEADER_DIFAT_ENTRIES];
        for (i, entry) in difat_table.iter_mut().enumerate() {
            let offset = 76 + i * 4;
            let val = u32::from_le_bytes([
                bytes[offset],
                bytes[offset + 1],
                bytes[offset + 2],
                bytes[offset + 3],
            ]);
            *entry = SectorId::new(val);
        }

        Ok(Self {
            version,
            num_dir_sectors,
            num_fat_sectors,
            first_dir_sector,
            transaction_sig,
            mini_stream_cutoff,
            first_minifat_sector,
            num_minifat_sectors,
            first_difat_sector,
            num_difat_sectors,
            difat_table,
        })
    }

    /// Serializes this header into exactly 512 bytes ([MS-CFB] 2.2).
    ///
    /// # Returns
    ///
    /// A 512-byte array containing the binary CFB header.
    #[must_use]
    pub fn to_bytes(&self) -> [u8; CFB_HEADER_SIZE] {
        let mut buf = [0u8; CFB_HEADER_SIZE];

        // 1. Signature
        buf[0..8].copy_from_slice(&CFB_SIGNATURE);

        // 2. Header CLSID (zeroed) - buf[8..24] is already 0

        // 3. Minor Version (0x003E)
        buf[24..26].copy_from_slice(&CFB_MINOR_VERSION.to_le_bytes());

        // 4. Major Version (3 or 4)
        buf[26..28].copy_from_slice(&self.version.major_number().to_le_bytes());

        // 5. Byte Order (0xFFFE)
        buf[28..30].copy_from_slice(&CFB_BYTE_ORDER_LE.to_le_bytes());

        // 6. Sector Shift (9 or 12)
        buf[30..32].copy_from_slice(&self.version.sector_shift().to_le_bytes());

        // 7. Mini Sector Shift (6)
        buf[32..34].copy_from_slice(&CFB_MINI_SECTOR_SHIFT_STANDARD.to_le_bytes());

        // 8. Reserved (zeroed) - buf[34..40] is already 0

        // 9. Number of Directory Sectors
        buf[40..44].copy_from_slice(&self.num_dir_sectors.to_le_bytes());

        // 10. Number of FAT Sectors
        buf[44..48].copy_from_slice(&self.num_fat_sectors.to_le_bytes());

        // 11. First Directory Sector Location
        buf[48..52].copy_from_slice(&self.first_dir_sector.as_u32().to_le_bytes());

        // 12. Transaction Signature Number
        buf[52..56].copy_from_slice(&self.transaction_sig.to_le_bytes());

        // 13. Mini Stream Cutoff Size
        buf[56..60].copy_from_slice(&self.mini_stream_cutoff.to_le_bytes());

        // 14. First MiniFAT Sector Location
        buf[60..64].copy_from_slice(&self.first_minifat_sector.as_u32().to_le_bytes());

        // 15. Number of MiniFAT Sectors
        buf[64..68].copy_from_slice(&self.num_minifat_sectors.to_le_bytes());

        // 16. First DIFAT Sector Location
        buf[68..72].copy_from_slice(&self.first_difat_sector.as_u32().to_le_bytes());

        // 17. Number of DIFAT Sectors
        buf[72..76].copy_from_slice(&self.num_difat_sectors.to_le_bytes());

        // 18. DIFAT Table (109 entries)
        for (i, entry) in self.difat_table.iter().enumerate() {
            let offset = 76 + i * 4;
            buf[offset..offset + 4].copy_from_slice(&entry.as_u32().to_le_bytes());
        }

        buf
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Tests serialization and deserialization roundtrip for [`CfbVersion::V3`].
    #[test]
    fn test_cfb_header_roundtrip_v3() {
        let mut header = CfbHeader::new(CfbVersion::V3);
        header.set_num_fat_sectors(1);
        header.set_first_dir_sector(SectorId::new(0));
        header.set_first_minifat_sector(SectorId::new(1));
        header.set_num_minifat_sectors(1);
        header.set_first_difat_sector(SectorId::END_OF_CHAIN);
        header.set_num_difat_sectors(0);
        header.difat_table_mut()[0] = SectorId::new(2);

        let bytes = header.to_bytes();
        assert_eq!(bytes.len(), CFB_HEADER_SIZE);

        let parsed = CfbHeader::parse(&bytes);
        assert_eq!(parsed, Ok(header.clone()));

        assert_eq!(header.version(), CfbVersion::V3);
        assert_eq!(header.sector_size(), 512);
        assert_eq!(header.sector_shift(), 9);
        assert_eq!(header.mini_sector_size(), 64);
        assert_eq!(header.mini_sector_shift(), 6);
        assert_eq!(header.num_dir_sectors(), 0);
        assert_eq!(header.num_fat_sectors(), 1);
        assert_eq!(header.first_dir_sector(), SectorId::new(0));
        assert_eq!(header.transaction_sig(), 0);
        assert_eq!(header.mini_stream_cutoff(), 4096);
        assert_eq!(header.first_minifat_sector(), SectorId::new(1));
        assert_eq!(header.num_minifat_sectors(), 1);
        assert_eq!(header.first_difat_sector(), SectorId::END_OF_CHAIN);
        assert_eq!(header.num_difat_sectors(), 0);
        assert_eq!(header.difat_table()[0], SectorId::new(2));
    }

    /// Tests serialization and deserialization roundtrip for [`CfbVersion::V4`].
    #[test]
    fn test_cfb_header_roundtrip_v4() {
        let mut header = CfbHeader::new(CfbVersion::V4);
        header.set_num_dir_sectors(4);
        header.set_num_fat_sectors(2);

        let bytes = header.to_bytes();
        assert_eq!(bytes.len(), CFB_HEADER_SIZE);

        let parsed = CfbHeader::parse(&bytes);
        assert_eq!(parsed, Ok(header.clone()));

        assert_eq!(header.version(), CfbVersion::V4);
        assert_eq!(header.sector_size(), 4096);
        assert_eq!(header.sector_shift(), 12);
        assert_eq!(header.num_dir_sectors(), 4);
    }

    /// Tests header parsing failures on corrupted or invalid headers.
    #[test]
    fn test_cfb_header_parse_errors() {
        let header = CfbHeader::new(CfbVersion::V3);
        let bytes = header.to_bytes();

        // 1. Too short
        assert!(matches!(
            CfbHeader::parse(&bytes[0..511]),
            Err(Error::CfbCorrupted { .. })
        ));

        // 2. Bad signature
        let mut bad = bytes;
        bad[0] = 0x00;
        assert!(matches!(
            CfbHeader::parse(&bad),
            Err(Error::InvalidCfbSignature { .. })
        ));

        // 3. Bad CLSID
        bad = bytes;
        bad[8] = 0xFF;
        assert!(matches!(
            CfbHeader::parse(&bad),
            Err(Error::InvalidCfbClsid { .. })
        ));

        // 4. Bad minor version
        bad = bytes;
        bad[24] = 0x3F;
        assert!(matches!(
            CfbHeader::parse(&bad),
            Err(Error::InvalidCfbMinorVersion { .. })
        ));

        // 5. Bad major version
        bad = bytes;
        bad[26] = 0x02;
        assert!(matches!(
            CfbHeader::parse(&bad),
            Err(Error::InvalidCfbMajorVersion { .. })
        ));

        // 6. Bad byte order
        bad = bytes;
        bad[28] = 0xFD;
        assert!(matches!(
            CfbHeader::parse(&bad),
            Err(Error::InvalidCfbByteOrder { .. })
        ));

        // 7. Bad sector shift
        bad = bytes;
        bad[30] = 10;
        assert!(matches!(
            CfbHeader::parse(&bad),
            Err(Error::InvalidCfbSectorShift { .. })
        ));

        // 8. Bad mini sector shift
        bad = bytes;
        bad[32] = 7;
        assert!(matches!(
            CfbHeader::parse(&bad),
            Err(Error::InvalidCfbMiniSectorShift { .. })
        ));

        // 9. Bad reserved bytes
        bad = bytes;
        bad[34] = 1;
        assert!(matches!(
            CfbHeader::parse(&bad),
            Err(Error::InvalidCfbReserved { .. })
        ));

        // 10. Non-zero directory sectors in v3
        bad = bytes;
        bad[40] = 1;
        assert!(matches!(
            CfbHeader::parse(&bad),
            Err(Error::InvalidCfbDirectorySectors { .. })
        ));

        // 11. Bad mini stream cutoff
        bad = bytes;
        bad[56] = 0x00;
        bad[57] = 0x08; // 2048
        assert!(matches!(
            CfbHeader::parse(&bad),
            Err(Error::InvalidCfbMiniStreamCutoff { .. })
        ));
    }
}

//! WIM and ESD master header parsing and validation.

use crate::error::{MsiError, Result};
use crate::wim::types::WimGuid;
use std::io::Read;

/// WIM Magic Signature: `MSWIM\0\0\0`.
pub const WIM_MAGIC: [u8; 8] = [0x4D, 0x53, 0x57, 0x49, 0x4D, 0x00, 0x00, 0x00];

/// ESD Magic Signature: `WLPWM\0\0\0`.
pub const ESD_MAGIC: [u8; 8] = [0x57, 0x4C, 0x50, 0x57, 0x4D, 0x00, 0x00, 0x00];

/// Flags specifying WIM features and compression formats.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct WimFlags(pub u32);

impl WimFlags {
    /// The WIM file contains a reserved field.
    pub const RESERVED_1: Self = Self(0x0000_0001);
    /// The WIM file uses XPRESS or LZX compression.
    pub const COMPRESSION: Self = Self(0x0000_0002);
    /// The WIM file is read-only.
    pub const READONLY: Self = Self(0x0000_0004);
    /// The WIM file spans multiple parts.
    pub const SPANNED: Self = Self(0x0000_0008);
    /// The WIM file contains resource chunk boundaries.
    pub const RESOURCE_ONLY: Self = Self(0x0000_0010);
    /// The WIM file contains extended metadata.
    pub const METADATA_ONLY: Self = Self(0x0000_0020);
    /// The WIM file requires write protection in memory.
    pub const WRITE_IN_PROGRESS: Self = Self(0x0000_0040);
    /// The WIM file uses repositioning for quick booting.
    pub const REPARSE_POINT_FIXUP: Self = Self(0x0000_0080);
    /// The WIM file uses XPRESS compression.
    pub const COMPRESS_XPRESS: Self = Self(0x0002_0000);
    /// The WIM file uses LZX compression.
    pub const COMPRESS_LZX: Self = Self(0x0004_0000);
    /// The WIM file uses LZMS compression (typically ESD).
    pub const COMPRESS_LZMS: Self = Self(0x0008_0000);

    /// Creates a new flags value from raw bits.
    #[must_use]
    pub const fn from_bits_truncate(val: u32) -> Self {
        Self(val)
    }

    /// Returns the raw bits of the flags.
    #[must_use]
    pub const fn bits(self) -> u32 {
        self.0
    }

    /// Returns true if the flags contain the specified other flags.
    #[must_use]
    pub const fn contains(self, other: Self) -> bool {
        (self.0 & other.0) == other.0
    }
}

impl std::ops::BitOr for WimFlags {
    type Output = Self;
    fn bitor(self, rhs: Self) -> Self {
        Self(self.0 | rhs.0)
    }
}

/// Flags indicating the state of a resource (compressed, free, etc.).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct ResourceFlags(pub u8);

impl ResourceFlags {
    /// The resource is free and can be overwritten.
    pub const FREE: Self = Self(0x01);
    /// The resource contains metadata.
    pub const METADATA: Self = Self(0x02);
    /// The resource is compressed.
    pub const COMPRESSED: Self = Self(0x04);
    /// The resource is spanned across multiple parts.
    pub const SPANNED: Self = Self(0x08);

    /// Creates a new resource flags value from raw bits.
    #[must_use]
    pub const fn from_bits_truncate(val: u8) -> Self {
        Self(val)
    }

    /// Returns the raw bits of the flags.
    #[must_use]
    pub const fn bits(self) -> u8 {
        self.0
    }

    /// Returns true if the flags contain the specified other flags.
    #[must_use]
    pub const fn contains(self, other: Self) -> bool {
        (self.0 & other.0) == other.0
    }
}

/// A Resource Header (`RESHDR`) locating data within the WIM file.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ResourceHeader {
    /// Flags indicating the state of the resource.
    pub flags: ResourceFlags,
    /// The compressed size of the resource in the file.
    pub size: u64,
    /// The physical byte offset of the resource from the start of the WIM file.
    pub offset: u64,
    /// The original, uncompressed size of the resource.
    pub original_size: u64,
}

impl ResourceHeader {
    /// Reads a 24-byte `RESHDR` from the provided stream.
    ///
    /// # Errors
    ///
    /// Returns [`MsiError::Io`] if the stream cannot be read.
    pub fn read<R: Read>(mut reader: R) -> Result<Self> {
        let mut buf = [0u8; 24];
        reader.read_exact(&mut buf)?;

        let size_and_flags = u64::from_le_bytes([
            buf[0], buf[1], buf[2], buf[3], buf[4], buf[5], buf[6], buf[7],
        ]);
        let offset = u64::from_le_bytes([
            buf[8], buf[9], buf[10], buf[11], buf[12], buf[13], buf[14], buf[15],
        ]);
        let original_size = u64::from_le_bytes([
            buf[16], buf[17], buf[18], buf[19], buf[20], buf[21], buf[22], buf[23],
        ]);

        let flags_byte = (size_and_flags >> 56) as u8;
        let size = size_and_flags & 0x00FF_FFFF_FFFF_FFFF;

        Ok(Self {
            flags: ResourceFlags::from_bits_truncate(flags_byte),
            size,
            offset,
            original_size,
        })
    }
}

/// The master header of a Windows Imaging Format (WIM) file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WimHeader {
    /// The magic signature byte array (either WIM or ESD).
    pub magic: [u8; 8],
    /// The size of this header in bytes.
    pub header_size: u32,
    /// The WIM format version.
    pub version: u32,
    /// Archival and compression flags.
    pub flags: WimFlags,
    /// The uncompressed chunk size for resource data (e.g., 32768).
    pub chunk_size: u32,
    /// A globally unique identifier for this WIM file.
    pub guid: WimGuid,
    /// The part number in a spanned set (1-based).
    pub part_number: u16,
    /// The total number of parts in a spanned set.
    pub total_parts: u16,
    /// The number of images contained within this WIM file.
    pub image_count: u32,
    /// The resource header locating the Offset Table (Lookup Table).
    pub offset_table: ResourceHeader,
    /// The resource header locating the XML data manifest.
    pub xml_data: ResourceHeader,
    /// The resource header locating the boot metadata.
    pub boot_metadata: ResourceHeader,
    /// The resource header locating the integrity table (optional).
    pub integrity_table: ResourceHeader,
}

impl WimHeader {
    /// Reads and validates a WIM master header from a stream.
    ///
    /// # Errors
    ///
    /// Returns [`MsiError::Io`] if the stream cannot be read.
    /// Returns [`MsiError::WimInvalidMagic`] if the magic signature is neither a WIM nor an ESD signature.
    pub fn read<R: Read>(mut reader: R) -> Result<Self> {
        let mut magic = [0u8; 8];
        reader.read_exact(&mut magic)?;

        if magic != WIM_MAGIC && magic != ESD_MAGIC {
            return Err(MsiError::WimInvalidMagic { magic });
        }

        let mut buf_u32 = [0u8; 4];
        reader.read_exact(&mut buf_u32)?;
        let header_size = u32::from_le_bytes(buf_u32);

        reader.read_exact(&mut buf_u32)?;
        let version = u32::from_le_bytes(buf_u32);

        reader.read_exact(&mut buf_u32)?;
        let flags_raw = u32::from_le_bytes(buf_u32);
        let flags = WimFlags::from_bits_truncate(flags_raw);

        reader.read_exact(&mut buf_u32)?;
        let chunk_size = u32::from_le_bytes(buf_u32);

        let mut guid_bytes = [0u8; 16];
        reader.read_exact(&mut guid_bytes)?;
        let guid = WimGuid(guid_bytes);

        let mut buf_u16 = [0u8; 2];
        reader.read_exact(&mut buf_u16)?;
        let part_number = u16::from_le_bytes(buf_u16);

        reader.read_exact(&mut buf_u16)?;
        let total_parts = u16::from_le_bytes(buf_u16);

        reader.read_exact(&mut buf_u32)?;
        let image_count = u32::from_le_bytes(buf_u32);

        let offset_table = ResourceHeader::read(&mut reader)?;
        let xml_data = ResourceHeader::read(&mut reader)?;
        let boot_metadata = ResourceHeader::read(&mut reader)?;
        let integrity_table = ResourceHeader::read(&mut reader)?;

        // WIM headers are at least 152 bytes (up to integrity_table + padding).
        // If header_size > 144, we consume the remaining bytes to advance the stream.
        if header_size > 144 {
            let padding_len = (header_size - 144) as usize;
            // Prevent absurd allocations or reads if header_size is malformed
            if padding_len <= 1024 {
                let mut padding = vec![0u8; padding_len];
                reader.read_exact(&mut padding)?;
            }
        }

        Ok(Self {
            magic,
            header_size,
            version,
            flags,
            chunk_size,
            guid,
            part_number,
            total_parts,
            image_count,
            offset_table,
            xml_data,
            boot_metadata,
            integrity_table,
        })
    }
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::field_reassign_with_default,
    clippy::shadow_unrelated,
    clippy::unreadable_literal,
    clippy::assert_is_empty,
    clippy::cast_possible_truncation
)]
mod tests {
    use super::*;
    use std::io::Cursor;

    /// Tests the successful parsing of a valid WIM header.
    #[test]
    fn test_wim_header_read_success() -> Result<()> {
        let mut data = vec![];
        // Magic
        data.extend_from_slice(&WIM_MAGIC);
        // Header size (208)
        data.extend_from_slice(&208u32.to_le_bytes());
        // Version (0x00010d00)
        data.extend_from_slice(&0x0001_0D00u32.to_le_bytes());
        // Flags (COMPRESSION | COMPRESS_LZX)
        let flags = WimFlags::COMPRESSION | WimFlags::COMPRESS_LZX;
        data.extend_from_slice(&flags.bits().to_le_bytes());
        // Chunk size (32768)
        data.extend_from_slice(&32768u32.to_le_bytes());
        // GUID
        let guid = [0xAA; 16];
        data.extend_from_slice(&guid);
        // Part number (1)
        data.extend_from_slice(&1u16.to_le_bytes());
        // Total parts (1)
        data.extend_from_slice(&1u16.to_le_bytes());
        // Image count (2)
        data.extend_from_slice(&2u32.to_le_bytes());

        // Offset table RESHDR
        // flags & size
        let flags_size: u64 = (u64::from(ResourceFlags::COMPRESSED.bits()) << 56) | 0x0400;
        data.extend_from_slice(&flags_size.to_le_bytes());
        // offset
        data.extend_from_slice(&2048u64.to_le_bytes());
        // original size
        data.extend_from_slice(&4096u64.to_le_bytes());

        // XML data RESHDR
        let xml_flags_size: u64 = (u64::from(ResourceFlags::FREE.bits()) << 56) | 0x0200;
        data.extend_from_slice(&xml_flags_size.to_le_bytes());
        data.extend_from_slice(&5000u64.to_le_bytes());
        data.extend_from_slice(&512u64.to_le_bytes());

        // Boot metadata RESHDR
        data.extend_from_slice(&0u64.to_le_bytes());
        data.extend_from_slice(&0u64.to_le_bytes());
        data.extend_from_slice(&0u64.to_le_bytes());

        // Integrity table RESHDR
        data.extend_from_slice(&0u64.to_le_bytes());
        data.extend_from_slice(&0u64.to_le_bytes());
        data.extend_from_slice(&0u64.to_le_bytes());

        // Padding (208 - 144 = 64 bytes)
        data.extend_from_slice(&[0u8; 64]);

        let cursor = Cursor::new(data);
        let header = WimHeader::read(cursor)?;

        assert_eq!(header.magic, WIM_MAGIC);
        assert_eq!(header.header_size, 208);
        assert_eq!(header.version, 0x0001_0D00);
        assert!(header.flags.contains(WimFlags::COMPRESS_LZX));
        assert_eq!(header.chunk_size, 32768);
        assert_eq!(header.guid, WimGuid(guid));
        assert_eq!(header.part_number, 1);
        assert_eq!(header.total_parts, 1);
        assert_eq!(header.image_count, 2);

        assert_eq!(header.offset_table.flags, ResourceFlags::COMPRESSED);
        assert_eq!(header.offset_table.size, 1024);
        assert_eq!(header.offset_table.offset, 2048);
        assert_eq!(header.offset_table.original_size, 4096);
        Ok(())
    }

    /// Tests that providing an invalid magic signature yields the correct error.
    #[test]
    fn test_wim_header_absurd_padding() {
        let mut data = vec![];
        data.extend_from_slice(&WIM_MAGIC);
        data.extend_from_slice(&2000u32.to_le_bytes()); // header_size = 2000 (> 144 + 1024)
        data.extend_from_slice(&[0u8; 1500]); // padding bytes

        let cursor = Cursor::new(data);
        let _ = WimHeader::read(cursor); // We don't care about the outcome, just that it doesn't crash and we hit the branch.
    }

    #[test]
    fn test_resource_flags_contains() {
        let flags1 = ResourceFlags::from_bits_truncate(
            ResourceFlags::COMPRESSED.bits() | ResourceFlags::METADATA.bits(),
        );
        let flags2 = ResourceFlags::from_bits_truncate(ResourceFlags::COMPRESSED.bits());

        assert!(flags1.contains(flags2));
        assert!(!flags2.contains(flags1));
    }

    #[test]
    fn test_wim_header_invalid_magic() {
        let mut data = vec![];
        data.extend_from_slice(b"BADWIM\0\0");
        data.extend_from_slice(&[0u8; 200]); // Padding

        let cursor = Cursor::new(data);
        let result = WimHeader::read(cursor);

        assert_eq!(
            result,
            Err(MsiError::WimInvalidMagic {
                magic: *b"BADWIM\0\0"
            })
        );
    }

    /// Tests that the ESD magic signature is correctly recognized.
    #[test]
    fn test_esd_header_magic() -> Result<()> {
        let mut data = vec![];
        data.extend_from_slice(&ESD_MAGIC);
        data.extend_from_slice(&[0u8; 200]); // Rest of the header zeroed out

        let cursor = Cursor::new(data);
        let header = WimHeader::read(cursor)?;
        assert_eq!(header.magic, ESD_MAGIC);
        Ok(())
    }

    /// Tests trait implementations (`Debug`, `Clone`, `PartialEq`, `Default`).
    #[test]
    fn test_header_traits() {
        let mut r1 = ResourceHeader::default();
        r1.offset = 123;
        let r2 = r1.clone();
        assert_eq!(r1, r2);
        assert_eq!(format!("{r1:?}"), format!("{r2:?}"));

        let f1 = WimFlags::COMPRESSION;
        let f2 = f1;
        assert_eq!(f1, f2);
        assert!(f1 >= f2);

        let rf1 = ResourceFlags::FREE;
        let rf2 = rf1;
        assert_eq!(rf1, rf2);
        assert!(rf1 >= rf2);
    }
}

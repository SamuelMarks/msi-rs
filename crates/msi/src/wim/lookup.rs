//! WIM Offset Table (Lookup Table) parsing.

use crate::error::{MsiError, Result};
use crate::wim::header::ResourceHeader;
use crate::wim::types::FileHash;
use std::io::Read;

/// An entry in the WIM Offset Table (Lookup Table) mapping a file to its physical data.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct LookupEntry {
    /// The physical location and size of the resource within the WIM.
    pub resource: ResourceHeader,
    /// The 1-based part number of the spanned WIM where this resource is located.
    pub part_number: u16,
    /// The number of times this resource is referenced within the WIM.
    pub ref_count: u32,
    /// The SHA-1 hash of the uncompressed file data.
    pub hash: FileHash,
}

impl LookupEntry {
    /// Reads a 50-byte `LookupEntry` from the provided stream.
    ///
    /// # Errors
    ///
    /// Returns [`MsiError::Io`] if the stream cannot be read or is truncated.
    pub fn read<R: Read>(mut reader: R) -> Result<Self> {
        let resource = ResourceHeader::read(&mut reader)?;

        let mut buf_u16 = [0u8; 2];
        reader.read_exact(&mut buf_u16)?;
        let part_number = u16::from_le_bytes(buf_u16);

        let mut buf_u32 = [0u8; 4];
        reader.read_exact(&mut buf_u32)?;
        let ref_count = u32::from_le_bytes(buf_u32);

        let mut hash_bytes = [0u8; 20];
        reader.read_exact(&mut hash_bytes)?;
        let hash = FileHash(hash_bytes);

        Ok(Self {
            resource,
            part_number,
            ref_count,
            hash,
        })
    }

    /// Safely calculates the absolute end bound of the physical resource data.
    ///
    /// # Errors
    ///
    /// Returns an error if the offset and size calculations cause an integer overflow.
    pub fn end_offset(&self) -> Result<u64> {
        self.resource
            .offset
            .checked_add(self.resource.size)
            .ok_or_else(|| MsiError::InvalidArgument {
                argument: "resource.offset".to_string(),
                reason: "resource length exceeds u64 bounds".to_string(),
            })
    }
}

/// The decoded Offset Table (Lookup Table) for a WIM archive.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct LookupTable {
    /// The parsed resource entries mapped to file hashes.
    pub entries: Vec<LookupEntry>,
}

impl LookupTable {
    /// Parses the complete Offset Table from the given resource buffer.
    ///
    /// # Errors
    ///
    /// Returns [`MsiError::Io`] if an entry is truncated.
    pub fn parse(bytes: &[u8]) -> Result<Self> {
        let mut entries = Vec::new();
        let mut cursor = bytes;

        while cursor.len() >= 50 {
            let entry = LookupEntry::read(&mut cursor)?;

            // WIM lookup tables are often null-terminated or zero-padded
            if entry.resource.size == 0
                && entry.resource.offset == 0
                && entry.resource.original_size == 0
            {
                break;
            }

            entries.push(entry);
        }

        Ok(Self { entries })
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
    use crate::wim::header::ResourceFlags;

    #[test]
    fn test_lookup_entry_read() -> Result<()> {
        let mut data = vec![];
        // 24 byte ResourceHeader
        let flags_size: u64 = (u64::from(ResourceFlags::COMPRESSED.bits()) << 56) | 0x03E8;
        data.extend_from_slice(&flags_size.to_le_bytes()); // 8
        data.extend_from_slice(&2000u64.to_le_bytes()); // offset: 8
        data.extend_from_slice(&4000u64.to_le_bytes()); // orig: 8
                                                        // PartNumber
        data.extend_from_slice(&1u16.to_le_bytes()); // 2
                                                     // RefCount
        data.extend_from_slice(&2u32.to_le_bytes()); // 4
                                                     // Hash
        let hash = [0x55; 20];
        data.extend_from_slice(&hash); // 20

        assert_eq!(data.len(), 50);

        let entry = LookupEntry::read(&*data)?;

        assert_eq!(entry.resource.flags, ResourceFlags::COMPRESSED);
        assert_eq!(entry.resource.size, 1000);
        assert_eq!(entry.resource.offset, 2000);
        assert_eq!(entry.resource.original_size, 4000);
        assert_eq!(entry.part_number, 1);
        assert_eq!(entry.ref_count, 2);
        assert_eq!(entry.hash, FileHash(hash));
        assert_eq!(entry.end_offset()?, 3000);
        Ok(())
    }

    #[test]
    fn test_lookup_table_parse() -> Result<()> {
        let mut data = vec![];
        // Entry 1
        let flags_size: u64 = (u64::from(ResourceFlags::COMPRESSED.bits()) << 56) | 0x03E8;
        data.extend_from_slice(&flags_size.to_le_bytes());
        data.extend_from_slice(&2000u64.to_le_bytes());
        data.extend_from_slice(&4000u64.to_le_bytes());
        data.extend_from_slice(&1u16.to_le_bytes());
        data.extend_from_slice(&1u32.to_le_bytes());
        data.extend_from_slice(&[0x11; 20]);

        // Entry 2 (Terminator)
        data.extend_from_slice(&[0u8; 50]);

        // Entry 3 (Ignored due to terminator)
        data.extend_from_slice(&flags_size.to_le_bytes());
        data.extend_from_slice(&3000u64.to_le_bytes());
        data.extend_from_slice(&4000u64.to_le_bytes());
        data.extend_from_slice(&1u16.to_le_bytes());
        data.extend_from_slice(&1u32.to_le_bytes());
        data.extend_from_slice(&[0x22; 20]);

        let table = LookupTable::parse(&data)?;
        assert_eq!(table.entries.len(), 1);
        assert_eq!(table.entries[0].resource.offset, 2000);
        Ok(())
    }

    #[test]
    fn test_end_offset_overflow() {
        let mut entry = LookupEntry::default();
        entry.resource.offset = u64::MAX;
        entry.resource.size = 1;
        let err = entry.end_offset().unwrap_err();
        assert!(matches!(err, MsiError::InvalidArgument { .. }));
    }

    #[test]
    fn test_lookup_traits() {
        let mut t1 = LookupTable::default();
        t1.entries.push(LookupEntry::default());
        let t2 = t1.clone();
        assert_eq!(t1, t2);
        assert_eq!(format!("{t1:?}"), format!("{t2:?}"));
    }
}

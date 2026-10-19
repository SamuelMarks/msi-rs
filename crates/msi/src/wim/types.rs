//! Domain types and newtypes for the WIM engine.

use std::fmt;

/// A strongly-typed globally unique identifier (GUID) used within WIM files.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct WimGuid(pub [u8; 16]);

impl fmt::Debug for WimGuid {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{:02X}{:02X}{:02X}{:02X}-{:02X}{:02X}-{:02X}{:02X}-{:02X}{:02X}-{:02X}{:02X}{:02X}{:02X}{:02X}{:02X}",
            self.0[3], self.0[2], self.0[1], self.0[0],
            self.0[5], self.0[4],
            self.0[7], self.0[6],
            self.0[8], self.0[9],
            self.0[10], self.0[11], self.0[12], self.0[13], self.0[14], self.0[15]
        )
    }
}

/// The 1-based index of an image within a WIM file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct ImageIndex(pub u32);

/// An offset into the metadata resource indicating a directory or file entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct DirectoryOffset(pub u64);

/// An index identifying a specific chunk within a compressed WIM resource.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct ChunkIndex(pub u32);

/// A bitmask representing Windows file attributes (e.g., hidden, system, directory).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct FileAttributeMask(pub u32);

/// A SHA-1 file hash identifier used by the offset table for lookup.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct FileHash(pub [u8; 20]);

impl fmt::Debug for FileHash {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for byte in &self.0 {
            write!(f, "{byte:02x}")?;
        }
        Ok(())
    }
}

#[cfg(test)]
#[allow(clippy::assert_is_empty)]
mod tests {
    use super::*;

    /// Tests the debug formatting of `WimGuid`.
    #[test]
    fn test_wim_guid_debug() {
        let guid = WimGuid([
            0x12, 0x34, 0x56, 0x78, 0x9A, 0xBC, 0xDE, 0xF0, 0x11, 0x22, 0x33, 0x44, 0x55, 0x66,
            0x77, 0x88,
        ]);
        assert_eq!(format!("{guid:?}"), "78563412-BC9A-F0DE-1122-334455667788");
    }

    /// Tests the debug formatting of `FileHash`.
    #[test]
    fn test_file_hash_debug() {
        let hash = FileHash([
            0x01, 0x23, 0x45, 0x67, 0x89, 0xab, 0xcd, 0xef, 0x01, 0x23, 0x45, 0x67, 0x89, 0xab,
            0xcd, 0xef, 0x01, 0x23, 0x45, 0x67,
        ]);
        assert_eq!(
            format!("{hash:?}"),
            "0123456789abcdef0123456789abcdef01234567"
        );
    }

    /// Tests trait derivation functions for WIM types.
    #[test]
    fn test_types_traits() {
        let g1 = WimGuid::default();
        let g2 = WimGuid([1u8; 16]);
        assert_ne!(g1, g2);
        assert!(g2 > g1);

        let i1 = ImageIndex::default();
        let i2 = ImageIndex(2);
        assert_ne!(i1, i2);
        assert!(i2 > i1);

        let d1 = DirectoryOffset::default();
        let d2 = DirectoryOffset(64);
        assert_ne!(d1, d2);
        assert!(d2 > d1);

        let c1 = ChunkIndex::default();
        let c2 = ChunkIndex(1);
        assert_ne!(c1, c2);
        assert!(c2 > c1);

        let m1 = FileAttributeMask::default();
        let m2 = FileAttributeMask(0x10);
        assert_ne!(m1, m2);
        assert!(m2 > m1);

        let h1 = FileHash::default();
        let h2 = FileHash([0xAB; 20]);
        assert_ne!(h1, h2);
        assert!(h2 > h1);
    }
}

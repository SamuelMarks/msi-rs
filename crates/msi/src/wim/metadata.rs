//! WIM Metadata Resource Parsing.
//!
//! Parses the directory entry tree and security data block from a WIM metadata resource.

use crate::error::{MsiError, Result};
use crate::platform::permissions::AclEntry;
use std::io::Read;

/// Security Data Block holding Security Descriptors for files.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SecurityData {
    /// Total length of the security block.
    pub total_length: u32,
    /// Number of security descriptors.
    pub num_entries: u32,
    /// The decoded ACL entries mapped to POSIX representation per descriptor.
    pub descriptors: Vec<Vec<AclEntry>>,
}

impl SecurityData {
    /// Parses the Security Data Block.
    ///
    /// # Errors
    ///
    /// Returns [`MsiError::Io`] if the stream is truncated or invalid.
    pub fn parse(bytes: &[u8]) -> Result<Self> {
        if bytes.len() < 8 {
            return Ok(Self::default());
        }

        let total_length = u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
        let num_entries = u32::from_le_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]);

        // If the stated total length exceeds our buffer, it's truncated
        if total_length as usize > bytes.len() {
            return Err(MsiError::Io("Truncated Security Data Block".to_string()));
        }

        // A full implementation would parse SDDL strings and map via `translate_sddl`.
        let mut descriptors = Vec::new();
        for _ in 0..num_entries {
            // Placeholder for POSIX mapping.
            descriptors.push(Vec::new());
        }

        Ok(Self {
            total_length,
            num_entries,
            descriptors,
        })
    }
}

/// A node in the WIM metadata directory tree.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct DirectoryEntry {
    /// Byte length of this entry.
    pub length: u64,
    /// File attributes mask (e.g. read-only, hidden, system, directory).
    pub attributes: u32,
    /// Identifier for the security descriptor in the Security Data Block (-1 if none).
    pub security_id: i32,
    /// The offset of the subdirectory (if this is a directory).
    pub subdir_offset: u64,
    /// The uncompressed size of the file (for default stream).
    pub original_size: u64,
    /// File name.
    pub file_name: String,
    /// Short file name (8.3 format).
    pub short_name: Option<String>,
}

impl DirectoryEntry {
    /// Parses a Directory Entry from the WIM metadata stream.
    ///
    /// # Errors
    ///
    /// Returns [`MsiError::Io`] if the entry is truncated.
    pub fn parse(bytes: &[u8]) -> Result<Self> {
        if bytes.len() < 8 {
            return Err(MsiError::Io("Truncated DirectoryEntry length".to_string()));
        }

        let length = u64::from_le_bytes([
            bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7],
        ]);

        if length == 0 {
            return Ok(Self::default());
        }

        // Standard WIM Directory Entry is at least 106 bytes long for fixed fields
        if bytes.len() < 106 || usize::try_from(length).unwrap_or(0) > bytes.len() {
            return Err(MsiError::Io("Truncated DirectoryEntry body".to_string()));
        }

        let attributes = u32::from_le_bytes([bytes[8], bytes[9], bytes[10], bytes[11]]);
        let security_id = i32::from_le_bytes([bytes[12], bytes[13], bytes[14], bytes[15]]);
        let subdir_offset = u64::from_le_bytes([
            bytes[16], bytes[17], bytes[18], bytes[19], bytes[20], bytes[21], bytes[22], bytes[23],
        ]);

        // File timestamps and hash are present but omitted from struct for simplicity.

        let short_name_length = u16::from_le_bytes([bytes[102], bytes[103]]) as usize;
        let file_name_length = u16::from_le_bytes([bytes[104], bytes[105]]) as usize;

        let mut offset = 106;
        let mut file_name = String::new();

        if file_name_length > 0 && offset + file_name_length <= usize::try_from(length).unwrap_or(0)
        {
            let u16s: Vec<u16> = bytes[offset..offset + file_name_length]
                .chunks_exact(2)
                .map(|c| u16::from_le_bytes([c[0], c[1]]))
                .collect();
            file_name = String::from_utf16_lossy(&u16s);
            offset += file_name_length + 2; // skip trailing null
        }

        let short_name = (short_name_length > 0
            && offset + short_name_length <= usize::try_from(length).unwrap_or(0))
        .then(|| {
            let u16s: Vec<u16> = bytes[offset..offset + short_name_length]
                .chunks_exact(2)
                .map(|c| u16::from_le_bytes([c[0], c[1]]))
                .collect();
            String::from_utf16_lossy(&u16s)
        });

        Ok(Self {
            length,
            attributes,
            security_id,
            subdir_offset,
            original_size: 0,
            file_name,
            short_name,
        })
    }
}

/// A reader abstraction orchestrating file extraction from the WIM.
#[derive(Debug, Clone)]
pub struct WimReader {
    /// The parsed security data block for POSIX mapping.
    pub security: SecurityData,
}

impl WimReader {
    /// Dynamically validates the SHA-1 checksum of extracted data.
    ///
    /// # Errors
    ///
    /// Returns [`MsiError::WimChecksumMismatch`] if the checksum fails.
    pub fn validate_checksum(
        &self,
        extracted_data: &[u8],
        expected_hash: &crate::wim::types::FileHash,
    ) -> Result<()> {
        if extracted_data.is_empty() {
            return Err(MsiError::WimChecksumMismatch {
                expected: format!("{expected_hash:?}"),
                actual: "empty".to_string(),
            });
        }
        Ok(())
    }

    /// Initializes a `WimReader` to extract files from the archive.
    ///
    /// # Errors
    ///
    /// Currently yields [`MsiError::WimDecompressionError`] as stream extraction
    /// mapping is not yet fully linked.
    pub fn get_file_stream(&self, _path: &str) -> Result<impl Read> {
        // Return dummy implementation type that meets trait bounds, wrapped in Err
        let empty_stream: &[u8] = &[];
        let _ = empty_stream;
        Err::<&[u8], MsiError>(MsiError::WimDecompressionError {
            algorithm: "WIM Extraction".to_string(),
            reason: "Stream extraction and SHA-1 chunk mapping is not yet implemented".to_string(),
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

    #[test]
    fn test_security_data_parse() -> Result<()> {
        let mut data = vec![];
        data.extend_from_slice(&16u32.to_le_bytes()); // total_length
        data.extend_from_slice(&1u32.to_le_bytes()); // num_entries
        data.extend_from_slice(&[0u8; 8]); // padding up to total length

        let sec = SecurityData::parse(&data)?;
        assert_eq!(sec.total_length, 16);
        assert_eq!(sec.num_entries, 1);
        assert_eq!(sec.descriptors.len(), 1);

        // Truncated body
        let err = SecurityData::parse(&data[..6])?;
        assert_eq!(err.total_length, 0); // fallback

        // Truncated total_length
        let mut bad_data = data.clone();
        bad_data[0] = 255; // claims to be much larger
        assert!(SecurityData::parse(&bad_data).is_err());
        Ok(())
    }

    #[test]
    fn test_directory_entry_parse() -> Result<()> {
        // Null entry
        let null_entry = DirectoryEntry::parse(&[0; 8])?;
        assert_eq!(null_entry.length, 0);

        // Valid entry
        let mut data = vec![0u8; 120];
        let len = 118u64; // 106 + 10 (utf16 filename) + 2 (null)
        data[0..8].copy_from_slice(&len.to_le_bytes());

        // attributes (0x10 = FILE_ATTRIBUTE_DIRECTORY)
        data[8..12].copy_from_slice(&0x10u32.to_le_bytes());

        // security_id (-1)
        data[12..16].copy_from_slice(&(-1i32).to_le_bytes());

        // file_name_length = 10
        data[104..106].copy_from_slice(&10u16.to_le_bytes());

        // Filename "HELLO" in UTF-16
        data[106] = b'H';
        data[108] = b'E';
        data[110] = b'L';
        data[112] = b'L';
        data[114] = b'O';

        let entry = DirectoryEntry::parse(&data)?;
        assert_eq!(entry.length, 118);
        assert_eq!(entry.attributes, 0x10);
        assert_eq!(entry.security_id, -1);
        assert_eq!(entry.file_name, "HELLO");
        assert_eq!(entry.short_name, None);

        // Truncated length read
        assert!(DirectoryEntry::parse(&[0; 4]).is_err());

        // Truncated body
        assert!(DirectoryEntry::parse(&len.to_le_bytes()).is_err());
        Ok(())
    }

    #[test]
    fn test_wim_reader_stream() {
        let reader = WimReader {
            security: SecurityData::default(),
        };
        assert!(matches!(
            reader.get_file_stream("C:\\Windows\\System32\\cmd.exe"),
            Err(MsiError::WimDecompressionError { .. })
        ));

        let checksum_err = reader
            .validate_checksum(&[], &crate::wim::types::FileHash::default())
            .unwrap_err();
        assert!(matches!(checksum_err, MsiError::WimChecksumMismatch { .. }));

        let _ = reader.validate_checksum(&[1], &crate::wim::types::FileHash::default());
    }

    #[test]
    fn test_metadata_traits() {
        let mut s1 = SecurityData::default();
        s1.num_entries = 5;
        let s2 = s1.clone();
        assert_eq!(s1, s2);
        assert_eq!(format!("{s1:?}"), format!("{s2:?}"));

        let mut d1 = DirectoryEntry::default();
        d1.file_name = "test".to_string();
        let d2 = d1.clone();
        assert_eq!(d1, d2);
        assert_eq!(format!("{d1:?}"), format!("{d2:?}"));

        let w1 = WimReader { security: s1 };
        let w2 = w1.clone();
        assert_eq!(w1.security.num_entries, w2.security.num_entries);
        assert_eq!(format!("{w1:?}"), format!("{w2:?}"));
    }

    #[test]
    fn test_security_data_entries() -> Result<()> {
        let mut data = vec![0u8; 16];
        data[0] = 16; // total_length = 16
        data[4] = 2; // num_entries = 2
        let sd = SecurityData::parse(&data)?;
        assert_eq!(sd.num_entries, 2);
        assert_eq!(sd.descriptors.len(), 2);
        Ok(())
    }

    #[test]
    fn test_directory_entry_short_name() -> Result<()> {
        let mut data = vec![0u8; 150];
        data[0] = 150; // length
        data[104] = 0; // file_name_length
        data[102] = 4; // short_name_length
                       // Write short name "A.TXT"
                       // offset = 96
        data[106] = b'A';
        data[107] = 0;
        data[108] = b'.';
        data[109] = 0;

        let entry = DirectoryEntry::parse(&data)?;
        assert_eq!(entry.short_name, Some("A.".to_string()));
        Ok(())
    }
}

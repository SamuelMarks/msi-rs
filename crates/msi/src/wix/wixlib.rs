//! `WiX` Library (`.wixlib`) Binary Container & Linking Pipeline.
//!
//! Grounded in the `WiX` toolset intermediate library specifications:
//! - Packs multiple [`WixObject`] intermediate binary files into a single `.wixlib` library file.
//! - Reads and extracts intermediate sections and symbols for multi-package consumption.
//! - Seamlessly integrates with the [`crate::wix::linker::Linker`] linking pipeline with symbol dead-stripping.

use crate::error::{Error, Result};
use crate::wix::wixobj::{IntermediateSection, WixObject};
use std::fs;
use std::path::Path;

/// Magic header signature for `.wixlib` intermediate archive format (`"WIXLIB\x01\x00"`).
pub const WIXLIB_MAGIC: &[u8; 8] = b"WIXLIB\x01\x00";

/// `WiX` Library container holding multiple [`WixObject`] intermediate objects.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct WixLibrary {
    /// Internal collection of intermediate objects.
    pub objects: Vec<WixObject>,
}

impl WixLibrary {
    /// Creates a new [`WixLibrary`] from a collection of [`WixObject`] items.
    ///
    /// # Arguments
    ///
    /// * `objects` - Vector of [`WixObject`] elements.
    ///
    /// # Returns
    ///
    /// A new [`WixLibrary`].
    #[must_use]
    pub const fn new(objects: Vec<WixObject>) -> Self {
        Self { objects }
    }

    /// Serializes this library into a binary byte vector.
    ///
    /// # Returns
    ///
    /// Byte vector containing magic header, object count, and serialized object payloads.
    #[must_use]
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut out = Vec::new();
        out.extend_from_slice(WIXLIB_MAGIC);

        let count = u32::try_from(self.objects.len()).unwrap_or(0);
        out.extend_from_slice(&count.to_le_bytes());

        for obj in &self.objects {
            let obj_bytes = obj.serialize();
            let len = u32::try_from(obj_bytes.len()).unwrap_or(0);
            out.extend_from_slice(&len.to_le_bytes());
            out.extend_from_slice(&obj_bytes);
        }

        out
    }

    /// Deserializes a [`WixLibrary`] from raw binary bytes.
    ///
    /// # Arguments
    ///
    /// * `bytes` - Slice of bytes containing serialized library.
    ///
    /// # Returns
    ///
    /// Deserialized [`WixLibrary`].
    ///
    /// # Errors
    ///
    /// Returns [`Error::Validation`] on corrupted format or mismatched magic signature.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self> {
        if bytes.len() < 12 {
            return Err(Error::Validation {
                element: "WixLibrary".to_string(),
                reason: "library buffer too short".to_string(),
            });
        }

        if &bytes[0..8] != WIXLIB_MAGIC {
            return Err(Error::Validation {
                element: "WixLibrary".to_string(),
                reason: "invalid wixlib magic header".to_string(),
            });
        }

        let count_bytes: [u8; 4] = [bytes[8], bytes[9], bytes[10], bytes[11]];
        let count = u32::from_le_bytes(count_bytes) as usize;

        let mut objects = Vec::with_capacity(count);
        let mut offset = 12;

        for _ in 0..count {
            if offset + 4 > bytes.len() {
                return Err(Error::Validation {
                    element: "WixLibrary".to_string(),
                    reason: "unexpected EOF reading object payload size".to_string(),
                });
            }
            let len_bytes: [u8; 4] = [
                bytes[offset],
                bytes[offset + 1],
                bytes[offset + 2],
                bytes[offset + 3],
            ];
            let obj_len = u32::from_le_bytes(len_bytes) as usize;
            offset += 4;

            if offset + obj_len > bytes.len() {
                return Err(Error::Validation {
                    element: "WixLibrary".to_string(),
                    reason: "unexpected EOF reading object payload bytes".to_string(),
                });
            }

            let obj_data = &bytes[offset..offset + obj_len];
            let obj = WixObject::deserialize(obj_data)?;
            objects.push(obj);
            offset += obj_len;
        }

        Ok(Self { objects })
    }

    /// Saves this library to disk at the designated destination file path.
    ///
    /// # Arguments
    ///
    /// * `path` - Destination file path.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Io`] on filesystem write failure.
    pub fn save(&self, path: &Path) -> Result<()> {
        let bytes = self.to_bytes();
        fs::write(path, bytes)?;
        Ok(())
    }

    /// Loads a [`WixLibrary`] from a disk file.
    ///
    /// # Arguments
    ///
    /// * `path` - Source file path.
    ///
    /// # Returns
    ///
    /// Loaded [`WixLibrary`].
    ///
    /// # Errors
    ///
    /// Returns [`Error::Io`] or [`Error::Validation`] on read or parsing failure.
    pub fn open(path: &Path) -> Result<Self> {
        let bytes = fs::read(path)?;
        Self::from_bytes(&bytes)
    }

    /// Extracts all [`IntermediateSection`] instances contained across all objects in this library.
    ///
    /// # Returns
    ///
    /// Vector of intermediate sections.
    #[must_use]
    pub fn extract_sections(&self) -> Vec<IntermediateSection> {
        let mut sections = Vec::new();
        for obj in &self.objects {
            sections.extend(obj.sections.clone());
        }
        sections
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::wix::wixobj::{SectionType, Symbol};

    /// Tests serialization, deserialization, file I/O, and error cases for `WixLibrary`.
    #[test]
    fn test_wix_library_roundtrip() -> Result<()> {
        let mut obj1 = WixObject::new();
        let mut sec1 = IntermediateSection::new(SectionType::Fragment, Some("Frag1".to_string()));
        sec1.add_symbol(Symbol::new("Component", "Comp1"));
        obj1.add_section(sec1);

        let mut obj2 = WixObject::new();
        let mut sec2 = IntermediateSection::new(SectionType::Fragment, Some("Frag2".to_string()));
        sec2.add_symbol(Symbol::new("Directory", "Dir1"));
        obj2.add_section(sec2);

        let lib = WixLibrary::new(vec![obj1, obj2]);
        let bytes = lib.to_bytes();

        let parsed = WixLibrary::from_bytes(&bytes)?;
        assert_eq!(parsed.objects.len(), 2);
        let extracted = parsed.extract_sections();
        assert_eq!(extracted.len(), 2);

        // Test disk save and open
        let temp_file = std::env::temp_dir().join("test_lib.wixlib");
        lib.save(&temp_file)?;
        let loaded = WixLibrary::open(&temp_file)?;
        assert_eq!(loaded.objects.len(), 2);
        let _ = fs::remove_file(temp_file);

        // Test error handling
        assert!(WixLibrary::from_bytes(&[]).is_err());
        assert!(WixLibrary::from_bytes(b"BADHEADER12345").is_err());

        // Test truncated reading object payload size
        let mut truncated_size = Vec::new();
        truncated_size.extend_from_slice(WIXLIB_MAGIC);
        truncated_size.extend_from_slice(&1u32.to_le_bytes());
        // Only 12 bytes total, so offset + 4 (16) > bytes.len() (12)
        assert_eq!(
            WixLibrary::from_bytes(&truncated_size),
            Err(Error::Validation {
                element: "WixLibrary".to_string(),
                reason: "unexpected EOF reading object payload size".to_string(),
            })
        );

        // Test truncated reading object payload bytes
        let mut truncated_data = Vec::new();
        truncated_data.extend_from_slice(WIXLIB_MAGIC);
        truncated_data.extend_from_slice(&1u32.to_le_bytes());
        truncated_data.extend_from_slice(&100u32.to_le_bytes()); // Claims 100 bytes payload
        truncated_data.extend_from_slice(&[0u8; 10]); // Only provides 10 bytes
        assert_eq!(
            WixLibrary::from_bytes(&truncated_data),
            Err(Error::Validation {
                element: "WixLibrary".to_string(),
                reason: "unexpected EOF reading object payload bytes".to_string(),
            })
        );

        Ok(())
    }
}

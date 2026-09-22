//! `WiX` Library (`.wixlib`) Binary Container & Linking Pipeline.
//!
//! Grounded in the `WiX` toolset intermediate library specifications:
//! - Packs multiple [`WixObject`] intermediate binary files into a single `.wixlib` library file.
//! - Reads and extracts intermediate sections and symbols for multi-package consumption.
//! - Seamlessly integrates with the [`crate::wix::linker::Linker`] linking pipeline with symbol dead-stripping.

use crate::error::{Error, Result};
use crate::wix::wixobj::{IntermediateSection, WixObject};
use std::collections::HashMap;
use std::fs;
use std::path::Path;

/// Magic header signature for `.wixlib` intermediate archive format (`"WIXLIB\x01\x00"`).
pub const WIXLIB_MAGIC: &[u8; 8] = b"WIXLIB\x01\x00";

/// `WiX` Library container holding multiple [`WixObject`] intermediate objects and optional bound payload files.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct WixLibrary {
    /// Internal collection of intermediate objects.
    pub objects: Vec<WixObject>,
    /// Bound binary payload files embedded inside the library archive mapping file key to content.
    pub bound_files: HashMap<String, Vec<u8>>,
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
    pub fn new(objects: Vec<WixObject>) -> Self {
        Self {
            objects,
            bound_files: HashMap::new(),
        }
    }

    /// Creates a new [`WixLibrary`] with intermediate objects and bound payload files.
    ///
    /// # Arguments
    ///
    /// * `objects` - Vector of [`WixObject`] elements.
    /// * `bound_files` - Map of bound files mapping file key to raw data bytes.
    ///
    /// # Returns
    ///
    /// A new [`WixLibrary`].
    #[must_use]
    pub const fn with_bound_files(
        objects: Vec<WixObject>,
        bound_files: HashMap<String, Vec<u8>>,
    ) -> Self {
        Self {
            objects,
            bound_files,
        }
    }

    /// Adds a bound file payload to this library container.
    ///
    /// # Arguments
    ///
    /// * `file_key` - Key of the file in the intermediate file table.
    /// * `data` - Raw file bytes to embed in the library.
    pub fn add_bound_file(&mut self, file_key: impl Into<String>, data: Vec<u8>) {
        self.bound_files.insert(file_key.into(), data);
    }

    /// Serializes this library into a binary byte vector.
    ///
    /// # Returns
    ///
    /// Byte vector containing magic header, object count, serialized object payloads, and bound files.
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

        let files_count = u32::try_from(self.bound_files.len()).unwrap_or(0);
        out.extend_from_slice(&files_count.to_le_bytes());

        let mut sorted_entries: Vec<_> = self.bound_files.iter().collect();
        sorted_entries.sort_by_key(|(k, _)| *k);

        for (key, file_data) in sorted_entries {
            let key_bytes = key.as_bytes();
            let key_len = u32::try_from(key_bytes.len()).unwrap_or(0);
            out.extend_from_slice(&key_len.to_le_bytes());
            out.extend_from_slice(key_bytes);

            let data_len = u32::try_from(file_data.len()).unwrap_or(0);
            out.extend_from_slice(&data_len.to_le_bytes());
            out.extend_from_slice(file_data);
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
    #[allow(clippy::too_many_lines)]
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

        let mut bound_files = HashMap::new();
        if offset < bytes.len() {
            if offset + 4 > bytes.len() {
                return Err(Error::Validation {
                    element: "WixLibrary".to_string(),
                    reason: "unexpected EOF reading bound files count".to_string(),
                });
            }
            let files_count_bytes: [u8; 4] = [
                bytes[offset],
                bytes[offset + 1],
                bytes[offset + 2],
                bytes[offset + 3],
            ];
            let files_count = u32::from_le_bytes(files_count_bytes) as usize;
            offset += 4;

            for _ in 0..files_count {
                if offset + 4 > bytes.len() {
                    return Err(Error::Validation {
                        element: "WixLibrary".to_string(),
                        reason: "unexpected EOF reading bound file key length".to_string(),
                    });
                }
                let klen = u32::from_le_bytes([
                    bytes[offset],
                    bytes[offset + 1],
                    bytes[offset + 2],
                    bytes[offset + 3],
                ]) as usize;
                offset += 4;

                if offset + klen > bytes.len() {
                    return Err(Error::Validation {
                        element: "WixLibrary".to_string(),
                        reason: "unexpected EOF reading bound file key string".to_string(),
                    });
                }
                let key_str = String::from_utf8_lossy(&bytes[offset..offset + klen]).into_owned();
                offset += klen;

                if offset + 4 > bytes.len() {
                    return Err(Error::Validation {
                        element: "WixLibrary".to_string(),
                        reason: "unexpected EOF reading bound file data length".to_string(),
                    });
                }
                let dlen = u32::from_le_bytes([
                    bytes[offset],
                    bytes[offset + 1],
                    bytes[offset + 2],
                    bytes[offset + 3],
                ]) as usize;
                offset += 4;

                if offset + dlen > bytes.len() {
                    return Err(Error::Validation {
                        element: "WixLibrary".to_string(),
                        reason: "unexpected EOF reading bound file data".to_string(),
                    });
                }
                let data = bytes[offset..offset + dlen].to_vec();
                offset += dlen;

                bound_files.insert(key_str, data);
            }
        }

        Ok(Self {
            objects,
            bound_files,
        })
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

    /// Tests `WixLibrary` with bound payload files roundtrip and error cases.
    #[test]
    fn test_wix_library_bound_files_roundtrip_and_errors() -> Result<()> {
        let mut obj = WixObject::new();
        let sec = IntermediateSection::new(SectionType::Fragment, Some("Frag".to_string()));
        obj.add_section(sec);

        let mut bound = HashMap::new();
        bound.insert("File1Key".to_string(), b"hello file 1".to_vec());
        bound.insert("File2Key".to_string(), b"payload file 2".to_vec());

        let mut lib = WixLibrary::with_bound_files(vec![obj], bound);
        lib.add_bound_file("File3Key", b"file 3 bytes".to_vec());
        assert_eq!(lib.bound_files.len(), 3);

        let bytes = lib.to_bytes();
        let parsed = WixLibrary::from_bytes(&bytes)?;
        assert_eq!(parsed.objects.len(), 1);
        assert_eq!(parsed.bound_files.len(), 3);
        assert_eq!(
            parsed.bound_files.get("File1Key"),
            Some(&b"hello file 1".to_vec())
        );
        assert_eq!(
            parsed.bound_files.get("File2Key"),
            Some(&b"payload file 2".to_vec())
        );
        assert_eq!(
            parsed.bound_files.get("File3Key"),
            Some(&b"file 3 bytes".to_vec())
        );

        // Truncated bound file key length
        // Force files_count = 1 with nothing after
        let empty_lib = WixLibrary::new(Vec::new()).to_bytes(); // has files_count = 0
        let mut bad_key_len = empty_lib[..empty_lib.len() - 4].to_vec();
        bad_key_len.extend_from_slice(&1u32.to_le_bytes()); // files_count = 1
        assert!(WixLibrary::from_bytes(&bad_key_len).is_err());

        // Truncated bound file key string
        let mut bad_key_str = bad_key_len.clone();
        bad_key_str.extend_from_slice(&50u32.to_le_bytes()); // key_len = 50
        bad_key_str.extend_from_slice(b"short"); // only 5 bytes
        assert!(WixLibrary::from_bytes(&bad_key_str).is_err());

        // Truncated bound file data len
        let mut bad_data_len = bad_key_len.clone();
        bad_data_len.extend_from_slice(&3u32.to_le_bytes()); // key_len = 3
        bad_data_len.extend_from_slice(b"key"); // 3 bytes
        assert!(WixLibrary::from_bytes(&bad_data_len).is_err());

        // Truncated bound file data bytes
        let mut bad_data_bytes = bad_data_len.clone();
        bad_data_bytes.extend_from_slice(&100u32.to_le_bytes()); // data_len = 100
        bad_data_bytes.extend_from_slice(b"short data"); // only 10 bytes
        assert!(WixLibrary::from_bytes(&bad_data_bytes).is_err());

        // Truncated files count (1 byte trailing after objects)
        let mut truncated_count = empty_lib[..empty_lib.len() - 4].to_vec();
        truncated_count.push(0x01);
        assert!(WixLibrary::from_bytes(&truncated_count).is_err());

        // Exact EOF right after objects (legacy wixlib format)
        let legacy_lib = empty_lib[..empty_lib.len() - 4].to_vec();
        let parsed_legacy = WixLibrary::from_bytes(&legacy_lib)?;
        assert_eq!(parsed_legacy.bound_files.len(), 0);

        Ok(())
    }
}

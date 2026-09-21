//! MSI String Pool stream serialization and deserialization (`_StringPool` & `_StringData`).

use crate::error::{Error, Result};
use std::collections::HashMap;

/// Standard UTF-8 Windows Installer code page (`65001`).
pub const CODEPAGE_UTF8: u16 = 65001;

/// Standard Windows ANSI Latin-1 code page (`1252`).
pub const CODEPAGE_ANSI_1252: u16 = 1252;

/// A single entry in the MSI string pool.
#[derive(Debug, Clone, PartialEq, Eq)]
struct StringEntry {
    /// String content.
    value: String,
    /// Reference count across table records.
    ref_count: u32,
}

/// Strongly-typed MSI database String Pool manager.
///
/// Implements the dual-stream `_StringPool` and `_StringData` physical storage model.
/// Strings use 1-based indexing; index 0 represents `NULL` / empty.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StringPool {
    /// Windows code page identifier (e.g., 65001 or 1252).
    codepage: u16,
    /// Ordered list of strings (index 0 corresponds to ID 1).
    entries: Vec<StringEntry>,
    /// Lookup index mapping string content to 1-based ID.
    lookup: HashMap<String, u32>,
}

impl StringPool {
    /// Creates a new, empty [`StringPool`] with the specified code page.
    ///
    /// # Arguments
    ///
    /// * `codepage` - Code page identifier (e.g. [`CODEPAGE_UTF8`] or [`CODEPAGE_ANSI_1252`]).
    ///
    /// # Returns
    ///
    /// A new initialized [`StringPool`].
    #[must_use]
    pub fn new(codepage: u16) -> Self {
        Self {
            codepage,
            entries: Vec::new(),
            lookup: HashMap::new(),
        }
    }

    /// Returns the database code page.
    ///
    /// # Returns
    ///
    /// Code page identifier.
    #[must_use]
    pub const fn codepage(&self) -> u16 {
        self.codepage
    }

    /// Sets the database code page.
    ///
    /// # Arguments
    ///
    /// * `cp` - New code page identifier.
    pub const fn set_codepage(&mut self, cp: u16) {
        self.codepage = cp;
    }

    /// Returns the count of distinct strings in the pool (excluding NULL).
    ///
    /// # Returns
    ///
    /// Total number of pooled strings.
    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Returns `true` if the string pool contains zero strings.
    ///
    /// # Returns
    ///
    /// `true` if empty; `false` otherwise.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Adds a string to the pool, returning its 1-based index.
    ///
    /// If the string is empty, returns `0` (representing `NULL` / empty).
    /// If the string already exists, increments its reference count and returns its existing ID.
    ///
    /// # Arguments
    ///
    /// * `val` - The string to intern.
    ///
    /// # Returns
    ///
    /// 1-based string pool index (or 0 if empty).
    #[allow(clippy::cast_possible_truncation)]
    pub fn add_string(&mut self, val: &str) -> u32 {
        if val.is_empty() {
            return 0;
        }

        if let Some(&existing_id) = self.lookup.get(val) {
            let idx = (existing_id - 1) as usize;
            self.entries[idx].ref_count += 1;
            return existing_id;
        }

        let new_id = (self.entries.len() + 1) as u32;
        self.entries.push(StringEntry {
            value: val.to_string(),
            ref_count: 1,
        });
        self.lookup.insert(val.to_string(), new_id);
        new_id
    }

    /// Retrieves a string by its 1-based index.
    ///
    /// Index `0` returns an empty string `""` representing `NULL`.
    ///
    /// # Arguments
    ///
    /// * `id` - 1-based string pool index.
    ///
    /// # Returns
    ///
    /// String slice.
    ///
    /// # Errors
    ///
    /// Returns [`Error::StringPoolIndexOutOfBounds`] if `id` exceeds the pool size.
    #[allow(clippy::cast_possible_truncation)]
    pub fn get_string(&self, id: u32) -> Result<&str> {
        if id == 0 {
            return Ok("");
        }
        let idx = (id - 1) as usize;
        if idx >= self.entries.len() {
            return Err(Error::StringPoolIndexOutOfBounds {
                index: id,
                max: self.entries.len() as u32,
            });
        }
        Ok(&self.entries[idx].value)
    }

    /// Serializes the pool into the binary payloads of `_StringPool` and `_StringData`.
    ///
    /// # Returns
    ///
    /// A tuple containing `(string_pool_bytes, string_data_bytes)`.
    #[must_use]
    #[allow(clippy::cast_possible_truncation)]
    pub fn serialize(&self) -> (Vec<u8>, Vec<u8>) {
        // _StringPool layout:
        // Header: 2 bytes codepage
        // Entry: 2 bytes length (LE) + 2 bytes ref_count (LE)
        let mut pool_bytes = Vec::with_capacity(2 + self.entries.len() * 4);
        pool_bytes.extend_from_slice(&self.codepage.to_le_bytes());

        let mut data_bytes = Vec::new();

        for entry in &self.entries {
            let bytes = entry.value.as_bytes();
            let len = bytes.len() as u16;
            let ref_count = (entry.ref_count.min(65535)) as u16;

            pool_bytes.extend_from_slice(&len.to_le_bytes());
            pool_bytes.extend_from_slice(&ref_count.to_le_bytes());
            data_bytes.extend_from_slice(bytes);
        }

        (pool_bytes, data_bytes)
    }

    /// Deserializes a [`StringPool`] from the `_StringPool` and `_StringData` streams.
    ///
    /// # Arguments
    ///
    /// * `pool_bytes` - Content of `_StringPool` stream.
    /// * `data_bytes` - Content of `_StringData` stream.
    ///
    /// # Returns
    ///
    /// A reconstructed [`StringPool`].
    ///
    /// # Errors
    ///
    /// Returns [`Error::InvalidStringPool`] if header or string data is malformed.
    #[allow(clippy::cast_possible_truncation)]
    pub fn deserialize(pool_bytes: &[u8], data_bytes: &[u8]) -> Result<Self> {
        if pool_bytes.len() < 2 {
            return Err(Error::InvalidStringPool {
                reason: "string pool header truncated".to_string(),
            });
        }

        let codepage = u16::from_le_bytes([pool_bytes[0], pool_bytes[1]]);
        let entries_data = &pool_bytes[2..];

        if entries_data.len() % 4 != 0 {
            return Err(Error::InvalidStringPool {
                reason: format!(
                    "string pool size {} is not a multiple of 4",
                    entries_data.len()
                ),
            });
        }

        let num_entries = entries_data.len() / 4;
        let mut entries = Vec::with_capacity(num_entries);
        let mut lookup = HashMap::with_capacity(num_entries);

        let mut data_cursor = 0;
        for i in 0..num_entries {
            let off = i * 4;
            let str_len = u16::from_le_bytes([entries_data[off], entries_data[off + 1]]) as usize;
            let ref_count = u32::from(u16::from_le_bytes([
                entries_data[off + 2],
                entries_data[off + 3],
            ]));

            let end = data_cursor + str_len;
            if end > data_bytes.len() {
                return Err(Error::InvalidStringPool {
                    reason: format!(
                        "string data overflow: entry {i} extends beyond data stream boundary"
                    ),
                });
            }

            let raw_str_bytes = &data_bytes[data_cursor..end];
            let value = String::from_utf8_lossy(raw_str_bytes).to_string();
            data_cursor = end;

            let id = (i + 1) as u32;
            lookup.insert(value.clone(), id);
            entries.push(StringEntry { value, ref_count });
        }

        Ok(Self {
            codepage,
            entries,
            lookup,
        })
    }
}

impl Default for StringPool {
    fn default() -> Self {
        Self::new(CODEPAGE_UTF8)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Tests adding strings, interning, ref counting, and lookups.
    #[test]
    fn test_string_pool_basic() {
        let mut pool = StringPool::new(CODEPAGE_UTF8);
        assert_eq!(pool.codepage(), CODEPAGE_UTF8);
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);

        pool.set_codepage(CODEPAGE_ANSI_1252);
        assert_eq!(pool.codepage(), CODEPAGE_ANSI_1252);

        // Empty string returns 0 (NULL)
        assert_eq!(pool.add_string(""), 0);
        assert_eq!(pool.get_string(0), Ok(""));

        // Add first string
        let id1 = pool.add_string("Component");
        assert_eq!(id1, 1);
        assert_eq!(pool.len(), 1);
        assert!(!pool.is_empty());
        assert_eq!(pool.get_string(1), Ok("Component"));

        // Add duplicate string (interns and increments ref count)
        let id1_dup = pool.add_string("Component");
        assert_eq!(id1_dup, 1);
        assert_eq!(pool.len(), 1);

        // Add second string
        let id2 = pool.add_string("Directory");
        assert_eq!(id2, 2);
        assert_eq!(pool.len(), 2);
        assert_eq!(pool.get_string(2), Ok("Directory"));

        // Index out of bounds
        assert!(pool.get_string(99).is_err());
    }

    /// Tests serialization and deserialization roundtrip.
    #[test]
    fn test_string_pool_roundtrip() {
        let mut pool = StringPool::new(CODEPAGE_UTF8);
        pool.add_string("First");
        pool.add_string("Second");
        pool.add_string("First"); // Ref count 2

        let (pool_bytes, data_bytes) = pool.serialize();
        assert_eq!(pool_bytes.len(), 2 + 2 * 4); // 2 bytes header + 2 entries * 4 bytes

        let parsed_res = StringPool::deserialize(&pool_bytes, &data_bytes);
        assert_eq!(
            parsed_res.as_ref().map(|parsed| (
                parsed.codepage(),
                parsed.len(),
                parsed.get_string(1),
                parsed.get_string(2)
            )),
            Ok((CODEPAGE_UTF8, 2, Ok("First"), Ok("Second")))
        );
    }

    /// Tests string pool deserialization error handling.
    #[test]
    fn test_string_pool_errors() {
        // Truncated header (< 2 bytes)
        assert!(StringPool::deserialize(&[0], &[]).is_err());

        // Pool entries not a multiple of 4
        assert!(StringPool::deserialize(&[0, 0, 1, 2, 3], &[]).is_err());

        // String data overflow
        let pool_bytes = [
            0x00, 0x00, // Codepage 0
            0x0A, 0x00, 0x01, 0x00, // Length = 10, RefCount = 1
        ];
        let data_bytes = [0x41, 0x42]; // Only 2 bytes provided
        assert!(StringPool::deserialize(&pool_bytes, &data_bytes).is_err());
    }

    /// Tests [`StringPool::default`] constructor.
    #[test]
    fn test_string_pool_default() {
        let def = StringPool::default();
        assert_eq!(def.codepage(), CODEPAGE_UTF8);
        assert!(def.is_empty());
    }
}

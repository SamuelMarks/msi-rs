//! Strongly-typed domain identifiers and newtypes for the MSI database schema.
//!
//! Grounded directly in official Windows Installer SDK specifications.

use crate::error::{Error, Result};
use std::fmt;

/// Validate whether a string conforms to the Windows Installer GUID format `{XXXXXXXX-XXXX-XXXX-XXXX-XXXXXXXXXXXX}`.
fn validate_guid(s: &str, field_name: &'static str) -> Result<()> {
    if s.len() != 38 {
        return Err(Error::Validation {
            element: field_name.to_string(),
            reason: format!(
                "GUID must be exactly 38 characters in '{{...}}' format, got {}",
                s.len()
            ),
        });
    }
    let bytes = s.as_bytes();
    if bytes[0] != b'{' || bytes[37] != b'}' {
        return Err(Error::Validation {
            element: field_name.to_string(),
            reason: "GUID must start with '{' and end with '}'".to_string(),
        });
    }
    for (i, &b) in bytes.iter().enumerate() {
        if i == 0 || i == 37 {
            continue;
        }
        if i == 9 || i == 14 || i == 19 || i == 24 {
            if b != b'-' {
                return Err(Error::Validation {
                    element: field_name.to_string(),
                    reason: format!("expected '-' at position {i} in GUID"),
                });
            }
        } else if !b.is_ascii_hexdigit() {
            return Err(Error::Validation {
                element: field_name.to_string(),
                reason: format!(
                    "invalid non-hex character '{}' in GUID at position {i}",
                    b as char
                ),
            });
        }
    }
    Ok(())
}

/// Computes 160-bit SHA-1 digest for arbitrary byte buffer (FIPS PUB 180-1 / RFC 3174).
#[must_use]
#[allow(clippy::many_single_char_names)]
fn compute_sha1(data: &[u8]) -> [u8; 20] {
    let mut h0: u32 = 0x6745_2301;
    let mut h1: u32 = 0xEFCD_AB89;
    let mut h2: u32 = 0x98BA_DCFE;
    let mut h3: u32 = 0x1032_5476;
    let mut h4: u32 = 0xC3D2_E1F0;

    let bit_len = (data.len() as u64).wrapping_mul(8);
    let mut padded = data.to_vec();
    padded.push(0x80);
    while (padded.len() % 64) != 56 {
        padded.push(0x00);
    }
    padded.extend_from_slice(&bit_len.to_be_bytes());

    for chunk in padded.chunks_exact(64) {
        let mut w = [0u32; 80];
        for (i, slot) in w.iter_mut().take(16).enumerate() {
            let offset = i * 4;
            *slot = u32::from_be_bytes([
                chunk[offset],
                chunk[offset + 1],
                chunk[offset + 2],
                chunk[offset + 3],
            ]);
        }
        for i in 16..80 {
            w[i] = (w[i - 3] ^ w[i - 8] ^ w[i - 14] ^ w[i - 16]).rotate_left(1);
        }

        let mut a = h0;
        let mut b = h1;
        let mut c = h2;
        let mut d = h3;
        let mut e = h4;

        for (i, &word) in w.iter().enumerate() {
            let (f, k) = match i {
                0..=19 => ((b & c) | ((!b) & d), 0x5A82_7999),
                20..=39 => (b ^ c ^ d, 0x6ED9_EBA1),
                40..=59 => ((b & c) | (b & d) | (c & d), 0x8F1B_BCDC),
                _ => (b ^ c ^ d, 0xCA62_C1D6),
            };
            let temp = a
                .rotate_left(5)
                .wrapping_add(f)
                .wrapping_add(e)
                .wrapping_add(k)
                .wrapping_add(word);
            e = d;
            d = c;
            c = b.rotate_left(30);
            b = a;
            a = temp;
        }

        h0 = h0.wrapping_add(a);
        h1 = h1.wrapping_add(b);
        h2 = h2.wrapping_add(c);
        h3 = h3.wrapping_add(d);
        h4 = h4.wrapping_add(e);
    }

    let mut out = [0u8; 20];
    out[0..4].copy_from_slice(&h0.to_be_bytes());
    out[4..8].copy_from_slice(&h1.to_be_bytes());
    out[8..12].copy_from_slice(&h2.to_be_bytes());
    out[12..16].copy_from_slice(&h3.to_be_bytes());
    out[16..20].copy_from_slice(&h4.to_be_bytes());
    out
}

/// Strongly-typed Component GUID (e.g. `{12345678-1234-1234-1234-1234567890AB}`).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ComponentGuid(String);

impl ComponentGuid {
    /// Creates a new validated [`ComponentGuid`].
    ///
    /// # Arguments
    ///
    /// * `guid` - String representation in `{XXXXXXXX-XXXX-XXXX-XXXX-XXXXXXXXXXXX}` format.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Validation`] if the GUID format is invalid.
    pub fn parse(guid: impl Into<String>) -> Result<Self> {
        let s = guid.into();
        validate_guid(&s, "ComponentGuid")?;
        Ok(Self(s))
    }

    /// Generates a deterministic RFC-4122 v5 UUID based on namespace and name strings.
    ///
    /// # Arguments
    ///
    /// * `namespace_seed` - Namespace seed string (e.g. Directory ID or Product GUID).
    /// * `name` - Resource name string (e.g. Component ID or file path).
    ///
    /// # Returns
    ///
    /// A deterministic [`ComponentGuid`].
    #[must_use]
    pub fn generate(namespace_seed: &str, name: &str) -> Self {
        let mut input = Vec::with_capacity(namespace_seed.len() + name.len() + 1);
        input.extend_from_slice(namespace_seed.as_bytes());
        input.push(b':');
        input.extend_from_slice(name.as_bytes());

        let mut digest = compute_sha1(&input);
        // RFC 4122 version 5 (SHA-1 name based): set top 4 bits of octet 6 to 0101 (5)
        digest[6] = (digest[6] & 0x0F) | 0x50;
        // RFC 4122 variant: set top 2 bits of octet 8 to 10
        digest[8] = (digest[8] & 0x3F) | 0x80;

        let formatted = format!(
            "{{{:02X}{:02X}{:02X}{:02X}-{:02X}{:02X}-{:02X}{:02X}-{:02X}{:02X}-{:02X}{:02X}{:02X}{:02X}{:02X}{:02X}}}",
            digest[0], digest[1], digest[2], digest[3],
            digest[4], digest[5],
            digest[6], digest[7],
            digest[8], digest[9],
            digest[10], digest[11], digest[12], digest[13], digest[14], digest[15]
        );
        Self(formatted)
    }

    /// Generates a deterministic RFC-4122 v5 UUID based on namespace and name strings.
    ///
    /// # Arguments
    ///
    /// * `namespace_seed` - Namespace seed string (e.g. Directory ID or Product GUID).
    /// * `name` - Resource name string (e.g. Component ID or file path).
    ///
    /// # Returns
    ///
    /// A deterministic [`ComponentGuid`].
    ///
    /// # Errors
    ///
    /// Returns [`Error::Validation`] if formatting fails.
    pub fn generate_deterministic(namespace_seed: &str, name: &str) -> Result<Self> {
        Ok(Self::generate(namespace_seed, name))
    }

    /// Returns the string slice of this GUID.
    ///
    /// # Returns
    ///
    /// Reference to the underlying string.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for ComponentGuid {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Strongly-typed `ProductCode` GUID.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ProductCode(String);

impl ProductCode {
    /// Creates a new validated [`ProductCode`].
    ///
    /// # Arguments
    ///
    /// * `code` - String representation in `{XXXXXXXX-XXXX-XXXX-XXXX-XXXXXXXXXXXX}` format.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Validation`] if the GUID format is invalid.
    pub fn parse(code: impl Into<String>) -> Result<Self> {
        let s = code.into();
        validate_guid(&s, "ProductCode")?;
        Ok(Self(s))
    }

    /// Returns the string slice of this `ProductCode`.
    ///
    /// # Returns
    ///
    /// Reference to the underlying string.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for ProductCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Strongly-typed `UpgradeCode` GUID.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct UpgradeCode(String);

impl UpgradeCode {
    /// Creates a new validated [`UpgradeCode`].
    ///
    /// # Arguments
    ///
    /// * `code` - String representation in `{XXXXXXXX-XXXX-XXXX-XXXX-XXXXXXXXXXXX}` format.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Validation`] if the GUID format is invalid.
    pub fn parse(code: impl Into<String>) -> Result<Self> {
        let s = code.into();
        validate_guid(&s, "UpgradeCode")?;
        Ok(Self(s))
    }

    /// Returns the string slice of this `UpgradeCode`.
    ///
    /// # Returns
    ///
    /// Reference to the underlying string.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for UpgradeCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Strongly-typed Table identifier newtype.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TableId(String);

impl TableId {
    /// Creates a new [`TableId`].
    ///
    /// # Arguments
    ///
    /// * `name` - Table name identifier.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Validation`] if `name` is empty or exceeds 64 characters.
    pub fn new(name: impl Into<String>) -> Result<Self> {
        Self::new_inner(name.into())
    }

    /// Validates and constructs a [`TableId`] from an owned string.
    fn new_inner(s: String) -> Result<Self> {
        if s.is_empty() || s.len() > 64 {
            return Err(Error::Validation {
                element: "TableId".to_string(),
                reason: format!(
                    "Table identifier must be between 1 and 64 characters, got {}",
                    s.len()
                ),
            });
        }
        Ok(Self(s))
    }

    /// Returns the string slice of this table identifier.
    ///
    /// # Returns
    ///
    /// Reference to the underlying string.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for TableId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Strongly-typed 1-based Column index in a table.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ColumnIndex(u16);

impl ColumnIndex {
    /// Creates a new 1-based [`ColumnIndex`].
    ///
    /// # Arguments
    ///
    /// * `index` - 1-based index (must be >= 1).
    ///
    /// # Errors
    ///
    /// Returns [`Error::Validation`] if index is 0.
    pub fn new(index: u16) -> Result<Self> {
        if index == 0 {
            return Err(Error::Validation {
                element: "ColumnIndex".to_string(),
                reason: "Column index in MSI table must be 1-based".to_string(),
            });
        }
        Ok(Self(index))
    }

    /// Returns the raw 1-based index value.
    ///
    /// # Returns
    ///
    /// The index as a `u16`.
    #[must_use]
    pub const fn get(self) -> u16 {
        self.0
    }
}

impl fmt::Display for ColumnIndex {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Strongly-typed String Pool entry identifier (0 represents `NULL`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct StringPoolId(u32);

impl StringPoolId {
    /// Identifier representing a `NULL` string.
    pub const NULL: Self = Self(0);

    /// Creates a [`StringPoolId`] with the given raw value.
    ///
    /// # Arguments
    ///
    /// * `id` - 1-based index or 0 for `NULL`.
    ///
    /// # Returns
    ///
    /// A new [`StringPoolId`].
    #[must_use]
    pub const fn new(id: u32) -> Self {
        Self(id)
    }

    /// Returns whether this identifier represents `NULL` (id == 0).
    ///
    /// # Returns
    ///
    /// `true` if `NULL`, `false` otherwise.
    #[must_use]
    pub const fn is_null(self) -> bool {
        self.0 == 0
    }

    /// Returns the raw numeric index.
    ///
    /// # Returns
    ///
    /// Raw integer identifier.
    #[must_use]
    pub const fn get(self) -> u32 {
        self.0
    }
}

impl fmt::Display for StringPoolId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.is_null() {
            write!(f, "NULL")
        } else {
            write!(f, "StringPool#{}", self.0)
        }
    }
}

/// Strongly-typed Record index in an MSI table.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RecordIndex(u32);

impl RecordIndex {
    /// Creates a new [`RecordIndex`].
    ///
    /// # Arguments
    ///
    /// * `index` - 0-based record index.
    ///
    /// # Returns
    ///
    /// A new [`RecordIndex`].
    #[must_use]
    pub const fn new(index: u32) -> Self {
        Self(index)
    }

    /// Returns the raw record index.
    ///
    /// # Returns
    ///
    /// 0-based index value.
    #[must_use]
    pub const fn get(self) -> u32 {
        self.0
    }
}

impl fmt::Display for RecordIndex {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Record#{}", self.0)
    }
}

/// Strongly-typed Sequence number in actions or media.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SequenceNumber(i32);

impl SequenceNumber {
    /// Creates a new [`SequenceNumber`].
    ///
    /// # Arguments
    ///
    /// * `seq` - Numeric sequence number.
    ///
    /// # Returns
    ///
    /// A new [`SequenceNumber`].
    #[must_use]
    pub const fn new(seq: i32) -> Self {
        Self(seq)
    }

    /// Returns the sequence value.
    ///
    /// # Returns
    ///
    /// Integer sequence value.
    #[must_use]
    pub const fn get(self) -> i32 {
        self.0
    }
}

impl fmt::Display for SequenceNumber {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Truncates and deterministically hashes an identifier that exceeds 72 characters.
///
/// If `s` has 72 or fewer characters, it is returned unchanged.
/// If `s` exceeds 72 characters, the first 55 bytes (adjusted to UTF-8 char boundary)
/// are preserved and suffixed with `_` and a 16-hex-character deterministic SHA-1 hash
/// of the full identifier, ensuring the result is at most 72 characters and unique.
///
/// # Arguments
///
/// * `s` - Raw identifier string.
///
/// # Returns
///
/// Deterministically sanitized identifier string of 72 characters or fewer.
#[must_use]
pub fn sanitize_identifier_length(s: String) -> String {
    if s.len() <= 72 {
        return s;
    }
    let hash = compute_sha1(s.as_bytes());
    let hex_hash = format!(
        "{:02X}{:02X}{:02X}{:02X}{:02X}{:02X}{:02X}{:02X}",
        hash[0], hash[1], hash[2], hash[3], hash[4], hash[5], hash[6], hash[7]
    );
    let mut boundary = 55;
    while !s.is_char_boundary(boundary) {
        boundary -= 1;
    }
    format!("{}_{hex_hash}", &s[..boundary])
}

/// Strongly-typed File key identifier (primary key in `File` table).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FileKey(String);

impl FileKey {
    /// Creates a new [`FileKey`].
    ///
    /// Identifiers exceeding 72 characters are deterministically hashed to conform
    /// to the standard Windows Installer column length limit.
    ///
    /// # Arguments
    ///
    /// * `key` - Primary key identifier string.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Validation`] if `key` is empty.
    pub fn new(key: impl Into<String>) -> Result<Self> {
        Self::new_inner(key.into())
    }

    /// Validates and constructs a [`FileKey`] from an owned string.
    fn new_inner(s: String) -> Result<Self> {
        if s.is_empty() {
            return Err(Error::Validation {
                element: "FileKey".to_string(),
                reason: "FileKey must not be empty".to_string(),
            });
        }
        let sanitized = sanitize_identifier_length(s);
        Ok(Self(sanitized))
    }

    /// Returns the string slice.
    ///
    /// # Returns
    ///
    /// String slice.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for FileKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Strongly-typed Feature name identifier (primary key in `Feature` table).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FeatureName(String);

impl FeatureName {
    /// Creates a new [`FeatureName`].
    ///
    /// # Arguments
    ///
    /// * `name` - Feature name string.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Validation`] if `name` is empty or exceeds 38 characters.
    pub fn new(name: impl Into<String>) -> Result<Self> {
        Self::new_inner(name.into())
    }

    /// Validates and constructs a [`FeatureName`] from an owned string.
    fn new_inner(s: String) -> Result<Self> {
        if s.is_empty() || s.len() > 38 {
            return Err(Error::Validation {
                element: "FeatureName".to_string(),
                reason: format!(
                    "FeatureName must be between 1 and 38 characters, got {}",
                    s.len()
                ),
            });
        }
        Ok(Self(s))
    }

    /// Returns the string slice.
    ///
    /// # Returns
    ///
    /// String slice.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for FeatureName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Strongly-typed Component name identifier (primary key in `Component` table).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ComponentName(String);

impl ComponentName {
    /// Creates a new [`ComponentName`].
    ///
    /// Identifiers exceeding 72 characters are deterministically hashed to conform
    /// to the standard Windows Installer column length limit.
    ///
    /// # Arguments
    ///
    /// * `name` - Component name string.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Validation`] if `name` is empty.
    pub fn new(name: impl Into<String>) -> Result<Self> {
        Self::new_inner(name.into())
    }

    /// Validates and constructs a [`ComponentName`] from an owned string.
    fn new_inner(s: String) -> Result<Self> {
        if s.is_empty() {
            return Err(Error::Validation {
                element: "ComponentName".to_string(),
                reason: "ComponentName must not be empty".to_string(),
            });
        }
        let sanitized = sanitize_identifier_length(s);
        Ok(Self(sanitized))
    }

    /// Returns the string slice.
    ///
    /// # Returns
    ///
    /// String slice.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for ComponentName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Strongly-typed Directory identifier (primary key in `Directory` table).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DirectoryId(String);

impl DirectoryId {
    /// Creates a new [`DirectoryId`].
    ///
    /// Identifiers exceeding 72 characters are deterministically hashed to conform
    /// to the standard Windows Installer column length limit.
    ///
    /// # Arguments
    ///
    /// * `id` - Directory identifier string (e.g. `TARGETDIR`, `INSTALLDIR`).
    ///
    /// # Errors
    ///
    /// Returns [`Error::Validation`] if `id` is empty.
    pub fn new(id: impl Into<String>) -> Result<Self> {
        Self::new_inner(id.into())
    }

    /// Validates and constructs a [`DirectoryId`] from an owned string.
    fn new_inner(s: String) -> Result<Self> {
        if s.is_empty() {
            return Err(Error::Validation {
                element: "DirectoryId".to_string(),
                reason: "DirectoryId must not be empty".to_string(),
            });
        }
        let sanitized = sanitize_identifier_length(s);
        Ok(Self(sanitized))
    }

    /// Returns the string slice.
    ///
    /// # Returns
    ///
    /// String slice.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for DirectoryId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Strongly-typed Property name (primary key in `Property` table).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PropertyName(String);

impl PropertyName {
    /// Creates a new [`PropertyName`].
    ///
    /// # Arguments
    ///
    /// * `name` - Property name string.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Validation`] if `name` is empty or exceeds 72 characters.
    pub fn new(name: impl Into<String>) -> Result<Self> {
        Self::new_inner(name.into())
    }

    /// Validates and constructs a [`PropertyName`] from an owned string.
    fn new_inner(s: String) -> Result<Self> {
        if s.is_empty() || s.len() > 72 {
            return Err(Error::Validation {
                element: "PropertyName".to_string(),
                reason: format!(
                    "PropertyName must be between 1 and 72 characters, got {}",
                    s.len()
                ),
            });
        }
        Ok(Self(s))
    }

    /// Returns the string slice.
    ///
    /// # Returns
    ///
    /// String slice.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for PropertyName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_guid_validation() -> Result<()> {
        let valid = "{12345678-1234-1234-1234-1234567890AB}";
        let cg = ComponentGuid::parse(valid)?;
        assert_eq!(cg.as_str(), valid);
        assert_eq!(format!("{cg}"), valid);

        let pc = ProductCode::parse(valid)?;
        assert_eq!(pc.as_str(), valid);
        assert_eq!(format!("{pc}"), valid);

        let uc = UpgradeCode::parse(valid)?;
        assert_eq!(uc.as_str(), valid);
        assert_eq!(format!("{uc}"), valid);

        // Invalid length
        assert!(ComponentGuid::parse("{1234}").is_err());
        // Missing braces
        assert!(ComponentGuid::parse("12345678-1234-1234-1234-1234567890AB12").is_err());
        let bad_braces = "X1234567-1234-1234-1234-1234567890ABX";
        assert!(ComponentGuid::parse(bad_braces).is_err());
        let bad_closing_brace = format!("{}X", &valid[..37]);
        assert!(ComponentGuid::parse(bad_closing_brace).is_err());
        // Missing dashes at positions 9, 14, 19, 24
        assert!(ComponentGuid::parse("{12345678X1234-1234-1234-1234567890AB}").is_err());
        assert!(ComponentGuid::parse("{12345678-1234X1234-1234-1234567890AB}").is_err());
        assert!(ComponentGuid::parse("{12345678-1234-1234X1234-1234567890AB}").is_err());
        assert!(ComponentGuid::parse("{12345678-1234-1234-1234X1234567890AB}").is_err());
        // Non-hex character
        let bad_hex = "{12345678-1234-1234-1234-1234567890ZZ}";
        assert!(ComponentGuid::parse(bad_hex).is_err());
        Ok(())
    }

    #[test]
    fn test_table_id() -> Result<()> {
        assert!(TableId::new("").is_err());
        assert!(TableId::new("a".repeat(65)).is_err());
        let t = TableId::new("Component")?;
        assert_eq!(t.as_str(), "Component");
        assert_eq!(format!("{t}"), "Component");
        Ok(())
    }

    #[test]
    fn test_column_index() -> Result<()> {
        assert!(ColumnIndex::new(0).is_err());
        let c = ColumnIndex::new(1)?;
        assert_eq!(c.get(), 1);
        assert_eq!(format!("{c}"), "1");
        Ok(())
    }

    #[test]
    fn test_string_pool_id() {
        let null_id = StringPoolId::NULL;
        assert!(null_id.is_null());
        assert_eq!(null_id.get(), 0);
        assert_eq!(format!("{null_id}"), "NULL");

        let id = StringPoolId::new(42);
        assert!(!id.is_null());
        assert_eq!(id.get(), 42);
        assert_eq!(format!("{id}"), "StringPool#42");
    }

    #[test]
    fn test_record_index() {
        let r = RecordIndex::new(5);
        assert_eq!(r.get(), 5);
        assert_eq!(format!("{r}"), "Record#5");
    }

    #[test]
    fn test_sequence_number() {
        let s = SequenceNumber::new(-10);
        assert_eq!(s.get(), -10);
        assert_eq!(format!("{s}"), "-10");
    }

    #[test]
    fn test_file_key() -> Result<()> {
        assert!(FileKey::new("").is_err());
        let f_long = FileKey::new("a".repeat(73))?;
        assert!(f_long.as_str().len() <= 72);
        assert!(f_long.as_str().starts_with(&"a".repeat(55)));
        let f = FileKey::new("bin_file")?;
        assert_eq!(f.as_str(), "bin_file");
        assert_eq!(format!("{f}"), "bin_file");
        Ok(())
    }

    #[test]
    fn test_feature_name() -> Result<()> {
        assert!(FeatureName::new("").is_err());
        assert!(FeatureName::new("a".repeat(39)).is_err());
        let feat = FeatureName::new("MainFeature")?;
        assert_eq!(feat.as_str(), "MainFeature");
        assert_eq!(format!("{feat}"), "MainFeature");
        Ok(())
    }

    #[test]
    fn test_component_name() -> Result<()> {
        assert!(ComponentName::new("").is_err());
        let c_long = ComponentName::new(
            "CMP_H__lib_web_servers_nginx_conf_simple_location_proxy_websockets_conf",
        )?;
        assert!(c_long.as_str().len() <= 72);
        assert!(c_long
            .as_str()
            .starts_with("CMP_H__lib_web_servers_nginx_conf_simple_location_proxy"));
        // Test deterministic behavior
        let c_long_repeat = ComponentName::new(
            "CMP_H__lib_web_servers_nginx_conf_simple_location_proxy_websockets_conf",
        )?;
        assert_eq!(c_long, c_long_repeat);
        let c = ComponentName::new("MainComp")?;
        assert_eq!(c.as_str(), "MainComp");
        assert_eq!(format!("{c}"), "MainComp");
        Ok(())
    }

    #[test]
    fn test_directory_id() -> Result<()> {
        assert!(DirectoryId::new("").is_err());
        let d_long = DirectoryId::new(
            "DIR_H__lib_web_servers_nginx_conf_simple_location_proxy_websockets_conf",
        )?;
        assert!(d_long.as_str().len() <= 72);
        let d = DirectoryId::new("TARGETDIR")?;
        assert_eq!(d.as_str(), "TARGETDIR");
        assert_eq!(format!("{d}"), "TARGETDIR");
        Ok(())
    }

    #[test]
    fn test_sanitize_identifier_multibyte_utf8() {
        // Multi-byte character around index 55
        let s = format!("{}🦀{}", "a".repeat(54), "b".repeat(30));
        let sanitized = sanitize_identifier_length(s);
        assert!(sanitized.len() <= 72);
    }

    #[test]
    fn test_property_name() -> Result<()> {
        assert!(PropertyName::new("").is_err());
        assert!(PropertyName::new("a".repeat(73)).is_err());
        let p = PropertyName::new("ProductVersion")?;
        assert_eq!(p.as_str(), "ProductVersion");
        assert_eq!(format!("{p}"), "ProductVersion");
        Ok(())
    }

    #[test]
    fn test_deterministic_guid() -> Result<()> {
        let guid1 = ComponentGuid::generate_deterministic("INSTALLDIR", "Comp1")?;
        let guid2 = ComponentGuid::generate_deterministic("INSTALLDIR", "Comp1")?;
        let guid3 = ComponentGuid::generate_deterministic("INSTALLDIR", "Comp2")?;

        assert_eq!(guid1, guid2);
        assert_ne!(guid1, guid3);
        assert!(guid1.as_str().starts_with('{'));
        assert!(guid1.as_str().ends_with('}'));
        assert_eq!(guid1.as_str().len(), 38);

        // Verify version 5 character at position 15
        assert_eq!(&guid1.as_str()[15..16], "5");

        // Test empty input sha1 coverage
        let empty_guid = ComponentGuid::generate_deterministic("", "")?;
        assert_eq!(empty_guid.as_str().len(), 38);
        Ok(())
    }
}

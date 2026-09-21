//! Compound File Binary Format Directory Entries and Red-Black Tree ([MS-CFB] 2.6).

use crate::cfb::sector::SectorId;
use crate::error::{Error, Result};
use std::cmp::Ordering;
use std::fmt;

/// Maximum number of UTF-16 code units in a directory entry name (including null terminator).
pub const MAX_DIRECTORY_NAME_LEN: usize = 32;

/// Exact byte length of a serialized directory entry ([MS-CFB] 2.6).
pub const DIRECTORY_ENTRY_SIZE: usize = 128;

/// Strongly-typed Directory Entry / Stream identifier ([MS-CFB] 2.6).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct StreamId(pub u32);

impl StreamId {
    /// Sentinel value indicating no sibling or child stream (`0xFFFFFFFF`).
    pub const NO_STREAM: Self = Self(0xFFFF_FFFF);

    /// Maximum valid stream ID (`0xFFFFFFFA`).
    pub const MAX_VALID: Self = Self(0xFFFF_FFFA);

    /// Creates a new [`StreamId`].
    ///
    /// # Arguments
    ///
    /// * `value` - Raw 32-bit stream identifier.
    ///
    /// # Returns
    ///
    /// A new [`StreamId`].
    #[must_use]
    pub const fn new(value: u32) -> Self {
        Self(value)
    }

    /// Returns the raw 32-bit integer.
    ///
    /// # Returns
    ///
    /// Stream identifier as a [`u32`].
    #[must_use]
    pub const fn as_u32(self) -> u32 {
        self.0
    }

    /// Determines whether this stream ID is valid (not `NO_STREAM` or reserved).
    ///
    /// # Returns
    ///
    /// `true` if this is a valid stream index; `false` otherwise.
    #[must_use]
    pub const fn is_valid(self) -> bool {
        self.0 <= Self::MAX_VALID.0
    }

    /// Determines whether this stream ID represents `NO_STREAM`.
    ///
    /// # Returns
    ///
    /// `true` if `NO_STREAM`; `false` otherwise.
    #[must_use]
    pub const fn is_none(self) -> bool {
        self.0 == Self::NO_STREAM.0
    }
}

impl fmt::Display for StreamId {
    /// Formats the stream ID for display.
    ///
    /// # Arguments
    ///
    /// * `f` - Formatter.
    ///
    /// # Returns
    ///
    /// Format result.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.is_none() {
            write!(f, "NO_STREAM")
        } else {
            write!(f, "StreamId({})", self.0)
        }
    }
}

/// CFB Directory Entry Object Type ([MS-CFB] 2.6.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(u8)]
pub enum ObjectType {
    /// Unallocated / unknown object type (`0x00`).
    Unknown = 0x00,
    /// Storage object (`0x01`).
    Storage = 0x01,
    /// Stream object (`0x02`).
    Stream = 0x02,
    /// Root storage object (`0x05`).
    Root = 0x05,
}

impl ObjectType {
    /// Parses an [`ObjectType`] from a raw byte.
    ///
    /// # Arguments
    ///
    /// * `byte` - Raw byte representation.
    ///
    /// # Returns
    ///
    /// A parsed [`ObjectType`], or an error if the byte is invalid.
    ///
    /// # Errors
    ///
    /// Returns [`Error::InvalidDirectoryEntry`] if the byte is not a known object type.
    pub fn from_u8(byte: u8) -> Result<Self> {
        match byte {
            0x00 => Ok(Self::Unknown),
            0x01 => Ok(Self::Storage),
            0x02 => Ok(Self::Stream),
            0x05 => Ok(Self::Root),
            other => Err(Error::InvalidDirectoryEntry {
                index: 0,
                reason: format!("unknown object type byte 0x{other:02X}"),
            }),
        }
    }

    /// Converts the [`ObjectType`] to its raw byte representation.
    ///
    /// # Returns
    ///
    /// The object type byte.
    #[must_use]
    pub const fn as_u8(self) -> u8 {
        self as u8
    }
}

/// CFB Red-Black Tree node color flag ([MS-CFB] 2.6.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(u8)]
pub enum ColorFlag {
    /// Red node (`0x00`).
    Red = 0x00,
    /// Black node (`0x01`).
    Black = 0x01,
}

impl ColorFlag {
    /// Parses a [`ColorFlag`] from a raw byte.
    ///
    /// # Arguments
    ///
    /// * `byte` - Raw color byte (`0x00` or `0x01`).
    ///
    /// # Returns
    ///
    /// The parsed [`ColorFlag`].
    ///
    /// # Errors
    ///
    /// Returns [`Error::InvalidDirectoryEntry`] if byte is neither 0 nor 1.
    pub fn from_u8(byte: u8) -> Result<Self> {
        match byte {
            0x00 => Ok(Self::Red),
            0x01 => Ok(Self::Black),
            other => Err(Error::InvalidDirectoryEntry {
                index: 0,
                reason: format!("invalid node color flag 0x{other:02X}"),
            }),
        }
    }

    /// Returns the raw byte representation of the color flag.
    ///
    /// # Returns
    ///
    /// `0x00` for Red, `0x01` for Black.
    #[must_use]
    pub const fn as_u8(self) -> u8 {
        self as u8
    }
}

/// A parsed 128-byte Directory Entry ([MS-CFB] 2.6).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DirectoryEntry {
    /// Name of the directory entry as a UTF-16 string (without null terminator).
    name: String,
    /// Type of this object (Stream, Storage, Root, Unknown).
    object_type: ObjectType,
    /// Red-Black Tree node color.
    color: ColorFlag,
    /// Left sibling stream ID (`StreamId::NO_STREAM` if none).
    left_sibling: StreamId,
    /// Right sibling stream ID (`StreamId::NO_STREAM` if none).
    right_sibling: StreamId,
    /// Child stream ID (`StreamId::NO_STREAM` if none).
    child: StreamId,
    /// Class identifier GUID (16 bytes).
    clsid: [u8; 16],
    /// User-defined state bits flags.
    state_bits: u32,
    /// Windows creation `FILETIME` (64-bit integer).
    creation_time: u64,
    /// Windows modification `FILETIME` (64-bit integer).
    modified_time: u64,
    /// Starting sector of stream or mini-stream.
    start_sector: SectorId,
    /// 64-bit file/stream length in bytes.
    stream_size: u64,
}

impl DirectoryEntry {
    /// Creates a new [`DirectoryEntry`] with default fields.
    ///
    /// # Arguments
    ///
    /// * `name` - The entry name.
    /// * `object_type` - Object type (Stream, Storage, Root).
    ///
    /// # Returns
    ///
    /// A new [`DirectoryEntry`].
    #[must_use]
    pub fn new(name: impl Into<String>, object_type: ObjectType) -> Self {
        Self {
            name: name.into(),
            object_type,
            color: ColorFlag::Black,
            left_sibling: StreamId::NO_STREAM,
            right_sibling: StreamId::NO_STREAM,
            child: StreamId::NO_STREAM,
            clsid: [0u8; 16],
            state_bits: 0,
            creation_time: 0,
            modified_time: 0,
            start_sector: SectorId::END_OF_CHAIN,
            stream_size: 0,
        }
    }

    /// Creates an empty, unallocated directory entry.
    ///
    /// # Returns
    ///
    /// An unallocated [`DirectoryEntry`].
    #[must_use]
    pub fn empty() -> Self {
        Self::new(String::new(), ObjectType::Unknown)
    }

    /// Returns the name of the directory entry.
    ///
    /// # Returns
    ///
    /// Entry name string slice.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Sets the name of the directory entry.
    ///
    /// # Arguments
    ///
    /// * `name` - The new name.
    pub fn set_name(&mut self, name: impl Into<String>) {
        self.name = name.into();
    }

    /// Returns the object type.
    ///
    /// # Returns
    ///
    /// The [`ObjectType`].
    #[must_use]
    pub const fn object_type(&self) -> ObjectType {
        self.object_type
    }

    /// Sets the object type.
    ///
    /// # Arguments
    ///
    /// * `object_type` - The object type.
    pub const fn set_object_type(&mut self, object_type: ObjectType) {
        self.object_type = object_type;
    }

    /// Returns the node color flag.
    ///
    /// # Returns
    ///
    /// The [`ColorFlag`].
    #[must_use]
    pub const fn color(&self) -> ColorFlag {
        self.color
    }

    /// Sets the node color flag.
    ///
    /// # Arguments
    ///
    /// * `color` - Node color flag.
    pub const fn set_color(&mut self, color: ColorFlag) {
        self.color = color;
    }

    /// Returns the left sibling stream ID.
    ///
    /// # Returns
    ///
    /// Left sibling [`StreamId`].
    #[must_use]
    pub const fn left_sibling(&self) -> StreamId {
        self.left_sibling
    }

    /// Sets the left sibling stream ID.
    ///
    /// # Arguments
    ///
    /// * `id` - Left sibling [`StreamId`].
    pub const fn set_left_sibling(&mut self, id: StreamId) {
        self.left_sibling = id;
    }

    /// Returns the right sibling stream ID.
    ///
    /// # Returns
    ///
    /// Right sibling [`StreamId`].
    #[must_use]
    pub const fn right_sibling(&self) -> StreamId {
        self.right_sibling
    }

    /// Sets the right sibling stream ID.
    ///
    /// # Arguments
    ///
    /// * `id` - Right sibling [`StreamId`].
    pub const fn set_right_sibling(&mut self, id: StreamId) {
        self.right_sibling = id;
    }

    /// Returns the child stream ID.
    ///
    /// # Returns
    ///
    /// Child [`StreamId`].
    #[must_use]
    pub const fn child(&self) -> StreamId {
        self.child
    }

    /// Sets the child stream ID.
    ///
    /// # Arguments
    ///
    /// * `id` - Child [`StreamId`].
    pub const fn set_child(&mut self, id: StreamId) {
        self.child = id;
    }

    /// Returns the 16-byte CLSID.
    ///
    /// # Returns
    ///
    /// CLSID byte array.
    #[must_use]
    pub const fn clsid(&self) -> &[u8; 16] {
        &self.clsid
    }

    /// Sets the 16-byte CLSID.
    ///
    /// # Arguments
    ///
    /// * `clsid` - 16-byte CLSID.
    pub const fn set_clsid(&mut self, clsid: [u8; 16]) {
        self.clsid = clsid;
    }

    /// Returns the state bits flags.
    ///
    /// # Returns
    ///
    /// State bits as [`u32`].
    #[must_use]
    pub const fn state_bits(&self) -> u32 {
        self.state_bits
    }

    /// Returns the creation time.
    ///
    /// # Returns
    ///
    /// Creation `FILETIME` as [`u64`].
    #[must_use]
    pub const fn creation_time(&self) -> u64 {
        self.creation_time
    }

    /// Returns the modification time.
    ///
    /// # Returns
    ///
    /// Modification `FILETIME` as [`u64`].
    #[must_use]
    pub const fn modified_time(&self) -> u64 {
        self.modified_time
    }

    /// Returns the starting sector ID.
    ///
    /// # Returns
    ///
    /// Starting [`SectorId`].
    #[must_use]
    pub const fn start_sector(&self) -> SectorId {
        self.start_sector
    }

    /// Sets the starting sector ID.
    ///
    /// # Arguments
    ///
    /// * `sector` - Starting [`SectorId`].
    pub const fn set_start_sector(&mut self, sector: SectorId) {
        self.start_sector = sector;
    }

    /// Returns the stream size in bytes.
    ///
    /// # Returns
    ///
    /// Stream size as [`u64`].
    #[must_use]
    pub const fn stream_size(&self) -> u64 {
        self.stream_size
    }

    /// Sets the stream size in bytes.
    ///
    /// # Arguments
    ///
    /// * `size` - Stream byte length.
    pub const fn set_stream_size(&mut self, size: u64) {
        self.stream_size = size;
    }

    /// Parses a 128-byte directory entry from a byte slice ([MS-CFB] 2.6).
    ///
    /// # Arguments
    ///
    /// * `bytes` - Exactly 128 bytes of serialized directory entry.
    /// * `index` - The entry index in the directory array for error reporting.
    ///
    /// # Returns
    ///
    /// A parsed [`DirectoryEntry`].
    ///
    /// # Errors
    ///
    /// Returns [`Error::InvalidDirectoryEntry`] or [`Error::CfbCorrupted`] if the entry is invalid.
    pub fn parse(bytes: &[u8], index: u32) -> Result<Self> {
        if bytes.len() < DIRECTORY_ENTRY_SIZE {
            return Err(Error::CfbCorrupted {
                offset: u64::from(index) * DIRECTORY_ENTRY_SIZE as u64,
                reason: format!(
                    "Directory entry byte slice too short: expected {DIRECTORY_ENTRY_SIZE}, got {}",
                    bytes.len()
                ),
            });
        }

        // Name length in bytes (including null terminator) at 0x40..0x42
        let name_byte_len = u16::from_le_bytes([bytes[64], bytes[65]]) as usize;
        let name = if name_byte_len == 0 {
            String::new()
        } else {
            if name_byte_len > 64 || name_byte_len % 2 != 0 {
                return Err(Error::InvalidDirectoryEntry {
                    index,
                    reason: format!("invalid directory entry name byte length: {name_byte_len}"),
                });
            }

            // UTF-16 characters up to name_byte_len - 2 (drop null terminator)
            let char_count = (name_byte_len / 2).saturating_sub(1);
            let mut u16_chars = Vec::with_capacity(char_count);
            for i in 0..char_count {
                let offset = i * 2;
                let ch = u16::from_le_bytes([bytes[offset], bytes[offset + 1]]);
                u16_chars.push(ch);
            }

            String::from_utf16(&u16_chars).map_err(|err| Error::InvalidDirectoryEntry {
                index,
                reason: format!("invalid UTF-16 in entry name: {err}"),
            })?
        };

        // Object type at 0x42
        let object_type = ObjectType::from_u8(bytes[66])?;

        // Color at 0x43
        let color = ColorFlag::from_u8(bytes[67])?;

        // Sibling IDs at 0x44, 0x48, 0x4C
        let left_sibling = StreamId::new(u32::from_le_bytes([
            bytes[68], bytes[69], bytes[70], bytes[71],
        ]));
        let right_sibling = StreamId::new(u32::from_le_bytes([
            bytes[72], bytes[73], bytes[74], bytes[75],
        ]));
        let child = StreamId::new(u32::from_le_bytes([
            bytes[76], bytes[77], bytes[78], bytes[79],
        ]));

        // CLSID at 0x50..0x60
        let mut clsid = [0u8; 16];
        clsid.copy_from_slice(&bytes[80..96]);

        // State bits at 0x60..0x64
        let state_bits = u32::from_le_bytes([bytes[96], bytes[97], bytes[98], bytes[99]]);

        // Creation time at 0x64..0x6C
        let mut creation_time_bytes = [0u8; 8];
        creation_time_bytes.copy_from_slice(&bytes[100..108]);
        let creation_time = u64::from_le_bytes(creation_time_bytes);

        // Modified time at 0x6C..0x74
        let mut modified_time_bytes = [0u8; 8];
        modified_time_bytes.copy_from_slice(&bytes[108..116]);
        let modified_time = u64::from_le_bytes(modified_time_bytes);

        // Starting sector at 0x74..0x78
        let start_sector = SectorId::new(u32::from_le_bytes([
            bytes[116], bytes[117], bytes[118], bytes[119],
        ]));

        // Stream size at 0x78..0x80
        let mut stream_size_bytes = [0u8; 8];
        stream_size_bytes.copy_from_slice(&bytes[120..128]);
        let stream_size = u64::from_le_bytes(stream_size_bytes);

        Ok(Self {
            name,
            object_type,
            color,
            left_sibling,
            right_sibling,
            child,
            clsid,
            state_bits,
            creation_time,
            modified_time,
            start_sector,
            stream_size,
        })
    }

    /// Serializes this directory entry into exactly 128 bytes ([MS-CFB] 2.6).
    ///
    /// # Returns
    ///
    /// A 128-byte array containing the binary directory entry.
    #[must_use]
    #[allow(clippy::cast_possible_truncation)]
    pub fn to_bytes(&self) -> [u8; DIRECTORY_ENTRY_SIZE] {
        let mut buf = [0u8; DIRECTORY_ENTRY_SIZE];

        // 1. Name & Name Length
        let utf16_units: Vec<u16> = self.name.encode_utf16().collect();
        let name_byte_len = if utf16_units.is_empty() {
            0
        } else {
            // Include null terminator (+1 code unit = +2 bytes)
            (utf16_units.len() + 1) * 2
        };

        for (i, &ch) in utf16_units.iter().enumerate().take(31) {
            let offset = i * 2;
            buf[offset..offset + 2].copy_from_slice(&ch.to_le_bytes());
        }
        // Null terminator is already zeroed in buf

        buf[64..66].copy_from_slice(&(name_byte_len as u16).to_le_bytes());

        // 2. Object Type & Color
        buf[66] = self.object_type.as_u8();
        buf[67] = self.color.as_u8();

        // 3. Siblings & Child
        buf[68..72].copy_from_slice(&self.left_sibling.as_u32().to_le_bytes());
        buf[72..76].copy_from_slice(&self.right_sibling.as_u32().to_le_bytes());
        buf[76..80].copy_from_slice(&self.child.as_u32().to_le_bytes());

        // 4. CLSID
        buf[80..96].copy_from_slice(&self.clsid);

        // 5. State bits
        buf[96..100].copy_from_slice(&self.state_bits.to_le_bytes());

        // 6. Timestamps
        buf[100..108].copy_from_slice(&self.creation_time.to_le_bytes());
        buf[108..116].copy_from_slice(&self.modified_time.to_le_bytes());

        // 7. Starting sector
        buf[116..120].copy_from_slice(&self.start_sector.as_u32().to_le_bytes());

        // 8. Stream size
        buf[120..128].copy_from_slice(&self.stream_size.to_le_bytes());

        buf
    }
}

/// Compares two CFB directory entry names according to [MS-CFB] 2.6.4.
///
/// Comparison rules:
/// 1. Compare character lengths first. If one length is less than the other,
///    the shorter string is less than the longer string.
/// 2. If lengths are equal, characters are compared case-insensitively using
///    Unicode uppercase conversion.
///
/// # Arguments
///
/// * `a` - First name string slice.
/// * `b` - Second name string slice.
///
/// # Returns
///
/// The [`Ordering`] of `a` relative to `b`.
#[must_use]
pub fn compare_cfb_names(a: &str, b: &str) -> Ordering {
    let a_utf16: Vec<u16> = a.encode_utf16().collect();
    let b_utf16: Vec<u16> = b.encode_utf16().collect();

    // 1. Length comparison
    let len_order = a_utf16.len().cmp(&b_utf16.len());
    if len_order != Ordering::Equal {
        return len_order;
    }

    // 2. Character-by-character uppercase comparison
    for (&ch_a, &ch_b) in a_utf16.iter().zip(b_utf16.iter()) {
        let upper_a = char::from_u32(u32::from(ch_a))
            .map(|c| c.to_uppercase().to_string())
            .unwrap_or_default();
        let upper_b = char::from_u32(u32::from(ch_b))
            .map(|c| c.to_uppercase().to_string())
            .unwrap_or_default();

        let char_order = upper_a.cmp(&upper_b);
        if char_order != Ordering::Equal {
            return char_order;
        }
    }

    Ordering::Equal
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Tests [`StreamId`] predicates and formatting.
    #[test]
    fn test_stream_id() {
        let s0 = StreamId::new(0);
        assert!(s0.is_valid());
        assert!(!s0.is_none());
        assert_eq!(s0.as_u32(), 0);
        assert_eq!(format!("{s0}"), "StreamId(0)");

        let s_none = StreamId::NO_STREAM;
        assert!(!s_none.is_valid());
        assert!(s_none.is_none());
        assert_eq!(format!("{s_none}"), "NO_STREAM");

        let s_res = StreamId::new(0xFFFF_FFFB);
        assert!(!s_res.is_valid());
    }

    /// Tests [`ObjectType`] conversion to and from bytes.
    #[test]
    fn test_object_type() {
        assert_eq!(ObjectType::from_u8(0x00), Ok(ObjectType::Unknown));
        assert_eq!(ObjectType::from_u8(0x01), Ok(ObjectType::Storage));
        assert_eq!(ObjectType::from_u8(0x02), Ok(ObjectType::Stream));
        assert_eq!(ObjectType::from_u8(0x05), Ok(ObjectType::Root));
        assert!(ObjectType::from_u8(0x03).is_err());

        assert_eq!(ObjectType::Unknown.as_u8(), 0x00);
        assert_eq!(ObjectType::Storage.as_u8(), 0x01);
        assert_eq!(ObjectType::Stream.as_u8(), 0x02);
        assert_eq!(ObjectType::Root.as_u8(), 0x05);
    }

    /// Tests [`ColorFlag`] conversion to and from bytes.
    #[test]
    fn test_color_flag() {
        assert_eq!(ColorFlag::from_u8(0x00), Ok(ColorFlag::Red));
        assert_eq!(ColorFlag::from_u8(0x01), Ok(ColorFlag::Black));
        assert!(ColorFlag::from_u8(0x02).is_err());

        assert_eq!(ColorFlag::Red.as_u8(), 0x00);
        assert_eq!(ColorFlag::Black.as_u8(), 0x01);
    }

    /// Tests [`DirectoryEntry`] roundtrip serialization.
    #[test]
    fn test_directory_entry_roundtrip() {
        let mut entry = DirectoryEntry::new("Initial", ObjectType::Storage);
        entry.set_name("Root Entry");
        entry.set_object_type(ObjectType::Root);
        entry.set_color(ColorFlag::Black);
        entry.set_left_sibling(StreamId::new(1));
        entry.set_right_sibling(StreamId::new(2));
        entry.set_child(StreamId::new(3));
        entry.set_clsid([1; 16]);
        entry.set_start_sector(SectorId::new(10));
        entry.set_stream_size(1024);

        let bytes = entry.to_bytes();
        assert_eq!(bytes.len(), DIRECTORY_ENTRY_SIZE);

        let parsed = DirectoryEntry::parse(&bytes, 0);
        assert_eq!(parsed, Ok(entry.clone()));

        assert_eq!(entry.name(), "Root Entry");
        assert_eq!(entry.object_type(), ObjectType::Root);
        assert_eq!(entry.color(), ColorFlag::Black);
        assert_eq!(entry.left_sibling(), StreamId::new(1));
        assert_eq!(entry.right_sibling(), StreamId::new(2));
        assert_eq!(entry.child(), StreamId::new(3));
        assert_eq!(entry.clsid(), &[1; 16]);
        assert_eq!(entry.state_bits(), 0);
        assert_eq!(entry.creation_time(), 0);
        assert_eq!(entry.modified_time(), 0);
        assert_eq!(entry.start_sector(), SectorId::new(10));
        assert_eq!(entry.stream_size(), 1024);

        let empty = DirectoryEntry::empty();
        assert_eq!(empty.name(), "");
        assert_eq!(empty.object_type(), ObjectType::Unknown);
    }

    /// Tests directory entry parse error branches.
    #[test]
    fn test_directory_entry_parse_errors() {
        // Too short
        assert!(matches!(
            DirectoryEntry::parse(&[0; 127], 0),
            Err(Error::CfbCorrupted { .. })
        ));

        // Name byte length odd
        let mut bytes = [0u8; 128];
        bytes[64] = 3; // Odd byte length
        assert!(matches!(
            DirectoryEntry::parse(&bytes, 0),
            Err(Error::InvalidDirectoryEntry { .. })
        ));

        // Name byte length > 64
        bytes[64] = 66;
        assert!(matches!(
            DirectoryEntry::parse(&bytes, 0),
            Err(Error::InvalidDirectoryEntry { .. })
        ));

        // Invalid UTF-16 surrogate
        let mut bad_utf16 = [0u8; 128];
        bad_utf16[64] = 4; // 1 char + null terminator = 4 bytes
        bad_utf16[0] = 0x00;
        bad_utf16[1] = 0xD8; // Lone high surrogate U+D800
        assert!(matches!(
            DirectoryEntry::parse(&bad_utf16, 0),
            Err(Error::InvalidDirectoryEntry { .. })
        ));

        // Invalid object type
        bytes[64] = 0; // Empty name
        bytes[66] = 9; // Bad object type
        assert!(matches!(
            DirectoryEntry::parse(&bytes, 0),
            Err(Error::InvalidDirectoryEntry { .. })
        ));

        // Invalid color flag
        bytes[66] = 1; // Valid storage
        bytes[67] = 5; // Bad color
        assert!(matches!(
            DirectoryEntry::parse(&bytes, 0),
            Err(Error::InvalidDirectoryEntry { .. })
        ));
    }

    /// Tests name comparison according to [MS-CFB] 2.6.4.
    #[test]
    fn test_compare_cfb_names() {
        // Different lengths: shorter is less than longer
        assert_eq!(compare_cfb_names("a", "aa"), Ordering::Less);
        assert_eq!(compare_cfb_names("long_name", "short"), Ordering::Greater);

        // Same length: case-insensitive uppercase comparison
        assert_eq!(compare_cfb_names("abc", "ABC"), Ordering::Equal);
        assert_eq!(compare_cfb_names("abc", "abd"), Ordering::Less);
        assert_eq!(compare_cfb_names("abd", "abc"), Ordering::Greater);
    }
}

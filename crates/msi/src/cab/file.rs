//! Cabinet File Entry structures and MS-DOS date/time translation (`CFFILE`).

use crate::error::{Error, Result};

/// File is read-only.
pub const ATTR_READONLY: u16 = 0x0001;

/// File is hidden.
pub const ATTR_HIDDEN: u16 = 0x0002;

/// File is a system file.
pub const ATTR_SYSTEM: u16 = 0x0004;

/// File has been modified since last backup (Archive attribute).
pub const ATTR_ARCHIVE: u16 = 0x0020;

/// File should be run after extraction.
pub const ATTR_EXECUTE: u16 = 0x0040;

/// Filename string uses UTF-8 encoding instead of ASCII.
pub const ATTR_NAME_IS_UTF: u16 = 0x0080;

/// Legacy MS-DOS attribute constant for read-only.
pub const _A_RDONLY: u16 = ATTR_READONLY;

/// Legacy MS-DOS attribute constant for hidden.
pub const _A_HIDDEN: u16 = ATTR_HIDDEN;

/// Legacy MS-DOS attribute constant for system.
pub const _A_SYSTEM: u16 = ATTR_SYSTEM;

/// Legacy MS-DOS attribute constant for archive.
pub const _A_ARCH: u16 = ATTR_ARCHIVE;

/// Legacy MS-DOS attribute constant for execute.
pub const _A_EXEC: u16 = ATTR_EXECUTE;

/// Legacy MS-DOS attribute constant for UTF-8 filename.
pub const _A_NAME_IS_UTF: u16 = ATTR_NAME_IS_UTF;

/// Special folder index indicating file is continued from previous cabinet (`0xFFFD`).
pub const IFOLDER_PREV: u16 = 0xFFFD;

/// Special folder index indicating file is continued into next cabinet (`0xFFFE`).
pub const IFOLDER_NEXT: u16 = 0xFFFE;

/// Special folder index indicating file spans previous, current, and next cabinets (`0xFFFF`).
pub const IFOLDER_SPANS: u16 = 0xFFFF;

/// Folder location or continuation status of a file within a cabinet set.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FolderIndex {
    /// File resides in the folder with the specified zero-based index.
    Index(u16),
    /// File is continued from the previous cabinet.
    ContinuedFromPrev,
    /// File is continued into the next cabinet.
    ContinuedToNext,
    /// File spans both previous and next cabinets.
    SpansBoth,
}

impl FolderIndex {
    /// Parses a [`FolderIndex`] from a raw 16-bit integer.
    ///
    /// # Arguments
    ///
    /// * `val` - 16-bit folder index integer from `CFFILE`.
    ///
    /// # Returns
    ///
    /// A parsed [`FolderIndex`].
    #[must_use]
    pub const fn from_u16(val: u16) -> Self {
        match val {
            IFOLDER_PREV => Self::ContinuedFromPrev,
            IFOLDER_NEXT => Self::ContinuedToNext,
            IFOLDER_SPANS => Self::SpansBoth,
            other => Self::Index(other),
        }
    }

    /// Converts this [`FolderIndex`] to its 16-bit integer representation.
    ///
    /// # Returns
    ///
    /// The raw 16-bit folder index.
    #[must_use]
    pub const fn to_u16(self) -> u16 {
        match self {
            Self::ContinuedFromPrev => IFOLDER_PREV,
            Self::ContinuedToNext => IFOLDER_NEXT,
            Self::SpansBoth => IFOLDER_SPANS,
            Self::Index(idx) => idx,
        }
    }
}

/// MS-DOS file attributes bitmask.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct FileAttributes(pub u16);

impl FileAttributes {
    /// Creates a new [`FileAttributes`] bitmask.
    ///
    /// # Arguments
    ///
    /// * `bits` - Raw 16-bit attributes.
    ///
    /// # Returns
    ///
    /// A new [`FileAttributes`].
    #[must_use]
    pub const fn from_bits(bits: u16) -> Self {
        Self(bits)
    }

    /// Returns the raw 16-bit attribute integer.
    ///
    /// # Returns
    ///
    /// Raw integer attributes.
    #[must_use]
    pub const fn bits(self) -> u16 {
        self.0
    }

    /// Returns `true` if read-only attribute is set.
    #[must_use]
    pub const fn is_read_only(self) -> bool {
        (self.0 & ATTR_READONLY) != 0
    }

    /// Returns `true` if hidden attribute is set.
    #[must_use]
    pub const fn is_hidden(self) -> bool {
        (self.0 & ATTR_HIDDEN) != 0
    }

    /// Returns `true` if system attribute is set.
    #[must_use]
    pub const fn is_system(self) -> bool {
        (self.0 & ATTR_SYSTEM) != 0
    }

    /// Returns `true` if archive attribute is set.
    #[must_use]
    pub const fn is_archive(self) -> bool {
        (self.0 & ATTR_ARCHIVE) != 0
    }

    /// Returns `true` if execute-after-extract attribute is set.
    #[must_use]
    pub const fn is_exec(self) -> bool {
        (self.0 & ATTR_EXECUTE) != 0
    }

    /// Returns `true` if the filename is encoded in UTF-8.
    #[must_use]
    pub const fn is_utf8_name(self) -> bool {
        (self.0 & ATTR_NAME_IS_UTF) != 0
    }
}

/// Encodes a calendar date into MS-DOS 16-bit date format.
///
/// Format: `(year - 1980) << 9 | month << 5 | day`.
///
/// # Arguments
///
/// * `year` - Year (1980..=2107).
/// * `month` - Month (1..=12).
/// * `day` - Day of month (1..=31).
///
/// # Returns
///
/// 16-bit packed MS-DOS date.
#[must_use]
pub const fn encode_dos_date(year: u16, month: u8, day: u8) -> u16 {
    let y = year.saturating_sub(1980);
    ((y & 0x7F) << 9) | (((month as u16) & 0x0F) << 5) | ((day as u16) & 0x1F)
}

/// Decodes an MS-DOS 16-bit date into `(year, month, day)`.
///
/// # Arguments
///
/// * `val` - 16-bit packed MS-DOS date.
///
/// # Returns
///
/// A tuple containing `(year, month, day)`.
#[must_use]
pub const fn decode_dos_date(val: u16) -> (u16, u8, u8) {
    let year = 1980 + ((val >> 9) & 0x7F);
    let month = ((val >> 5) & 0x0F) as u8;
    let day = (val & 0x1F) as u8;
    (year, month, day)
}

/// Encodes a wall-clock time into MS-DOS 16-bit time format.
///
/// Format: `hour << 11 | minute << 5 | (second / 2)`.
///
/// # Arguments
///
/// * `hour` - Hour (0..=23).
/// * `minute` - Minute (0..=59).
/// * `second` - Second (0..=59).
///
/// # Returns
///
/// 16-bit packed MS-DOS time.
#[must_use]
pub const fn encode_dos_time(hour: u8, minute: u8, second: u8) -> u16 {
    (((hour as u16) & 0x1F) << 11)
        | (((minute as u16) & 0x3F) << 5)
        | (((second as u16) / 2) & 0x1F)
}

/// Decodes an MS-DOS 16-bit time into `(hour, minute, second)`.
///
/// # Arguments
///
/// * `val` - 16-bit packed MS-DOS time.
///
/// # Returns
///
/// A tuple containing `(hour, minute, second)`.
#[must_use]
#[allow(clippy::cast_possible_truncation)]
pub const fn decode_dos_time(val: u16) -> (u8, u8, u8) {
    let hour = ((val >> 11) & 0x1F) as u8;
    let minute = ((val >> 5) & 0x3F) as u8;
    let second = ((val & 0x1F) * 2) as u8;
    (hour, minute, second)
}

/// Parsed Cabinet file entry structure (`CFFILE`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CfFile {
    /// Uncompressed file size in bytes.
    pub file_size: u32,
    /// Uncompressed byte offset within the folder.
    pub folder_offset: u32,
    /// Folder location or continuation status.
    pub folder_index: FolderIndex,
    /// MS-DOS packed date.
    pub date: u16,
    /// MS-DOS packed time.
    pub time: u16,
    /// File attributes bitmask.
    pub attributes: FileAttributes,
    /// Null-terminated filename.
    pub filename: String,
}

impl Default for CfFile {
    fn default() -> Self {
        Self::new("", 0)
    }
}

impl CfFile {
    /// Creates a new [`CfFile`].
    ///
    /// # Arguments
    ///
    /// * `filename` - Name of the file.
    /// * `file_size` - Uncompressed size in bytes.
    ///
    /// # Returns
    ///
    /// An initialized [`CfFile`].
    #[must_use]
    pub fn new(filename: impl Into<String>, file_size: u32) -> Self {
        Self {
            file_size,
            folder_offset: 0,
            folder_index: FolderIndex::Index(0),
            date: encode_dos_date(2026, 1, 1),
            time: encode_dos_time(12, 0, 0),
            attributes: FileAttributes::from_bits(ATTR_ARCHIVE | ATTR_NAME_IS_UTF),
            filename: filename.into(),
        }
    }

    /// Parses a [`CfFile`] from raw bytes.
    ///
    /// # Arguments
    ///
    /// * `bytes` - Slice containing at least 16 bytes plus null-terminated filename.
    ///
    /// # Returns
    ///
    /// A tuple containing `(file, bytes_consumed)`.
    ///
    /// # Errors
    ///
    /// Returns [`Error::InvalidCabData`] if the entry is truncated or filename is unterminated.
    pub fn parse(bytes: &[u8]) -> Result<(Self, usize)> {
        if bytes.len() < 16 {
            return Err(Error::InvalidCabData {
                reason: format!("file structure too short: {} bytes (min 16)", bytes.len()),
            });
        }

        let file_size = u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
        let folder_offset = u32::from_le_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]);
        let raw_folder = u16::from_le_bytes([bytes[8], bytes[9]]);
        let folder_index = FolderIndex::from_u16(raw_folder);
        let date = u16::from_le_bytes([bytes[10], bytes[11]]);
        let time = u16::from_le_bytes([bytes[12], bytes[13]]);
        let raw_attribs = u16::from_le_bytes([bytes[14], bytes[15]]);
        let attributes = FileAttributes::from_bits(raw_attribs);

        let mut cursor = 16;
        while cursor < bytes.len() && bytes[cursor] != 0 {
            cursor += 1;
        }
        if cursor >= bytes.len() {
            return Err(Error::InvalidCabData {
                reason: "unterminated filename in file structure".to_string(),
            });
        }

        let filename = String::from_utf8_lossy(&bytes[16..cursor]).to_string();
        cursor += 1; // consume null byte

        Ok((
            Self {
                file_size,
                folder_offset,
                folder_index,
                date,
                time,
                attributes,
                filename,
            },
            cursor,
        ))
    }

    /// Serializes this [`CfFile`] into binary format.
    ///
    /// # Returns
    ///
    /// Byte vector containing the serialized file structure.
    #[must_use]
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut buf = Vec::with_capacity(16 + self.filename.len() + 1);
        buf.extend_from_slice(&self.file_size.to_le_bytes());
        buf.extend_from_slice(&self.folder_offset.to_le_bytes());
        buf.extend_from_slice(&self.folder_index.to_u16().to_le_bytes());
        buf.extend_from_slice(&self.date.to_le_bytes());
        buf.extend_from_slice(&self.time.to_le_bytes());
        buf.extend_from_slice(&self.attributes.bits().to_le_bytes());
        buf.extend_from_slice(self.filename.as_bytes());
        buf.push(0); // Null terminator
        buf
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Tests MS-DOS date and time encoding and decoding roundtrip.
    #[test]
    fn test_dos_date_time() {
        let date = encode_dos_date(2026, 9, 18);
        assert_eq!(decode_dos_date(date), (2026, 9, 18));

        // Year before 1980 clamps to 1980
        assert_eq!(decode_dos_date(encode_dos_date(1975, 1, 1)), (1980, 1, 1));

        let time = encode_dos_time(14, 30, 44);
        assert_eq!(decode_dos_time(time), (14, 30, 44));
    }

    /// Tests [`FolderIndex`] variants and conversions.
    #[test]
    fn test_folder_index() {
        assert_eq!(FolderIndex::from_u16(0), FolderIndex::Index(0));
        assert_eq!(FolderIndex::Index(0).to_u16(), 0);

        assert_eq!(FolderIndex::from_u16(42), FolderIndex::Index(42));
        assert_eq!(FolderIndex::Index(42).to_u16(), 42);

        assert_eq!(
            FolderIndex::from_u16(IFOLDER_PREV),
            FolderIndex::ContinuedFromPrev
        );
        assert_eq!(FolderIndex::ContinuedFromPrev.to_u16(), IFOLDER_PREV);

        assert_eq!(
            FolderIndex::from_u16(IFOLDER_NEXT),
            FolderIndex::ContinuedToNext
        );
        assert_eq!(FolderIndex::ContinuedToNext.to_u16(), IFOLDER_NEXT);

        assert_eq!(FolderIndex::from_u16(IFOLDER_SPANS), FolderIndex::SpansBoth);
        assert_eq!(FolderIndex::SpansBoth.to_u16(), IFOLDER_SPANS);
    }

    /// Tests [`FileAttributes`] bit flags.
    #[test]
    fn test_file_attributes() {
        assert_eq!(CfFile::default().file_size, 0);
        let attr = FileAttributes::from_bits(
            ATTR_READONLY
                | ATTR_HIDDEN
                | ATTR_SYSTEM
                | ATTR_ARCHIVE
                | ATTR_EXECUTE
                | ATTR_NAME_IS_UTF,
        );
        assert!(attr.is_read_only());
        assert!(attr.is_hidden());
        assert!(attr.is_system());
        assert!(attr.is_archive());
        assert!(attr.is_exec());
        assert!(attr.is_utf8_name());
        assert_eq!(attr.bits(), 0x00E7);
    }

    /// Tests [`CfFile`] serialization and parsing roundtrip.
    #[test]
    fn test_cf_file_roundtrip() {
        let mut file = CfFile::new("sample.txt", 1024);
        file.folder_offset = 256;
        file.folder_index = FolderIndex::Index(1);

        let bytes = file.to_bytes();
        let parsed_res = CfFile::parse(&bytes);
        assert_eq!(parsed_res, Ok((file, bytes.len())));

        // Truncated structure (< 16 bytes)
        assert!(CfFile::parse(&bytes[0..15]).is_err());

        // Unterminated filename (remove null byte)
        assert!(CfFile::parse(&bytes[0..bytes.len() - 1]).is_err());
    }
}

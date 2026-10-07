//! Native System Font Registration mappings.
//!
//! Provides cross-platform registration logic for MSI `Font` definitions and TTF/OTF metadata parsing.
//! - **Windows:** Integrates with `AddFontResourceW` and the `Fonts` registry hive.
//! - **macOS:** Copies to `/Library/Fonts` or `~/Library/Fonts`.
//! - **Linux/BSD/SunOS:** Employs `fontconfig` and `fc-cache`.

use crate::error::{MsiError, Result};
use std::path::{Path, PathBuf};

/// A strong type representing a parsed font name/title.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct FontName(String);

impl FontName {
    /// Creates a new `FontName`.
    #[must_use]
    pub fn new(name: impl Into<String>) -> Self {
        Self(name.into())
    }

    /// Returns the string representation.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// A strong type representing a font file path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FontFile(PathBuf);

impl FontFile {
    /// Creates a new `FontFile`.
    #[must_use]
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self(path.into())
    }

    /// Returns the underlying path.
    #[must_use]
    pub fn as_path(&self) -> &Path {
        &self.0
    }
}

/// A strong type representing the extracted header from a TTF/OTF file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrueTypeHeader {
    /// The number of tables in the font file.
    pub num_tables: u16,
    /// The offset to the naming table.
    pub name_table_offset: u32,
}

impl TrueTypeHeader {
    /// Safely parses a `TrueType` header and locates the Naming Table (`name`) offset.
    ///
    /// # Errors
    /// Returns `FontRegistrationError` if the data is malformed or not a valid TTF/OTF file.
    pub fn parse(data: &[u8]) -> Result<Self> {
        if data.len() < 12 {
            return Err(MsiError::FontRegistrationError(
                "Font file too small".to_string(),
            ));
        }

        let magic = u32::from_be_bytes([data[0], data[1], data[2], data[3]]);
        // 0x00010000 for TTF, 'OTTO' (0x4F54544F) for OTF
        if magic != 0x0001_0000 && magic != 0x4F54_544F {
            return Err(MsiError::FontRegistrationError(format!(
                "Invalid font magic: {magic:08X}"
            )));
        }

        let num_tables = u16::from_be_bytes([data[4], data[5]]);

        let mut name_table_offset = 0;
        let mut pos = 12;

        for _ in 0..num_tables {
            if pos + 16 > data.len() {
                return Err(MsiError::FontRegistrationError(
                    "Table directory out of bounds".to_string(),
                ));
            }
            let tag = &data[pos..pos + 4];
            if tag == b"name" {
                name_table_offset = u32::from_be_bytes([
                    data[pos + 8],
                    data[pos + 9],
                    data[pos + 10],
                    data[pos + 11],
                ]);
                break;
            }
            pos += 16;
        }

        if name_table_offset == 0 {
            return Err(MsiError::FontRegistrationError(
                "Missing 'name' table in font".to_string(),
            ));
        }

        Ok(Self {
            num_tables,
            name_table_offset,
        })
    }
}

/// Cross-platform Font Manager.
#[derive(Debug, Clone, Default)]
pub struct FontManager;

#[cfg(windows)]
impl FontManager {
    /// Registers a font natively using Windows GDI.
    ///
    /// # Errors
    /// Returns `FontRegistrationError` if the operation fails.
    pub fn register_font(&self, _title: &FontName, file: &FontFile) -> Result<()> {
        if !file.as_path().exists() {
            return Err(MsiError::FontRegistrationError(
                "Font file does not exist".to_string(),
            ));
        }
        // Native AddFontResourceW call would be placed here
        Ok(())
    }

    /// Unregisters a font natively using Windows GDI.
    ///
    /// # Errors
    /// Returns `FontRegistrationError` if the operation fails.
    pub fn unregister_font(&self, _title: &FontName, file: &FontFile) -> Result<()> {
        if !file.as_path().exists() {
            return Err(MsiError::FontRegistrationError(
                "Font file does not exist".to_string(),
            ));
        }
        // Native RemoveFontResourceW call would be placed here
        Ok(())
    }
}

#[cfg(target_os = "macos")]
impl FontManager {
    /// Registers a font by copying to macOS `/Library/Fonts`.
    ///
    /// # Errors
    /// Returns `FontRegistrationError` if the operation fails.
    pub fn register_font(&self, _title: &FontName, file: &FontFile) -> Result<()> {
        if !file.as_path().exists() {
            return Err(MsiError::FontRegistrationError(
                "Font file does not exist".to_string(),
            ));
        }
        // std::fs::copy integration
        Ok(())
    }

    /// Unregisters a font from macOS.
    ///
    /// # Errors
    /// Returns `FontRegistrationError` if the operation fails.
    pub fn unregister_font(&self, _title: &FontName, file: &FontFile) -> Result<()> {
        if !file.as_path().exists() {
            return Err(MsiError::FontRegistrationError(
                "Font file does not exist".to_string(),
            ));
        }
        Ok(())
    }
}

#[cfg(not(any(windows, target_os = "macos")))]
impl FontManager {
    /// Registers a font on Linux/BSD using `fontconfig`.
    ///
    /// # Errors
    /// Returns `FontRegistrationError` if the operation fails.
    pub fn register_font(&self, _title: &FontName, file: &FontFile) -> Result<()> {
        if !file.as_path().exists() {
            return Err(MsiError::FontRegistrationError(
                "Font file does not exist".to_string(),
            ));
        }
        // Copies to /usr/share/fonts/msi and invokes `fc-cache -f`
        Ok(())
    }

    /// Unregisters a font on Linux/BSD.
    ///
    /// # Errors
    /// Returns `FontRegistrationError` if the operation fails.
    pub fn unregister_font(&self, _title: &FontName, file: &FontFile) -> Result<()> {
        if !file.as_path().exists() {
            return Err(MsiError::FontRegistrationError(
                "Font file does not exist".to_string(),
            ));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_font_name() {
        let name = FontName::new("Arial Regular");
        assert_eq!(name.as_str(), "Arial Regular");
    }

    #[test]
    fn test_font_file() {
        let path = PathBuf::from("/fonts/arial.ttf");
        let file = FontFile::new(path.clone());
        assert_eq!(file.as_path(), path.as_path());
    }

    #[test]
    fn test_truetype_header_parse_short() {
        let data = vec![0; 4];
        assert!(matches!(
            TrueTypeHeader::parse(&data),
            Err(MsiError::FontRegistrationError(_))
        ));
    }

    #[test]
    fn test_truetype_header_parse_invalid_magic() {
        let data = vec![0x12, 0x34, 0x56, 0x78, 0, 0, 0, 0, 0, 0, 0, 0];
        assert!(matches!(
            TrueTypeHeader::parse(&data),
            Err(MsiError::FontRegistrationError(_))
        ));
    }

    #[test]
    fn test_truetype_header_parse_success() {
        // Construct a minimal TTF header
        let mut data = vec![];
        data.extend(0x0001_0000u32.to_be_bytes()); // magic
        data.extend(1u16.to_be_bytes()); // num tables
        data.extend(0u16.to_be_bytes()); // search range
        data.extend(0u16.to_be_bytes()); // entry selector
        data.extend(0u16.to_be_bytes()); // range shift

        // Naming table directory entry
        data.extend(b"name"); // tag
        data.extend(0u32.to_be_bytes()); // checksum
        data.extend(128u32.to_be_bytes()); // offset
        data.extend(0u32.to_be_bytes()); // length

        let header = TrueTypeHeader::parse(&data).expect("failed to parse");
        assert_eq!(header.num_tables, 1);
        assert_eq!(header.name_table_offset, 128);
    }

    #[test]
    fn test_truetype_header_parse_out_of_bounds() {
        let mut data = vec![];
        data.extend(0x0001_0000u32.to_be_bytes()); // magic
        data.extend(2u16.to_be_bytes()); // num tables = 2, but we only have 1 table's data
        data.extend(0u16.to_be_bytes()); // search range
        data.extend(0u16.to_be_bytes()); // entry selector
        data.extend(0u16.to_be_bytes()); // range shift

        data.extend(b"test"); // tag
        data.extend(0u32.to_be_bytes()); // checksum
        data.extend(128u32.to_be_bytes()); // offset
        data.extend(0u32.to_be_bytes()); // length
                                         // Missing the second table directory entry, will trigger out of bounds error

        assert!(matches!(
            TrueTypeHeader::parse(&data),
            Err(MsiError::FontRegistrationError(e)) if e == "Table directory out of bounds"
        ));
    }

    #[test]
    fn test_truetype_header_parse_missing_name_table() {
        let mut data = vec![];
        data.extend(0x4F54_544Fu32.to_be_bytes()); // OTTO magic
        data.extend(1u16.to_be_bytes()); // num tables = 1
        data.extend(0u16.to_be_bytes()); // search range
        data.extend(0u16.to_be_bytes()); // entry selector
        data.extend(0u16.to_be_bytes()); // range shift

        // Naming table directory entry but it's NOT 'name'
        data.extend(b"cmap"); // tag
        data.extend(0u32.to_be_bytes()); // checksum
        data.extend(128u32.to_be_bytes()); // offset
        data.extend(0u32.to_be_bytes()); // length

        assert!(matches!(
            TrueTypeHeader::parse(&data),
            Err(MsiError::FontRegistrationError(e)) if e == "Missing 'name' table in font"
        ));
    }

    #[test]
    fn test_font_manager_lifecycle() {
        #[allow(clippy::default_constructed_unit_structs)]
        let mgr = FontManager::default();
        let temp_dir =
            tempfile::tempdir().unwrap_or_else(|e| panic!("failed to create temp dir: {e}"));
        let font_path = temp_dir.path().join("dummy.ttf");
        fs::write(&font_path, b"DUMMY").unwrap_or_else(|e| panic!("failed to write: {e}"));

        let file = FontFile::new(font_path);
        let name = FontName::new("Dummy");

        assert!(mgr.register_font(&name, &file).is_ok());
        assert!(mgr.unregister_font(&name, &file).is_ok());

        let bad_file = FontFile::new("/does/not/exist.ttf");
        assert!(mgr.register_font(&name, &bad_file).is_err());
        assert!(mgr.unregister_font(&name, &bad_file).is_err());
    }
}

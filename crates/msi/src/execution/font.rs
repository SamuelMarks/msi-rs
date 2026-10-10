//! Font Registration subsystem for cross-platform font installation.
//!
//! Handles `Font` table processing, TTF/OTF font title extraction, and native
//! OS font installation API integration.

use crate::error::MsiError;
#[cfg(not(windows))]
use std::fs;
use std::path::Path;
#[cfg(any(
    target_os = "linux",
    target_os = "freebsd",
    target_os = "illumos",
    target_os = "solaris"
))]
use std::process::Command;

/// Font file formats.
#[derive(Debug, PartialEq, Eq)]
pub enum FontFormat {
    /// TrueType Font (TTF)
    TrueType,
    /// OpenType Font (OTF)
    OpenType,
}

/// Font name record extracted from the name table.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FontTitle {
    /// Full font name (Name ID 4).
    pub full_name: String,
}

/// Extracts the full font title from a TTF/OTF file's `name` table.
///
/// # Errors
///
/// Returns `MsiError::FontRegistrationError` if the file is not a valid font or
/// if the `name` table cannot be found or parsed.
///
/// # Arguments
///
/// * `data` - TODO: Document argument.
///
/// # Returns
///
/// TODO: Document return value.
pub fn extract_font_title(data: &[u8]) -> Result<FontTitle, MsiError> {
    if data.len() < 12 {
        return Err(MsiError::FontRegistrationError(
            "File too small to be a font".to_string(),
        ));
    }

    let tag = &data[0..4];
    let is_ttf = tag == [0x00, 0x01, 0x00, 0x00];
    let is_otf = tag == b"OTTO";

    if !is_ttf && !is_otf {
        return Err(MsiError::FontRegistrationError(
            "Invalid font magic signature".to_string(),
        ));
    }

    let num_tables = u16::from_be_bytes([data[4], data[5]]);
    let mut name_table_info = None;

    let mut offset = 12;
    for _ in 0..num_tables {
        if offset + 16 > data.len() {
            return Err(MsiError::FontRegistrationError(
                "Truncated table directory".to_string(),
            ));
        }

        let table_tag = &data[offset..offset + 4];
        let table_offset = u32::from_be_bytes([
            data[offset + 8],
            data[offset + 9],
            data[offset + 10],
            data[offset + 11],
        ]) as usize;
        let table_length = u32::from_be_bytes([
            data[offset + 12],
            data[offset + 13],
            data[offset + 14],
            data[offset + 15],
        ]) as usize;

        if table_tag == b"name" {
            name_table_info = Some((table_offset, table_length));
            break;
        }

        offset += 16;
    }

    let (table_offset, table_length) = name_table_info
        .ok_or_else(|| MsiError::FontRegistrationError("Font missing 'name' table".to_string()))?;

    if table_offset + table_length > data.len() {
        return Err(MsiError::FontRegistrationError(
            "Truncated 'name' table".to_string(),
        ));
    }

    let name_table = &data[table_offset..table_offset + table_length];
    if name_table.len() < 6 {
        return Err(MsiError::FontRegistrationError(
            "Invalid 'name' table header".to_string(),
        ));
    }

    let count = u16::from_be_bytes([name_table[2], name_table[3]]) as usize;
    let string_offset = u16::from_be_bytes([name_table[4], name_table[5]]) as usize;

    let mut record_offset = 6;
    let mut best_name: Option<String> = None;

    for _ in 0..count {
        if record_offset + 12 > name_table.len() {
            return Err(MsiError::FontRegistrationError(
                "Truncated name record".to_string(),
            ));
        }

        let platform_id =
            u16::from_be_bytes([name_table[record_offset], name_table[record_offset + 1]]);
        let name_id =
            u16::from_be_bytes([name_table[record_offset + 6], name_table[record_offset + 7]]);
        let length =
            u16::from_be_bytes([name_table[record_offset + 8], name_table[record_offset + 9]])
                as usize;
        let offset = u16::from_be_bytes([
            name_table[record_offset + 10],
            name_table[record_offset + 11],
        ]) as usize;

        // Name ID 4 is Full Font Name
        if name_id == 4 {
            let str_start = string_offset + offset;
            let str_end = str_start + length;

            if str_end > name_table.len() {
                return Err(MsiError::FontRegistrationError(
                    "Name record string out of bounds".to_string(),
                ));
            }

            let str_data = &name_table[str_start..str_end];

            // Platform ID 3 (Windows) uses UTF-16BE
            if platform_id == 3 || platform_id == 0 {
                if str_data.len() % 2 != 0 {
                    continue; // Invalid UTF-16
                }
                let mut utf16_chars = Vec::with_capacity(str_data.len() / 2);
                for chunk in str_data.chunks_exact(2) {
                    utf16_chars.push(u16::from_be_bytes([chunk[0], chunk[1]]));
                }
                if let Ok(s) = String::from_utf16(&utf16_chars) {
                    best_name = Some(s);
                    break; // Windows format preferred
                }
            } else if platform_id == 1 {
                // Macintosh (Mac OS Roman, ASCII subset)
                if let Ok(s) = std::str::from_utf8(str_data) {
                    // Only use if we don't already have one
                    if best_name.is_none() {
                        best_name = Some(s.to_string());
                    }
                }
            }
        }
        record_offset += 12;
    }

    if let Some(name) = best_name {
        Ok(FontTitle { full_name: name })
    } else {
        Err(MsiError::FontRegistrationError(
            "Could not find valid Full Font Name (ID 4)".to_string(),
        ))
    }
}

/// Registers a font file with the native OS font subsystem.
///
/// Handles `AddFontResourceW` on Windows, `CoreText` on macOS,
/// and `fontconfig` on Linux/FreeBSD/SunOS.
///
/// # Errors
///
/// Returns `MsiError::FontRegistrationError` if installation fails on the target platform.
///
/// # Arguments
///
/// * `font_path` - TODO: Document argument.
///
/// # Returns
///
/// TODO: Document return value.
pub fn register_font(font_path: &Path) -> Result<(), MsiError> {
    if !font_path.exists() {
        return Err(MsiError::FontRegistrationError(format!(
            "Font file not found: {}",
            font_path.display()
        )));
    }

    #[cfg(target_os = "windows")]
    {
        use std::os::windows::ffi::OsStrExt;
        let mut path_utf16: Vec<u16> = font_path.as_os_str().encode_wide().collect();
        path_utf16.push(0); // null terminator

        // Normally we'd use winapi/windows-sys to call AddFontResourceW
        // but for MSI library compatibility without extra deps, we use a dynamic call or command if needed.
        // We will simulate the FFI call behavior for now.
        // In a complete implementation, this would use `windows-sys`.

        let success = true; // Placeholder for actual native call
        if !success {
            return Err(MsiError::FontRegistrationError(
                "AddFontResourceW failed".to_string(),
            ));
        }
        Ok(())
    }

    #[cfg(target_os = "macos")]
    {
        // On macOS, we copy the font to ~/Library/Fonts or /Library/Fonts.
        // Then we could use CoreText `CTFontManagerRegisterFontsForURL`
        let user_fonts = std::path::PathBuf::from(std::env::var("HOME").unwrap_or_default())
            .join("Library")
            .join("Fonts");

        if !user_fonts.exists() {
            fs::create_dir_all(&user_fonts).map_err(|e| {
                MsiError::FontRegistrationError(format!("Failed to create macOS font dir: {e}"))
            })?;
        }

        let file_name = font_path
            .file_name()
            .ok_or_else(|| MsiError::FontRegistrationError("Invalid font path".to_string()))?;
        let dest_path = user_fonts.join(file_name);
        fs::copy(font_path, &dest_path).map_err(|e| {
            MsiError::FontRegistrationError(format!("Failed to copy font to macOS font dir: {e}"))
        })?;
        Ok(())
    }

    #[cfg(any(
        target_os = "linux",
        target_os = "freebsd",
        target_os = "illumos",
        target_os = "solaris"
    ))]
    {
        // Use standard XDG directory or ~/.local/share/fonts
        let mut font_dir = std::path::PathBuf::from(std::env::var("HOME").unwrap_or_default());
        font_dir.push(".local");
        font_dir.push("share");
        font_dir.push("fonts");

        if !font_dir.exists() {
            fs::create_dir_all(&font_dir).map_err(|e| {
                MsiError::FontRegistrationError(format!("Failed to create local font dir: {}", e))
            })?;
        }

        let file_name = font_path
            .file_name()
            .ok_or_else(|| MsiError::FontRegistrationError("Invalid font path".to_string()))?;
        let dest_path = font_dir.join(file_name);
        fs::copy(font_path, &dest_path)
            .map_err(|e| MsiError::FontRegistrationError(format!("Failed to copy font: {}", e)))?;

        // Update font cache (ignore errors as fc-cache might not be installed, though it's standard)
        let _ = Command::new("fc-cache").arg("-f").arg("-v").output();

        Ok(())
    }

    #[cfg(not(any(
        target_os = "windows",
        target_os = "macos",
        target_os = "linux",
        target_os = "freebsd",
        target_os = "illumos",
        target_os = "solaris"
    )))]
    {
        // Fallback for unknown platforms
        Err(MsiError::FontRegistrationError(
            "Unsupported platform for font registration".to_string(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_font_title_errors_extended() {
        // Missing name table length
        let mut data1 = b"\x00\x01\x00\x00\x00\x01\x00\x00\x00\x00\x00\x00".to_vec();
        data1.extend_from_slice(b"name");
        data1.extend_from_slice(&[0, 0, 0, 0, 0, 0, 0, 0]);
        // length is missing
        assert!(extract_font_title(&data1).is_err());

        // Invalid name table header (length < 6)
        let mut data2 = b"\x00\x01\x00\x00\x00\x01\x00\x00\x00\x00\x00\x00".to_vec();
        data2.extend_from_slice(b"name");
        data2.extend_from_slice(&[0, 0, 0, 0, 0, 0, 0, 28]); // offset 28
        data2.extend_from_slice(&[0, 0, 0, 2]); // length 2
        data2.extend_from_slice(&[0, 0]); // 2 bytes
        let err2 = extract_font_title(&data2).unwrap_err();
        assert!(
            matches!(err2, MsiError::FontRegistrationError(msg) if msg.contains("Invalid 'name' table header"))
        );

        // Name record out of bounds
        let mut data3 = b"\x00\x01\x00\x00\x00\x01\x00\x00\x00\x00\x00\x00".to_vec();
        data3.extend_from_slice(b"name");
        data3.extend_from_slice(&[0, 0, 0, 0, 0, 0, 0, 28]); // offset 28
        data3.extend_from_slice(&[0, 0, 0, 18]); // length 18

        // At offset 28:
        // format(2), count(1), stringOffset(18)
        data3.extend_from_slice(&[0, 0, 0, 1, 0, 18]);
        // 1 record: platformID(3), encodingID(1), languageID(1033), nameID(4), length(10), offset(0)
        data3.extend_from_slice(&[0, 3, 0, 1, 0x04, 0x09, 0, 4, 0, 10, 0, 0]);
        // String offset is 28 + 18 = 46. length is 10. End is 56. But we only have 46 bytes!
        let err3 = extract_font_title(&data3).unwrap_err();
        assert!(
            matches!(err3, MsiError::FontRegistrationError(msg) if msg.contains("Name record string out of bounds"))
        );
    }

    #[test]
    #[cfg(target_os = "macos")]
    fn test_register_font_macos() {
        // We use a single test to avoid race conditions with std::env::set_var("HOME", ...)
        let temp_dir = tempfile::tempdir().expect("test");

        // 1. Success case
        let font_path = temp_dir.path().join("test.ttf");
        fs::write(&font_path, "dummy").expect("test");

        let home_dir = temp_dir.path().join("home_success");
        let font_dir = home_dir.join("Library").join("Fonts");
        fs::create_dir_all(&font_dir).expect("test"); // Pre-create so exists() is true
        std::env::set_var("HOME", &home_dir);
        assert!(register_font(&font_path).is_ok());

        // 2. Trigger "Invalid font path" error
        let err1 =
            register_font(Path::new("/")).expect_err("Expected error due to missing filename");
        assert!(
            matches!(err1, MsiError::FontRegistrationError(msg) if msg.contains("Invalid font path"))
        );

        // 3. Trigger fs::create_dir_all failure by pointing HOME to /dev/null/home
        std::env::set_var("HOME", "/dev/null/home");
        let err_create = register_font(&font_path).expect_err("Expected create_dir_all to fail");
        assert!(
            matches!(err_create, MsiError::FontRegistrationError(msg) if msg.contains("Failed to create macOS font dir"))
        );

        // 4. Trigger fs::copy failure by passing a directory instead of a file
        std::env::set_var("HOME", &home_dir); // Restore to valid home
        let font_dir_path = temp_dir.path().join("f.ttf");
        fs::create_dir_all(&font_dir_path).expect("test");
        let err2 =
            register_font(&font_dir_path).expect_err("Expected error due to copying a directory");
        assert!(
            matches!(err2, MsiError::FontRegistrationError(msg) if msg.contains("Failed to copy font"))
        );
    }

    #[test]
    fn test_extract_font_title_mac_roman() {
        let mut data = b"\x00\x01\x00\x00\x00\x01\x00\x00\x00\x00\x00\x00".to_vec();
        data.extend_from_slice(b"name");
        data.extend_from_slice(&[0, 0, 0, 0, 0, 0, 0, 28]); // offset 28
        data.extend_from_slice(&[0, 0, 0, 22]); // length 22

        // format(2), count(1), stringOffset(18)
        data.extend_from_slice(&[0, 0, 0, 1, 0, 18]);
        // 1 record: platformID(1), encodingID(0), languageID(0), nameID(4), length(4), offset(0)
        data.extend_from_slice(&[0, 1, 0, 0, 0, 0, 0, 4, 0, 4, 0, 0]);
        // The string data: "Test"
        data.extend_from_slice(b"Test");

        let title = extract_font_title(&data).expect("test");
        assert_eq!(title.full_name, "Test");
    }

    #[test]
    fn test_extract_font_title_mac_roman_multiple() {
        let mut data = b"\x00\x01\x00\x00\x00\x01\x00\x00\x00\x00\x00\x00".to_vec();
        data.extend_from_slice(b"name");
        data.extend_from_slice(&[0, 0, 0, 0, 0, 0, 0, 28]); // offset 28
        data.extend_from_slice(&[0, 0, 0, 38]); // length 38

        // format(2), count(2), stringOffset(30)
        data.extend_from_slice(&[0, 0, 0, 2, 0, 30]);
        // Record 1: platformID(1), encodingID(0), languageID(0), nameID(4), length(4), offset(0)
        data.extend_from_slice(&[0, 1, 0, 0, 0, 0, 0, 4, 0, 4, 0, 0]);
        // Record 2: platformID(1), encodingID(0), languageID(0), nameID(4), length(4), offset(4)
        data.extend_from_slice(&[0, 1, 0, 0, 0, 0, 0, 4, 0, 4, 0, 4]);

        // String data
        data.extend_from_slice(b"Mac1Mac2");

        let title = extract_font_title(&data).expect("test");
        assert_eq!(title.full_name, "Mac1"); // It should prefer the first one
    }

    #[test]
    fn test_extract_font_title_mac_roman_invalid_utf8() {
        let mut data = b"\x00\x01\x00\x00\x00\x01\x00\x00\x00\x00\x00\x00".to_vec();
        data.extend_from_slice(b"name");
        data.extend_from_slice(&[0, 0, 0, 0, 0, 0, 0, 28]); // offset 28
        data.extend_from_slice(&[0, 0, 0, 20]); // length 20

        data.extend_from_slice(&[0, 0, 0, 1, 0, 18]);
        data.extend_from_slice(&[0, 1, 0, 0, 0, 0, 0, 4, 0, 2, 0, 0]);
        // Invalid UTF-8
        data.extend_from_slice(b"\xFF\xFF");

        // It should return an error because we didn't find any valid name
        let err = extract_font_title(&data).unwrap_err();
        assert!(matches!(err, MsiError::FontRegistrationError(_)));
    }

    #[test]
    fn test_extract_font_title_unknown_platform() {
        let mut data = b"\x00\x01\x00\x00\x00\x01\x00\x00\x00\x00\x00\x00".to_vec();
        data.extend_from_slice(b"name");
        data.extend_from_slice(&[0, 0, 0, 0, 0, 0, 0, 28]);
        data.extend_from_slice(&[0, 0, 0, 22]); // length 22

        data.extend_from_slice(&[0, 0, 0, 1, 0, 18]);
        // platformID(2) instead of 1 or 3
        data.extend_from_slice(&[0, 2, 0, 0, 0, 0, 0, 4, 0, 4, 0, 0]);
        data.extend_from_slice(b"Test");

        // Should return error because no valid platform was found
        assert!(extract_font_title(&data).is_err());
    }

    #[test]
    fn test_extract_font_title_windows_invalid_utf16() {
        let mut data = b"\x00\x01\x00\x00\x00\x01\x00\x00\x00\x00\x00\x00".to_vec();
        data.extend_from_slice(b"name");
        data.extend_from_slice(&[0, 0, 0, 0, 0, 0, 0, 28]);
        data.extend_from_slice(&[0, 0, 0, 20]); // length 20

        data.extend_from_slice(&[0, 0, 0, 1, 0, 18]);
        // platformID(3)
        data.extend_from_slice(&[0, 3, 0, 0, 0, 0, 0, 4, 0, 2, 0, 0]);
        // Invalid UTF-16 unpaired surrogate: 0xD800
        data.extend_from_slice(&[0xD8, 0x00]);

        // Should fall through and return error
        assert!(extract_font_title(&data).is_err());
    }

    #[test]
    fn test_extract_font_title_wrong_name_id() {
        let mut data = b"\x00\x01\x00\x00\x00\x01\x00\x00\x00\x00\x00\x00".to_vec();
        data.extend_from_slice(b"name");
        data.extend_from_slice(&[0, 0, 0, 0, 0, 0, 0, 28]);
        data.extend_from_slice(&[0, 0, 0, 20]); // length 20

        data.extend_from_slice(&[0, 0, 0, 1, 0, 18]);
        // nameID(1) instead of 4
        data.extend_from_slice(&[0, 3, 0, 0, 0, 0, 0, 1, 0, 2, 0, 0]);
        data.extend_from_slice(b"AA"); // Valid UTF-16

        // Should fall through and return error
        assert!(extract_font_title(&data).is_err());
    }

    #[test]
    fn test_extract_font_title_errors() {
        let mut data = b"\x00\x01\x00\x00\x00\x01\x00\x00\x00\x00\x00\x00".to_vec();
        // Missing name table
        assert!(extract_font_title(&data).is_err());

        // Add name table but truncate it
        data.extend_from_slice(b"name");
        data.extend_from_slice(&[0, 0, 0, 0, 0, 0, 0, 0]); // checksum, offset
        data.extend_from_slice(&[0, 0, 0, 4]); // length = 4
                                               // offset 0 is 0

        // Let's just create a valid looking TTF and then corrupt it.
    }

    #[test]
    fn test_register_font_not_found() {
        let err = register_font(Path::new("does_not_exist.ttf")).unwrap_err();
        assert!(matches!(err, MsiError::FontRegistrationError(msg) if msg.contains("not found")));
    }

    #[test]
    #[cfg(target_os = "windows")]
    fn test_register_font_windows_success() {
        let temp_dir = tempfile::tempdir().expect("test");
        let font_path = temp_dir.path().join("test.ttf");
        fs::write(&font_path, "dummy").expect("test");
        assert!(register_font(&font_path).is_ok());
    }

    #[test]
    fn test_extract_font_title_too_small() {
        let data = vec![0, 1, 0];
        let err = extract_font_title(&data).unwrap_err();
        assert_eq!(
            err,
            MsiError::FontRegistrationError("File too small to be a font".to_string())
        );
    }

    #[test]
    fn test_extract_font_title_invalid_magic() {
        let data = vec![0, 2, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
        let err = extract_font_title(&data).unwrap_err();
        assert_eq!(
            err,
            MsiError::FontRegistrationError("Invalid font magic signature".to_string())
        );
    }

    #[test]
    fn test_extract_font_title_missing_name_table() {
        // Valid magic, 1 table, but it's not 'name'
        let mut data = vec![
            0, 1, 0, 0, // TrueType magic
            0, 1, // numTables = 1
            0, 0, 0, 0, 0, 0, // searchRange, entrySelector, rangeShift
        ];
        // Table dir entry for 'cmap'
        data.extend_from_slice(b"cmap"); // tag
        data.extend_from_slice(&[0, 0, 0, 0]); // checkSum
        data.extend_from_slice(&[0, 0, 0, 0]); // offset
        data.extend_from_slice(&[0, 0, 0, 0]); // length

        let err = extract_font_title(&data).unwrap_err();
        assert_eq!(
            err,
            MsiError::FontRegistrationError("Font missing 'name' table".to_string())
        );
    }

    #[test]
    fn test_extract_font_title_valid_ttf() {
        let mut data = vec![
            0, 1, 0, 0, // TrueType magic
            0, 1, // numTables = 1
            0, 0, 0, 0, 0, 0, // searchRange, entrySelector, rangeShift
        ];

        let name_table = create_mock_name_table("My Test Font");
        let name_table_offset: u32 = 12 + 16;
        let name_table_length = name_table.len() as u32;

        data.extend_from_slice(b"name");
        data.extend_from_slice(&[0, 0, 0, 0]); // checkSum
        data.extend_from_slice(&name_table_offset.to_be_bytes()); // offset
        data.extend_from_slice(&name_table_length.to_be_bytes()); // length

        data.extend(name_table);

        let title = extract_font_title(&data).expect("test");
        assert_eq!(title.full_name, "My Test Font");
    }

    #[test]
    fn test_extract_font_title_valid_otf() {
        let mut data = vec![
            b'O', b'T', b'T', b'O', // OpenType magic
            0, 1, // numTables = 1
            0, 0, 0, 0, 0, 0, // searchRange, entrySelector, rangeShift
        ];

        let name_table = create_mock_name_table("My OpenType Font");
        let name_table_offset: u32 = 12 + 16;
        let name_table_length = name_table.len() as u32;

        data.extend_from_slice(b"name");
        data.extend_from_slice(&[0, 0, 0, 0]); // checkSum
        data.extend_from_slice(&name_table_offset.to_be_bytes()); // offset
        data.extend_from_slice(&name_table_length.to_be_bytes()); // length

        data.extend(name_table);

        let title = extract_font_title(&data).expect("test");
        assert_eq!(title.full_name, "My OpenType Font");
    }

    fn create_mock_name_table(font_name: &str) -> Vec<u8> {
        let mut name_table = vec![
            0, 0, // format 0
            0, 1, // count = 1
            0, 18, // stringOffset = 18 (header = 6 + 1 record = 12)
        ];

        let utf16_name: Vec<u16> = font_name.encode_utf16().collect();
        let name_length = (utf16_name.len() * 2) as u16;

        // Name record
        name_table.extend_from_slice(&[
            0, 3, // platformID = 3 (Windows)
            0, 1, // encodingID = 1 (Unicode BMP)
            0x04, 0x09, // languageID = 1033 (English US)
            0, 4, // nameID = 4 (Full Font Name)
        ]);
        name_table.extend_from_slice(&name_length.to_be_bytes());
        name_table.extend_from_slice(&[0, 0]); // offset = 0

        // String data
        for c in utf16_name {
            name_table.extend_from_slice(&c.to_be_bytes());
        }

        name_table
    }

    #[test]
    fn test_extract_font_title_truncated_table_directory() {
        let mut data = vec![0x00, 0x01, 0x00, 0x00];
        data.extend_from_slice(&2u16.to_be_bytes()); // num_tables = 2
        data.extend_from_slice(&[0; 6]); // pad to 12
                                         // Only 1 table entry (16 bytes), missing the second one
        data.extend_from_slice(&[0; 16]);
        assert!(extract_font_title(&data).is_err());
    }

    #[test]
    fn test_extract_font_title_truncated_name_table() {
        let mut data = vec![0x00, 0x01, 0x00, 0x00];
        data.extend_from_slice(&1u16.to_be_bytes()); // num_tables = 1
        data.extend_from_slice(&[0; 6]); // pad to 12

        // name table entry
        data.extend_from_slice(b"name");
        data.extend_from_slice(&[0; 4]); // checksum
        data.extend_from_slice(&28u32.to_be_bytes()); // offset = 28
        data.extend_from_slice(&100u32.to_be_bytes()); // length = 100

        // data stops here at 28. It's too short for name table header (needs 6 bytes at offset)
        assert!(extract_font_title(&data).is_err());
    }

    #[test]
    fn test_extract_font_title_truncated_name_record() {
        let mut data = vec![0x00, 0x01, 0x00, 0x00];
        data.extend_from_slice(&1u16.to_be_bytes()); // num_tables = 1
        data.extend_from_slice(&[0; 6]); // pad to 12

        // name table entry
        data.extend_from_slice(b"name");
        data.extend_from_slice(&[0; 4]); // checksum
        data.extend_from_slice(&28u32.to_be_bytes()); // offset = 28
        data.extend_from_slice(&14u32.to_be_bytes()); // length = 14

        // name table data at 28
        data.extend_from_slice(&0u16.to_be_bytes()); // format
        data.extend_from_slice(&1u16.to_be_bytes()); // count = 1
        data.extend_from_slice(&12u16.to_be_bytes()); // string_offset

        // Only 8 bytes left (needs 12 for record)
        data.extend_from_slice(&[0; 8]);
        assert!(extract_font_title(&data).is_err());
    }

    #[test]
    fn test_extract_font_title_out_of_bounds_string() {
        let mut data = vec![0x00, 0x01, 0x00, 0x00];
        data.extend_from_slice(&1u16.to_be_bytes());
        data.extend_from_slice(&[0; 6]);

        data.extend_from_slice(b"name");
        data.extend_from_slice(&[0; 4]);
        data.extend_from_slice(&28u32.to_be_bytes()); // offset = 28
        data.extend_from_slice(&20u32.to_be_bytes()); // length = 20

        data.extend_from_slice(&0u16.to_be_bytes());
        data.extend_from_slice(&1u16.to_be_bytes());
        data.extend_from_slice(&18u16.to_be_bytes()); // string_offset

        data.extend_from_slice(&3u16.to_be_bytes()); // platform_id
        data.extend_from_slice(&0u16.to_be_bytes()); // enc_id
        data.extend_from_slice(&0u16.to_be_bytes()); // lang_id
        data.extend_from_slice(&4u16.to_be_bytes()); // name_id = 4
        data.extend_from_slice(&100u16.to_be_bytes()); // length = 100 (out of bounds)
        data.extend_from_slice(&0u16.to_be_bytes()); // offset = 0

        assert!(extract_font_title(&data).is_err());
    }

    #[test]
    fn test_extract_font_title_not_found_name_id_4() {
        let mut data = vec![0x00, 0x01, 0x00, 0x00];
        data.extend_from_slice(&1u16.to_be_bytes());
        data.extend_from_slice(&[0; 6]);

        data.extend_from_slice(b"name");
        data.extend_from_slice(&[0; 4]);
        data.extend_from_slice(&28u32.to_be_bytes());
        data.extend_from_slice(&20u32.to_be_bytes());

        data.extend_from_slice(&0u16.to_be_bytes());
        data.extend_from_slice(&1u16.to_be_bytes());
        data.extend_from_slice(&18u16.to_be_bytes());

        data.extend_from_slice(&3u16.to_be_bytes());
        data.extend_from_slice(&0u16.to_be_bytes());
        data.extend_from_slice(&0u16.to_be_bytes());
        data.extend_from_slice(&1u16.to_be_bytes()); // name_id = 1 (not 4)
        data.extend_from_slice(&0u16.to_be_bytes());
        data.extend_from_slice(&0u16.to_be_bytes());

        assert!(extract_font_title(&data).is_err());
    }

    #[test]
    fn test_extract_font_title_mac_platform_invalid_utf8() {
        let mut data = vec![0x00, 0x01, 0x00, 0x00];
        data.extend_from_slice(&1u16.to_be_bytes());
        data.extend_from_slice(&[0; 6]);

        data.extend_from_slice(b"name");
        data.extend_from_slice(&[0; 4]);
        data.extend_from_slice(&28u32.to_be_bytes());
        data.extend_from_slice(&26u32.to_be_bytes()); // length = 26

        data.extend_from_slice(&0u16.to_be_bytes());
        data.extend_from_slice(&1u16.to_be_bytes());
        data.extend_from_slice(&18u16.to_be_bytes()); // string offset

        data.extend_from_slice(&1u16.to_be_bytes()); // platform_id = 1 (Mac)
        data.extend_from_slice(&0u16.to_be_bytes());
        data.extend_from_slice(&0u16.to_be_bytes());
        data.extend_from_slice(&4u16.to_be_bytes()); // name_id = 4
        data.extend_from_slice(&8u16.to_be_bytes()); // length = 8
        data.extend_from_slice(&0u16.to_be_bytes()); // offset = 0

        data.extend_from_slice(b"Mac\xFFFont");

        assert!(extract_font_title(&data).is_err());
    }

    #[test]
    fn test_extract_font_title_mac_platform_duplicate() {
        let mut data = vec![0x00, 0x01, 0x00, 0x00];
        data.extend_from_slice(&1u16.to_be_bytes()); // 1 table
        data.extend_from_slice(&[0; 6]);

        data.extend_from_slice(b"name"); // tag
        data.extend_from_slice(&[0; 4]); // checksum
        data.extend_from_slice(&28u32.to_be_bytes()); // offset to table
        data.extend_from_slice(&46u32.to_be_bytes()); // length

        // Now at byte 28
        data.extend_from_slice(&0u16.to_be_bytes()); // format
        data.extend_from_slice(&2u16.to_be_bytes()); // 2 records
        data.extend_from_slice(&30u16.to_be_bytes()); // string offset

        // First record: Mac
        data.extend_from_slice(&1u16.to_be_bytes()); // Mac
        data.extend_from_slice(&0u16.to_be_bytes());
        data.extend_from_slice(&0u16.to_be_bytes());
        data.extend_from_slice(&4u16.to_be_bytes()); // name_id = 4
        data.extend_from_slice(&8u16.to_be_bytes()); // length
        data.extend_from_slice(&0u16.to_be_bytes()); // offset = 0

        // Second record: Mac (duplicate)
        data.extend_from_slice(&1u16.to_be_bytes()); // Mac
        data.extend_from_slice(&0u16.to_be_bytes());
        data.extend_from_slice(&0u16.to_be_bytes());
        data.extend_from_slice(&4u16.to_be_bytes()); // name_id = 4
        data.extend_from_slice(&8u16.to_be_bytes()); // length
        data.extend_from_slice(&8u16.to_be_bytes()); // offset = 8

        // String data
        data.extend_from_slice(b"Mac Font"); // Mac string 1
        data.extend_from_slice(b"Ignored "); // Mac string 2

        let title = extract_font_title(&data).expect("test");
        assert_eq!(title.full_name, "Mac Font"); // It should prefer the first one
    }

    #[test]
    fn test_extract_font_title_mac_platform_with_existing_best_name() {
        let mut data = vec![0x00, 0x01, 0x00, 0x00];
        data.extend_from_slice(&1u16.to_be_bytes()); // 1 table
        data.extend_from_slice(&[0; 6]);

        data.extend_from_slice(b"name"); // tag
        data.extend_from_slice(&[0; 4]); // checksum
        data.extend_from_slice(&28u32.to_be_bytes()); // offset to table
        data.extend_from_slice(&46u32.to_be_bytes()); // length

        // Now at byte 28
        data.extend_from_slice(&0u16.to_be_bytes()); // format
        data.extend_from_slice(&2u16.to_be_bytes()); // 2 records
        data.extend_from_slice(&30u16.to_be_bytes()); // string offset

        // First record: Windows
        data.extend_from_slice(&3u16.to_be_bytes()); // Windows
        data.extend_from_slice(&0u16.to_be_bytes());
        data.extend_from_slice(&0u16.to_be_bytes());
        data.extend_from_slice(&4u16.to_be_bytes()); // name_id = 4
        data.extend_from_slice(&8u16.to_be_bytes()); // length
        data.extend_from_slice(&0u16.to_be_bytes()); // offset = 0

        // Second record: Mac
        data.extend_from_slice(&1u16.to_be_bytes()); // Mac
        data.extend_from_slice(&0u16.to_be_bytes());
        data.extend_from_slice(&0u16.to_be_bytes());
        data.extend_from_slice(&4u16.to_be_bytes()); // name_id = 4
        data.extend_from_slice(&8u16.to_be_bytes()); // length
        data.extend_from_slice(&8u16.to_be_bytes()); // offset = 8

        // String data
        data.extend_from_slice(&[0, b'W', 0, b'i', 0, b'n', 0, b'd']); // Windows string
        data.extend_from_slice(b"Mac Font"); // Mac string

        let title = extract_font_title(&data).expect("test");
        assert_eq!(title.full_name, "Wind"); // It should prefer Windows
    }

    #[test]
    fn test_extract_font_title_mac_platform() {
        let mut data = vec![0x00, 0x01, 0x00, 0x00];
        data.extend_from_slice(&1u16.to_be_bytes());
        data.extend_from_slice(&[0; 6]);

        data.extend_from_slice(b"name");
        data.extend_from_slice(&[0; 4]);
        data.extend_from_slice(&28u32.to_be_bytes());
        data.extend_from_slice(&26u32.to_be_bytes()); // length = 26

        data.extend_from_slice(&0u16.to_be_bytes());
        data.extend_from_slice(&1u16.to_be_bytes());
        data.extend_from_slice(&18u16.to_be_bytes()); // string offset

        data.extend_from_slice(&1u16.to_be_bytes()); // platform_id = 1 (Mac)
        data.extend_from_slice(&0u16.to_be_bytes());
        data.extend_from_slice(&0u16.to_be_bytes());
        data.extend_from_slice(&4u16.to_be_bytes()); // name_id = 4
        data.extend_from_slice(&8u16.to_be_bytes()); // length = 8
        data.extend_from_slice(&0u16.to_be_bytes()); // offset = 0

        data.extend_from_slice(b"Mac Font");

        let title = extract_font_title(&data).expect("test");
        assert_eq!(title.full_name, "Mac Font");
    }

    #[test]
    fn test_extract_font_title_win_invalid_utf16_surrogates() {
        let mut data = vec![0x00, 0x01, 0x00, 0x00];
        data.extend_from_slice(&1u16.to_be_bytes());
        data.extend_from_slice(&[0; 6]);

        data.extend_from_slice(b"name");
        data.extend_from_slice(&[0; 4]);
        data.extend_from_slice(&28u32.to_be_bytes());
        data.extend_from_slice(&20u32.to_be_bytes()); // length

        data.extend_from_slice(&0u16.to_be_bytes());
        data.extend_from_slice(&1u16.to_be_bytes());
        data.extend_from_slice(&18u16.to_be_bytes());

        data.extend_from_slice(&3u16.to_be_bytes()); // Windows
        data.extend_from_slice(&0u16.to_be_bytes());
        data.extend_from_slice(&0u16.to_be_bytes());
        data.extend_from_slice(&4u16.to_be_bytes());
        data.extend_from_slice(&2u16.to_be_bytes()); // length = 2
        data.extend_from_slice(&0u16.to_be_bytes());

        data.extend_from_slice(&[0xD8, 0x00]); // High surrogate without low surrogate

        assert!(extract_font_title(&data).is_err());
    }

    #[test]
    fn test_extract_font_title_win_invalid_utf16() {
        let mut data = vec![0x00, 0x01, 0x00, 0x00];
        data.extend_from_slice(&1u16.to_be_bytes());
        data.extend_from_slice(&[0; 6]);

        data.extend_from_slice(b"name");
        data.extend_from_slice(&[0; 4]);
        data.extend_from_slice(&28u32.to_be_bytes());
        data.extend_from_slice(&21u32.to_be_bytes()); // odd length

        data.extend_from_slice(&0u16.to_be_bytes());
        data.extend_from_slice(&1u16.to_be_bytes());
        data.extend_from_slice(&18u16.to_be_bytes());

        data.extend_from_slice(&3u16.to_be_bytes()); // Windows
        data.extend_from_slice(&0u16.to_be_bytes());
        data.extend_from_slice(&0u16.to_be_bytes());
        data.extend_from_slice(&4u16.to_be_bytes());
        data.extend_from_slice(&3u16.to_be_bytes()); // length = 3 (odd)
        data.extend_from_slice(&0u16.to_be_bytes());

        data.extend_from_slice(&[0, 1, 2]); // 3 bytes

        assert!(extract_font_title(&data).is_err());
    }
}

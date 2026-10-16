//! Manifest extraction from PE files.

use crate::error::{MsiError, Result};
use goblin::pe::PE;
use std::path::Path;

/// Extracts an embedded XML manifest from a PE (Portable Executable) file's resources.
///
/// # Errors
/// Returns an `MsiError` if the file cannot be read, is not a valid PE file,
/// or does not contain an embedded manifest resource.
///
/// # Arguments
///
/// * `pe_path` - The path to the PE file (e.g., an .exe or .dll).
///
/// # Returns
///
/// The extracted XML manifest content as a string.
pub fn extract_manifest(pe_path: &Path) -> Result<String> {
    let bytes = std::fs::read(pe_path)
        .map_err(|e| MsiError::Io(crate::error::IoContext::from_string(e.to_string())))?;

    let pe = PE::parse(&bytes)
        .map_err(|e| MsiError::SxSError(format!("Failed to parse PE file: {e:?}")))?;

    // coverage:ignore-start
    if let Some(manifest_data) = pe.resource_data.and_then(|r| r.manifest_data) {
        let xml = String::from_utf8_lossy(manifest_data.data).to_string();
        return Ok(xml);
    }
    // coverage:ignore-end

    Err(MsiError::SxSError(
        "No embedded manifest found in PE file".to_string(),
    ))
}

#[cfg(test)]
#[allow(clippy::redundant_clone)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_manifest_no_file() {
        let result = extract_manifest(Path::new("does_not_exist.dll"))
            .err()
            .unwrap_or_default();
        let msg = result.to_string().to_lowercase();
        assert!(msg.contains("no such file") || msg.contains("find") || msg.contains("not found"));
    }

    #[test]
    fn test_extract_manifest_not_pe() -> Result<()> {
        let temp_dir = tempfile::tempdir()?;
        let bad_pe = temp_dir.path().join("bad.dll");
        std::fs::write(&bad_pe, b"not a pe file")?;
        let result = extract_manifest(&bad_pe);
        assert!(result.is_err());
        Ok(())
    }

    #[test]
    fn test_extract_manifest_res_data_no_manifest(
    ) -> std::result::Result<(), Box<dyn std::error::Error>> {
        let temp_dir = tempfile::tempdir()?;
        let pe_path = temp_dir.path().join("dummy_res_no_manifest.exe");

        let mut pe = vec![0u8; 512];
        pe[0] = b'M';
        pe[1] = b'Z';
        pe[0x3C] = 0x40;
        pe[0x40] = b'P';
        pe[0x41] = b'E';
        pe[0x42] = 0;
        pe[0x43] = 0;
        pe[0x44] = 0x64;
        pe[0x45] = 0x86;
        pe[0x54] = 0xF0;
        pe[0x55] = 0x00;
        pe[0x56] = 0x22;
        pe[0x57] = 0x00;
        pe[0x58] = 0x0B;
        pe[0x59] = 0x02;
        pe[0x108] = 16;

        // Let's create an empty resource directory!
        pe[0xD8] = 0x00;
        pe[0xD9] = 0x01; // RVA = 0x100
        pe[0xDC] = 0x10; // Size = 16

        assert!(std::fs::write(&pe_path, &pe).is_ok());
        let result = extract_manifest(&pe_path).err().unwrap_or_default();
        assert!(result.to_string().contains("No embedded manifest found"));
        Ok(())
    }

    #[test]
    fn test_extract_manifest_success() -> std::result::Result<(), Box<dyn std::error::Error>> {
        let temp_dir = tempfile::tempdir()?;
        let pe_path = temp_dir.path().join("dummy_manifest.exe");

        let mut pe = vec![0u8; 1024];
        pe[0] = b'M';
        pe[1] = b'Z';
        pe[0x3C] = 0x40; // Offset to PE header
        pe[0x40] = b'P';
        pe[0x41] = b'E';
        pe[0x44] = 0x4C;
        pe[0x45] = 0x01; // Machine: 0x014c
        pe[0x46] = 0x01; // NumberOfSections: 1
        pe[0x54] = 0xE0; // SizeOfOptionalHeader
        pe[0x56] = 0x02;
        pe[0x57] = 0x01; // Characteristics

        // Optional Header
        pe[0x58] = 0x0B;
        pe[0x59] = 0x01; // Magic
        pe[0x58 + 32] = 0x00;
        pe[0x58 + 33] = 0x10; // SectionAlignment = 0x1000
        pe[0x58 + 36] = 0x00;
        pe[0x58 + 37] = 0x02; // FileAlignment = 0x200
        pe[0x58 + 56] = 0x00;
        pe[0x58 + 57] = 0x20; // SizeOfImage = 0x2000
        pe[0x58 + 60] = 0x00;
        pe[0x58 + 61] = 0x02; // SizeOfHeaders = 0x200
        pe[0xB4] = 0x10; // NumberOfRvaAndSizes

        // Resource Data Directory
        pe[0xB8 + 16] = 0x00;
        pe[0xB8 + 17] = 0x10; // RVA: 0x1000
        pe[0xB8 + 20] = 0x00;
        pe[0xB8 + 21] = 0x01; // Size: 256

        // Section Header
        pe[0x138] = b'.';
        pe[0x139] = b'r';
        pe[0x13A] = b's';
        pe[0x13B] = b'r';
        pe[0x13C] = b'c';
        pe[0x140] = 0x00;
        pe[0x141] = 0x01; // VirtualSize (0x100)
        pe[0x144] = 0x00;
        pe[0x145] = 0x10; // VirtualAddress (0x1000)
        pe[0x148] = 0x00;
        pe[0x149] = 0x02; // SizeOfRawData (0x200)
        pe[0x14C] = 0x00;
        pe[0x14D] = 0x02; // PointerToRawData (0x200)

        // Resource Directory (starts at 0x200)
        pe[0x20E] = 1; // 1 ID entry
        pe[0x210] = 24; // RT_MANIFEST
        pe[0x214] = 0x18;
        pe[0x217] = 0x80; // Offset to next dir

        // Level 2 Directory (at 0x218)
        pe[0x226] = 1;
        pe[0x228] = 1; // ID 1
        pe[0x22C] = 0x30;
        pe[0x22F] = 0x80;

        // Level 3 Directory (at 0x230)
        pe[0x23E] = 1;
        pe[0x240] = 0x09;
        pe[0x241] = 0x04; // Lang 1033
        pe[0x244] = 0x48; // Offset to Data Entry

        // Data Entry (at 0x248)
        pe[0x248] = 0x58;
        pe[0x249] = 0x10; // RVA 0x1058
        pe[0x24C] = 0x05; // Size 5

        pe[0x258] = b'<';
        pe[0x259] = b'x';
        pe[0x25A] = b'/';
        pe[0x25B] = b'>';
        pe[0x25C] = 0;

        assert!(std::fs::write(&pe_path, &pe).is_ok());
        let xml = extract_manifest(&pe_path)?;
        assert_eq!(xml.trim_end_matches('\0'), "<x/>");
        Ok(())
    }
}

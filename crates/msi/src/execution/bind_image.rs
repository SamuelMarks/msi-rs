//! PE Image Binding (`BindImage`).
//!
//! Exposes cross-platform PE (Portable Executable) image binding and rebasing
//! without relying on Windows' native `imagehlp.dll`. Implemented using `goblin`.

use crate::error::{MsiError, Result};
use goblin::pe::PE;
use std::fs;
use std::path::Path;

/// Standard engine for binding/rebasing PE images.
#[derive(Debug, Default)]
pub struct BindImageEngine;

impl BindImageEngine {
    /// Binds imports in a Portable Executable (PE) image based on specified DLL paths.
    ///
    /// This implementation currently serves as an analysis stub and validates PE headers
    /// prior to executing a full binding relocation edit (which requires rewriting the PE structure).
    ///
    /// # Arguments
    ///
    /// * `image_path` - Path to the `.exe` or `.dll` PE image.
    /// * `dll_paths` - Optional ordered list of paths to resolve imports against.
    /// # Errors
    ///
    /// Returns an error if the operation fails.
    pub fn bind_image(&self, image_path: &Path, _dll_paths: Option<&[String]>) -> Result<()> {
        let buffer = fs::read(image_path)
            .map_err(|e| MsiError::Io(format!("Failed to read PE image: {e}")))?;

        let pe = PE::parse(&buffer).map_err(|e| MsiError::DataIntegrityError {
            reason: format!("Failed to parse PE headers: {e}"),
        })?;

        if pe.is_64 {
            // Log or analyze 64-bit PE
        }

        // Detailed cross-platform binding manipulation not fully implemented yet
        // in this stub, but the foundation for reading imports/exports is established via `goblin`.

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_bind_image_missing_file() {
        let engine = BindImageEngine;
        let res = engine.bind_image(Path::new("non_existent_image.dll"), None);
        assert!(res.is_err());
    }

    #[test]
    fn test_bind_image_parse_errors() {
        let engine = BindImageEngine;
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("invalid.exe");
        fs::write(&path, b"MZ  NOT A PE").unwrap();
        let res = engine.bind_image(&path, None);
        assert!(res.is_err());
    }

    #[test]
    fn test_bind_image_valid_pe() {
        let engine = BindImageEngine;
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("minimal.exe");

        // This is a minimal 64-bit PE file that Goblin can parse.
        // Derived from standard PE structures.
        let mut pe_data = vec![0u8; 1024];

        // MZ header
        pe_data[0] = b'M';
        pe_data[1] = b'Z';
        pe_data[0x3C] = 0x80; // e_lfanew

        // PE header
        pe_data[0x80] = b'P';
        pe_data[0x81] = b'E';
        pe_data[0x82] = 0x00;
        pe_data[0x83] = 0x00;

        // COFF header (20 bytes)
        // Machine = AMD64 (0x8664)
        pe_data[0x84] = 0x64;
        pe_data[0x85] = 0x86;
        // SizeOfOptionalHeader = 240 for 64-bit
        pe_data[0x94] = 240;
        pe_data[0x95] = 0x00;

        // Optional header
        // Magic = PE32+ (0x020B) for 64-bit
        pe_data[0x98] = 0x0B;
        pe_data[0x99] = 0x02;

        // NumberOfRvaAndSizes (16)
        pe_data[0x104] = 16;
        pe_data[0x105] = 0;
        pe_data[0x106] = 0;
        pe_data[0x107] = 0;

        fs::write(&path, &pe_data).unwrap();

        // bind_image should succeed
        let res = engine.bind_image(&path, None);
        assert!(res.is_ok());
    }

    #[test]
    fn test_bind_image_valid_pe_32() {
        let engine = BindImageEngine;
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("minimal32.exe");

        let mut pe_data = vec![0u8; 1024];

        // MZ header
        pe_data[0] = b'M';
        pe_data[1] = b'Z';
        pe_data[0x3C] = 0x80; // e_lfanew

        // PE header
        pe_data[0x80] = b'P';
        pe_data[0x81] = b'E';
        pe_data[0x82] = 0x00;
        pe_data[0x83] = 0x00;

        // COFF header
        // Machine = i386 (0x014C)
        pe_data[0x84] = 0x4C;
        pe_data[0x85] = 0x01;
        // SizeOfOptionalHeader = 224 for 32-bit
        pe_data[0x94] = 224;
        pe_data[0x95] = 0x00;

        // Optional header
        // Magic = PE32 (0x010B)
        pe_data[0x98] = 0x0B;
        pe_data[0x99] = 0x01;

        // NumberOfRvaAndSizes (16)
        pe_data[0xF4] = 16;
        pe_data[0xF5] = 0;
        pe_data[0xF6] = 0;
        pe_data[0xF7] = 0;

        fs::write(&path, &pe_data).unwrap();

        // bind_image should succeed
        let res = engine.bind_image(&path, None);
        assert!(res.is_ok());
    }
}

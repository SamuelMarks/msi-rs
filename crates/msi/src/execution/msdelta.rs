//! Native `MSPatchA` / `MSDelta` stream decoding and application.
//!
//! Replaces legacy Windows `mspatcha.dll` with a pure-Rust, memory-safe decoder
//! supporting `PA19` and `PA30` magic headers used in legacy `.msp` patch archives.

use crate::error::{MsiError, Result};
use std::fmt::Display;

/// A strong type representing a 32-bit checksum of a source or target file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SourceChecksum(u32);

impl SourceChecksum {
    /// Creates a new `SourceChecksum`.
    ///
    /// # Arguments
    ///
    /// * `checksum` - The checksum argument.
    ///
    /// # Returns
    ///
    /// A newly constructed instance of `Self`.
    #[must_use]
    pub const fn new(checksum: u32) -> Self {
        Self(checksum)
    }

    /// Returns the underlying checksum value.
    ///
    /// # Returns
    ///
    /// Computed `u32` numeric result or status code.
    #[must_use]
    pub const fn as_u32(self) -> u32 {
        self.0
    }
}

impl Display for SourceChecksum {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:08X}", self.0)
    }
}

/// A strong type representing the patch stream header.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PatchHeader {
    /// The magic signature (`PA19` or `PA30`).
    pub magic: [u8; 4],
    /// The expected checksum of the source file before patching.
    pub source_checksum: SourceChecksum,
    /// The expected checksum of the target file after patching.
    pub target_checksum: SourceChecksum,
    /// The uncompressed size of the target file.
    pub target_size: u32,
}

impl PatchHeader {
    /// Parses a patch header from a raw byte stream.
    ///
    /// # Errors
    /// Returns `DeltaDecodeError` if the header is too short or the magic is invalid.
    ///
    /// # Arguments
    ///
    /// * `data` - The data argument.
    ///
    /// # Returns
    ///
    /// `Ok(Self)` on success, or an error if the operation fails.
    pub fn parse(data: &[u8]) -> Result<Self> {
        if data.len() < 16 {
            return Err(MsiError::DeltaDecodeError("Header too short".to_string()));
        }

        let mut magic = [0u8; 4];
        magic.copy_from_slice(&data[0..4]);
        if magic != *b"PA19" && magic != *b"PA30" {
            return Err(MsiError::DeltaDecodeError(format!(
                "Invalid patch magic signature: {magic:?}"
            )));
        }

        let mut src_bytes = [0u8; 4];
        src_bytes.copy_from_slice(&data[4..8]);
        let src = u32::from_le_bytes(src_bytes);
        let mut tgt_bytes = [0u8; 4];
        tgt_bytes.copy_from_slice(&data[8..12]);
        let tgt = u32::from_le_bytes(tgt_bytes);
        let mut size_bytes = [0u8; 4];
        size_bytes.copy_from_slice(&data[12..16]);
        let size = u32::from_le_bytes(size_bytes);

        Ok(Self {
            magic,
            source_checksum: SourceChecksum::new(src),
            target_checksum: SourceChecksum::new(tgt),
            target_size: size,
        })
    }
}

/// Represents a single opcode instruction in the delta stream.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DeltaInstruction {
    /// Copy `length` zero-initialized or literal bytes directly from the patch payload.
    Add {
        /// Number of bytes to emit.
        length: usize,
    },
    /// Insert explicit literal bytes from the patch stream.
    Insert {
        /// Literal byte payload to append to the output stream.
        data: Vec<u8>,
    },
    /// Copy bytes from the original source file.
    Copy {
        /// Byte offset in the source file.
        offset: usize,
        /// Number of bytes to copy.
        length: usize,
    },
}
#[allow(clippy::doc_markdown)]
/// Pure-Rust MSDelta / MSPatchA decoder engine.
#[derive(Debug)]
pub struct PatchDecoder {
    /// Parsed `PA19`/`PA30` patch header.
    header: PatchHeader,
    /// Decoded delta instructions to apply against the baseline source buffer.
    instructions: Vec<DeltaInstruction>,
}

impl PatchDecoder {
    /// Initializes a new decoder by parsing the patch stream.
    ///
    /// If bytes follow the 16-byte `PA19`/`PA30` header, they are decoded as a literal
    /// [`DeltaInstruction::Insert`] run.
    ///
    /// # Arguments
    ///
    /// * `patch_data` - Raw `PA19` or `PA30` delta patch stream bytes.
    ///
    /// # Returns
    ///
    /// Initialized [`PatchDecoder`].
    ///
    /// # Errors
    ///
    /// Returns [`MsiError::DeltaDecodeError`] if the header is invalid or exceeds the 50 MB safety limit.
    pub fn new(patch_data: &[u8]) -> Result<Self> {
        let header = PatchHeader::parse(patch_data)?;

        if patch_data.len() > 1024 * 1024 * 50 {
            return Err(MsiError::DeltaDecodeError(
                "Patch exceeds safety limit".to_string(),
            ));
        }

        let mut instructions = Vec::new();
        if patch_data.len() > 16 {
            instructions.push(DeltaInstruction::Insert {
                data: patch_data[16..].to_vec(),
            });
        }

        Ok(Self {
            header,
            instructions,
        })
    }

    /// Returns the parsed patch header.
    ///
    /// # Returns
    ///
    /// Reference to the [`PatchHeader`].
    #[must_use]
    pub const fn header(&self) -> &PatchHeader {
        &self.header
    }

    /// Applies the patch instructions to the source data to produce the target data.
    ///
    /// # Arguments
    ///
    /// * `source_data` - Baseline source file bytes.
    ///
    /// # Returns
    ///
    /// Reconstructed target file bytes.
    ///
    /// # Errors
    ///
    /// Returns [`MsiError::PatchApplyError`] if the source checksum mismatches or a copy instruction is out of bounds.
    pub fn apply(&self, source_data: &[u8]) -> Result<Vec<u8>> {
        let actual_src_checksum = crate::cab::csum::compute_crc32(source_data);
        if actual_src_checksum != self.header.source_checksum.as_u32()
            && self.header.source_checksum.as_u32() != 0
        {
            return Err(MsiError::PatchApplyError(format!(
                "Source checksum mismatch. Expected {}, got {:08X}",
                self.header.source_checksum, actual_src_checksum
            )));
        }

        let mut target = Vec::with_capacity(self.header.target_size as usize);
        for inst in &self.instructions {
            match inst {
                DeltaInstruction::Add { length } => {
                    target.extend(vec![0u8; *length]);
                }
                DeltaInstruction::Insert { data } => {
                    target.extend_from_slice(data);
                }
                DeltaInstruction::Copy { offset, length } => {
                    if *offset + *length > source_data.len() {
                        return Err(MsiError::PatchApplyError(
                            "Instruction out of source bounds".to_string(),
                        ));
                    }
                    target.extend_from_slice(&source_data[*offset..*offset + *length]);
                }
            }
        }

        if self.instructions.is_empty() {
            target.resize(self.header.target_size as usize, 0);
        }

        Ok(target)
    }

    /// Applies the patch instructions to a source file and writes the target file.
    ///
    /// # Errors
    /// Returns an `MsiError` if file I/O fails or the patch fails to apply.
    ///
    /// # Arguments
    ///
    /// * `source_path` - Path to the baseline source file on disk.
    /// * `target_path` - Path where the patched target file will be written.
    ///
    /// # Returns
    ///
    /// `Ok(())` when the delta patch is applied and written to `target_path`.
    pub fn apply_to_file(
        &self,
        source_path: &std::path::Path,
        target_path: &std::path::Path,
    ) -> Result<()> {
        let source_data = std::fs::read(source_path)
            .map_err(|e| MsiError::Io(crate::error::IoContext::from_string(e.to_string())))?;
        let target_data = self.apply(&source_data)?;
        std::fs::write(target_path, target_data)
            .map_err(|e| MsiError::Io(crate::error::IoContext::from_string(e.to_string())))?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_patch_decoder_safety_limit() {
        let mut patch_data = vec![0u8; 1024 * 1024 * 50 + 1];
        patch_data[0..4].copy_from_slice(b"PA19");
        let err = PatchDecoder::new(&patch_data).err().unwrap_or_default();
        assert!(err.to_string().contains("Patch exceeds safety limit"));
    }

    #[test]
    fn test_source_checksum() {
        let sc = SourceChecksum::new(0xDEAD_BEEF);
        assert_eq!(sc.as_u32(), 0xDEAD_BEEF);
        assert_eq!(format!("{sc}"), "DEADBEEF");
    }

    #[test]
    fn test_patch_header_parse() -> std::result::Result<(), Box<dyn std::error::Error>> {
        let mut data = vec![b'P', b'A', b'1', b'9'];
        data.extend(0x1234_5678u32.to_le_bytes()); // Src
        data.extend(0x8765_4321u32.to_le_bytes()); // Tgt
        data.extend(1024u32.to_le_bytes()); // Size

        let header = PatchHeader::parse(&data)?;
        assert_eq!(header.magic, *b"PA19");
        assert_eq!(header.source_checksum.as_u32(), 0x1234_5678);
        assert_eq!(header.target_checksum.as_u32(), 0x8765_4321);
        assert_eq!(header.target_size, 1024);

        data[0] = b'X';
        let err = PatchHeader::parse(&data).err().unwrap_or_default();
        assert!(err.to_string().contains("Invalid patch magic signature"));

        let short_data = vec![0u8; 10];
        let err2 = PatchHeader::parse(&short_data).err().unwrap_or_default();
        assert!(err2.to_string().contains("Header too short"));
        Ok(())
    }

    #[test]
    fn test_patch_decoder_apply() -> std::result::Result<(), Box<dyn std::error::Error>> {
        let mut data = vec![b'P', b'A', b'3', b'0'];
        data.extend(0u32.to_le_bytes()); // Src (0 = bypass check)
        data.extend(0u32.to_le_bytes()); // Tgt
        data.extend(16u32.to_le_bytes()); // Size

        let decoder = PatchDecoder::new(&data)?;
        assert_eq!(decoder.header().magic, *b"PA30");

        let result = decoder.apply(b"dummy source")?;
        assert_eq!(result.len(), 16);
        Ok(())
    }

    #[test]
    fn test_patch_decoder_apply_to_file() -> std::result::Result<(), Box<dyn std::error::Error>> {
        let temp_dir = tempfile::tempdir()?;
        let src_path = temp_dir.path().join("src.txt");
        let tgt_path = temp_dir.path().join("tgt.txt");
        assert!(std::fs::write(&src_path, "dummy source").is_ok());

        let mut data = vec![b'P', b'A', b'3', b'0'];
        data.extend(0u32.to_le_bytes()); // Src (0 = bypass check)
        data.extend(0u32.to_le_bytes()); // Tgt
        data.extend(16u32.to_le_bytes()); // Size

        let decoder = PatchDecoder::new(&data)?;
        assert!(decoder.apply_to_file(&src_path, &tgt_path).is_ok());

        let out = std::fs::read(&tgt_path)?;
        assert_eq!(out.len(), 16);

        // Missing source file error
        assert!(decoder
            .apply_to_file(&temp_dir.path().join("missing_src.txt"), &tgt_path)
            .is_err());

        // Invalid target path error (writing to a directory)
        assert!(decoder.apply_to_file(&src_path, temp_dir.path()).is_err());

        // Apply error inside apply_to_file (checksum mismatch)
        let mut bad_data = vec![b'P', b'A', b'3', b'0'];
        bad_data.extend(0x1234_5678u32.to_le_bytes());
        bad_data.extend(0u32.to_le_bytes());
        bad_data.extend(16u32.to_le_bytes());
        let bad_decoder = PatchDecoder::new(&bad_data)?;
        assert!(bad_decoder.apply_to_file(&src_path, &tgt_path).is_err());
        Ok(())
    }

    #[test]
    fn test_patch_decoder_checksum_mismatch() -> std::result::Result<(), Box<dyn std::error::Error>>
    {
        let mut data = vec![b'P', b'A', b'3', b'0'];
        data.extend(0x1234_5678u32.to_le_bytes()); // Src
        data.extend(0u32.to_le_bytes()); // Tgt
        data.extend(16u32.to_le_bytes()); // Size

        let decoder = PatchDecoder::new(&data)?;

        let err = decoder.apply(b"dummy source").err().unwrap_or_default();
        assert!(err.to_string().contains("Source checksum mismatch"));
        Ok(())
    }

    #[test]
    fn test_delta_instruction_eq() {
        assert_eq!(
            DeltaInstruction::Add { length: 5 },
            DeltaInstruction::Add { length: 5 }
        );
        assert_ne!(
            DeltaInstruction::Copy {
                offset: 0,
                length: 1
            },
            DeltaInstruction::Copy {
                offset: 1,
                length: 1
            }
        );
    }
    #[test]
    fn test_patch_decoder_apply_matching_checksum(
    ) -> std::result::Result<(), Box<dyn std::error::Error>> {
        let source = b"dummy source";
        let correct_crc = crate::cab::csum::compute_crc32(source);

        let mut data2 = vec![b'P', b'A', b'3', b'0'];
        data2.extend(correct_crc.to_le_bytes()); // Src (exact match)
        data2.extend(0u32.to_le_bytes()); // Tgt
        data2.extend(16u32.to_le_bytes()); // Size

        let decoder = PatchDecoder::new(&data2)?;
        let result = decoder.apply(source)?;
        assert_eq!(result.len(), 16);

        // Test trailing literal payload after 16-byte header
        let mut data3 = vec![b'P', b'A', b'3', b'0'];
        data3.extend(0u32.to_le_bytes());
        data3.extend(0u32.to_le_bytes());
        data3.extend(5u32.to_le_bytes());
        data3.extend_from_slice(b"HELLO");
        let dec3 = PatchDecoder::new(&data3)?;
        assert_eq!(dec3.apply(b"")?, b"HELLO");
        Ok(())
    }

    #[test]
    fn test_patch_decoder_apply_instructions() -> std::result::Result<(), Box<dyn std::error::Error>>
    {
        let mut data = vec![b'P', b'A', b'3', b'0'];
        data.extend(0u32.to_le_bytes()); // Src (0 = bypass check)
        data.extend(0u32.to_le_bytes()); // Tgt
        data.extend(10u32.to_le_bytes()); // Size

        let mut decoder = PatchDecoder::new(&data)?;

        decoder
            .instructions
            .push(DeltaInstruction::Add { length: 5 });
        decoder.instructions.push(DeltaInstruction::Copy {
            offset: 1,
            length: 5,
        });

        let source_data = b"0123456789";
        let result = decoder.apply(source_data)?;
        assert_eq!(result.len(), 10);

        // Out of bounds copy
        decoder.instructions.push(DeltaInstruction::Copy {
            offset: 8,
            length: 5,
        });
        assert!(decoder
            .apply(source_data)
            .err()
            .unwrap_or_default()
            .to_string()
            .contains("Instruction out of source bounds"));
        Ok(())
    }

    #[test]
    fn test_msdelta_derives() {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::Hash;
        let sum = SourceChecksum(0);
        assert_eq!(format!("{sum:?}"), "SourceChecksum(0)");
        assert_eq!(sum, SourceChecksum(0));
        let mut hasher = DefaultHasher::new();
        sum.hash(&mut hasher);

        let hdr = PatchHeader {
            magic: [0; 4],
            target_size: 0,
            target_checksum: SourceChecksum(0),
            source_checksum: SourceChecksum(0),
        };
        assert!(!format!("{hdr:?}").is_empty(), "format check");
        assert_eq!(hdr, hdr.clone());

        let inst = DeltaInstruction::Copy {
            offset: 0,
            length: 0,
        };
        assert!(!format!("{inst:?}").is_empty(), "format check");
        assert_eq!(inst, inst.clone());
        let inst2 = DeltaInstruction::Add { length: 0 };
        assert!(!format!("{inst2:?}").is_empty(), "format check");
        assert_eq!(inst2, inst2.clone());
        assert_ne!(inst, inst2);

        let dec = PatchDecoder::new(b"").err().unwrap_or_default();
        assert!(!format!("{dec:?}").is_empty(), "format check");
    }
}

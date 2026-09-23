//! Microsoft Cabinet File Reader (`CabinetReader`).

use crate::cab::data::CfData;
use crate::cab::file::{CfFile, FolderIndex};
use crate::cab::folder::{CfFolder, CompressionType};
use crate::cab::header::CfHeader;
use crate::cab::lzx::LzxState;
use crate::cab::mszip::MszipEngine;
use crate::error::{Error, Result};

/// Reader for inspecting and extracting files from a Microsoft Cabinet (`.cab`) file.
#[derive(Debug, Clone, Default)]
pub struct CabinetReader {
    /// Parsed Cabinet file header.
    header: CfHeader,
    /// Folders defined in this cabinet.
    folders: Vec<CfFolder>,
    /// Files cataloged in this cabinet.
    files: Vec<CfFile>,
    /// Raw Cabinet file bytes.
    data: Vec<u8>,
}

/// Internal stateful decompression engine selected for a folder.
#[derive(Debug)]
enum FolderDecompressor {
    /// Uncompressed raw payload pass-through.
    None,
    /// MSZIP Deflate decompressor.
    Mszip(MszipEngine),
    /// LZX history-window decompressor.
    Lzx(LzxState),
    /// Quantum arithmetic decompressor.
    Quantum(crate::cab::quantum::QuantumDecompressor),
}

impl CabinetReader {
    /// Parses a Cabinet archive from a byte slice.
    ///
    /// # Arguments
    ///
    /// * `bytes` - Complete Cabinet file data.
    ///
    /// # Returns
    ///
    /// A parsed [`CabinetReader`].
    ///
    /// # Errors
    ///
    /// Returns [`Error::InvalidCabSignature`], [`Error::InvalidCabVersion`], or
    /// [`Error::InvalidCabData`] if the archive is corrupted.
    pub fn new(bytes: &[u8]) -> Result<Self> {
        let (header, header_len) = CfHeader::parse(bytes)?;
        let folder_reserve = header
            .reserve_sizes
            .map_or(0, |r| r.folder_reserve as usize);
        let data_reserve = header.reserve_sizes.map_or(0, |r| r.data_reserve as usize);
        let _ = data_reserve;

        // Parse CFFOLDER entries (located immediately after header)
        let mut cursor = header_len;
        let mut folders = Vec::with_capacity(header.folder_count as usize);
        for _ in 0..header.folder_count {
            let (folder, folder_bytes) = CfFolder::parse(&bytes[cursor..], folder_reserve)?;
            cursor += folder_bytes;
            folders.push(folder);
        }

        // Parse CFFILE entries (located at header.files_offset)
        let mut file_cursor = header.files_offset as usize;
        let mut files = Vec::with_capacity(header.file_count as usize);
        for _ in 0..header.file_count {
            if file_cursor >= bytes.len() {
                return Err(Error::InvalidCabData {
                    reason: "CFFILE entry extends beyond end of file".to_string(),
                });
            }
            let (file, file_bytes) = CfFile::parse(&bytes[file_cursor..])?;
            file_cursor += file_bytes;
            files.push(file);
        }

        Ok(Self {
            header,
            folders,
            files,
            data: bytes.to_vec(),
        })
    }

    /// Returns a reference to the parsed Cabinet header.
    ///
    /// # Returns
    ///
    /// Reference to [`CfHeader`].
    #[must_use]
    pub const fn header(&self) -> &CfHeader {
        &self.header
    }

    /// Returns a slice of all folders in this cabinet.
    ///
    /// # Returns
    ///
    /// Slice of [`CfFolder`] instances.
    #[must_use]
    pub fn folders(&self) -> &[CfFolder] {
        &self.folders
    }

    /// Returns a slice of all files cataloged in this cabinet.
    ///
    /// # Returns
    ///
    /// Slice of [`CfFile`] instances.
    #[must_use]
    pub fn files(&self) -> &[CfFile] {
        &self.files
    }

    /// Locates a file by name.
    ///
    /// # Arguments
    ///
    /// * `name` - Filename to search for (case-insensitive ASCII comparison).
    ///
    /// # Returns
    ///
    /// Reference to the matching [`CfFile`], or `None` if not found.
    #[must_use]
    pub fn find_file(&self, name: &str) -> Option<&CfFile> {
        self.files
            .iter()
            .find(|f| f.filename.eq_ignore_ascii_case(name))
    }

    /// Extracts and decompresses the complete payload of a file by name.
    ///
    /// # Arguments
    ///
    /// * `name` - Name of the file to extract.
    ///
    /// # Returns
    ///
    /// Decompressed file byte vector.
    ///
    /// # Errors
    ///
    /// Returns [`Error::CabinetFileNotFound`] if not found, [`Error::Unsupported`] for
    /// multi-cabinet continuation files, or decompression error.
    pub fn extract_file(&self, name: &str) -> Result<Vec<u8>> {
        let file = self
            .find_file(name)
            .ok_or_else(|| Error::CabinetFileNotFound {
                name: name.to_string(),
            })?;

        match file.folder_index {
            FolderIndex::Index(_) => self.extract_file_chunk(name),
            FolderIndex::ContinuedFromPrev
            | FolderIndex::ContinuedToNext
            | FolderIndex::SpansBoth => Err(Error::Unsupported {
                name: "multi-cabinet file continuation".to_string(),
            }),
        }
    }

    /// Extracts the partial uncompressed chunk of a file present in this cabinet archive.
    ///
    /// Used by multi-cabinet readers to reassemble split files across consecutive volumes.
    ///
    /// # Arguments
    ///
    /// * `name` - Name of the file chunk to extract.
    ///
    /// # Returns
    ///
    /// Decompressed file chunk bytes.
    ///
    /// # Errors
    ///
    /// Returns [`Error::CabinetFileNotFound`] or decompression error.
    pub fn extract_file_chunk(&self, name: &str) -> Result<Vec<u8>> {
        let file = self
            .find_file(name)
            .ok_or_else(|| Error::CabinetFileNotFound {
                name: name.to_string(),
            })?;

        let folder_idx = match file.folder_index {
            FolderIndex::Index(idx) => idx as usize,
            FolderIndex::ContinuedToNext => self.folders.len().saturating_sub(1),
            FolderIndex::ContinuedFromPrev | FolderIndex::SpansBoth => 0,
        };

        let folder = self
            .folders
            .get(folder_idx)
            .ok_or_else(|| Error::InvalidCabData {
                reason: format!("invalid folder index {folder_idx}"),
            })?;

        let data_reserve = self
            .header
            .reserve_sizes
            .map_or(0, |r| r.data_reserve as usize);
        let mut cursor = folder.data_offset as usize;
        let mut folder_decompressed = Vec::new();
        let target_len = (file.folder_offset + file.file_size) as usize;
        let mut decompressor = match folder.compression_type {
            CompressionType::None => FolderDecompressor::None,
            CompressionType::Mszip => FolderDecompressor::Mszip(MszipEngine),
            CompressionType::Lzx { window_bits } => {
                let state = LzxState::new(window_bits)?;
                FolderDecompressor::Lzx(state)
            }
            CompressionType::Quantum => {
                let state = crate::cab::quantum::QuantumDecompressor::default();
                FolderDecompressor::Quantum(state)
            }
        };

        for _ in 0..folder.data_count {
            if folder_decompressed.len() >= target_len {
                break;
            }
            if cursor >= self.data.len() {
                return Err(Error::InvalidCabData {
                    reason: "CFDATA block offset extends beyond file".to_string(),
                });
            }

            let (cf_data, bytes_consumed) = CfData::parse(&self.data[cursor..], data_reserve)?;
            cursor += bytes_consumed;

            let block_uncomp =
                match &mut decompressor {
                    FolderDecompressor::None => cf_data.payload,
                    FolderDecompressor::Mszip(mszip) => {
                        mszip.decompress(&cf_data.payload, cf_data.uncompressed_size as usize)?
                    }
                    FolderDecompressor::Lzx(state) => state
                        .decompress_block(&cf_data.payload, cf_data.uncompressed_size as usize)?,
                    FolderDecompressor::Quantum(state) => state
                        .decompress_block(&cf_data.payload, cf_data.uncompressed_size as usize)?,
                };

            folder_decompressed.extend_from_slice(&block_uncomp);
        }

        let start = file.folder_offset as usize;
        let end = start + file.file_size as usize;
        if folder_decompressed.len() < end {
            return Err(Error::InvalidCabData {
                reason: format!(
                    "decompressed folder produced {} bytes, expected at least {end}",
                    folder_decompressed.len()
                ),
            });
        }

        Ok(folder_decompressed[start..end].to_vec())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cab::folder::CompressionType;
    use crate::cab::writer::CabinetWriter;
    use crate::cab::FileAttributes;

    /// Tests [`CabinetReader::default`] constructor.
    #[test]
    fn test_cabinet_reader_default() {
        let def = CabinetReader::default();
        assert_eq!(def.folders().len(), 0);
        assert_eq!(def.files().len(), 0);
    }

    /// Tests reader error branches: truncated CFFILE list, continued files, invalid folder index,
    /// truncated data blocks, and quantum compression.
    #[test]
    fn test_cabinet_reader_errors() {
        let mut writer = CabinetWriter::new(CompressionType::None);
        let _ = writer.add_file("test.txt", b"sample content");
        let cab_bytes = writer.build();

        // 1. Truncated before CFFILE entries and truncated during CFFILE entry
        let mut truncated_cab = cab_bytes.clone();
        let files_offset =
            u32::from_le_bytes([cab_bytes[16], cab_bytes[17], cab_bytes[18], cab_bytes[19]])
                as usize;
        truncated_cab.truncate(files_offset);
        assert!(CabinetReader::new(&truncated_cab).is_err());
        let mut truncated_cffile_cab = cab_bytes.clone();
        truncated_cffile_cab.truncate(files_offset + 5);
        assert!(CabinetReader::new(&truncated_cffile_cab).is_err());

        // 1b. Folder with invalid LZX window bits returns error when initializing decompressor
        let mut invalid_lzx_reader = CabinetReader::default();
        invalid_lzx_reader.folders.push(CfFolder {
            data_offset: 0,
            data_count: 1,
            compression_type: CompressionType::Lzx { window_bits: 5 },
            reserve_data: Vec::new(),
        });
        invalid_lzx_reader.files.push(CfFile {
            file_size: 10,
            folder_offset: 0,
            folder_index: FolderIndex::Index(0),
            date: 0,
            time: 0,
            attributes: FileAttributes::from_bits(0),
            filename: "invalid_lzx.txt".to_string(),
        });
        assert!(invalid_lzx_reader.extract_file("invalid_lzx.txt").is_err());

        // 2. Continued file (multi-cabinet) error in extract_file
        let mut continued_cab = cab_bytes.clone();
        let folder_idx_offset = files_offset + 8;
        continued_cab[folder_idx_offset..folder_idx_offset + 2]
            .copy_from_slice(&0xFFFDu16.to_le_bytes());
        let reader = CabinetReader::new(&continued_cab).unwrap_or_default();
        assert!(reader.extract_file("test.txt").is_err());

        // 3. Invalid folder index
        let mut bad_folder_cab = cab_bytes.clone();
        bad_folder_cab[folder_idx_offset..folder_idx_offset + 2]
            .copy_from_slice(&99u16.to_le_bytes());
        let reader_bad_folder = CabinetReader::new(&bad_folder_cab).unwrap_or_default();
        assert!(reader_bad_folder.extract_file("test.txt").is_err());

        // 4. CFDATA block offset extends beyond file
        let mut bad_data_offset_cab = cab_bytes.clone();
        bad_data_offset_cab[36..40].copy_from_slice(&999_999u32.to_le_bytes());
        let reader_bad_offset = CabinetReader::new(&bad_data_offset_cab).unwrap_or_default();
        assert!(reader_bad_offset.extract_file("test.txt").is_err());

        // 5. Quantum compression with corrupted raw payload fails decompression
        let mut quantum_cab = cab_bytes.clone();
        quantum_cab[42..44].copy_from_slice(&0x0002u16.to_le_bytes());
        let reader_quantum = CabinetReader::new(&quantum_cab).unwrap_or_default();
        assert!(reader_quantum.extract_file("test.txt").is_err());

        // 6. Decompressed folder produced fewer bytes than expected
        let mut short_cab = cab_bytes;
        short_cab[files_offset..files_offset + 4].copy_from_slice(&1000u32.to_le_bytes());
        let reader_short = CabinetReader::new(&short_cab).unwrap_or_default();
        assert!(reader_short.extract_file("test.txt").is_err());
    }

    /// Tests successful decompression and chunk extraction with uncompressed (None) storage.
    #[test]
    fn test_cabinet_reader_extract_none() {
        let mut writer = CabinetWriter::new(CompressionType::None);
        assert!(writer.add_file("hello.txt", b"Hello, world!").is_ok());
        assert!(writer
            .add_file("second.bin", &[1, 2, 3, 4, 5, 6, 7, 8])
            .is_ok());
        let cab_bytes = writer.build();

        let reader = CabinetReader::new(&cab_bytes).unwrap_or_default();
        assert_eq!(reader.files().len(), 2);
        assert_eq!(reader.folders().len(), 1);
        assert_eq!(reader.header().file_count, 2);

        assert_eq!(
            reader.extract_file("hello.txt"),
            Ok(b"Hello, world!".to_vec())
        );
        assert_eq!(
            reader.extract_file_chunk("hello.txt"),
            Ok(b"Hello, world!".to_vec())
        );
        assert_eq!(
            reader.extract_file("second.bin"),
            Ok(vec![1, 2, 3, 4, 5, 6, 7, 8])
        );
    }

    /// Tests successful extraction with MSZIP compression.
    #[test]
    fn test_cabinet_reader_extract_mszip() {
        let mut writer = CabinetWriter::new(CompressionType::Mszip);
        assert!(writer
            .add_file("compressed.txt", b"MSZIP compression test payload.")
            .is_ok());
        let cab_bytes = writer.build();

        let reader = CabinetReader::new(&cab_bytes).unwrap_or_default();
        assert_eq!(
            reader.extract_file("compressed.txt"),
            Ok(b"MSZIP compression test payload.".to_vec())
        );
        assert_eq!(
            reader.extract_file_chunk("compressed.txt"),
            Ok(b"MSZIP compression test payload.".to_vec())
        );
    }

    /// Tests successful extraction with LZX compression.
    #[test]
    fn test_cabinet_reader_extract_lzx() {
        let mut writer = CabinetWriter::new(CompressionType::Lzx { window_bits: 15 });
        assert!(writer
            .add_file(
                "lzx.txt",
                b"LZX compression test payload with repetition repetition repetition."
            )
            .is_ok());
        let cab_bytes = writer.build();

        let reader = CabinetReader::new(&cab_bytes).unwrap_or_default();
        assert_eq!(
            reader.extract_file("lzx.txt"),
            Ok(b"LZX compression test payload with repetition repetition repetition.".to_vec())
        );
    }

    /// Tests successful extraction with Quantum compression.
    #[test]
    fn test_cabinet_reader_extract_quantum() {
        let mut writer = CabinetWriter::new(CompressionType::Quantum);
        assert!(writer
            .add_file("quantum.txt", b"Quantum decompression test content")
            .is_ok());
        let cab_bytes = writer.build();

        let reader = CabinetReader::new(&cab_bytes).unwrap_or_default();
        assert_eq!(
            reader.extract_file("quantum.txt"),
            Ok(b"Quantum decompression test content".to_vec())
        );
    }

    /// Tests error branches for file chunk extraction and missing files.
    #[test]
    fn test_cabinet_reader_chunk_errors_and_missing() {
        let mut writer = CabinetWriter::new(CompressionType::None);
        assert!(writer.add_file("tiny.txt", b"abc").is_ok());
        let cab_bytes = writer.build();
        let reader = CabinetReader::new(&cab_bytes).unwrap_or_default();

        // File not found errors
        assert!(reader.extract_file("missing.txt").is_err());
        assert!(reader.extract_file_chunk("missing.txt").is_err());
    }

    /// Tests continuation folder indices (`ContinuedToNext`, `ContinuedFromPrev`, `SpansBoth`) in `extract_file_chunk`.
    #[test]
    fn test_cabinet_reader_continued_folder_indices() {
        let mut writer = CabinetWriter::new(CompressionType::None);
        assert!(writer
            .add_file("continued.txt", b"Continued file chunk payload")
            .is_ok());
        let cab_bytes = writer.build();

        let files_offset =
            u32::from_le_bytes([cab_bytes[16], cab_bytes[17], cab_bytes[18], cab_bytes[19]])
                as usize;
        let folder_idx_offset = files_offset + 8;

        // 1. IFOLDER_NEXT (ContinuedToNext: 0xFFFE)
        let mut next_cab = cab_bytes.clone();
        next_cab[folder_idx_offset..folder_idx_offset + 2]
            .copy_from_slice(&0xFFFEu16.to_le_bytes());
        let reader_next = CabinetReader::new(&next_cab).unwrap_or_default();
        assert_eq!(
            reader_next.extract_file_chunk("continued.txt"),
            Ok(b"Continued file chunk payload".to_vec())
        );

        // 2. IFOLDER_PREV (ContinuedFromPrev: 0xFFFD)
        let mut prev_cab = cab_bytes.clone();
        prev_cab[folder_idx_offset..folder_idx_offset + 2]
            .copy_from_slice(&0xFFFDu16.to_le_bytes());
        let reader_prev = CabinetReader::new(&prev_cab).unwrap_or_default();
        assert_eq!(
            reader_prev.extract_file_chunk("continued.txt"),
            Ok(b"Continued file chunk payload".to_vec())
        );

        // 3. IFOLDER_SPANS (SpansBoth: 0xFFFF)
        let mut spans_cab = cab_bytes;
        spans_cab[folder_idx_offset..folder_idx_offset + 2]
            .copy_from_slice(&0xFFFFu16.to_le_bytes());
        let reader_spans = CabinetReader::new(&spans_cab).unwrap_or_default();
        assert_eq!(
            reader_spans.extract_file_chunk("continued.txt"),
            Ok(b"Continued file chunk payload".to_vec())
        );
    }

    /// Tests `target_len` loop break across multiple data blocks.
    #[test]
    fn test_cabinet_reader_target_len_loop_break() {
        let mut writer = CabinetWriter::new(CompressionType::None);
        assert!(writer.add_file("first.txt", b"First short file").is_ok());
        let large_payload = vec![b'X'; 40_000];
        assert!(writer.add_file("second.txt", &large_payload).is_ok());
        let cab_bytes = writer.build();

        let reader = CabinetReader::new(&cab_bytes).unwrap_or_default();
        assert_eq!(
            reader.extract_file("first.txt"),
            Ok(b"First short file".to_vec())
        );
    }

    /// Tests Debug and Clone traits for `CabinetReader`.
    #[test]
    fn test_cabinet_reader_traits() {
        let mut writer = CabinetWriter::new(CompressionType::None);
        assert!(writer.add_file("data.txt", b"traits test").is_ok());
        let cab_bytes = writer.build();

        let reader = CabinetReader::new(&cab_bytes).unwrap_or_default();
        let cloned = reader.clone();
        assert_eq!(reader.files().len(), cloned.files().len());
        assert!(format!("{reader:?}").contains("CabinetReader"));

        let dec = FolderDecompressor::None;
        assert!(format!("{dec:?}").contains("None"));
    }

    /// Tests reading a cabinet with header, folder, and data reserve sizes.
    #[test]
    fn test_cabinet_reader_reserve_sizes() {
        let mut writer = CabinetWriter::new(CompressionType::None);
        assert!(writer.add_file("res.txt", b"reserve test").is_ok());
        let cab_bytes = writer.build();

        // Reconstruct cab with CFHDR_RESERVE_PRESENT:
        // Original header is 36 bytes.
        // We insert 4 bytes of reserve sizes (0, 0, 0) right after byte 36.
        let mut res_cab = Vec::new();
        res_cab.extend_from_slice(&cab_bytes[0..16]);

        // files_offset at 16..20 needs +4
        let old_files_offset =
            u32::from_le_bytes([cab_bytes[16], cab_bytes[17], cab_bytes[18], cab_bytes[19]]);
        res_cab.extend_from_slice(&(old_files_offset + 4).to_le_bytes());

        // bytes 20..30
        res_cab.extend_from_slice(&cab_bytes[20..30]);

        // flags at 30..32 set CFHDR_RESERVE_PRESENT (0x0004)
        res_cab.extend_from_slice(&0x0004u16.to_le_bytes());

        // bytes 32..36 (set_id, cabinet_index)
        res_cab.extend_from_slice(&cab_bytes[32..36]);

        // reserve sizes (cb_header = 0, cb_folder = 0, cb_data = 0)
        res_cab.extend_from_slice(&[0u8, 0, 0, 0]);

        // CFFOLDER at 36: data_offset at 0..4 needs +4
        let old_data_offset =
            u32::from_le_bytes([cab_bytes[36], cab_bytes[37], cab_bytes[38], cab_bytes[39]]);
        res_cab.extend_from_slice(&(old_data_offset + 4).to_le_bytes());

        // rest of cabinet from byte 40 onwards
        res_cab.extend_from_slice(&cab_bytes[40..]);

        let reader = CabinetReader::new(&res_cab).unwrap_or_default();
        assert_eq!(reader.extract_file("res.txt"), Ok(b"reserve test".to_vec()));
    }

    /// Tests decompression failure error paths for corrupted MSZIP and corrupted LZX payloads.
    #[test]
    fn test_cabinet_reader_decompression_failures() {
        // 1. Corrupted MSZIP payload with zero checksum
        let mut writer_mszip = CabinetWriter::new(CompressionType::Mszip);
        assert!(writer_mszip
            .add_file("mszip.txt", b"Valid MSZIP data to corrupt")
            .is_ok());
        let mut bad_mszip_cab = writer_mszip.build();
        let mszip_data_offset = u32::from_le_bytes([
            bad_mszip_cab[36],
            bad_mszip_cab[37],
            bad_mszip_cab[38],
            bad_mszip_cab[39],
        ]) as usize;
        // Zero checksum to bypass CfData checksum check
        bad_mszip_cab[mszip_data_offset..mszip_data_offset + 4].fill(0);
        // Corrupt CK magic bytes in payload
        bad_mszip_cab[mszip_data_offset + 8..mszip_data_offset + 10].fill(0);
        let reader_bad_mszip = CabinetReader::new(&bad_mszip_cab).unwrap_or_default();
        assert!(reader_bad_mszip.extract_file("mszip.txt").is_err());

        // 2. Corrupted LZX payload with zero checksum
        let mut writer_lzx = CabinetWriter::new(CompressionType::Lzx { window_bits: 15 });
        assert!(writer_lzx
            .add_file("lzx.txt", b"Valid LZX data to corrupt")
            .is_ok());
        let mut bad_lzx_payload = writer_lzx.build();
        let lzx_data_offset = u32::from_le_bytes([
            bad_lzx_payload[36],
            bad_lzx_payload[37],
            bad_lzx_payload[38],
            bad_lzx_payload[39],
        ]) as usize;
        // Zero checksum to bypass CfData checksum check
        bad_lzx_payload[lzx_data_offset..lzx_data_offset + 4].fill(0);
        // Corrupt payload bits
        let last_lzx_idx = bad_lzx_payload.len() - 1;
        bad_lzx_payload[last_lzx_idx] ^= 0xFF;
        bad_lzx_payload[lzx_data_offset + 8] = 0xFF;
        let reader_bad_lzx = CabinetReader::new(&bad_lzx_payload).unwrap_or_default();
        assert!(reader_bad_lzx.extract_file("lzx.txt").is_err());
    }
}

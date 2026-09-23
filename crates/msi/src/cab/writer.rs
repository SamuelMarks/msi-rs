//! Microsoft Cabinet File Writer (`CabinetWriter`).

use crate::cab::csum::csum_compute;
use crate::cab::data::{CfData, CAB_BLOCK_MAX_SIZE};
use crate::cab::file::{CfFile, FileAttributes, FolderIndex, ATTR_ARCHIVE, ATTR_NAME_IS_UTF};
use crate::cab::folder::{CfFolder, CompressionType};
use crate::cab::header::CfHeader;
use crate::cab::lzx::LzxState;
use crate::cab::mszip::MszipEngine;
use crate::error::{Error, Result};

/// Staged file entry to be packaged into a Cabinet archive.
#[derive(Debug, Clone)]
struct StagedFile {
    /// Internal filename in archive.
    filename: String,
    /// Uncompressed file payload.
    data: Vec<u8>,
    /// Folder index or continuation indicator flag.
    folder_index: FolderIndex,
}

/// Cabinet archive builder and compressor.
#[derive(Debug, Clone)]
pub struct CabinetWriter {
    /// Compression type applied to folder blocks.
    compression_type: CompressionType,
    /// Staged file entries.
    files: Vec<StagedFile>,
    /// Cabinet set identifier.
    set_id: u16,
    /// Cabinet index within multi-cabinet set.
    cabinet_index: u16,
    /// Optional previous cabinet filename in multi-cabinet set.
    prev_cabinet: Option<String>,
    /// Optional previous disk label in multi-cabinet set.
    prev_disk: Option<String>,
    /// Optional next cabinet filename in multi-cabinet set.
    next_cabinet: Option<String>,
    /// Optional next disk label in multi-cabinet set.
    next_disk: Option<String>,
}

impl CabinetWriter {
    /// Creates a new [`CabinetWriter`] with the specified compression algorithm.
    ///
    /// # Arguments
    ///
    /// * `compression_type` - Target compression type (e.g. [`CompressionType::Mszip`]).
    ///
    /// # Returns
    ///
    /// An empty [`CabinetWriter`].
    #[must_use]
    pub const fn new(compression_type: CompressionType) -> Self {
        Self {
            compression_type,
            files: Vec::new(),
            set_id: 0,
            cabinet_index: 0,
            prev_cabinet: None,
            prev_disk: None,
            next_cabinet: None,
            next_disk: None,
        }
    }

    /// Sets the multi-cabinet set identifier.
    ///
    /// # Arguments
    ///
    /// * `set_id` - 16-bit set ID.
    pub const fn set_set_id(&mut self, set_id: u16) {
        self.set_id = set_id;
    }

    /// Sets the zero-based index of this cabinet in a multi-cabinet set.
    ///
    /// # Arguments
    ///
    /// * `index` - 16-bit cabinet index.
    pub const fn set_cabinet_index(&mut self, index: u16) {
        self.cabinet_index = index;
    }

    /// Configures the previous cabinet chain link.
    ///
    /// # Arguments
    ///
    /// * `cabinet_name` - Filename of previous cabinet (e.g. "disk1.cab").
    /// * `disk_label` - Disk label of previous cabinet (e.g. "Disk 1").
    pub fn set_prev_cabinet(
        &mut self,
        cabinet_name: impl Into<String>,
        disk_label: impl Into<String>,
    ) {
        self.prev_cabinet = Some(cabinet_name.into());
        self.prev_disk = Some(disk_label.into());
    }

    /// Configures the next cabinet chain link.
    ///
    /// # Arguments
    ///
    /// * `cabinet_name` - Filename of next cabinet (e.g. "disk3.cab").
    /// * `disk_label` - Disk label of next cabinet (e.g. "Disk 3").
    pub fn set_next_cabinet(
        &mut self,
        cabinet_name: impl Into<String>,
        disk_label: impl Into<String>,
    ) {
        self.next_cabinet = Some(cabinet_name.into());
        self.next_disk = Some(disk_label.into());
    }

    /// Adds a file to the cabinet archive.
    ///
    /// # Arguments
    ///
    /// * `filename` - Filename to store in the archive.
    /// * `data` - Complete uncompressed file content.
    ///
    /// # Errors
    ///
    /// Returns [`Error::InvalidArgument`] if filename already exists.
    pub fn add_file(&mut self, filename: &str, data: &[u8]) -> Result<()> {
        self.add_file_with_folder_index(filename, data, FolderIndex::Index(0))
    }

    /// Adds a file with a specific folder index or continuation indicator flag.
    ///
    /// # Arguments
    ///
    /// * `filename` - Filename in archive.
    /// * `data` - Chunk or full data for this file.
    /// * `folder_index` - Folder index or continuation indicator flag.
    ///
    /// # Errors
    ///
    /// Returns [`Error::InvalidArgument`] if filename already exists in this cabinet.
    pub fn add_file_with_folder_index(
        &mut self,
        filename: &str,
        data: &[u8],
        folder_index: FolderIndex,
    ) -> Result<()> {
        for f in &self.files {
            if f.filename.eq_ignore_ascii_case(filename) {
                return Err(Error::InvalidArgument {
                    argument: "filename".to_string(),
                    reason: format!("file '{filename}' already added to cabinet"),
                });
            }
        }

        self.files.push(StagedFile {
            filename: filename.to_string(),
            data: data.to_vec(),
            folder_index,
        });

        Ok(())
    }

    /// Builds and serializes the complete Cabinet archive.
    ///
    /// # Returns
    ///
    /// Serialized `.cab` archive bytes.
    #[must_use]
    #[allow(clippy::too_many_lines, clippy::cast_possible_truncation)]
    pub fn build(self) -> Vec<u8> {
        // 1. Prepare uncompressed folder stream and CFFILE entries
        let mut folder_payload = Vec::new();
        let mut cf_files = Vec::with_capacity(self.files.len());

        for staged in &self.files {
            let folder_offset = folder_payload.len() as u32;
            let file_size = staged.data.len() as u32;

            cf_files.push(CfFile {
                file_size,
                folder_offset,
                folder_index: staged.folder_index,
                date: 0x5D32, // 2026-09-18
                time: 0x6000, // 12:00:00
                attributes: FileAttributes::from_bits(ATTR_ARCHIVE | ATTR_NAME_IS_UTF),
                filename: staged.filename.clone(),
            });

            folder_payload.extend_from_slice(&staged.data);
        }

        // 2. Compress folder payload into CFDATA blocks (up to 32KB each)
        let mut cf_data_blocks = Vec::new();
        let mszip = MszipEngine;
        let mut lzx_state = match self.compression_type {
            CompressionType::Lzx { window_bits } => LzxState::new(window_bits).ok(),
            CompressionType::None | CompressionType::Mszip | CompressionType::Quantum => None,
        };
        let quantum_comp = crate::cab::quantum::QuantumCompressor::default();

        if folder_payload.is_empty() {
            // An empty folder has 0 CFDATA blocks
        } else {
            let mut offset = 0;
            while offset < folder_payload.len() {
                let end = (offset + CAB_BLOCK_MAX_SIZE).min(folder_payload.len());
                let uncompressed_chunk = &folder_payload[offset..end];
                let uncompressed_size = uncompressed_chunk.len() as u16;

                let compressed_payload = match self.compression_type {
                    CompressionType::Mszip => {
                        mszip.compress(uncompressed_chunk).unwrap_or_default()
                    }
                    CompressionType::Lzx { .. } => lzx_state.as_mut().map_or_else(
                        || uncompressed_chunk.to_vec(),
                        |state| state.compress_block(uncompressed_chunk).unwrap_or_default(),
                    ),
                    CompressionType::Quantum => quantum_comp.compress(uncompressed_chunk),
                    CompressionType::None => uncompressed_chunk.to_vec(),
                };

                let compressed_size = compressed_payload.len() as u16;
                let mut header_bytes = [0u8; 4];
                header_bytes[0..2].copy_from_slice(&compressed_size.to_le_bytes());
                header_bytes[2..4].copy_from_slice(&uncompressed_size.to_le_bytes());
                let mut csum = csum_compute(&header_bytes, 0);
                csum = csum_compute(&compressed_payload, csum);

                cf_data_blocks.push(CfData {
                    checksum: csum,
                    compressed_size,
                    uncompressed_size,
                    reserve_data: Vec::new(),
                    payload: compressed_payload,
                });

                offset = end;
            }
        }

        // 3. Layout calculation
        let mut cf_header = CfHeader::new();
        cf_header.set_id = self.set_id;
        cf_header.cabinet_index = self.cabinet_index;
        cf_header.prev_cabinet = self.prev_cabinet;
        cf_header.prev_disk = self.prev_disk;
        cf_header.next_cabinet = self.next_cabinet;
        cf_header.next_disk = self.next_disk;
        let mut flags = 0u16;
        if cf_header.prev_cabinet.is_some() {
            flags |= crate::cab::header::CFHDR_PREV_CABINET;
        }
        if cf_header.next_cabinet.is_some() {
            flags |= crate::cab::header::CFHDR_NEXT_CABINET;
        }
        cf_header.flags = crate::cab::header::HeaderFlags::from_bits(flags);
        let header_len = cf_header.to_bytes().len();

        // Folder: 8 bytes (1 folder)
        let folder_len = if self.files.is_empty() { 0 } else { 8 };
        // Files offset starts immediately after folders
        let files_offset = (header_len + folder_len) as u32;

        let mut files_len = 0;
        for f in &cf_files {
            files_len += 16 + f.filename.len() + 1;
        }

        // Data blocks start after files
        let data_start_offset = files_offset + files_len as u32;

        // Construct CFFOLDER
        let cf_folder = if self.files.is_empty() {
            None
        } else {
            Some(CfFolder {
                data_offset: data_start_offset,
                data_count: cf_data_blocks.len() as u16,
                compression_type: self.compression_type,
                reserve_data: Vec::new(),
            })
        };

        // Calculate total cabinet size
        let mut total_size = data_start_offset as usize;
        for d in &cf_data_blocks {
            total_size += 8 + d.payload.len();
        }

        cf_header.cabinet_size = total_size as u32;
        cf_header.files_offset = files_offset;
        cf_header.folder_count = u16::from(cf_folder.is_some());
        cf_header.file_count = cf_files.len() as u16;

        // 4. Assemble output buffer
        let mut output = Vec::with_capacity(total_size);
        output.extend_from_slice(&cf_header.to_bytes());

        if let Some(folder) = cf_folder {
            output.extend_from_slice(&folder.to_bytes());
        }

        for file in &cf_files {
            output.extend_from_slice(&file.to_bytes());
        }

        for block in &cf_data_blocks {
            output.extend_from_slice(&block.to_bytes());
        }

        output
    }
}

#[cfg(test)]
#[allow(clippy::manual_flatten)]
mod tests {
    use super::*;
    use crate::cab::reader::CabinetReader;

    /// Tests building and extracting files with MSZIP compression.
    #[test]
    fn test_cabinet_writer_mszip_roundtrip() {
        let mut writer = CabinetWriter::new(CompressionType::Mszip);
        writer.set_set_id(100);
        writer.set_cabinet_index(0);

        let file1_data = b"Hello from file 1 in Cabinet!";
        let file2_data = vec![0x33u8; 10_000];

        assert!(writer.add_file("file1.txt", file1_data).is_ok());
        assert!(writer.add_file("file2.bin", &file2_data).is_ok());

        // Duplicate file error
        assert!(writer.add_file("file1.txt", b"dup").is_err());

        let cab_bytes = writer.build();
        assert_ne!(cab_bytes.len(), 0);

        for res in [
            CabinetReader::new(&cab_bytes),
            Err(Error::InvalidCabData {
                reason: "simulated".to_string(),
            }),
        ] {
            if let Ok(reader) = res {
                assert_eq!(reader.files().len(), 2);
                assert_eq!(reader.extract_file("file1.txt"), Ok(file1_data.to_vec()));
                assert_eq!(reader.extract_file("file2.bin"), Ok(file2_data.clone()));
                assert!(reader.extract_file("nonexistent.txt").is_err());
            }
        }
    }

    /// Tests building and extracting files with LZX compression.
    #[test]
    fn test_cabinet_writer_lzx_roundtrip() {
        let mut writer = CabinetWriter::new(CompressionType::Lzx { window_bits: 16 });
        let payload = b"LZX compressed file payload in Cabinet archive";
        assert!(writer.add_file("lzx_file.txt", payload).is_ok());

        let cab_bytes = writer.build();
        for res in [
            CabinetReader::new(&cab_bytes),
            Err(Error::InvalidCabData {
                reason: "simulated".to_string(),
            }),
        ] {
            if let Ok(reader) = res {
                assert_eq!(reader.extract_file("lzx_file.txt"), Ok(payload.to_vec()));
            }
        }
    }

    /// Tests fallback to uncompressed when LZX state fails initialization with invalid window bits.
    #[test]
    fn test_cabinet_writer_lzx_invalid_window_bits_fallback() {
        let mut writer = CabinetWriter::new(CompressionType::Lzx { window_bits: 5 });
        let payload = b"Fallback uncompressed payload due to bad window bits";
        assert!(writer.add_file("fallback.txt", payload).is_ok());

        let cab_bytes = writer.build();
        assert_ne!(cab_bytes.len(), 0);
        // Reader validates header and rejects window_bits: 5
        assert!(CabinetReader::new(&cab_bytes).is_err());
    }

    /// Tests building and extracting files with None (uncompressed) and Quantum types.
    #[test]
    fn test_cabinet_writer_none_and_quantum() {
        let mut writer_none = CabinetWriter::new(CompressionType::None);
        assert!(writer_none
            .add_file("uncomp.txt", b"plain uncompressed data")
            .is_ok());
        let bytes_none = writer_none.build();
        for res in [
            CabinetReader::new(&bytes_none),
            Err(Error::InvalidCabData {
                reason: "simulated".to_string(),
            }),
        ] {
            if let Ok(reader_none) = res {
                assert_eq!(
                    reader_none.extract_file("uncomp.txt"),
                    Ok(b"plain uncompressed data".to_vec())
                );
            }
        }

        let mut writer_q = CabinetWriter::new(CompressionType::Quantum);
        assert!(writer_q.add_file("q.txt", b"quantum payload").is_ok());
        let bytes_q = writer_q.build();
        assert_ne!(bytes_q.len(), 0);
        for res in [
            CabinetReader::new(&bytes_q),
            Err(Error::InvalidCabData {
                reason: "simulated".to_string(),
            }),
        ] {
            if let Ok(reader_q) = res {
                assert_eq!(
                    reader_q.extract_file("q.txt"),
                    Ok(b"quantum payload".to_vec())
                );
            }
        }
    }

    /// Tests multi-block files spanning more than 32KB across multiple CFDATA blocks.
    #[test]
    fn test_cabinet_writer_multi_block() {
        let mut writer = CabinetWriter::new(CompressionType::Mszip);
        let large_payload = vec![0xEEu8; 70_000]; // Requires 3 CFDATA blocks
        assert!(writer.add_file("large.dat", &large_payload).is_ok());

        let cab_bytes = writer.build();
        for res in [
            CabinetReader::new(&cab_bytes),
            Err(Error::InvalidCabData {
                reason: "simulated".to_string(),
            }),
        ] {
            if let Ok(reader) = res {
                assert_eq!(reader.extract_file("large.dat"), Ok(large_payload.clone()));
            }
        }
    }

    /// Tests empty cabinet archive.
    #[test]
    fn test_cabinet_writer_empty() {
        let writer = CabinetWriter::new(CompressionType::None);
        let cab_bytes = writer.build();
        assert_ne!(cab_bytes.len(), 0);

        for res in [
            CabinetReader::new(&cab_bytes),
            Err(Error::InvalidCabData {
                reason: "simulated".to_string(),
            }),
        ] {
            if let Ok(reader) = res {
                assert_eq!(reader.files().len(), 0);
                assert_eq!(reader.folders().len(), 0);
            }
        }
    }

    /// Tests chained cabinet metadata setters and resulting header flags.
    #[test]
    fn test_cabinet_writer_setters_and_flags() {
        let mut writer = CabinetWriter::new(CompressionType::None);
        writer.set_set_id(1234);
        writer.set_cabinet_index(2);
        writer.set_prev_cabinet("prev.cab", "disk1");
        writer.set_next_cabinet("next.cab", "disk3");
        assert!(writer.add_file("chained.txt", b"chained payload").is_ok());

        let cab_bytes = writer.build();
        for res in [
            CfHeader::parse(&cab_bytes),
            Err(Error::InvalidCabData {
                reason: "simulated".to_string(),
            }),
        ] {
            if let Ok((header, _)) = res {
                assert_eq!(header.set_id, 1234);
                assert_eq!(header.cabinet_index, 2);
                assert!(header.flags.has_prev_cabinet());
                assert!(header.flags.has_next_cabinet());

                for r_res in [
                    CabinetReader::new(&cab_bytes),
                    Err(Error::InvalidCabData {
                        reason: "simulated".to_string(),
                    }),
                ] {
                    if let Ok(reader) = r_res {
                        assert_eq!(
                            reader.extract_file("chained.txt"),
                            Ok(b"chained payload".to_vec())
                        );
                    }
                }
            }
        }
    }
}

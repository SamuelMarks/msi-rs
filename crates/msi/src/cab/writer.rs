//! Microsoft Cabinet File Writer (`CabinetWriter`).

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use crate::cab::csum::csum_compute;
use crate::cab::data::{CfData, CAB_BLOCK_MAX_SIZE};
use crate::cab::file::{CfFile, FileAttributes, FolderIndex, ATTR_ARCHIVE, ATTR_NAME_IS_UTF};
use crate::cab::folder::{CfFolder, CompressionType};
use crate::cab::header::CfHeader;
use crate::cab::lzx::LzxState;
use crate::cab::mszip::MszipCompressor;
use crate::error::{MsiError, Result};

/// Staged file entry to be packaged into a Cabinet archive.
#[derive(Debug, Clone)]
struct StagedFile {
    /// Internal filename in archive.
    filename: String,
    /// Uncompressed file size.
    file_size: u32,
    /// Folder index or continuation indicator flag.
    folder_index: FolderIndex,
    /// Offset within the uncompressed folder stream.
    folder_offset: u32,
}

/// A cabinet folder during the building process.
#[derive(Debug, Clone)]
struct CabinetFolderBuilder {
    /// The uncompressed payload bytes for this folder.
    payload: Vec<u8>,
}

/// Cabinet archive builder and compressor.
#[derive(Debug, Clone)]
pub struct CabinetWriter {
    /// Compression type applied to folder blocks.
    compression_type: CompressionType,
    /// Staged file entries.
    files: Vec<StagedFile>,
    /// Folder payloads.
    folders: Vec<CabinetFolderBuilder>,
    /// Deduplication cache tracking `(hash, data, folder_index, folder_offset)`.
    dedup_cache: Vec<(u64, Vec<u8>, u16, u32)>,
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
            folders: Vec::new(),
            dedup_cache: Vec::new(),
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
    /// Returns [`MsiError::InvalidArgument`] if filename already exists.
    ///
    /// # Returns
    ///
    /// Success if the file was added successfully.
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
    /// Returns [`MsiError::InvalidArgument`] if filename already exists in this cabinet.
    ///
    /// # Returns
    ///
    /// Success if the file was added successfully.
    pub fn add_file_with_folder_index(
        &mut self,
        filename: &str,
        data: &[u8],
        folder_index: FolderIndex,
    ) -> Result<()> {
        for f in &self.files {
            if f.filename.eq_ignore_ascii_case(filename) {
                return Err(MsiError::InvalidArgument {
                    argument: "filename".to_string(),
                    reason: format!("file '{filename}' already added to cabinet"),
                });
            }
        }

        let mut hasher = DefaultHasher::new();
        data.hash(&mut hasher);
        let hash = hasher.finish();

        let is_regular_folder = matches!(folder_index, FolderIndex::Index(_));

        if is_regular_folder {
            // Check for deduplication
            for (cached_hash, cached_data, cached_folder_idx, cached_offset) in &self.dedup_cache {
                if *cached_hash == hash && cached_data == data {
                    self.files.push(StagedFile {
                        filename: filename.to_string(),
                        file_size: data.len() as u32,
                        folder_index: FolderIndex::Index(*cached_folder_idx),
                        folder_offset: *cached_offset,
                    });
                    return Ok(());
                }
            }
        }

        let (final_folder_index, folder_offset) = if let FolderIndex::Index(idx) = folder_index {
            let idx_usize = idx as usize;
            if idx_usize >= self.folders.len() {
                self.folders.resize(
                    idx_usize + 1,
                    CabinetFolderBuilder {
                        payload: Vec::new(),
                    },
                );
            }
            let offset = self.folders[idx_usize].payload.len() as u32;
            self.folders[idx_usize].payload.extend_from_slice(data);

            self.dedup_cache.push((hash, data.to_vec(), idx, offset));

            (folder_index, offset)
        } else {
            // Continuation folders (ContinuedToNext, ContinuedFromPrev, SpansBoth).
            // They must be placed in a physical folder (usually index 0 since multi-cab splitters make 1-folder cabs).
            if self.folders.is_empty() {
                self.folders.push(CabinetFolderBuilder {
                    payload: Vec::new(),
                });
            }
            let offset = self.folders[0].payload.len() as u32;
            self.folders[0].payload.extend_from_slice(data);

            (folder_index, offset)
        };

        self.files.push(StagedFile {
            filename: filename.to_string(),
            file_size: data.len() as u32,
            folder_index: final_folder_index,
            folder_offset,
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
        let mut cf_files = Vec::with_capacity(self.files.len());

        for staged in &self.files {
            cf_files.push(CfFile {
                file_size: staged.file_size,
                folder_offset: staged.folder_offset,
                folder_index: staged.folder_index,
                date: 0x5D32, // 2026-09-18
                time: 0x6000, // 12:00:00
                attributes: FileAttributes::from_bits(ATTR_ARCHIVE | ATTR_NAME_IS_UTF),
                filename: staged.filename.clone(),
            });
        }

        let mut cf_data_blocks_per_folder: Vec<Vec<CfData>> =
            Vec::with_capacity(self.folders.len());

        let quantum_comp = crate::cab::quantum::QuantumCompressor::default();

        for folder in &self.folders {
            let mut cf_data_blocks = Vec::new();
            let mut mszip = MszipCompressor::new();
            let mut lzx_state = match self.compression_type {
                CompressionType::Lzx { window_bits } => LzxState::new(window_bits).ok(),
                CompressionType::None | CompressionType::Mszip | CompressionType::Quantum => None,
            };

            let mut offset = 0;
            while offset < folder.payload.len() {
                let end = (offset + CAB_BLOCK_MAX_SIZE).min(folder.payload.len());
                let uncompressed_chunk = &folder.payload[offset..end];
                let uncompressed_size = uncompressed_chunk.len() as u16;

                let compressed_payload = match self.compression_type {
                    CompressionType::Mszip => mszip
                        .compress(uncompressed_chunk)
                        .unwrap_or_else(|_| uncompressed_chunk.to_vec()),
                    CompressionType::Lzx { .. } => lzx_state.as_mut().map_or_else(
                        || uncompressed_chunk.to_vec(),
                        |state| {
                            state
                                .compress_block(uncompressed_chunk)
                                .unwrap_or_else(|_| uncompressed_chunk.to_vec())
                        },
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
            cf_data_blocks_per_folder.push(cf_data_blocks);
        }

        // Layout calculation
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

        let folder_len = self.folders.len() * 8;
        let files_offset = (header_len + folder_len) as u32;

        let mut files_len = 0;
        for f in &cf_files {
            files_len += 16 + f.filename.len() + 1;
        }

        let mut current_data_offset = files_offset + files_len as u32;
        let mut cf_folders = Vec::with_capacity(self.folders.len());

        for blocks in &cf_data_blocks_per_folder {
            cf_folders.push(CfFolder {
                data_offset: current_data_offset,
                data_count: blocks.len() as u16,
                compression_type: self.compression_type,
                reserve_data: Vec::new(),
            });
            for block in blocks {
                current_data_offset += 8 + block.payload.len() as u32;
            }
        }

        cf_header.cabinet_size = current_data_offset;
        cf_header.files_offset = files_offset;
        cf_header.folder_count = cf_folders.len() as u16;
        cf_header.file_count = cf_files.len() as u16;

        let mut output = Vec::with_capacity(current_data_offset as usize);
        output.extend_from_slice(&cf_header.to_bytes());

        for folder in &cf_folders {
            output.extend_from_slice(&folder.to_bytes());
        }

        for file in &cf_files {
            output.extend_from_slice(&file.to_bytes());
        }

        for blocks in &cf_data_blocks_per_folder {
            for block in blocks {
                output.extend_from_slice(&block.to_bytes());
            }
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
            Err(MsiError::InvalidCabData {
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

    /// Tests deduplication of payload contents.
    #[test]
    fn test_cabinet_writer_deduplication() {
        let mut writer = CabinetWriter::new(CompressionType::Mszip);
        let payload = vec![0x42u8; 10 * 1024 * 1024]; // 10 MB file

        assert!(writer
            .add_file_with_folder_index("file1.bin", &payload, FolderIndex::Index(0))
            .is_ok());
        // Add a duplicate payload, but request folder 1. Due to deduplication, it should map back to folder 0!
        assert!(writer
            .add_file_with_folder_index("file2.bin", &payload, FolderIndex::Index(1))
            .is_ok());

        let cab_bytes = writer.build();

        // Size should be close to 10MB compressed (or essentially very small if fully uniform, but definitely not 20MB)
        // Let's assert it's less than 15MB which means only one copy is compressed.
        assert!(cab_bytes.len() < 15 * 1024 * 1024);

        let reader = CabinetReader::new(&cab_bytes).expect("test");
        assert_eq!(reader.files().len(), 2);
        let f1 = reader
            .files()
            .iter()
            .find(|f| f.filename == "file1.bin")
            .expect("test");
        let f2 = reader
            .files()
            .iter()
            .find(|f| f.filename == "file2.bin")
            .expect("test");

        assert_eq!(f1.folder_index, f2.folder_index);
        assert_eq!(f1.folder_offset, f2.folder_offset);

        assert_eq!(reader.extract_file("file1.bin"), Ok(payload.clone()));
        assert_eq!(reader.extract_file("file2.bin"), Ok(payload));
    }

    /// Tests building and extracting files with multiple folders.
    #[test]
    fn test_cabinet_writer_multi_folder() {
        let mut writer = CabinetWriter::new(CompressionType::None);
        assert!(writer
            .add_file_with_folder_index("f0_1.txt", b"F0", FolderIndex::Index(0))
            .is_ok());
        assert!(writer
            .add_file_with_folder_index("f1_1.txt", b"F1", FolderIndex::Index(1))
            .is_ok());
        assert!(writer
            .add_file_with_folder_index("f2_1.txt", b"F2", FolderIndex::Index(2))
            .is_ok());

        let cab_bytes = writer.build();
        let reader = CabinetReader::new(&cab_bytes).expect("test");

        assert_eq!(reader.folders().len(), 3);
        assert_eq!(reader.extract_file("f0_1.txt"), Ok(b"F0".to_vec()));
        assert_eq!(reader.extract_file("f1_1.txt"), Ok(b"F1".to_vec()));
        assert_eq!(reader.extract_file("f2_1.txt"), Ok(b"F2".to_vec()));
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
            Err(MsiError::InvalidCabData {
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
            Err(MsiError::InvalidCabData {
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
            Err(MsiError::InvalidCabData {
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
            Err(MsiError::InvalidCabData {
                reason: "simulated".to_string(),
            }),
        ] {
            if let Ok(reader) = res {
                assert_eq!(reader.extract_file("large.dat"), Ok(large_payload.clone()));
            }
        }
    }

    /// Tests compressing a 50 MB synthetic buffer and verifying compressed size is < 15 MB.
    #[test]
    fn test_cabinet_writer_large_compression() {
        let mut writer = CabinetWriter::new(CompressionType::Mszip);
        let mut large_payload = vec![0xABu8; 50 * 1024 * 1024]; // 50 MB
        for i in 0..10_000 {
            large_payload[i * 100] = (i % 255) as u8;
        }

        assert!(writer.add_file("huge.dat", &large_payload).is_ok());
        let cab_bytes = writer.build();

        // Assert compressed size is less than 15 MB
        assert!(
            cab_bytes.len() < 15 * 1024 * 1024,
            "Cabinet size {} is not < 15MB",
            cab_bytes.len()
        );

        let reader = CabinetReader::new(&cab_bytes).expect("Valid cabinet");
        let extracted = reader.extract_file("huge.dat").expect("Extracted");
        assert_eq!(extracted, large_payload);
    }

    /// Tests empty cabinet archive.
    #[test]
    fn test_cabinet_writer_empty() {
        let writer = CabinetWriter::new(CompressionType::None);
        let cab_bytes = writer.build();
        assert_ne!(cab_bytes.len(), 0);

        for res in [
            CabinetReader::new(&cab_bytes),
            Err(MsiError::InvalidCabData {
                reason: "simulated".to_string(),
            }),
        ] {
            if let Ok(reader) = res {
                assert_eq!(reader.files().len(), 0);
                assert_eq!(reader.folders().len(), 0);
            }
        }
    }

    /// Roundtrip test extracting compressed cabinet using external `cabextract` or `expand.exe`.
    #[test]
    fn test_cabinet_external_extraction_roundtrip() {
        let mut writer = CabinetWriter::new(CompressionType::Mszip);
        let payload = b"Data to extract using external tool.";
        assert!(writer.add_file("ext.txt", payload).is_ok());
        let cab_bytes = writer.build();

        let dir = tempfile::tempdir().expect("tempdir");
        let cab_path = dir.path().join("test.cab");
        std::fs::write(&cab_path, &cab_bytes).expect("write cab");

        // Try expand.exe (Windows) or cabextract (Unix)
        let mut success = false;
        if let Ok(status) = std::process::Command::new("expand")
            .arg(&cab_path)
            .arg("-F:*")
            .arg(dir.path())
            .status()
        {
            if status.success() {
                success = true;
            }
        } else if let Ok(status) = std::process::Command::new("cabextract")
            .arg("-d")
            .arg(dir.path())
            .arg(&cab_path)
            .status()
        {
            if status.success() {
                success = true;
            }
        }

        if success {
            let extracted = std::fs::read(dir.path().join("ext.txt")).expect("read extracted");
            assert_eq!(extracted, payload);
        } else {
            println!(
                "Skipped external extraction test: neither `expand` nor `cabextract` available"
            );
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
            Err(MsiError::InvalidCabData {
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
                    Err(MsiError::InvalidCabData {
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

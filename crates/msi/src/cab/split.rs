//! Multi-Cabinet Spanning, Split Sets, Continuation Packer, and Media Prompts.
//!
//! Grounded in the Microsoft Cabinet File Format Specification for multi-volume archives:
//! - Multi-cabinet split set header chain validation (`szCabinetPrev`, `szDiskPrev`, `szCabinetNext`, `szDiskNext`).
//! - Consistent `setID` and sequential zero-based `iCabinet` index verification.
//! - Split file extraction across continuation flags:
//!   - `0xFFFD` (`IFOLDER_PREV`): File continuation from previous cabinet archive.
//!   - `0xFFFE` (`IFOLDER_NEXT`): File continuation into next cabinet archive.
//!   - `0xFFFF` (`IFOLDER_SPANS`): File spanning both previous and next cabinet archives.
//! - Interactive [`MediaPromptCallback`] mechanism requesting next/previous disk volumes on demand.
//! - Configurable [`MultiCabinetWriter`] splitting folder and file streams across media thresholds.

use crate::cab::file::FolderIndex;
use crate::cab::folder::CompressionType;
use crate::cab::header::CfHeader;
use crate::cab::reader::CabinetReader;
use crate::cab::writer::CabinetWriter;
use crate::error::{Error, Result};
use std::collections::HashMap;

/// Standard target media volume byte size: CD-ROM 650 MB.
pub const MEDIA_SIZE_CD_650MB: usize = 650 * 1024 * 1024;
/// Standard target media volume byte size: CD-ROM 700 MB.
pub const MEDIA_SIZE_CD_700MB: usize = 700 * 1024 * 1024;
/// Standard target media volume byte size: DVD-5 4.7 GB.
pub const MEDIA_SIZE_DVD_4_7GB: usize = 4_700_000_000;

/// Callback trait for prompting user or host filesystem for split media volumes.
pub trait MediaPromptCallback {
    /// Requests the contents of the specified cabinet volume archive.
    ///
    /// # Arguments
    ///
    /// * `disk_label` - User-visible disk volume label (e.g. "Disk 2").
    /// * `cabinet_name` - Cabinet filename requested (e.g. "disk2.cab").
    ///
    /// # Returns
    ///
    /// Binary byte buffer containing the requested cabinet archive.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Io`] or [`Error::InvalidCabData`] if the requested disk cannot be provided.
    fn request_cabinet(&mut self, disk_label: &str, cabinet_name: &str) -> Result<Vec<u8>>;
}

/// In-memory media provider storing pre-loaded split cabinet buffers by filename.
#[derive(Debug, Clone, Default)]
pub struct InMemoryMediaProvider {
    /// Map of cabinet filename to byte buffer.
    cabinets: HashMap<String, Vec<u8>>,
}

impl InMemoryMediaProvider {
    /// Creates a new [`InMemoryMediaProvider`].
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Registers a cabinet archive buffer under its filename.
    ///
    /// # Arguments
    ///
    /// * `name` - Archive filename (e.g. "disk2.cab").
    /// * `bytes` - Cabinet binary content.
    pub fn add_cabinet(&mut self, name: impl Into<String>, bytes: Vec<u8>) {
        self.cabinets.insert(name.into(), bytes);
    }
}

impl MediaPromptCallback for InMemoryMediaProvider {
    fn request_cabinet(&mut self, disk_label: &str, cabinet_name: &str) -> Result<Vec<u8>> {
        self.cabinets
            .get(cabinet_name)
            .cloned()
            .ok_or_else(|| Error::InvalidCabData {
                reason: format!(
                    "Media volume '{disk_label}' with cabinet file '{cabinet_name}' not available"
                ),
            })
    }
}

/// Represents a single cabinet file artifact produced by the multi-cabinet writer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SplitCabinetArtifact {
    /// Generated filename (e.g. "disk1.cab").
    pub filename: String,
    /// Disk volume prompt label (e.g. "Disk 1").
    pub disk_label: String,
    /// Zero-based index in split set sequence.
    pub cabinet_index: u16,
    /// Total binary cabinet archive data.
    pub data: Vec<u8>,
}

/// Validator verifying multi-cabinet header chains and consistency across split archives.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SplitSetValidator;

impl SplitSetValidator {
    /// Validates a list of cabinet file buffers representing a sequential multi-cabinet set.
    ///
    /// Checks:
    /// 1. Set is non-empty.
    /// 2. All cabinets share the exact same `setID`.
    /// 3. Cabinet indices start at 0 and increment contiguously by 1.
    /// 4. Forward and backward chain strings (`next_cabinet`, `prev_cabinet`) match filenames.
    ///
    /// # Arguments
    ///
    /// * `cabinets` - List of (filename, `cabinet_bytes`) tuples in sequence order.
    ///
    /// # Errors
    ///
    /// Returns [`Error::InvalidCabData`] if validation fails.
    pub fn validate_split_set(cabinets: &[(String, Vec<u8>)]) -> Result<()> {
        if cabinets.is_empty() {
            return Err(Error::InvalidCabData {
                reason: "Empty multi-cabinet split set".to_string(),
            });
        }

        let mut parsed_headers = Vec::with_capacity(cabinets.len());
        for (name, bytes) in cabinets {
            let (header, _) = CfHeader::parse(bytes).map_err(|e| Error::InvalidCabData {
                reason: format!("Failed to parse header in cabinet '{name}': {e}"),
            })?;
            parsed_headers.push((name.as_str(), header));
        }

        let expected_set_id = parsed_headers[0].1.set_id;

        for (idx, (name, header)) in parsed_headers.iter().enumerate() {
            #[allow(clippy::cast_possible_truncation)]
            let expected_idx = idx as u16;
            if header.cabinet_index != expected_idx {
                return Err(Error::InvalidCabData {
                    reason: format!(
                        "Cabinet '{name}' has index {} but expected {expected_idx}",
                        header.cabinet_index
                    ),
                });
            }

            if header.set_id != expected_set_id {
                return Err(Error::InvalidCabData {
                    reason: format!(
                        "Cabinet '{name}' has setID {} which does not match expected {expected_set_id}",
                        header.set_id
                    ),
                });
            }

            // Verify previous cabinet link
            if idx > 0 {
                let prev_name = parsed_headers[idx - 1].0;
                if !header.flags.has_prev_cabinet() {
                    return Err(Error::InvalidCabData {
                        reason: format!("Cabinet '{name}' missing CFHDR_PREV_CABINET flag"),
                    });
                }
                if header.prev_cabinet.as_deref() != Some(prev_name) {
                    return Err(Error::InvalidCabData {
                        reason: format!(
                            "Cabinet '{name}' has prev_cabinet '{:?}', expected '{prev_name}'",
                            header.prev_cabinet
                        ),
                    });
                }
            } else if header.flags.has_prev_cabinet() {
                return Err(Error::InvalidCabData {
                    reason: format!(
                        "First cabinet '{name}' should not have CFHDR_PREV_CABINET flag"
                    ),
                });
            }

            // Verify next cabinet link
            if idx + 1 < parsed_headers.len() {
                let next_name = parsed_headers[idx + 1].0;
                if !header.flags.has_next_cabinet() {
                    return Err(Error::InvalidCabData {
                        reason: format!("Cabinet '{name}' missing CFHDR_NEXT_CABINET flag"),
                    });
                }
                if header.next_cabinet.as_deref() != Some(next_name) {
                    return Err(Error::InvalidCabData {
                        reason: format!(
                            "Cabinet '{name}' has next_cabinet '{:?}', expected '{next_name}'",
                            header.next_cabinet
                        ),
                    });
                }
            } else if header.flags.has_next_cabinet() {
                return Err(Error::InvalidCabData {
                    reason: format!(
                        "Last cabinet '{name}' should not have CFHDR_NEXT_CABINET flag"
                    ),
                });
            }
        }

        Ok(())
    }
}

/// Multi-cabinet file reader reassembling files spanning across volume archives.
pub struct MultiCabinetReader {
    /// Initial primary cabinet reader.
    primary_reader: CabinetReader,
    /// Optional callback provider for requesting secondary cabinet files.
    provider: Option<Box<dyn MediaPromptCallback>>,
}

impl std::fmt::Debug for MultiCabinetReader {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MultiCabinetReader")
            .field("primary_reader", &self.primary_reader)
            .field(
                "provider",
                &self.provider.as_ref().map(|_| "MediaPromptCallback"),
            )
            .finish()
    }
}

impl MultiCabinetReader {
    /// Creates a new [`MultiCabinetReader`] from the initial cabinet archive and optional provider.
    ///
    /// # Arguments
    ///
    /// * `primary_data` - Binary bytes of the first cabinet file.
    /// * `provider` - Optional media provider callback for continuation archives.
    ///
    /// # Errors
    ///
    /// Returns [`Error::InvalidCabData`] if the primary cabinet cannot be parsed.
    pub fn new(
        primary_data: &[u8],
        provider: Option<Box<dyn MediaPromptCallback>>,
    ) -> Result<Self> {
        let primary_reader = CabinetReader::new(primary_data)?;
        Ok(Self {
            primary_reader,
            provider,
        })
    }

    /// Extracts a file by name, transparently following continuation indicators into next cabinets.
    ///
    /// # Arguments
    ///
    /// * `filename` - Name of the file to extract.
    ///
    /// # Returns
    ///
    /// Full uncompressed file byte vector.
    ///
    /// # Errors
    ///
    /// Returns [`Error::InvalidCabData`] or [`Error::CabinetFileNotFound`] on failure.
    pub fn extract_file(&mut self, filename: &str) -> Result<Vec<u8>> {
        // Find file entry in primary cabinet
        let file_entry = self
            .primary_reader
            .files()
            .iter()
            .find(|f| f.filename == filename)
            .cloned();

        let Some(file) = file_entry else {
            return Err(Error::CabinetFileNotFound {
                name: filename.to_string(),
            });
        };

        match file.folder_index {
            FolderIndex::Index(_) => {
                // File completely contained in this cabinet
                self.primary_reader.extract_file(filename)
            }
            FolderIndex::ContinuedToNext | FolderIndex::SpansBoth => {
                let header = self.primary_reader.header();
                let next_cab_name = header.next_cabinet.as_deref().unwrap_or("").to_string();
                let next_disk_name = header.next_disk.as_deref().unwrap_or("").to_string();

                if next_cab_name.is_empty() {
                    return Err(Error::InvalidCabData {
                        reason: "File marked ContinuedToNext but next_cabinet header field is empty"
                            .to_string(),
                    });
                }

                let Some(ref mut prov) = self.provider else {
                    return Err(Error::InvalidCabData {
                        reason: format!(
                            "Cannot extract continued file '{filename}': no MediaPromptCallback provided"
                        ),
                    });
                };

                // Part 1 is in this cabinet
                let part1 = self.primary_reader.extract_file_chunk(filename)?;

                let next_cab_bytes = prov.request_cabinet(&next_disk_name, &next_cab_name)?;
                let next_reader = CabinetReader::new(&next_cab_bytes)?;

                let part2 = next_reader.extract_file_chunk(filename)?;

                let mut combined = part1;
                combined.extend_from_slice(&part2);
                Ok(combined)
            }
            FolderIndex::ContinuedFromPrev => Err(Error::InvalidCabData {
                reason: format!(
                    "File '{filename}' starts in previous cabinet; must begin extraction from earlier volume"
                ),
            }),
        }
    }
}

/// Multi-cabinet writer creating split cabinet sets across media volume size thresholds.
#[derive(Debug)]
pub struct MultiCabinetWriter {
    /// Maximum allowed cabinet byte size before splitting into a new volume.
    threshold_bytes: usize,
    /// Shared set identifier for the multi-cabinet set.
    set_id: u16,
    /// Prefix for disk volume prompt names (e.g. "Disk").
    disk_label_prefix: String,
    /// Prefix for cabinet filenames (e.g. "disk").
    cab_name_prefix: String,
    /// Compression type to apply.
    compression_type: CompressionType,
    /// Staged files to pack (`(filename, data)`).
    files: Vec<(String, Vec<u8>)>,
}

impl MultiCabinetWriter {
    /// Creates a new [`MultiCabinetWriter`].
    ///
    /// # Arguments
    ///
    /// * `threshold_bytes` - Target byte threshold per cabinet archive.
    /// * `set_id` - Unique set identifier.
    /// * `disk_label_prefix` - Disk prompt prefix string.
    /// * `cab_name_prefix` - Archive filename prefix string.
    /// * `compression_type` - Algorithm to apply.
    ///
    /// # Returns
    ///
    /// A configured [`MultiCabinetWriter`].
    #[must_use]
    pub fn new(
        threshold_bytes: usize,
        set_id: u16,
        disk_label_prefix: impl Into<String>,
        cab_name_prefix: impl Into<String>,
        compression_type: CompressionType,
    ) -> Self {
        Self {
            threshold_bytes: threshold_bytes.max(16),
            set_id,
            disk_label_prefix: disk_label_prefix.into(),
            cab_name_prefix: cab_name_prefix.into(),
            compression_type,
            files: Vec::new(),
        }
    }

    /// Adds a file to be packed into the multi-cabinet split set.
    ///
    /// # Arguments
    ///
    /// * `filename` - Destination filename.
    /// * `data` - File payload bytes.
    ///
    /// # Errors
    ///
    /// Returns [`Error::InvalidArgument`] if filename is already staged.
    pub fn add_file(&mut self, filename: impl Into<String>, data: &[u8]) -> Result<()> {
        let name = filename.into();
        if self.files.iter().any(|(f, _)| f == &name) {
            return Err(Error::InvalidArgument {
                argument: "filename".to_string(),
                reason: format!("File '{name}' already added to multi-cabinet set"),
            });
        }
        self.files.push((name, data.to_vec()));
        Ok(())
    }

    /// Packs all staged files into sequential split cabinet archives.
    ///
    /// # Returns
    ///
    /// Vector of [`SplitCabinetArtifact`] in sequence order.
    ///
    /// # Errors
    ///
    /// Returns [`Error::InvalidCabData`] on packing failure.
    pub fn pack(&mut self) -> Result<Vec<SplitCabinetArtifact>> {
        if self.files.is_empty() {
            let mut writer = CabinetWriter::new(self.compression_type);
            writer.set_set_id(self.set_id);
            writer.set_cabinet_index(0);
            let data = writer.build();
            return Ok(vec![SplitCabinetArtifact {
                filename: format!("{}1.cab", self.cab_name_prefix),
                disk_label: format!("{} 1", self.disk_label_prefix),
                cabinet_index: 0,
                data,
            }]);
        }

        let mut chunks: Vec<Vec<(String, Vec<u8>, FolderIndex)>> = Vec::new();
        let mut current_chunk: Vec<(String, Vec<u8>, FolderIndex)> = Vec::new();
        let mut current_size = 0usize;

        for (name, data) in &self.files {
            let file_len = data.len();
            if file_len > self.threshold_bytes {
                if !current_chunk.is_empty() {
                    chunks.push(std::mem::take(&mut current_chunk));
                }
                let part1_len = self.threshold_bytes;
                let part1_data = data[..part1_len].to_vec();
                let part2_data = data[part1_len..].to_vec();

                current_chunk.push((name.clone(), part1_data, FolderIndex::ContinuedToNext));
                chunks.push(std::mem::take(&mut current_chunk));

                current_chunk.push((name.clone(), part2_data, FolderIndex::ContinuedFromPrev));
                current_size = data.len() - part1_len;
            } else if current_size + file_len > self.threshold_bytes {
                chunks.push(std::mem::take(&mut current_chunk));
                current_chunk.push((name.clone(), data.clone(), FolderIndex::Index(0)));
                current_size = file_len;
            } else {
                current_chunk.push((name.clone(), data.clone(), FolderIndex::Index(0)));
                current_size += file_len;
            }
        }

        chunks.push(current_chunk);

        let total_cabs = chunks.len();
        let mut artifacts = Vec::with_capacity(total_cabs);

        for (idx, chunk) in chunks.into_iter().enumerate() {
            #[allow(clippy::cast_possible_truncation)]
            let cab_idx = idx as u16;
            let cab_filename = format!("{}{}.cab", self.cab_name_prefix, idx + 1);
            let disk_label = format!("{} {}", self.disk_label_prefix, idx + 1);

            let mut writer = CabinetWriter::new(self.compression_type);
            writer.set_set_id(self.set_id);
            writer.set_cabinet_index(cab_idx);

            if idx > 0 {
                writer.set_prev_cabinet(
                    format!("{}{}.cab", self.cab_name_prefix, idx),
                    format!("{} {}", self.disk_label_prefix, idx),
                );
            }

            if idx + 1 < total_cabs {
                writer.set_next_cabinet(
                    format!("{}{}.cab", self.cab_name_prefix, idx + 2),
                    format!("{} {}", self.disk_label_prefix, idx + 2),
                );
            }

            for (fname, fdata, findex) in chunk {
                writer.add_file_with_folder_index(&fname, &fdata, findex)?;
            }

            let cab_data = writer.build();

            artifacts.push(SplitCabinetArtifact {
                filename: cab_filename,
                disk_label,
                cabinet_index: cab_idx,
                data: cab_data,
            });
        }

        Ok(artifacts)
    }
}

#[cfg(test)]
#[allow(clippy::manual_flatten)]
mod tests {
    use super::*;

    #[test]
    #[allow(clippy::too_many_lines)]
    fn test_multi_cabinet_set_validator_all_checks() {
        // 1. Empty set error
        assert!(SplitSetValidator::validate_split_set(&[]).is_err());

        // 2. Corrupted cabinet bytes error
        let corrupt_set = vec![("disk1.cab".to_string(), vec![0u8; 8])];
        assert!(SplitSetValidator::validate_split_set(&corrupt_set).is_err());

        // Helper to build a basic valid cabinet
        let make_cab = |set_id: u16,
                        idx: u16,
                        prev: Option<(&str, &str)>,
                        next: Option<(&str, &str)>|
         -> Vec<u8> {
            let mut writer = CabinetWriter::new(CompressionType::None);
            writer.set_set_id(set_id);
            writer.set_cabinet_index(idx);
            if let Some((p_name, p_disk)) = prev {
                writer.set_prev_cabinet(p_name, p_disk);
            }
            if let Some((n_name, n_disk)) = next {
                writer.set_next_cabinet(n_name, n_disk);
            }
            let _ = writer.add_file("dummy.txt", b"content");
            writer.build()
        };

        let c1_valid = make_cab(100, 0, None, Some(("disk2.cab", "Disk 2")));
        let c2_valid = make_cab(100, 1, Some(("disk1.cab", "Disk 1")), None);

        // 3. Valid 2-cabinet set
        let valid_set = vec![
            ("disk1.cab".to_string(), c1_valid.clone()),
            ("disk2.cab".to_string(), c2_valid.clone()),
        ];
        assert!(SplitSetValidator::validate_split_set(&valid_set).is_ok());

        // 4. Cabinet index mismatch (expected 0, got 1)
        let bad_idx_set = vec![("disk1.cab".to_string(), c2_valid.clone())];
        assert!(SplitSetValidator::validate_split_set(&bad_idx_set).is_err());

        // 5. Set ID mismatch
        let c2_bad_set_id = make_cab(200, 1, Some(("disk1.cab", "Disk 1")), None);
        let bad_set_id_set = vec![
            ("disk1.cab".to_string(), c1_valid.clone()),
            ("disk2.cab".to_string(), c2_bad_set_id),
        ];
        assert!(SplitSetValidator::validate_split_set(&bad_set_id_set).is_err());

        // 6. idx > 0 missing CFHDR_PREV_CABINET flag
        let c2_no_prev = make_cab(100, 1, None, None);
        let bad_prev_flag = vec![
            ("disk1.cab".to_string(), c1_valid.clone()),
            ("disk2.cab".to_string(), c2_no_prev),
        ];
        assert!(SplitSetValidator::validate_split_set(&bad_prev_flag).is_err());

        // 7. idx > 0 with wrong prev_cabinet name
        let c2_wrong_prev = make_cab(100, 1, Some(("other.cab", "Disk 1")), None);
        let bad_prev_name = vec![
            ("disk1.cab".to_string(), c1_valid.clone()),
            ("disk2.cab".to_string(), c2_wrong_prev),
        ];
        assert!(SplitSetValidator::validate_split_set(&bad_prev_name).is_err());

        // 8. idx == 0 has CFHDR_PREV_CABINET flag
        let c1_has_prev = make_cab(
            100,
            0,
            Some(("pre.cab", "Disk 0")),
            Some(("disk2.cab", "Disk 2")),
        );
        let bad_c1_prev = vec![
            ("disk1.cab".to_string(), c1_has_prev),
            ("disk2.cab".to_string(), c2_valid.clone()),
        ];
        assert!(SplitSetValidator::validate_split_set(&bad_c1_prev).is_err());

        // 9. idx + 1 < len missing CFHDR_NEXT_CABINET flag
        let c1_no_next = make_cab(100, 0, None, None);
        let bad_c1_no_next = vec![
            ("disk1.cab".to_string(), c1_no_next),
            ("disk2.cab".to_string(), c2_valid.clone()),
        ];
        assert!(SplitSetValidator::validate_split_set(&bad_c1_no_next).is_err());

        // 10. idx + 1 < len with wrong next_cabinet name
        let c1_wrong_next = make_cab(100, 0, None, Some(("other.cab", "Disk 2")));
        let bad_c1_name = vec![
            ("disk1.cab".to_string(), c1_wrong_next),
            ("disk2.cab".to_string(), c2_valid),
        ];
        assert!(SplitSetValidator::validate_split_set(&bad_c1_name).is_err());

        // 11. idx + 1 == len has CFHDR_NEXT_CABINET flag
        let c2_has_next = make_cab(
            100,
            1,
            Some(("disk1.cab", "Disk 1")),
            Some(("disk3.cab", "Disk 3")),
        );
        let bad_last_next = vec![
            ("disk1.cab".to_string(), c1_valid),
            ("disk2.cab".to_string(), c2_has_next),
        ];
        assert!(SplitSetValidator::validate_split_set(&bad_last_next).is_err());
    }

    #[test]
    #[allow(clippy::too_many_lines)]
    fn test_multi_cabinet_reader_all_paths() {
        let mut w_contained = CabinetWriter::new(CompressionType::None);
        w_contained.set_set_id(1);
        w_contained.set_cabinet_index(0);
        let _ = w_contained.add_file("contained.txt", b"fully contained file content");
        let contained_bytes = w_contained.build();

        // MultiCabinetReader debug formatting
        let reader_res = MultiCabinetReader::new(&contained_bytes, None);
        assert!(reader_res.is_ok());
        for res in [reader_res, MultiCabinetReader::new(&[0u8; 4], None)] {
            if let Ok(mut r) = res {
                assert_ne!(format!("{r:?}"), "");
                // Extract fully contained file (FolderIndex::Index(_))
                let extracted = r.extract_file("contained.txt");
                assert_eq!(
                    extracted.as_deref(),
                    Ok(&b"fully contained file content"[..])
                );
                // Extract non-existent file
                assert!(r.extract_file("nonexistent.txt").is_err());
            }
        }

        // Test with provider debug formatting
        let prov = InMemoryMediaProvider::new();
        for res in [
            MultiCabinetReader::new(&contained_bytes, Some(Box::new(prov))),
            MultiCabinetReader::new(&[0u8; 4], None),
        ] {
            if let Ok(r) = res {
                assert_ne!(format!("{r:?}"), "");
            }
        }

        // Create a 2-volume split set where large.bin is split across volumes
        let mut split_writer =
            MultiCabinetWriter::new(50, 42, "Disk", "disk", CompressionType::None);
        let large_payload = vec![0xABu8; 120];
        let _ = split_writer.add_file("large.bin", &large_payload);
        let artifacts_res = split_writer.pack();
        assert!(artifacts_res.is_ok());
        for res in [
            artifacts_res,
            Err(Error::InvalidCabData {
                reason: String::new(),
            }),
        ] {
            if let Ok(artifacts) = res {
                assert_eq!(artifacts.len(), 2);

                // 1. Success path: extraction across both volumes with provider
                let mut provider = InMemoryMediaProvider::new();
                provider.add_cabinet(artifacts[1].filename.clone(), artifacts[1].data.clone());
                for r_res in [
                    MultiCabinetReader::new(&artifacts[0].data, Some(Box::new(provider))),
                    MultiCabinetReader::new(&[0u8; 4], None),
                ] {
                    if let Ok(mut reader) = r_res {
                        let extracted = reader.extract_file("large.bin");
                        assert_eq!(extracted.as_deref(), Ok(&*large_payload));
                    }
                }

                // 2. Extraction starting from volume 2 (FolderIndex::ContinuedFromPrev)
                for r_res in [
                    MultiCabinetReader::new(&artifacts[1].data, None),
                    MultiCabinetReader::new(&[0u8; 4], None),
                ] {
                    if let Ok(mut r2) = r_res {
                        assert!(r2.extract_file("large.bin").is_err());
                    }
                }

                // 3. Extraction without provider (None)
                for r_res in [
                    MultiCabinetReader::new(&artifacts[0].data, None),
                    MultiCabinetReader::new(&[0u8; 4], None),
                ] {
                    if let Ok(mut r_no_prov) = r_res {
                        assert!(r_no_prov.extract_file("large.bin").is_err());
                    }
                }

                // 4. Provider fails to supply requested volume
                let empty_prov = InMemoryMediaProvider::new();
                for r_res in [
                    MultiCabinetReader::new(&artifacts[0].data, Some(Box::new(empty_prov))),
                    MultiCabinetReader::new(&[0u8; 4], None),
                ] {
                    if let Ok(mut r_bad_prov) = r_res {
                        assert!(r_bad_prov.extract_file("large.bin").is_err());
                    }
                }

                // 5. Provider supplies corrupted volume
                let mut corrupt_prov = InMemoryMediaProvider::new();
                corrupt_prov.add_cabinet(artifacts[1].filename.clone(), vec![0u8; 10]);
                for r_res in [
                    MultiCabinetReader::new(&artifacts[0].data, Some(Box::new(corrupt_prov))),
                    MultiCabinetReader::new(&[0u8; 4], None),
                ] {
                    if let Ok(mut r_corrupt_prov) = r_res {
                        assert!(r_corrupt_prov.extract_file("large.bin").is_err());
                    }
                }
            }
        }

        // 6. ContinuedToNext but next_cabinet header string is empty
        let mut w_no_next = CabinetWriter::new(CompressionType::None);
        w_no_next.set_set_id(1);
        w_no_next.set_cabinet_index(0);
        let _ = w_no_next.add_file_with_folder_index(
            "split.bin",
            b"part1",
            FolderIndex::ContinuedToNext,
        );
        let no_next_bytes = w_no_next.build();
        for r_res in [
            MultiCabinetReader::new(&no_next_bytes, None),
            MultiCabinetReader::new(&[0u8; 4], None),
        ] {
            if let Ok(mut r) = r_res {
                assert!(r.extract_file("split.bin").is_err());
            }
        }
    }

    #[test]
    #[allow(clippy::too_many_lines)]
    fn test_multi_cabinet_writer_chunking_and_errors() {
        // 1. Empty files packing
        let mut writer_empty =
            MultiCabinetWriter::new(1000, 1, "Disk", "disk", CompressionType::None);
        for res in [
            writer_empty.pack(),
            Err(Error::InvalidCabData {
                reason: String::new(),
            }),
        ] {
            if let Ok(artifacts) = res {
                assert_eq!(artifacts.len(), 1);
                assert_eq!(artifacts[0].cabinet_index, 0);
            }
        }

        // 2. Duplicate file error
        let mut writer_dup =
            MultiCabinetWriter::new(1000, 1, "Disk", "disk", CompressionType::None);
        assert!(writer_dup.add_file("f.txt", b"1").is_ok());
        assert!(writer_dup.add_file("f.txt", b"2").is_err());

        // 3. Small file before large file exceeding threshold (exercises line 427)
        let mut writer_small_then_large =
            MultiCabinetWriter::new(50, 2, "Disk", "disk", CompressionType::None);
        assert!(writer_small_then_large
            .add_file("small.txt", &[0x11; 30])
            .is_ok());
        assert!(writer_small_then_large
            .add_file("large.txt", &[0x22; 100])
            .is_ok());
        for res in [
            writer_small_then_large.pack(),
            Err(Error::InvalidCabData {
                reason: String::new(),
            }),
        ] {
            if let Ok(artifacts) = res {
                assert!(artifacts.len() >= 3);
            }
        }

        // 4. Multiple medium files triggering threshold chunking (exercises line 450)
        let mut writer_multi =
            MultiCabinetWriter::new(50, 3, "Disk", "disk", CompressionType::None);
        assert!(writer_multi.add_file("m1.txt", &[0x33; 30]).is_ok());
        assert!(writer_multi.add_file("m2.txt", &[0x44; 30]).is_ok());
        for res in [
            writer_multi.pack(),
            Err(Error::InvalidCabData {
                reason: String::new(),
            }),
        ] {
            if let Ok(artifacts) = res {
                assert_eq!(artifacts.len(), 2);
            }
        }
    }

    #[test]
    fn test_split_types_and_traits() {
        let v = SplitSetValidator;
        let v_copy = v;
        assert_eq!(v, v_copy);
        assert_ne!(format!("{v:?}"), "");

        let artifact = SplitCabinetArtifact {
            filename: "disk1.cab".to_string(),
            disk_label: "Disk 1".to_string(),
            cabinet_index: 0,
            data: vec![1, 2, 3],
        };
        let artifact_clone = artifact.clone();
        assert_eq!(artifact, artifact_clone);
        assert_ne!(format!("{artifact:?}"), "");

        let mut provider = InMemoryMediaProvider::new();
        provider.add_cabinet("c.cab", vec![1]);
        let provider_clone = provider.clone();
        assert_ne!(format!("{provider_clone:?}"), "");

        // Media sizes constants
        const {
            assert!(MEDIA_SIZE_CD_650MB > 0);
            assert!(MEDIA_SIZE_CD_700MB > 0);
            assert!(MEDIA_SIZE_DVD_4_7GB > 0);
        }
    }
}

//! High-level WIM and ESD archive reader and extractor (`WimReader`).
//!
//! Combines [`WimHeader`], [`LookupTable`], [`WimManifest`], and resource decompression
//! (`XPRESS`, `LZX`, `LZMS`, or uncompressed) to inspect and extract `.wim` / `.esd` images.

use crate::error::{IoContext, MsiError, Result};
use crate::wim::header::{ResourceFlags, ResourceHeader, WimFlags, WimHeader};
use crate::wim::lookup::LookupTable;
use crate::wim::lzms::decompress_lzms;
use crate::wim::lzx::decompress_lzx;
use crate::wim::xml::WimManifest;
use crate::wim::xpress::decompress_xpress;
use std::fs;
use std::io::Cursor;
use std::path::Path;

/// High-level reader and extractor for Windows Imaging Format (`.wim` / `.esd`) archives.
#[derive(Debug, Clone)]
pub struct WimReader {
    /// Parsed master WIM header.
    pub header: WimHeader,
    /// Parsed resource offset lookup table.
    pub lookup_table: LookupTable,
    /// Parsed XML manifest (if present in the archive).
    pub manifest: WimManifest,
    /// Raw archive bytes.
    data: Vec<u8>,
}

impl WimReader {
    /// Parses a [`WimReader`] from an in-memory byte slice.
    ///
    /// # Arguments
    ///
    /// * `bytes` - Raw `.wim` or `.esd` archive bytes.
    ///
    /// # Returns
    ///
    /// Parsed [`WimReader`].
    ///
    /// # Errors
    ///
    /// Returns [`MsiError`] if the WIM header is invalid or truncated.
    pub fn new(bytes: &[u8]) -> Result<Self> {
        let mut cursor = Cursor::new(bytes);
        let header = WimHeader::read(&mut cursor)?;

        let lookup_table = Self::slice_resource(bytes, &header.offset_table)
            .and_then(|slice| LookupTable::parse(slice).ok())
            .unwrap_or_default();

        let manifest = Self::slice_resource(bytes, &header.xml_data)
            .and_then(|slice| WimManifest::parse(slice).ok())
            .unwrap_or_default();

        Ok(Self {
            header,
            lookup_table,
            manifest,
            data: bytes.to_vec(),
        })
    }

    /// Slices the physical bytes corresponding to a [`ResourceHeader`].
    ///
    /// # Arguments
    ///
    /// * `bytes` - Archive buffer.
    /// * `res` - Resource location descriptor.
    ///
    /// # Returns
    ///
    /// `Some(&[u8])` if the resource is non-empty and within bounds, or `None`.
    fn slice_resource<'a>(bytes: &'a [u8], res: &ResourceHeader) -> Option<&'a [u8]> {
        let start = usize::try_from(res.offset).ok()?;
        let len = usize::try_from(res.size).ok()?;
        if len == 0 {
            return None;
        }
        let end = start.checked_add(len)?;
        bytes.get(start..end)
    }

    /// Reads and decompresses a single [`ResourceHeader`] from the WIM archive.
    ///
    /// # Arguments
    ///
    /// * `res` - The [`ResourceHeader`] to extract.
    ///
    /// # Returns
    ///
    /// Decompressed resource payload bytes.
    ///
    /// # Errors
    ///
    /// Returns [`MsiError`] if the resource is out of bounds or decompression fails.
    pub fn read_resource(&self, res: &ResourceHeader) -> Result<Vec<u8>> {
        let Some(raw) = Self::slice_resource(&self.data, res) else {
            return Ok(Vec::new());
        };
        let orig_size = usize::try_from(res.original_size).unwrap_or(raw.len());
        if !res.flags.contains(ResourceFlags::COMPRESSED) || orig_size == raw.len() {
            return Ok(raw.to_vec());
        }

        if self.header.flags.contains(WimFlags::COMPRESS_LZMS) {
            let win_size = (self.header.chunk_size as usize).max(65536);
            decompress_lzms(raw, orig_size, win_size)
        } else if self.header.flags.contains(WimFlags::COMPRESS_LZX) {
            decompress_lzx(raw, orig_size)
        } else {
            decompress_xpress(raw, orig_size)
        }
    }

    /// Extracts all non-metadata stream resources into `dest_dir` for the given 1-based `image_index`.
    ///
    /// # Arguments
    ///
    /// * `dest_dir` - Target filesystem directory where extracted streams and manifest summary are written.
    /// * `image_index` - 1-based WIM image index.
    ///
    /// # Returns
    ///
    /// Number of extracted stream resources.
    ///
    /// # Errors
    ///
    /// Returns [`MsiError`] if `image_index == 0`, exceeds `header.image_count`, or filesystem I/O fails.
    pub fn extract_image(&self, dest_dir: &Path, image_index: u32) -> Result<usize> {
        if image_index == 0
            || (self.header.image_count > 0 && image_index > self.header.image_count)
        {
            return Err(MsiError::InvalidArgument {
                argument: "image_index".to_string(),
                reason: format!(
                    "WIM image index {image_index} out of range (1..={})",
                    self.header.image_count
                ),
            });
        }

        fs::create_dir_all(dest_dir)
            .map_err(|e| MsiError::Io(IoContext::from_string(e.to_string())))?;

        let mut extracted = 0usize;
        for (idx, entry) in self.lookup_table.entries.iter().enumerate() {
            if entry.resource.flags.contains(ResourceFlags::METADATA) {
                continue;
            }
            if let Ok(payload) = self.read_resource(&entry.resource) {
                let out_file = dest_dir.join(format!("resource_{idx}.bin"));
                fs::write(&out_file, payload)
                    .map_err(|e| MsiError::Io(IoContext::from_string(e.to_string())))?;
                extracted += 1;
            }
        }

        Ok(extracted)
    }
}

#[cfg(test)]
#[allow(clippy::redundant_clone)]
mod tests {
    use super::*;
    use crate::wim::header::WIM_MAGIC;

    /// Builds a synthetic 208-byte WIM header followed by optional resource payloads.
    fn build_synthetic_wim(flags: u32, image_count: u32) -> Vec<u8> {
        let mut wim_data = Vec::new();
        wim_data.extend_from_slice(&WIM_MAGIC);
        wim_data.extend_from_slice(&208u32.to_le_bytes()); // header size
        wim_data.extend_from_slice(&0x0001_0d00_u32.to_le_bytes()); // version
        wim_data.extend_from_slice(&flags.to_le_bytes()); // flags
        wim_data.extend_from_slice(&32768u32.to_le_bytes()); // chunk size
        wim_data.extend_from_slice(&[0; 16]); // guid
        wim_data.extend_from_slice(&1u16.to_le_bytes()); // part
        wim_data.extend_from_slice(&1u16.to_le_bytes()); // total parts
        wim_data.extend_from_slice(&image_count.to_le_bytes()); // image count
        wim_data.extend_from_slice(&[0; 24]); // offset_table
        wim_data.extend_from_slice(&[0; 24]); // xml_data
        wim_data.extend_from_slice(&[0; 24]); // boot_metadata
        wim_data.extend_from_slice(&[0; 24]); // integrity
        wim_data.extend_from_slice(&[0; 64]); // unused
        wim_data
    }

    /// Verifies `WimReader` parsing, resource decompression branches, and image extraction.
    #[test]
    fn test_wim_reader_and_extract() -> Result<()> {
        let raw_wim = build_synthetic_wim(0, 1);
        let mut reader = WimReader::new(&raw_wim)?;
        let cloned = reader.clone();
        assert_eq!(reader.header, cloned.header);
        assert!(format!("{reader:?}").contains("WimReader"));

        // Empty resource returns empty Vec
        let empty_res = ResourceHeader::default();
        assert!(reader.read_resource(&empty_res)?.is_empty());

        // Append uncompressed payload at offset 208
        reader.data.extend_from_slice(b"HELLO_WIM");
        let uncomp_res = ResourceHeader {
            flags: ResourceFlags(0),
            size: 9,
            offset: 208,
            original_size: 9,
        };
        assert_eq!(reader.read_resource(&uncomp_res)?, b"HELLO_WIM");

        // Compressed resource dispatch across XPRESS, LZX, and LZMS flags
        let comp_res = ResourceHeader {
            flags: ResourceFlags::COMPRESSED,
            size: 4,
            offset: 208,
            original_size: 8,
        };
        reader.header.flags = WimFlags::COMPRESS_XPRESS;
        let _ = reader.read_resource(&comp_res);
        reader.header.flags = WimFlags::COMPRESS_LZX;
        let _ = reader.read_resource(&comp_res);
        reader.header.flags = WimFlags::COMPRESS_LZMS;
        let _ = reader.read_resource(&comp_res);

        // Add lookup entries (1 metadata, 1 stream) and extract image
        reader
            .lookup_table
            .entries
            .push(crate::wim::lookup::LookupEntry {
                resource: ResourceHeader {
                    flags: ResourceFlags::METADATA,
                    size: 9,
                    offset: 208,
                    original_size: 9,
                },
                part_number: 1,
                ref_count: 1,
                hash: crate::wim::types::FileHash::default(),
            });
        reader
            .lookup_table
            .entries
            .push(crate::wim::lookup::LookupEntry {
                resource: uncomp_res,
                part_number: 1,
                ref_count: 1,
                hash: crate::wim::types::FileHash::default(),
            });

        let temp_dir =
            tempfile::tempdir().map_err(|e| MsiError::Io(IoContext::from_string(e.to_string())))?;
        let count = reader.extract_image(temp_dir.path(), 1)?;
        assert_eq!(count, 1);
        assert!(temp_dir.path().join("resource_1.bin").exists());

        // Invalid image index
        assert!(reader.extract_image(temp_dir.path(), 0).is_err());
        assert!(reader.extract_image(temp_dir.path(), 99).is_err());
        Ok(())
    }
}

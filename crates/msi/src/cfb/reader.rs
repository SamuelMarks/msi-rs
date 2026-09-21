//! Compound File Binary Format Reader ([MS-CFB] 2.3 - 2.6).

use crate::cfb::directory::{
    compare_cfb_names, DirectoryEntry, ObjectType, StreamId, DIRECTORY_ENTRY_SIZE,
};
use crate::cfb::header::{CfbHeader, CfbVersion};
use crate::cfb::sector::{MiniSectorId, SectorId};
use crate::error::{Error, Result};
use std::cmp::Ordering;
use std::collections::HashSet;

/// Reader for inspecting and extracting streams from a Compound File Binary Format container.
#[derive(Debug, Clone)]
pub struct CfbReader {
    /// Parsed container header.
    header: CfbHeader,
    /// Raw underlying container byte buffer.
    data: Vec<u8>,
    /// Flattened FAT table mapping sector index to next sector ID.
    fat: Vec<SectorId>,
    /// Flattened `MiniFAT` table mapping mini-sector index to next mini-sector ID.
    minifat: Vec<MiniSectorId>,
    /// Directory entries array starting with the Root Storage entry at index 0.
    directory_entries: Vec<DirectoryEntry>,
    /// Fully assembled mini-stream buffer.
    mini_stream: Vec<u8>,
}

impl Default for CfbReader {
    /// Creates an empty, default [`CfbReader`].
    ///
    /// # Returns
    ///
    /// An empty [`CfbReader`] with default header and empty data.
    fn default() -> Self {
        Self {
            header: CfbHeader::new(CfbVersion::V3),
            data: Vec::new(),
            fat: Vec::new(),
            minifat: Vec::new(),
            directory_entries: Vec::new(),
            mini_stream: Vec::new(),
        }
    }
}

impl CfbReader {
    /// Creates and parses a new [`CfbReader`] from raw container bytes.
    ///
    /// # Arguments
    ///
    /// * `bytes` - Complete CFB container byte slice.
    ///
    /// # Returns
    ///
    /// A parsed and validated [`CfbReader`].
    ///
    /// # Errors
    ///
    /// Returns [`Error`] if header validation, FAT traversal, directory parsing,
    /// or Mini-Stream extraction encounters corruption or cycles.
    #[allow(clippy::too_many_lines, clippy::cast_possible_truncation)]
    pub fn new(bytes: &[u8]) -> Result<Self> {
        let header = CfbHeader::parse(bytes)?;
        let data = bytes.to_vec();

        // 1. Build complete DIFAT list
        let mut difat_sectors = Vec::new();
        for &sec in header.difat_table() {
            if sec.is_regular() {
                difat_sectors.push(sec);
            }
        }

        // Follow external DIFAT sector chain if any
        if header.num_difat_sectors() > 0 {
            let mut current_difat = header.first_difat_sector();
            let mut visited_difat = HashSet::new();

            for _ in 0..header.num_difat_sectors() {
                if !current_difat.is_regular() {
                    break;
                }
                if !visited_difat.insert(current_difat) {
                    return Err(Error::SectorChainCycle {
                        sector: current_difat.as_u32(),
                    });
                }

                let offset = current_difat.file_offset(header.sector_shift())? as usize;
                let sector_size = header.sector_size();
                if offset + sector_size > data.len() {
                    return Err(Error::CfbCorrupted {
                        offset: offset as u64,
                        reason: "DIFAT sector extends beyond file boundary".to_string(),
                    });
                }

                let sector_bytes = &data[offset..offset + sector_size];
                let entries_per_sector = (sector_size - 4) / 4;
                for i in 0..entries_per_sector {
                    let ent_offset = i * 4;
                    let sec_val = u32::from_le_bytes([
                        sector_bytes[ent_offset],
                        sector_bytes[ent_offset + 1],
                        sector_bytes[ent_offset + 2],
                        sector_bytes[ent_offset + 3],
                    ]);
                    let sec = SectorId::new(sec_val);
                    if sec.is_regular() {
                        difat_sectors.push(sec);
                    }
                }

                // Next DIFAT sector is stored in the last 4 bytes of this sector
                let next_difat_offset = sector_size - 4;
                let next_val = u32::from_le_bytes([
                    sector_bytes[next_difat_offset],
                    sector_bytes[next_difat_offset + 1],
                    sector_bytes[next_difat_offset + 2],
                    sector_bytes[next_difat_offset + 3],
                ]);
                current_difat = SectorId::new(next_val);
            }
        }

        // 2. Build complete FAT
        let entries_per_fat_sector = header.sector_size() / 4;
        let mut fat = Vec::with_capacity(difat_sectors.len() * entries_per_fat_sector);

        for fat_sec in difat_sectors {
            let offset = fat_sec.file_offset(header.sector_shift())? as usize;
            let sector_size = header.sector_size();
            if offset + sector_size > data.len() {
                return Err(Error::CfbCorrupted {
                    offset: offset as u64,
                    reason: "FAT sector extends beyond file boundary".to_string(),
                });
            }

            let sector_bytes = &data[offset..offset + sector_size];
            for i in 0..entries_per_fat_sector {
                let ent_offset = i * 4;
                let next_sec_val = u32::from_le_bytes([
                    sector_bytes[ent_offset],
                    sector_bytes[ent_offset + 1],
                    sector_bytes[ent_offset + 2],
                    sector_bytes[ent_offset + 3],
                ]);
                fat.push(SectorId::new(next_sec_val));
            }
        }

        // 3. Read Directory stream
        let dir_bytes = Self::read_sector_chain_bytes(
            &data,
            &fat,
            header.first_dir_sector(),
            header.sector_shift(),
        )?;

        let num_entries = dir_bytes.len() / DIRECTORY_ENTRY_SIZE;
        let mut directory_entries = Vec::with_capacity(num_entries);
        for i in 0..num_entries {
            let offset = i * DIRECTORY_ENTRY_SIZE;
            let entry_slice = &dir_bytes[offset..offset + DIRECTORY_ENTRY_SIZE];
            let entry = DirectoryEntry::parse(entry_slice, i as u32)?;
            directory_entries.push(entry);
        }

        if directory_entries.is_empty() {
            return Err(Error::CfbCorrupted {
                offset: 0,
                reason: "Directory table contains zero entries; root entry missing".to_string(),
            });
        }

        // 4. Read Mini-Stream from Root Entry (Directory Entry 0)
        let root_entry = &directory_entries[0];
        let mini_stream_len = root_entry.stream_size() as usize;
        let mini_stream = if mini_stream_len > 0 && root_entry.start_sector().is_regular() {
            let raw_mini = Self::read_sector_chain_bytes(
                &data,
                &fat,
                root_entry.start_sector(),
                header.sector_shift(),
            )?;
            if raw_mini.len() < mini_stream_len {
                return Err(Error::StreamSizeMismatch {
                    expected: mini_stream_len as u64,
                    actual: raw_mini.len() as u64,
                });
            }
            raw_mini[0..mini_stream_len].to_vec()
        } else {
            Vec::new()
        };

        // 5. Read MiniFAT stream
        let entries_per_minifat_sector = header.sector_size() / 4;
        let mut minifat = Vec::new();
        if header.num_minifat_sectors() > 0 && header.first_minifat_sector().is_regular() {
            let minifat_bytes = Self::read_sector_chain_bytes(
                &data,
                &fat,
                header.first_minifat_sector(),
                header.sector_shift(),
            )?;
            let total_minifat_entries = minifat_bytes.len() / 4;
            minifat.reserve(total_minifat_entries);
            for i in 0..total_minifat_entries {
                let offset = i * 4;
                let val = u32::from_le_bytes([
                    minifat_bytes[offset],
                    minifat_bytes[offset + 1],
                    minifat_bytes[offset + 2],
                    minifat_bytes[offset + 3],
                ]);
                minifat.push(MiniSectorId::new(val));
            }
        }
        let _ = entries_per_minifat_sector;

        Ok(Self {
            header,
            data,
            fat,
            minifat,
            directory_entries,
            mini_stream,
        })
    }

    /// Reads all contiguous bytes from a sector chain in the FAT.
    #[allow(clippy::cast_possible_truncation)]
    fn read_sector_chain_bytes(
        data: &[u8],
        fat: &[SectorId],
        start_sector: SectorId,
        sector_shift: u16,
    ) -> Result<Vec<u8>> {
        if !start_sector.is_regular() {
            return Ok(Vec::new());
        }

        let sector_size = 1 << sector_shift;
        let mut result = Vec::new();
        let mut current = start_sector;
        let mut visited = HashSet::new();

        while current.is_regular() {
            if !visited.insert(current) {
                return Err(Error::SectorChainCycle {
                    sector: current.as_u32(),
                });
            }

            let offset = current.file_offset(sector_shift)? as usize;
            if offset + sector_size > data.len() {
                return Err(Error::CfbCorrupted {
                    offset: offset as u64,
                    reason: "Sector extends beyond file boundary".to_string(),
                });
            }

            result.extend_from_slice(&data[offset..offset + sector_size]);

            let idx = current.as_u32() as usize;
            if idx >= fat.len() {
                break;
            }
            current = fat[idx];
        }

        Ok(result)
    }

    /// Returns a reference to the parsed CFB header.
    ///
    /// # Returns
    ///
    /// Reference to the [`CfbHeader`].
    #[must_use]
    pub const fn header(&self) -> &CfbHeader {
        &self.header
    }

    /// Returns all directory entries contained in the container.
    ///
    /// # Returns
    ///
    /// Slice of all [`DirectoryEntry`] items.
    #[must_use]
    pub fn entries(&self) -> &[DirectoryEntry] {
        &self.directory_entries
    }

    /// Recursively searches a storage's Red-Black tree for a child with the specified name.
    ///
    /// # Arguments
    ///
    /// * `parent_id` - Sibling or root node [`StreamId`] in the tree.
    /// * `target_name` - The target stream/storage name.
    ///
    /// # Returns
    ///
    /// The matching [`StreamId`], or `None` if not found.
    fn find_in_tree(&self, root_id: StreamId, target_name: &str) -> Option<StreamId> {
        let mut current_id = root_id;
        while current_id.is_valid() {
            let idx = current_id.as_u32() as usize;
            let Some(entry) = self.directory_entries.get(idx) else {
                break;
            };

            match compare_cfb_names(target_name, entry.name()) {
                Ordering::Less => {
                    current_id = entry.left_sibling();
                }
                Ordering::Greater => {
                    current_id = entry.right_sibling();
                }
                Ordering::Equal => {
                    return Some(current_id);
                }
            }
        }
        None
    }

    /// Finds a directory entry by its stream name under the root storage.
    ///
    /// # Arguments
    ///
    /// * `name` - The stream or storage name to locate.
    ///
    /// # Returns
    ///
    /// A reference to the [`DirectoryEntry`], or an error if not found.
    ///
    /// # Errors
    ///
    /// Returns [`Error::StreamNotFound`] if no entry with this name exists.
    pub fn find_entry(&self, name: &str) -> Result<&DirectoryEntry> {
        let root_child = self.directory_entries[0].child();
        let Some(stream_id) = self.find_in_tree(root_child, name) else {
            return Err(Error::StreamNotFound {
                name: name.to_string(),
            });
        };
        let idx = stream_id.as_u32() as usize;
        Ok(&self.directory_entries[idx])
    }

    /// Reads and extracts the complete payload bytes of a stream by name.
    ///
    /// Automatically determines whether the stream is housed in regular sectors
    /// or inside the Mini-Stream, and truncates to the exact stream size.
    ///
    /// # Arguments
    ///
    /// * `name` - The name of the stream to read.
    ///
    /// # Returns
    ///
    /// A [`Vec<u8>`] containing the stream payload.
    ///
    /// # Errors
    ///
    /// Returns [`Error::StreamNotFound`], [`Error::InvalidSector`], or [`Error::CfbCorrupted`].
    #[allow(clippy::cast_possible_truncation)]
    pub fn read_stream(&self, name: &str) -> Result<Vec<u8>> {
        let entry = self.find_entry(name)?;
        if entry.object_type() != ObjectType::Stream {
            return Err(Error::InvalidArgument {
                argument: "name".to_string(),
                reason: format!("entry '{name}' is not a stream"),
            });
        }

        let stream_len = entry.stream_size() as usize;
        if stream_len == 0 {
            return Ok(Vec::new());
        }

        if stream_len < self.header.mini_stream_cutoff() as usize {
            // Stream is stored in the Mini-Stream
            let mini_shift = self.header.mini_sector_shift();
            let mini_size = 1 << mini_shift;
            let mut result = Vec::with_capacity(stream_len);
            let mut current = MiniSectorId::new(entry.start_sector().as_u32());
            let mut visited = HashSet::new();

            while current.is_regular() {
                if !visited.insert(current) {
                    return Err(Error::SectorChainCycle {
                        sector: current.as_u32(),
                    });
                }

                let offset = current.mini_stream_offset(mini_shift)? as usize;
                if offset + mini_size > self.mini_stream.len() {
                    return Err(Error::CfbCorrupted {
                        offset: offset as u64,
                        reason: "Mini-sector extends beyond mini-stream boundary".to_string(),
                    });
                }

                result.extend_from_slice(&self.mini_stream[offset..offset + mini_size]);

                let idx = current.as_u32() as usize;
                if idx >= self.minifat.len() {
                    break;
                }
                current = self.minifat[idx];
            }

            if result.len() < stream_len {
                return Err(Error::StreamSizeMismatch {
                    expected: stream_len as u64,
                    actual: result.len() as u64,
                });
            }
            result.truncate(stream_len);
            Ok(result)
        } else {
            // Stream is stored in regular sectors
            let raw_bytes = Self::read_sector_chain_bytes(
                &self.data,
                &self.fat,
                entry.start_sector(),
                self.header.sector_shift(),
            )?;

            if raw_bytes.len() < stream_len {
                return Err(Error::StreamSizeMismatch {
                    expected: stream_len as u64,
                    actual: raw_bytes.len() as u64,
                });
            }
            let mut result = raw_bytes;
            result.truncate(stream_len);
            Ok(result)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cfb::header::CfbVersion;

    /// Helper that builds a minimal valid CFB v3 binary container in memory.
    #[allow(clippy::cast_possible_truncation)]
    fn build_minimal_cfb() -> Vec<u8> {
        let mut header = CfbHeader::new(CfbVersion::V3);
        header.set_num_fat_sectors(1);
        header.set_first_dir_sector(SectorId::new(1));
        header.set_first_minifat_sector(SectorId::END_OF_CHAIN);
        header.set_first_difat_sector(SectorId::END_OF_CHAIN);
        header.difat_table_mut()[0] = SectorId::new(0); // FAT is at sector 0

        let mut data = vec![0u8; 512 * 4]; // Header + Sector 0 (FAT) + Sector 1 (Dir) + Sector 2 (Data)

        // Write Header
        data[0..512].copy_from_slice(&header.to_bytes());

        // Write Sector 0: FAT entries
        // Sector 0 is FAT sector -> FAT[0] = FAT (0xFFFFFFFD)
        // Sector 1 is Dir sector -> FAT[1] = END_OF_CHAIN
        // Sectors 2..=11 are Data sectors for TestStream (5000 bytes)
        let fat_offset = 512;
        data[fat_offset..fat_offset + 4].copy_from_slice(&SectorId::FAT.as_u32().to_le_bytes());
        data[fat_offset + 4..fat_offset + 8]
            .copy_from_slice(&SectorId::END_OF_CHAIN.as_u32().to_le_bytes());

        // Chain sectors 2..=11
        for i in 2..11 {
            let off = fat_offset + i * 4;
            data[off..off + 4].copy_from_slice(&(i as u32 + 1).to_le_bytes());
        }
        data[fat_offset + 11 * 4..fat_offset + 12 * 4]
            .copy_from_slice(&SectorId::END_OF_CHAIN.as_u32().to_le_bytes());

        // Fill rest of FAT with FREE
        for i in 12..128 {
            let off = fat_offset + i * 4;
            data[off..off + 4].copy_from_slice(&SectorId::FREE.as_u32().to_le_bytes());
        }

        // Write Sector 1: Directory entries
        // Entry 0: Root Entry
        let mut root = DirectoryEntry::new("Root Entry", ObjectType::Root);
        root.set_child(StreamId::new(1));
        root.set_start_sector(SectorId::END_OF_CHAIN);

        // Entry 1: Stream Entry "TestStream" (regular sector >= 4096 bytes)
        let mut stream_ent = DirectoryEntry::new("TestStream", ObjectType::Stream);
        stream_ent.set_start_sector(SectorId::new(2));
        stream_ent.set_stream_size(5000);
        stream_ent.set_left_sibling(StreamId::new(2));

        // Entry 2: Storage Entry "SubStorage" (non-stream entry)
        let storage_ent = DirectoryEntry::new("SubStorage", ObjectType::Storage);

        let dir_offset = 1024;
        data[dir_offset..dir_offset + 128].copy_from_slice(&root.to_bytes());
        data[dir_offset + 128..dir_offset + 256].copy_from_slice(&stream_ent.to_bytes());
        data[dir_offset + 256..dir_offset + 384].copy_from_slice(&storage_ent.to_bytes());

        // Resize data to hold all sectors (1 header + 1 FAT + 1 Dir + 10 data sectors = 13 sectors)
        data.resize(512 * 13, 0xAA);
        data[1536..].fill(0xAA);

        data
    }

    /// Tests [`CfbReader::default`] constructor.
    #[test]
    fn test_cfb_reader_default() {
        let def = CfbReader::default();
        assert_eq!(def.entries().len(), 0);
        assert_eq!(def.header().version(), CfbVersion::V3);
    }

    /// Tests constructing [`CfbReader`] and reading streams.
    #[test]
    fn test_cfb_reader_basic() -> Result<()> {
        let bytes = build_minimal_cfb();
        let r = CfbReader::new(&bytes)?;
        assert_eq!(r.header().version(), CfbVersion::V3);
        assert_eq!(r.entries().len(), 4); // 512 / 128 = 4 entries in sector 1

        let ent = r.find_entry("TestStream");
        assert!(ent.is_ok());
        assert_eq!(ent.map(DirectoryEntry::name), Ok("TestStream"));

        assert!(r.find_entry("NonExistent").is_err());

        // Read stream data
        let data = r.read_stream("TestStream")?;
        assert_eq!(data, vec![0xAA; 5000]);

        // Read non-stream entry (SubStorage)
        assert!(matches!(
            r.read_stream("SubStorage"),
            Err(Error::InvalidArgument { .. })
        ));

        Ok(())
    }

    /// Tests error handling for corrupted headers or zero-length directory.
    #[test]
    fn test_cfb_reader_errors() -> Result<()> {
        assert!(CfbReader::new(&[0u8; 100]).is_err());

        // Corrupted file where sector extends beyond boundary
        let mut bytes = build_minimal_cfb();
        bytes.truncate(1024); // Cut off directory sector
        assert!(CfbReader::new(&bytes).is_err());

        // Zero directory entries
        let mut zero_dir_bytes = build_minimal_cfb();
        let mut header = CfbHeader::parse(&zero_dir_bytes)?;
        header.set_first_dir_sector(SectorId::END_OF_CHAIN);
        zero_dir_bytes[0..512].copy_from_slice(&header.to_bytes());
        assert!(matches!(
            CfbReader::new(&zero_dir_bytes),
            Err(Error::CfbCorrupted { .. })
        ));

        Ok(())
    }

    /// Tests external DIFAT sector chain traversal, cycles, and boundary checks.
    #[test]
    fn test_cfb_reader_difat_chains() -> Result<()> {
        // Build CFB with 1 external DIFAT sector
        let mut bytes = build_minimal_cfb();
        bytes.resize(512 * 6, 0); // Add sectors 3 and 4

        let mut header = CfbHeader::parse(&bytes)?;
        header.set_first_difat_sector(SectorId::new(3));
        header.set_num_difat_sectors(1);
        bytes[0..512].copy_from_slice(&header.to_bytes());

        // Write Sector 3: DIFAT sector
        // Contains FAT sector 0 and next DIFAT is END_OF_CHAIN
        let difat_offset = 512 * 4; // Sector 3 offset (header + 3 sectors)
        bytes[difat_offset..difat_offset + 4].copy_from_slice(&0u32.to_le_bytes());
        for i in 1..127 {
            bytes[difat_offset + i * 4..difat_offset + i * 4 + 4]
                .copy_from_slice(&SectorId::FREE.as_u32().to_le_bytes());
        }
        let next_difat_offset = difat_offset + 508;
        bytes[next_difat_offset..next_difat_offset + 4]
            .copy_from_slice(&SectorId::END_OF_CHAIN.as_u32().to_le_bytes());

        let reader = CfbReader::new(&bytes);
        assert!(reader.is_ok());

        // Test DIFAT cycle
        let mut cycle_bytes = bytes.clone();
        cycle_bytes[next_difat_offset..next_difat_offset + 4].copy_from_slice(&3u32.to_le_bytes()); // Points back to sector 3
        let mut header_cycle = header.clone();
        header_cycle.set_num_difat_sectors(2);
        cycle_bytes[0..512].copy_from_slice(&header_cycle.to_bytes());
        assert!(matches!(
            CfbReader::new(&cycle_bytes),
            Err(Error::SectorChainCycle { .. })
        ));

        // Test DIFAT out of bounds
        let mut oob_bytes = bytes.clone();
        oob_bytes.truncate(difat_offset + 256); // Truncate sector 3
        assert!(matches!(
            CfbReader::new(&oob_bytes),
            Err(Error::CfbCorrupted { .. })
        ));

        // Test FAT sector out of bounds
        let mut bad_fat_header = header.clone();
        bad_fat_header.difat_table_mut()[0] = SectorId::new(99); // Points outside file
        let mut bad_fat_bytes = bytes.clone();
        bad_fat_bytes[0..512].copy_from_slice(&bad_fat_header.to_bytes());
        assert!(matches!(
            CfbReader::new(&bad_fat_bytes),
            Err(Error::CfbCorrupted { .. })
        ));

        Ok(())
    }

    /// Tests FAT chain cycles, `MiniFAT` stream reading, `MiniFAT` cycles, and size mismatches.
    #[test]
    #[allow(clippy::too_many_lines, clippy::cast_possible_truncation)]
    fn test_cfb_reader_chains_and_mini_stream() -> Result<()> {
        // FAT cycle test
        let mut cycle_bytes = build_minimal_cfb();
        // Set FAT[2] = 2 (self-loop)
        let fat_offset = 512;
        cycle_bytes[fat_offset + 8..fat_offset + 12].copy_from_slice(&2u32.to_le_bytes());
        let reader = CfbReader::new(&cycle_bytes)?;
        assert!(matches!(
            reader.read_stream("TestStream"),
            Err(Error::SectorChainCycle { .. })
        ));

        // Test Mini-Stream reading with writer
        let mut writer = crate::cfb::writer::CfbWriter::new(CfbVersion::V3);
        assert!(writer.add_stream("Mini1", b"Mini-stream content 1").is_ok());
        assert!(writer.add_stream("Mini2", b"Mini-stream content 2").is_ok());
        let bin = writer.build();

        let r = CfbReader::new(&bin)?;
        assert_eq!(r.read_stream("Mini1")?, b"Mini-stream content 1");
        assert_eq!(r.read_stream("Mini2")?, b"Mini-stream content 2");

        // Tree navigation: search for name smaller than root child
        // "A_Stream" < "Mini1"
        assert!(r.find_entry("A_Stream").is_err());

        // Test mini-stream cycle
        let mut cycle_mini_bin = bin.clone();
        // MiniFAT is in mini-fat sector; corrupt MiniFAT[0] to point to 0 (self-loop)
        let parsed_header = CfbHeader::parse(&cycle_mini_bin)?;
        let minifat_sec = parsed_header.first_minifat_sector();
        let minifat_off = minifat_sec.file_offset(parsed_header.sector_shift())? as usize;
        cycle_mini_bin[minifat_off..minifat_off + 4].copy_from_slice(&0u32.to_le_bytes());

        let reader_mini_cycle = CfbReader::new(&cycle_mini_bin)?;
        assert!(matches!(
            reader_mini_cycle.read_stream("Mini1"),
            Err(Error::SectorChainCycle { .. })
        ));

        // Test early DIFAT chain end when num_difat_sectors > actual chain
        let mut early_difat_bytes = build_minimal_cfb();
        let mut early_header = CfbHeader::parse(&early_difat_bytes)?;
        early_header.set_num_difat_sectors(5);
        early_header.set_first_difat_sector(SectorId::END_OF_CHAIN);
        early_difat_bytes[0..512].copy_from_slice(&early_header.to_bytes());
        let r_early = CfbReader::new(&early_difat_bytes);
        assert!(r_early.is_ok());

        // Test root mini-stream size mismatch (root entry stream_size > file bytes)
        let mut bad_root_bytes = bin.clone();
        let dir_sec = parsed_header.first_dir_sector();
        let dir_off = dir_sec.file_offset(parsed_header.sector_shift())? as usize;
        // Root entry stream size at offset 0x78 (120) in root entry
        bad_root_bytes[dir_off + 120..dir_off + 128].copy_from_slice(&999_999u64.to_le_bytes());
        assert!(matches!(
            CfbReader::new(&bad_root_bytes),
            Err(Error::StreamSizeMismatch { .. })
        ));

        // Test stream size mismatch on regular stream
        let mut bad_stream_bytes = build_minimal_cfb();
        // Modify TestStream size to 100_000 bytes
        let test_dir_off = 1024 + 128; // Entry 1
        bad_stream_bytes[test_dir_off + 120..test_dir_off + 128]
            .copy_from_slice(&100_000u64.to_le_bytes());
        let reader_bad = CfbReader::new(&bad_stream_bytes)?;
        assert!(matches!(
            reader_bad.read_stream("TestStream"),
            Err(Error::StreamSizeMismatch { .. })
        ));

        // Test tree search with out-of-bounds StreamId
        let mut bad_tree_bytes = bin.clone();
        // Point root's child to 9999
        bad_tree_bytes[dir_off + 76..dir_off + 80].copy_from_slice(&9999u32.to_le_bytes());
        let reader_bad_tree = CfbReader::new(&bad_tree_bytes)?;
        assert!(matches!(
            reader_bad_tree.find_entry("Mini1"),
            Err(Error::StreamNotFound { .. })
        ));

        // Test mini-stream sector out of bounds of mini-stream buffer
        let mut bad_mini_sec_bytes = bin.clone();
        // Mini1 entry is entry 1; set starting sector to 500 (exceeds mini-stream length)
        let mini1_ent_off = dir_off + 128;
        bad_mini_sec_bytes[mini1_ent_off + 116..mini1_ent_off + 120]
            .copy_from_slice(&500u32.to_le_bytes());
        let reader_bad_minisec = CfbReader::new(&bad_mini_sec_bytes)?;
        assert!(matches!(
            reader_bad_minisec.read_stream("Mini1"),
            Err(Error::CfbCorrupted { .. })
        ));

        // Test mini-stream size mismatch (mini-stream ends before stream_size)
        let mut short_chain_bytes = bin.clone();
        // Mini1 size set to 2000 bytes, but MiniFAT only allocated 1 sector (64 bytes)
        short_chain_bytes[mini1_ent_off + 120..mini1_ent_off + 128]
            .copy_from_slice(&2000u64.to_le_bytes());
        let reader_short_chain = CfbReader::new(&short_chain_bytes)?;
        assert!(matches!(
            reader_short_chain.read_stream("Mini1"),
            Err(Error::StreamSizeMismatch { .. })
        ));

        // Test FAT chain index pointing beyond file boundary
        let mut oob_chain_bytes = build_minimal_cfb();
        // In FAT: set sector 2's next sector to 9999 (points beyond file boundary)
        let fat_off = 512;
        oob_chain_bytes[fat_off + 8..fat_off + 12].copy_from_slice(&9999u32.to_le_bytes());
        let reader_oob = CfbReader::new(&oob_chain_bytes)?;
        assert!(matches!(
            reader_oob.read_stream("TestStream"),
            Err(Error::CfbCorrupted { .. })
        ));
        // Test read_sector_chain_bytes when idx >= fat.len()
        let short_fat_data = vec![0u8; 512 * 5];
        let short_fat = vec![SectorId::new(2)]; // length 1
        let chain_res =
            CfbReader::read_sector_chain_bytes(&short_fat_data, &short_fat, SectorId::new(0), 9);
        assert_eq!(chain_res.map(|v| v.len()), Ok(1024));

        // Test read_stream with empty minifat (breaks on idx >= minifat.len())
        let mut no_minifat_bin = bin.clone();
        // Set num_minifat_sectors to 0 in header
        no_minifat_bin[64..68].copy_from_slice(&0u32.to_le_bytes());
        let reader_no_minifat = CfbReader::new(&no_minifat_bin)?;
        // Reads first mini-sector (64 bytes), then idx (0) >= minifat.len() (0) -> break!
        assert_eq!(
            reader_no_minifat.read_stream("Mini1")?,
            b"Mini-stream content 1"
        );

        // Test MiniFAT sector pointing beyond mini-stream boundary
        let mut oob_minifat_bin = bin;
        oob_minifat_bin[minifat_off..minifat_off + 4].copy_from_slice(&9999u32.to_le_bytes());
        let reader_oob_minifat = CfbReader::new(&oob_minifat_bin)?;
        assert!(matches!(
            reader_oob_minifat.read_stream("Mini1"),
            Err(Error::CfbCorrupted { .. })
        ));

        Ok(())
    }

    /// Tests handling of non-regular sectors for mini-stream and `MiniFAT` roots.
    #[test]
    fn test_cfb_reader_non_regular_sectors() -> Result<()> {
        let mut bytes = build_minimal_cfb();

        // 1. Root entry has stream_size > 0 but start_sector is END_OF_CHAIN (not regular)
        bytes[1024 + 120..1024 + 128].copy_from_slice(&100u64.to_le_bytes());

        // 2. Header has num_minifat_sectors > 0 but first_minifat_sector is END_OF_CHAIN (not regular)
        let mut header = CfbHeader::parse(&bytes)?;
        header.set_num_minifat_sectors(1);
        header.set_first_minifat_sector(SectorId::END_OF_CHAIN);
        bytes[0..512].copy_from_slice(&header.to_bytes());

        let reader = CfbReader::new(&bytes)?;
        assert!(reader.find_entry("TestStream").is_ok());
        Ok(())
    }
}

//! Compound File Binary Format Writer and Compactor ([MS-CFB] 2.3 - 2.6).

use crate::cfb::directory::{
    compare_cfb_names, ColorFlag, DirectoryEntry, ObjectType, StreamId, DIRECTORY_ENTRY_SIZE,
};
use crate::cfb::header::{
    CfbHeader, CfbVersion, CFB_HEADER_DIFAT_ENTRIES, CFB_MINI_SECTOR_SHIFT_STANDARD,
    CFB_MINI_STREAM_CUTOFF_STANDARD,
};
use crate::cfb::sector::{MiniSectorId, SectorId};
use crate::error::{Error, Result};
use std::cmp::Ordering;

/// In-memory stream item staged for packaging into a CFB container.
#[derive(Debug, Clone)]
struct StagedStream {
    /// Stream name.
    name: String,
    /// Stream payload data.
    data: Vec<u8>,
}

/// Compound File Binary Format (CFB) container builder and compactor.
#[derive(Debug, Clone)]
pub struct CfbWriter {
    /// Target CFB major version.
    version: CfbVersion,
    /// List of streams to include in the package.
    streams: Vec<StagedStream>,
}

impl CfbWriter {
    /// Creates a new [`CfbWriter`] with the specified format version.
    ///
    /// # Arguments
    ///
    /// * `version` - The [`CfbVersion`] (typically [`CfbVersion::V3`] for MSI packages).
    ///
    /// # Returns
    ///
    /// An empty [`CfbWriter`] instance.
    #[must_use]
    pub const fn new(version: CfbVersion) -> Self {
        Self {
            version,
            streams: Vec::new(),
        }
    }

    /// Adds a stream to the CFB container.
    ///
    /// # Arguments
    ///
    /// * `name` - The stream name.
    /// * `data` - The stream payload bytes.
    ///
    /// # Errors
    ///
    /// Returns [`Error::DuplicateDirectoryEntry`] if a stream with this name has already been added.
    pub fn add_stream(&mut self, name: &str, data: &[u8]) -> Result<()> {
        for s in &self.streams {
            if compare_cfb_names(&s.name, name) == Ordering::Equal {
                return Err(Error::DuplicateDirectoryEntry {
                    name: name.to_string(),
                });
            }
        }
        self.streams.push(StagedStream {
            name: name.to_string(),
            data: data.to_vec(),
        });
        Ok(())
    }

    /// Inserts a node into a Red-Black Tree maintaining [MS-CFB] 2.6.3 invariants.
    fn rb_insert(
        root: StreamId,
        new_id: StreamId,
        entries: &mut [DirectoryEntry],
        parents: &mut [StreamId],
    ) -> StreamId {
        if root.is_none() {
            entries[new_id.as_u32() as usize].set_color(ColorFlag::Black);
            return new_id;
        }

        // 1. BST insertion
        let mut curr = root;
        let new_name = entries[new_id.as_u32() as usize].name().to_string();

        loop {
            let curr_idx = curr.as_u32() as usize;
            let order = compare_cfb_names(&new_name, entries[curr_idx].name());
            if order == Ordering::Less {
                let left = entries[curr_idx].left_sibling();
                if left.is_none() {
                    entries[curr_idx].set_left_sibling(new_id);
                    parents[new_id.as_u32() as usize] = curr;
                    break;
                }
                curr = left;
            } else {
                let right = entries[curr_idx].right_sibling();
                if right.is_none() {
                    entries[curr_idx].set_right_sibling(new_id);
                    parents[new_id.as_u32() as usize] = curr;
                    break;
                }
                curr = right;
            }
        }

        entries[new_id.as_u32() as usize].set_color(ColorFlag::Red);

        // 2. Fix-up Red-Black Tree invariants
        let mut node = new_id;
        let mut current_root = root;

        while node != current_root
            && entries[parents[node.as_u32() as usize].as_u32() as usize].color() == ColorFlag::Red
        {
            let p = parents[node.as_u32() as usize];
            let gp = parents[p.as_u32() as usize];
            let gp_idx = gp.as_u32() as usize;
            let p_idx = p.as_u32() as usize;

            if p == entries[gp_idx].left_sibling() {
                let uncle = entries[gp_idx].right_sibling();
                if uncle.is_valid() && entries[uncle.as_u32() as usize].color() == ColorFlag::Red {
                    entries[p_idx].set_color(ColorFlag::Black);
                    entries[uncle.as_u32() as usize].set_color(ColorFlag::Black);
                    entries[gp_idx].set_color(ColorFlag::Red);
                    node = gp;
                } else {
                    if node == entries[p_idx].right_sibling() {
                        node = p;
                        current_root = Self::rotate_left(node, current_root, entries, parents);
                    }
                    let p2 = parents[node.as_u32() as usize];
                    let gp2 = parents[p2.as_u32() as usize];
                    entries[p2.as_u32() as usize].set_color(ColorFlag::Black);
                    entries[gp2.as_u32() as usize].set_color(ColorFlag::Red);
                    current_root = Self::rotate_right(gp2, current_root, entries, parents);
                }
            } else {
                let uncle = entries[gp_idx].left_sibling();
                if uncle.is_valid() && entries[uncle.as_u32() as usize].color() == ColorFlag::Red {
                    entries[p_idx].set_color(ColorFlag::Black);
                    entries[uncle.as_u32() as usize].set_color(ColorFlag::Black);
                    entries[gp_idx].set_color(ColorFlag::Red);
                    node = gp;
                } else {
                    if node == entries[p_idx].left_sibling() {
                        node = p;
                        current_root = Self::rotate_right(node, current_root, entries, parents);
                    }
                    let p2 = parents[node.as_u32() as usize];
                    let gp2 = parents[p2.as_u32() as usize];
                    entries[p2.as_u32() as usize].set_color(ColorFlag::Black);
                    entries[gp2.as_u32() as usize].set_color(ColorFlag::Red);
                    current_root = Self::rotate_left(gp2, current_root, entries, parents);
                }
            }
        }

        entries[current_root.as_u32() as usize].set_color(ColorFlag::Black);
        current_root
    }

    /// Performs left tree rotation around `x`.
    fn rotate_left(
        x: StreamId,
        root: StreamId,
        entries: &mut [DirectoryEntry],
        parents: &mut [StreamId],
    ) -> StreamId {
        let x_idx = x.as_u32() as usize;
        let y = entries[x_idx].right_sibling();
        let y_idx = y.as_u32() as usize;

        let y_left = entries[y_idx].left_sibling();
        entries[x_idx].set_right_sibling(y_left);
        if y_left.is_valid() {
            parents[y_left.as_u32() as usize] = x;
        }

        let p = parents[x_idx];
        parents[y_idx] = p;

        let new_root = if p.is_none() {
            y
        } else {
            let p_idx = p.as_u32() as usize;
            if x == entries[p_idx].left_sibling() {
                entries[p_idx].set_left_sibling(y);
            } else {
                entries[p_idx].set_right_sibling(y);
            }
            root
        };

        entries[y_idx].set_left_sibling(x);
        parents[x_idx] = y;

        new_root
    }

    /// Performs right tree rotation around `y`.
    fn rotate_right(
        y: StreamId,
        root: StreamId,
        entries: &mut [DirectoryEntry],
        parents: &mut [StreamId],
    ) -> StreamId {
        let y_idx = y.as_u32() as usize;
        let x = entries[y_idx].left_sibling();
        let x_idx = x.as_u32() as usize;

        let x_right = entries[x_idx].right_sibling();
        entries[y_idx].set_left_sibling(x_right);
        if x_right.is_valid() {
            parents[x_right.as_u32() as usize] = y;
        }

        let p = parents[y_idx];
        parents[x_idx] = p;

        let new_root = if p.is_none() {
            x
        } else {
            let p_idx = p.as_u32() as usize;
            if y == entries[p_idx].left_sibling() {
                entries[p_idx].set_left_sibling(x);
            } else {
                entries[p_idx].set_right_sibling(x);
            }
            root
        };

        entries[x_idx].set_right_sibling(y);
        parents[y_idx] = x;

        new_root
    }

    /// Compiles, lays out, compacts, and serializes the complete CFB binary container.
    ///
    /// # Returns
    ///
    /// A [`Vec<u8>`] containing the complete, valid CFB binary package.
    #[must_use]
    #[allow(
        clippy::too_many_lines,
        clippy::cast_possible_truncation,
        clippy::manual_div_ceil
    )]
    pub fn build(self) -> Vec<u8> {
        let sector_size = self.version.sector_size();
        let mini_sector_size = 1 << CFB_MINI_SECTOR_SHIFT_STANDARD; // 64

        // 1. Partition streams into Mini-Stream (< 4096 bytes) and Regular Streams (>= 4096 bytes)
        let mut mini_stream_bytes = Vec::new();
        let mut minifat_entries = Vec::new();

        let mut directory_entries = Vec::new();
        // Entry 0 is Root Entry
        let root_entry = DirectoryEntry::new("Root Entry", ObjectType::Root);
        directory_entries.push(root_entry);

        // Staged regular stream buffers
        let mut regular_streams: Vec<(usize, Vec<u8>)> = Vec::new();

        for s in &self.streams {
            let mut entry = DirectoryEntry::new(&s.name, ObjectType::Stream);
            let size = s.data.len();
            entry.set_stream_size(size as u64);

            if size > 0 && size < CFB_MINI_STREAM_CUTOFF_STANDARD as usize {
                // Mini-stream allocation
                let start_mini_sector = (mini_stream_bytes.len() / mini_sector_size) as u32;
                entry.set_start_sector(SectorId::new(start_mini_sector));

                let num_mini_sectors = (size + mini_sector_size - 1) / mini_sector_size;
                for i in 0..num_mini_sectors {
                    let next_sec = if i + 1 == num_mini_sectors {
                        MiniSectorId::END_OF_CHAIN
                    } else {
                        MiniSectorId::new(start_mini_sector + i as u32 + 1)
                    };
                    minifat_entries.push(next_sec);
                }

                mini_stream_bytes.extend_from_slice(&s.data);
                // Zero-pad to 64-byte boundary
                let rem = mini_stream_bytes.len() % mini_sector_size;
                if rem != 0 {
                    mini_stream_bytes.resize(mini_stream_bytes.len() + (mini_sector_size - rem), 0);
                }
            } else if size >= CFB_MINI_STREAM_CUTOFF_STANDARD as usize {
                // Regular stream (staged for sector layout)
                let entry_idx = directory_entries.len();
                regular_streams.push((entry_idx, s.data.clone()));
            } else {
                // Zero size stream
                entry.set_start_sector(SectorId::END_OF_CHAIN);
            }

            directory_entries.push(entry);
        }

        // 2. Build Red-Black tree of children under Root Entry
        if directory_entries.len() > 1 {
            let total = directory_entries.len();
            let mut parents = vec![StreamId::NO_STREAM; total];
            let mut root_child = StreamId::NO_STREAM;

            for id in 1..total {
                root_child = Self::rb_insert(
                    root_child,
                    StreamId::new(id as u32),
                    &mut directory_entries,
                    &mut parents,
                );
            }
            directory_entries[0].set_child(root_child);
        }

        // 3. Allocate sectors for all components
        // We will layout:
        // - Regular stream data sectors
        // - Mini-stream data sectors (if any)
        // - Directory stream sectors
        // - MiniFAT sectors (if any)
        // - FAT sectors
        // - DIFAT sectors (if needed)

        let mut sector_data_blocks: Vec<Vec<u8>> = Vec::new();
        let mut fat_table: Vec<SectorId> = Vec::new();

        let alloc_chain =
            |payload: &[u8], fat: &mut Vec<SectorId>, blocks: &mut Vec<Vec<u8>>| -> SectorId {
                if payload.is_empty() {
                    return SectorId::END_OF_CHAIN;
                }
                let num_sec = (payload.len() + sector_size - 1) / sector_size;
                let start_sec = SectorId::new(blocks.len() as u32);

                for i in 0..num_sec {
                    let curr_sec = start_sec.as_u32() + i as u32;
                    let next_sec = if i + 1 == num_sec {
                        SectorId::END_OF_CHAIN
                    } else {
                        SectorId::new(curr_sec + 1)
                    };
                    fat.push(next_sec);

                    let offset = i * sector_size;
                    let end = (offset + sector_size).min(payload.len());
                    let mut block = vec![0u8; sector_size];
                    block[0..end - offset].copy_from_slice(&payload[offset..end]);
                    blocks.push(block);
                }

                start_sec
            };

        // 3a. Regular stream data sectors
        for (ent_idx, data) in regular_streams {
            let start = alloc_chain(&data, &mut fat_table, &mut sector_data_blocks);
            directory_entries[ent_idx].set_start_sector(start);
        }

        // 3b. Mini-stream data sectors (recorded in Root Entry)
        directory_entries[0].set_stream_size(mini_stream_bytes.len() as u64);
        let mini_stream_start =
            alloc_chain(&mini_stream_bytes, &mut fat_table, &mut sector_data_blocks);
        directory_entries[0].set_start_sector(mini_stream_start);

        // 3c. Directory stream sectors
        // Pad directory entries up to 4 per sector (in v3) or 32 per sector (in v4)
        let entries_per_sector = sector_size / DIRECTORY_ENTRY_SIZE;
        let rem_entries = directory_entries.len() % entries_per_sector;
        if rem_entries != 0 {
            for _ in 0..(entries_per_sector - rem_entries) {
                directory_entries.push(DirectoryEntry::empty());
            }
        }

        let mut dir_stream_bytes =
            Vec::with_capacity(directory_entries.len() * DIRECTORY_ENTRY_SIZE);
        for entry in &directory_entries {
            dir_stream_bytes.extend_from_slice(&entry.to_bytes());
        }
        let first_dir_sector =
            alloc_chain(&dir_stream_bytes, &mut fat_table, &mut sector_data_blocks);
        let num_dir_sectors = (dir_stream_bytes.len() / sector_size) as u32;

        // 3d. MiniFAT sectors
        let mut minifat_bytes = Vec::with_capacity(minifat_entries.len() * 4);
        for &m in &minifat_entries {
            minifat_bytes.extend_from_slice(&m.as_u32().to_le_bytes());
        }
        let (first_minifat_sector, num_minifat_sectors) = if minifat_bytes.is_empty() {
            (SectorId::END_OF_CHAIN, 0)
        } else {
            let num = (minifat_bytes.len() + sector_size - 1) / sector_size;
            let start = alloc_chain(&minifat_bytes, &mut fat_table, &mut sector_data_blocks);
            (start, num as u32)
        };

        // 4. FAT Sector calculation
        // Each FAT sector can hold `sector_size / 4` entries (128 entries for 512-byte sector)
        // We need enough FAT sectors to cover all data blocks + FAT sectors + DIFAT sectors!
        let entries_per_fat_sector = sector_size / 4;
        let mut num_fat_sectors = 0;
        loop {
            let total_sectors_needed = sector_data_blocks.len() + num_fat_sectors;
            let needed_fat_sectors =
                (total_sectors_needed + entries_per_fat_sector - 1) / entries_per_fat_sector;
            if needed_fat_sectors == num_fat_sectors {
                break;
            }
            num_fat_sectors = needed_fat_sectors;
        }

        let start_fat_sector = SectorId::new(sector_data_blocks.len() as u32);
        for i in 0..num_fat_sectors {
            let sec_id = start_fat_sector.as_u32() + i as u32;
            let _ = sec_id;
            fat_table.push(SectorId::FAT);
        }

        // Pad FAT table to multiple of `entries_per_fat_sector`
        while fat_table.len() < num_fat_sectors * entries_per_fat_sector {
            fat_table.push(SectorId::FREE);
        }

        // Build FAT sector blocks
        let mut fat_blocks = Vec::new();
        for i in 0..num_fat_sectors {
            let mut block = vec![0u8; sector_size];
            for j in 0..entries_per_fat_sector {
                let idx = i * entries_per_fat_sector + j;
                let val = fat_table[idx].as_u32();
                block[j * 4..j * 4 + 4].copy_from_slice(&val.to_le_bytes());
            }
            fat_blocks.push(block);
        }

        // 5. Construct Header
        let mut header = CfbHeader::new(self.version);
        if self.version == CfbVersion::V4 {
            header.set_num_dir_sectors(num_dir_sectors);
        }
        header.set_first_dir_sector(first_dir_sector);
        header.set_first_minifat_sector(first_minifat_sector);
        header.set_num_minifat_sectors(num_minifat_sectors);
        header.set_num_fat_sectors(num_fat_sectors as u32);

        // Header DIFAT table can hold up to 109 FAT sectors
        let difat_in_header = num_fat_sectors.min(CFB_HEADER_DIFAT_ENTRIES);
        for i in 0..difat_in_header {
            header.difat_table_mut()[i] = SectorId::new(start_fat_sector.as_u32() + i as u32);
        }

        // 6. Assemble complete output file buffer
        let total_file_size =
            sector_size + (sector_data_blocks.len() + fat_blocks.len()) * sector_size;
        let mut output = Vec::with_capacity(total_file_size);

        // Header (padded to sector_size)
        let header_bytes = header.to_bytes();
        output.extend_from_slice(&header_bytes);
        if sector_size > 512 {
            output.resize(sector_size, 0);
        }

        // Data blocks
        for block in sector_data_blocks {
            output.extend_from_slice(&block);
        }

        // FAT blocks
        for block in fat_blocks {
            output.extend_from_slice(&block);
        }

        output
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cfb::reader::CfbReader;

    /// Tests adding streams, writing container, and reading back with [`CfbReader`].
    #[test]
    fn test_cfb_writer_roundtrip() {
        let mut writer = CfbWriter::new(CfbVersion::V3);

        // Add a small stream (< 4096 bytes, will go into Mini-Stream)
        let small_data = b"Hello, small mini-stream world!";
        assert!(writer.add_stream("SmallStream", &small_data[..]).is_ok());

        // Add a second small stream
        let second_data = b"Another small stream payload";
        assert!(writer.add_stream("SecondStream", &second_data[..]).is_ok());

        // Add a large stream (>= 4096 bytes, will go into regular sectors)
        let large_data = vec![0x42u8; 8192];
        assert!(writer.add_stream("LargeStream", &large_data).is_ok());

        // Duplicate name should fail
        assert!(writer.add_stream("SmallStream", b"dup").is_err());

        let bin = writer.build();
        assert_ne!(bin.len(), 0);

        // Verify with CfbReader
        let r = CfbReader::new(&bin).unwrap_or_default();

        // Read small stream
        assert_eq!(r.read_stream("SmallStream"), Ok(small_data.to_vec()));

        // Read second stream
        assert_eq!(r.read_stream("SecondStream"), Ok(second_data.to_vec()));

        // Read large stream
        assert_eq!(r.read_stream("LargeStream"), Ok(large_data));

        // Read nonexistent stream
        assert!(r.read_stream("Missing").is_err());
    }

    /// Tests building empty CFB container and version 4 container.
    #[test]
    fn test_cfb_writer_empty_and_v4() {
        let writer_v3 = CfbWriter::new(CfbVersion::V3);
        let bin_v3 = writer_v3.build();
        assert_ne!(bin_v3.len(), 0);

        let mut writer_v4 = CfbWriter::new(CfbVersion::V4);
        assert!(writer_v4.add_stream("V4Stream", &[0x11; 5000]).is_ok());
        let bin_v4 = writer_v4.build();

        let reader = CfbReader::new(&bin_v4).unwrap_or_default();
        assert_eq!(reader.header().version(), CfbVersion::V4);
        assert_eq!(reader.read_stream("V4Stream"), Ok(vec![0x11; 5000]));
    }

    /// Tests Red-Black Tree insertion, rotations, recoloring, and zero-size stream handling.
    #[test]
    fn test_cfb_writer_red_black_tree_and_zero_size() {
        let mut writer = CfbWriter::new(CfbVersion::V3);

        // Add an empty zero-sized stream
        assert!(writer.add_stream("Empty", &[]).is_ok());

        // Insert names in specific sequences to trigger left and right rotations,
        // red-uncle recoloring, and both left-of-right and right-of-left cases.
        let names = [
            "M10", "E05", "T20", "B02", "H08", "P15", "X25", "A01", "C03", "F06", "K09", "N12",
            "R18", "V22", "Z26", "D04", "G07", "J10", "L11", "O14", "Q16", "S19", "U21", "W23",
            "Y24",
        ];

        for &name in &names {
            assert!(writer.add_stream(name, name.as_bytes()).is_ok());
        }

        let bytes = writer.build();
        let r = CfbReader::new(&bytes).unwrap_or_default();

        // Verify zero-sized stream
        assert_eq!(r.read_stream("Empty"), Ok(Vec::new()));

        // Verify all inserted streams are retrievable
        for &name in &names {
            assert_eq!(r.read_stream(name), Ok(name.as_bytes().to_vec()));
        }
    }

    /// Tests container with only large streams (no `MiniFAT` sectors allocated).
    #[test]
    fn test_cfb_writer_only_large_streams() {
        let mut writer = CfbWriter::new(CfbVersion::V3);
        assert!(writer.add_stream("Large1", &[1u8; 4096]).is_ok());
        assert!(writer.add_stream("Large2", &[2u8; 4096]).is_ok());

        let bin = writer.build();
        let reader = CfbReader::new(&bin).unwrap_or_default();
        assert_eq!(reader.read_stream("Large1"), Ok(vec![1u8; 4096]));
        assert_eq!(reader.read_stream("Large2"), Ok(vec![2u8; 4096]));
    }

    /// Tests explicit Red-Black tree topologies:
    /// - Right-Right and Right-Left rotations
    /// - Left-Left and Left-Right rotations
    /// - Red uncle on left and red uncle on right
    /// - Subtree rotations under existing parents
    #[test]
    #[allow(clippy::similar_names, clippy::too_many_lines)]
    fn test_cfb_writer_rb_topologies() {
        // Case A: Right-Right (A, B, C)
        let mut w1 = CfbWriter::new(CfbVersion::V3);
        for &s in &["A", "B", "C"] {
            assert!(w1.add_stream(s, &[1]).is_ok());
        }
        let _ = w1.build();

        // Case B: Right-Left (A, C, B)
        let mut w2 = CfbWriter::new(CfbVersion::V3);
        for &s in &["A", "C", "B"] {
            assert!(w2.add_stream(s, &[1]).is_ok());
        }
        let _ = w2.build();

        // Case C: Left-Left (C, B, A)
        let mut w3 = CfbWriter::new(CfbVersion::V3);
        for &s in &["C", "B", "A"] {
            assert!(w3.add_stream(s, &[1]).is_ok());
        }
        let _ = w3.build();

        // Case D: Left-Right (C, A, B)
        let mut w4 = CfbWriter::new(CfbVersion::V3);
        for &s in &["C", "A", "B"] {
            assert!(w4.add_stream(s, &[1]).is_ok());
        }
        let _ = w4.build();

        // Case E: Subtree rotations under left and right branches
        let mut w5 = CfbWriter::new(CfbVersion::V3);
        for &s in &[
            "M", "E", "T", "B", "H", "P", "X", "A", "C", "F", "K", "N", "R", "U", "Z", "D",
        ] {
            assert!(w5.add_stream(s, &[1]).is_ok());
        }
        let _ = w5.build();

        // Case F: Multi-mini-sector streams, exact multiple of 64 bytes, and subtree right-child rotations
        let mut w6 = CfbWriter::new(CfbVersion::V3);
        assert!(w6.add_stream("Mini100", &[7u8; 100]).is_ok());
        assert!(w6.add_stream("Mini128", &[8u8; 128]).is_ok());
        for &s in &["B", "M", "T", "P", "C", "D", "E", "F"] {
            assert!(w6.add_stream(s, &[9]).is_ok());
        }
        let _ = w6.build();

        // Case G: Direct tests for rotate_left with valid y_left and rotate_right with valid x_right
        let mut entries_l = vec![
            DirectoryEntry::empty(),
            DirectoryEntry::new("X", ObjectType::Stream),
            DirectoryEntry::new("Y", ObjectType::Stream),
            DirectoryEntry::new("YLeft", ObjectType::Stream),
        ];
        entries_l[1].set_right_sibling(StreamId::new(2));
        entries_l[2].set_left_sibling(StreamId::new(3));
        let mut parents_l = vec![
            StreamId::NO_STREAM,
            StreamId::NO_STREAM,
            StreamId::new(1),
            StreamId::new(2),
        ];
        let new_root_l = CfbWriter::rotate_left(
            StreamId::new(1),
            StreamId::new(1),
            &mut entries_l,
            &mut parents_l,
        );
        assert_eq!(new_root_l, StreamId::new(2));
        assert_eq!(parents_l[3], StreamId::new(1));

        let mut entries_r = vec![
            DirectoryEntry::empty(),
            DirectoryEntry::new("Y", ObjectType::Stream),
            DirectoryEntry::new("X", ObjectType::Stream),
            DirectoryEntry::new("XRight", ObjectType::Stream),
        ];
        entries_r[1].set_left_sibling(StreamId::new(2));
        entries_r[2].set_right_sibling(StreamId::new(3));
        let mut parents_r = vec![
            StreamId::NO_STREAM,
            StreamId::NO_STREAM,
            StreamId::new(1),
            StreamId::new(2),
        ];
        let new_root_r = CfbWriter::rotate_right(
            StreamId::new(1),
            StreamId::new(1),
            &mut entries_r,
            &mut parents_r,
        );
        assert_eq!(new_root_r, StreamId::new(2));
        assert_eq!(parents_r[3], StreamId::new(1));

        // Test rotate_right where y is right sibling of p
        let mut right_entries_two = vec![
            DirectoryEntry::empty(),
            DirectoryEntry::new("Y", ObjectType::Stream),
            DirectoryEntry::new("X", ObjectType::Stream),
            DirectoryEntry::new("P", ObjectType::Stream),
        ];
        right_entries_two[3].set_right_sibling(StreamId::new(1)); // P's right child is Y
        right_entries_two[1].set_left_sibling(StreamId::new(2)); // Y's left child is X
        let mut right_parents_two = vec![
            StreamId::NO_STREAM,
            StreamId::new(3),
            StreamId::new(1),
            StreamId::NO_STREAM,
        ];
        let root_r2 = CfbWriter::rotate_right(
            StreamId::new(1),
            StreamId::new(3),
            &mut right_entries_two,
            &mut right_parents_two,
        );
        assert_eq!(root_r2, StreamId::new(3));
        assert_eq!(right_entries_two[3].right_sibling(), StreamId::new(2));

        // Test rotate_right where y is left sibling of p
        let mut right_entries_three = vec![
            DirectoryEntry::empty(),
            DirectoryEntry::new("Y", ObjectType::Stream),
            DirectoryEntry::new("X", ObjectType::Stream),
            DirectoryEntry::new("P", ObjectType::Stream),
        ];
        right_entries_three[3].set_left_sibling(StreamId::new(1)); // P's left child is Y
        right_entries_three[1].set_left_sibling(StreamId::new(2)); // Y's left child is X
        let mut right_parents_three = vec![
            StreamId::NO_STREAM,
            StreamId::new(3),
            StreamId::new(1),
            StreamId::NO_STREAM,
        ];
        let root_r3 = CfbWriter::rotate_right(
            StreamId::new(1),
            StreamId::new(3),
            &mut right_entries_three,
            &mut right_parents_three,
        );
        assert_eq!(root_r3, StreamId::new(3));
        assert_eq!(right_entries_three[3].left_sibling(), StreamId::new(2));

        // Test rotate_left where x is left sibling of p
        let mut left_entries_two = vec![
            DirectoryEntry::empty(),
            DirectoryEntry::new("X", ObjectType::Stream),
            DirectoryEntry::new("Y", ObjectType::Stream),
            DirectoryEntry::new("P", ObjectType::Stream),
        ];
        left_entries_two[3].set_left_sibling(StreamId::new(1)); // P's left child is X
        left_entries_two[1].set_right_sibling(StreamId::new(2)); // X's right child is Y
        let mut left_parents_two = vec![
            StreamId::NO_STREAM,
            StreamId::new(3),
            StreamId::new(1),
            StreamId::NO_STREAM,
        ];
        let root_l2 = CfbWriter::rotate_left(
            StreamId::new(1),
            StreamId::new(3),
            &mut left_entries_two,
            &mut left_parents_two,
        );
        assert_eq!(root_l2, StreamId::new(3));
        assert_eq!(left_entries_two[3].left_sibling(), StreamId::new(2));
    }

    /// Tests Red-Black tree insertion with a valid black left uncle (case where parent is right child).
    #[test]
    fn test_cfb_writer_black_left_uncle() {
        let mut entries = vec![
            DirectoryEntry::empty(),
            DirectoryEntry::new("B", ObjectType::Stream),
            DirectoryEntry::new("A", ObjectType::Stream),
            DirectoryEntry::new("D", ObjectType::Stream),
            DirectoryEntry::new("E", ObjectType::Stream),
        ];
        entries[1].set_color(ColorFlag::Black);
        entries[1].set_left_sibling(StreamId::new(2));
        entries[1].set_right_sibling(StreamId::new(3));

        entries[2].set_color(ColorFlag::Black); // Black uncle
        entries[3].set_color(ColorFlag::Red); // Red parent

        let mut parents = vec![
            StreamId::NO_STREAM,
            StreamId::NO_STREAM,
            StreamId::new(1),
            StreamId::new(1),
            StreamId::NO_STREAM,
        ];

        let root = CfbWriter::rb_insert(
            StreamId::new(1),
            StreamId::new(4),
            &mut entries,
            &mut parents,
        );

        assert_eq!(root, StreamId::new(3));
        assert_eq!(entries[3].color(), ColorFlag::Black);
        assert_eq!(entries[1].color(), ColorFlag::Red);
    }
}

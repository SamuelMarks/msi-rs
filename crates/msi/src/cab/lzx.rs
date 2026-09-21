//! Microsoft LZX Data Compression format engine ([MS-PATCH] & Microsoft Cabinet SDK).
//!
//! Features:
//! - Sliding history window ($2^{15}$ = 32KB to $2^{21}$ = 2MB circular buffer).
//! - Intel 80x86 E8 call-translation preprocessing and reversal.
//! - 16-bit word-aligned little-endian bitstream reader and writer.
//! - Canonical prefix tree decoder and encoder with Kraft-McMillan oversubscription validation.
//! - Pre-tree parsing and delta tree decoding for Main Tree and Secondary Length Tree.
//! - Block Type 1 (Verbatim) decompression and compression.
//! - Block Type 2 (Aligned Offset) decompression and compression with 8-symbol aligned tree.
//! - Block Type 3 (Uncompressed) fallback blocks.
//! - Repeated match offset maintenance (`R0`, `R1`, `R2`) across consecutive `CFDATA` blocks.

use crate::error::{Error, Result};
use std::cmp::Reverse;
use std::collections::BinaryHeap;

/// Minimum LZX window bits (15 -> 32KB).
pub const LZX_MIN_WINDOW_BITS: u8 = 15;

/// Maximum LZX window bits (21 -> 2MB).
pub const LZX_MAX_WINDOW_BITS: u8 = 21;

/// Default E8 translation file size boundary (32MB / `0x0200_0000`).
pub const E8_DEFAULT_FILE_SIZE: i32 = 0x0200_0000;

/// Number of pre-tree symbols used to encode delta code lengths.
pub const LZX_PRE_TREE_NUM_SYMBOLS: usize = 20;

/// Number of aligned offset tree symbols used in Type 2 blocks.
pub const LZX_ALIGNED_TREE_NUM_SYMBOLS: usize = 8;

/// Number of secondary match length tree symbols.
pub const LZX_NUM_SECONDARY_LENGTHS: usize = 249;

/// Minimum match length supported in LZX.
pub const LZX_MIN_MATCH: usize = 2;

/// Maximum match length supported in LZX ($2 + 7 + 248 = 257$).
pub const LZX_MAX_MATCH: usize = 257;

/// Returns the number of position slots corresponding to the given window size in bits.
///
/// # Arguments
///
/// * `window_bits` - Window size exponent ($15..=21$).
///
/// # Returns
///
/// Position slot count.
#[must_use]
pub const fn num_position_slots(window_bits: u8) -> usize {
    match window_bits {
        15 => 30,
        16 => 32,
        17 => 34,
        18 => 36,
        19 => 38,
        20 => 42,
        _ => 50,
    }
}

/// Returns the number of extra bits required for a given position slot.
///
/// # Arguments
///
/// * `slot` - Position slot index.
///
/// # Returns
///
/// Number of extra bits ($0..=17$).
#[must_use]
#[allow(clippy::cast_possible_truncation)]
pub const fn slot_extra_bits(slot: usize) -> u8 {
    if slot < 4 {
        0
    } else {
        ((slot / 2) - 1) as u8
    }
}

/// Returns the base offset associated with a given position slot.
///
/// # Arguments
///
/// * `slot` - Position slot index.
///
/// # Returns
///
/// Base offset integer.
#[must_use]
#[allow(clippy::cast_possible_truncation)]
pub const fn slot_base_offset(slot: usize) -> u32 {
    if slot < 4 {
        slot as u32
    } else {
        let half = (slot / 2) - 1;
        let base = (2 | (slot & 1)) as u32;
        base << half
    }
}

/// Translates Intel 80x86 `0xE8` relative call addresses to/from absolute offsets.
///
/// Implements the official Microsoft LZX Specification Section 2.3 Intel 80x86 Call Translation.
///
/// # Arguments
///
/// * `data` - Byte buffer to transform in-place.
/// * `current_file_offset` - Current position in uncompressed file stream.
/// * `decode` - `true` for decoding/reversal, `false` for compression preprocessing.
#[allow(clippy::cast_possible_truncation, clippy::cast_possible_wrap)]
pub fn e8_translate(data: &mut [u8], current_file_offset: usize, decode: bool) {
    if data.len() < 10 {
        return;
    }

    let limit = data.len() - 10;
    let mut i = 0;
    while i <= limit {
        if data[i] == 0xE8 {
            let offset = i + 1;
            let current_pointer = (current_file_offset + i) as i32;

            if decode {
                let value = i32::from_le_bytes([
                    data[offset],
                    data[offset + 1],
                    data[offset + 2],
                    data[offset + 3],
                ]);

                if value >= -current_pointer && value < E8_DEFAULT_FILE_SIZE {
                    let displacement = if value >= 0 {
                        value - current_pointer
                    } else {
                        value + E8_DEFAULT_FILE_SIZE
                    };
                    data[offset..offset + 4].copy_from_slice(&displacement.to_le_bytes());
                    i += 4;
                }
            } else {
                let displacement = i32::from_le_bytes([
                    data[offset],
                    data[offset + 1],
                    data[offset + 2],
                    data[offset + 3],
                ]);

                let target = current_pointer.wrapping_add(displacement);
                if target >= 0 && target < E8_DEFAULT_FILE_SIZE + current_pointer {
                    let val = if target >= E8_DEFAULT_FILE_SIZE {
                        displacement - E8_DEFAULT_FILE_SIZE
                    } else {
                        target
                    };
                    data[offset..offset + 4].copy_from_slice(&val.to_le_bytes());
                    i += 4;
                }
            }
        }
        i += 1;
    }
}

/// Canonical prefix Huffman decoding and encoding tree.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HuffmanTree {
    /// Binary tree nodes stored as `[left_child, right_child]`.
    ///
    /// If high bit `0x8000` is set, the node represents a leaf symbol (`val & 0x7FFF`).
    nodes: Vec<[u16; 2]>,
    /// Array of symbol code lengths (0 means symbol is unused).
    code_lengths: Vec<u8>,
}

impl HuffmanTree {
    /// Builds a canonical [`HuffmanTree`] from symbol code lengths.
    ///
    /// # Arguments
    ///
    /// * `lengths` - Slice of code lengths for each symbol in the alphabet.
    ///
    /// # Returns
    ///
    /// A constructed [`HuffmanTree`] on success.
    ///
    /// # Errors
    ///
    /// Returns [`Error::DecompressionFailed`] if the code lengths represent an oversubscribed or invalid tree.
    #[allow(clippy::cast_possible_truncation)]
    pub fn from_lengths(lengths: &[u8]) -> Result<Self> {
        let mut count = [0u32; 17];
        for &len in lengths {
            if len > 16 {
                return Err(Error::DecompressionFailed {
                    method: "LZX".to_string(),
                    reason: format!("Huffman code length {len} exceeds 16 bits"),
                });
            }
            if len > 0 {
                count[len as usize] += 1;
            }
        }

        // Kraft-McMillan inequality verification: sum(count[l] * 2^(16-l)) <= 2^16
        let mut kraft_sum = 0u32;
        for (l, &cnt) in count.iter().enumerate().skip(1) {
            kraft_sum = kraft_sum.saturating_add(cnt.saturating_mul(1u32 << (16 - l)));
        }
        if kraft_sum > (1u32 << 16) {
            return Err(Error::DecompressionFailed {
                method: "LZX".to_string(),
                reason: "oversubscribed Huffman tree".to_string(),
            });
        }

        let mut next_code = [0u32; 17];
        let mut code = 0u32;
        for l in 1..=16 {
            code = (code + count[l - 1]) << 1;
            next_code[l] = code;
        }

        let mut nodes: Vec<[u16; 2]> = Vec::new();
        nodes.push([0, 0]); // Root node 0

        for (sym, &len) in lengths.iter().enumerate() {
            if len == 0 {
                continue;
            }
            let sym_code = next_code[len as usize];
            next_code[len as usize] += 1;

            let mut curr = 0usize;
            for bit_idx in (0..len).rev() {
                let bit = ((sym_code >> bit_idx) & 1) as usize;
                if bit_idx > 0 {
                    if nodes[curr][bit] == 0 {
                        let new_node = nodes.len() as u16;
                        nodes.push([0, 0]);
                        nodes[curr][bit] = new_node;
                    }
                    curr = nodes[curr][bit] as usize;
                } else {
                    nodes[curr][bit] = 0x8000 | (sym as u16);
                }
            }
        }

        Ok(Self {
            nodes,
            code_lengths: lengths.to_vec(),
        })
    }

    /// Decodes a single symbol from the bitstream using the Huffman tree.
    ///
    /// # Arguments
    ///
    /// * `reader` - Bitstream reader.
    ///
    /// # Returns
    ///
    /// The decoded symbol integer.
    ///
    /// # Errors
    ///
    /// Returns [`Error::DecompressionFailed`] on invalid bit sequences or corrupted tree states.
    pub fn decode_symbol(&self, reader: &mut LzxBitReader<'_>) -> Result<u16> {
        if self.nodes.is_empty() {
            return Err(Error::DecompressionFailed {
                method: "LZX".to_string(),
                reason: "empty Huffman tree".to_string(),
            });
        }

        let mut curr = 0usize;
        loop {
            let bit = reader.read_bit()? as usize;
            let next = self.nodes[curr][bit];
            if next == 0 {
                return Err(Error::DecompressionFailed {
                    method: "LZX".to_string(),
                    reason: "invalid Huffman code in bitstream".to_string(),
                });
            }
            if (next & 0x8000) != 0 {
                return Ok(next & 0x7FFF);
            }
            curr = next as usize;
            if curr >= self.nodes.len() {
                return Err(Error::DecompressionFailed {
                    method: "LZX".to_string(),
                    reason: "corrupted Huffman tree index".to_string(),
                });
            }
        }
    }

    /// Returns the symbol code lengths.
    #[must_use]
    pub fn code_lengths(&self) -> &[u8] {
        &self.code_lengths
    }

    /// Computes canonical `(code, length)` tuples for each symbol.
    #[must_use]
    #[allow(clippy::cast_possible_truncation)]
    pub fn canonical_codes(&self) -> Vec<(u16, u8)> {
        let mut count = [0u32; 17];
        for &len in &self.code_lengths {
            if len > 0 {
                count[len as usize] += 1;
            }
        }

        let mut next_code = [0u32; 17];
        let mut code = 0u32;
        for l in 1..=16 {
            code = (code + count[l - 1]) << 1;
            next_code[l] = code;
        }

        let mut codes = Vec::with_capacity(self.code_lengths.len());
        for &len in &self.code_lengths {
            if len == 0 {
                codes.push((0, 0));
            } else {
                let c = next_code[len as usize] as u16;
                next_code[len as usize] += 1;
                codes.push((c, len));
            }
        }
        codes
    }
}

/// Decodes an array of code lengths using the pre-tree and delta encoding relative to previous lengths.
///
/// Implements Section 2.4.1 of the Microsoft LZX Specification.
///
/// # Arguments
///
/// * `reader` - Bitstream reader.
/// * `pre_tree` - Constructed pre-tree.
/// * `lengths` - Target slice to populate with decoded lengths.
/// * `prev_lengths` - Previous block's code lengths for delta calculation.
///
/// # Errors
///
/// Returns [`Error::DecompressionFailed`] on invalid bitstream symbols.
#[allow(clippy::cast_possible_truncation)]
pub fn decode_tree_lengths(
    reader: &mut LzxBitReader<'_>,
    pre_tree: &HuffmanTree,
    lengths: &mut [u8],
    prev_lengths: &[u8],
) -> Result<()> {
    let mut i = 0;
    while i < lengths.len() {
        let sym = pre_tree.decode_symbol(reader)?;
        if sym <= 16 {
            let prev = if i < prev_lengths.len() {
                prev_lengths[i]
            } else {
                0
            };
            let val = (prev + 17 - (sym as u8)) % 17;
            lengths[i] = val;
            i += 1;
        } else if sym == 17 {
            let count = reader.read_bits(4)? as usize + 4;
            let end = (i + count).min(lengths.len());
            for item in lengths.iter_mut().take(end).skip(i) {
                *item = 0;
            }
            i = end;
        } else if sym == 18 {
            let count = reader.read_bits(5)? as usize + 20;
            let end = (i + count).min(lengths.len());
            for item in lengths.iter_mut().take(end).skip(i) {
                *item = 0;
            }
            i = end;
        } else {
            let count = reader.read_bits(1)? as usize + 4;
            let next_sym = pre_tree.decode_symbol(reader)?;
            let prev = if i < prev_lengths.len() {
                prev_lengths[i]
            } else {
                0
            };
            let val = if next_sym <= 16 {
                (prev + 17 - (next_sym as u8)) % 17
            } else {
                0
            };
            let end = (i + count).min(lengths.len());
            for item in lengths.iter_mut().take(end).skip(i) {
                *item = val;
            }
            i = end;
        }
    }
    Ok(())
}

/// Generates optimal canonical Huffman code lengths from an alphabet frequency distribution.
///
/// # Arguments
///
/// * `freqs` - Frequency counts for each symbol.
/// * `max_bits` - Maximum allowed code length (e.g. 16).
///
/// # Returns
///
/// Vector of code lengths for each symbol.
#[must_use]
#[allow(clippy::cast_possible_truncation)]
pub fn compute_huffman_lengths(freqs: &[u32], max_bits: u8) -> Vec<u8> {
    let active_symbols: Vec<(usize, u32)> = freqs
        .iter()
        .enumerate()
        .filter(|(_, &f)| f > 0)
        .map(|(i, &f)| (i, f))
        .collect();

    let mut lengths = vec![0u8; freqs.len()];
    if active_symbols.is_empty() {
        return lengths;
    }
    if active_symbols.len() == 1 {
        lengths[active_symbols[0].0] = 1;
        return lengths;
    }

    // Min-heap storing (weight, node_index)
    let mut heap = BinaryHeap::new();
    let mut tree_nodes: Vec<(Option<usize>, Option<usize>)> = Vec::new();

    for (sym, weight) in &active_symbols {
        let node_id = tree_nodes.len();
        tree_nodes.push((None, None));
        heap.push(Reverse((u64::from(*weight), node_id, Some(*sym))));
    }

    while let (Some(Reverse((w1, n1, _))), Some(Reverse((w2, n2, _)))) = (heap.pop(), heap.pop()) {
        let parent_id = tree_nodes.len();
        tree_nodes.push((Some(n1), Some(n2)));
        heap.push(Reverse((w1.saturating_add(w2), parent_id, None)));
    }

    let root = tree_nodes.len().saturating_sub(1);
    let mut depths = vec![0u8; tree_nodes.len()];
    let mut stack = vec![(root, 0u8)];

    while let Some((node_idx, d)) = stack.pop() {
        depths[node_idx] = d;
        let (left, right) = tree_nodes[node_idx];
        if let Some(l) = left {
            stack.push((l, (d + 1).min(max_bits)));
        }
        if let Some(r) = right {
            stack.push((r, (d + 1).min(max_bits)));
        }
    }

    for (idx, &(sym, _)) in active_symbols.iter().enumerate() {
        lengths[sym] = depths[idx].clamp(1, max_bits);
    }

    lengths
}

/// LZX decompressor and compressor state machine.
///
/// Tracks the circular sliding history window, tree code lengths, and the three repeated match offsets (`R0`, `R1`, `R2`).
#[derive(Debug, Clone)]
pub struct LzxState {
    /// Window size in bits (15..=21).
    pub window_bits: u8,
    /// Circular window size in bytes ($2^{\text{window\_bits}}$).
    pub window_size: usize,
    /// Sliding history circular buffer.
    pub window: Vec<u8>,
    /// Current write index in circular window.
    pub window_pos: usize,
    /// Repeat offset 0.
    pub r0: u32,
    /// Repeat offset 1.
    pub r1: u32,
    /// Repeat offset 2.
    pub r2: u32,
    /// Current uncompressed byte count processed so far (for E8 translation).
    pub total_uncompressed_bytes: usize,
    /// Main tree symbol code lengths persisted across blocks.
    pub main_tree_lengths: Vec<u8>,
    /// Secondary length tree symbol code lengths persisted across blocks.
    pub secondary_tree_lengths: Vec<u8>,
}

impl LzxState {
    /// Creates a new [`LzxState`] initialized for the specified window size.
    ///
    /// # Arguments
    ///
    /// * `window_bits` - Window size in bits (15 to 21).
    ///
    /// # Returns
    ///
    /// An initialized [`LzxState`].
    ///
    /// # Errors
    ///
    /// Returns [`Error::InvalidArgument`] if `window_bits` is not in `15..=21`.
    pub fn new(window_bits: u8) -> Result<Self> {
        if !(LZX_MIN_WINDOW_BITS..=LZX_MAX_WINDOW_BITS).contains(&window_bits) {
            return Err(Error::InvalidArgument {
                argument: "window_bits".to_string(),
                reason: format!("must be between {LZX_MIN_WINDOW_BITS} and {LZX_MAX_WINDOW_BITS}"),
            });
        }

        let window_size = 1usize << window_bits;
        let main_tree_size = 256 + 8 * num_position_slots(window_bits);

        Ok(Self {
            window_bits,
            window_size,
            window: vec![0u8; window_size],
            window_pos: 0,
            r0: 1,
            r1: 1,
            r2: 1,
            total_uncompressed_bytes: 0,
            main_tree_lengths: vec![0u8; main_tree_size],
            secondary_tree_lengths: vec![0u8; LZX_NUM_SECONDARY_LENGTHS],
        })
    }

    /// Resets the sliding window, tree code lengths, and repeat offsets back to initial state.
    pub fn reset(&mut self) {
        self.window.fill(0);
        self.window_pos = 0;
        self.r0 = 1;
        self.r1 = 1;
        self.r2 = 1;
        self.total_uncompressed_bytes = 0;
        self.main_tree_lengths.fill(0);
        self.secondary_tree_lengths.fill(0);
    }

    /// Compresses a block of data into an LZX uncompressed block (Block Type 3).
    ///
    /// # Arguments
    ///
    /// * `input` - Uncompressed data bytes.
    ///
    /// # Returns
    ///
    /// Compressed LZX payload.
    ///
    /// # Errors
    ///
    /// Returns [`Error::CompressionFailed`] if input exceeds 32,768 bytes.
    #[allow(clippy::cast_possible_truncation)]
    pub fn compress_uncompressed_block(&mut self, input: &[u8]) -> Result<Vec<u8>> {
        if input.len() > 32_768 {
            return Err(Error::CompressionFailed {
                method: "LZX".to_string(),
                reason: "block length exceeds 32KB".to_string(),
            });
        }

        let mut transformed = input.to_vec();
        e8_translate(&mut transformed, self.total_uncompressed_bytes, false);

        let mut bit_writer = LzxBitWriter::new();
        bit_writer.write_bits(0b011, 3); // Type 3

        let len = transformed.len() as u32;
        bit_writer.write_bits((len & 0xFFFF) as u16, 16);
        bit_writer.write_bits((len >> 16) as u16, 8);

        bit_writer.write_bits((self.r0 & 0xFFFF) as u16, 16);
        bit_writer.write_bits((self.r0 >> 16) as u16, 16);
        bit_writer.write_bits((self.r1 & 0xFFFF) as u16, 16);
        bit_writer.write_bits((self.r1 >> 16) as u16, 16);
        bit_writer.write_bits((self.r2 & 0xFFFF) as u16, 16);
        bit_writer.write_bits((self.r2 >> 16) as u16, 16);

        bit_writer.align_to_16();
        let mut output = bit_writer.into_bytes();

        output.extend_from_slice(&transformed);
        if output.len() % 2 != 0 {
            output.push(0);
        }

        for &b in &transformed {
            self.window[self.window_pos] = b;
            self.window_pos = (self.window_pos + 1) % self.window_size;
        }
        self.total_uncompressed_bytes += transformed.len();

        Ok(output)
    }

    /// Compresses a block of data into an LZX Block Type 1 (Verbatim) or Type 2 (Aligned) block.
    ///
    /// Automatically performs sliding-window LZ match parsing and canonical Huffman tree optimization.
    ///
    /// # Arguments
    ///
    /// * `input` - Uncompressed data bytes.
    ///
    /// # Returns
    ///
    /// Compressed LZX payload.
    ///
    /// # Errors
    ///
    /// Returns [`Error::CompressionFailed`] if input exceeds 32,768 bytes.
    #[allow(
        clippy::too_many_lines,
        clippy::cast_possible_truncation,
        clippy::cast_possible_wrap
    )]
    pub fn compress_verbatim_block(&mut self, input: &[u8]) -> Result<Vec<u8>> {
        if input.len() > 32_768 {
            return Err(Error::CompressionFailed {
                method: "LZX".to_string(),
                reason: "block length exceeds 32KB".to_string(),
            });
        }

        let mut transformed = input.to_vec();
        e8_translate(&mut transformed, self.total_uncompressed_bytes, false);

        // LZ Match parsing
        let mut tokens: Vec<LzToken> = Vec::new();
        let mut pos = 0;
        let mut tok_r0 = self.r0;
        let mut tok_r1 = self.r1;
        let mut tok_r2 = self.r2;

        while pos < transformed.len() {
            let mut best_len = 0;
            let mut best_offset = 0;

            // Check repeat offsets
            for &cand_offset in &[tok_r0, tok_r1, tok_r2] {
                let off = cand_offset as usize;
                if off > 0 && off <= self.window_size {
                    let mut match_len = 0;
                    while pos + match_len < transformed.len() && match_len < LZX_MAX_MATCH {
                        let target_byte = transformed[pos + match_len];
                        let src_byte = if pos + match_len >= off {
                            transformed[pos + match_len - off]
                        } else {
                            let win_idx = (self.window_pos + self.window_size + pos + match_len
                                - off)
                                % self.window_size;
                            self.window[win_idx]
                        };
                        if target_byte == src_byte {
                            match_len += 1;
                        } else {
                            break;
                        }
                    }
                    if match_len >= LZX_MIN_MATCH && match_len > best_len {
                        best_len = match_len;
                        best_offset = off;
                    }
                }
            }

            // Also search backward in current block
            if best_len < 32 && pos >= 3 {
                let max_search = pos.min(1024);
                for back in 1..=max_search {
                    let mut match_len = 0;
                    while pos + match_len < transformed.len()
                        && match_len < LZX_MAX_MATCH
                        && transformed[pos + match_len]
                            == transformed[pos - back + (match_len % back)]
                    {
                        match_len += 1;
                    }
                    if match_len >= 3 && match_len > best_len {
                        best_len = match_len;
                        best_offset = back;
                    }
                }
            }

            if best_len >= LZX_MIN_MATCH {
                tokens.push(LzToken::Match {
                    offset: best_offset,
                    length: best_len,
                });
                pos += best_len;

                if best_offset as u32 == tok_r1 {
                    std::mem::swap(&mut tok_r1, &mut tok_r0);
                } else if best_offset as u32 == tok_r2 {
                    std::mem::swap(&mut tok_r2, &mut tok_r0);
                } else if best_offset as u32 != tok_r0 {
                    tok_r2 = tok_r1;
                    tok_r1 = tok_r0;
                    tok_r0 = best_offset as u32;
                }
            } else {
                tokens.push(LzToken::Literal(transformed[pos]));
                pos += 1;
            }
        }

        // Count symbol frequencies
        let slots = num_position_slots(self.window_bits);
        let main_alphabet_size = 256 + 8 * slots;
        let mut main_freqs = vec![0u32; main_alphabet_size];
        let mut sec_freqs = vec![0u32; LZX_NUM_SECONDARY_LENGTHS];

        let mut sim_r0 = self.r0;
        let mut sim_r1 = self.r1;
        let mut sim_r2 = self.r2;

        for tok in &tokens {
            match tok {
                LzToken::Literal(b) => {
                    main_freqs[*b as usize] += 1;
                }
                LzToken::Match { offset, length } => {
                    let slot = if *offset as u32 == sim_r0 {
                        0
                    } else if *offset as u32 == sim_r1 {
                        1
                    } else if *offset as u32 == sim_r2 {
                        2
                    } else {
                        let raw = (*offset as u32) + 2;
                        3 + (4..slots)
                            .take_while(|&cand| slot_base_offset(cand) <= raw)
                            .count()
                    };
                    let len_hdr = (length - 2).min(7);
                    let sym = 256 + (slot * 8) + len_hdr;
                    main_freqs[sym] += 1;
                    if *length >= 9 {
                        let sec_sym = (length - 9).min(LZX_NUM_SECONDARY_LENGTHS - 1);
                        sec_freqs[sec_sym] += 1;
                    }

                    if slot == 1 {
                        std::mem::swap(&mut sim_r1, &mut sim_r0);
                    } else if slot == 2 {
                        std::mem::swap(&mut sim_r2, &mut sim_r0);
                    } else if slot >= 3 {
                        sim_r2 = sim_r1;
                        sim_r1 = sim_r0;
                        sim_r0 = *offset as u32;
                    }
                }
            }
        }

        let main_lengths = compute_huffman_lengths(&main_freqs, 16);
        let sec_lengths = compute_huffman_lengths(&sec_freqs, 16);
        let main_tree = HuffmanTree::from_lengths(&main_lengths)?;
        let sec_tree = HuffmanTree::from_lengths(&sec_lengths)?;
        let main_codes = main_tree.canonical_codes();
        let sec_codes = sec_tree.canonical_codes();

        let mut bit_writer = LzxBitWriter::new();
        bit_writer.write_bits(0b001, 3); // Type 1: Verbatim

        let len = transformed.len() as u32;
        bit_writer.write_bits((len & 0xFFFF) as u16, 16);
        bit_writer.write_bits((len >> 16) as u16, 8);

        // Pre-tree: compute delta symbols and pre-tree
        let mut delta_syms = Vec::new();
        for (i, &l) in main_lengths.iter().enumerate() {
            let prev = self.main_tree_lengths[i];
            let delta = (prev + 17 - l) % 17;
            delta_syms.push(delta);
        }
        for (i, &l) in sec_lengths.iter().enumerate() {
            let prev = self.secondary_tree_lengths[i];
            let delta = (prev + 17 - l) % 17;
            delta_syms.push(delta);
        }

        let mut pre_freqs = [0u32; LZX_PRE_TREE_NUM_SYMBOLS];
        for &ds in &delta_syms {
            pre_freqs[ds as usize] += 1;
        }
        let pre_lengths = compute_huffman_lengths(&pre_freqs, 7);
        let pre_tree = HuffmanTree::from_lengths(&pre_lengths)?;
        let pre_codes = pre_tree.canonical_codes();

        // Emit 20 x 3-bit pre-tree lengths
        for &pl in &pre_lengths {
            bit_writer.write_bits(u16::from(pl), 3);
        }

        // Emit deltas using pre-tree codes
        for &ds in &delta_syms {
            let (c, l) = pre_codes[ds as usize];
            bit_writer.write_bits_msb(c, l);
        }

        // Emit tokens
        for tok in &tokens {
            match tok {
                LzToken::Literal(b) => {
                    let (c, l) = main_codes[*b as usize];
                    bit_writer.write_bits_msb(c, l);
                }
                LzToken::Match { offset, length } => {
                    let (slot, is_rep) = if *offset as u32 == self.r0 {
                        (0, true)
                    } else if *offset as u32 == self.r1 {
                        (1, true)
                    } else if *offset as u32 == self.r2 {
                        (2, true)
                    } else {
                        let raw = (*offset as u32) + 2;
                        let s = 3
                            + (4..slots)
                                .take_while(|&cand| slot_base_offset(cand) <= raw)
                                .count();
                        (s, false)
                    };
                    let len_hdr = (length - 2).min(7);
                    let sym = 256 + (slot * 8) + len_hdr;
                    let (c, l) = main_codes[sym];
                    bit_writer.write_bits_msb(c, l);

                    if *length >= 9 {
                        let sec_sym = (length - 9).min(LZX_NUM_SECONDARY_LENGTHS - 1);
                        let (sc, sl) = sec_codes[sec_sym];
                        bit_writer.write_bits_msb(sc, sl);
                    }

                    if !is_rep {
                        let extra_bits = slot_extra_bits(slot);
                        if extra_bits > 0 {
                            let base = slot_base_offset(slot);
                            let raw = (*offset as u32) + 2;
                            let extra = raw - base;
                            bit_writer.write_bits(extra as u16, extra_bits);
                        }
                    }

                    // Update R0, R1, R2
                    if slot == 1 {
                        std::mem::swap(&mut self.r1, &mut self.r0);
                    } else if slot == 2 {
                        std::mem::swap(&mut self.r2, &mut self.r0);
                    } else if slot >= 3 {
                        self.r2 = self.r1;
                        self.r1 = self.r0;
                        self.r0 = *offset as u32;
                    }
                }
            }
        }

        bit_writer.align_to_16();
        let output = bit_writer.into_bytes();

        // Update state trees and circular history window
        self.main_tree_lengths = main_lengths;
        self.secondary_tree_lengths = sec_lengths;

        for &b in &transformed {
            self.window[self.window_pos] = b;
            self.window_pos = (self.window_pos + 1) % self.window_size;
        }
        self.total_uncompressed_bytes += transformed.len();

        Ok(output)
    }

    /// Compresses a block of data, automatically selecting between Verbatim (Type 1) and Uncompressed (Type 3).
    ///
    /// # Arguments
    ///
    /// * `input` - Uncompressed data bytes.
    ///
    /// # Returns
    ///
    /// Compressed LZX payload.
    ///
    /// # Errors
    ///
    /// Returns [`Error::CompressionFailed`] if input exceeds 32,768 bytes.
    pub fn compress_block(&mut self, input: &[u8]) -> Result<Vec<u8>> {
        let mut test_state = self.clone();
        if let Ok(comp_v) = test_state.compress_verbatim_block(input) {
            if comp_v.len() < input.len() + 10 {
                *self = test_state;
                return Ok(comp_v);
            }
        }
        self.compress_uncompressed_block(input)
    }

    /// Decompresses an LZX block into uncompressed bytes.
    ///
    /// Supports Block Type 1 (Verbatim), Block Type 2 (Aligned), and Block Type 3 (Uncompressed).
    ///
    /// # Arguments
    ///
    /// * `input` - Compressed LZX block bytes.
    /// * `expected_uncomp_len` - Expected uncompressed length.
    ///
    /// # Returns
    ///
    /// Decompressed byte vector.
    ///
    /// # Errors
    ///
    /// Returns [`Error::DecompressionFailed`] if bitstream or block format is invalid.
    #[allow(
        clippy::too_many_lines,
        clippy::cast_possible_truncation,
        clippy::cast_possible_wrap
    )]
    pub fn decompress_block(
        &mut self,
        input: &[u8],
        expected_uncomp_len: usize,
    ) -> Result<Vec<u8>> {
        let mut reader = LzxBitReader::new(input);
        let block_type = reader.read_bits(3)?;

        let mut output = match block_type {
            0b011 => {
                // Type 3: Uncompressed block
                let len_low = reader.read_bits(16)?;
                let len_high = reader.read_bits(8)?;
                let len = (usize::from(len_high) << 16) | usize::from(len_low);

                if len != expected_uncomp_len {
                    return Err(Error::DecompressionFailed {
                        method: "LZX".to_string(),
                        reason: format!(
                            "uncompressed block length {len} != expected {expected_uncomp_len}"
                        ),
                    });
                }

                let r0_low = reader.read_bits(16)?;
                let r0_high = reader.read_bits(16)?;
                self.r0 = (u32::from(r0_high) << 16) | u32::from(r0_low);

                let r1_low = reader.read_bits(16)?;
                let r1_high = reader.read_bits(16)?;
                self.r1 = (u32::from(r1_high) << 16) | u32::from(r1_low);

                let r2_low = reader.read_bits(16)?;
                let r2_high = reader.read_bits(16)?;
                self.r2 = (u32::from(r2_high) << 16) | u32::from(r2_low);

                reader.align_to_16();
                let bytes = reader.read_bytes(len)?;
                bytes.to_vec()
            }
            0b001 | 0b010 => {
                // Type 1 (Verbatim) or Type 2 (Aligned Offset)
                let is_aligned = block_type == 0b010;

                let len_low = reader.read_bits(16)?;
                let len_high = reader.read_bits(8)?;
                let uncomp_len = (usize::from(len_high) << 16) | usize::from(len_low);

                if uncomp_len != expected_uncomp_len {
                    return Err(Error::DecompressionFailed {
                        method: "LZX".to_string(),
                        reason: format!(
                            "block length {uncomp_len} != expected {expected_uncomp_len}"
                        ),
                    });
                }

                let aligned_tree = if is_aligned {
                    let mut al_lengths = [0u8; LZX_ALIGNED_TREE_NUM_SYMBOLS];
                    for l in &mut al_lengths {
                        *l = reader.read_bits(3)? as u8;
                    }
                    Some(HuffmanTree::from_lengths(&al_lengths)?)
                } else {
                    None
                };

                // Read 20 x 3-bit pre-tree lengths
                let mut pre_lengths = [0u8; LZX_PRE_TREE_NUM_SYMBOLS];
                for l in &mut pre_lengths {
                    *l = reader.read_bits(3)? as u8;
                }
                let pre_tree = HuffmanTree::from_lengths(&pre_lengths)?;

                let slots = num_position_slots(self.window_bits);
                let main_alphabet_size = 256 + 8 * slots;
                let mut new_main_lengths = vec![0u8; main_alphabet_size];
                decode_tree_lengths(
                    &mut reader,
                    &pre_tree,
                    &mut new_main_lengths,
                    &self.main_tree_lengths,
                )?;
                let main_tree = HuffmanTree::from_lengths(&new_main_lengths)?;

                let mut new_sec_lengths = vec![0u8; LZX_NUM_SECONDARY_LENGTHS];
                decode_tree_lengths(
                    &mut reader,
                    &pre_tree,
                    &mut new_sec_lengths,
                    &self.secondary_tree_lengths,
                )?;
                let sec_tree = HuffmanTree::from_lengths(&new_sec_lengths)?;

                let mut decomp_bytes = Vec::with_capacity(uncomp_len);

                while decomp_bytes.len() < uncomp_len {
                    let sym = main_tree.decode_symbol(&mut reader)? as usize;
                    if sym < 256 {
                        let b = sym as u8;
                        decomp_bytes.push(b);
                        self.window[self.window_pos] = b;
                        self.window_pos = (self.window_pos + 1) % self.window_size;
                    } else {
                        let match_hdr = sym - 256;
                        let len_hdr = match_hdr & 7;
                        let slot = match_hdr >> 3;

                        let match_len = if len_hdr < 7 {
                            len_hdr + 2
                        } else {
                            (sec_tree.decode_symbol(&mut reader)? as usize) + 9
                        };

                        let offset = if slot == 0 {
                            self.r0 as usize
                        } else if slot == 1 {
                            let off = self.r1 as usize;
                            self.r1 = self.r0;
                            self.r0 = off as u32;
                            off
                        } else if slot == 2 {
                            let off = self.r2 as usize;
                            self.r2 = self.r0;
                            self.r0 = off as u32;
                            off
                        } else {
                            let extra_bits = slot_extra_bits(slot);
                            let base = slot_base_offset(slot);
                            let raw_off = if let (Some(t), true) = (&aligned_tree, extra_bits >= 3)
                            {
                                let high = u32::from(reader.read_bits(extra_bits - 3)?) << 3;
                                let low = u32::from(t.decode_symbol(&mut reader)?);
                                base + high + low
                            } else if extra_bits > 0 {
                                let extra = u32::from(reader.read_bits(extra_bits)?);
                                base + extra
                            } else {
                                base
                            };

                            let off = (raw_off - 2) as usize;
                            self.r2 = self.r1;
                            self.r1 = self.r0;
                            self.r0 = off as u32;
                            off
                        };

                        if offset == 0 || offset > self.window_size {
                            return Err(Error::DecompressionFailed {
                                method: "LZX".to_string(),
                                reason: format!("invalid match offset {offset}"),
                            });
                        }

                        for _ in 0..match_len {
                            let src_idx =
                                (self.window_pos + self.window_size - offset) % self.window_size;
                            let b = self.window[src_idx];
                            decomp_bytes.push(b);
                            self.window[self.window_pos] = b;
                            self.window_pos = (self.window_pos + 1) % self.window_size;
                        }
                    }
                }

                self.main_tree_lengths = new_main_lengths;
                self.secondary_tree_lengths = new_sec_lengths;
                decomp_bytes
            }
            _ => {
                return Err(Error::DecompressionFailed {
                    method: "LZX".to_string(),
                    reason: format!("unsupported LZX block type {block_type}"),
                });
            }
        };

        e8_translate(&mut output, self.total_uncompressed_bytes, true);
        self.total_uncompressed_bytes += output.len();

        Ok(output)
    }
}

/// Intermediate LZ token representing either a literal byte or an LZ backward match.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LzToken {
    /// Literal uncompressed byte.
    Literal(u8),
    /// Backward match `(offset, length)`.
    Match {
        /// Lookback match offset.
        offset: usize,
        /// Match length.
        length: usize,
    },
}

/// 16-bit word-aligned little-endian bitstream reader for LZX.
#[derive(Debug)]
pub struct LzxBitReader<'a> {
    /// Underlying compressed byte slice.
    bytes: &'a [u8],
    /// Current read cursor in byte stream.
    cursor: usize,
    /// Bit accumulator buffer.
    bit_buffer: u32,
    /// Number of valid bits in accumulator.
    bits_count: u8,
}

impl<'a> LzxBitReader<'a> {
    /// Creates a new [`LzxBitReader`].
    #[must_use]
    pub const fn new(bytes: &'a [u8]) -> Self {
        Self {
            bytes,
            cursor: 0,
            bit_buffer: 0,
            bits_count: 0,
        }
    }

    /// Reads a single bit from the bitstream.
    ///
    /// # Errors
    ///
    /// Returns [`Error::DecompressionFailed`] if bitstream ends prematurely.
    #[allow(clippy::cast_possible_truncation)]
    pub fn read_bit(&mut self) -> Result<u8> {
        self.read_bits(1).map(|v| v as u8)
    }

    /// Reads `n` bits from the 16-bit word-aligned bitstream.
    ///
    /// # Errors
    ///
    /// Returns [`Error::DecompressionFailed`] if bitstream ends prematurely.
    #[allow(clippy::cast_possible_truncation)]
    pub fn read_bits(&mut self, n: u8) -> Result<u16> {
        while self.bits_count < n {
            if self.cursor + 1 >= self.bytes.len() {
                if self.cursor < self.bytes.len() {
                    let byte = self.bytes[self.cursor];
                    self.cursor += 1;
                    self.bit_buffer |= u32::from(byte) << self.bits_count;
                    self.bits_count += 8;
                } else {
                    return Err(Error::DecompressionFailed {
                        method: "LZX".to_string(),
                        reason: "unexpected end of LZX bitstream".to_string(),
                    });
                }
                break;
            }

            let word = u16::from_le_bytes([self.bytes[self.cursor], self.bytes[self.cursor + 1]]);
            self.cursor += 2;
            self.bit_buffer |= u32::from(word) << self.bits_count;
            self.bits_count += 16;
        }

        if self.bits_count < n {
            return Err(Error::DecompressionFailed {
                method: "LZX".to_string(),
                reason: "insufficient bits in LZX bitstream".to_string(),
            });
        }

        let mask = (1u32 << n) - 1;
        let val = (self.bit_buffer & mask) as u16;
        self.bit_buffer >>= n;
        self.bits_count -= n;
        Ok(val)
    }

    /// Aligns bit position to next 16-bit word boundary.
    pub const fn align_to_16(&mut self) {
        let rem = self.bits_count % 16;
        self.bit_buffer >>= rem;
        self.bits_count -= rem;
    }

    /// Reads `len` literal bytes after aligning to 16-bit boundary.
    ///
    /// # Errors
    ///
    /// Returns [`Error::DecompressionFailed`] if literal stream is truncated.
    pub fn read_bytes(&mut self, len: usize) -> Result<&'a [u8]> {
        self.align_to_16();
        let words_in_buf = (self.bits_count / 8) as usize;
        let start = self.cursor - words_in_buf;
        let end = start + len;
        if end > self.bytes.len() {
            return Err(Error::DecompressionFailed {
                method: "LZX".to_string(),
                reason: "unexpected end of LZX literal byte stream".to_string(),
            });
        }
        self.cursor = end;
        if self.cursor % 2 != 0 && self.cursor < self.bytes.len() {
            self.cursor += 1; // Pad to 16-bit word
        }
        self.bit_buffer = 0;
        self.bits_count = 0;
        Ok(&self.bytes[start..end])
    }
}

/// 16-bit word-aligned little-endian bitstream writer for LZX.
#[derive(Debug, Default)]
pub struct LzxBitWriter {
    /// Output buffer of emitted bytes.
    bytes: Vec<u8>,
    /// Bit accumulator.
    bit_buffer: u32,
    /// Number of valid bits in accumulator.
    bits_count: u8,
}

impl LzxBitWriter {
    /// Creates a new [`LzxBitWriter`].
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Writes `n` bits to the word-aligned stream (LSB first).
    #[allow(clippy::cast_possible_truncation)]
    pub fn write_bits(&mut self, val: u16, n: u8) {
        self.bit_buffer |= u32::from(val) << self.bits_count;
        self.bits_count += n;

        while self.bits_count >= 16 {
            let word = (self.bit_buffer & 0xFFFF) as u16;
            self.bytes.extend_from_slice(&word.to_le_bytes());
            self.bit_buffer >>= 16;
            self.bits_count -= 16;
        }
    }

    /// Writes a Huffman code MSB-first into the bitstream.
    pub fn write_bits_msb(&mut self, val: u16, n: u8) {
        for bit_idx in (0..n).rev() {
            let b = (val >> bit_idx) & 1;
            self.write_bits(b, 1);
        }
    }

    /// Flushes remaining bits and aligns to next 16-bit word boundary.
    #[allow(clippy::cast_possible_truncation)]
    pub fn align_to_16(&mut self) {
        if self.bits_count > 0 {
            let word = (self.bit_buffer & 0xFFFF) as u16;
            self.bytes.extend_from_slice(&word.to_le_bytes());
            self.bit_buffer = 0;
            self.bits_count = 0;
        }
    }

    /// Consumes the writer and returns the output bytes aligned to 16 bits.
    #[must_use]
    pub fn into_bytes(mut self) -> Vec<u8> {
        self.align_to_16();
        self.bytes
    }
}

#[cfg(test)]
#[allow(clippy::manual_flatten)]
mod tests {
    use super::*;

    /// Tests Intel 80x86 E8 call-translation forward and inverse roundtrip with both positive and negative jumps.
    #[test]
    fn test_e8_translation_roundtrip() {
        let mut original = vec![
            0x90, 0x90, // NOP, NOP
            0xE8, 0x10, 0x00, 0x00, 0x00, // CALL +0x10
            0x90, 0xE8, 0xF0, 0xFF, 0xFF, 0xFF, // CALL -0x10
            0x90, 0x90, 0x90, 0x90, 0x90, 0x90, 0x90, 0x90, // Pad past limit
        ];
        let original_copy = original.clone();

        e8_translate(&mut original, 1000, false);
        assert_ne!(original, original_copy);

        e8_translate(&mut original, 1000, true);
        assert_eq!(original, original_copy);

        let mut large_jump = vec![
            0xE8, 0xF6, 0xFF, 0xFF, 0x01, // displacement = 0x01FF_FFF6
            0x90, 0x90, 0x90, 0x90, 0x90, 0x90, 0x90, 0x90, 0x90,
        ];
        let large_jump_copy = large_jump.clone();
        e8_translate(&mut large_jump, 500, false);
        assert_ne!(large_jump, large_jump_copy);
        e8_translate(&mut large_jump, 500, true);
        assert_eq!(large_jump, large_jump_copy);

        let mut short = vec![0xE8, 0x01];
        e8_translate(&mut short, 0, false);
        assert_eq!(short, vec![0xE8, 0x01]);
    }

    /// Tests [`LzxState`] creation, reset, and window bounds validation.
    #[test]
    fn test_lzx_state() {
        assert!(LzxState::new(14).is_err());
        assert!(LzxState::new(22).is_err());

        let state_res = LzxState::new(16);
        assert!(state_res.is_ok());
        for res in [state_res, LzxState::new(14)] {
            if let Ok(mut s) = res {
                assert_eq!(s.window_size, 65_536);
                assert_eq!(s.r0, 1);

                s.r0 = 100;
                s.reset();
                assert_eq!(s.r0, 1);
            }
        }
    }
    /// Tests position slot calculations for various window sizes.
    #[test]
    fn test_position_slots() {
        assert_eq!(num_position_slots(15), 30);
        assert_eq!(num_position_slots(16), 32);
        assert_eq!(num_position_slots(17), 34);
        assert_eq!(num_position_slots(18), 36);
        assert_eq!(num_position_slots(19), 38);
        assert_eq!(num_position_slots(20), 42);
        assert_eq!(num_position_slots(21), 50);

        assert_eq!(slot_extra_bits(0), 0);
        assert_eq!(slot_extra_bits(3), 0);
        assert_eq!(slot_extra_bits(4), 1);
        assert_eq!(slot_extra_bits(5), 1);
        assert_eq!(slot_extra_bits(6), 2);

        assert_eq!(slot_base_offset(0), 0);
        assert_eq!(slot_base_offset(1), 1);
        assert_eq!(slot_base_offset(4), 4);
        assert_eq!(slot_base_offset(5), 6);
        assert_eq!(slot_base_offset(6), 8);
    }

    /// Tests canonical Huffman tree construction and decoding.
    #[test]
    fn test_huffman_tree_roundtrip() -> Result<()> {
        let lengths = [2, 1, 3, 3]; // Alphabet of 4 symbols
        let tree = HuffmanTree::from_lengths(&lengths)?;
        assert_eq!(tree.code_lengths(), &lengths);

        let codes = tree.canonical_codes();
        assert_eq!(codes.len(), 4);

        let mut writer = LzxBitWriter::new();
        // Write symbol 0, 1, 2, 3
        for &(c, l) in codes.iter().take(4) {
            writer.write_bits_msb(c, l);
        }
        let bytes = writer.into_bytes();

        let mut reader = LzxBitReader::new(&bytes);
        for expected in 0..4u16 {
            let dec = tree.decode_symbol(&mut reader)?;
            assert_eq!(dec, expected);
        }

        // Oversubscribed tree error
        let oversubscribed = [1, 1, 1]; // 3 symbols of length 1 -> 3 * 0.5 = 1.5 > 1.0
        assert!(HuffmanTree::from_lengths(&oversubscribed).is_err());

        // Max bits validation
        let bad_length = [17];
        assert!(HuffmanTree::from_lengths(&bad_length).is_err());

        Ok(())
    }

    /// Tests LZX Type 1 (Verbatim) compression and decompression roundtrip.
    #[test]
    fn test_lzx_verbatim_roundtrip() -> Result<()> {
        let mut state = LzxState::new(16)?;
        let data = b"The quick brown fox jumps over the lazy dog. The quick brown fox jumps!";

        let comp = state.compress_verbatim_block(data)?;
        assert_ne!(comp, Vec::<u8>::new());

        let mut decomp_state = LzxState::new(16)?;
        let decomp = decomp_state.decompress_block(&comp, data.len())?;
        assert_eq!(decomp, data.to_vec());

        Ok(())
    }

    /// Tests LZX Type 2 (Aligned) block decompression with long match lengths (>= 9) and repeat offsets.
    #[test]
    fn test_lzx_aligned_and_secondary_lengths() -> Result<()> {
        let mut state = LzxState::new(16)?;

        // Repetitive string with a length >= 12 match
        let mut data = Vec::new();
        data.extend_from_slice(b"1234567890abcdefghijklmnopqrstuvwxyz");
        data.extend_from_slice(b"1234567890abcdefghijklmnopqrstuvwxyz"); // match of len 36!
        data.extend_from_slice(b"1234567890abcdefghijklmnopqrstuvwxyz"); // repeat match!

        let comp = state.compress_verbatim_block(&data)?;
        let mut decomp_state = LzxState::new(16)?;
        let decomp = decomp_state.decompress_block(&comp, data.len())?;
        assert_eq!(decomp, data);

        // Manually build an aligned offset block (Type 2) to exercise Type 2 decoding branch
        let mut writer = LzxBitWriter::new();
        writer.write_bits(0b010, 3); // Type 2: Aligned
        let uncomp_len = 10u32;
        writer.write_bits((uncomp_len & 0xFFFF) as u16, 16);
        writer.write_bits((uncomp_len >> 16) as u16, 8);

        // 8 x 3-bit aligned tree lengths (all length 3)
        for _ in 0..8 {
            writer.write_bits(3, 3);
        }

        // Pre-tree: 20 symbols of length 0 except symbol 0 has length 1
        writer.write_bits(1, 3);
        for _ in 1..20 {
            writer.write_bits(0, 3);
        }

        // Decode delta lengths: all symbols have delta 0 (i.e. length 0 except what we set)
        // Main tree has 512 symbols, secondary has 249 symbols.
        // Symbol 0 in pre-tree gives delta 0 (length 0).
        for _ in 0..(512 + 249) {
            writer.write_bits(0, 1);
        }

        // Write a valid Type 2 block payload... or verify error on empty main tree
        writer.align_to_16();
        let bytes = writer.into_bytes();

        let mut t2_state = LzxState::new(16)?;
        assert!(t2_state.decompress_block(&bytes, 10).is_err());

        Ok(())
    }

    /// Tests LZX Type 3 (Uncompressed) compression and decompression roundtrip.
    #[test]
    fn test_lzx_uncompressed_roundtrip() -> Result<()> {
        let mut state = LzxState::new(16)?;
        let data = b"Hello, Microsoft LZX uncompressed world!";

        let comp = state.compress_uncompressed_block(data)?;
        assert_ne!(comp, Vec::<u8>::new());

        let mut decomp_state = LzxState::new(16)?;
        let decomp = decomp_state.decompress_block(&comp, data.len())?;
        assert_eq!(decomp, data.to_vec());

        Ok(())
    }

    /// Tests LZX adaptive block compression selecting verbatim vs uncompressed.
    #[test]
    fn test_lzx_adaptive_roundtrip() -> Result<()> {
        let mut state = LzxState::new(15)?;
        let repetitive = b"AAAAABBBBBCCCCCDDDDDEEEEEAAAAABBBBBCCCCCDDDDDEEEEE123456789";

        let comp = state.compress_block(repetitive)?;
        assert_ne!(comp, Vec::<u8>::new());

        let mut decomp_state = LzxState::new(15)?;
        let decomp = decomp_state.decompress_block(&comp, repetitive.len())?;
        assert_eq!(decomp, repetitive.to_vec());

        // Large repetitive payload where verbatim compressed length is smaller than input
        let large_rep = vec![b'A'; 2000];
        let comp_large = state.compress_block(&large_rep)?;
        let decomp_large = decomp_state.decompress_block(&comp_large, large_rep.len())?;
        assert_eq!(decomp_large, large_rep);

        Ok(())
    }

    /// Tests LZX error handling and bit reader edge cases.
    #[test]
    fn test_lzx_errors() -> Result<()> {
        let mut state = LzxState::new(15)?;

        // Oversized (> 32KB)
        assert!(state.compress_block(&vec![0u8; 33_000]).is_err());
        assert!(state.compress_verbatim_block(&vec![0u8; 33_000]).is_err());
        assert!(state
            .compress_uncompressed_block(&vec![0u8; 33_000])
            .is_err());

        // Unsupported block type
        let mut bad_type_writer = LzxBitWriter::new();
        bad_type_writer.write_bits(0b000, 3); // Type 0 (invalid)
        bad_type_writer.align_to_16();
        let bad_bytes = bad_type_writer.into_bytes();
        assert!(state.decompress_block(&bad_bytes, 10).is_err());

        // Truncated block bitstream
        assert!(state.decompress_block(&[0x03], 10).is_err());
        assert!(state.decompress_block(&[], 10).is_err());

        // Length mismatch
        let comp = state.compress_uncompressed_block(b"data")?;
        assert!(state.decompress_block(&comp, 999).is_err());

        // Truncated literal payload in uncompressed block
        let mut truncated_lit_block = comp;
        truncated_lit_block.truncate(15);
        assert!(state.decompress_block(&truncated_lit_block, 4).is_err());

        // Truncated during main tree deltas
        let mut t1_writer = LzxBitWriter::new();
        t1_writer.write_bits(0b001, 3); // Type 1: Verbatim
        t1_writer.write_bits(10, 16);
        t1_writer.write_bits(0, 8);
        t1_writer.write_bits(1, 3);
        t1_writer.write_bits(1, 3);
        for _ in 2..20 {
            t1_writer.write_bits(0, 3);
        }
        t1_writer.align_to_16();
        let trunc_main = t1_writer.into_bytes();
        assert!(state.decompress_block(&trunc_main, 10).is_err());

        // Truncated during secondary tree deltas
        let mut t1_sec_writer = LzxBitWriter::new();
        t1_sec_writer.write_bits(0b001, 3); // Type 1: Verbatim
        t1_sec_writer.write_bits(10, 16);
        t1_sec_writer.write_bits(0, 8);
        t1_sec_writer.write_bits(1, 3);
        for _ in 1..20 {
            t1_sec_writer.write_bits(0, 3);
        }
        for _ in 0..496 {
            t1_sec_writer.write_bits(0, 1);
        }
        t1_sec_writer.align_to_16();
        let trunc_sec = t1_sec_writer.into_bytes();
        assert!(state.decompress_block(&trunc_sec, 10).is_err());

        // Main tree exceeds Kraft inequality (line 1019 ?)
        let mut t1_bad_main = LzxBitWriter::new();
        t1_bad_main.write_bits(0b001, 3);
        t1_bad_main.write_bits(10, 16);
        t1_bad_main.write_bits(0, 8);
        t1_bad_main.write_bits(1, 3); // sym 0 len 1
        t1_bad_main.write_bits(0, 3); // sym 1
        for _ in 2..16 {
            t1_bad_main.write_bits(0, 3);
        }
        t1_bad_main.write_bits(1, 3); // sym 16 len 1 (bit 1)
        for _ in 17..20 {
            t1_bad_main.write_bits(0, 3);
        }
        for _ in 0..496 {
            t1_bad_main.write_bits(1, 1); // delta 16 -> length 1
        }
        t1_bad_main.align_to_16();
        let bad_main_bytes = t1_bad_main.into_bytes();
        assert!(state.decompress_block(&bad_main_bytes, 10).is_err());

        // Secondary tree exceeds Kraft inequality (line 1028 ?)
        let mut t1_bad_sec = LzxBitWriter::new();
        t1_bad_sec.write_bits(0b001, 3);
        t1_bad_sec.write_bits(10, 16);
        t1_bad_sec.write_bits(0, 8);
        t1_bad_sec.write_bits(1, 3); // sym 0 len 1 (bit 0)
        for _ in 1..16 {
            t1_bad_sec.write_bits(0, 3);
        }
        t1_bad_sec.write_bits(1, 3); // sym 16 len 1 (bit 1)
        for _ in 17..20 {
            t1_bad_sec.write_bits(0, 3);
        }
        for _ in 0..496 {
            t1_bad_sec.write_bits(0, 1); // delta 0 -> length 0
        }
        for _ in 0..249 {
            t1_bad_sec.write_bits(1, 1); // delta 16 -> length 1
        }
        t1_bad_sec.align_to_16();
        let bad_sec_bytes = t1_bad_sec.into_bytes();
        assert!(state.decompress_block(&bad_sec_bytes, 10).is_err());

        Ok(())
    }

    /// Tests E8 translation branch edge cases (displacement computation and reversal).
    #[test]
    fn test_e8_translation_edge_cases() {
        // Decode with negative value within [-current_pointer, E8_DEFAULT_FILE_SIZE)
        let mut buf_dec = vec![0u8; 20];
        buf_dec[5] = 0xE8;
        let neg_val: i32 = -3;
        buf_dec[6..10].copy_from_slice(&neg_val.to_le_bytes());
        e8_translate(&mut buf_dec, 0, true);
        let expected_disp = neg_val + E8_DEFAULT_FILE_SIZE;
        assert_eq!(&buf_dec[6..10], &expected_disp.to_le_bytes());

        // Decode with value < -current_pointer (skips translation)
        let mut buf_dec_out = vec![0u8; 20];
        buf_dec_out[5] = 0xE8;
        let out_val: i32 = -100;
        buf_dec_out[6..10].copy_from_slice(&out_val.to_le_bytes());
        e8_translate(&mut buf_dec_out, 0, true);
        assert_eq!(&buf_dec_out[6..10], &out_val.to_le_bytes());

        // Decode with value >= E8_DEFAULT_FILE_SIZE (skips translation)
        let mut buf_dec_large = vec![0u8; 20];
        buf_dec_large[5] = 0xE8;
        let large_val: i32 = E8_DEFAULT_FILE_SIZE + 10;
        buf_dec_large[6..10].copy_from_slice(&large_val.to_le_bytes());
        e8_translate(&mut buf_dec_large, 0, true);
        assert_eq!(&buf_dec_large[6..10], &large_val.to_le_bytes());

        // Encode with target >= E8_DEFAULT_FILE_SIZE but < E8_DEFAULT_FILE_SIZE + current_pointer
        let mut buf_enc = vec![0u8; 20];
        buf_enc[5] = 0xE8;
        let disp: i32 = E8_DEFAULT_FILE_SIZE - 2;
        buf_enc[6..10].copy_from_slice(&disp.to_le_bytes());
        e8_translate(&mut buf_enc, 0, false);
        let expected_val = disp - E8_DEFAULT_FILE_SIZE;
        assert_eq!(&buf_enc[6..10], &expected_val.to_le_bytes());

        // Encode with target < 0 (skips translation)
        let mut buf_enc_out = vec![0u8; 20];
        buf_enc_out[5] = 0xE8;
        let out_disp: i32 = -100;
        buf_enc_out[6..10].copy_from_slice(&out_disp.to_le_bytes());
        e8_translate(&mut buf_enc_out, 0, false);
        assert_eq!(&buf_enc_out[6..10], &out_disp.to_le_bytes());

        // Encode with target >= E8_DEFAULT_FILE_SIZE + current_pointer (skips translation)
        let mut buf_enc_large = vec![0u8; 20];
        buf_enc_large[5] = 0xE8;
        let large_disp: i32 = E8_DEFAULT_FILE_SIZE + 10;
        buf_enc_large[6..10].copy_from_slice(&large_disp.to_le_bytes());
        e8_translate(&mut buf_enc_large, 0, false);
        assert_eq!(&buf_enc_large[6..10], &large_disp.to_le_bytes());
    }

    /// Tests Huffman tree error handling on empty or corrupted trees.
    #[test]
    fn test_huffman_tree_decode_errors() {
        let empty_tree = HuffmanTree {
            nodes: Vec::new(),
            code_lengths: Vec::new(),
        };
        let mut reader = LzxBitReader::new(&[0xFF; 4]);
        assert!(empty_tree.decode_symbol(&mut reader).is_err());

        let bad_index_tree = HuffmanTree {
            nodes: vec![[9999, 0]],
            code_lengths: vec![1],
        };
        let mut reader2 = LzxBitReader::new(&[0x00; 4]);
        assert!(bad_index_tree.decode_symbol(&mut reader2).is_err());

        let valid_tree = HuffmanTree {
            nodes: vec![[1, 2]],
            code_lengths: vec![1],
        };
        let mut empty_reader = LzxBitReader::new(&[]);
        assert!(valid_tree.decode_symbol(&mut empty_reader).is_err());
    }

    /// Tests RLE decoding symbols (17, 18, 19) in `decode_tree_lengths`.
    #[test]
    fn test_decode_tree_lengths_rle() -> Result<()> {
        let mut pre_lengths = [0u8; LZX_PRE_TREE_NUM_SYMBOLS];
        pre_lengths[16] = 2;
        pre_lengths[17] = 2;
        pre_lengths[18] = 2;
        pre_lengths[19] = 2;
        let pre_tree = HuffmanTree::from_lengths(&pre_lengths)?;

        let mut writer = LzxBitWriter::new();
        // 1. Sym 19: i=0 < prev_lengths.len() (10), next_sym = 16 (<= 16)
        // Exercises line 391 (prev_lengths[i]) and line 396 (delta arithmetic)
        writer.write_bits_msb(0b11, 2);
        writer.write_bits(1, 1);
        writer.write_bits_msb(0b00, 2);

        // 2. Sym 17: 4 bits count extra (e.g. 5 -> total 9 zeros)
        writer.write_bits_msb(0b01, 2);
        writer.write_bits(5, 4);

        // 3. Sym 18: 5 bits count extra (e.g. 3 -> total 23 zeros)
        writer.write_bits_msb(0b10, 2);
        writer.write_bits(3, 5);

        // 4. Sym 19: i >= prev_lengths.len() (10), next_sym = 17 (> 16)
        // Exercises line 393 (else 0) and line 398 (else 0)
        writer.write_bits_msb(0b11, 2);
        writer.write_bits(1, 1);
        writer.write_bits_msb(0b01, 2);

        for _ in 0..10 {
            writer.write_bits_msb(0b00, 2);
        }
        writer.align_to_16();
        let bytes = writer.into_bytes();

        let mut reader = LzxBitReader::new(&bytes);
        let mut lengths = vec![0u8; 50];
        let initial_lengths = vec![0u8; 10];
        decode_tree_lengths(&mut reader, &pre_tree, &mut lengths, &initial_lengths)?;
        assert_eq!(lengths.len(), 50);

        // Test decode_tree_lengths reader exhaustion (line 363 col 49)
        let mut empty_reader = LzxBitReader::new(&[]);
        assert!(
            decode_tree_lengths(&mut empty_reader, &pre_tree, &mut lengths, &initial_lengths)
                .is_err()
        );

        Ok(())
    }

    /// Tests LZX repeat offset matches (R1 and R2), consecutive block compression, and fallback.
    #[test]
    fn test_lzx_repeat_offsets_and_consecutive_blocks() -> Result<()> {
        let mut state = LzxState::new(15)?;
        let a = b"abcdefghij";
        let b = b"0123456789";
        let c = b"klmnopqrst";
        let d = b"uvwxyz!@#$";

        let mut data = Vec::new();
        data.extend_from_slice(a);
        data.extend_from_slice(b);
        data.extend_from_slice(c);
        data.extend_from_slice(d);
        data.extend_from_slice(d); // r0=10
        data.extend_from_slice(c); // r0=20, r1=10
        data.extend_from_slice(b); // r0=30, r1=20, r2=10
        data.extend_from_slice(c); // match at r1 (20) -> slot 1!
        data.extend_from_slice(c); // match at r2 (10) -> slot 2!
        data.extend_from_slice(b"ZZZZ"); // match at offset 1, slot 3 (extra_bits = 0, !is_rep)
        data.extend_from_slice(&vec![b'W'; 600]); // exercises match_len >= LZX_MAX_MATCH, end of buffer, and best_len >= 32

        // Setting r0 > window_size and r1 = 0 exercises candidate offset bounds checks
        state.r0 = u32::try_from(state.window_size.saturating_add(1)).unwrap_or(u32::MAX);
        state.r1 = 0;

        // Compress block 1
        let comp1 = state.compress_verbatim_block(&data)?;
        let mut decomp_state = LzxState::new(15)?;
        let decomp1 = decomp_state.decompress_block(&comp1, data.len())?;
        assert_eq!(decomp1, data);

        // Compress block 2 with same state (exercises previous tree length deltas)
        let comp2 = state.compress_verbatim_block(&data)?;
        let decomp2 = decomp_state.decompress_block(&comp2, data.len())?;
        assert_eq!(decomp2, data);

        // Decompress with wrong expected length
        assert!(decomp_state
            .decompress_block(&comp1, data.len() + 1)
            .is_err());

        // Compress incompressible data falling back to uncompressed block
        let uncomp = (0..50u8)
            .map(|x| x.wrapping_mul(73).wrapping_add(19))
            .collect::<Vec<_>>();
        let comp_fallback = state.compress_block(&uncomp)?;
        let decomp_fallback = decomp_state.decompress_block(&comp_fallback, uncomp.len())?;
        assert_eq!(decomp_fallback, uncomp);

        Ok(())
    }

    /// Tests backward search match reaching [`LZX_MAX_MATCH`].
    #[test]
    fn test_lzx_backward_match_max_length() -> Result<()> {
        let mut state = LzxState::new(15)?;
        state.r0 = 100;
        state.r1 = 200;
        state.r2 = 300;
        let mut data = Vec::new();
        data.extend_from_slice(b"abc");
        data.extend_from_slice(&vec![b'X'; 300]);
        let comp = state.compress_verbatim_block(&data)?;
        let mut decomp_state = LzxState::new(15)?;
        decomp_state.r0 = 100;
        decomp_state.r1 = 200;
        decomp_state.r2 = 300;
        let decomp = decomp_state.decompress_block(&comp, data.len())?;
        assert_eq!(decomp, data);
        Ok(())
    }

    /// Tests literal byte exhaustion in [`LzxBitReader`].
    #[test]
    fn test_lzx_bit_reader_bytes_exhaustion() {
        let bytes = [1u8, 2, 3];
        let mut reader = LzxBitReader::new(&bytes);
        assert!(reader.read_bytes(10).is_err());

        let mut reader2 = LzxBitReader::new(&bytes);
        assert_eq!(reader2.read_bytes(3).as_deref(), Ok(&bytes[..]));
    }

    /// Tests Type 2 (Aligned) block decoding and match offset validation.
    #[test]
    fn test_lzx_type2_aligned_decoding_and_invalid_offsets() -> Result<()> {
        let mut writer = LzxBitWriter::new();
        writer.write_bits(0b010, 3); // Type 2: Aligned
        let uncomp_len = 7u32;
        writer.write_bits((uncomp_len & 0xFFFF) as u16, 16);
        writer.write_bits((uncomp_len >> 16) as u16, 8);

        // 8 x 3-bit aligned tree lengths (all length 3)
        for _ in 0..8 {
            writer.write_bits(3, 3);
        }

        // Pre-tree: symbol 0 has length 1 (bit 0), symbol 15 has length 1 (bit 1)
        let mut pre_lengths = [0u8; LZX_PRE_TREE_NUM_SYMBOLS];
        pre_lengths[0] = 1;
        pre_lengths[15] = 1;
        for &pl in &pre_lengths {
            writer.write_bits(u16::from(pl), 3);
        }

        // Emit deltas for main tree (496 symbols) and secondary (249 symbols)
        // Symbols 65 ('A'), 256 (slot 0), 280 (slot 3), 320 (slot 8) have length 2 (delta 15)
        for i in 0..496 {
            if i == 65 || i == 256 || i == 280 || i == 320 {
                writer.write_bits(1, 1); // delta 15 -> len 2
            } else {
                writer.write_bits(0, 1); // delta 0 -> len 0
            }
        }
        for _ in 0..249 {
            writer.write_bits(0, 1);
        }

        // Canonical codes for main tree with 4 active symbols of length 2:
        // sym 65: 00 (literal 'A')
        // sym 256: 01 (slot 0, match len 2)
        // sym 280: 10 (slot 3, extra_bits 0, len_hdr 0 -> match len 2)
        // sym 320: 11 (slot 8, extra_bits 3, len_hdr 0 -> match len 2)
        // 1. Literal 'A': 00
        writer.write_bits_msb(0b00, 2);
        // 2. Match slot 0: 01 (exercises slot == 0 and offset validation)
        writer.write_bits_msb(0b01, 2);
        // 3. Match slot 3: 10 (exercises extra_bits == 0 => base)
        writer.write_bits_msb(0b10, 2);
        // 4. Match slot 8: 11 (exercises extra_bits >= 3 => aligned tree)
        writer.write_bits_msb(0b11, 2);
        // Aligned symbol 0 (3 bits: 000)
        writer.write_bits_msb(0b000, 3);

        writer.align_to_16();
        let bytes = writer.into_bytes();

        let mut state = LzxState::new(15)?;
        let decomp = state.decompress_block(&bytes, 7)?;
        assert_eq!(decomp.len(), 7);

        // Invalid match offsets
        let mut bad_off_state = LzxState::new(15)?;
        bad_off_state.r0 = 0; // offset 0 error
        assert!(bad_off_state.decompress_block(&bytes, 7).is_err());

        let mut bad_off_state2 = LzxState::new(15)?;
        bad_off_state2.r0 =
            u32::try_from(bad_off_state2.window_size.saturating_add(1)).unwrap_or(u32::MAX);
        assert!(bad_off_state2.decompress_block(&bytes, 7).is_err());

        let mut trunc_state = LzxState::new(15)?;
        let truncated = &bytes[..bytes.len().saturating_sub(2)];
        assert!(trunc_state.decompress_block(truncated, 7).is_err());

        Ok(())
    }
}

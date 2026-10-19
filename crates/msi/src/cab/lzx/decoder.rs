//! Microsoft LZX block decompression and 16-bit word-aligned bitstream reader.

use super::huffman::{decode_tree_lengths, HuffmanTree};
use super::{
    e8_translate, num_position_slots, slot_base_offset, slot_extra_bits, LzxState,
    LZX_ALIGNED_TREE_NUM_SYMBOLS, LZX_NUM_SECONDARY_LENGTHS, LZX_PRE_TREE_NUM_SYMBOLS,
};
use crate::error::{MsiError, Result};

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
    ///
    /// # Arguments
    ///
    /// * `bytes` - Compressed LZX bitstream slice.
    ///
    /// # Returns
    ///
    /// Initialized [`LzxBitReader`].
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
    /// Returns [`MsiError::LzxDecompressionFailed`] if bitstream ends prematurely.
    ///
    /// # Returns
    ///
    /// `0` or `1` bit value.
    pub fn read_bit(&mut self) -> Result<u8> {
        self.read_bits(1).map(|v| u8::try_from(v).unwrap_or(0))
    }

    /// Reads `n` bits from the 16-bit word-aligned bitstream.
    ///
    /// # Errors
    ///
    /// Returns [`MsiError::LzxDecompressionFailed`] if bitstream ends prematurely.
    ///
    /// # Arguments
    ///
    /// * `n` - Number of bits to read (`1..=16`).
    ///
    /// # Returns
    ///
    /// Extracted 16-bit unsigned value.
    pub fn read_bits(&mut self, n: u8) -> Result<u16> {
        while self.bits_count < n {
            if self.cursor + 1 >= self.bytes.len() {
                if self.cursor < self.bytes.len() {
                    let byte = self.bytes[self.cursor];
                    self.cursor += 1;
                    self.bit_buffer |= u32::from(byte) << self.bits_count;
                    self.bits_count += 8;
                } else {
                    return Err(MsiError::LzxDecompressionFailed {
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
            return Err(MsiError::LzxDecompressionFailed {
                reason: "insufficient bits in LZX bitstream".to_string(),
            });
        }

        let mask = (1u32 << n) - 1;
        let val = u16::try_from(self.bit_buffer & mask).unwrap_or(0);
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
    /// Returns [`MsiError::LzxDecompressionFailed`] if literal stream is truncated.
    ///
    /// # Arguments
    ///
    /// * `len` - Number of uncompressed literal bytes to read.
    ///
    /// # Returns
    ///
    /// Borrowed slice of `len` literal bytes.
    pub fn read_bytes(&mut self, len: usize) -> Result<&'a [u8]> {
        self.align_to_16();
        let words_in_buf = usize::from(self.bits_count / 8);
        let start = self.cursor - words_in_buf;
        let end = start + len;
        if end > self.bytes.len() {
            return Err(MsiError::LzxDecompressionFailed {
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

/// Decompresses an LZX block into uncompressed bytes.
pub(super) fn decompress_block_inner(
    state: &mut LzxState,
    input: &[u8],
    expected_uncomp_len: usize,
) -> Result<Vec<u8>> {
    let mut reader = LzxBitReader::new(input);
    let block_type = reader.read_bits(3)?;

    let mut output = match block_type {
        0b011 => decompress_uncompressed_block(state, &mut reader, expected_uncomp_len)?,
        0b001 | 0b010 => {
            let is_aligned = block_type == 0b010;
            decompress_lz_block(state, &mut reader, expected_uncomp_len, is_aligned)?
        }
        _ => {
            return Err(MsiError::LzxDecompressionFailed {
                reason: format!("unsupported LZX block type {block_type}"),
            });
        }
    };

    e8_translate(&mut output, state.total_uncompressed_bytes, true);
    state.total_uncompressed_bytes += output.len();

    Ok(output)
}

/// Decompresses a Type 3 (Uncompressed) LZX block.
fn decompress_uncompressed_block(
    state: &mut LzxState,
    reader: &mut LzxBitReader<'_>,
    expected_uncomp_len: usize,
) -> Result<Vec<u8>> {
    let len_low = reader.read_bits(16)?;
    let len_high = reader.read_bits(8)?;
    let len = (usize::from(len_high) << 16) | usize::from(len_low);

    if len != expected_uncomp_len {
        return Err(MsiError::LzxDecompressionFailed {
            reason: format!("uncompressed block length {len} != expected {expected_uncomp_len}"),
        });
    }

    let r0_low = reader.read_bits(16)?;
    let r0_high = reader.read_bits(16)?;
    state.r0 = (u32::from(r0_high) << 16) | u32::from(r0_low);

    let r1_low = reader.read_bits(16)?;
    let r1_high = reader.read_bits(16)?;
    state.r1 = (u32::from(r1_high) << 16) | u32::from(r1_low);

    let r2_low = reader.read_bits(16)?;
    let r2_high = reader.read_bits(16)?;
    state.r2 = (u32::from(r2_high) << 16) | u32::from(r2_low);

    reader.align_to_16();
    let bytes = reader.read_bytes(len)?;
    Ok(bytes.to_vec())
}

/// Decompresses a Type 1 (Verbatim) or Type 2 (Aligned Offset) LZX block.
fn decompress_lz_block(
    state: &mut LzxState,
    reader: &mut LzxBitReader<'_>,
    expected_uncomp_len: usize,
    is_aligned: bool,
) -> Result<Vec<u8>> {
    let len_low = reader.read_bits(16)?;
    let len_high = reader.read_bits(8)?;
    let uncomp_len = (usize::from(len_high) << 16) | usize::from(len_low);

    if uncomp_len != expected_uncomp_len {
        return Err(MsiError::LzxDecompressionFailed {
            reason: format!("block length {uncomp_len} != expected {expected_uncomp_len}"),
        });
    }

    let aligned_tree = if is_aligned {
        let mut al_lengths = [0u8; LZX_ALIGNED_TREE_NUM_SYMBOLS];
        for l in &mut al_lengths {
            *l = u8::try_from(reader.read_bits(3)?).unwrap_or(0);
        }
        Some(HuffmanTree::from_lengths(&al_lengths)?)
    } else {
        None
    };

    let mut pre_lengths = [0u8; LZX_PRE_TREE_NUM_SYMBOLS];
    for l in &mut pre_lengths {
        *l = u8::try_from(reader.read_bits(3)?).unwrap_or(0);
    }
    let pre_tree = HuffmanTree::from_lengths(&pre_lengths)?;

    let slots = num_position_slots(state.window_bits);
    let main_alphabet_size = 256 + 8 * slots;
    let mut new_main_lengths = vec![0u8; main_alphabet_size];
    decode_tree_lengths(
        reader,
        &pre_tree,
        &mut new_main_lengths,
        &state.main_tree_lengths,
    )?;
    let main_tree = HuffmanTree::from_lengths(&new_main_lengths)?;

    let mut new_sec_lengths = vec![0u8; LZX_NUM_SECONDARY_LENGTHS];
    decode_tree_lengths(
        reader,
        &pre_tree,
        &mut new_sec_lengths,
        &state.secondary_tree_lengths,
    )?;
    let sec_tree = HuffmanTree::from_lengths(&new_sec_lengths)?;

    let mut decomp_bytes = Vec::with_capacity(uncomp_len);

    while decomp_bytes.len() < uncomp_len {
        let sym = usize::from(main_tree.decode_symbol(reader)?);
        if sym < 256 {
            let b = u8::try_from(sym).unwrap_or(0);
            decomp_bytes.push(b);
            state.window[state.window_pos] = b;
            state.window_pos = (state.window_pos + 1) % state.window_size;
        } else {
            let match_hdr = sym - 256;
            let len_hdr = match_hdr & 7;
            let slot = match_hdr >> 3;

            let match_len = if len_hdr < 7 {
                len_hdr + 2
            } else {
                usize::from(sec_tree.decode_symbol(reader)?) + 9
            };

            let offset = decode_match_offset(state, reader, slot, aligned_tree.as_ref())?;

            if offset == 0 || offset > state.window_size {
                return Err(MsiError::LzxDecompressionFailed {
                    reason: format!("invalid match offset {offset}"),
                });
            }

            for _ in 0..match_len {
                let src_idx = (state.window_pos + state.window_size - offset) % state.window_size;
                let b = state.window[src_idx];
                decomp_bytes.push(b);
                state.window[state.window_pos] = b;
                state.window_pos = (state.window_pos + 1) % state.window_size;
            }
        }
    }

    state.main_tree_lengths = new_main_lengths;
    state.secondary_tree_lengths = new_sec_lengths;
    Ok(decomp_bytes)
}

/// Decodes a match lookback offset and updates repeated offsets `R0`, `R1`, `R2`.
fn decode_match_offset(
    state: &mut LzxState,
    reader: &mut LzxBitReader<'_>,
    slot: usize,
    aligned_tree: Option<&HuffmanTree>,
) -> Result<usize> {
    let offset = if slot == 0 {
        usize::try_from(state.r0).unwrap_or(0)
    } else if slot == 1 {
        let off = usize::try_from(state.r1).unwrap_or(0);
        state.r1 = state.r0;
        state.r0 = u32::try_from(off).unwrap_or(0);
        off
    } else if slot == 2 {
        let off = usize::try_from(state.r2).unwrap_or(0);
        state.r2 = state.r0;
        state.r0 = u32::try_from(off).unwrap_or(0);
        off
    } else {
        let extra_bits = slot_extra_bits(slot);
        let base = slot_base_offset(slot);
        let raw_off = if let (Some(t), true) = (aligned_tree, extra_bits >= 3) {
            let high = u32::from(reader.read_bits(extra_bits - 3)?) << 3;
            let low = u32::from(t.decode_symbol(reader)?);
            base + high + low
        } else if extra_bits > 0 {
            let extra = u32::from(reader.read_bits(extra_bits)?);
            base + extra
        } else {
            base
        };

        let off = usize::try_from(raw_off - 2).unwrap_or(0);
        state.r2 = state.r1;
        state.r1 = state.r0;
        state.r0 = u32::try_from(off).unwrap_or(0);
        off
    };
    Ok(offset)
}

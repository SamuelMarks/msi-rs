//! Microsoft LZX block compression and 16-bit word-aligned bitstream writer.

use super::huffman::{compute_huffman_lengths, HuffmanTree};
use super::{
    e8_translate, num_position_slots, slot_base_offset, slot_extra_bits, LzxState, LZX_HASH_SIZE,
    LZX_MAX_MATCH, LZX_MIN_MATCH, LZX_NUM_SECONDARY_LENGTHS, LZX_PRE_TREE_NUM_SYMBOLS,
};
use crate::error::{MsiError, Result};

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
    ///
    /// # Returns
    ///
    /// Empty [`LzxBitWriter`].
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Writes `n` bits to the word-aligned stream (LSB first).
    ///
    /// # Arguments
    ///
    /// * `val` - 16-bit value containing the bits to write.
    /// * `n` - Number of low-order bits from `val` to emit.
    pub fn write_bits(&mut self, val: u16, n: u8) {
        self.bit_buffer |= u32::from(val) << self.bits_count;
        self.bits_count += n;

        while self.bits_count >= 16 {
            let word = u16::try_from(self.bit_buffer & 0xFFFF).unwrap_or(0);
            self.bytes.extend_from_slice(&word.to_le_bytes());
            self.bit_buffer >>= 16;
            self.bits_count -= 16;
        }
    }

    /// Writes a Huffman code MSB-first into the bitstream.
    ///
    /// # Arguments
    ///
    /// * `val` - Canonical Huffman code word.
    /// * `n` - Bit length of the Huffman code word.
    pub fn write_bits_msb(&mut self, val: u16, n: u8) {
        for bit_idx in (0..n).rev() {
            let b = (val >> bit_idx) & 1;
            self.write_bits(b, 1);
        }
    }

    /// Flushes remaining bits and aligns to next 16-bit word boundary.
    pub fn align_to_16(&mut self) {
        if self.bits_count > 0 {
            let word = u16::try_from(self.bit_buffer & 0xFFFF).unwrap_or(0);
            self.bytes.extend_from_slice(&word.to_le_bytes());
            self.bit_buffer = 0;
            self.bits_count = 0;
        }
    }

    /// Consumes the writer and returns the output bytes aligned to 16 bits.
    ///
    /// # Returns
    ///
    /// Byte vector containing the 16-bit aligned bitstream output.
    #[must_use]
    pub fn into_bytes(mut self) -> Vec<u8> {
        self.align_to_16();
        self.bytes
    }
}

/// Compresses a block of data into an LZX uncompressed block (Block Type 3).
pub(super) fn compress_uncompressed_block_inner(
    state: &mut LzxState,
    input: &[u8],
) -> Result<Vec<u8>> {
    if input.len() > 32_768 {
        return Err(MsiError::LzxCompressionFailed {
            reason: "block length exceeds 32KB".to_string(),
        });
    }

    let mut transformed = input.to_vec();
    e8_translate(&mut transformed, state.total_uncompressed_bytes, false);

    let mut bit_writer = LzxBitWriter::new();
    bit_writer.write_bits(0b011, 3); // Type 3

    let len = u32::try_from(transformed.len()).unwrap_or(0);
    bit_writer.write_bits(u16::try_from(len & 0xFFFF).unwrap_or(0), 16);
    bit_writer.write_bits(u16::try_from(len >> 16).unwrap_or(0), 8);

    bit_writer.write_bits(u16::try_from(state.r0 & 0xFFFF).unwrap_or(0), 16);
    bit_writer.write_bits(u16::try_from(state.r0 >> 16).unwrap_or(0), 16);
    bit_writer.write_bits(u16::try_from(state.r1 & 0xFFFF).unwrap_or(0), 16);
    bit_writer.write_bits(u16::try_from(state.r1 >> 16).unwrap_or(0), 16);
    bit_writer.write_bits(u16::try_from(state.r2 & 0xFFFF).unwrap_or(0), 16);
    bit_writer.write_bits(u16::try_from(state.r2 >> 16).unwrap_or(0), 16);

    bit_writer.align_to_16();
    let mut output = bit_writer.into_bytes();

    output.extend_from_slice(&transformed);
    if output.len() % 2 != 0 {
        output.push(0);
    }

    for &b in &transformed {
        state.window[state.window_pos] = b;
        state.window_pos = (state.window_pos + 1) % state.window_size;
    }
    state.total_uncompressed_bytes += transformed.len();

    Ok(output)
}

/// Compresses a block of data into an LZX Block Type 1 (Verbatim) or Type 2 (Aligned) block.
pub(super) fn compress_verbatim_block_inner(state: &mut LzxState, input: &[u8]) -> Result<Vec<u8>> {
    if input.len() > 32_768 {
        return Err(MsiError::LzxCompressionFailed {
            reason: "block length exceeds 32KB".to_string(),
        });
    }

    let mut transformed = input.to_vec();
    e8_translate(&mut transformed, state.total_uncompressed_bytes, false);

    let tokens = parse_lz_tokens(state, &transformed);
    let slots = num_position_slots(state.window_bits);
    let (main_lengths, sec_lengths) = compute_token_tree_lengths(state, &tokens, slots);
    let main_tree = HuffmanTree::from_lengths(&main_lengths)?;
    let sec_tree = HuffmanTree::from_lengths(&sec_lengths)?;
    let main_codes = main_tree.canonical_codes();
    let sec_codes = sec_tree.canonical_codes();

    let mut bit_writer = LzxBitWriter::new();
    bit_writer.write_bits(0b001, 3); // Type 1: Verbatim

    let len = u32::try_from(transformed.len()).unwrap_or(0);
    bit_writer.write_bits(u16::try_from(len & 0xFFFF).unwrap_or(0), 16);
    bit_writer.write_bits(u16::try_from(len >> 16).unwrap_or(0), 8);

    write_pre_tree_and_deltas(state, &mut bit_writer, &main_lengths, &sec_lengths)?;
    write_lz_tokens(
        state,
        &mut bit_writer,
        &tokens,
        slots,
        &main_codes,
        &sec_codes,
    );

    bit_writer.align_to_16();
    let output = bit_writer.into_bytes();

    state.main_tree_lengths = main_lengths;
    state.secondary_tree_lengths = sec_lengths;

    for &b in &transformed {
        state.window[state.window_pos] = b;
        state.window_pos = (state.window_pos + 1) % state.window_size;
    }
    state.total_uncompressed_bytes += transformed.len();

    Ok(output)
}

/// Parses input bytes into LZ literal and match tokens using repeat offsets and hash chains.
fn parse_lz_tokens(state: &LzxState, transformed: &[u8]) -> Vec<LzToken> {
    let mut tokens: Vec<LzToken> = Vec::new();
    let mut pos = 0;
    let mut tok_r0 = state.r0;
    let mut tok_r1 = state.r1;
    let mut tok_r2 = state.r2;

    let mut head = vec![usize::MAX; LZX_HASH_SIZE];
    let mut chain_prev = vec![usize::MAX; transformed.len()];

    while pos < transformed.len() {
        let (mut best_len, mut best_offset) =
            find_repeat_match(state, transformed, pos, [tok_r0, tok_r1, tok_r2]);

        if pos + 2 < transformed.len() {
            let h = (((usize::from(transformed[pos])) << 5)
                ^ ((usize::from(transformed[pos + 1])) << 2)
                ^ usize::from(transformed[pos + 2]))
                & (LZX_HASH_SIZE - 1);
            let mut cand = head[h];
            head[h] = pos;
            chain_prev[pos] = cand;

            let mut steps = 0;
            while cand != usize::MAX && steps < 64 {
                let off = pos - cand;
                let mut match_len = 0;
                while pos + match_len < transformed.len()
                    && match_len < LZX_MAX_MATCH
                    && transformed[pos + match_len] == transformed[cand + match_len]
                {
                    match_len += 1;
                }
                if match_len >= LZX_MIN_MATCH && match_len > best_len {
                    best_len = match_len;
                    best_offset = off;
                    if match_len >= 128 {
                        break;
                    }
                }
                cand = chain_prev[cand];
                steps += 1;
            }
        }

        if best_len >= LZX_MIN_MATCH {
            tokens.push(LzToken::Match {
                offset: best_offset,
                length: best_len,
            });
            let old_pos = pos;
            pos += best_len;

            for i in (old_pos + 1)..pos {
                if i + 2 < transformed.len() {
                    let h = (((usize::from(transformed[i])) << 5)
                        ^ ((usize::from(transformed[i + 1])) << 2)
                        ^ usize::from(transformed[i + 2]))
                        & (LZX_HASH_SIZE - 1);
                    chain_prev[i] = head[h];
                    head[h] = i;
                }
            }

            let best_off_u32 = u32::try_from(best_offset).unwrap_or(0);
            if best_off_u32 == tok_r1 {
                std::mem::swap(&mut tok_r1, &mut tok_r0);
            } else if best_off_u32 == tok_r2 {
                std::mem::swap(&mut tok_r2, &mut tok_r0);
            } else if best_off_u32 != tok_r0 {
                tok_r2 = tok_r1;
                tok_r1 = tok_r0;
                tok_r0 = best_off_u32;
            }
        } else {
            tokens.push(LzToken::Literal(transformed[pos]));
            pos += 1;
        }
    }
    tokens
}

/// Checks `R0`, `R1`, `R2` repeat offsets for a match at `pos`.
fn find_repeat_match(
    state: &LzxState,
    transformed: &[u8],
    pos: usize,
    reps: [u32; 3],
) -> (usize, usize) {
    let mut best_len = 0;
    let mut best_offset = 0;
    for cand_offset in reps {
        let off = usize::try_from(cand_offset).unwrap_or(0);
        if off > 0 && off <= state.window_size {
            let mut match_len = 0;
            while pos + match_len < transformed.len() && match_len < LZX_MAX_MATCH {
                let target_byte = transformed[pos + match_len];
                let src_byte = if pos + match_len >= off {
                    transformed[pos + match_len - off]
                } else {
                    let win_idx = (state.window_pos + state.window_size + pos + match_len - off)
                        % state.window_size;
                    state.window[win_idx]
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
    (best_len, best_offset)
}

/// Computes Huffman symbol code lengths for the main and secondary length alphabets.
fn compute_token_tree_lengths(
    state: &LzxState,
    tokens: &[LzToken],
    slots: usize,
) -> (Vec<u8>, Vec<u8>) {
    let main_alphabet_size = 256 + 8 * slots;
    let mut main_freqs = vec![0u32; main_alphabet_size];
    let mut sec_freqs = vec![0u32; LZX_NUM_SECONDARY_LENGTHS];

    let mut sim_r0 = state.r0;
    let mut sim_r1 = state.r1;
    let mut sim_r2 = state.r2;

    for tok in tokens {
        match tok {
            LzToken::Literal(b) => {
                main_freqs[usize::from(*b)] += 1;
            }
            LzToken::Match { offset, length } => {
                let off_u32 = u32::try_from(*offset).unwrap_or(0);
                let slot = if off_u32 == sim_r0 {
                    0
                } else if off_u32 == sim_r1 {
                    1
                } else if off_u32 == sim_r2 {
                    2
                } else {
                    let raw = off_u32 + 2;
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
                    sim_r0 = off_u32;
                }
            }
        }
    }

    (
        compute_huffman_lengths(&main_freqs, 16),
        compute_huffman_lengths(&sec_freqs, 16),
    )
}

/// Writes the 20-symbol pre-tree lengths and delta-encoded main/secondary lengths.
fn write_pre_tree_and_deltas(
    state: &LzxState,
    bit_writer: &mut LzxBitWriter,
    main_lengths: &[u8],
    sec_lengths: &[u8],
) -> Result<()> {
    let mut delta_syms = Vec::new();
    for (i, &l) in main_lengths.iter().enumerate() {
        let prev = state.main_tree_lengths[i];
        let delta = (prev + 17 - l) % 17;
        delta_syms.push(delta);
    }
    for (i, &l) in sec_lengths.iter().enumerate() {
        let prev = state.secondary_tree_lengths[i];
        let delta = (prev + 17 - l) % 17;
        delta_syms.push(delta);
    }

    let mut pre_freqs = [0u32; LZX_PRE_TREE_NUM_SYMBOLS];
    for &ds in &delta_syms {
        pre_freqs[usize::from(ds)] += 1;
    }
    let pre_lengths = compute_huffman_lengths(&pre_freqs, 7);
    let pre_tree = HuffmanTree::from_lengths(&pre_lengths)?;
    let pre_codes = pre_tree.canonical_codes();

    for &pl in &pre_lengths {
        bit_writer.write_bits(u16::from(pl), 3);
    }

    for &ds in &delta_syms {
        let (c, l) = pre_codes[usize::from(ds)];
        bit_writer.write_bits_msb(c, l);
    }
    Ok(())
}

/// Emits Huffman-encoded literal and match tokens to the bitstream and updates `R0`, `R1`, `R2`.
fn write_lz_tokens(
    state: &mut LzxState,
    bit_writer: &mut LzxBitWriter,
    tokens: &[LzToken],
    slots: usize,
    main_codes: &[(u16, u8)],
    sec_codes: &[(u16, u8)],
) {
    for tok in tokens {
        match tok {
            LzToken::Literal(b) => {
                let (c, l) = main_codes[usize::from(*b)];
                bit_writer.write_bits_msb(c, l);
            }
            LzToken::Match { offset, length } => {
                let off_u32 = u32::try_from(*offset).unwrap_or(0);
                let (slot, is_rep) = if off_u32 == state.r0 {
                    (0, true)
                } else if off_u32 == state.r1 {
                    (1, true)
                } else if off_u32 == state.r2 {
                    (2, true)
                } else {
                    let raw = off_u32 + 2;
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
                        let raw = off_u32 + 2;
                        let extra = u16::try_from(raw - base).unwrap_or(0);
                        bit_writer.write_bits(extra, extra_bits);
                    }
                }

                if slot == 1 {
                    std::mem::swap(&mut state.r1, &mut state.r0);
                } else if slot == 2 {
                    std::mem::swap(&mut state.r2, &mut state.r0);
                } else if slot >= 3 {
                    state.r2 = state.r1;
                    state.r1 = state.r0;
                    state.r0 = off_u32;
                }
            }
        }
    }
}

/// Compresses a block of data, automatically selecting between Verbatim (Type 1) and Uncompressed (Type 3).
pub(super) fn compress_block_inner(state: &mut LzxState, input: &[u8]) -> Result<Vec<u8>> {
    let mut test_state = state.clone();
    if let Ok(comp_v) = compress_verbatim_block_inner(&mut test_state, input) {
        if comp_v.len() < input.len() + 10 {
            *state = test_state;
            return Ok(comp_v);
        }
    }
    compress_uncompressed_block_inner(state, input)
}

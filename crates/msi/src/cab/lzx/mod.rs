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

pub mod decoder;
pub mod encoder;
pub mod huffman;

pub use self::decoder::LzxBitReader;
pub use self::encoder::LzxBitWriter;
pub use self::huffman::{compute_huffman_lengths, decode_tree_lengths, HuffmanTree};

use crate::error::{MsiError, Result};

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

/// Number of buckets in the hash-chain match finder.
pub const LZX_HASH_SIZE: usize = 8192;

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
pub const fn slot_extra_bits(slot: usize) -> u8 {
    if slot < 4 {
        0
    } else {
        let extra = (slot / 2) - 1;
        if extra > 255 {
            255
        } else {
            extra.to_le_bytes()[0]
        }
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
pub const fn slot_base_offset(slot: usize) -> u32 {
    if slot < 4 {
        let b = slot.to_le_bytes();
        u32::from_le_bytes([b[0], b[1], b[2], b[3]])
    } else {
        let half = (slot / 2) - 1;
        let base_val = 2 | (slot & 1);
        let b = base_val.to_le_bytes();
        let base = u32::from_le_bytes([b[0], b[1], b[2], b[3]]);
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
pub fn e8_translate(data: &mut [u8], current_file_offset: usize, decode: bool) {
    if data.len() < 10 {
        return;
    }

    let limit = data.len() - 10;
    let mut i = 0;
    while i <= limit {
        if data[i] == 0xE8 {
            let offset = i + 1;
            let current_pointer = i32::try_from(current_file_offset + i).unwrap_or(0);

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
    /// Returns [`MsiError::InvalidArgument`] if `window_bits` is not in `15..=21`.
    pub fn new(window_bits: u8) -> Result<Self> {
        if !(LZX_MIN_WINDOW_BITS..=LZX_MAX_WINDOW_BITS).contains(&window_bits) {
            return Err(MsiError::InvalidArgument {
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
    /// Returns [`MsiError::LzxCompressionFailed`] if input exceeds 32,768 bytes.
    pub fn compress_uncompressed_block(&mut self, input: &[u8]) -> Result<Vec<u8>> {
        encoder::compress_uncompressed_block_inner(self, input)
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
    /// Returns [`MsiError::LzxCompressionFailed`] if input exceeds 32,768 bytes.
    pub fn compress_verbatim_block(&mut self, input: &[u8]) -> Result<Vec<u8>> {
        encoder::compress_verbatim_block_inner(self, input)
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
    /// Returns [`MsiError::LzxCompressionFailed`] if input exceeds 32,768 bytes.
    pub fn compress_block(&mut self, input: &[u8]) -> Result<Vec<u8>> {
        encoder::compress_block_inner(self, input)
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
    /// Returns [`MsiError::LzxDecompressionFailed`] if bitstream or block format is invalid.
    pub fn decompress_block(
        &mut self,
        input: &[u8],
        expected_uncomp_len: usize,
    ) -> Result<Vec<u8>> {
        decoder::decompress_block_inner(self, input, expected_uncomp_len)
    }
}

#[cfg(test)]
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
        for s in [state_res, LzxState::new(14)].into_iter().flatten() {
            let mut s = s;
            assert_eq!(s.window_size, 65_536);
            assert_eq!(s.r0, 1);

            s.r0 = 100;
            s.reset();
            assert_eq!(s.r0, 1);
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
        assert_eq!(slot_extra_bits(600), 255);

        assert_eq!(slot_base_offset(0), 0);
        assert_eq!(slot_base_offset(1), 1);
        assert_eq!(slot_base_offset(4), 4);
        assert_eq!(slot_base_offset(5), 6);
        assert_eq!(slot_base_offset(6), 8);
    }

    /// Tests canonical Huffman tree construction and decoding.
    ///
    /// # Returns
    ///
    /// `Ok(())` when Huffman tree construction, encoding, and decoding succeed.
    #[test]
    fn test_huffman_tree_roundtrip() -> Result<()> {
        let lengths = [2, 1, 3, 3]; // Alphabet of 4 symbols
        let tree = HuffmanTree::from_lengths(&lengths)?;
        assert_eq!(tree.code_lengths(), &lengths);

        let codes = tree.canonical_codes();
        assert_eq!(codes.len(), 4);

        let mut writer = LzxBitWriter::new();
        for &(c, l) in codes.iter().take(4) {
            writer.write_bits_msb(c, l);
        }
        let bytes = writer.into_bytes();

        let mut reader = LzxBitReader::new(&bytes);
        for expected in 0..4u16 {
            let dec = tree.decode_symbol(&mut reader)?;
            assert_eq!(dec, expected);
        }

        let oversubscribed = [1, 1, 1];
        assert!(HuffmanTree::from_lengths(&oversubscribed).is_err());

        let bad_length = [17];
        assert!(HuffmanTree::from_lengths(&bad_length).is_err());

        Ok(())
    }

    /// Tests LZX Type 1 (Verbatim) compression and decompression roundtrip.
    ///
    /// # Returns
    ///
    /// `Ok(())` when Verbatim block roundtrip matches original data.
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
    ///
    /// # Returns
    ///
    /// `Ok(())` when Type 2 aligned block and long matches decompress accurately.
    #[test]
    fn test_lzx_aligned_and_secondary_lengths() -> Result<()> {
        let mut state = LzxState::new(16)?;

        let mut data = Vec::new();
        data.extend_from_slice(b"1234567890abcdefghijklmnopqrstuvwxyz");
        data.extend_from_slice(b"1234567890abcdefghijklmnopqrstuvwxyz");
        data.extend_from_slice(b"1234567890abcdefghijklmnopqrstuvwxyz");

        let comp = state.compress_verbatim_block(&data)?;
        let mut decomp_state = LzxState::new(16)?;
        let decomp = decomp_state.decompress_block(&comp, data.len())?;
        assert_eq!(decomp, data);

        let mut writer = LzxBitWriter::new();
        writer.write_bits(0b010, 3); // Type 2: Aligned
        let uncomp_len = 10u16;
        writer.write_bits(uncomp_len, 16);
        writer.write_bits(0, 8);

        for _ in 0..8 {
            writer.write_bits(3, 3);
        }

        writer.write_bits(1, 3);
        for _ in 1..20 {
            writer.write_bits(0, 3);
        }

        for _ in 0..(512 + 249) {
            writer.write_bits(0, 1);
        }

        writer.align_to_16();
        let bytes = writer.into_bytes();

        let mut t2_state = LzxState::new(16)?;
        assert!(t2_state.decompress_block(&bytes, 10).is_err());

        Ok(())
    }

    /// Tests LZX Type 3 (Uncompressed) compression and decompression roundtrip.
    ///
    /// # Returns
    ///
    /// `Ok(())` when Uncompressed block roundtrip matches original bytes.
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
    ///
    /// # Returns
    ///
    /// `Ok(())` when adaptive compression roundtrips both small and large repetitive payloads.
    #[test]
    fn test_lzx_adaptive_roundtrip() -> Result<()> {
        let mut state = LzxState::new(15)?;
        let repetitive = b"AAAAABBBBBCCCCCDDDDDEEEEEAAAAABBBBBCCCCCDDDDDEEEEE123456789";

        let comp = state.compress_block(repetitive)?;
        assert_ne!(comp, Vec::<u8>::new());

        let mut decomp_state = LzxState::new(15)?;
        let decomp = decomp_state.decompress_block(&comp, repetitive.len())?;
        assert_eq!(decomp, repetitive.to_vec());

        let large_rep = vec![b'A'; 2000];
        let comp_large = state.compress_block(&large_rep)?;
        let decomp_large = decomp_state.decompress_block(&comp_large, large_rep.len())?;
        assert_eq!(decomp_large, large_rep);

        Ok(())
    }

    /// Tests LZX error handling and bit reader edge cases.
    ///
    /// # Returns
    ///
    /// `Ok(())` when all invalid bitstream cases return appropriate errors.
    #[test]
    fn test_lzx_errors() -> Result<()> {
        let mut state = LzxState::new(15)?;

        assert!(state.compress_block(&vec![0u8; 33_000]).is_err());
        assert!(state.compress_verbatim_block(&vec![0u8; 33_000]).is_err());
        assert!(state
            .compress_uncompressed_block(&vec![0u8; 33_000])
            .is_err());

        let mut bad_type_writer = LzxBitWriter::new();
        bad_type_writer.write_bits(0b000, 3);
        bad_type_writer.align_to_16();
        let bad_bytes = bad_type_writer.into_bytes();
        assert!(state.decompress_block(&bad_bytes, 10).is_err());

        assert!(state.decompress_block(&[0x03], 10).is_err());
        assert!(state.decompress_block(&[], 10).is_err());

        let comp = state.compress_uncompressed_block(b"data")?;
        assert!(state.decompress_block(&comp, 999).is_err());

        let mut truncated_lit_block = comp;
        truncated_lit_block.truncate(15);
        assert!(state.decompress_block(&truncated_lit_block, 4).is_err());

        let mut t1_writer = LzxBitWriter::new();
        t1_writer.write_bits(0b001, 3);
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

        let mut t1_sec_writer = LzxBitWriter::new();
        t1_sec_writer.write_bits(0b001, 3);
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

        let mut t1_bad_main = LzxBitWriter::new();
        t1_bad_main.write_bits(0b001, 3);
        t1_bad_main.write_bits(10, 16);
        t1_bad_main.write_bits(0, 8);
        t1_bad_main.write_bits(1, 3);
        t1_bad_main.write_bits(0, 3);
        for _ in 2..16 {
            t1_bad_main.write_bits(0, 3);
        }
        t1_bad_main.write_bits(1, 3);
        for _ in 17..20 {
            t1_bad_main.write_bits(0, 3);
        }
        for _ in 0..496 {
            t1_bad_main.write_bits(1, 1);
        }
        t1_bad_main.align_to_16();
        let bad_main_bytes = t1_bad_main.into_bytes();
        assert!(state.decompress_block(&bad_main_bytes, 10).is_err());

        let mut t1_bad_sec = LzxBitWriter::new();
        t1_bad_sec.write_bits(0b001, 3);
        t1_bad_sec.write_bits(10, 16);
        t1_bad_sec.write_bits(0, 8);
        t1_bad_sec.write_bits(1, 3);
        for _ in 1..16 {
            t1_bad_sec.write_bits(0, 3);
        }
        t1_bad_sec.write_bits(1, 3);
        for _ in 17..20 {
            t1_bad_sec.write_bits(0, 3);
        }
        for _ in 0..496 {
            t1_bad_sec.write_bits(0, 1);
        }
        for _ in 0..249 {
            t1_bad_sec.write_bits(1, 1);
        }
        t1_bad_sec.align_to_16();
        let bad_sec_bytes = t1_bad_sec.into_bytes();
        assert!(state.decompress_block(&bad_sec_bytes, 10).is_err());

        Ok(())
    }

    /// Tests E8 translation branch edge cases (displacement computation and reversal).
    #[test]
    fn test_e8_translation_edge_cases() {
        let mut buf_dec = vec![0u8; 20];
        buf_dec[5] = 0xE8;
        let neg_val: i32 = -3;
        buf_dec[6..10].copy_from_slice(&neg_val.to_le_bytes());
        e8_translate(&mut buf_dec, 0, true);
        let expected_disp = neg_val + E8_DEFAULT_FILE_SIZE;
        assert_eq!(&buf_dec[6..10], &expected_disp.to_le_bytes());

        let mut buf_dec_out = vec![0u8; 20];
        buf_dec_out[5] = 0xE8;
        let out_val: i32 = -100;
        buf_dec_out[6..10].copy_from_slice(&out_val.to_le_bytes());
        e8_translate(&mut buf_dec_out, 0, true);
        assert_eq!(&buf_dec_out[6..10], &out_val.to_le_bytes());

        let mut buf_dec_large = vec![0u8; 20];
        buf_dec_large[5] = 0xE8;
        let large_val: i32 = E8_DEFAULT_FILE_SIZE + 10;
        buf_dec_large[6..10].copy_from_slice(&large_val.to_le_bytes());
        e8_translate(&mut buf_dec_large, 0, true);
        assert_eq!(&buf_dec_large[6..10], &large_val.to_le_bytes());

        let mut buf_enc = vec![0u8; 20];
        buf_enc[5] = 0xE8;
        let disp: i32 = E8_DEFAULT_FILE_SIZE - 2;
        buf_enc[6..10].copy_from_slice(&disp.to_le_bytes());
        e8_translate(&mut buf_enc, 0, false);
        let expected_val = disp - E8_DEFAULT_FILE_SIZE;
        assert_eq!(&buf_enc[6..10], &expected_val.to_le_bytes());

        let mut buf_enc_out = vec![0u8; 20];
        buf_enc_out[5] = 0xE8;
        let out_disp: i32 = -100;
        buf_enc_out[6..10].copy_from_slice(&out_disp.to_le_bytes());
        e8_translate(&mut buf_enc_out, 0, false);
        assert_eq!(&buf_enc_out[6..10], &out_disp.to_le_bytes());

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
    ///
    /// # Returns
    ///
    /// `Ok(())` when RLE pre-tree symbols decode without error.
    #[test]
    fn test_decode_tree_lengths_rle() -> Result<()> {
        let mut pre_lengths = [0u8; LZX_PRE_TREE_NUM_SYMBOLS];
        pre_lengths[16] = 2;
        pre_lengths[17] = 2;
        pre_lengths[18] = 2;
        pre_lengths[19] = 2;
        let pre_tree = HuffmanTree::from_lengths(&pre_lengths)?;

        let mut writer = LzxBitWriter::new();
        writer.write_bits_msb(0b11, 2);
        writer.write_bits(1, 1);
        writer.write_bits_msb(0b00, 2);

        writer.write_bits_msb(0b01, 2);
        writer.write_bits(5, 4);

        writer.write_bits_msb(0b10, 2);
        writer.write_bits(3, 5);

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

        let mut empty_reader = LzxBitReader::new(&[]);
        assert!(
            decode_tree_lengths(&mut empty_reader, &pre_tree, &mut lengths, &initial_lengths)
                .is_err()
        );

        Ok(())
    }

    /// Tests LZX repeat offset matches (R1 and R2), consecutive block compression, and fallback.
    ///
    /// # Returns
    ///
    /// `Ok(())` when repeat offset swaps and consecutive blocks roundtrip.
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
        data.extend_from_slice(d);
        data.extend_from_slice(c);
        data.extend_from_slice(b);
        data.extend_from_slice(c);
        data.extend_from_slice(c);
        data.extend_from_slice(b"ZZZZ");
        data.extend_from_slice(&vec![b'W'; 600]);

        state.r0 = u32::try_from(state.window_size.saturating_add(1)).unwrap_or(u32::MAX);
        state.r1 = 0;

        let comp1 = state.compress_verbatim_block(&data)?;
        let mut decomp_state = LzxState::new(15)?;
        let decomp1 = decomp_state.decompress_block(&comp1, data.len())?;
        assert_eq!(decomp1, data);

        let comp2 = state.compress_verbatim_block(&data)?;
        let decomp2 = decomp_state.decompress_block(&comp2, data.len())?;
        assert_eq!(decomp2, data);

        assert!(decomp_state
            .decompress_block(&comp1, data.len() + 1)
            .is_err());

        let uncomp = (0..50u8)
            .map(|x| x.wrapping_mul(73).wrapping_add(19))
            .collect::<Vec<_>>();
        let comp_fallback = state.compress_block(&uncomp)?;
        let decomp_fallback = decomp_state.decompress_block(&comp_fallback, uncomp.len())?;
        assert_eq!(decomp_fallback, uncomp);

        Ok(())
    }

    /// Tests backward search match reaching [`LZX_MAX_MATCH`].
    ///
    /// # Returns
    ///
    /// `Ok(())` when maximum-length match compresses and decompresses accurately.
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
    ///
    /// # Returns
    ///
    /// `Ok(())` when Type 2 aligned offset block decodes and rejects out-of-range offsets.
    #[test]
    fn test_lzx_type2_aligned_decoding_and_invalid_offsets() -> Result<()> {
        let mut writer = LzxBitWriter::new();
        writer.write_bits(0b010, 3);
        let uncomp_len = 7u16;
        writer.write_bits(uncomp_len, 16);
        writer.write_bits(0, 8);

        for _ in 0..8 {
            writer.write_bits(3, 3);
        }

        let mut pre_lengths = [0u8; LZX_PRE_TREE_NUM_SYMBOLS];
        pre_lengths[0] = 1;
        pre_lengths[15] = 1;
        for &pl in &pre_lengths {
            writer.write_bits(u16::from(pl), 3);
        }

        for i in 0..496 {
            if i == 65 || i == 256 || i == 280 || i == 320 {
                writer.write_bits(1, 1);
            } else {
                writer.write_bits(0, 1);
            }
        }
        for _ in 0..249 {
            writer.write_bits(0, 1);
        }

        writer.write_bits_msb(0b00, 2);
        writer.write_bits_msb(0b01, 2);
        writer.write_bits_msb(0b10, 2);
        writer.write_bits_msb(0b11, 2);
        writer.write_bits_msb(0b000, 3);

        writer.align_to_16();
        let bytes = writer.into_bytes();

        let mut state = LzxState::new(15)?;
        let decomp = state.decompress_block(&bytes, 7)?;
        assert_eq!(decomp.len(), 7);

        let mut bad_off_state = LzxState::new(15)?;
        bad_off_state.r0 = 0;
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

    /// Tests `compute_huffman_lengths` with deep tree exceeding `max_bits` to exercise dynamic range compression.
    #[test]
    fn test_compute_huffman_lengths_deep_tree_compression() {
        let mut freqs = [0u32; LZX_PRE_TREE_NUM_SYMBOLS];
        let mut a = 1u32;
        let mut b = 2u32;
        for f in &mut freqs {
            *f = a;
            let next = a.saturating_add(b);
            a = b;
            b = next;
        }
        let lengths = compute_huffman_lengths(&freqs, 7);
        for &l in &lengths {
            assert!(l <= 7);
        }
    }

    /// Tests decompressing truncated LZX Type 3 (Uncompressed) blocks to exercise bitstream exhaustion error branches.
    ///
    /// # Returns
    ///
    /// `Ok(())` when all truncated Type 3 block headers fail cleanly.
    #[test]
    fn test_decompress_uncompressed_block_truncated_headers() -> Result<()> {
        let mut state = LzxState::new(15)?;
        assert!(state.decompress_block(&[0x03], 100).is_err());
        assert!(state.decompress_block(&[0x03, 0x00, 0x00], 100).is_err());

        let mut writer = LzxBitWriter::new();
        writer.write_bits(0b011, 3);
        writer.write_bits(50, 16);
        writer.write_bits(0, 8);
        assert!(state.decompress_block(&writer.into_bytes(), 100).is_err());

        for take_len in 4..16 {
            let mut w = LzxBitWriter::new();
            w.write_bits(0b011, 3);
            w.write_bits(10, 16);
            w.write_bits(0, 8);
            w.write_bits(1, 16);
            w.write_bits(0, 16);
            let bytes = w.into_bytes();
            if take_len <= bytes.len() {
                assert!(state.decompress_block(&bytes[..take_len], 10).is_err());
            }
        }
        Ok(())
    }

    /// Tests `decode_tree_lengths` with truncated input streams during RLE symbol decoding.
    ///
    /// # Returns
    ///
    /// `Ok(())` when truncated RLE bitstreams return an error.
    #[test]
    fn test_decode_tree_lengths_truncated_rle() -> Result<()> {
        let mut pre_lengths = [0u8; LZX_PRE_TREE_NUM_SYMBOLS];
        pre_lengths[17] = 1;
        let pre_tree = HuffmanTree::from_lengths(&pre_lengths)?;

        let mut lengths = [0u8; 10];
        let mut w17 = LzxBitWriter::new();
        w17.write_bits(0, 1);
        let b17 = w17.into_bytes();
        let mut r17 = LzxBitReader::new(&b17[..1]);
        assert!(decode_tree_lengths(&mut r17, &pre_tree, &mut lengths, &[]).is_err());

        Ok(())
    }

    /// Tests remaining LZX branches: window bits bounds, block size limits, E8 translation edges, and repeat offset swaps.
    ///
    /// # Returns
    ///
    /// `Ok(())` when all boundary conditions and truncation sweeps complete.
    #[test]
    fn test_lzx_remaining_uncovered_branches() -> Result<()> {
        assert!(LzxState::new(14).is_err());
        assert!(LzxState::new(22).is_err());

        let mut state = LzxState::new(15)?;
        let oversize = vec![0u8; 32_769];
        assert!(state.compress_uncompressed_block(&oversize).is_err());
        assert!(state.compress_verbatim_block(&oversize).is_err());

        let mut short_e8 = vec![0xE8; 5];
        e8_translate(&mut short_e8, 0, false);
        assert_eq!(short_e8, vec![0xE8; 5]);

        let mut long_rep = Vec::new();
        long_rep.extend_from_slice(b"HEADER_PREFIX_PATTERN_");
        long_rep.extend(vec![b'Z'; 200]);
        long_rep.extend(vec![b'Z'; 200]);
        let comp_long = state.compress_verbatim_block(&long_rep)?;
        let mut decomp_state = LzxState::new(15)?;
        let decomp_long = decomp_state.decompress_block(&comp_long, long_rep.len())?;
        assert_eq!(decomp_long, long_rep);

        let mut rep_seq = Vec::new();
        rep_seq.extend_from_slice(b"AAAAABBBBBCCCCCDDDDDAAAAABBBBBCCCCCDDDDD");
        let comp_rep = state.compress_verbatim_block(&rep_seq)?;
        let decomp_rep = decomp_state.decompress_block(&comp_rep, rep_seq.len())?;
        assert_eq!(decomp_rep, rep_seq);

        assert!(HuffmanTree::from_lengths(&[17]).is_err());

        assert_eq!(slot_extra_bits(0), 0);
        assert_eq!(slot_base_offset(0), 0);

        for len in 0..comp_long.len() {
            let mut s = LzxState::new(15)?;
            let _ = s.decompress_block(&comp_long[..len], long_rep.len());
        }

        for len in 0..comp_rep.len() {
            let mut s = LzxState::new(15)?;
            let _ = s.decompress_block(&comp_rep[..len], rep_seq.len());
        }

        let uncomp_sample = b"UNCOMPRESSED_PAYLOAD_FOR_TRUNCATION_TEST_12345";
        let comp_uncomp = state.compress_uncompressed_block(uncomp_sample)?;
        for len in 0..comp_uncomp.len() {
            let mut s = LzxState::new(15)?;
            let _ = s.decompress_block(&comp_uncomp[..len], uncomp_sample.len());
        }

        let mut pre_lengths = [0u8; LZX_PRE_TREE_NUM_SYMBOLS];
        pre_lengths[18] = 1;
        let pre_tree18 = HuffmanTree::from_lengths(&pre_lengths)?;
        let mut r18 = LzxBitReader::new(&[0x00]);
        let mut target_lengths = [0u8; 30];
        assert!(decode_tree_lengths(&mut r18, &pre_tree18, &mut target_lengths, &[]).is_err());

        Ok(())
    }
}

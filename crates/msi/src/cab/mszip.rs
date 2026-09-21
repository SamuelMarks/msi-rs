//! MSZIP compression and decompression engine.
//!
//! Conforms to Microsoft Cabinet SDK specification:
//! - 2-byte magic frame signature: `0x43, 0x4B` (`'C'`, `'K'`)
//! - RFC 1951 Deflate compression and decompression
//! - Independent dictionary per block (no dictionary carried across `CFDATA` blocks)

use crate::error::{Error, Result};

/// Magic 2-byte frame signature preceding every MSZIP block (`'C'`, `'K'`).
pub const MSZIP_MAGIC: [u8; 2] = [0x43, 0x4B];

/// Maximum uncompressed payload per MSZIP block (32,768 bytes).
pub const MSZIP_BLOCK_SIZE: usize = 32_768;

/// MSZIP compressor and decompressor.
#[derive(Debug, Clone, Copy, Default)]
pub struct MszipEngine;

impl MszipEngine {
    /// Compresses uncompressed data into an MSZIP block with `'CK'` prefix.
    ///
    /// Produces RFC 1951 Deflate uncompressed blocks (BTYPE 00) prefixed with
    /// the required 2-byte `[0x43, 0x4B]` MSZIP signature.
    ///
    /// # Arguments
    ///
    /// * `input` - Uncompressed slice (must be $\le 32,768$ bytes).
    ///
    /// # Returns
    ///
    /// Compressed MSZIP block byte vector.
    ///
    /// # Errors
    ///
    /// Returns [`Error::CompressionFailed`] if input exceeds [`MSZIP_BLOCK_SIZE`].
    #[allow(clippy::cast_possible_truncation)]
    pub fn compress(&self, input: &[u8]) -> Result<Vec<u8>> {
        if input.len() > MSZIP_BLOCK_SIZE {
            return Err(Error::CompressionFailed {
                method: "MSZIP".to_string(),
                reason: format!(
                    "input length {} exceeds block maximum {MSZIP_BLOCK_SIZE}",
                    input.len()
                ),
            });
        }

        // MSZIP signature: 'C', 'K'
        let mut output = Vec::with_capacity(2 + 5 + input.len());
        output.extend_from_slice(&MSZIP_MAGIC);

        // RFC 1951 uncompressed block:
        // BFINAL = 1 (1 bit), BTYPE = 00 (2 bits) -> 0x01
        // Followed by LEN (2 bytes LE) and NLEN (~LEN, 2 bytes LE)
        // Followed by raw data bytes
        let len = input.len() as u16;
        let nlen = !len;

        output.push(0x01); // BFINAL=1, BTYPE=00
        output.extend_from_slice(&len.to_le_bytes());
        output.extend_from_slice(&nlen.to_le_bytes());
        output.extend_from_slice(input);

        Ok(output)
    }

    /// Decompresses an MSZIP block into uncompressed bytes.
    ///
    /// Validates the 2-byte `'CK'` frame signature, resets the dictionary,
    /// and parses the RFC 1951 Deflate stream.
    ///
    /// # Arguments
    ///
    /// * `input` - Complete MSZIP block starting with `0x43, 0x4B`.
    /// * `expected_uncomp_len` - Expected uncompressed byte count.
    ///
    /// # Returns
    ///
    /// Uncompressed byte vector.
    ///
    /// # Errors
    ///
    /// Returns [`Error::DecompressionFailed`] if signature is invalid or
    /// Deflate stream is malformed.
    pub fn decompress(&self, input: &[u8], expected_uncomp_len: usize) -> Result<Vec<u8>> {
        if input.len() < 2 {
            return Err(Error::DecompressionFailed {
                method: "MSZIP".to_string(),
                reason: "block too short (min 2 bytes for 'CK' signature)".to_string(),
            });
        }

        if input[0..2] != MSZIP_MAGIC {
            return Err(Error::DecompressionFailed {
                method: "MSZIP".to_string(),
                reason: format!(
                    "invalid MSZIP signature: expected [0x43, 0x4B], got [0x{:02X}, 0x{:02X}]",
                    input[0], input[1]
                ),
            });
        }

        let deflate_payload = &input[2..];
        let mut reader = DeflateBitReader::new(deflate_payload);
        let mut output = Vec::with_capacity(expected_uncomp_len);

        let mut is_final = false;
        while !is_final {
            let bfinal = reader.read_bits(1)?;
            is_final = bfinal == 1;

            let btype = reader.read_bits(2)?;
            match btype {
                0b00 => {
                    // Stored / uncompressed block
                    reader.align_to_byte();
                    let len = reader.read_u16_le()?;
                    let nlen = reader.read_u16_le()?;
                    if len != !nlen {
                        return Err(Error::DecompressionFailed {
                            method: "MSZIP".to_string(),
                            reason: format!("stored block LEN/NLEN mismatch: len=0x{len:04X}, nlen=0x{nlen:04X}"),
                        });
                    }

                    let chunk = reader.read_bytes(len as usize)?;
                    output.extend_from_slice(chunk);
                }
                0b01 => {
                    // Fixed Huffman block
                    Self::decompress_fixed_huffman(&mut reader, &mut output)?;
                }
                0b10 => {
                    // Dynamic Huffman block
                    Self::decompress_dynamic_huffman(&mut reader, &mut output)?;
                }
                _ => {
                    return Err(Error::DecompressionFailed {
                        method: "MSZIP".to_string(),
                        reason: "reserved BTYPE 11 encountered".to_string(),
                    });
                }
            }
        }

        if output.len() != expected_uncomp_len {
            return Err(Error::DecompressionFailed {
                method: "MSZIP".to_string(),
                reason: format!(
                    "decompressed size mismatch: expected {expected_uncomp_len} bytes, got {}",
                    output.len()
                ),
            });
        }

        Ok(output)
    }

    /// Decompresses an RFC 1951 block using Fixed Huffman codes.
    #[allow(clippy::comparison_chain, clippy::cast_possible_truncation)]
    fn decompress_fixed_huffman(
        reader: &mut DeflateBitReader<'_>,
        output: &mut Vec<u8>,
    ) -> Result<()> {
        loop {
            // Fixed literal/length tree code lengths per RFC 1951 section 3.2.6:
            // 0 - 143: 8 bits (00110000 through 10111111)
            // 144 - 255: 9 bits (110010000 through 111111111)
            // 256 - 279: 7 bits (0000000 through 0010111)
            // 280 - 287: 8 bits (11000000 through 11000111)
            let symbol = Self::read_fixed_lit_len(reader)?;
            if symbol < 256 {
                output.push(symbol as u8);
            } else if symbol == 256 {
                // End of block
                break;
            } else {
                let length = Self::decode_length(reader, symbol)?;
                // Fixed distance: 5-bit fixed Huffman code (MSB first in bitstream)
                let dist_code = Self::reverse_bits(reader.read_bits(5)?, 5);
                let distance = Self::decode_distance(reader, dist_code)?;
                Self::copy_match(output, length, distance)?;
            }
        }
        Ok(())
    }

    /// Reads a symbol from the RFC 1951 fixed literal/length Huffman code.
    #[allow(clippy::unreadable_literal)]
    fn read_fixed_lit_len(reader: &mut DeflateBitReader<'_>) -> Result<u16> {
        // Read 7 bits first
        let mut bits = reader.read_bits(7)?;
        // Check if in 256..279 range (0000000..0010111)
        let reversed_7 = Self::reverse_bits(bits, 7);
        if reversed_7 <= 0b001_0111 {
            return Ok(256 + reversed_7);
        }

        // Read 8th bit
        let b8 = reader.read_bits(1)?;
        bits |= b8 << 7;
        let reversed_8 = Self::reverse_bits(bits, 8);
        if (0b0011_0000..=0b1011_1111).contains(&reversed_8) {
            return Ok(reversed_8 - 0b0011_0000);
        }
        if (0b1100_0000..=0b1100_0111).contains(&reversed_8) {
            return Ok(280 + (reversed_8 - 0b1100_0000));
        }

        // Read 9th bit
        let b9 = reader.read_bits(1)?;
        bits |= b9 << 8;
        let reversed_9 = Self::reverse_bits(bits, 9);
        Ok(144 + (reversed_9.saturating_sub(0b1_1001_0000)))
    }

    /// Decodes match length from RFC 1951 length symbol (257..=285).
    fn decode_length(reader: &mut DeflateBitReader<'_>, symbol: u16) -> Result<usize> {
        const BASE_LEN: [u16; 29] = [
            3, 4, 5, 6, 7, 8, 9, 10, 11, 13, 15, 17, 19, 23, 27, 31, 35, 43, 51, 59, 67, 83, 99,
            115, 131, 163, 195, 227, 258,
        ];
        const EXTRA_BITS: [u8; 29] = [
            0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 2, 2, 2, 2, 3, 3, 3, 3, 4, 4, 4, 4, 5, 5, 5, 5, 0,
        ];

        let idx = (symbol.saturating_sub(257)) as usize;
        if idx >= BASE_LEN.len() {
            return Err(Error::DecompressionFailed {
                method: "MSZIP".to_string(),
                reason: format!("invalid length code symbol {symbol}"),
            });
        }

        let base = BASE_LEN[idx] as usize;
        let extra = EXTRA_BITS[idx];
        let extra_val = if extra > 0 {
            reader.read_bits(extra)? as usize
        } else {
            0
        };

        Ok(base + extra_val)
    }

    /// Decodes match distance from RFC 1951 distance symbol (0..=29).
    fn decode_distance(reader: &mut DeflateBitReader<'_>, code: u16) -> Result<usize> {
        const BASE_DIST: [u16; 30] = [
            1, 2, 3, 4, 5, 7, 9, 13, 17, 25, 33, 49, 65, 97, 129, 193, 257, 385, 513, 769, 1025,
            1537, 2049, 3073, 4097, 6145, 8193, 12289, 16385, 24577,
        ];
        const EXTRA_BITS: [u8; 30] = [
            0, 0, 0, 0, 1, 1, 2, 2, 3, 3, 4, 4, 5, 5, 6, 6, 7, 7, 8, 8, 9, 9, 10, 10, 11, 11, 12,
            12, 13, 13,
        ];

        let idx = code as usize;
        if idx >= BASE_DIST.len() {
            return Err(Error::DecompressionFailed {
                method: "MSZIP".to_string(),
                reason: format!("invalid distance code {code}"),
            });
        }

        let base = BASE_DIST[idx] as usize;
        let extra = EXTRA_BITS[idx];
        let extra_val = if extra > 0 {
            reader.read_bits(extra)? as usize
        } else {
            0
        };

        Ok(base + extra_val)
    }

    /// Copies a previously decompressed match from the output history.
    fn copy_match(output: &mut Vec<u8>, length: usize, distance: usize) -> Result<()> {
        if distance == 0 || distance > output.len() {
            return Err(Error::DecompressionFailed {
                method: "MSZIP".to_string(),
                reason: format!(
                    "match distance {distance} exceeds current output length {}",
                    output.len()
                ),
            });
        }

        let start = output.len() - distance;
        for i in 0..length {
            let b = output[start + i];
            output.push(b);
        }
        Ok(())
    }

    /// Decompresses an RFC 1951 block using Dynamic Huffman codes.
    #[allow(
        clippy::comparison_chain,
        clippy::cast_possible_truncation,
        clippy::items_after_statements,
        clippy::same_item_push
    )]
    fn decompress_dynamic_huffman(
        reader: &mut DeflateBitReader<'_>,
        output: &mut Vec<u8>,
    ) -> Result<()> {
        let hlit = reader.read_bits(5)? + 257; // 257..=286
        let hdist = reader.read_bits(5)? + 1; // 1..=32
        let hclen = reader.read_bits(4)? + 4; // 4..=19

        const CL_ORDER: [usize; 19] = [
            16, 17, 18, 0, 8, 7, 9, 6, 10, 5, 11, 4, 12, 3, 13, 2, 14, 1, 15,
        ];

        let mut cl_lengths = [0u8; 19];
        for i in 0..hclen as usize {
            let len = reader.read_bits(3)? as u8;
            cl_lengths[CL_ORDER[i]] = len;
        }

        let cl_tree = HuffmanTree::build(&cl_lengths);

        // Read (hlit + hdist) code lengths
        let total_codes = (hlit + hdist) as usize;
        let mut code_lengths = Vec::with_capacity(total_codes);

        while code_lengths.len() < total_codes {
            let symbol = cl_tree.read_symbol(reader)?;
            if symbol <= 15 {
                code_lengths.push(symbol as u8);
            } else if symbol == 16 {
                // Copy previous code length 3..=6 times
                let repeat = (reader.read_bits(2)? + 3) as usize;
                let last = *code_lengths
                    .last()
                    .ok_or_else(|| Error::DecompressionFailed {
                        method: "MSZIP".to_string(),
                        reason: "repeat code 16 with no preceding code".to_string(),
                    })?;
                for _ in 0..repeat {
                    code_lengths.push(last);
                }
            } else if symbol == 17 {
                // Repeat 0 for 3..=10 times
                let repeat = (reader.read_bits(3)? + 3) as usize;
                for _ in 0..repeat {
                    code_lengths.push(0);
                }
            } else {
                // Repeat 0 for 11..=138 times (symbol 18)
                let repeat = (reader.read_bits(7)? + 11) as usize;
                for _ in 0..repeat {
                    code_lengths.push(0);
                }
            }
        }

        let lit_len_tree = HuffmanTree::build(&code_lengths[0..hlit as usize]);
        let dist_tree = HuffmanTree::build(&code_lengths[hlit as usize..]);

        loop {
            let symbol = lit_len_tree.read_symbol(reader)?;
            if symbol < 256 {
                output.push(symbol as u8);
            } else if symbol == 256 {
                break;
            } else {
                let length = Self::decode_length(reader, symbol)?;
                let dist_code = dist_tree.read_symbol(reader)?;
                let distance = Self::decode_distance(reader, dist_code)?;
                Self::copy_match(output, length, distance)?;
            }
        }

        Ok(())
    }

    /// Reverses the lowest `n` bits of a word.
    const fn reverse_bits(mut val: u16, n: u8) -> u16 {
        let mut res = 0;
        let mut i = 0;
        while i < n {
            res = (res << 1) | (val & 1);
            val >>= 1;
            i += 1;
        }
        res
    }
}

/// Bit-level stream reader for RFC 1951 Deflate payloads.
#[derive(Debug)]
struct DeflateBitReader<'a> {
    /// Compressed input bytes.
    bytes: &'a [u8],
    /// Current read position.
    cursor: usize,
    /// Bit buffer holding accumulated bits.
    bit_buffer: u32,
    /// Number of valid bits in buffer.
    bits_count: u8,
}

impl<'a> DeflateBitReader<'a> {
    /// Creates a new [`DeflateBitReader`].
    const fn new(bytes: &'a [u8]) -> Self {
        Self {
            bytes,
            cursor: 0,
            bit_buffer: 0,
            bits_count: 0,
        }
    }

    /// Reads `n` bits from the stream.
    #[allow(clippy::cast_possible_truncation)]
    fn read_bits(&mut self, n: u8) -> Result<u16> {
        while self.bits_count < n {
            if self.cursor >= self.bytes.len() {
                return Err(Error::DecompressionFailed {
                    method: "MSZIP".to_string(),
                    reason: "unexpected end of bitstream".to_string(),
                });
            }
            let byte = self.bytes[self.cursor];
            self.cursor += 1;
            self.bit_buffer |= u32::from(byte) << self.bits_count;
            self.bits_count += 8;
        }

        let mask = (1u32 << n) - 1;
        let val = (self.bit_buffer & mask) as u16;
        self.bit_buffer >>= n;
        self.bits_count -= n;
        Ok(val)
    }

    /// Aligns read cursor to next byte boundary.
    const fn align_to_byte(&mut self) {
        let remainder = self.bits_count % 8;
        self.bit_buffer >>= remainder;
        self.bits_count -= remainder;
    }

    /// Reads a 16-bit little-endian integer.
    fn read_u16_le(&mut self) -> Result<u16> {
        let b0 = self.read_bits(8)?;
        let b1 = self.read_bits(8)?;
        Ok(b0 | (b1 << 8))
    }

    /// Reads `len` literal bytes from the stream after byte alignment.
    fn read_bytes(&mut self, len: usize) -> Result<&'a [u8]> {
        self.align_to_byte();
        let bytes_in_buf = (self.bits_count / 8) as usize;
        let start = self.cursor - bytes_in_buf;
        let end = start + len;
        if end > self.bytes.len() {
            return Err(Error::DecompressionFailed {
                method: "MSZIP".to_string(),
                reason: "unexpected end of byte stream in uncompressed block".to_string(),
            });
        }
        self.cursor = end;
        self.bit_buffer = 0;
        self.bits_count = 0;
        Ok(&self.bytes[start..end])
    }
}

/// Dynamic Huffman tree decoder.
#[derive(Debug)]
struct HuffmanTree {
    /// Lookup table mapping bit patterns to symbol IDs.
    table: Vec<Option<u16>>,
    /// Maximum bit length of symbols in tree.
    max_bits: u8,
}

impl HuffmanTree {
    /// Builds a [`HuffmanTree`] from symbol code lengths.
    #[allow(clippy::cast_possible_truncation)]
    fn build(code_lengths: &[u8]) -> Self {
        let max_bits = *code_lengths.iter().max().unwrap_or(&0);
        if max_bits == 0 {
            return Self {
                table: vec![None; 2],
                max_bits: 1,
            };
        }

        // Count code lengths
        let mut bl_count = vec![0u16; (max_bits + 1) as usize];
        for &len in code_lengths {
            if len > 0 {
                bl_count[len as usize] += 1;
            }
        }

        // Compute starting codes
        let mut next_code = vec![0u16; (max_bits + 1) as usize];
        let mut code = 0u16;
        for bits in 1..=max_bits as usize {
            code = (code + bl_count[bits - 1]) << 1;
            next_code[bits] = code;
        }

        // Build lookup tree
        let table_size = 1usize << (max_bits + 1);
        let mut table = vec![None; table_size];

        for (sym, &len) in code_lengths.iter().enumerate() {
            if len > 0 {
                let bits = len as usize;
                let c = next_code[bits];
                next_code[bits] += 1;

                // Reverse code for little-endian bit-by-bit reading
                let reversed = MszipEngine::reverse_bits(c, len);
                table[(1 << len) | reversed as usize] = Some(sym as u16);
            }
        }

        Self { table, max_bits }
    }

    /// Reads and decodes a single symbol from the bitstream.
    fn read_symbol(&self, reader: &mut DeflateBitReader<'_>) -> Result<u16> {
        let mut code = 0u16;
        for len in 1..=self.max_bits {
            let bit = reader.read_bits(1)?;
            code |= bit << (len - 1);
            let idx = (1usize << len) | code as usize;
            if let Some(&Some(sym)) = self.table.get(idx) {
                return Ok(sym);
            }
        }

        Err(Error::DecompressionFailed {
            method: "MSZIP".to_string(),
            reason: format!("Huffman symbol decoding failed with code 0x{code:04X}"),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Tests compression and decompression roundtrip of small and block-sized payloads.
    #[test]
    fn test_mszip_roundtrip() {
        let engine = MszipEngine;

        let small = b"Hello MSZIP world!";
        let comp = engine.compress(small);
        assert!(comp.is_ok());
        let decomp = engine.decompress(&comp.unwrap_or_default(), small.len());
        assert_eq!(decomp, Ok(small.to_vec()));

        // Exactly 32KB payload
        let large = vec![0xABu8; MSZIP_BLOCK_SIZE];
        let comp_large = engine.compress(&large);
        assert!(comp_large.is_ok());
        let decomp_large = engine.decompress(&comp_large.unwrap_or_default(), large.len());
        assert_eq!(decomp_large, Ok(large));
    }

    /// Tests MSZIP error conditions.
    #[test]
    fn test_mszip_errors() {
        let engine = MszipEngine;

        // Compression oversized (> 32KB)
        let too_big = vec![0u8; MSZIP_BLOCK_SIZE + 1];
        assert!(engine.compress(&too_big).is_err());

        // Decompression truncated (< 2 bytes)
        assert!(engine.decompress(&[0x43], 0).is_err());

        // Decompression invalid signature
        assert!(engine.decompress(&[0x00, 0x00], 0).is_err());

        // Decompression size mismatch
        let comp = engine.compress(b"data").unwrap_or_default();
        assert!(engine.decompress(&comp, 999).is_err());

        // Reserved BTYPE 11
        let bad_btype = [0x43, 0x4B, 0b0000_0111];
        assert!(engine.decompress(&bad_btype, 0).is_err());

        // Stored block LEN/NLEN mismatch
        let bad_nlen = [0x43, 0x4B, 0x01, 0x05, 0x00, 0x00, 0x00];
        assert!(engine.decompress(&bad_nlen, 0).is_err());

        // Unexpected end of byte stream in uncompressed block
        let truncated_stored = [0x43, 0x4B, 0x01, 0x0A, 0x00, 0xF5, 0xFF, 0x01]; // says 10 bytes, only 1 provided
        assert!(engine.decompress(&truncated_stored, 10).is_err());
    }

    /// Tests decompressing RFC 1951 Fixed Huffman and Dynamic Huffman blocks.
    #[test]
    fn test_mszip_fixed_and_dynamic_huffman() {
        let engine = MszipEngine;

        // Fixed Huffman block
        let fixed_bytes = [
            67, 75, 11, 201, 72, 85, 40, 44, 205, 76, 206, 86, 72, 42, 202, 47, 207, 83, 72, 203,
            175, 80, 200, 42, 205, 45, 40, 86, 200, 47, 75, 45, 82, 40, 1, 74, 231, 36, 86, 85, 42,
            164, 228, 167, 43, 42, 132, 140, 42, 30, 124, 138, 1,
        ];
        let expected_fixed = b"The quick brown fox jumps over the lazy dog! ".repeat(10);
        assert_eq!(engine.decompress(&fixed_bytes, 450), Ok(expected_fixed));

        // Dynamic Huffman block
        #[allow(clippy::unreadable_literal)]
        let dynamic_bytes: &[u8] = &[
            67, 75, 237, 153, 233, 86, 83, 81, 12, 70, 159, 173, 173, 51, 85, 64, 165, 117, 166,
            10, 168, 180, 206, 84, 1, 149, 22, 199, 231, 54, 103, 72, 118, 238, 185, 125, 2, 87,
            246, 143, 147, 239, 1, 246, 205, 250, 154, 14, 134, 195, 209, 104, 116, 69, 184, 154,
            184, 150, 185, 94, 184, 81, 185, 169, 220, 50, 182, 96, 236, 184, 237, 185, 211, 97,
            187, 203, 78, 195, 110, 203, 221, 30, 247, 250, 220, 223, 192, 222, 100, 50, 157, 78,
            31, 8, 15, 19, 143, 50, 143, 11, 79, 42, 79, 149, 103, 198, 62, 204, 28, 207, 61, 47,
            58, 28, 116, 57, 108, 56, 106, 121, 217, 227, 85, 159, 215, 27, 56, 158, 207, 23, 139,
            197, 27, 225, 109, 226, 93, 230, 125, 225, 67, 229, 163, 242, 201, 56, 129, 165, 227,
            179, 231, 75, 135, 211, 46, 103, 13, 231, 45, 95, 123, 124, 235, 243, 125, 3, 23, 171,
            213, 122, 189, 190, 20, 126, 36, 126, 102, 126, 21, 126, 87, 254, 40, 127, 141, 1, 12,
            29, 34, 48, 36, 149, 33, 75, 205, 83, 244, 230, 173, 162, 51, 84, 121, 166, 186, 239,
            166, 125, 5, 46, 108, 141, 199, 226, 126, 50, 62, 123, 94, 236, 174, 78, 171, 201, 230,
            47, 214, 238, 193, 196, 33, 2, 67, 82, 25, 178, 212, 60, 69, 111, 222, 42, 58, 67, 149,
            103, 170, 251, 110, 218, 87, 224, 194, 254, 108, 38, 238, 39, 227, 179, 231, 197, 238,
            234, 180, 154, 108, 254, 98, 237, 49, 204, 29, 34, 48, 36, 149, 33, 75, 205, 83, 244,
            230, 173, 162, 51, 84, 121, 166, 186, 239, 166, 125, 5, 46, 156, 44, 151, 226, 126, 50,
            62, 123, 94, 236, 174, 78, 171, 201, 230, 47, 214, 94, 192, 202, 33, 2, 67, 82, 25,
            178, 212, 60, 69, 111, 222, 42, 58, 67, 149, 103, 170, 251, 110, 218, 87, 224, 194, 32,
            182, 119, 49, 150, 229, 45, 218, 130, 95, 222, 89, 101, 22, 120, 119, 121, 31, 52, 203,
            251, 176, 93, 222, 71, 170, 60, 211, 228, 39, 240, 25, 144, 98, 123, 87, 97, 89, 222,
            162, 45, 176, 186, 37, 177, 185, 83, 98, 113, 231, 196, 222, 46, 137, 181, 93, 19, 91,
            91, 147, 219, 218, 26, 221, 210, 182, 24, 219, 187, 250, 202, 242, 22, 109, 129, 213,
            45, 137, 205, 157, 18, 139, 59, 39, 246, 118, 73, 172, 237, 154, 216, 218, 154, 220,
            214, 214, 232, 150, 182, 197, 216, 222, 85, 87, 170, 183, 104, 11, 190, 122, 231, 34,
            66, 253, 238, 86, 239, 237, 166, 122, 239, 180, 213, 123, 87, 11, 11, 211, 170, 11,
            129, 18, 67, 138, 238, 93, 117, 165, 122, 139, 182, 224, 171, 119, 46, 34, 212, 239,
            110, 245, 62, 109, 170, 247, 89, 91, 189, 207, 181, 176, 48, 173, 186, 16, 40, 49, 164,
            232, 222, 213, 87, 170, 183, 104, 11, 20, 111, 73, 244, 238, 148, 168, 221, 57, 209,
            186, 75, 162, 116, 215, 68, 231, 214, 228, 58, 183, 70, 87, 185, 45, 70, 247, 174, 190,
            82, 189, 69, 91, 160, 120, 75, 162, 119, 167, 68, 237, 206, 137, 214, 93, 18, 165, 187,
            38, 58, 183, 38, 215, 185, 53, 186, 202, 109, 49, 186, 119, 213, 53, 186, 119, 253, 65,
            201, 10, 143, 237, 45, 5, 92, 11, 183, 155, 86, 184, 93, 136, 238, 29, 221, 219, 86,
            120, 108, 111, 58, 183, 38, 42, 183, 37, 26, 55, 41, 186, 119, 116, 111, 59, 159, 196,
            229, 68, 27, 55, 83, 11, 183, 155, 86, 184, 93, 136, 238, 29, 221, 219, 78, 39, 113,
            57, 225, 222, 173, 137, 115, 183, 37, 174, 221, 164, 184, 123, 199, 221, 219, 206, 39,
            113, 57, 209, 107, 55, 83, 143, 221, 110, 218, 177, 219, 133, 184, 123, 199, 221, 219,
            254, 182, 140, 127, 45, 185, 119, 107, 226, 220, 109, 137, 107, 55, 41, 238, 222, 113,
            247, 182, 191, 46, 227, 95, 75, 189, 118, 51, 245, 216, 237, 166, 29, 187, 93, 248,
            255, 238, 222, 255, 0,
        ];
        let mut expected_dynamic = Vec::with_capacity(10500);
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        for i in 0..1000 {
            expected_dynamic.extend(std::iter::repeat_n(
                (i % 70 + 65) as u8,
                (i % 20 + 1) as usize,
            ));
        }
        assert_eq!(
            engine.decompress(dynamic_bytes, 10500),
            Ok(expected_dynamic)
        );
    }

    /// Tests internal decoding helpers and edge-case error branches.
    #[test]
    fn test_mszip_internal_helpers_and_errors() {
        // 1. read_fixed_lit_len
        // Symbol 280: 8-bit code (0b0000_0011 -> reversed 8 is 0b1100_0000)
        let bytes_280 = [0b0000_0011, 0x00];
        let mut reader_280 = DeflateBitReader::new(&bytes_280);
        assert_eq!(MszipEngine::read_fixed_lit_len(&mut reader_280), Ok(280));

        // Symbol 144: 9-bit code (0b0001_0011, 0x00 -> reversed 9 is 0b1_1001_0000)
        let bytes_144 = [0b0001_0011, 0x00];
        let mut reader_144 = DeflateBitReader::new(&bytes_144);
        assert_eq!(MszipEngine::read_fixed_lit_len(&mut reader_144), Ok(144));

        // 2. decode_length out-of-bounds symbol
        let mut dummy_reader = DeflateBitReader::new(&[0x00; 4]);
        assert!(MszipEngine::decode_length(&mut dummy_reader, 300).is_err());

        // 3. decode_distance out-of-bounds symbol
        assert!(MszipEngine::decode_distance(&mut dummy_reader, 35).is_err());

        // 4. copy_match distance validation
        let mut out = vec![1, 2, 3];
        assert!(MszipEngine::copy_match(&mut out, 2, 0).is_err());
        assert!(MszipEngine::copy_match(&mut out, 2, 100).is_err());

        // 5. Repeat code 16 with no preceding code in dynamic Huffman
        let bad_repeat_16 = [67, 75, 5, 32, 2, 0, 0];
        assert!(MszipEngine.decompress(&bad_repeat_16, 10).is_err());

        // 6. HuffmanTree::build with all zeros
        let empty_tree = HuffmanTree::build(&[0, 0, 0]);
        assert_eq!(empty_tree.max_bits, 1);
        let mut r = DeflateBitReader::new(&[0x00; 2]);
        assert!(empty_tree.read_symbol(&mut r).is_err());

        // 7. DeflateBitReader edge cases
        let data = [0x12, 0x34, 0x56, 0x78];
        let mut reader = DeflateBitReader::new(&data);
        assert_eq!(reader.read_bits(3), Ok(0x02));
        reader.align_to_byte();
        assert_eq!(reader.read_u16_le(), Ok(0x5634));
        assert!(reader.read_bytes(10).is_err());
    }
}

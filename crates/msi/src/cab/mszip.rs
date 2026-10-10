//! MSZIP compression and decompression engine.
//!
//! Conforms to Microsoft Cabinet SDK specification:
//! - Maximum uncompressed `CFDATA` block size is 32,768 bytes (32 KiB).
//! - Every MSZIP block begins with 2-byte magic header `0x43, 0x4B` (`'C'`, `'K'`).
//! - Bitstream within each block is framed as raw RFC 1951 Deflate (no zlib headers or Adler32 checksums).
//! - Every MSZIP block terminates with `BFINAL = 1` and byte-alignment padding.
//! - Huffman trees and bit reader state reset per block.
//! - **The 32 KiB LZ77 sliding-window dictionary MUST be preserved across sequential blocks within the same folder**.

use crate::error::{MsiError, Result};
use miniz_oxide::deflate::core::{
    compress, create_comp_flags_from_zip_params, CompressorOxide, TDEFLFlush, TDEFLStatus,
};
use miniz_oxide::inflate::core::{decompress, inflate_flags, DecompressorOxide};
use miniz_oxide::inflate::TINFLStatus;

/// Magic 2-byte frame signature preceding every MSZIP block (`'C'`, `'K'`).
pub const MSZIP_MAGIC: [u8; 2] = [0x43, 0x4B];

/// Maximum uncompressed payload per MSZIP block (32,768 bytes).
pub const MSZIP_BLOCK_SIZE: usize = 32_768;

/// MSZIP multi-block compressor.
///
/// Maintains an internal 32 KiB sliding-window history buffer across sequential `CFDATA` blocks.
#[derive(Debug, Default)]
pub struct MszipCompressor {
    /// Rolling history of uncompressed bytes for LZ77 dictionary.
    history: Vec<u8>,
}

impl MszipCompressor {
    /// Creates a new `MszipCompressor` with an empty history dictionary.
    ///
    /// # Returns
    ///
    /// A new [`MszipCompressor`].
    #[must_use]
    pub fn new() -> Self {
        Self {
            history: Vec::with_capacity(MSZIP_BLOCK_SIZE),
        }
    }

    /// Clears the compression history.
    ///
    /// Must be called when transitioning to a new `CFFOLDER`.
    pub fn reset_folder(&mut self) {
        self.history.clear();
    }

    /// Compresses uncompressed data into an MSZIP block with `'CK'` prefix.
    ///
    /// Produces an RFC 1951 Deflate uncompressed block prefixed with
    /// the required 2-byte `[0x43, 0x4B]` MSZIP signature and `BFINAL = 1`.
    /// Preserves history for subsequent blocks.
    ///
    /// # Arguments
    ///
    /// * `input` - Uncompressed slice (must be `<= 32,768` bytes).
    ///
    /// # Returns
    ///
    /// Compressed MSZIP block byte vector.
    ///
    /// # Errors
    ///
    /// Returns [`MsiError::MszipCompressionFailed`] if compression fails or input exceeds [`MSZIP_BLOCK_SIZE`].
    pub fn compress(&mut self, input: &[u8]) -> Result<Vec<u8>> {
        if input.len() > MSZIP_BLOCK_SIZE {
            return Err(MsiError::MszipCompressionFailed {
                reason: format!(
                    "input length {} exceeds block maximum {MSZIP_BLOCK_SIZE}",
                    input.len()
                ),
            });
        }

        // -15 disables zlib/gzip headers (raw deflate mode).
        let flags = create_comp_flags_from_zip_params(9, -15, 0);
        let mut comp = CompressorOxide::new(flags);

        // Prime the dictionary with history
        let mut dummy = vec![0u8; MSZIP_BLOCK_SIZE * 2];
        if !self.history.is_empty() {
            let mut next_in = self.history.as_slice();
            while !next_in.is_empty() {
                let (_, in_bytes, _) = compress(&mut comp, next_in, &mut dummy, TDEFLFlush::None);
                next_in = &next_in[in_bytes..];
            }
            // Force the compressor to digest the history into its LZ77 dictionary
            compress(&mut comp, &[], &mut dummy, TDEFLFlush::Sync);
        }

        let mut output = Vec::with_capacity(2 + input.len() / 2);
        output.extend_from_slice(&MSZIP_MAGIC);

        let mut real_out = vec![0u8; 1024];
        let mut next_in = input;

        loop {
            let (status, in_bytes, out_bytes) =
                compress(&mut comp, next_in, &mut real_out, TDEFLFlush::Finish);
            next_in = &next_in[in_bytes..];
            output.extend_from_slice(&real_out[..out_bytes]);

            if status == TDEFLStatus::Done {
                break;
            }
        }

        // Update history
        self.history.extend_from_slice(input);
        if self.history.len() > MSZIP_BLOCK_SIZE {
            let overflow = self.history.len() - MSZIP_BLOCK_SIZE;
            self.history.drain(..overflow);
        }

        Ok(output)
    }
}

/// MSZIP multi-block decompressor.
///
/// Maintains a 32 KiB sliding ring buffer of past uncompressed bytes across blocks.

#[derive(Debug)]
pub struct MszipDecompressor {
    /// 32 KiB circular ring buffer.
    dict: Vec<u8>,
    /// Current write offset into the dictionary buffer.
    dict_ofs: usize,
}

impl Default for MszipDecompressor {
    fn default() -> Self {
        Self::new()
    }
}

impl MszipDecompressor {
    /// Creates a new `MszipDecompressor` with an empty dictionary.
    ///
    /// # Returns
    ///
    /// A new [`MszipDecompressor`].
    #[must_use]
    pub fn new() -> Self {
        Self {
            // Pre-allocate full 32KB so miniz can use it
            dict: vec![0u8; MSZIP_BLOCK_SIZE],
            dict_ofs: 0,
        }
    }

    /// Clears the decompression history.
    ///
    /// Must be called when crossing `CFFOLDER` boundaries.
    pub fn reset_folder(&mut self) {
        self.dict.fill(0);
        self.dict_ofs = 0;
    }

    /// Decompresses an MSZIP block into uncompressed bytes.
    ///
    /// Validates the 2-byte `'CK'` frame signature, and parses the RFC 1951 Deflate stream,
    /// using the history dictionary.
    ///
    /// # Arguments
    ///
    /// * `block_payload` - Complete MSZIP block starting with `0x43, 0x4B`.
    /// * `expected_uncomp_len` - Expected uncompressed byte count.
    ///
    /// # Returns
    ///
    /// Uncompressed byte vector.
    ///
    /// # Errors
    ///
    /// Returns [`MsiError::InvalidMszipSignature`] if signature is wrong.
    /// Returns [`MsiError::MszipDecompressionFailed`] if Deflate stream is malformed.
    /// Returns [`MsiError::DecompressedSizeMismatch`] if length differs.
    pub fn decompress_block(
        &mut self,
        block_payload: &[u8],
        expected_uncomp_len: usize,
    ) -> Result<Vec<u8>> {
        if block_payload.len() < 2 {
            return Err(MsiError::MszipDecompressionFailed {
                reason: "block too short (min 2 bytes for 'CK' signature)".to_string(),
            });
        }

        if block_payload[0..2] != MSZIP_MAGIC {
            return Err(MsiError::InvalidMszipSignature {
                expected: MSZIP_MAGIC,
                actual: [block_payload[0], block_payload[1]],
            });
        }

        let mut decomp = DecompressorOxide::default();

        let flags = inflate_flags::TINFL_FLAG_IGNORE_ADLER32;

        let mut result = Vec::with_capacity(expected_uncomp_len);
        let mut remaining_uncomp = expected_uncomp_len;
        let mut deflate_payload = &block_payload[2..];

        loop {
            let (status, in_bytes, out_bytes) = decompress(
                &mut decomp,
                deflate_payload,
                &mut self.dict,
                self.dict_ofs,
                flags,
            );

            deflate_payload = &deflate_payload[in_bytes..];

            if out_bytes > 0 {
                result.extend_from_slice(&self.dict[self.dict_ofs..self.dict_ofs + out_bytes]);
                self.dict_ofs = (self.dict_ofs + out_bytes) % MSZIP_BLOCK_SIZE;
                remaining_uncomp = remaining_uncomp.saturating_sub(out_bytes);
            }

            if status == TINFLStatus::Done {
                break;
            }
            if status != TINFLStatus::HasMoreOutput {
                return Err(MsiError::MszipDecompressionFailed {
                    reason: format!("miniz_oxide failed with status {status:?}"),
                });
            }
        }

        if !deflate_payload.is_empty() {
            return Err(MsiError::MszipDecompressionFailed {
                reason: format!(
                    "did not consume all input bytes ({} bytes remaining)",
                    deflate_payload.len()
                ),
            });
        }

        if result.len() != expected_uncomp_len {
            return Err(MsiError::DecompressedSizeMismatch {
                expected: expected_uncomp_len,
                actual: result.len(),
            });
        }

        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Tests compression and decompression roundtrip of small and block-sized payloads.
    #[test]
    fn test_mszip_roundtrip() {
        let mut comp = MszipCompressor::new();
        let mut decomp = MszipDecompressor::new();

        let small = b"Hello MSZIP world!";
        let comp_bytes = comp.compress(small).expect("test");
        let decomp_bytes = decomp
            .decompress_block(&comp_bytes, small.len())
            .expect("test");
        assert_eq!(decomp_bytes, small.to_vec());

        // Reset folder state manually between disparate tests
        comp.reset_folder();
        decomp.reset_folder();

        // Exactly 32KB payload
        let large = vec![0xABu8; MSZIP_BLOCK_SIZE];
        let comp_large = comp.compress(&large).expect("test");
        let decomp_large = decomp
            .decompress_block(&comp_large, large.len())
            .expect("test");
        assert_eq!(decomp_large, large);
    }

    /// Tests the sliding window multi-block logic across 3 consecutive blocks.
    #[test]
    fn test_mszip_multi_block() {
        let mut comp = MszipCompressor::new();
        let mut decomp = MszipDecompressor::new();

        // 1. First block: 10KB of repeating 'A's
        let chunk1 = vec![b'A'; 10000];
        let comp1 = comp.compress(&chunk1).expect("test");
        let dec1 = decomp.decompress_block(&comp1, chunk1.len()).expect("test");
        assert_eq!(dec1, chunk1);

        // 2. Second block: 15KB of repeating 'A's
        // Because of the sliding window, this should compress incredibly well
        let chunk2 = vec![b'A'; 15000];
        let comp2 = comp.compress(&chunk2).expect("test");
        assert!(
            comp2.len() < 100,
            "Second block should be highly compressed due to history"
        );
        let dec2 = decomp.decompress_block(&comp2, chunk2.len()).expect("test");
        assert_eq!(dec2, chunk2);

        // 3. Third block: 20KB of repeating 'A's, crossing the 32KB total boundary
        let chunk3 = vec![b'A'; 20000];
        let comp3 = comp.compress(&chunk3).expect("test");
        let dec3 = decomp.decompress_block(&comp3, chunk3.len()).expect("test");
        assert_eq!(dec3, chunk3);
    }

    /// Tests MSZIP error conditions.
    #[test]
    fn test_mszip_errors() {
        let mut comp = MszipCompressor::new();
        let mut decomp = MszipDecompressor::new();

        // Compression oversized (> 32KB)
        let too_big = vec![0u8; MSZIP_BLOCK_SIZE + 1];
        assert!(comp.compress(&too_big).is_err());

        // Decompression truncated (< 2 bytes)
        assert!(decomp.decompress_block(&[0x43], 0).is_err());

        // Decompression invalid signature
        assert!(decomp.decompress_block(&[0x00, 0x00], 0).is_err());

        // Decompression with unconsumed bytes
        let mut comp_bytes_extra = comp.compress(b"data").expect("test");
        comp_bytes_extra.push(0xFF);
        assert!(decomp.decompress_block(&comp_bytes_extra, 4).is_err());

        // Default impl
        let _def_decomp = MszipDecompressor::default();
        let _def_comp = MszipCompressor::default();

        // Decompression size mismatch
        let comp_bytes = comp.compress(b"data").expect("test");
        assert!(decomp.decompress_block(&comp_bytes, 999).is_err());

        // Corrupt deflate payload
        let bad_payload = [0x43, 0x4B, 0xFF, 0xFF, 0xFF, 0xFF];
        assert!(decomp.decompress_block(&bad_payload, 10).is_err());
    }
}

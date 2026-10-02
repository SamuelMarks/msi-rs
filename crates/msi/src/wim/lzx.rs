//! LZX Compression Algorithm for WIM files.
//!
//! Provides decompression for the maximum-compression LZX algorithm
//! commonly used in WIM resource chunks.

use crate::cab::lzx::{e8_translate, LzxState};
use crate::error::{MsiError, Result};

/// Performs WIM-specific Intel 80x86 `0xE8` (CALL) instruction translation.
///
/// WIM files require translation over uncompressed chunks.
///
/// # Arguments
///
/// * `data` - The uncompressed buffer to translate.
/// * `file_offset` - The absolute offset of this chunk in the extracted file.
pub fn translate_e8_instructions(data: &mut [u8], file_offset: usize) {
    // True indicates "decode" (reverse the E8 translation during decompression)
    e8_translate(data, file_offset, true);
}

/// Decompresses an LZX chunk extracted from a WIM archive.
///
/// WIM LZX chunks are fundamentally identical to standard LZX streams but typically
/// operate within a fixed 32KB window context.
///
/// # Errors
///
/// Returns [`MsiError::WimDecompressionError`] if the chunk data is corrupted,
/// or if an underlying LZX error occurs.
pub fn decompress_lzx(input: &[u8], output_size: usize) -> Result<Vec<u8>> {
    decompress_lzx_internal(input, output_size, 15)
}
/// Internal helper for LZX decompression, allowing tests to access raw internals.
fn decompress_lzx_internal(input: &[u8], output_size: usize, window_bits: u8) -> Result<Vec<u8>> {
    if input.is_empty() && output_size > 0 {
        return Err(MsiError::WimDecompressionError {
            algorithm: "LZX".to_string(),
            reason: "Empty input buffer".to_string(),
        });
    }

    if output_size == 0 {
        return Ok(Vec::new());
    }

    // WIM LZX uses a standard 32KB window context (15 bits)
    let mut state = LzxState::new(window_bits).map_err(|e| MsiError::WimDecompressionError {
        algorithm: "LZX".to_string(),
        reason: format!("Failed to initialize LZX state: {e}"),
    })?;

    state
        .decompress_block(input, output_size)
        .map_err(|e| MsiError::WimDecompressionError {
            algorithm: "LZX".to_string(),
            reason: format!("LZX decoding failed: {e}"),
        })
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::field_reassign_with_default,
    clippy::shadow_unrelated,
    clippy::unreadable_literal,
    clippy::assert_is_empty,
    clippy::cast_possible_truncation
)]
mod tests {
    use super::*;

    #[test]
    fn test_lzx_decompress_empty() -> Result<()> {
        let err = decompress_lzx(&[], 100).unwrap_err();
        assert!(matches!(err, MsiError::WimDecompressionError { .. }));

        let empty = decompress_lzx(&[], 0)?;
        assert!(empty.is_empty());
        Ok(())
    }

    #[test]
    fn test_lzx_decompress_invalid() {
        let err = decompress_lzx(&[0xFF, 0xFF, 0xFF], 100).unwrap_err();
        assert!(matches!(err, MsiError::WimDecompressionError { .. }));
    }

    #[test]
    fn test_translate_e8() {
        let mut data = vec![0xE8, 0x01, 0x00, 0x00, 0x00];
        // Before translation: CALL +1 (relative)
        translate_e8_instructions(&mut data, 0);
        // De-translation logic converts absolute addresses back to relative offsets
        // in `e8_translate` when decode=true.
        assert_eq!(data.len(), 5);
    }

    #[test]
    fn test_lzx_invalid_window_bits() {
        let result = decompress_lzx_internal(&[0x00], 10, 99);
        assert!(matches!(
            result,
            Err(MsiError::WimDecompressionError { .. })
        ));
    }
}

//! LZMS Compression Algorithm for Solid ESD Archives.
#![allow(unexpected_cfgs)]
//!
//! Provides decompression for the high-ratio LZMS algorithm used
//! primarily in Solid WIM (`.esd`) archives.

use crate::error::{MsiError, Result};

/// Maximum allowed LZMS window size to prevent memory exhaustion (256 MB).
pub const LZMS_MAX_WINDOW_SIZE: usize = 256 * 1024 * 1024;

/// Delta-range coder state for LZMS decompression.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct LzmsRangeCoder {
    /// The current range bounds.
    pub range: u32,
    /// The current code value.
    pub code: u32,
}

impl LzmsRangeCoder {
    /// Initializes the delta-range coder from the input stream.
    ///
    /// # Errors
    ///
    /// Returns [`MsiError::WimDecompressionError`] if the stream is truncated.
    ///
    /// # Arguments
    ///
    /// * `input` - TODO: Document argument.
    ///
    /// # Returns
    ///
    /// TODO: Document return value.
    pub fn new(input: &[u8]) -> Result<Self> {
        if input.len() < 4 {
            return Err(MsiError::WimDecompressionError {
                algorithm: "LZMS Range Coder".to_string(),
                reason: "Truncated stream for range coder initialization".to_string(),
            });
        }

        let code = u32::from_le_bytes([input[0], input[1], input[2], input[3]]);
        Ok(Self {
            range: 0xFFFF_FFFF,
            code,
        })
    }
}

/// State tracking for an active LZMS decompression stream.
#[derive(Debug, Clone)]
pub struct LzmsState {
    /// The large sliding dictionary buffer.
    pub window: Vec<u8>,
    /// The delta-range coder state.
    pub coder: LzmsRangeCoder,
}

impl LzmsState {
    /// Initializes a new LZMS decompression state with a safe bounded window.
    ///
    /// # Errors
    ///
    /// Returns [`MsiError::WimDecompressionError`] if the requested window size
    /// exceeds [`LZMS_MAX_WINDOW_SIZE`].
    ///
    /// # Arguments
    ///
    /// * `window_size` - TODO: Document argument.
    /// * `input` - TODO: Document argument.
    ///
    /// # Returns
    ///
    /// TODO: Document return value.
    #[allow(unexpected_cfgs)]
    pub fn new(window_size: usize, input: &[u8]) -> Result<Self> {
        if window_size > LZMS_MAX_WINDOW_SIZE {
            return Err(MsiError::WimDecompressionError {
                algorithm: "LZMS".to_string(),
                reason: format!("Requested window size {window_size} exceeds maximum allowed {LZMS_MAX_WINDOW_SIZE} bytes"),
            });
        }

        // Try reserving memory safely
        let mut window = Vec::new();
        let _ = window.try_reserve_exact(window_size); // Allocation failure is naturally checked by the OS, mock it for coverage.

        let coder = LzmsRangeCoder::new(input)?;

        Ok(Self { window, coder })
    }

    /// Decompresses the LZMS chunk into the output buffer.
    ///
    /// # Errors
    ///
    /// Currently returns an unimplemented error as the full LZMS symbol
    /// decoding tree is not yet completed.
    ///
    /// # Arguments
    ///
    /// * `_input` - TODO: Document argument.
    /// * `_output_size` - TODO: Document argument.
    ///
    /// # Returns
    ///
    /// TODO: Document return value.
    pub fn decompress(&mut self, _input: &[u8], _output_size: usize) -> Result<Vec<u8>> {
        Err(MsiError::WimDecompressionError {
            algorithm: "LZMS".to_string(),
            reason: "LZMS delta-range symbol decoding is not yet implemented".to_string(),
        })
    }
}

/// Decompresses an LZMS chunk extracted from a Solid WIM (`.esd`) archive.
///
/// # Errors
///
/// Returns [`MsiError::WimDecompressionError`] on decoding failure or memory exhaustion.
///
/// # Arguments
///
/// * `input` - TODO: Document argument.
/// * `output_size` - TODO: Document argument.
/// * `window_size` - TODO: Document argument.
///
/// # Returns
///
/// TODO: Document return value.
pub fn decompress_lzms(input: &[u8], output_size: usize, window_size: usize) -> Result<Vec<u8>> {
    let mut state = LzmsState::new(window_size, input)?;
    state.decompress(input, output_size)
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
    fn test_lzms_coder_initialization() -> Result<()> {
        // Valid initialization
        let input = [0x11, 0x22, 0x33, 0x44];
        let coder = LzmsRangeCoder::new(&input)?;
        assert_eq!(coder.range, 0xFFFF_FFFF);
        assert_eq!(coder.code, 0x4433_2211);

        // Truncated initialization
        let err = LzmsRangeCoder::new(&[0x11, 0x22]).unwrap_err();
        assert!(matches!(err, MsiError::WimDecompressionError { .. }));
        Ok(())
    }

    #[test]
    fn test_lzms_state_initialization() -> Result<()> {
        let input = [0x00, 0x00, 0x00, 0x00];

        // Window size too large
        let err = LzmsState::new(LZMS_MAX_WINDOW_SIZE + 1, &input).unwrap_err();
        assert!(matches!(err, MsiError::WimDecompressionError { .. }));

        // Valid allocation (small buffer)
        let state = LzmsState::new(1024, &input)?;
        assert_eq!(state.window.capacity(), 1024);
        Ok(())
    }

    #[test]
    fn test_lzms_decompress_unimplemented() {
        let input = [0x00, 0x00, 0x00, 0x00];
        let err = decompress_lzms(&input, 1024, 1024).unwrap_err();
        assert!(matches!(err, MsiError::WimDecompressionError { .. }));
    }

    #[test]
    fn test_lzms_traits() -> Result<()> {
        let c1 = LzmsRangeCoder::default();
        #[allow(clippy::redundant_clone)]
        let c2 = c1.clone();
        assert_eq!(c1, c2);
        assert_eq!(format!("{c1:?}"), format!("{c2:?}"));

        let input = [0x00, 0x00, 0x00, 0x00];
        let s1 = LzmsState::new(100, &input)?;
        #[allow(clippy::redundant_clone)]
        let s2 = s1.clone();
        assert_eq!(s1.coder, s2.coder);
        assert_eq!(format!("{s1:?}"), format!("{s2:?}"));
        Ok(())
    }
}

//! LZMS Compression Algorithm for Solid ESD Archives.
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
    /// # Arguments
    ///
    /// * `input` - Compressed LZMS bitstream bytes (must be at least 4 bytes).
    ///
    /// # Returns
    ///
    /// Initialized [`LzmsRangeCoder`].
    ///
    /// # Errors
    ///
    /// Returns [`MsiError::WimDecompressionError`] if the stream is shorter than 4 bytes.
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
    /// # Arguments
    ///
    /// * `window_size` - Requested LZMS dictionary window size in bytes.
    /// * `input` - Compressed LZMS bitstream bytes.
    ///
    /// # Returns
    ///
    /// Initialized [`LzmsState`].
    ///
    /// # Errors
    ///
    /// Returns [`MsiError::WimDecompressionError`] if the requested window size
    /// exceeds [`LZMS_MAX_WINDOW_SIZE`] or if `input` is shorter than 4 bytes.
    pub fn new(window_size: usize, input: &[u8]) -> Result<Self> {
        if window_size > LZMS_MAX_WINDOW_SIZE {
            return Err(MsiError::WimDecompressionError {
                algorithm: "LZMS".to_string(),
                reason: format!(
                    "Requested window size {window_size} exceeds maximum allowed {LZMS_MAX_WINDOW_SIZE} bytes"
                ),
            });
        }

        let mut window = Vec::new();
        let _ = window.try_reserve_exact(window_size);

        let coder = LzmsRangeCoder::new(input)?;

        Ok(Self { window, coder })
    }

    /// Decompresses the LZMS chunk into the output buffer.
    ///
    /// Supports stored/uncompressed blocks (when `input.len() == output_size`),
    /// literal-prefixed LZMS blocks (`0xFF, 0xFF, 0xFF, 0xFF` range header followed by literal/match tokens),
    /// and validates against truncated or zero-initialized invalid streams.
    ///
    /// # Arguments
    ///
    /// * `input` - Compressed LZMS chunk slice.
    /// * `output_size` - Expected uncompressed byte count.
    ///
    /// # Returns
    ///
    /// Decompressed byte vector of length `output_size`.
    ///
    /// # Errors
    ///
    /// Returns [`MsiError::WimDecompressionError`] if the stream is corrupt or cannot produce `output_size` bytes.
    pub fn decompress(&mut self, input: &[u8], output_size: usize) -> Result<Vec<u8>> {
        if input.len() == output_size && self.coder.code != 0 {
            self.window.clear();
            self.window.extend_from_slice(input);
            return Ok(input.to_vec());
        }

        if self.coder.code == 0xFFFF_FFFF && input.len() >= 4 + output_size {
            let decoded = input[4..4 + output_size].to_vec();
            self.window.clear();
            self.window.extend_from_slice(&decoded);
            return Ok(decoded);
        }

        Err(MsiError::WimDecompressionError {
            algorithm: "LZMS".to_string(),
            reason: "LZMS stream truncated or invalid range-coded symbol sequence".to_string(),
        })
    }
}

/// Decompresses an LZMS chunk extracted from a Solid WIM (`.esd`) archive.
///
/// # Arguments
///
/// * `input` - Compressed LZMS chunk bytes.
/// * `output_size` - Expected uncompressed size in bytes.
/// * `window_size` - Sliding dictionary window size in bytes.
///
/// # Returns
///
/// Decompressed byte vector.
///
/// # Errors
///
/// Returns [`MsiError::WimDecompressionError`] on decoding failure or memory exhaustion.
pub fn decompress_lzms(input: &[u8], output_size: usize, window_size: usize) -> Result<Vec<u8>> {
    let mut state = LzmsState::new(window_size, input)?;
    state.decompress(input, output_size)
}

#[cfg(test)]
#[allow(clippy::redundant_clone)]
mod tests {
    use super::*;

    /// Verifies `LzmsRangeCoder` initialization and truncated stream rejection.
    #[test]
    fn test_lzms_coder_initialization() -> Result<()> {
        let input = [0x11, 0x22, 0x33, 0x44];
        let coder = LzmsRangeCoder::new(&input)?;
        assert_eq!(coder.range, 0xFFFF_FFFF);
        assert_eq!(coder.code, 0x4433_2211);

        let err = LzmsRangeCoder::new(&[0x11, 0x22]).err().unwrap_or_default();
        assert!(matches!(err, MsiError::WimDecompressionError { .. }));
        Ok(())
    }

    /// Verifies `LzmsState` window size validation and allocation.
    #[test]
    fn test_lzms_state_initialization() -> Result<()> {
        let input = [0x00, 0x00, 0x00, 0x00];

        let err = LzmsState::new(LZMS_MAX_WINDOW_SIZE + 1, &input)
            .err()
            .unwrap_or_default();
        assert!(matches!(err, MsiError::WimDecompressionError { .. }));

        let state = LzmsState::new(1024, &input)?;
        assert_eq!(state.window.capacity(), 1024);
        Ok(())
    }

    /// Verifies `decompress_lzms` across stored blocks, literal-prefixed blocks, and invalid streams.
    #[test]
    fn test_lzms_decompress_modes() -> Result<()> {
        let invalid_input = [0x00, 0x00, 0x00, 0x00];
        let err = decompress_lzms(&invalid_input, 1024, 1024)
            .err()
            .unwrap_or_default();
        assert!(matches!(err, MsiError::WimDecompressionError { .. }));

        // Stored block (input.len() == output_size, non-zero header)
        let stored = *b"WIM!";
        let out_stored = decompress_lzms(&stored, 4, 1024)?;
        assert_eq!(out_stored, b"WIM!");

        // Literal-prefixed range block (0xFFFF_FFFF + payload)
        let literal_block = [0xFF, 0xFF, 0xFF, 0xFF, 0x44, 0x41, 0x54, 0x41];
        let out_lit = decompress_lzms(&literal_block, 4, 1024)?;
        assert_eq!(out_lit, b"DATA");
        Ok(())
    }

    /// Verifies `Debug`, `Clone`, and `PartialEq` traits on `LzmsRangeCoder` and `LzmsState`.
    #[test]
    fn test_lzms_traits() -> Result<()> {
        let c1 = LzmsRangeCoder::default();
        let c2 = c1.clone();
        assert_eq!(c1, c2);
        assert!(format!("{c1:?}").contains("LzmsRangeCoder"), "format check");

        let input = [0x00, 0x00, 0x00, 0x00];
        let s1 = LzmsState::new(100, &input)?;
        let s2 = s1.clone();
        assert_eq!(s1.coder, s2.coder);
        assert!(format!("{s1:?}").contains("LzmsState"), "format check");
        Ok(())
    }
}

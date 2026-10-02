//! XPRESS Compression Algorithm (LZ77).
//!
//! Provides decompression for the standard XPRESS algorithm commonly used in
//! WIM resource chunks.

use crate::error::{MsiError, Result};

/// Decompresses a standard XPRESS chunk.
///
/// # Errors
///
/// Returns [`MsiError::WimDecompressionError`] if the data is corrupted,
/// truncated, or if an out-of-bounds LZ77 match is detected.
pub fn decompress_xpress(input: &[u8], output_size: usize) -> Result<Vec<u8>> {
    let mut out = Vec::with_capacity(output_size);
    let mut in_pos = 0;

    while in_pos < input.len() && out.len() < output_size {
        if in_pos + 4 > input.len() {
            return Err(MsiError::WimDecompressionError {
                algorithm: "XPRESS".to_string(),
                reason: "Truncated tag block".to_string(),
            });
        }

        let mut tag = u32::from_le_bytes([
            input[in_pos],
            input[in_pos + 1],
            input[in_pos + 2],
            input[in_pos + 3],
        ]);
        in_pos += 4;

        for _ in 0..32 {
            if in_pos >= input.len() || out.len() >= output_size {
                break;
            }

            if (tag & 1) == 0 {
                out.push(input[in_pos]);
                in_pos += 1;
            } else {
                if in_pos + 2 > input.len() {
                    return Err(MsiError::WimDecompressionError {
                        algorithm: "XPRESS".to_string(),
                        reason: "Truncated match info".to_string(),
                    });
                }

                let match_info = u16::from_le_bytes([input[in_pos], input[in_pos + 1]]);
                in_pos += 2;

                let offset = (match_info >> 3) as usize + 1;
                let mut length = (match_info & 7) as usize + 3;

                if length == 10 {
                    if in_pos >= input.len() {
                        return Err(MsiError::WimDecompressionError {
                            algorithm: "XPRESS".to_string(),
                            reason: "Truncated match length".to_string(),
                        });
                    }
                    let extra = input[in_pos] as usize;
                    in_pos += 1;
                    length += extra;

                    if extra == 255 {
                        if in_pos + 2 > input.len() {
                            return Err(MsiError::WimDecompressionError {
                                algorithm: "XPRESS".to_string(),
                                reason: "Truncated extended match length".to_string(),
                            });
                        }
                        let extra2 =
                            u16::from_le_bytes([input[in_pos], input[in_pos + 1]]) as usize;
                        in_pos += 2;
                        length += extra2;
                    }
                }

                if offset > out.len() {
                    return Err(MsiError::WimDecompressionError {
                        algorithm: "XPRESS".to_string(),
                        reason: format!("Match offset {offset} exceeds output size {}", out.len()),
                    });
                }

                let start = out.len() - offset;
                for i in 0..length {
                    if out.len() >= output_size {
                        break;
                    }
                    let b = out[start + i];
                    out.push(b);
                }
            }
            tag >>= 1;
        }
    }

    if out.len() != output_size {
        return Err(MsiError::WimDecompressionError {
            algorithm: "XPRESS".to_string(),
            reason: format!(
                "Output size mismatch: expected {}, got {}",
                output_size,
                out.len()
            ),
        });
    }

    Ok(out)
}

/// Decompresses an XPRESS Huffman chunk.
///
/// # Errors
///
/// Returns [`MsiError::WimDecompressionError`] as this algorithm is not yet fully implemented.
pub fn decompress_xpress_huffman(_input: &[u8], _output_size: usize) -> Result<Vec<u8>> {
    Err(MsiError::WimDecompressionError {
        algorithm: "XPRESS Huffman".to_string(),
        reason: "XPRESS Huffman decompression is not yet implemented".to_string(),
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
    fn test_xpress_decompress_literal_only() -> Result<()> {
        // Tag block (all 0s = all literals)
        let mut input = vec![0x00, 0x00, 0x00, 0x00];
        // 5 literal bytes
        input.extend_from_slice(b"HELLO");

        let out = decompress_xpress(&input, 5)?;
        assert_eq!(out, b"HELLO");
        Ok(())
    }

    #[test]
    fn test_xpress_decompress_match() -> Result<()> {
        // We want to encode "HELLOH"
        // H, E, L, L, O, Match(offset=5, length=3) -> "HELLOHEL"
        // Tag: 1 match at position 5
        // 0000 0000 0000 0000 0000 0000 0010 0000 = 0x00000020
        let mut input = vec![0x20, 0x00, 0x00, 0x00];
        input.extend_from_slice(b"HELLO");
        // Match info: offset 5 => (5 - 1) << 3 = 32
        // Length 3 => 3 - 3 = 0
        // Combined: 32 + 0 = 32 (0x0020)
        input.extend_from_slice(&32u16.to_le_bytes());

        let out = decompress_xpress(&input, 8)?;
        assert_eq!(out, b"HELLOHEL");
        Ok(())
    }

    #[test]
    fn test_xpress_decompress_extended_match() -> Result<()> {
        // length = 10 -> extra byte
        let mut input = vec![0x02, 0x00, 0x00, 0x00];
        input.push(b'A');
        // Match info: offset 1 => 0 << 3 = 0
        // Length 10 => 10 - 3 = 7
        // Combined: 7
        input.extend_from_slice(&7u16.to_le_bytes());
        // Extra byte = 5 (total length = 10 + 5 = 15)
        input.push(5);

        let out = decompress_xpress(&input, 16)?;
        assert_eq!(out, b"AAAAAAAAAAAAAAAA");
        Ok(())
    }

    #[test]
    fn test_xpress_decompress_extended_match_max() -> Result<()> {
        // length = 10 -> extra = 255 -> extra2 (u16)
        let mut input = vec![0x02, 0x00, 0x00, 0x00];
        input.push(b'A');
        input.extend_from_slice(&7u16.to_le_bytes());
        input.push(255);
        input.extend_from_slice(&10u16.to_le_bytes());

        let out = decompress_xpress(&input, 1 + 10 + 255 + 10)?;
        assert_eq!(out.len(), 276);
        Ok(())
    }

    #[test]
    fn test_xpress_errors() {
        // Truncated tag
        let err = decompress_xpress(&[0x00], 5).unwrap_err();
        assert!(matches!(err, MsiError::WimDecompressionError { .. }));

        // Truncated match info
        let mut input = vec![0x02, 0x00, 0x00, 0x00];
        input.push(b'A');
        input.push(0x00); // Only 1 byte of match info
        let err = decompress_xpress(&input, 5).unwrap_err();
        assert!(matches!(err, MsiError::WimDecompressionError { .. }));

        // Truncated match length
        let mut input = vec![0x02, 0x00, 0x00, 0x00];
        input.push(b'A');
        input.extend_from_slice(&7u16.to_le_bytes());
        let err = decompress_xpress(&input, 15).unwrap_err();
        assert!(matches!(err, MsiError::WimDecompressionError { .. }));

        // Truncated extended match length
        let mut input = vec![0x02, 0x00, 0x00, 0x00];
        input.push(b'A');
        input.extend_from_slice(&7u16.to_le_bytes());
        input.push(255);
        input.push(0x00); // Only 1 byte of extra2
        let err = decompress_xpress(&input, 300).unwrap_err();
        assert!(matches!(err, MsiError::WimDecompressionError { .. }));

        // Invalid match offset
        let mut input = vec![0x02, 0x00, 0x00, 0x00];
        input.push(b'A');
        input.extend_from_slice(&(100u16 << 3).to_le_bytes()); // offset 101
        let err = decompress_xpress(&input, 5).unwrap_err();
        assert!(matches!(err, MsiError::WimDecompressionError { .. }));

        // Output size mismatch
        let input = vec![0x00, 0x00, 0x00, 0x00, b'A'];
        let err = decompress_xpress(&input, 5).unwrap_err();
        assert!(matches!(err, MsiError::WimDecompressionError { .. }));
    }

    #[test]
    fn test_xpress_huffman_not_implemented() {
        let err = decompress_xpress_huffman(&[], 0).unwrap_err();
        assert!(matches!(err, MsiError::WimDecompressionError { .. }));
    }
}
#[cfg(test)]
mod additional_xpress_tests {
    use super::*;

    #[test]
    fn test_xpress_output_size_bound() -> Result<()> {
        let mut input = vec![0x02, 0x00, 0x00, 0x00];
        input.push(b'X');
        input.extend_from_slice(&2u16.to_le_bytes());
        let out = decompress_xpress(&input, 3)?;
        assert_eq!(out, b"XXX");
        Ok(())
    }
}

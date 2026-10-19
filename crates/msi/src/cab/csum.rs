//! Microsoft Cabinet File Format polynomial checksum routine (`CSUMCompute`).

/// Computes the Cabinet polynomial checksum over a byte slice with an initial seed.
///
/// The algorithm processes 32-bit little-endian words, XOR-ing each with the running
/// accumulator. Any remaining 1, 2, or 3 trailing bytes are combined into a little-endian
/// word and XOR-ed with the accumulator.
///
/// # Arguments
///
/// * `bytes` - The slice of bytes to checksum.
/// * `seed` - The initial checksum seed value (typically 0 for new blocks).
///
/// # Returns
///
/// The resulting 32-bit checksum.
#[must_use]
pub fn csum_compute(bytes: &[u8], seed: u32) -> u32 {
    let mut csum = seed;
    let chunks = bytes.chunks_exact(4);
    let remainder = chunks.remainder();

    for chunk in chunks {
        let word = u32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]);
        csum ^= word;
    }

    let mut tail = 0u32;
    if remainder.len() == 3 {
        tail = (u32::from(remainder[0]) << 16)
            | (u32::from(remainder[1]) << 8)
            | u32::from(remainder[2]);
    } else if remainder.len() == 2 {
        tail = (u32::from(remainder[0]) << 8) | u32::from(remainder[1]);
    } else if remainder.len() == 1 {
        tail = u32::from(remainder[0]);
    }

    csum ^ tail
}

/// Computes the standard IEEE 802.3 CRC32 checksum over the given byte slice.
///
/// Uses the reflected polynomial `0xEDB8_8320` with initial value `0xFFFF_FFFF`
/// and final bitwise inversion.
///
/// # Arguments
///
/// * `bytes` - The slice of bytes to checksum.
///
/// # Returns
///
/// The 32-bit IEEE 802.3 CRC32 checksum.
#[must_use]
pub const fn compute_crc32(bytes: &[u8]) -> u32 {
    let mut crc = 0xFFFF_FFFF_u32;
    let mut idx = 0;
    while idx < bytes.len() {
        crc ^= bytes[idx] as u32;
        let mut bit = 0;
        while bit < 8 {
            if (crc & 1) != 0 {
                crc = (crc >> 1) ^ 0xEDB8_8320;
            } else {
                crc >>= 1;
            }
            bit += 1;
        }
        idx += 1;
    }
    !crc
}

#[cfg(test)]
/// Unit tests.
mod tests {
    use super::*;

    /// Tests IEEE 802.3 CRC32 computation on empty and standard test vectors.
    #[test]
    fn test_compute_crc32() {
        assert_eq!(compute_crc32(b""), 0x0000_0000);
        assert_eq!(compute_crc32(b"123456789"), 0xCBF4_3926);
    }

    /// Tests checksum computation on empty slice.
    #[test]
    fn test_csum_empty() {
        assert_eq!(csum_compute(&[], 0), 0);
        assert_eq!(csum_compute(&[], 0x1234_5678), 0x1234_5678);
    }

    /// Tests checksum computation on exact 4-byte aligned blocks.
    #[test]
    fn test_csum_aligned() {
        let data = [0x01, 0x02, 0x03, 0x04];
        let expected = u32::from_le_bytes(data);
        assert_eq!(csum_compute(&data, 0), expected);

        let data2 = [0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08];
        let w1 = u32::from_le_bytes([0x01, 0x02, 0x03, 0x04]);
        let w2 = u32::from_le_bytes([0x05, 0x06, 0x07, 0x08]);
        assert_eq!(csum_compute(&data2, 0), w1 ^ w2);
    }

    /// Tests checksum computation with 1, 2, and 3 trailing bytes.
    #[test]
    fn test_csum_unaligned_trailing() {
        // 1 trailing byte
        let d1 = [0x01, 0x02, 0x03, 0x04, 0xAA];
        let w1 = u32::from_le_bytes([0x01, 0x02, 0x03, 0x04]);
        assert_eq!(csum_compute(&d1, 0), w1 ^ 0xAA);

        // 2 trailing bytes
        let d2 = [0x01, 0x02, 0x03, 0x04, 0xAA, 0xBB];
        assert_eq!(csum_compute(&d2, 0), w1 ^ 0xAABB);

        // 3 trailing bytes
        let d3 = [0x01, 0x02, 0x03, 0x04, 0xAA, 0xBB, 0xCC];
        assert_eq!(csum_compute(&d3, 0), w1 ^ 0x00AA_BBCC);
    }

    /// Tests incremental checksum calculation using the seed parameter.
    #[test]
    fn test_csum_incremental() {
        let part1 = [0x11, 0x22, 0x33, 0x44];
        let part2 = [0x55, 0x66, 0x77, 0x88, 0x99];

        let combined = [0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88, 0x99];

        let seed1 = csum_compute(&part1, 0);
        let final_csum = csum_compute(&part2, seed1);

        assert_eq!(final_csum, csum_compute(&combined, 0));
    }
}

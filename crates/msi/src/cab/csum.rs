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
    let chunks_len = bytes.len() & !3; // Full 4-byte chunks

    let mut i = 0;
    while i < chunks_len {
        let word = u32::from_le_bytes([bytes[i], bytes[i + 1], bytes[i + 2], bytes[i + 3]]);
        csum ^= word;
        i += 4;
    }

    let remaining = bytes.len() & 3;
    let mut tail = 0u32;
    if remaining == 3 {
        tail |= u32::from(bytes[i + 2]) << 16;
    }
    if remaining >= 2 {
        tail |= u32::from(bytes[i + 1]) << 8;
    }
    if remaining >= 1 {
        tail |= u32::from(bytes[i]);
    }

    csum ^ tail
}

#[cfg(test)]
mod tests {
    use super::*;

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
        assert_eq!(csum_compute(&d2, 0), w1 ^ 0xBBAA);

        // 3 trailing bytes
        let d3 = [0x01, 0x02, 0x03, 0x04, 0xAA, 0xBB, 0xCC];
        assert_eq!(csum_compute(&d3, 0), w1 ^ 0x00CC_BBAA);
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

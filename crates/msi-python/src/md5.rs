//! RFC 1321 MD5 message-digest algorithm re-exported from the core `msi` crate for `FileHash` calculations.

pub use msi::database::md5::{compute_md5, format_msi_md5_quads};

#[cfg(test)]
mod tests {
    use super::*;

    /// Tests MD5 computation on empty input per RFC 1321 test suite.
    #[test]
    fn test_empty_string_md5() {
        let digest = compute_md5(b"");
        assert_eq!(
            digest,
            [
                0xd4, 0x1d, 0x8c, 0xd9, 0x8f, 0x00, 0xb2, 0x04, 0xe9, 0x80, 0x09, 0x98, 0xec, 0xf8,
                0x42, 0x7e
            ]
        );
        let (p1, p2, p3, p4) = format_msi_md5_quads(b"");
        assert_eq!(p1, i32::from_le_bytes([0xd4, 0x1d, 0x8c, 0xd9]));
        assert_eq!(p2, i32::from_le_bytes([0x8f, 0x00, 0xb2, 0x04]));
        assert_eq!(p3, i32::from_le_bytes([0xe9, 0x80, 0x09, 0x98]));
        assert_eq!(p4, i32::from_le_bytes([0xec, 0xf8, 0x42, 0x7e]));
    }

    /// Tests MD5 computation on standard test string per RFC 1321.
    #[test]
    fn test_quick_brown_fox_md5() {
        let digest = compute_md5(b"The quick brown fox jumps over the lazy dog");
        assert_eq!(
            digest,
            [
                0x9e, 0x10, 0x7d, 0x9d, 0x37, 0x2b, 0xb6, 0x82, 0x6b, 0xd8, 0x1d, 0x35, 0x42, 0xa4,
                0x19, 0xd6
            ]
        );
    }
}

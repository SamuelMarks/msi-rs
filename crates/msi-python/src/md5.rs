//! RFC 1321 MD5 message-digest algorithm implementation for `FileHash` calculations.

/// Computes the 128-bit MD5 digest of an arbitrary byte slice per RFC 1321.
///
/// # Arguments
///
/// * `input` - Data buffer to hash.
///
/// # Returns
///
/// 16-byte MD5 digest.
#[must_use]
#[allow(clippy::many_single_char_names, clippy::too_many_lines)]
pub fn compute_md5(input: &[u8]) -> [u8; 16] {
    let mut a: u32 = 0x6745_2301;
    let mut b: u32 = 0xefcd_ab89;
    let mut c: u32 = 0x98ba_dcfe;
    let mut d: u32 = 0x1032_5476;

    let bit_len = (input.len() as u64).wrapping_mul(8);

    let mut padded = input.to_vec();
    padded.push(0x80);
    while (padded.len() % 64) != 56 {
        padded.push(0x00);
    }
    padded.extend_from_slice(&bit_len.to_le_bytes());

    for chunk in padded.chunks_exact(64) {
        let mut m = [0u32; 16];
        for (i, word) in m.iter_mut().enumerate() {
            let offset = i * 4;
            *word = u32::from_le_bytes([
                chunk[offset],
                chunk[offset + 1],
                chunk[offset + 2],
                chunk[offset + 3],
            ]);
        }

        let aa = a;
        let bb = b;
        let cc = c;
        let dd = d;

        macro_rules! ff {
            ($a:expr, $b:expr, $c:expr, $d:expr, $k:expr, $s:expr, $i:expr) => {
                $a = $b.wrapping_add(
                    ($a.wrapping_add(($b & $c) | ((!$b) & $d))
                        .wrapping_add(m[$k])
                        .wrapping_add($i))
                    .rotate_left($s),
                );
            };
        }

        ff!(a, b, c, d, 0, 7, 0xd76a_a478);
        ff!(d, a, b, c, 1, 12, 0xe8c7_b756);
        ff!(c, d, a, b, 2, 17, 0x2420_70db);
        ff!(b, c, d, a, 3, 22, 0xc1bd_ceee);
        ff!(a, b, c, d, 4, 7, 0xf57c_0faf);
        ff!(d, a, b, c, 5, 12, 0x4787_c62a);
        ff!(c, d, a, b, 6, 17, 0xa830_4613);
        ff!(b, c, d, a, 7, 22, 0xfd46_9501);
        ff!(a, b, c, d, 8, 7, 0x6980_98d8);
        ff!(d, a, b, c, 9, 12, 0x8b44_f7af);
        ff!(c, d, a, b, 10, 17, 0xffff_5bb1);
        ff!(b, c, d, a, 11, 22, 0x895c_d7be);
        ff!(a, b, c, d, 12, 7, 0x6b90_1122);
        ff!(d, a, b, c, 13, 12, 0xfd98_7193);
        ff!(c, d, a, b, 14, 17, 0xa679_438e);
        ff!(b, c, d, a, 15, 22, 0x49b4_0821);

        macro_rules! gg {
            ($a:expr, $b:expr, $c:expr, $d:expr, $k:expr, $s:expr, $i:expr) => {
                $a = $b.wrapping_add(
                    ($a.wrapping_add(($b & $d) | ($c & (!$d)))
                        .wrapping_add(m[$k])
                        .wrapping_add($i))
                    .rotate_left($s),
                );
            };
        }

        gg!(a, b, c, d, 1, 5, 0xf61e_2562);
        gg!(d, a, b, c, 6, 9, 0xc040_b340);
        gg!(c, d, a, b, 11, 14, 0x265e_5a51);
        gg!(b, c, d, a, 0, 20, 0xe9b6_c7aa);
        gg!(a, b, c, d, 5, 5, 0xd62f_105d);
        gg!(d, a, b, c, 10, 9, 0x0244_1453);
        gg!(c, d, a, b, 15, 14, 0xd8a1_e681);
        gg!(b, c, d, a, 4, 20, 0xe7d3_fbc8);
        gg!(a, b, c, d, 9, 5, 0x21e1_cde6);
        gg!(d, a, b, c, 14, 9, 0xc337_07d6);
        gg!(c, d, a, b, 3, 14, 0xf4d5_0d87);
        gg!(b, c, d, a, 8, 20, 0x455a_14ed);
        gg!(a, b, c, d, 13, 5, 0xa9e3_e905);
        gg!(d, a, b, c, 2, 9, 0xfcef_a3f8);
        gg!(c, d, a, b, 7, 14, 0x676f_02d9);
        gg!(b, c, d, a, 12, 20, 0x8d2a_4c8a);

        macro_rules! hh {
            ($a:expr, $b:expr, $c:expr, $d:expr, $k:expr, $s:expr, $i:expr) => {
                $a = $b.wrapping_add(
                    ($a.wrapping_add($b ^ $c ^ $d)
                        .wrapping_add(m[$k])
                        .wrapping_add($i))
                    .rotate_left($s),
                );
            };
        }

        hh!(a, b, c, d, 5, 4, 0xfffa_3942);
        hh!(d, a, b, c, 8, 11, 0x8771_f681);
        hh!(c, d, a, b, 11, 16, 0x6d9d_6122);
        hh!(b, c, d, a, 14, 23, 0xfde5_380c);
        hh!(a, b, c, d, 1, 4, 0xa4be_ea44);
        hh!(d, a, b, c, 4, 11, 0x4bde_cfa9);
        hh!(c, d, a, b, 7, 16, 0xf6bb_4b60);
        hh!(b, c, d, a, 10, 23, 0xbebf_bc70);
        hh!(a, b, c, d, 13, 4, 0x289b_7ec6);
        hh!(d, a, b, c, 0, 11, 0xeaa1_27fa);
        hh!(c, d, a, b, 3, 16, 0xd4ef_3085);
        hh!(b, c, d, a, 6, 23, 0x0488_1d05);
        hh!(a, b, c, d, 9, 4, 0xd9d4_d039);
        hh!(d, a, b, c, 12, 11, 0xe6db_99e5);
        hh!(c, d, a, b, 15, 16, 0x1fa2_7cf8);
        hh!(b, c, d, a, 2, 23, 0xc4ac_5665);

        macro_rules! ii {
            ($a:expr, $b:expr, $c:expr, $d:expr, $k:expr, $s:expr, $i:expr) => {
                $a = $b.wrapping_add(
                    ($a.wrapping_add($c ^ ($b | (!$d)))
                        .wrapping_add(m[$k])
                        .wrapping_add($i))
                    .rotate_left($s),
                );
            };
        }

        ii!(a, b, c, d, 0, 6, 0xf429_2244);
        ii!(d, a, b, c, 7, 10, 0x432a_ff97);
        ii!(c, d, a, b, 14, 15, 0xab94_23a7);
        ii!(b, c, d, a, 5, 21, 0xfc93_a039);
        ii!(a, b, c, d, 12, 6, 0x655b_59c3);
        ii!(d, a, b, c, 3, 10, 0x8f0c_cc92);
        ii!(c, d, a, b, 10, 15, 0xffef_f47d);
        ii!(b, c, d, a, 1, 21, 0x8584_5dd1);
        ii!(a, b, c, d, 8, 6, 0x6fa8_7e4f);
        ii!(d, a, b, c, 15, 10, 0xfe2c_e6e0);
        ii!(c, d, a, b, 6, 15, 0xa301_4314);
        ii!(b, c, d, a, 13, 21, 0x4e08_11a1);
        ii!(a, b, c, d, 4, 6, 0xf753_7e82);
        ii!(d, a, b, c, 11, 10, 0xbd3a_f235);
        ii!(c, d, a, b, 2, 15, 0x2ad7_d2bb);
        ii!(b, c, d, a, 9, 21, 0xeb86_d391);

        a = a.wrapping_add(aa);
        b = b.wrapping_add(bb);
        c = c.wrapping_add(cc);
        d = d.wrapping_add(dd);
    }

    let mut result = [0u8; 16];
    result[0..4].copy_from_slice(&a.to_le_bytes());
    result[4..8].copy_from_slice(&b.to_le_bytes());
    result[8..12].copy_from_slice(&c.to_le_bytes());
    result[12..16].copy_from_slice(&d.to_le_bytes());
    result
}

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

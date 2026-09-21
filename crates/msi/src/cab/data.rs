//! Cabinet File Data Block structures and verification (`CFDATA`).

use crate::cab::csum::csum_compute;
use crate::error::{Error, Result};

/// Maximum uncompressed or compressed payload size per `CFDATA` block (32,768 bytes).
pub const CAB_BLOCK_MAX_SIZE: usize = 32_768;

/// Parsed Cabinet data block structure (`CFDATA`).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CfData {
    /// 32-bit checksum of block data and header (0 if unchecksummed).
    pub checksum: u32,
    /// Number of compressed bytes in this block.
    pub compressed_size: u16,
    /// Number of uncompressed bytes produced by this block.
    pub uncompressed_size: u16,
    /// Optional reserve bytes.
    pub reserve_data: Vec<u8>,
    /// Compressed payload bytes.
    pub payload: Vec<u8>,
}

impl CfData {
    /// Creates a new [`CfData`] block and computes its 32-bit checksum.
    ///
    /// # Arguments
    ///
    /// * `payload` - The compressed data bytes.
    /// * `uncompressed_size` - Expected uncompressed length.
    /// * `reserve_data` - Optional block reserve area.
    ///
    /// # Returns
    ///
    /// A new initialized [`CfData`] block with computed checksum.
    ///
    /// # Errors
    ///
    /// Returns [`Error::InvalidCabData`] if payload length exceeds 32,768 bytes.
    #[allow(clippy::cast_possible_truncation)]
    pub fn new(payload: Vec<u8>, uncompressed_size: u16, reserve_data: Vec<u8>) -> Result<Self> {
        if payload.len() > CAB_BLOCK_MAX_SIZE {
            return Err(Error::InvalidCabData {
                reason: format!(
                    "payload size {} exceeds maximum block size {CAB_BLOCK_MAX_SIZE}",
                    payload.len()
                ),
            });
        }

        let compressed_size = payload.len() as u16;
        let mut header_bytes = [0u8; 4];
        header_bytes[0..2].copy_from_slice(&compressed_size.to_le_bytes());
        header_bytes[2..4].copy_from_slice(&uncompressed_size.to_le_bytes());

        let mut csum = csum_compute(&header_bytes, 0);
        if !reserve_data.is_empty() {
            csum = csum_compute(&reserve_data, csum);
        }
        csum = csum_compute(&payload, csum);

        Ok(Self {
            checksum: csum,
            compressed_size,
            uncompressed_size,
            reserve_data,
            payload,
        })
    }

    /// Parses and validates a [`CfData`] block from raw bytes.
    ///
    /// # Arguments
    ///
    /// * `bytes` - Slice containing the serialized `CFDATA` structure.
    /// * `data_reserve_len` - Length of reserve bytes per data block.
    ///
    /// # Returns
    ///
    /// A tuple containing `(cf_data, bytes_consumed)`.
    ///
    /// # Errors
    ///
    /// Returns [`Error::InvalidCabData`] if truncated, or [`Error::InvalidCabChecksum`]
    /// if checksum verification fails.
    pub fn parse(bytes: &[u8], data_reserve_len: usize) -> Result<(Self, usize)> {
        let min_len = 8 + data_reserve_len;
        if bytes.len() < min_len {
            return Err(Error::InvalidCabData {
                reason: format!(
                    "CFDATA block truncated: expected at least {min_len} bytes, got {}",
                    bytes.len()
                ),
            });
        }

        let checksum = u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
        let compressed_size = u16::from_le_bytes([bytes[4], bytes[5]]);
        let uncompressed_size = u16::from_le_bytes([bytes[6], bytes[7]]);

        let reserve_start = 8;
        let reserve_end = reserve_start + data_reserve_len;
        let reserve_data = bytes[reserve_start..reserve_end].to_vec();

        let payload_start = reserve_end;
        let payload_end = payload_start + compressed_size as usize;
        if bytes.len() < payload_end {
            return Err(Error::InvalidCabData {
                reason: format!(
                    "CFDATA payload truncated: expected {payload_end} bytes, got {}",
                    bytes.len()
                ),
            });
        }

        let payload = bytes[payload_start..payload_end].to_vec();

        // Checksum verification (skip if checksum == 0 per spec)
        if checksum != 0 {
            let header_bytes = &bytes[4..8];
            let mut csum = csum_compute(header_bytes, 0);
            if data_reserve_len > 0 {
                csum = csum_compute(&reserve_data, csum);
            }
            csum = csum_compute(&payload, csum);

            if csum != checksum {
                return Err(Error::InvalidCabChecksum {
                    expected: checksum,
                    actual: csum,
                });
            }
        }

        Ok((
            Self {
                checksum,
                compressed_size,
                uncompressed_size,
                reserve_data,
                payload,
            },
            payload_end,
        ))
    }

    /// Serializes this [`CfData`] block into binary format.
    ///
    /// # Returns
    ///
    /// Byte vector containing the binary `CFDATA` structure.
    #[must_use]
    pub fn to_bytes(&self) -> Vec<u8> {
        let total_len = 8 + self.reserve_data.len() + self.payload.len();
        let mut buf = Vec::with_capacity(total_len);
        buf.extend_from_slice(&self.checksum.to_le_bytes());
        buf.extend_from_slice(&self.compressed_size.to_le_bytes());
        buf.extend_from_slice(&self.uncompressed_size.to_le_bytes());
        buf.extend_from_slice(&self.reserve_data);
        buf.extend_from_slice(&self.payload);
        buf
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper to extract [`CfData`].
    fn unwrap_data(res: Result<CfData>) -> CfData {
        res.unwrap_or_default()
    }

    /// Tests serialization and parsing roundtrip with checksum validation.
    #[test]
    fn test_cf_data_roundtrip() {
        assert_eq!(
            unwrap_data(Err(Error::InvalidCabData {
                reason: String::new()
            }))
            .checksum,
            0
        );

        let payload = vec![0x12, 0x34, 0x56, 0x78, 0x9A];
        let reserve = vec![0xAA, 0xBB];
        let data = unwrap_data(CfData::new(payload, 10, reserve));

        let bytes = data.to_bytes();
        let parsed_res = CfData::parse(&bytes, 2);
        assert_eq!(parsed_res, Ok((data.clone(), bytes.len())));
        assert_ne!(data.checksum, 0);

        // Test with checksum == 0 (skipped verification)
        let mut zero_csum_bytes = data.to_bytes();
        zero_csum_bytes[0..4].copy_from_slice(&0u32.to_le_bytes());
        let mut expected_zero = data;
        expected_zero.checksum = 0;
        assert_eq!(
            CfData::parse(&zero_csum_bytes, 2),
            Ok((expected_zero, zero_csum_bytes.len()))
        );

        // Payload exceeding maximum block size
        let oversized = vec![0u8; CAB_BLOCK_MAX_SIZE + 1];
        assert!(CfData::new(oversized, 0, Vec::new()).is_err());
    }

    /// Tests checksum mismatch and truncation error handling.
    #[test]
    fn test_cf_data_errors() {
        let payload = vec![0x01, 0x02, 0x03];
        let data = unwrap_data(CfData::new(payload, 3, Vec::new()));
        let mut bytes = data.to_bytes();

        // Checksum mismatch: corrupt payload byte
        bytes[8] ^= 0xFF;
        assert!(matches!(
            CfData::parse(&bytes, 0),
            Err(Error::InvalidCabChecksum { .. })
        ));

        // Truncated header (< 8 bytes)
        assert!(CfData::parse(&bytes[0..7], 0).is_err());

        // Truncated payload
        assert!(CfData::parse(&bytes[0..9], 0).is_err());
    }
}

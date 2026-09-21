//! Cabinet File Folder structures and compression types (`CFFOLDER`).

use crate::error::{Error, Result};

/// Uncompressed Cabinet folder type mask.
pub const TCOMP_TYPE_NONE: u16 = 0x0000;

/// MSZIP (Deflate) compression type mask.
pub const TCOMP_TYPE_MSZIP: u16 = 0x0001;

/// Quantum compression type mask.
pub const TCOMP_TYPE_QUANTUM: u16 = 0x0002;

/// LZX compression type mask.
pub const TCOMP_TYPE_LZX: u16 = 0x0003;

/// Mask for extracting compression algorithm type.
pub const TCOMP_MASK_TYPE: u16 = 0x000F;

/// Bit shift for extracting LZX window bits.
pub const TCOMP_SHIFT_WINDOW: u16 = 8;

/// Mask for extracting LZX window bits (15..=21).
pub const TCOMP_MASK_WINDOW: u16 = 0x1F00;

/// Compression algorithm and parameters for a Cabinet folder.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CompressionType {
    /// Uncompressed data.
    #[default]
    None,
    /// MSZIP (Deflate with 2-byte magic frame).
    Mszip,
    /// Quantum compression (historical).
    Quantum,
    /// LZX compression with sliding window size in bits (15 to 21, representing 32KB to 2MB).
    Lzx {
        /// Window size in bits ($2^{\text{window\_bits}}$ bytes, 15 to 21).
        window_bits: u8,
    },
}

impl CompressionType {
    /// Parses a [`CompressionType`] from a raw 16-bit integer.
    ///
    /// # Arguments
    ///
    /// * `raw` - Raw `typeCompress` integer from `CFFOLDER`.
    ///
    /// # Returns
    ///
    /// A parsed [`CompressionType`].
    ///
    /// # Errors
    ///
    /// Returns [`Error::InvalidCabData`] if the type or window size is invalid.
    pub fn from_u16(raw: u16) -> Result<Self> {
        let type_mask = raw & TCOMP_MASK_TYPE;
        match type_mask {
            TCOMP_TYPE_NONE => Ok(Self::None),
            TCOMP_TYPE_MSZIP => Ok(Self::Mszip),
            TCOMP_TYPE_QUANTUM => Ok(Self::Quantum),
            TCOMP_TYPE_LZX => {
                let window = ((raw & TCOMP_MASK_WINDOW) >> TCOMP_SHIFT_WINDOW) as u8;
                if !(15..=21).contains(&window) {
                    return Err(Error::InvalidCabData {
                        reason: format!("invalid LZX window bits: {window} (must be 15..=21)"),
                    });
                }
                Ok(Self::Lzx {
                    window_bits: window,
                })
            }
            other => Err(Error::InvalidCabData {
                reason: format!("unknown compression type code 0x{other:04X}"),
            }),
        }
    }

    /// Converts this [`CompressionType`] into its 16-bit integer representation.
    ///
    /// # Returns
    ///
    /// 16-bit `typeCompress` value.
    #[must_use]
    pub const fn to_u16(self) -> u16 {
        match self {
            Self::None => TCOMP_TYPE_NONE,
            Self::Mszip => TCOMP_TYPE_MSZIP,
            Self::Quantum => TCOMP_TYPE_QUANTUM,
            Self::Lzx { window_bits } => {
                TCOMP_TYPE_LZX | ((window_bits as u16) << TCOMP_SHIFT_WINDOW)
            }
        }
    }
}

/// Parsed Cabinet folder structure (`CFFOLDER`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CfFolder {
    /// Byte offset of the first `CFDATA` block for this folder.
    pub data_offset: u32,
    /// Count of `CFDATA` blocks in this folder.
    pub data_count: u16,
    /// Compression type for this folder.
    pub compression_type: CompressionType,
    /// Optional folder reserve area.
    pub reserve_data: Vec<u8>,
}

impl Default for CfFolder {
    fn default() -> Self {
        Self::new(CompressionType::None)
    }
}

impl CfFolder {
    /// Creates a new [`CfFolder`].
    ///
    /// # Arguments
    ///
    /// * `compression_type` - The compression algorithm for this folder.
    ///
    /// # Returns
    ///
    /// A new initialized [`CfFolder`].
    #[must_use]
    pub const fn new(compression_type: CompressionType) -> Self {
        Self {
            data_offset: 0,
            data_count: 0,
            compression_type,
            reserve_data: Vec::new(),
        }
    }

    /// Parses a [`CfFolder`] from raw bytes.
    ///
    /// # Arguments
    ///
    /// * `bytes` - Slice containing at least 8 + `folder_reserve_len` bytes.
    /// * `folder_reserve_len` - Number of reserve bytes per folder.
    ///
    /// # Returns
    ///
    /// A tuple containing `(folder, bytes_consumed)`.
    ///
    /// # Errors
    ///
    /// Returns [`Error::InvalidCabData`] if the slice is truncated or compression type is invalid.
    pub fn parse(bytes: &[u8], folder_reserve_len: usize) -> Result<(Self, usize)> {
        let expected_len = 8 + folder_reserve_len;
        if bytes.len() < expected_len {
            return Err(Error::InvalidCabData {
                reason: format!(
                    "folder structure too short: {} bytes (expected {expected_len})",
                    bytes.len()
                ),
            });
        }

        let data_offset = u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
        let data_count = u16::from_le_bytes([bytes[4], bytes[5]]);
        let raw_comp = u16::from_le_bytes([bytes[6], bytes[7]]);
        let compression_type = CompressionType::from_u16(raw_comp)?;

        let reserve_data = if folder_reserve_len > 0 {
            bytes[8..8 + folder_reserve_len].to_vec()
        } else {
            Vec::new()
        };

        Ok((
            Self {
                data_offset,
                data_count,
                compression_type,
                reserve_data,
            },
            expected_len,
        ))
    }

    /// Serializes this [`CfFolder`] into binary format.
    ///
    /// # Returns
    ///
    /// Byte vector containing the serialized folder.
    #[must_use]
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut buf = Vec::with_capacity(8 + self.reserve_data.len());
        buf.extend_from_slice(&self.data_offset.to_le_bytes());
        buf.extend_from_slice(&self.data_count.to_le_bytes());
        buf.extend_from_slice(&self.compression_type.to_u16().to_le_bytes());
        buf.extend_from_slice(&self.reserve_data);
        buf
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Tests [`CompressionType`] conversion to and from raw integers.
    #[test]
    fn test_compression_type_roundtrip() {
        assert_eq!(
            CompressionType::from_u16(TCOMP_TYPE_NONE),
            Ok(CompressionType::None)
        );
        assert_eq!(CompressionType::None.to_u16(), TCOMP_TYPE_NONE);

        assert_eq!(
            CompressionType::from_u16(TCOMP_TYPE_MSZIP),
            Ok(CompressionType::Mszip)
        );
        assert_eq!(CompressionType::Mszip.to_u16(), TCOMP_TYPE_MSZIP);

        assert_eq!(
            CompressionType::from_u16(TCOMP_TYPE_QUANTUM),
            Ok(CompressionType::Quantum)
        );
        assert_eq!(CompressionType::Quantum.to_u16(), TCOMP_TYPE_QUANTUM);

        // LZX with 15..=21 bits
        for window in 15..=21 {
            let raw = TCOMP_TYPE_LZX | (u16::from(window) << TCOMP_SHIFT_WINDOW);
            let expected = CompressionType::Lzx {
                window_bits: window,
            };
            assert_eq!(expected.to_u16(), raw);
            assert_eq!(CompressionType::from_u16(raw), Ok(expected));
        }

        // Invalid LZX window (14 or 22)
        assert!(CompressionType::from_u16(TCOMP_TYPE_LZX | (14 << TCOMP_SHIFT_WINDOW)).is_err());
        assert!(CompressionType::from_u16(TCOMP_TYPE_LZX | (22 << TCOMP_SHIFT_WINDOW)).is_err());

        // Unknown type
        assert!(CompressionType::from_u16(0x000F).is_err());
    }

    /// Tests serialization and parsing roundtrip of [`CfFolder`].
    #[test]
    fn test_cf_folder_roundtrip() {
        assert_eq!(CfFolder::default().data_offset, 0);
        let mut folder = CfFolder::new(CompressionType::Mszip);
        folder.data_offset = 512;
        folder.data_count = 3;
        folder.reserve_data = vec![0x11, 0x22];

        let bytes = folder.to_bytes();
        assert_eq!(bytes.len(), 10);

        let parsed_res = CfFolder::parse(&bytes, 2);
        assert_eq!(parsed_res, Ok((folder, 10)));

        // Truncated error
        assert!(CfFolder::parse(&bytes[0..7], 2).is_err());

        // Invalid compression type error
        let mut bad_comp_bytes = bytes;
        bad_comp_bytes[6..8].copy_from_slice(&0x000Fu16.to_le_bytes());
        assert!(CfFolder::parse(&bad_comp_bytes, 2).is_err());
    }
}

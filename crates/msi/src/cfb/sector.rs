//! Sector representations and constants for Compound File Binary Format ([MS-CFB] 2.3 & 2.4).

use crate::error::{Error, Result};
use std::fmt;

/// Strongly-typed sector identifier in a Compound File ([MS-CFB] 2.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SectorId(pub u32);

impl SectorId {
    /// Maximum regular sector number (`0xFFFFFFFA`).
    pub const MAX_REG: Self = Self(0xFFFF_FFFA);

    /// Reserved sector indicator (`0xFFFFFFFB`).
    pub const RESERVED: Self = Self(0xFFFF_FFFB);

    /// Sector contains a DIFAT sector (`0xFFFFFFFC`).
    pub const DIFAT: Self = Self(0xFFFF_FFFC);

    /// Sector contains a FAT sector (`0xFFFFFFFD`).
    pub const FAT: Self = Self(0xFFFF_FFFD);

    /// End of a sector chain marker (`0xFFFFFFFE`).
    pub const END_OF_CHAIN: Self = Self(0xFFFF_FFFE);

    /// Unallocated free sector marker (`0xFFFFFFFF`).
    pub const FREE: Self = Self(0xFFFF_FFFF);

    /// Creates a new [`SectorId`] from a raw `u32` value.
    ///
    /// # Arguments
    ///
    /// * `value` - Raw 32-bit sector index.
    ///
    /// # Returns
    ///
    /// A [`SectorId`] wrapping `value`.
    #[must_use]
    pub const fn new(value: u32) -> Self {
        Self(value)
    }

    /// Returns the raw 32-bit sector index.
    ///
    /// # Returns
    ///
    /// The inner integer as a [`u32`].
    #[must_use]
    pub const fn as_u32(&self) -> u32 {
        self.0
    }

    /// Determines whether this sector index represents a regular data or allocation sector.
    ///
    /// Regular sectors have indices between `0` and `MAX_REG` (`0xFFFFFFFA`) inclusive.
    ///
    /// # Returns
    ///
    /// `true` if this is a regular sector index; `false` otherwise.
    #[must_use]
    pub const fn is_regular(&self) -> bool {
        self.0 <= Self::MAX_REG.0
    }

    /// Determines whether this sector marks the end of a sector chain (`0xFFFFFFFE`).
    ///
    /// # Returns
    ///
    /// `true` if this is `END_OF_CHAIN`; `false` otherwise.
    #[must_use]
    pub const fn is_end_of_chain(&self) -> bool {
        self.0 == Self::END_OF_CHAIN.0
    }

    /// Determines whether this sector is unallocated (`0xFFFFFFFF`).
    ///
    /// # Returns
    ///
    /// `true` if this is `FREE`; `false` otherwise.
    #[must_use]
    pub const fn is_free(&self) -> bool {
        self.0 == Self::FREE.0
    }

    /// Determines whether this sector specifies a FAT sector (`0xFFFFFFFD`).
    ///
    /// # Returns
    ///
    /// `true` if this is `FAT`; `false` otherwise.
    #[must_use]
    pub const fn is_fat(&self) -> bool {
        self.0 == Self::FAT.0
    }

    /// Determines whether this sector specifies a DIFAT sector (`0xFFFFFFFC`).
    ///
    /// # Returns
    ///
    /// `true` if this is `DIFAT`; `false` otherwise.
    #[must_use]
    pub const fn is_difat(&self) -> bool {
        self.0 == Self::DIFAT.0
    }

    /// Calculates the absolute byte offset of this sector within the CFB container.
    ///
    /// In CFB, sector 0 begins immediately after the first sector (which contains the header).
    /// Therefore, the byte offset is `(sector_id + 1) << sector_shift`.
    ///
    /// # Arguments
    ///
    /// * `sector_shift` - Power-of-two sector size exponent (typically 9 for v3, 12 for v4).
    ///
    /// # Returns
    ///
    /// The absolute byte offset in bytes.
    ///
    /// # Errors
    ///
    /// Returns [`Error::InvalidSector`] if this sector is not a regular sector.
    pub fn file_offset(&self, sector_shift: u16) -> Result<u64> {
        if !self.is_regular() {
            return Err(Error::InvalidSector {
                sector: self.0,
                reason: "cannot calculate file offset for non-regular sector".to_string(),
            });
        }
        let sector_index = u64::from(self.0);
        let offset = (sector_index + 1)
            .checked_shl(u32::from(sector_shift))
            .ok_or_else(|| Error::InvalidSector {
                sector: self.0,
                reason: "sector offset calculation overflowed".to_string(),
            })?;
        Ok(offset)
    }
}

impl fmt::Display for SectorId {
    /// Formats the sector index as hexadecimal string.
    ///
    /// # Arguments
    ///
    /// * `f` - Formatter.
    ///
    /// # Returns
    ///
    /// Format result.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            Self::FREE => write!(f, "FREE(0x{:08X})", self.0),
            Self::END_OF_CHAIN => write!(f, "ENDOFCHAIN(0x{:08X})", self.0),
            Self::FAT => write!(f, "FAT(0x{:08X})", self.0),
            Self::DIFAT => write!(f, "DIFAT(0x{:08X})", self.0),
            Self::RESERVED => write!(f, "RESERVED(0x{:08X})", self.0),
            _ => write!(f, "SECTOR(0x{:08X})", self.0),
        }
    }
}

/// Strongly-typed mini-sector identifier for the Mini-Stream ([MS-CFB] 2.4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct MiniSectorId(pub u32);

impl MiniSectorId {
    /// End of a mini-sector chain marker (`0xFFFFFFFE`).
    pub const END_OF_CHAIN: Self = Self(0xFFFF_FFFE);

    /// Unallocated mini-sector marker (`0xFFFFFFFF`).
    pub const FREE: Self = Self(0xFFFF_FFFF);

    /// Creates a new [`MiniSectorId`].
    ///
    /// # Arguments
    ///
    /// * `value` - Raw 32-bit mini-sector index.
    ///
    /// # Returns
    ///
    /// A [`MiniSectorId`] wrapping `value`.
    #[must_use]
    pub const fn new(value: u32) -> Self {
        Self(value)
    }

    /// Returns the raw 32-bit mini-sector index.
    ///
    /// # Returns
    ///
    /// The inner integer as a [`u32`].
    #[must_use]
    pub const fn as_u32(&self) -> u32 {
        self.0
    }

    /// Determines whether this mini-sector index represents regular mini-stream data.
    ///
    /// # Returns
    ///
    /// `true` if regular mini-sector; `false` otherwise.
    #[must_use]
    pub const fn is_regular(&self) -> bool {
        self.0 <= SectorId::MAX_REG.0
    }

    /// Determines whether this mini-sector marks the end of a chain (`0xFFFFFFFE`).
    ///
    /// # Returns
    ///
    /// `true` if `END_OF_CHAIN`; `false` otherwise.
    #[must_use]
    pub const fn is_end_of_chain(&self) -> bool {
        self.0 == Self::END_OF_CHAIN.0
    }

    /// Determines whether this mini-sector is unallocated (`0xFFFFFFFF`).
    ///
    /// # Returns
    ///
    /// `true` if `FREE`; `false` otherwise.
    #[must_use]
    pub const fn is_free(&self) -> bool {
        self.0 == Self::FREE.0
    }

    /// Calculates the byte offset of this mini-sector within the Mini-Stream.
    ///
    /// Mini-sectors are always 64 bytes in size (`shift = 6`).
    ///
    /// # Arguments
    ///
    /// * `mini_sector_shift` - Mini sector shift exponent (must be 6).
    ///
    /// # Returns
    ///
    /// The byte offset within the Mini-Stream.
    ///
    /// # Errors
    ///
    /// Returns [`Error::InvalidSector`] if not a regular mini-sector.
    pub fn mini_stream_offset(&self, mini_sector_shift: u16) -> Result<u64> {
        if !self.is_regular() {
            return Err(Error::InvalidSector {
                sector: self.0,
                reason: "cannot calculate mini-stream offset for non-regular mini-sector"
                    .to_string(),
            });
        }
        let index = u64::from(self.0);
        let offset = index
            .checked_shl(u32::from(mini_sector_shift))
            .ok_or_else(|| Error::InvalidSector {
                sector: self.0,
                reason: "mini-sector offset calculation overflowed".to_string(),
            })?;
        Ok(offset)
    }
}

impl fmt::Display for MiniSectorId {
    /// Formats the mini sector index as a string.
    ///
    /// # Arguments
    ///
    /// * `f` - Formatter.
    ///
    /// # Returns
    ///
    /// Format result.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            Self::FREE => write!(f, "MINI_FREE(0x{:08X})", self.0),
            Self::END_OF_CHAIN => write!(f, "MINI_ENDOFCHAIN(0x{:08X})", self.0),
            _ => write!(f, "MINI_SECTOR(0x{:08X})", self.0),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Tests [`SectorId`] predicates and properties.
    #[test]
    fn test_sector_id_predicates() {
        let sec_0 = SectorId::new(0);
        assert!(sec_0.is_regular());
        assert!(!sec_0.is_end_of_chain());
        assert!(!sec_0.is_free());
        assert!(!sec_0.is_fat());
        assert!(!sec_0.is_difat());
        assert_eq!(sec_0.as_u32(), 0);

        let sec_max = SectorId::MAX_REG;
        assert!(sec_max.is_regular());

        let sec_res = SectorId::RESERVED;
        assert!(!sec_res.is_regular());

        let sec_difat = SectorId::DIFAT;
        assert!(sec_difat.is_difat());
        assert!(!sec_difat.is_regular());

        let sec_fat = SectorId::FAT;
        assert!(sec_fat.is_fat());
        assert!(!sec_fat.is_regular());

        let sec_end = SectorId::END_OF_CHAIN;
        assert!(sec_end.is_end_of_chain());
        assert!(!sec_end.is_regular());

        let sec_free = SectorId::FREE;
        assert!(sec_free.is_free());
        assert!(!sec_free.is_regular());
    }

    /// Tests [`SectorId::file_offset`] computation and error handling.
    #[test]
    fn test_sector_id_file_offset() {
        let sec_0 = SectorId::new(0);
        assert_eq!(sec_0.file_offset(9), Ok(512));
        assert_eq!(sec_0.file_offset(12), Ok(4096));

        let sec_1 = SectorId::new(1);
        assert_eq!(sec_1.file_offset(9), Ok(1024));

        let sec_free = SectorId::FREE;
        assert!(sec_free.file_offset(9).is_err());

        // Test overflow condition: shift >= 64
        assert!(sec_0.file_offset(64).is_err());
    }

    /// Tests [`SectorId`] Display implementation.
    #[test]
    fn test_sector_id_display() {
        assert_eq!(format!("{}", SectorId::FREE), "FREE(0xFFFFFFFF)");
        assert_eq!(
            format!("{}", SectorId::END_OF_CHAIN),
            "ENDOFCHAIN(0xFFFFFFFE)"
        );
        assert_eq!(format!("{}", SectorId::FAT), "FAT(0xFFFFFFFD)");
        assert_eq!(format!("{}", SectorId::DIFAT), "DIFAT(0xFFFFFFFC)");
        assert_eq!(format!("{}", SectorId::RESERVED), "RESERVED(0xFFFFFFFB)");
        assert_eq!(format!("{}", SectorId::new(42)), "SECTOR(0x0000002A)");
    }

    /// Tests [`MiniSectorId`] predicates, offset calculations, and display.
    #[test]
    fn test_mini_sector_id() {
        let m_0 = MiniSectorId::new(0);
        assert!(m_0.is_regular());
        assert!(!m_0.is_end_of_chain());
        assert!(!m_0.is_free());
        assert_eq!(m_0.as_u32(), 0);
        assert_eq!(m_0.mini_stream_offset(6), Ok(0));

        let m_1 = MiniSectorId::new(1);
        assert_eq!(m_1.mini_stream_offset(6), Ok(64));

        let m_end = MiniSectorId::END_OF_CHAIN;
        assert!(m_end.is_end_of_chain());
        assert!(!m_end.is_regular());
        assert!(m_end.mini_stream_offset(6).is_err());

        let m_free = MiniSectorId::FREE;
        assert!(m_free.is_free());
        assert!(!m_free.is_regular());

        // Overflow condition
        assert!(m_1.mini_stream_offset(64).is_err());

        assert_eq!(format!("{m_free}"), "MINI_FREE(0xFFFFFFFF)");
        assert_eq!(format!("{m_end}"), "MINI_ENDOFCHAIN(0xFFFFFFFE)");
        assert_eq!(format!("{m_0}"), "MINI_SECTOR(0x00000000)");
    }
}

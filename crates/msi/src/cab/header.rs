//! Cabinet File Header structures and flags (`CFHEADER`).

use crate::error::{Error, Result};

/// Magic 4-byte signature for Microsoft Cabinet files (`MSCF` / `0x4D534346`).
pub const CAB_SIGNATURE: [u8; 4] = *b"MSCF";

/// Expected major version for Cabinet files (`1`).
pub const CAB_VERSION_MAJOR: u8 = 1;

/// Expected minor version for Cabinet files (`3`).
pub const CAB_VERSION_MINOR: u8 = 3;

/// Flag indicating that this cabinet is preceded by another cabinet in a multi-cabinet set.
pub const CFHDR_PREV_CABINET: u16 = 0x0001;

/// Flag indicating that this cabinet is followed by another cabinet in a multi-cabinet set.
pub const CFHDR_NEXT_CABINET: u16 = 0x0002;

/// Flag indicating that per-cabinet, per-folder, and per-data reserve areas are present.
pub const CFHDR_RESERVE_PRESENT: u16 = 0x0004;

/// Strongly-typed Cabinet header flags bitmask.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct HeaderFlags(pub u16);

impl HeaderFlags {
    /// Creates a new [`HeaderFlags`] from raw `u16`.
    ///
    /// # Arguments
    ///
    /// * `val` - The raw 16-bit flag integer.
    ///
    /// # Returns
    ///
    /// A new [`HeaderFlags`].
    #[must_use]
    pub const fn from_bits(val: u16) -> Self {
        Self(val)
    }

    /// Returns the raw 16-bit flag value.
    ///
    /// # Returns
    ///
    /// Raw integer flags as [`u16`].
    #[must_use]
    pub const fn bits(self) -> u16 {
        self.0
    }

    /// Returns `true` if there is a previous cabinet in the set.
    ///
    /// # Returns
    ///
    /// Boolean indicating previous cabinet flag status.
    #[must_use]
    pub const fn has_prev_cabinet(self) -> bool {
        (self.0 & CFHDR_PREV_CABINET) != 0
    }

    /// Returns `true` if there is a next cabinet in the set.
    ///
    /// # Returns
    ///
    /// Boolean indicating next cabinet flag status.
    #[must_use]
    pub const fn has_next_cabinet(self) -> bool {
        (self.0 & CFHDR_NEXT_CABINET) != 0
    }

    /// Returns `true` if reserve sizes are present in the cabinet structures.
    ///
    /// # Returns
    ///
    /// Boolean indicating reserve present flag status.
    #[must_use]
    pub const fn has_reserve(self) -> bool {
        (self.0 & CFHDR_RESERVE_PRESENT) != 0
    }
}

/// Optional reserve sizes present in Cabinet files when [`CFHDR_RESERVE_PRESENT`] is set.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct CabReserveSizes {
    /// Number of bytes of reserve in [`CfHeader`].
    pub header_reserve: u16,
    /// Number of bytes of reserve in each `CFFOLDER`.
    pub folder_reserve: u8,
    /// Number of bytes of reserve in each `CFDATA`.
    pub data_reserve: u8,
}

/// Parsed Microsoft Cabinet file header structure (`CFHEADER`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CfHeader {
    /// Total size of this cabinet file in bytes.
    pub cabinet_size: u32,
    /// Absolute byte offset of the first `CFFILE` entry.
    pub files_offset: u32,
    /// Number of `CFFOLDER` entries in this cabinet.
    pub folder_count: u16,
    /// Number of `CFFILE` entries in this cabinet.
    pub file_count: u16,
    /// Header flags.
    pub flags: HeaderFlags,
    /// Set identifier shared across all cabinets in a multi-cabinet split set.
    pub set_id: u16,
    /// Zero-based index of this cabinet in a multi-cabinet set.
    pub cabinet_index: u16,
    /// Optional reserve sizes.
    pub reserve_sizes: Option<CabReserveSizes>,
    /// Optional header reserve bytes.
    pub header_reserve_data: Vec<u8>,
    /// Optional previous cabinet filename.
    pub prev_cabinet: Option<String>,
    /// Optional previous disk name.
    pub prev_disk: Option<String>,
    /// Optional next cabinet filename.
    pub next_cabinet: Option<String>,
    /// Optional next disk name.
    pub next_disk: Option<String>,
}

impl CfHeader {
    /// Creates a new, default [`CfHeader`].
    ///
    /// # Returns
    ///
    /// An empty initialized [`CfHeader`].
    #[must_use]
    pub fn new() -> Self {
        Self {
            cabinet_size: 0,
            files_offset: 0,
            folder_count: 0,
            file_count: 0,
            flags: HeaderFlags::default(),
            set_id: 0,
            cabinet_index: 0,
            reserve_sizes: None,
            header_reserve_data: Vec::new(),
            prev_cabinet: None,
            prev_disk: None,
            next_cabinet: None,
            next_disk: None,
        }
    }

    /// Parses a null-terminated string from `bytes` starting at `cursor`.
    fn parse_cstring(bytes: &[u8], cursor: &mut usize) -> Result<String> {
        let start = *cursor;
        while *cursor < bytes.len() && bytes[*cursor] != 0 {
            *cursor += 1;
        }
        if *cursor >= bytes.len() {
            return Err(Error::InvalidCabData {
                reason: "unterminated string in cabinet header".to_string(),
            });
        }
        let s = String::from_utf8_lossy(&bytes[start..*cursor]).to_string();
        *cursor += 1; // consume null byte
        Ok(s)
    }

    /// Parses and validates a [`CfHeader`] from raw bytes.
    ///
    /// # Arguments
    ///
    /// * `bytes` - The slice of bytes starting at the beginning of the Cabinet file.
    ///
    /// # Returns
    ///
    /// A tuple containing `(header, bytes_consumed)`.
    ///
    /// # Errors
    ///
    /// Returns [`Error::InvalidCabSignature`], [`Error::InvalidCabVersion`], or
    /// [`Error::InvalidCabData`] if the header is corrupted.
    #[allow(clippy::too_many_lines)]
    pub fn parse(bytes: &[u8]) -> Result<(Self, usize)> {
        if bytes.len() < 32 {
            return Err(Error::InvalidCabData {
                reason: format!("cabinet header too short: {} bytes (min 32)", bytes.len()),
            });
        }

        // 1. Signature
        if bytes[0..4] != CAB_SIGNATURE {
            return Err(Error::InvalidCabSignature {
                found: [bytes[0], bytes[1], bytes[2], bytes[3]],
            });
        }

        // 2. Reserved1 (must be 0)
        let res1 = u32::from_le_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]);
        if res1 != 0 {
            return Err(Error::InvalidCabData {
                reason: format!("reserved1 field must be 0, found 0x{res1:08X}"),
            });
        }

        // 3. cbCabinet
        let cabinet_size = u32::from_le_bytes([bytes[8], bytes[9], bytes[10], bytes[11]]);

        // 4. Reserved2 (must be 0)
        let res2 = u32::from_le_bytes([bytes[12], bytes[13], bytes[14], bytes[15]]);
        if res2 != 0 {
            return Err(Error::InvalidCabData {
                reason: format!("reserved2 field must be 0, found 0x{res2:08X}"),
            });
        }

        // 5. coffFiles
        let files_offset = u32::from_le_bytes([bytes[16], bytes[17], bytes[18], bytes[19]]);

        // 6. Reserved3 (must be 0)
        let res3 = u32::from_le_bytes([bytes[20], bytes[21], bytes[22], bytes[23]]);
        if res3 != 0 {
            return Err(Error::InvalidCabData {
                reason: format!("reserved3 field must be 0, found 0x{res3:08X}"),
            });
        }

        // 7. Version Minor & Major
        let version_minor = bytes[24];
        let version_major = bytes[25];
        if version_major != CAB_VERSION_MAJOR || version_minor != CAB_VERSION_MINOR {
            return Err(Error::InvalidCabVersion {
                major: version_major,
                minor: version_minor,
            });
        }

        // 8. cFolders & cFiles
        let folder_count = u16::from_le_bytes([bytes[26], bytes[27]]);
        let file_count = u16::from_le_bytes([bytes[28], bytes[29]]);

        // 9. Flags
        let raw_flags = u16::from_le_bytes([bytes[30], bytes[31]]);
        let flags = HeaderFlags::from_bits(raw_flags);

        // 10. setID & iCabinet
        if bytes.len() < 36 {
            return Err(Error::InvalidCabData {
                reason: "cabinet header truncated before setID/iCabinet".to_string(),
            });
        }
        let set_id = u16::from_le_bytes([bytes[32], bytes[33]]);
        let cabinet_index = u16::from_le_bytes([bytes[34], bytes[35]]);

        let mut cursor = 36;
        let mut reserve_sizes = None;
        let mut header_reserve_data = Vec::new();

        // 11. Optional reserve
        if flags.has_reserve() {
            if bytes.len() < cursor + 4 {
                return Err(Error::InvalidCabData {
                    reason: "cabinet header truncated in reserve sizes".to_string(),
                });
            }
            let cb_header = u16::from_le_bytes([bytes[cursor], bytes[cursor + 1]]);
            let cb_folder = bytes[cursor + 2];
            let cb_data = bytes[cursor + 3];
            cursor += 4;

            reserve_sizes = Some(CabReserveSizes {
                header_reserve: cb_header,
                folder_reserve: cb_folder,
                data_reserve: cb_data,
            });

            if cb_header > 0 {
                let end = cursor + cb_header as usize;
                if bytes.len() < end {
                    return Err(Error::InvalidCabData {
                        reason: "cabinet header truncated in reserve data".to_string(),
                    });
                }
                header_reserve_data = bytes[cursor..end].to_vec();
                cursor = end;
            }
        }

        // 12. Optional previous cabinet chain fields
        let (prev_cabinet, prev_disk) = if flags.has_prev_cabinet() {
            (
                Some(Self::parse_cstring(bytes, &mut cursor)?),
                Some(Self::parse_cstring(bytes, &mut cursor)?),
            )
        } else {
            (None, None)
        };

        // 13. Optional next cabinet chain fields
        let (next_cabinet, next_disk) = if flags.has_next_cabinet() {
            (
                Some(Self::parse_cstring(bytes, &mut cursor)?),
                Some(Self::parse_cstring(bytes, &mut cursor)?),
            )
        } else {
            (None, None)
        };

        Ok((
            Self {
                cabinet_size,
                files_offset,
                folder_count,
                file_count,
                flags,
                set_id,
                cabinet_index,
                reserve_sizes,
                header_reserve_data,
                prev_cabinet,
                prev_disk,
                next_cabinet,
                next_disk,
            },
            cursor,
        ))
    }

    /// Serializes this [`CfHeader`] into binary format.
    ///
    /// # Returns
    ///
    /// Byte vector containing the binary cabinet header.
    #[must_use]
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut buf = Vec::with_capacity(64);

        // 1. Signature
        buf.extend_from_slice(&CAB_SIGNATURE);

        // 2. Reserved1
        buf.extend_from_slice(&0u32.to_le_bytes());

        // 3. cbCabinet
        buf.extend_from_slice(&self.cabinet_size.to_le_bytes());

        // 4. Reserved2
        buf.extend_from_slice(&0u32.to_le_bytes());

        // 5. coffFiles
        buf.extend_from_slice(&self.files_offset.to_le_bytes());

        // 6. Reserved3
        buf.extend_from_slice(&0u32.to_le_bytes());

        // 7. Version Minor & Major
        buf.push(CAB_VERSION_MINOR);
        buf.push(CAB_VERSION_MAJOR);

        // 8. cFolders & cFiles
        buf.extend_from_slice(&self.folder_count.to_le_bytes());
        buf.extend_from_slice(&self.file_count.to_le_bytes());

        // 9. Flags
        buf.extend_from_slice(&self.flags.bits().to_le_bytes());

        // 10. setID & iCabinet
        buf.extend_from_slice(&self.set_id.to_le_bytes());
        buf.extend_from_slice(&self.cabinet_index.to_le_bytes());

        // 11. Reserve
        if let Some(res) = self.reserve_sizes {
            buf.extend_from_slice(&res.header_reserve.to_le_bytes());
            buf.push(res.folder_reserve);
            buf.push(res.data_reserve);
            if res.header_reserve > 0 {
                buf.extend_from_slice(&self.header_reserve_data);
            }
        }

        // 12. Prev cabinet chain
        if let Some(ref prev_cab) = self.prev_cabinet {
            buf.extend_from_slice(prev_cab.as_bytes());
            buf.push(0);
            let dsk = self.prev_disk.as_deref().unwrap_or("");
            buf.extend_from_slice(dsk.as_bytes());
            buf.push(0);
        }

        // 13. Next cabinet chain
        if let Some(ref next_cab) = self.next_cabinet {
            buf.extend_from_slice(next_cab.as_bytes());
            buf.push(0);
            let dsk = self.next_disk.as_deref().unwrap_or("");
            buf.extend_from_slice(dsk.as_bytes());
            buf.push(0);
        }

        buf
    }
}

impl Default for CfHeader {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Tests [`CfHeader::default`] constructor.
    #[test]
    fn test_cf_header_default() {
        let def = CfHeader::default();
        assert_eq!(def.cabinet_size, 0);
        assert_eq!(def.file_count, 0);
    }

    /// Tests [`HeaderFlags`] predicate methods.
    #[test]
    fn test_header_flags() {
        let f = HeaderFlags::from_bits(CFHDR_PREV_CABINET | CFHDR_RESERVE_PRESENT);
        assert!(f.has_prev_cabinet());
        assert!(!f.has_next_cabinet());
        assert!(f.has_reserve());
        assert_eq!(f.bits(), 0x0005);
    }

    /// Tests serialization and parsing roundtrip of a minimal [`CfHeader`].
    #[test]
    fn test_cf_header_roundtrip_minimal() {
        let mut header = CfHeader::new();
        header.cabinet_size = 1000;
        header.files_offset = 64;
        header.folder_count = 1;
        header.file_count = 2;
        header.set_id = 42;
        header.cabinet_index = 0;

        let bytes = header.to_bytes();
        assert_eq!(bytes.len(), 36);

        let parsed_res = CfHeader::parse(&bytes);
        assert_eq!(parsed_res, Ok((header, 36)));
    }

    /// Tests serialization and parsing roundtrip with reserve and multi-cabinet chains.
    #[test]
    fn test_cf_header_roundtrip_full() {
        let mut header = CfHeader::new();
        header.cabinet_size = 2000;
        header.files_offset = 128;
        header.folder_count = 2;
        header.file_count = 5;
        header.flags =
            HeaderFlags::from_bits(CFHDR_PREV_CABINET | CFHDR_NEXT_CABINET | CFHDR_RESERVE_PRESENT);
        header.set_id = 1234;
        header.cabinet_index = 1;
        header.reserve_sizes = Some(CabReserveSizes {
            header_reserve: 4,
            folder_reserve: 2,
            data_reserve: 2,
        });
        header.header_reserve_data = vec![0xDE, 0xAD, 0xBE, 0xEF];
        header.prev_cabinet = Some("disk1.cab".to_string());
        header.prev_disk = Some("Disk 1".to_string());
        header.next_cabinet = Some("disk3.cab".to_string());
        header.next_disk = Some("Disk 3".to_string());

        let bytes = header.to_bytes();
        let parsed_res = CfHeader::parse(&bytes);
        assert_eq!(parsed_res, Ok((header, bytes.len())));
    }

    /// Tests roundtrip when reserve sizes are present but `header_reserve == 0`.
    #[test]
    fn test_cf_header_roundtrip_zero_header_reserve() {
        let mut header = CfHeader::new();
        header.flags = HeaderFlags::from_bits(CFHDR_RESERVE_PRESENT);
        header.reserve_sizes = Some(CabReserveSizes {
            header_reserve: 0,
            folder_reserve: 5,
            data_reserve: 10,
        });
        let bytes = header.to_bytes();
        let parsed_res = CfHeader::parse(&bytes);
        assert_eq!(parsed_res, Ok((header, bytes.len())));
    }

    /// Tests cabinet chain serialization when disk names are None.
    #[test]
    fn test_cf_header_roundtrip_chains_without_disk_names() {
        let mut header = CfHeader::new();
        header.flags = HeaderFlags::from_bits(CFHDR_PREV_CABINET | CFHDR_NEXT_CABINET);
        header.prev_cabinet = Some("prev.cab".to_string());
        header.prev_disk = None;
        header.next_cabinet = Some("next.cab".to_string());
        header.next_disk = None;
        let bytes = header.to_bytes();
        let parsed = CfHeader::parse(&bytes);
        let mut expected = header;
        expected.prev_disk = Some(String::new());
        expected.next_disk = Some(String::new());
        assert_eq!(parsed, Ok((expected, bytes.len())));
    }

    /// Tests header parsing validation errors.
    #[test]
    #[allow(clippy::cast_possible_truncation)]
    fn test_cf_header_parse_errors() {
        // Truncated (< 32 bytes)
        assert!(CfHeader::parse(&[0; 10]).is_err());

        let header = CfHeader::new();
        let bytes = header.to_bytes();

        // Bad signature
        let mut bad_sig = bytes.clone();
        bad_sig[0] = b'X';
        assert!(matches!(
            CfHeader::parse(&bad_sig),
            Err(Error::InvalidCabSignature { .. })
        ));

        // Bad reserved1
        let mut bad_res1 = bytes.clone();
        bad_res1[4] = 1;
        assert!(matches!(
            CfHeader::parse(&bad_res1),
            Err(Error::InvalidCabData { .. })
        ));

        // Bad reserved2
        let mut bad_res2 = bytes.clone();
        bad_res2[12] = 1;
        assert!(matches!(
            CfHeader::parse(&bad_res2),
            Err(Error::InvalidCabData { .. })
        ));

        // Bad reserved3
        let mut bad_res3 = bytes.clone();
        bad_res3[20] = 1;
        assert!(matches!(
            CfHeader::parse(&bad_res3),
            Err(Error::InvalidCabData { .. })
        ));

        // Bad major version
        let mut bad_ver = bytes.clone();
        bad_ver[25] = 2; // Major version 2
        assert!(matches!(
            CfHeader::parse(&bad_ver),
            Err(Error::InvalidCabVersion { .. })
        ));

        // Bad minor version
        let mut bad_ver_minor = bytes.clone();
        bad_ver_minor[24] = 99; // Minor version 99 with major 1
        assert!(matches!(
            CfHeader::parse(&bad_ver_minor),
            Err(Error::InvalidCabVersion { .. })
        ));

        // Truncated before setID
        assert!(CfHeader::parse(&bytes[0..34]).is_err());

        // Reserve present flag but truncated
        let mut bad_res = bytes.clone();
        bad_res[30] = CFHDR_RESERVE_PRESENT as u8;
        assert!(CfHeader::parse(&bad_res).is_err());

        // Reserve data truncated
        let mut bad_res_data = bytes.clone();
        bad_res_data[30] = CFHDR_RESERVE_PRESENT as u8;
        bad_res_data.extend_from_slice(&10u16.to_le_bytes()); // header_reserve = 10
        bad_res_data.push(0); // folder_reserve
        bad_res_data.push(0); // data_reserve
                              // only 4 bytes of data instead of 10
        bad_res_data.extend_from_slice(&[0; 4]);
        assert!(CfHeader::parse(&bad_res_data).is_err());

        // Unterminated cstring in prev cabinet
        let mut bad_str = bytes;
        bad_str[30] = CFHDR_PREV_CABINET as u8;
        bad_str.extend_from_slice(b"unterminated_string_without_null");
        assert!(CfHeader::parse(&bad_str).is_err());
    }
}

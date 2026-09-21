//! OLE Property Set Summary Information Stream implementation ([MS-OLEPS]).

use crate::error::{Error, Result};

/// OLE Property Set byte order marker (0xFFFE).
pub const OLEPS_BYTE_ORDER: u16 = 0xFFFE;

/// Standard Format ID for Summary Information (`{F29F85E0-4FF9-1068-AB91-08002B27B3D9}`).
pub const FMTID_SUMMARY_INFORMATION: [u8; 16] = [
    0xE0, 0x85, 0x9F, 0xF2, 0xF9, 0x4F, 0x68, 0x10, 0xAB, 0x91, 0x08, 0x00, 0x2B, 0x27, 0xB3, 0xD9,
];

/// Property ID for code page (`PID_CODEPAGE`).
pub const PID_CODEPAGE: u32 = 0x0001;

/// Property ID for title (`PID_TITLE`).
pub const PID_TITLE: u32 = 0x0002;

/// Property ID for subject (`PID_SUBJECT`).
pub const PID_SUBJECT: u32 = 0x0003;

/// Property ID for author (`PID_AUTHOR`).
pub const PID_AUTHOR: u32 = 0x0004;

/// Property ID for keywords (`PID_KEYWORDS`).
pub const PID_KEYWORDS: u32 = 0x0005;

/// Property ID for comments (`PID_COMMENTS`).
pub const PID_COMMENTS: u32 = 0x0006;

/// Property ID for template target platform and languages (`PID_TEMPLATE`).
pub const PID_TEMPLATE: u32 = 0x0007;

/// Property ID for last author / transform author (`PID_LASTAUTHOR`).
pub const PID_LASTAUTHOR: u32 = 0x0008;

/// Property ID for package revision GUID (`PID_REVNUMBER`).
pub const PID_REVNUMBER: u32 = 0x0009;

/// Property ID for last printed timestamp (`PID_LASTPRINTED`).
pub const PID_LASTPRINTED: u32 = 0x000B;

/// Property ID for creation timestamp (`PID_CREATE_DTM`).
pub const PID_CREATE_DTM: u32 = 0x000C;

/// Property ID for last save timestamp (`PID_LASTSAVE_DTM`).
pub const PID_LASTSAVE_DTM: u32 = 0x000D;

/// Property ID for minimum installer version / page count (`PID_PAGECOUNT`).
pub const PID_PAGECOUNT: u32 = 0x000E;

/// Property ID for package flags / word count (`PID_WORDCOUNT`).
pub const PID_WORDCOUNT: u32 = 0x000F;

/// Property ID for character count (`PID_CHARCOUNT`).
pub const PID_CHARCOUNT: u32 = 0x0010;

/// Property ID for authoring application name (`PID_APPNAME`).
pub const PID_APPNAME: u32 = 0x0012;

/// Property ID for security flags (`PID_SECURITY`).
pub const PID_SECURITY: u32 = 0x0013;

/// OLE Property Set Variant type for 16-bit integer (`VT_I2`).
pub const VT_I2: u32 = 2;

/// OLE Property Set Variant type for 32-bit integer (`VT_I4`).
pub const VT_I4: u32 = 3;

/// OLE Property Set Variant type for byte string (`VT_LPSTR`).
pub const VT_LPSTR: u32 = 30;

/// OLE Property Set Variant type for 64-bit Windows `FILETIME` (`VT_FILETIME`).
pub const VT_FILETIME: u32 = 64;

/// Strongly-typed representation of an MSI package's Summary Information stream.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SummaryInfo {
    /// ANSI or UTF-8 code page (`PID_CODEPAGE`).
    pub codepage: Option<u16>,
    /// Package title (`PID_TITLE`).
    pub title: Option<String>,
    /// Product subject / name (`PID_SUBJECT`).
    pub subject: Option<String>,
    /// Manufacturer / Author (`PID_AUTHOR`).
    pub author: Option<String>,
    /// Search keywords (`PID_KEYWORDS`).
    pub keywords: Option<String>,
    /// Description comments (`PID_COMMENTS`).
    pub comments: Option<String>,
    /// Target platform and languages string, e.g. `x64;1033` (`PID_TEMPLATE`).
    pub template: Option<String>,
    /// Transform or patch author (`PID_LASTAUTHOR`).
    pub last_author: Option<String>,
    /// Package GUID (`PID_REVNUMBER`).
    pub rev_number: Option<String>,
    /// Administrative installation timestamp (`PID_LASTPRINTED`).
    pub last_printed: Option<u64>,
    /// Package creation timestamp (`PID_CREATE_DTM`).
    pub create_time: Option<u64>,
    /// Package modification timestamp (`PID_LASTSAVE_DTM`).
    pub last_save_time: Option<u64>,
    /// Minimum installer version, e.g. 500 for MSI 5.0 (`PID_PAGECOUNT`).
    pub page_count: Option<i32>,
    /// Package flags (1=compressed, 2=short names, 4=elevation required) (`PID_WORDCOUNT`).
    pub word_count: Option<i32>,
    /// Unused/reserved character count (`PID_CHARCOUNT`).
    pub char_count: Option<i32>,
    /// Authoring application name (`PID_APPNAME`).
    pub app_name: Option<String>,
    /// Security restriction flags (`PID_SECURITY`).
    pub security: Option<i32>,
}

impl SummaryInfo {
    /// Creates an empty [`SummaryInfo`].
    ///
    /// # Returns
    ///
    /// An empty initialized [`SummaryInfo`].
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Serializes this [`SummaryInfo`] into standard OLE Property Set binary stream format.
    ///
    /// # Returns
    ///
    /// Byte vector containing the serialized `\005SummaryInformation` stream payload.
    #[must_use]
    #[allow(clippy::too_many_lines, clippy::cast_possible_truncation)]
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut props: Vec<(u32, u32, Vec<u8>)> = Vec::new(); // (propid, vt, raw_val)

        if let Some(cp) = self.codepage {
            props.push((PID_CODEPAGE, VT_I2, cp.to_le_bytes().to_vec()));
        }
        if let Some(ref title) = self.title {
            props.push((PID_TITLE, VT_LPSTR, Self::encode_lpstr(title)));
        }
        if let Some(ref subject) = self.subject {
            props.push((PID_SUBJECT, VT_LPSTR, Self::encode_lpstr(subject)));
        }
        if let Some(ref author) = self.author {
            props.push((PID_AUTHOR, VT_LPSTR, Self::encode_lpstr(author)));
        }
        if let Some(ref keywords) = self.keywords {
            props.push((PID_KEYWORDS, VT_LPSTR, Self::encode_lpstr(keywords)));
        }
        if let Some(ref comments) = self.comments {
            props.push((PID_COMMENTS, VT_LPSTR, Self::encode_lpstr(comments)));
        }
        if let Some(ref template) = self.template {
            props.push((PID_TEMPLATE, VT_LPSTR, Self::encode_lpstr(template)));
        }
        if let Some(ref last_author) = self.last_author {
            props.push((PID_LASTAUTHOR, VT_LPSTR, Self::encode_lpstr(last_author)));
        }
        if let Some(ref rev) = self.rev_number {
            props.push((PID_REVNUMBER, VT_LPSTR, Self::encode_lpstr(rev)));
        }
        if let Some(ts) = self.last_printed {
            props.push((PID_LASTPRINTED, VT_FILETIME, ts.to_le_bytes().to_vec()));
        }
        if let Some(ts) = self.create_time {
            props.push((PID_CREATE_DTM, VT_FILETIME, ts.to_le_bytes().to_vec()));
        }
        if let Some(ts) = self.last_save_time {
            props.push((PID_LASTSAVE_DTM, VT_FILETIME, ts.to_le_bytes().to_vec()));
        }
        if let Some(val) = self.page_count {
            props.push((PID_PAGECOUNT, VT_I4, val.to_le_bytes().to_vec()));
        }
        if let Some(val) = self.word_count {
            props.push((PID_WORDCOUNT, VT_I4, val.to_le_bytes().to_vec()));
        }
        if let Some(val) = self.char_count {
            props.push((PID_CHARCOUNT, VT_I4, val.to_le_bytes().to_vec()));
        }
        if let Some(ref app) = self.app_name {
            props.push((PID_APPNAME, VT_LPSTR, Self::encode_lpstr(app)));
        }
        if let Some(val) = self.security {
            props.push((PID_SECURITY, VT_I4, val.to_le_bytes().to_vec()));
        }

        // Stream layout:
        // 0..28: Header (28 bytes)
        // 28..48: Section entry (20 bytes)
        // 48..: Section body
        let header_len = 48usize;
        let mut section_data = Vec::new();

        let num_props = props.len() as u32;
        // Section header: 4 bytes cbSection, 4 bytes cProperties
        // Followed by property identifier + offset table (8 bytes per property)
        let prop_table_len = 8 + (num_props as usize) * 8;
        let mut values_data = Vec::new();
        let mut prop_entries = Vec::new();

        for (propid, vt, raw_val) in props {
            let val_offset = (prop_table_len + values_data.len()) as u32;
            prop_entries.extend_from_slice(&propid.to_le_bytes());
            prop_entries.extend_from_slice(&val_offset.to_le_bytes());

            values_data.extend_from_slice(&vt.to_le_bytes());
            values_data.extend_from_slice(&raw_val);
            // Pad value to 4 bytes if not already aligned
            let rem = values_data.len() % 4;
            if rem != 0 {
                values_data.extend_from_slice(&vec![0; 4 - rem]);
            }
        }

        let section_len = (8 + prop_entries.len() + values_data.len()) as u32;
        section_data.extend_from_slice(&section_len.to_le_bytes());
        section_data.extend_from_slice(&num_props.to_le_bytes());
        section_data.extend_from_slice(&prop_entries);
        section_data.extend_from_slice(&values_data);

        let mut output = Vec::with_capacity(header_len + section_data.len());
        // Header (28 bytes)
        output.extend_from_slice(&OLEPS_BYTE_ORDER.to_le_bytes()); // wByteOrder
        output.extend_from_slice(&0u16.to_le_bytes()); // wFormat
        output.extend_from_slice(&0x0002_0105u32.to_le_bytes()); // dwOSVer
        output.extend_from_slice(&[0u8; 16]); // clsid (null)
        output.extend_from_slice(&1u32.to_le_bytes()); // cSections (1)

        // Section Entry (20 bytes)
        output.extend_from_slice(&FMTID_SUMMARY_INFORMATION); // fmtid
        output.extend_from_slice(&(header_len as u32).to_le_bytes()); // dwOffset

        output.extend_from_slice(&section_data);
        output
    }

    /// Helper to encode a string into `VT_LPSTR` format (4-byte length + string + null byte + padding).
    #[allow(clippy::cast_possible_truncation)]
    fn encode_lpstr(s: &str) -> Vec<u8> {
        let bytes = s.as_bytes();
        let len_with_null = (bytes.len() + 1) as u32;
        let mut buf = Vec::with_capacity(4 + bytes.len() + 1);
        buf.extend_from_slice(&len_with_null.to_le_bytes());
        buf.extend_from_slice(bytes);
        buf.push(0); // null terminator
        let rem = buf.len() % 4;
        if rem != 0 {
            buf.extend_from_slice(&vec![0; 4 - rem]);
        }
        buf
    }

    /// Parses an OLE Property Set Summary Information stream.
    ///
    /// # Arguments
    ///
    /// * `bytes` - Raw stream bytes.
    ///
    /// # Returns
    ///
    /// A parsed [`SummaryInfo`].
    ///
    /// # Errors
    ///
    /// Returns [`Error::InvalidSummaryInfo`] if the stream header or property set is malformed.
    #[allow(clippy::too_many_lines, clippy::cast_possible_wrap)]
    pub fn parse(bytes: &[u8]) -> Result<Self> {
        if bytes.len() < 48 {
            return Err(Error::InvalidSummaryInfo {
                reason: format!(
                    "stream truncated: {} bytes (expected at least 48)",
                    bytes.len()
                ),
            });
        }

        let byte_order = u16::from_le_bytes([bytes[0], bytes[1]]);
        if byte_order != OLEPS_BYTE_ORDER {
            return Err(Error::InvalidSummaryInfo {
                reason: format!("invalid byte order mark 0x{byte_order:04X}"),
            });
        }

        let num_sections = u32::from_le_bytes([bytes[24], bytes[25], bytes[26], bytes[27]]);
        if num_sections == 0 {
            return Err(Error::InvalidSummaryInfo {
                reason: "zero sections in property set".to_string(),
            });
        }

        let section_offset =
            u32::from_le_bytes([bytes[44], bytes[45], bytes[46], bytes[47]]) as usize;
        if section_offset >= bytes.len() {
            return Err(Error::InvalidSummaryInfo {
                reason: "section offset extends beyond stream boundary".to_string(),
            });
        }

        let section_bytes = &bytes[section_offset..];
        if section_bytes.len() < 8 {
            return Err(Error::InvalidSummaryInfo {
                reason: "section header truncated".to_string(),
            });
        }

        let num_props = u32::from_le_bytes([
            section_bytes[4],
            section_bytes[5],
            section_bytes[6],
            section_bytes[7],
        ]) as usize;

        let mut summary = Self::new();

        for i in 0..num_props {
            let entry_offset = 8 + i * 8;
            if entry_offset + 8 > section_bytes.len() {
                break;
            }
            let propid = u32::from_le_bytes([
                section_bytes[entry_offset],
                section_bytes[entry_offset + 1],
                section_bytes[entry_offset + 2],
                section_bytes[entry_offset + 3],
            ]);
            let prop_offset = u32::from_le_bytes([
                section_bytes[entry_offset + 4],
                section_bytes[entry_offset + 5],
                section_bytes[entry_offset + 6],
                section_bytes[entry_offset + 7],
            ]) as usize;

            if prop_offset + 4 > section_bytes.len() {
                continue;
            }

            let vt = u32::from_le_bytes([
                section_bytes[prop_offset],
                section_bytes[prop_offset + 1],
                section_bytes[prop_offset + 2],
                section_bytes[prop_offset + 3],
            ]);
            let val_bytes = &section_bytes[prop_offset + 4..];

            match vt {
                VT_I2 => {
                    if val_bytes.len() >= 2 {
                        let val = u16::from_le_bytes([val_bytes[0], val_bytes[1]]);
                        if propid == PID_CODEPAGE {
                            summary.codepage = Some(val);
                        }
                    }
                }
                VT_I4 => {
                    if val_bytes.len() >= 4 {
                        let val = i32::from_le_bytes([
                            val_bytes[0],
                            val_bytes[1],
                            val_bytes[2],
                            val_bytes[3],
                        ]);
                        match propid {
                            PID_PAGECOUNT => summary.page_count = Some(val),
                            PID_WORDCOUNT => summary.word_count = Some(val),
                            PID_CHARCOUNT => summary.char_count = Some(val),
                            PID_SECURITY => summary.security = Some(val),
                            _ => {}
                        }
                    }
                }
                VT_FILETIME => {
                    if val_bytes.len() >= 8 {
                        let mut ts = [0u8; 8];
                        ts.copy_from_slice(&val_bytes[0..8]);
                        let val = u64::from_le_bytes(ts);
                        match propid {
                            PID_LASTPRINTED => summary.last_printed = Some(val),
                            PID_CREATE_DTM => summary.create_time = Some(val),
                            PID_LASTSAVE_DTM => summary.last_save_time = Some(val),
                            _ => {}
                        }
                    }
                }
                VT_LPSTR if val_bytes.len() >= 4 => {
                    let str_len = u32::from_le_bytes([
                        val_bytes[0],
                        val_bytes[1],
                        val_bytes[2],
                        val_bytes[3],
                    ]) as usize;
                    let text_end = (4 + str_len).min(val_bytes.len());
                    let raw_text = &val_bytes[4..text_end];
                    // Strip trailing null
                    let clean_text = if raw_text.ends_with(&[0]) {
                        &raw_text[0..raw_text.len() - 1]
                    } else {
                        raw_text
                    };
                    let val = String::from_utf8_lossy(clean_text).to_string();

                    match propid {
                        PID_TITLE => summary.title = Some(val),
                        PID_SUBJECT => summary.subject = Some(val),
                        PID_AUTHOR => summary.author = Some(val),
                        PID_KEYWORDS => summary.keywords = Some(val),
                        PID_COMMENTS => summary.comments = Some(val),
                        PID_TEMPLATE => summary.template = Some(val),
                        PID_LASTAUTHOR => summary.last_author = Some(val),
                        PID_REVNUMBER => summary.rev_number = Some(val),
                        PID_APPNAME => summary.app_name = Some(val),
                        _ => {}
                    }
                }
                _ => {}
            }
        }

        Ok(summary)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Tests serialization and parsing roundtrip of complete [`SummaryInfo`].
    #[test]
    fn test_summary_info_roundtrip() {
        let mut info = SummaryInfo::new();
        info.codepage = Some(1252);
        info.title = Some("Installation Database".to_string());
        info.subject = Some("Sample Product".to_string());
        info.author = Some("Acme Corp".to_string());
        info.keywords = Some("Installer, MSI".to_string());
        info.comments = Some("Product Description".to_string());
        info.template = Some("x64;1033".to_string());
        info.last_author = Some("Packager".to_string());
        info.rev_number = Some("{12345678-1234-1234-1234-1234567890AB}".to_string());
        info.last_printed = Some(100_000);
        info.create_time = Some(200_000);
        info.last_save_time = Some(300_000);
        info.page_count = Some(500);
        info.word_count = Some(2);
        info.char_count = Some(0);
        info.app_name = Some("msi-rs".to_string());
        info.security = Some(0);

        let bytes = info.to_bytes();
        let parsed_res = SummaryInfo::parse(&bytes);
        assert_eq!(parsed_res, Ok(info));

        // Also test empty SummaryInfo roundtrip (covers None branches in to_bytes)
        let empty_info = SummaryInfo::default();
        let empty_bytes = empty_info.to_bytes();
        let parsed_empty = SummaryInfo::parse(&empty_bytes);
        assert_eq!(parsed_empty, Ok(empty_info));
    }

    /// Tests summary information error handling for truncated or corrupted streams.
    #[test]
    fn test_summary_info_errors() {
        // Truncated (< 48 bytes)
        assert!(SummaryInfo::parse(&[0; 20]).is_err());

        // Bad byte order
        let mut bad_bo = [0u8; 48];
        bad_bo[0] = 0x00;
        bad_bo[1] = 0x00;
        assert!(SummaryInfo::parse(&bad_bo).is_err());

        // Zero sections
        let mut zero_sections = [0u8; 48];
        zero_sections[0] = 0xFE;
        zero_sections[1] = 0xFF; // Valid OLEPS byte order
        zero_sections[24] = 0; // 0 sections
        assert!(SummaryInfo::parse(&zero_sections).is_err());

        // Section offset out of bounds
        let mut oob_sec = [0u8; 48];
        oob_sec[0] = 0xFE;
        oob_sec[1] = 0xFF;
        oob_sec[24] = 1; // 1 section
        oob_sec[44] = 0xFF; // Section offset = 0xFF000000
        assert!(SummaryInfo::parse(&oob_sec).is_err());

        // Section header truncated (< 8 bytes)
        let mut trunc_sec_hdr = vec![0u8; 50];
        trunc_sec_hdr[0] = 0xFE;
        trunc_sec_hdr[1] = 0xFF;
        trunc_sec_hdr[24] = 1;
        trunc_sec_hdr[44] = 48; // section starts at 48, so section_bytes.len() is 2 (< 8)
        assert!(SummaryInfo::parse(&trunc_sec_hdr).is_err());

        // Synthesize valid stream with unknown property types and partial entries
        let mut custom_stream = vec![0u8; 48];
        custom_stream[0] = 0xFE;
        custom_stream[1] = 0xFF;
        custom_stream[24] = 1;
        custom_stream[44] = 48; // Section offset = 48

        // Section header: cbSection = 120, cProperties = 6
        let mut section = Vec::new();
        section.extend_from_slice(&120u32.to_le_bytes());
        section.extend_from_slice(&6u32.to_le_bytes());

        // Entry 0: propid 0x9999 (unknown), offset 64 (in section) -> vt 0x9999 (unknown)
        section.extend_from_slice(&0x9999u32.to_le_bytes());
        section.extend_from_slice(&64u32.to_le_bytes());

        // Entry 1: propid 0x8888, offset out of bounds
        section.extend_from_slice(&0x8888u32.to_le_bytes());
        section.extend_from_slice(&500u32.to_le_bytes());

        // Entry 2: propid 0x7777 (unknown I4), offset 72
        section.extend_from_slice(&0x7777u32.to_le_bytes());
        section.extend_from_slice(&72u32.to_le_bytes());

        // Entry 3: propid 0x6666 (unknown FILETIME), offset 80
        section.extend_from_slice(&0x6666u32.to_le_bytes());
        section.extend_from_slice(&80u32.to_le_bytes());

        // Entry 4: propid 0x5555 (unknown LPSTR without null terminator), offset 92
        section.extend_from_slice(&0x5555u32.to_le_bytes());
        section.extend_from_slice(&92u32.to_le_bytes());

        // Entry 5: propid 0x1111 (VT_I2 with non-codepage propid), offset 104
        section.extend_from_slice(&0x1111u32.to_le_bytes());
        section.extend_from_slice(&104u32.to_le_bytes());

        // Pad up to offset 64 in section
        while section.len() < 64 {
            section.push(0);
        }
        // At offset 64: unknown vt
        section.extend_from_slice(&0x9999u32.to_le_bytes());
        section.extend_from_slice(&[1, 2, 3, 4]);

        // At offset 72: VT_I4 with unknown propid
        section.extend_from_slice(&VT_I4.to_le_bytes());
        section.extend_from_slice(&42i32.to_le_bytes());

        // At offset 80: VT_FILETIME with unknown propid
        section.extend_from_slice(&VT_FILETIME.to_le_bytes());
        section.extend_from_slice(&12345u64.to_le_bytes());

        // At offset 92: VT_LPSTR without trailing null
        section.extend_from_slice(&VT_LPSTR.to_le_bytes());
        section.extend_from_slice(&4u32.to_le_bytes()); // string len 4
        section.extend_from_slice(b"test"); // no trailing 0

        // At offset 104: VT_I2 with non-codepage propid
        section.extend_from_slice(&VT_I2.to_le_bytes());
        section.extend_from_slice(&42u16.to_le_bytes());

        custom_stream.extend_from_slice(&section);
        let parsed_custom = SummaryInfo::parse(&custom_stream);
        assert!(parsed_custom.is_ok());

        // Also test entry offset overflow: cProperties = 1000 but stream ends
        let mut entry_overflow = vec![0u8; 48];
        entry_overflow[0] = 0xFE;
        entry_overflow[1] = 0xFF;
        entry_overflow[24] = 1;
        entry_overflow[44] = 48;
        entry_overflow.extend_from_slice(&100u32.to_le_bytes());
        entry_overflow.extend_from_slice(&1000u32.to_le_bytes());
        assert!(SummaryInfo::parse(&entry_overflow).is_ok());
    }

    /// Tests parsing properties whose value bytes are truncated near the end of the section.
    #[test]
    #[allow(clippy::cast_possible_truncation)]
    fn test_summary_info_truncated_property_payloads() {
        for (vt, extra_bytes) in [(VT_I2, 1), (VT_I4, 2), (VT_FILETIME, 4), (VT_LPSTR, 2)] {
            let mut stream = vec![0u8; 48];
            stream[0] = 0xFE;
            stream[1] = 0xFF;
            stream[24] = 1;
            stream[44] = 48;

            let section_len = 16 + 4 + extra_bytes;
            let mut section = Vec::new();
            section.extend_from_slice(&(section_len as u32).to_le_bytes());
            section.extend_from_slice(&1u32.to_le_bytes()); // 1 property
            section.extend_from_slice(&PID_TITLE.to_le_bytes());
            section.extend_from_slice(&16u32.to_le_bytes()); // offset 16

            section.extend_from_slice(&vt.to_le_bytes());
            section.extend_from_slice(&vec![0u8; extra_bytes]);

            stream.extend_from_slice(&section);
            assert!(SummaryInfo::parse(&stream).is_ok());
        }
    }
}

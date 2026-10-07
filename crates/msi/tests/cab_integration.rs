//! Integration tests for the CAB API.
#![allow(unused_variables)]

use msi::cab::data::CfData;
use msi::cab::file::{
    decode_dos_date, decode_dos_time, encode_dos_date, encode_dos_time, CfFile, FileAttributes,
};
use msi::cab::folder::{CfFolder, CompressionType};
use msi::cab::header::CfHeader;

#[test]
fn test_cab_api_surface() {
    // CfData
    let _ = CfData::new(vec![1, 2, 3], 3, vec![]);
    let _ = CfData::default();

    // CfFile
    let f = CfFile::new("test", 100);
    let _ = CfFile::default();
    let b = f.to_bytes();
    let _ = CfFile::parse(&b);

    let attrs = FileAttributes(0xFFFF);
    let _ = attrs.is_read_only();
    let _ = attrs.is_hidden();
    let _ = attrs.is_system();
    let _ = attrs.is_archive();
    let _ = attrs.is_exec();
    let _ = attrs.is_utf8_name();

    let d = encode_dos_date(2023, 10, 11);
    let _ = decode_dos_date(d);
    let t = encode_dos_time(12, 30, 45);
    let _ = decode_dos_time(t);

    // CfFolder
    let _ = CfFolder::new(CompressionType::None);
    let _ = CfFolder::default();

    // CfHeader
    let _ = CfHeader::default();
    let mut header_bytes = [0u8; 64];
    header_bytes[0..4].copy_from_slice(b"MSCF"); // signature
    header_bytes[24] = 3; // version minor
    header_bytes[25] = 1; // version major
    header_bytes[30] = 0x03; // FLAG_PREV_CABINET | FLAG_NEXT_CABINET
                             // Null terminators for cstrings
    header_bytes[36] = 0;
    header_bytes[37] = 0;
    header_bytes[38] = 0;
    header_bytes[39] = 0;
    let _ = CfHeader::parse(&header_bytes);
}

//! Integration tests for the CAB API.

use msi::cab::data::CfData;
use msi::cab::file::{
    decode_dos_date, decode_dos_time, encode_dos_date, encode_dos_time, CfFile, FileAttributes,
};
use msi::cab::folder::{CfFolder, CompressionType};
use msi::cab::header::CfHeader;
use msi::error::Result;

#[test]
fn test_cab_api_surface() -> Result<()> {
    // CfData
    let data = CfData::new(vec![1, 2, 3], 3, vec![])?;
    assert_eq!(data.uncompressed_size, 3);
    let default_data = CfData::default();
    assert_eq!(default_data.uncompressed_size, 0);

    // CfFile
    let f = CfFile::new("test", 100);
    assert_eq!(f.filename, "test");
    assert_eq!(f.file_size, 100);
    let default_file = CfFile::default();
    assert!(default_file.filename.is_empty());
    let b = f.to_bytes();
    let (parsed_file, consumed) = CfFile::parse(&b)?;
    assert_eq!(parsed_file.filename, "test");
    assert_eq!(parsed_file.file_size, 100);
    assert_eq!(consumed, b.len());

    let attrs = FileAttributes(0xFFFF);
    assert!(attrs.is_read_only());
    assert!(attrs.is_hidden());
    assert!(attrs.is_system());
    assert!(attrs.is_archive());
    assert!(attrs.is_exec());
    assert!(attrs.is_utf8_name());

    let d = encode_dos_date(2023, 10, 11);
    assert_eq!(decode_dos_date(d), (2023, 10, 11));
    let t = encode_dos_time(12, 30, 44);
    assert_eq!(decode_dos_time(t), (12, 30, 44));

    // CfFolder
    let folder = CfFolder::new(CompressionType::None);
    assert_eq!(folder.compression_type, CompressionType::None);
    let default_folder = CfFolder::default();
    assert_eq!(default_folder.compression_type, CompressionType::Mszip);

    // CfHeader
    let default_header = CfHeader::default();
    assert_eq!(default_header.cabinet_size, 0);
    assert_eq!(default_header.file_count, 0);
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
    let (parsed_header, hdr_len) = CfHeader::parse(&header_bytes)?;
    assert!(parsed_header.flags.has_prev_cabinet());
    assert!(parsed_header.flags.has_next_cabinet());
    assert_eq!(hdr_len, 40);
    Ok(())
}

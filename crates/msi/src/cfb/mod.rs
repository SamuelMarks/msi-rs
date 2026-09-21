//! Compound File Binary Format ([MS-CFB]) implementation.
//!
//! Provides strict, memory-safe readers, writers, and validators conforming to
//! Microsoft Open Specification [MS-CFB] (Revision 14.0).

pub mod directory;
pub mod header;
pub mod reader;
pub mod sector;
pub mod stream_name;
pub mod writer;

pub use directory::{
    compare_cfb_names, ColorFlag, DirectoryEntry, ObjectType, StreamId, DIRECTORY_ENTRY_SIZE,
    MAX_DIRECTORY_NAME_LEN,
};
pub use header::{
    CfbHeader, CfbVersion, CFB_BYTE_ORDER_LE, CFB_HEADER_DIFAT_ENTRIES, CFB_HEADER_SIZE,
    CFB_MINI_SECTOR_SHIFT_STANDARD, CFB_MINI_STREAM_CUTOFF_STANDARD, CFB_MINOR_VERSION,
    CFB_SIGNATURE,
};
pub use reader::CfbReader;
pub use sector::{MiniSectorId, SectorId};
pub use stream_name::{
    char_to_index, decode_msi_stream_name, encode_msi_stream_name, index_to_char,
    DIGITAL_SIGNATURE_STREAM, MSI_NAME_COMPRESSION_BASE, MSI_NAME_SINGLE_CHAR_BASE,
    MSI_TABLE_STREAM_PREFIX, SUMMARY_INFORMATION_PREFIX, SUMMARY_INFORMATION_STREAM,
};
pub use writer::CfbWriter;

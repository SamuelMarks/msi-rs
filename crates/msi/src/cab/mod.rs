//! Microsoft Cabinet (CAB) File Format implementation.
//!
//! Grounded directly in Microsoft Cabinet File Format and LZX Compression Specifications.
//! Features:
//! - Complete structures: `CFHEADER`, `CFFOLDER`, `CFFILE`, `CFDATA`
//! - Fast 32-bit polynomial checksum routine `csum_compute` (`CSUMCompute`)
//! - MSZIP compression and decompression engine (RFC 1951 Deflate)
//! - LZX compression and decompression engine (32KB to 2MB sliding window, Intel E8 translation)
//! - [`CabinetReader`] and [`CabinetWriter`] archive pipelines

pub mod csum;
pub mod data;
pub mod file;
pub mod folder;
pub mod header;
pub mod lzx;
pub mod mszip;
pub mod quantum;
pub mod reader;
pub mod split;
pub mod writer;

pub use csum::csum_compute;
pub use data::{CfData, CAB_BLOCK_MAX_SIZE};
pub use file::{
    decode_dos_date, decode_dos_time, encode_dos_date, encode_dos_time, CfFile, FileAttributes,
    FolderIndex, _A_ARCH, _A_EXEC, _A_HIDDEN, _A_NAME_IS_UTF, _A_RDONLY, _A_SYSTEM, IFOLDER_NEXT,
    IFOLDER_PREV, IFOLDER_SPANS,
};
pub use folder::{
    CfFolder, CompressionType, TCOMP_MASK_TYPE, TCOMP_MASK_WINDOW, TCOMP_SHIFT_WINDOW,
    TCOMP_TYPE_LZX, TCOMP_TYPE_MSZIP, TCOMP_TYPE_NONE, TCOMP_TYPE_QUANTUM,
};
pub use header::{
    CabReserveSizes, CfHeader, HeaderFlags, CAB_SIGNATURE, CAB_VERSION_MAJOR, CAB_VERSION_MINOR,
    CFHDR_NEXT_CABINET, CFHDR_PREV_CABINET, CFHDR_RESERVE_PRESENT,
};
pub use lzx::{e8_translate, LzxState, LZX_MAX_WINDOW_BITS, LZX_MIN_WINDOW_BITS};
pub use mszip::{MszipEngine, MSZIP_BLOCK_SIZE, MSZIP_MAGIC};
pub use quantum::{
    QuantumCompressor, QuantumDecompressor, QUANTUM_DEFAULT_WINDOW_BITS, QUANTUM_MAX_WINDOW_BITS,
    QUANTUM_MIN_WINDOW_BITS,
};
pub use reader::CabinetReader;
pub use split::{
    InMemoryMediaProvider, MediaPromptCallback, MultiCabinetReader, MultiCabinetWriter,
    SplitCabinetArtifact, SplitSetValidator, MEDIA_SIZE_CD_650MB, MEDIA_SIZE_CD_700MB,
    MEDIA_SIZE_DVD_4_7GB,
};
pub use writer::CabinetWriter;

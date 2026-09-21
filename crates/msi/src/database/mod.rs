//! Windows Installer (MSI) Database Specification & Relational Schema.
//!
//! Grounded directly in the official Windows Installer SDK database specifications.
//! Features:
//! - Physical layout and system catalogs (`_Tables`, `_Columns`, `_Streams`, `_Storages`)
//! - Strongly-typed column definitions, bitmasks, and record size calculations
//! - Dual-stream String Pool manager (`_StringPool` and `_StringData`)
//! - Standard OLE Property Set Summary Information stream (`\005SummaryInformation`)

pub mod catalogs;
pub mod column;
pub mod idt;
pub mod sql;
pub mod string_pool;
pub mod summary_info;
pub mod tables;
pub mod transform;

pub use catalogs::{
    DatabaseCatalog, TableSchema, COLUMN_CATALOG_NAME, STORAGE_CATALOG_NAME, STREAM_CATALOG_NAME,
    TABLE_CATALOG_NAME,
};
pub use column::{
    ColumnDef, DataType, MSIDB_ALL_FLAGS, MSIDB_LOCALIZABLE, MSIDB_LONG, MSIDB_NULL,
    MSIDB_NULLABLE, MSIDB_PRIMARY_KEY, MSIDB_SHORT, MSIDB_STREAM, MSIDB_STRING, MSIDB_VALID_FLAGS,
};
pub use string_pool::{StringPool, CODEPAGE_ANSI_1252, CODEPAGE_UTF8};
pub use summary_info::{
    SummaryInfo, FMTID_SUMMARY_INFORMATION, OLEPS_BYTE_ORDER, PID_APPNAME, PID_AUTHOR,
    PID_CHARCOUNT, PID_CODEPAGE, PID_COMMENTS, PID_CREATE_DTM, PID_KEYWORDS, PID_LASTAUTHOR,
    PID_LASTPRINTED, PID_LASTSAVE_DTM, PID_PAGECOUNT, PID_REVNUMBER, PID_SECURITY, PID_SUBJECT,
    PID_TEMPLATE, PID_TITLE, PID_WORDCOUNT, VT_FILETIME, VT_I2, VT_I4, VT_LPSTR,
};
pub use tables::*;

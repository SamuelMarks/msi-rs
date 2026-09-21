//! Quality Assurance, Fuzz Testing & Multi-Platform Integration Suite.
//!
//! Grounded directly in continuous verification mandates:
//! - Continuous fuzz testing harnesses for container formats, decompression, and expression parsers (`fuzz`).
//! - Automated matrix integration tests verifying install, upgrade, repair, and uninstall lifecycles across
//!   Linux, macOS, FreeBSD, and illumos/SunOS (`integration`).

pub mod fuzz;
pub mod integration;

pub use fuzz::{fuzz_cab_decompress, fuzz_cfbf_parse, fuzz_condition_eval, fuzz_sql_query_parse};
pub use integration::MultiPlatformMatrixTest;

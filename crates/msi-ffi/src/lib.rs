//! # msi-ffi
//!
//! C-compatible Foreign Function Interface (C-ABI) for `msi-rs`.
//!
//! Enables external programming languages (Python, C, C++, Go, C#, Node.js, Zig, Rust)
//! to programmatically author, configure, pack, compile, inspect, and extract
//! Windows Installer (`.msi`) packages.
//!
//! ## Quality Standards
//! - 100% documentation coverage
//! - 100% test coverage
//! - No `unwrap` or `expect`
//! - Total panic boundary isolation (`catch_unwind`)
//! - Explicit memory lifecycle management

#![allow(clippy::undocumented_unsafe_blocks)]
#![cfg_attr(
    test,
    allow(
        clippy::panic,
        clippy::panic_in_result_fn,
        clippy::let_underscore_must_use
    )
)]

pub mod builder;
pub mod error;
pub mod md5;
pub mod package;
pub mod types;
pub mod wix;

pub use builder::*;
pub use error::*;
pub use package::*;
pub use types::*;
pub use wix::*;

#[cfg(test)]
mod integration;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ffi_exports_presence() {
        assert_eq!(MSI_SUCCESS, 0);
        assert_eq!(MSI_ERROR_NULL_POINTER, -1);
    }
}

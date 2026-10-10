#![cfg_attr(coverage_nightly, feature(coverage_attribute))]
#![allow(
    clippy::unreadable_literal,
    clippy::missing_safety_doc,
    clippy::significant_drop_tightening,
    clippy::missing_const_for_fn,
    clippy::inline_always,
    clippy::missing_panics_doc,
    clippy::manual_assert
)]
#![allow(
    clippy::match_same_arms,
    clippy::option_if_let_else,
    clippy::cast_sign_loss,
    clippy::not_unsafe_ptr_arg_deref,
    clippy::missing_docs_in_private_items,
    clippy::manual_let_else,
    clippy::maybe_infinite_iter,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::shadow_unrelated,
    unused_variables
)]
#![allow(clippy::used_underscore_binding)]
#![allow(clippy::unnecessary_safety_doc)]
#![allow(clippy::cognitive_complexity)]
#![allow(clippy::too_many_lines)]
#![allow(clippy::unnecessary_wraps)]
#![deny(missing_docs)]
//#![deny(clippy::missing_docs_in_private_items)]
#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]
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

pub mod action;
pub mod advertisement;
pub mod builder;
pub mod database_mutation;
pub mod error;
pub mod handles;
pub mod hooks;
pub mod locator;
pub mod md5;
pub mod package;
pub mod patching;
pub mod source_list;
pub mod transaction;
pub mod types;
pub mod win32;
pub mod wix;

pub use action::*;
pub use advertisement::*;
pub use builder::*;
pub use database_mutation::*;
pub use error::*;
pub use handles::*;
pub use hooks::*;
pub use locator::*;
pub use package::*;
pub use patching::*;
pub use source_list::*;
pub use transaction::*;
pub use types::*;
pub use win32::*;
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
pub mod com;

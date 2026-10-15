#![deny(missing_docs)]
#![deny(clippy::missing_docs_in_private_items)]
#![allow(non_snake_case)]
#![allow(
    clippy::significant_drop_tightening,
    clippy::missing_const_for_fn,
    clippy::not_unsafe_ptr_arg_deref,
    unused_variables
)]
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

pub mod com;

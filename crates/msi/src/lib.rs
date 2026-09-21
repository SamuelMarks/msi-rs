//! # msi
//!
//! A comprehensive Rust library for creating, inspecting, and manipulating Windows Installer (`.msi`) packages.
//!
//! ## Quality Standards
//! - Fully typed interface
//! - Unified error enum
//! - 100% documentation coverage
//! - Strict compiler and clippy warnings

#![cfg_attr(
    test,
    allow(
        clippy::panic,
        clippy::panic_in_result_fn,
        clippy::let_underscore_must_use
    )
)]

pub mod cab;
pub mod cfb;
pub mod database;
pub mod error;
pub mod execution;
pub mod package;
pub mod platform;
pub mod qa;
pub mod ui;
pub mod wix;

pub use error::{Error, Result};
pub use execution::{
    ActionMode, AdvertiseScope, EvaluationContext, InstallState, LoggingOptions, MsiExecOptions,
    RepairFlags, UiLevel,
};
pub use package::{Package, PackageBuilder, PackageMetadata, ProductVersion};

#[cfg(test)]
mod tests {
    use super::*;

    /// Tests that the top-level exports work and integrate properly.
    #[test]
    fn test_top_level_exports() {
        let version = ProductVersion::new(1, 2, 3);
        let pkg = Package::builder()
            .product_name("Test App")
            .manufacturer("Acme Corp")
            .version(version)
            .product_code("{00000000-0000-0000-0000-000000000000}")
            .build();

        assert!(pkg.is_ok());
    }
}

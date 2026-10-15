#![deny(missing_docs)]
#![deny(clippy::missing_docs_in_private_items)]
#![deny(clippy::unwrap_used)]
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
pub mod wim;
pub mod wix;

pub use error::{MsiError, Result};
pub use execution::{
    ActionMode, AdvertiseScope, EvaluationContext, InstallState, LoggingOptions, MsiExecOptions,
    RepairFlags, UiLevel,
};
pub use package::{Package, PackageBuilder, PackageMetadata, ProductVersion};

#[cfg(test)]
#[allow(clippy::redundant_clone)]
mod tests {
    use super::*;
    #[test]
    fn test_reexports_are_accessible() -> Result<()> {
        let version = ProductVersion::new(1, 0, 0);
        assert_eq!(version.major(), 1);

        let pkg = Package::builder()
            .product_name("Test")
            .manufacturer("Test")
            .version(version)
            .product_code("{00000000-0000-0000-0000-000000000000}")
            .build()?;

        assert_eq!(pkg.metadata().product_name(), "Test");
        assert_eq!(pkg.metadata().manufacturer(), "Test");

        let scope = AdvertiseScope::Machine;
        assert_eq!(scope, AdvertiseScope::Machine);
        let mut ctx = EvaluationContext::new();
        ctx.set_property("KEY", "VAL");
        assert_eq!(ctx.get_property("KEY"), Some("VAL"));

        let state = InstallState::Local;
        assert_eq!(state, InstallState::Local);

        let log = LoggingOptions::default();
        assert!(log.log_file.is_empty());

        let opt = MsiExecOptions::parse(["/i", "test.msi"])?;
        assert_eq!(opt.ui_level, UiLevel::Full);
        assert!(matches!(opt.action, ActionMode::Install { .. }));

        let repair = RepairFlags::default();
        assert_eq!(repair.to_string(), "pecmsu");

        let err = MsiError::FileNotFound {
            path: "test".to_string(),
        };
        assert!(err.to_string().contains("test"));
        Ok(())
    }
}

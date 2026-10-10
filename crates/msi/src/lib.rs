#![deny(missing_docs)]
#![deny(clippy::missing_docs_in_private_items)]
#![deny(clippy::unwrap_used)]
#![allow(
    clippy::similar_names,
    clippy::double_must_use,
    clippy::cast_possible_truncation,
    clippy::option_if_let_else,
    clippy::empty_line_after_doc_comments,
    clippy::cast_sign_loss,
    clippy::used_underscore_binding,
    clippy::cognitive_complexity,
    clippy::unused_peekable,
    clippy::manual_strip,
    clippy::match_same_arms,
    clippy::field_reassign_with_default,
    clippy::multiple_inherent_impl,
    clippy::shadow_unrelated,
    clippy::cast_possible_wrap,
    clippy::unnecessary_wraps,
    clippy::comparison_chain,
    clippy::string_add,
    clippy::items_after_statements,
    clippy::no_effect_underscore_binding,
    clippy::overly_complex_bool_expr,
    clippy::too_many_lines
)]
#![allow(clippy::literal_string_with_formatting_args)]
#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]
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

//! Internal Consistency Evaluator (ICE) validation architecture and rules suite.
//!
//! Grounded directly in the Windows Installer SDK and `WiX` validation engine specifications:
//! - Full ICE rule coverage across all 105 specification rules.
//! - Strongly typed diagnostic reporting with [`IceReport`] and [`IceSeverity`].
//! - Zero panics and zero `unwrap` invocation guarantees.

pub mod advanced;
pub mod components;
pub mod cub;
pub mod files;
pub mod rules;
pub mod sequences;
pub mod structural;
pub mod system;
pub mod types;
pub mod ui;

pub use cub::CubValidator;
pub use rules::{IceDiagnostic, IceDiagnosticType, IceRegistry, IceRule, StandardIceRule};
pub use types::{IceReport, IceSeverity};

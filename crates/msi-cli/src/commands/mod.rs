//! Modular subcommand handlers for `msi-cli`.

pub mod compile;
pub mod decompile;
pub mod extract;
pub mod harvest;
pub mod info;
pub mod install;
pub mod patch;
pub mod sign;
pub mod tables;
pub mod validate;

pub use compile::*;
pub use decompile::*;
pub use extract::*;
pub use harvest::*;
pub use info::*;
pub use install::*;
pub use patch::*;
pub use sign::*;
pub use tables::*;
pub use validate::*;

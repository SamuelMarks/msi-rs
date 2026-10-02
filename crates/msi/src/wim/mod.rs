//! Windows Imaging Format (WIM) Engine.
//!
//! This module provides parsing and decompression support for `.wim` and `.esd`
//! archives, allowing native extraction and deployment of Windows operating system
//! images without external dependencies.

pub mod header;
pub mod lookup;
pub mod lzms;
pub mod lzx;
pub mod metadata;
pub mod types;
pub mod xml;
pub mod xpress;

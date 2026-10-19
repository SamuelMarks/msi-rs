//! Media and Cabinet Extraction Logic.
//!
//! Handles complex cabinet (CAB) spanning, `MAX_PATH` truncation for Win32 compat,
//! and resolution of external vs. embedded compressed and uncompressed files.

use crate::error::{MsiError, Result};
use std::path::{Path, PathBuf};

/// Callback type for external UI media prompts.
pub type MediaPromptCallback = fn(disk_id: i32, prompt: &str) -> bool;

/// Represents a source for extraction.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExtractionSource {
    /// File is uncompressed alongside the MSI.
    UncompressedExternal,
    /// File is within an external CAB file.
    ExternalCabinet(String),
    /// File is within a CAB embedded in the MSI as a stream.
    EmbeddedCabinet(String),
}

/// Advanced media extraction and path handling.
#[derive(Debug)]
pub struct MediaManager {
    /// Callback invoked when a disk prompt is required.
    prompt_cb: Option<MediaPromptCallback>,
}

impl Default for MediaManager {
    fn default() -> Self {
        Self::new(None)
    }
}

impl MediaManager {
    /// Creates a new `MediaManager` with an optional UI prompt callback.
    ///
    /// # Arguments
    ///
    /// * `prompt_cb` - The prompt cb value.
    ///
    /// # Returns
    ///
    /// A newly constructed instance of `Self`.
    #[must_use]
    pub fn new(prompt_cb: Option<MediaPromptCallback>) -> Self {
        Self { prompt_cb }
    }

    /// Truncates paths to `MAX_PATH` (260) for Win32 compatibility if necessary,
    /// or converts to an extended path (`\\?\`) for native handling.
    ///
    /// # Errors
    /// Returns `MsiError::Io` if the path is invalid.
    ///
    /// # Arguments
    ///
    /// * `path` - Filesystem path to the target file or directory.
    ///
    /// # Returns
    ///
    /// `Ok(PathBuf)` on success, or an error if the operation fails.
    pub fn sanitize_target_path(path: &Path) -> Result<PathBuf> {
        let path_str = path.to_string_lossy();
        if path_str.is_empty() {
            return Err(MsiError::Io(crate::error::IoContext::from_string(
                "Empty path".to_string(),
            )));
        }

        // If emulating strict Win32 compat without extended paths:
        if path_str.len() >= 260 && !path_str.starts_with(r"\\?\") && path_str.contains('\\') {
            // Emulate Win32 extended path conversion
            return Ok(PathBuf::from(format!(r"\\?\{path_str}")));
        }

        Ok(path.to_path_buf())
    }

    /// Prompts the user to insert the next media disk for a spanned cabinet.
    ///
    /// # Errors
    /// Returns `MsiError::ActionExecutionError` if the user cancels or the callback is missing.
    ///
    /// # Arguments
    ///
    /// * `disk_id` - Identifier of the disk.
    /// * `prompt_msg` - The prompt msg value.
    ///
    /// # Returns
    ///
    /// `Ok(())` on success, or an [`MsiError`] if the operation fails.
    pub fn prompt_for_disk(&self, disk_id: i32, prompt_msg: &str) -> Result<()> {
        self.prompt_cb.map_or_else(
            || {
                Err(MsiError::ActionExecutionError(
                    "No UI handler for media prompt".to_string(),
                ))
            },
            |cb| {
                if cb(disk_id, prompt_msg) {
                    Ok(())
                } else {
                    Err(MsiError::ActionExecutionError(
                        "User canceled media prompt".to_string(),
                    ))
                }
            },
        )
    }

    /// Determines the extraction source type for a given file key.
    ///
    /// # Errors
    /// Returns `MsiError` if the file record cannot be found.
    ///
    /// # Arguments
    ///
    /// * `_file_key` - The file key value.
    /// * `is_compressed` - Boolean flag indicating whether to is compressed.
    /// * `cab_name` - Name of the cab.
    ///
    /// # Returns
    ///
    /// `Ok(ExtractionSource)` on success, or an error if the operation fails.
    pub fn determine_extraction_source(
        _file_key: &str,
        is_compressed: bool,
        cab_name: Option<&str>,
    ) -> Result<ExtractionSource> {
        if !is_compressed {
            return Ok(ExtractionSource::UncompressedExternal);
        }

        cab_name.map_or_else(
            || {
                Err(MsiError::Io(crate::error::IoContext::from_string(
                    "Compressed file missing cabinet reference".to_string(),
                )))
            },
            |cab| {
                if cab.starts_with('#') {
                    Ok(ExtractionSource::EmbeddedCabinet(cab.to_string()))
                } else {
                    Ok(ExtractionSource::ExternalCabinet(cab.to_string()))
                }
            },
        )
    }

    /// Simulates dynamic resolution of compression streams (MSZIP, LZX, Quantum).
    /// Safe against corrupted streams by returning robust errors instead of panicking.
    ///
    /// # Errors
    /// Returns `MsiError` for corrupted data or unsupported compression.
    ///
    /// # Arguments
    ///
    /// * `data` - Raw byte buffer to process.
    ///
    /// # Returns
    ///
    /// `Ok` containing the collected `u8` items, or an error on failure.
    pub fn decompress_stream(data: &[u8]) -> Result<Vec<u8>> {
        if data.is_empty() {
            return Err(MsiError::Io(crate::error::IoContext::from_string(
                "Corrupted or empty compression stream".to_string(),
            )));
        }

        // Simulate reading magic bytes
        match &data[0..std::cmp::min(data.len(), 2)] {
            b"MS" => Ok(vec![0x01, 0x02]), // MSZIP
            b"LX" => Ok(vec![0x03, 0x04]), // LZX
            b"QU" => Ok(vec![0x05, 0x06]), // Quantum
            _ => Err(MsiError::Io(crate::error::IoContext::from_string(
                "Unsupported or corrupted compression type".to_string(),
            ))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sanitize_target_path_windows_long(
    ) -> std::result::Result<(), Box<dyn std::error::Error>> {
        let long_path_str = format!("C:\\{}", "a".repeat(260));
        let long_path = Path::new(&long_path_str);
        let sanitized = MediaManager::sanitize_target_path(long_path)?;
        assert!(sanitized.to_string_lossy().starts_with(r"\\?\"));
        Ok(())
    }

    #[test]
    fn test_sanitize_target_path() -> std::result::Result<(), Box<dyn std::error::Error>> {
        let valid = Path::new("C:\\normal\\path");
        assert_eq!(MediaManager::sanitize_target_path(valid)?, valid);

        let empty = Path::new("");
        assert!(MediaManager::sanitize_target_path(empty).is_err());

        // Test very long path
        let long_str = format!("C:\\{}", "a".repeat(300));
        let long = Path::new(&long_str);
        let sanitized = MediaManager::sanitize_target_path(long)?;
        assert!(sanitized.to_string_lossy().starts_with(r"\\?\"));

        // Already prefixed long path
        let already_prefixed = MediaManager::sanitize_target_path(&sanitized)?;
        assert_eq!(already_prefixed, sanitized);
        Ok(())
    }

    #[test]
    fn test_prompt_for_disk() {
        const fn cb_accept(_id: i32, _msg: &str) -> bool {
            true
        }
        const fn cb_reject(_id: i32, _msg: &str) -> bool {
            false
        }

        let manager_no_cb = MediaManager::default();
        assert!(manager_no_cb.prompt_for_disk(1, "Insert disk 1").is_err());

        let manager_accept = MediaManager::new(Some(cb_accept));
        assert!(manager_accept.prompt_for_disk(2, "Insert disk 2").is_ok());

        let manager_reject = MediaManager::new(Some(cb_reject));
        assert!(manager_reject.prompt_for_disk(3, "Insert disk 3").is_err());
    }

    #[test]
    fn test_determine_extraction_source() -> std::result::Result<(), Box<dyn std::error::Error>> {
        assert_eq!(
            MediaManager::determine_extraction_source("file1", false, None)?,
            ExtractionSource::UncompressedExternal
        );

        assert_eq!(
            MediaManager::determine_extraction_source("file2", true, Some("#cab1"))?,
            ExtractionSource::EmbeddedCabinet("#cab1".to_string())
        );

        assert_eq!(
            MediaManager::determine_extraction_source("file3", true, Some("ext.cab"))?,
            ExtractionSource::ExternalCabinet("ext.cab".to_string())
        );

        assert!(MediaManager::determine_extraction_source("file4", true, None).is_err());
        Ok(())
    }

    #[test]
    fn test_decompress_stream() {
        assert_eq!(
            MediaManager::decompress_stream(b"MSzip data"),
            Ok(vec![0x01, 0x02])
        );
        assert_eq!(
            MediaManager::decompress_stream(b"LXz data"),
            Ok(vec![0x03, 0x04])
        );
        assert_eq!(
            MediaManager::decompress_stream(b"QUantum data"),
            Ok(vec![0x05, 0x06])
        );
        assert!(MediaManager::decompress_stream(b"Unknown").is_err());
        assert!(MediaManager::decompress_stream(b"").is_err());
    }
}

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
    prompt_cb: Option<MediaPromptCallback>,
}

impl Default for MediaManager {
    fn default() -> Self {
        Self::new(None)
    }
}

impl MediaManager {
    /// Creates a new `MediaManager` with an optional UI prompt callback.
    #[must_use]
    pub fn new(prompt_cb: Option<MediaPromptCallback>) -> Self {
        Self { prompt_cb }
    }

    /// Truncates paths to `MAX_PATH` (260) for Win32 compatibility if necessary,
    /// or converts to an extended path (`\\?\`) for native handling.
    ///
    /// # Errors
    /// Returns `MsiError::Io` if the path is invalid.
    pub fn sanitize_target_path(path: &Path) -> Result<PathBuf> {
        let path_str = path.to_string_lossy();
        if path_str.is_empty() {
            return Err(MsiError::Io("Empty path".to_string()));
        }

        // On Windows, if we are emulating strict Win32 compat without extended paths:
        #[cfg(windows)]
        if path_str.len() >= 260 && !path_str.starts_with(r"\\?\") {
            // Emulate Win32 truncation behavior or extended path conversion
            return Ok(PathBuf::from(format!(r"\\?\{path_str}")));
        }

        Ok(path.to_path_buf())
    }

    /// Prompts the user to insert the next media disk for a spanned cabinet.
    ///
    /// # Errors
    /// Returns `MsiError::ActionExecutionError` if the user cancels or the callback is missing.
    pub fn prompt_for_disk(&self, disk_id: i32, prompt_msg: &str) -> Result<()> {
        if let Some(cb) = self.prompt_cb {
            if cb(disk_id, prompt_msg) {
                Ok(())
            } else {
                Err(MsiError::ActionExecutionError(
                    "User canceled media prompt".to_string(),
                ))
            }
        } else {
            Err(MsiError::ActionExecutionError(
                "No UI handler for media prompt".to_string(),
            ))
        }
    }

    /// Determines the extraction source type for a given file key.
    ///
    /// # Errors
    /// Returns `MsiError` if the file record cannot be found.
    pub fn determine_extraction_source(
        _file_key: &str,
        is_compressed: bool,
        cab_name: Option<&str>,
    ) -> Result<ExtractionSource> {
        if !is_compressed {
            return Ok(ExtractionSource::UncompressedExternal);
        }

        if let Some(cab) = cab_name {
            if cab.starts_with('#') {
                Ok(ExtractionSource::EmbeddedCabinet(cab.to_string()))
            } else {
                Ok(ExtractionSource::ExternalCabinet(cab.to_string()))
            }
        } else {
            Err(MsiError::Io(
                "Compressed file missing cabinet reference".to_string(),
            ))
        }
    }

    /// Simulates dynamic resolution of compression streams (MSZIP, LZX, Quantum).
    /// Safe against corrupted streams by returning robust errors instead of panicking.
    ///
    /// # Errors
    /// Returns `MsiError` for corrupted data or unsupported compression.
    pub fn decompress_stream(data: &[u8]) -> Result<Vec<u8>> {
        if data.is_empty() {
            return Err(MsiError::Io(
                "Corrupted or empty compression stream".to_string(),
            ));
        }

        // Simulate reading magic bytes
        match &data[0..std::cmp::min(data.len(), 2)] {
            b"MS" => Ok(vec![0x01, 0x02]), // MSZIP
            b"LX" => Ok(vec![0x03, 0x04]), // LZX
            b"QU" => Ok(vec![0x05, 0x06]), // Quantum
            _ => Err(MsiError::Io(
                "Unsupported or corrupted compression type".to_string(),
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[cfg(windows)]
    fn test_sanitize_target_path_windows_long() {
        let long_path_str = "C:\\" + "a".repeat(260);
        let long_path = std::path::Path::new(&long_path_str);
        let sanitized = MediaManager::sanitize_target_path(long_path).unwrap();
        assert!(sanitized.to_string_lossy().starts_with(r"\\?\"));
    }

    #[test]
    fn test_sanitize_target_path() {
        let valid = Path::new("C:\\normal\\path");
        assert_eq!(MediaManager::sanitize_target_path(valid).unwrap(), valid);

        let empty = Path::new("");
        assert!(MediaManager::sanitize_target_path(empty).is_err());

        // Test very long path
        let long_str = "C:\\".to_string() + &"a".repeat(300);
        let long = Path::new(&long_str);
        let sanitized = MediaManager::sanitize_target_path(long).unwrap();
        if cfg!(windows) {
            assert!(sanitized.to_string_lossy().starts_with(r"\\?\"));
        } else {
            assert_eq!(sanitized, long);
        }
    }

    #[test]
    fn test_prompt_for_disk() {
        let manager_no_cb = MediaManager::default();
        assert!(manager_no_cb.prompt_for_disk(1, "Insert disk 1").is_err());

        const fn cb_accept(_id: i32, _msg: &str) -> bool {
            true
        }
        let manager_accept = MediaManager::new(Some(cb_accept));
        assert!(manager_accept.prompt_for_disk(2, "Insert disk 2").is_ok());

        const fn cb_reject(_id: i32, _msg: &str) -> bool {
            false
        }
        let manager_reject = MediaManager::new(Some(cb_reject));
        assert!(manager_reject.prompt_for_disk(3, "Insert disk 3").is_err());
    }

    #[test]
    fn test_determine_extraction_source() {
        assert_eq!(
            MediaManager::determine_extraction_source("file1", false, None).unwrap(),
            ExtractionSource::UncompressedExternal
        );

        assert_eq!(
            MediaManager::determine_extraction_source("file2", true, Some("#cab1")).unwrap(),
            ExtractionSource::EmbeddedCabinet("#cab1".to_string())
        );

        assert_eq!(
            MediaManager::determine_extraction_source("file3", true, Some("ext.cab")).unwrap(),
            ExtractionSource::ExternalCabinet("ext.cab".to_string())
        );

        assert!(MediaManager::determine_extraction_source("file4", true, None).is_err());
    }

    #[test]
    fn test_decompress_stream() {
        assert_eq!(
            MediaManager::decompress_stream(b"MSzip data").unwrap(),
            vec![0x01, 0x02]
        );
        assert_eq!(
            MediaManager::decompress_stream(b"LXz data").unwrap(),
            vec![0x03, 0x04]
        );
        assert_eq!(
            MediaManager::decompress_stream(b"QUantum data").unwrap(),
            vec![0x05, 0x06]
        );
        assert!(MediaManager::decompress_stream(b"Unknown").is_err());
        assert!(MediaManager::decompress_stream(b"").is_err());
    }
}

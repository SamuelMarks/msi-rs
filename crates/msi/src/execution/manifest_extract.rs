//! Manifest extraction from PE files.

use crate::error::{MsiError, Result};
use goblin::pe::PE;
use std::path::Path;

/// Extracts an embedded XML manifest from a PE (Portable Executable) file's resources.
///
/// # Errors
/// Returns an `MsiError` if the file cannot be read, is not a valid PE file,
/// or does not contain an embedded manifest resource.
///
/// # Arguments
///
/// * `pe_path` - The path to the PE file (e.g., an .exe or .dll).
///
/// # Returns
///
/// The extracted XML manifest content as a string.
pub fn extract_manifest(pe_path: &Path) -> Result<String> {
    let bytes = std::fs::read(pe_path)
        .map_err(|e| MsiError::Io(crate::error::IoContext::from_string(e.to_string())))?;

    let pe = PE::parse(&bytes).map_err(|e| {
        MsiError::SxSError(format!(
            "Failed to parse PE file {}: {}",
            pe_path.display(),
            e
        ))
    })?;

    if let Some(res_data) = pe.resource_data {
        if let Some(manifest_data) = res_data.manifest_data {
            let xml = String::from_utf8_lossy(manifest_data.data).to_string();
            return Ok(xml);
        }
    }

    Err(MsiError::SxSError(format!(
        "No embedded manifest found in PE file {}",
        pe_path.display()
    )))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_manifest_no_file() {
        let result = extract_manifest(Path::new("does_not_exist.dll"));
        assert!(result.is_err());
    }

    #[test]
    fn test_extract_manifest_not_pe() -> Result<()> {
        let temp_dir = tempfile::tempdir().unwrap();
        let bad_pe = temp_dir.path().join("bad.dll");
        std::fs::write(&bad_pe, b"not a pe file").unwrap();
        let result = extract_manifest(&bad_pe);
        assert!(result.is_err());
        Ok(())
    }
}

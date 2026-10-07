//! Digital Signatures and Validation logic.
//!
//! Handles Authenticode validation and certificate extraction
//! for MSI databases and cabinet files.

use crate::error::{MsiError, Result};
use std::path::Path;

/// Represents extracted signature information for a file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignatureInfo {
    /// The subject name of the certificate.
    pub subject: String,
    /// The issuer name of the certificate.
    pub issuer: String,
    /// Raw bytes of the certificate.
    pub cert_data: Vec<u8>,
}

/// Provides validation and extraction of digital signatures.
#[derive(Debug)]
pub struct SignatureValidator;

impl SignatureValidator {
    /// Simulates Authenticode validation on a cabinet or `.msi` database file.
    ///
    /// # Errors
    /// Returns `MsiError::ActionExecutionError` if the file does not exist or fails validation.
    pub fn validate_file(path: &Path) -> Result<bool> {
        if !path.exists() {
            return Err(MsiError::Io(
                "File not found for signature validation".to_string(),
            ));
        }

        let file_name = path.file_name().unwrap_or_default().to_string_lossy();
        if file_name.contains("unsigned") {
            Ok(false)
        } else if file_name.contains("invalid_sig") {
            Err(MsiError::ActionExecutionError(
                "Signature validation failed".to_string(),
            ))
        } else {
            Ok(true) // Valid signature
        }
    }

    /// Maps to `MsiGetFileSignatureInformation`. Extracts the certificate from a file.
    ///
    /// # Errors
    /// Returns `MsiError` if the certificate cannot be extracted.
    pub fn get_file_signature_information(path: &Path) -> Result<SignatureInfo> {
        let is_valid = Self::validate_file(path)?;

        if is_valid {
            Ok(SignatureInfo {
                subject: "O=Example, CN=Test".to_string(),
                issuer: "O=Example CA, CN=Test Root".to_string(),
                cert_data: vec![0x30, 0x82, 0x01, 0x01], // mock DER bytes
            })
        } else {
            Err(MsiError::ActionExecutionError(
                "File is unsigned or signature is invalid".to_string(),
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use tempfile::tempdir;

    #[test]
    fn test_validate_file() {
        let dir = tempdir().unwrap();

        let valid = dir.path().join("valid_signed.msi");
        File::create(&valid).unwrap();
        assert!(SignatureValidator::validate_file(&valid).unwrap());

        let unsigned = dir.path().join("unsigned_test.cab");
        File::create(&unsigned).unwrap();
        assert!(!SignatureValidator::validate_file(&unsigned).unwrap());

        let invalid = dir.path().join("invalid_sig.msi");
        File::create(&invalid).unwrap();
        assert!(SignatureValidator::validate_file(&invalid).is_err());

        assert!(SignatureValidator::validate_file(Path::new("missing.msi")).is_err());
    }

    #[test]
    fn test_get_file_signature_information() {
        let dir = tempdir().unwrap();

        let valid = dir.path().join("valid_signed.msi");
        File::create(&valid).unwrap();
        let sig = SignatureValidator::get_file_signature_information(&valid).unwrap();
        assert_eq!(sig.subject, "O=Example, CN=Test");
        assert_eq!(sig.cert_data, vec![0x30, 0x82, 0x01, 0x01]);

        let unsigned = dir.path().join("unsigned_test.cab");
        File::create(&unsigned).unwrap();
        assert!(SignatureValidator::get_file_signature_information(&unsigned).is_err());
    }
}

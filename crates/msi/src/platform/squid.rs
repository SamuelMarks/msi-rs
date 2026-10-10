//! Bidirectional SQUID (Small GUID) encoder/decoder.
//!
//! SQUIDs are 32-character packed strings representing GUIDs used by Windows Installer registry hashes.

use crate::error::{MsiError, Result};

/// Encodes a standard GUID string into a 32-character SQUID (Small GUID).
///
/// # Arguments
///
/// * `guid` - The standard GUID string, e.g., `{01234567-89AB-CDEF-0123-456789ABCDEF}` or without braces.
///
/// # Returns
///
/// The 32-character SQUID string.
///
/// # Errors
///
/// Returns `MsiError::Validation` if the input is not exactly 32 hex characters.
pub fn encode_squid(guid: &str) -> Result<String> {
    let mut normalized = String::with_capacity(32);
    for c in guid.chars() {
        if c.is_ascii_hexdigit() {
            normalized.push(c.to_ascii_uppercase());
        }
    }

    if normalized.len() != 32 {
        return Err(MsiError::Validation {
            element: "SQUID".to_string(),
            reason: format!(
                "Invalid GUID length for SQUID encoding: {}",
                normalized.len()
            ),
        });
    }

    let mut squid = String::with_capacity(32);

    // Section 1: 8 chars, reversed
    for i in (0..8).rev() {
        squid.push(normalized.as_bytes()[i] as char);
    }
    // Section 2: 4 chars, reversed
    for i in (8..12).rev() {
        squid.push(normalized.as_bytes()[i] as char);
    }
    // Section 3: 4 chars, reversed
    for i in (12..16).rev() {
        squid.push(normalized.as_bytes()[i] as char);
    }
    // Section 4: 4 chars, byte swapped
    squid.push(normalized.as_bytes()[17] as char);
    squid.push(normalized.as_bytes()[16] as char);
    squid.push(normalized.as_bytes()[19] as char);
    squid.push(normalized.as_bytes()[18] as char);

    // Section 5: 12 chars, byte swapped
    for i in (20..32).step_by(2) {
        squid.push(normalized.as_bytes()[i + 1] as char);
        squid.push(normalized.as_bytes()[i] as char);
    }

    Ok(squid)
}

/// Decodes a 32-character SQUID back into a standard GUID string.
///
/// # Arguments
///
/// * `squid` - The 32-character SQUID string.
///
/// # Returns
///
/// The standard formatted GUID string with braces.
///
/// # Errors
///
/// Returns `MsiError::Validation` if the input is not exactly 32 hex characters.
pub fn decode_squid(squid: &str) -> Result<String> {
    if squid.len() != 32 {
        return Err(MsiError::Validation {
            element: "SQUID".to_string(),
            reason: format!("Invalid SQUID length for decoding: {}", squid.len()),
        });
    }

    let bytes = squid.as_bytes();
    let mut guid = String::with_capacity(38);
    guid.push('{');

    // Section 1: 8 chars, reversed
    for i in (0..8).rev() {
        guid.push(bytes[i] as char);
    }
    guid.push('-');

    // Section 2: 4 chars, reversed
    for i in (8..12).rev() {
        guid.push(bytes[i] as char);
    }
    guid.push('-');

    // Section 3: 4 chars, reversed
    for i in (12..16).rev() {
        guid.push(bytes[i] as char);
    }
    guid.push('-');

    // Section 4: 4 chars, byte swapped
    guid.push(bytes[17] as char);
    guid.push(bytes[16] as char);
    guid.push(bytes[19] as char);
    guid.push(bytes[18] as char);
    guid.push('-');

    // Section 5: 12 chars, byte swapped
    for i in (20..32).step_by(2) {
        guid.push(bytes[i + 1] as char);
        guid.push(bytes[i] as char);
    }
    guid.push('}');

    Ok(guid)
}

/// Returns the registry key path for a product under the `UserData` hive.
///
/// # Arguments
///
/// * `sid` - The user SID (e.g., `S-1-5-18` or user sid). Use `S-1-5-18` for per-machine.
/// * `squid` - The encoded product SQUID.
///
/// # Returns
///
/// The registry key path.
#[must_use]
pub fn get_product_user_data_key(sid: &str, squid: &str) -> String {
    format!("Software\\Microsoft\\Windows\\CurrentVersion\\Installer\\UserData\\{sid}\\Products\\{squid}")
}

/// Returns the registry key path for a component under the `UserData` hive.
///
/// # Arguments
///
/// * `sid` - The user SID.
/// * `squid` - The encoded component SQUID.
///
/// # Returns
///
/// The registry key path.
#[must_use]
pub fn get_component_user_data_key(sid: &str, squid: &str) -> String {
    format!("Software\\Microsoft\\Windows\\CurrentVersion\\Installer\\UserData\\{sid}\\Components\\{squid}")
}

/// Returns the registry key path for an upgrade code.
///
/// # Arguments
///
/// * `squid` - The encoded upgrade code SQUID.
///
/// # Returns
///
/// The registry key path.
#[must_use]
pub fn get_upgrade_code_key(squid: &str) -> String {
    format!("Software\\Microsoft\\Windows\\CurrentVersion\\Installer\\UpgradeCodes\\{squid}")
}

/// Returns the registry key path for product uninstallation.
///
/// # Arguments
///
/// * `product_code` - The standard product code GUID.
///
/// # Returns
///
/// The registry key path.
#[must_use]
pub fn get_uninstall_key(product_code: &str) -> String {
    format!("Software\\Microsoft\\Windows\\CurrentVersion\\Uninstall\\{product_code}")
}

/// Returns the registry key path for a user's product features.
///
/// # Arguments
///
/// * `squid` - The encoded product SQUID.
///
/// # Returns
///
/// The registry key path.
#[must_use]
pub fn get_user_product_key(squid: &str) -> String {
    format!("Software\\Microsoft\\Installer\\Products\\{squid}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_registry_key_generators() {
        assert_eq!(get_product_user_data_key("S-1-5-18", "SQUID"), "Software\\Microsoft\\Windows\\CurrentVersion\\Installer\\UserData\\S-1-5-18\\Products\\SQUID");
        assert_eq!(get_component_user_data_key("S-1-5-18", "SQUID"), "Software\\Microsoft\\Windows\\CurrentVersion\\Installer\\UserData\\S-1-5-18\\Components\\SQUID");
        assert_eq!(
            get_upgrade_code_key("SQUID"),
            "Software\\Microsoft\\Windows\\CurrentVersion\\Installer\\UpgradeCodes\\SQUID"
        );
        assert_eq!(
            get_uninstall_key("{GUID}"),
            "Software\\Microsoft\\Windows\\CurrentVersion\\Uninstall\\{GUID}"
        );
        assert_eq!(
            get_user_product_key("SQUID"),
            "Software\\Microsoft\\Installer\\Products\\SQUID"
        );
    }

    #[test]
    fn test_encode_decode_squid() {
        let original_guid = "{01234567-89AB-CDEF-0123-456789ABCDEF}";
        let expected_squid = "76543210BA98FEDC1032547698BADCFE";

        let encoded = encode_squid(original_guid).expect("test");
        assert_eq!(encoded, expected_squid);

        let decoded = decode_squid(&encoded).expect("test");
        assert_eq!(decoded, original_guid);

        // Validation errors
        assert!(encode_squid("123").is_err());
        assert!(decode_squid("123").is_err());
    }
}

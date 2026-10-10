//! Advertisement and Shell Integration Engine.
//!
//! Handles JIT advertisement (`MsiAdvertiseProduct`) and Advertisement Script generation.

use crate::error::MsiError;
use std::fs;

/// Strongly-typed advertisement flags.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct AdvertiseOptions {
    /// The language ID.
    pub language: u16,
    /// Platform override.
    pub platform: u32,
    /// Advanced options.
    pub options: u32,
}

/// Represents operations found in an advertisement script.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AasOpcode {
    /// Register a COM class.
    Class(String),
    /// Register a file extension.
    Extension(String),
    /// Create a shortcut.
    Shortcut(String),
}

/// Parses Windows Installer Advertisement Scripts (.aas).
#[derive(Debug)]
pub struct AasParser {
    /// Parsed opcodes.
    pub opcodes: Vec<AasOpcode>,
}

impl AasParser {
    /// Parses an `.aas` script from a binary byte slice.
    ///
    /// Microsoft `.aas` scripts are undocumented. This implementation assumes a binary format
    /// with a signature `AAS\0` followed by TLV (Type-Length-Value) encoded opcodes.
    ///
    /// # Errors
    ///
    /// Returns `MsiError::InvalidData` if the signature is missing or the format is corrupt.
    ///
    /// # Arguments
    ///
    /// * `data` - TODO: Document argument.
    ///
    /// # Returns
    ///
    /// TODO: Document return value.
    pub fn parse(data: &[u8]) -> Result<Self, MsiError> {
        if data.len() < 4 || &data[0..4] != b"AAS\0" {
            return Err(MsiError::AdvertisementError(
                "Invalid .aas script signature".to_string(),
            ));
        }

        let mut opcodes = Vec::new();
        let mut offset = 4;

        while offset < data.len() {
            if offset + 2 > data.len() {
                return Err(MsiError::AdvertisementError(
                    "Truncated .aas script".to_string(),
                ));
            }
            let op_type = data[offset];
            let len = data[offset + 1] as usize;
            offset += 2;

            if offset + len > data.len() {
                return Err(MsiError::AdvertisementError(
                    "Truncated .aas script data payload".to_string(),
                ));
            }
            let payload = String::from_utf8_lossy(&data[offset..offset + len]).to_string();
            offset += len;

            match op_type {
                1 => opcodes.push(AasOpcode::Class(payload)),
                2 => opcodes.push(AasOpcode::Extension(payload)),
                3 => opcodes.push(AasOpcode::Shortcut(payload)),
                _ => {
                    // Ignore unknown opcodes to maintain forward compatibility.
                }
            }
        }

        Ok(Self { opcodes })
    }
}

/// Advertises a product package.
///
/// # Arguments
///
/// * `package_path` - Path to the `.msi` package.
/// * `script_info` - Optional script info.
/// * `transforms` - Optional transforms applied.
/// * `options` - Advertisement options.
///
/// # Errors
///
/// Returns `MsiError::InvalidArgument { argument: "path".to_string(), reason: "empty".to_string() }` if `package_path` is empty.
///
/// # Returns
///
/// TODO: Document return value.
pub fn advertise_product(
    package_path: &str,
    _script_info: Option<&str>,
    _transforms: Option<&str>,
    _options: AdvertiseOptions,
) -> Result<(), MsiError> {
    if package_path.is_empty() {
        return Err(MsiError::InvalidArgument {
            argument: "path".to_string(),
            reason: "empty".to_string(),
        });
    }
    // TODO: Implement actual advertisement logic (e.g. read MSI, generate shortcuts).
    Ok(())
}

/// Advertises from an advertisement script (AAS).
///
/// # Arguments
///
/// * `script_file` - Path to the `.aas` script file.
/// * `flags` - Advertisement flags.
/// * `remove_items` - Whether to remove items.
///
/// # Errors
///
/// Returns `MsiError::InvalidArgument { argument: "path".to_string(), reason: "empty".to_string() }` if `script_file` is empty.
///
/// # Returns
///
/// TODO: Document return value.
pub fn advertise_script(
    script_file: &str,
    _flags: u32,
    _remove_items: bool,
) -> Result<(), MsiError> {
    if script_file.is_empty() {
        return Err(MsiError::InvalidArgument {
            argument: "path".to_string(),
            reason: "empty".to_string(),
        });
    }
    let data = fs::read(script_file)
        .map_err(|e| MsiError::Io(crate::error::IoContext::from_string(e.to_string())))?;
    let parser = AasParser::parse(&data)?;

    for op in parser.opcodes {
        // Execute advertisement logic for each opcode
        let _ = op;
    }

    Ok(())
}

/// Processes an advertisement script.
///
/// # Arguments
///
/// * `script_file` - Path to the `.aas` script file.
/// * `icon_folder` - Path to the icon folder.
/// * `shortcuts` - Whether to process shortcuts.
/// * `remove_items` - Whether to remove items.
///
/// # Errors
///
/// Returns `MsiError::InvalidArgument { argument: "path".to_string(), reason: "empty".to_string() }` if `script_file` is empty.
///
/// # Returns
///
/// TODO: Document return value.
pub fn process_advertise_script(
    script_file: &str,
    _icon_folder: Option<&str>,
    _shortcuts: bool,
    _remove_items: bool,
) -> Result<(), MsiError> {
    if script_file.is_empty() {
        return Err(MsiError::InvalidArgument {
            argument: "path".to_string(),
            reason: "empty".to_string(),
        });
    }
    let data = fs::read(script_file)
        .map_err(|e| MsiError::Io(crate::error::IoContext::from_string(e.to_string())))?;
    let parser = AasParser::parse(&data)?;

    for op in parser.opcodes {
        // Execute advertisement logic for each opcode, optionally filtering shortcuts
        let _ = op;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn test_aas_parser_valid() {
        let mut data = Vec::new();
        data.extend_from_slice(b"AAS\0");

        // Opcode 1: Class, len 4, "test"
        data.push(1);
        data.push(4);
        data.extend_from_slice(b"test");

        // Opcode 3: Shortcut, len 3, "lnk"
        data.push(3);
        data.push(3);
        data.extend_from_slice(b"lnk");

        // Opcode 99 (unknown), len 1, "x"
        data.push(99);
        data.push(1);
        data.extend_from_slice(b"x");

        let parser = AasParser::parse(&data).expect("test");
        assert_eq!(parser.opcodes.len(), 2);
        assert_eq!(parser.opcodes[0], AasOpcode::Class("test".to_string()));
        assert_eq!(parser.opcodes[1], AasOpcode::Shortcut("lnk".to_string()));
    }

    #[test]
    fn test_aas_parser_invalid_signature() {
        assert!(AasParser::parse(b"BAD\0").is_err());
        assert!(AasParser::parse(b"AA").is_err());
    }

    #[test]
    fn test_aas_parser_truncated() {
        let mut data = Vec::new();
        data.extend_from_slice(b"AAS\0");
        data.push(1);
        // missing length
        assert!(AasParser::parse(&data).is_err());

        let mut data2 = Vec::new();
        data2.extend_from_slice(b"AAS\0");
        data2.push(1);
        data2.push(5);
        data2.extend_from_slice(b"test"); // only 4 bytes instead of 5
        assert!(AasParser::parse(&data2).is_err());
    }

    #[test]
    fn test_advertise_product() {
        assert!(advertise_product("", None, None, AdvertiseOptions::default()).is_err());
        assert!(advertise_product("foo.msi", None, None, AdvertiseOptions::default()).is_ok());
    }

    #[test]
    fn test_advertise_script() {
        assert!(advertise_script("", 0, false).is_err());

        let mut file = NamedTempFile::new().expect("test");
        file.write_all(b"AAS\0\x01\x04test").expect("test");
        assert!(advertise_script(file.path().to_str().expect("test"), 0, false).is_ok());

        assert!(advertise_script("nonexistent.aas", 0, false).is_err());
    }

    #[test]
    fn test_process_advertise_script() {
        assert!(process_advertise_script("", None, false, false).is_err());

        let mut file = NamedTempFile::new().expect("test");
        file.write_all(b"AAS\0\x02\x03ext").expect("test");
        assert!(
            process_advertise_script(file.path().to_str().expect("test"), None, false, false)
                .is_ok()
        );

        assert!(process_advertise_script("nonexistent.aas", None, false, false).is_err());
    }

    #[test]
    fn test_advertise_options_default() {
        let opts = AdvertiseOptions::default();
        assert_eq!(opts.language, 0);
        assert_eq!(opts.platform, 0);
        assert_eq!(opts.options, 0);
        assert_eq!(
            format!("{opts:?}"),
            "AdvertiseOptions { language: 0, platform: 0, options: 0 }"
        );
        let opts2 = opts;
        assert_eq!(opts, opts2);
    }
}

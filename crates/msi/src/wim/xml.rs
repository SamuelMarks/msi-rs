//! WIM XML Manifest parsing.

use crate::error::{MsiError, Result};
use crate::wim::types::ImageIndex;
use crate::wix::xml::{XmlNode, XmlParser};

/// Metadata describing a specific image contained within the WIM file.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct WimImage {
    /// The 1-based index of this image.
    pub index: ImageIndex,
    /// The name of the image.
    pub name: String,
    /// A description of the image.
    pub description: Option<String>,
    /// The Windows architecture (e.g. `9` for x64, `0` for x86, `12` for ARM64).
    pub architecture: Option<u32>,
    /// The display name for the Windows edition.
    pub display_name: Option<String>,
    /// The default language (e.g. `en-US`).
    pub default_language: Option<String>,
}

/// The decoded XML manifest of a WIM archive.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct WimManifest {
    /// The total bytes of the WIM archive metadata.
    pub total_bytes: u64,
    /// List of images present in the archive.
    pub images: Vec<WimImage>,
}

impl WimManifest {
    /// Parses a WIM XML manifest from raw bytes (handling UTF-16LE or UTF-8).
    ///
    /// # Errors
    ///
    /// Returns [`MsiError::WimXmlParseError`] if the XML is malformed, missing the
    /// root `<WIM>` node, or contains invalid structures.
    pub fn parse(bytes: &[u8]) -> Result<Self> {
        if bytes.is_empty() {
            return Err(MsiError::WimXmlParseError {
                reason: "Empty XML manifest payload".to_string(),
            });
        }

        let xml_string = Self::decode_payload(bytes);
        let parser = XmlParser::new();

        let root = parser
            .parse(&xml_string)
            .map_err(|e| MsiError::WimXmlParseError {
                reason: format!("Failed to parse XML: {e}"),
            })?;

        if root.tag != "WIM" {
            return Err(MsiError::WimXmlParseError {
                reason: format!("Root node is '{}', expected 'WIM'", root.tag),
            });
        }

        let total_bytes = Self::parse_child_u64(&root, "TOTALBYTES").unwrap_or(0);
        let mut images = Vec::new();

        for child in &root.children {
            if child.tag == "IMAGE" {
                images.push(Self::parse_image(child)?);
            }
        }

        Ok(Self {
            total_bytes,
            images,
        })
    }

    /// Decodes the byte payload into a Rust string, automatically detecting
    /// UTF-16LE BOM or falling back to UTF-8 / UTF-16 decoding heuristics.
    fn decode_payload(bytes: &[u8]) -> String {
        if bytes.len() >= 2 && bytes[0] == 0xFF && bytes[1] == 0xFE {
            // UTF-16LE with BOM
            let u16s: Vec<u16> = bytes[2..]
                .chunks_exact(2)
                .map(|c| u16::from_le_bytes([c[0], c[1]]))
                .collect();
            String::from_utf16_lossy(&u16s)
        } else if bytes.len() >= 2 && bytes[0] == b'<' && bytes[1] == 0x00 {
            // UTF-16LE without BOM
            let u16s: Vec<u16> = bytes
                .chunks_exact(2)
                .map(|c| u16::from_le_bytes([c[0], c[1]]))
                .collect();
            String::from_utf16_lossy(&u16s)
        } else {
            // Assume UTF-8
            String::from_utf8_lossy(bytes).into_owned()
        }
    }

    /// Parses an `<IMAGE>` node into a [`WimImage`].
    fn parse_image(node: &XmlNode) -> Result<WimImage> {
        let index_str = node
            .attributes
            .get("INDEX")
            .ok_or_else(|| MsiError::WimXmlParseError {
                reason: "IMAGE tag missing 'INDEX' attribute".to_string(),
            })?;

        let index_val = index_str
            .parse::<u32>()
            .map_err(|_| MsiError::WimXmlParseError {
                reason: format!("Invalid IMAGE index: '{index_str}'"),
            })?;

        let name = Self::get_child_text(node, "NAME").unwrap_or_default();
        let description = Self::get_child_text(node, "DESCRIPTION");

        let mut architecture = None;
        let mut display_name = None;
        let mut default_language = None;

        if let Some(windows_node) = node.children.iter().find(|c| c.tag == "WINDOWS") {
            if let Some(arch_str) = Self::get_child_text(windows_node, "ARCH") {
                architecture = arch_str.parse::<u32>().ok();
            }
            if let Some(ed_node) = windows_node.children.iter().find(|c| c.tag == "EDITION") {
                display_name = Some(ed_node.text.clone());
            }
            if let Some(lang_node) = windows_node.children.iter().find(|c| c.tag == "LANGUAGES") {
                default_language = Self::get_child_text(lang_node, "DEFAULT");
            }
        }

        Ok(WimImage {
            index: ImageIndex(index_val),
            name,
            description,
            architecture,
            display_name,
            default_language,
        })
    }

    /// Retrieves the text content of a named child node, if it exists.
    fn get_child_text(node: &XmlNode, name: &str) -> Option<String> {
        node.children
            .iter()
            .find(|c| c.tag == name)
            .map(|c| c.text.clone())
    }

    /// Parses a `u64` from a named child node, if it exists and is valid.
    fn parse_child_u64(node: &XmlNode, name: &str) -> Option<u64> {
        Self::get_child_text(node, name).and_then(|text| text.parse::<u64>().ok())
    }
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::field_reassign_with_default,
    clippy::shadow_unrelated,
    clippy::unreadable_literal,
    clippy::assert_is_empty,
    clippy::cast_possible_truncation
)]
mod tests {
    use super::*;

    /// Tests WIM XML manifest parsing with UTF-16LE BOM encoding.
    #[test]
    fn test_wim_xml_parse_utf16le_bom() -> Result<()> {
        let xml = r#"<WIM><TOTALBYTES>12345</TOTALBYTES><IMAGE INDEX="1"><NAME>Windows 10 Pro</NAME><DESCRIPTION>Description</DESCRIPTION><WINDOWS><ARCH>9</ARCH><EDITION>Professional</EDITION><LANGUAGES><DEFAULT>en-US</DEFAULT></LANGUAGES></WINDOWS></IMAGE></WIM>"#;

        let mut bytes = vec![0xFF, 0xFE];
        for b in xml.encode_utf16() {
            bytes.extend_from_slice(&b.to_le_bytes());
        }

        let manifest = WimManifest::parse(&bytes)?;

        assert_eq!(manifest.total_bytes, 12345);
        assert_eq!(manifest.images.len(), 1);

        let img = &manifest.images[0];
        assert_eq!(img.index, ImageIndex(1));
        assert_eq!(img.name, "Windows 10 Pro");
        assert_eq!(img.description, Some("Description".to_string()));
        assert_eq!(img.architecture, Some(9));
        assert_eq!(img.display_name, Some("Professional".to_string()));
        assert_eq!(img.default_language, Some("en-US".to_string()));
        Ok(())
    }

    /// Tests WIM XML manifest parsing with UTF-8 encoding.
    #[test]
    fn test_wim_xml_parse_utf8() -> Result<()> {
        let xml = r#"<WIM><TOTALBYTES>42</TOTALBYTES><IMAGE INDEX="2"><NAME>ReactOS</NAME></IMAGE></WIM>"#;
        let bytes = xml.as_bytes();

        let manifest = WimManifest::parse(bytes)?;

        assert_eq!(manifest.total_bytes, 42);
        assert_eq!(manifest.images.len(), 1);

        let img = &manifest.images[0];
        assert_eq!(img.index, ImageIndex(2));
        assert_eq!(img.name, "ReactOS");
        assert_eq!(img.architecture, None);
        Ok(())
    }

    /// Tests parsing WIM XML manifest with UTF-16LE without BOM.
    #[test]
    fn test_wim_xml_parse_utf16le_no_bom() -> Result<()> {
        let xml = r#"<WIM><IMAGE INDEX="1"><NAME>Test</NAME></IMAGE></WIM>"#;

        let mut bytes = vec![];
        for b in xml.encode_utf16() {
            bytes.extend_from_slice(&b.to_le_bytes());
        }

        let manifest = WimManifest::parse(&bytes)?;
        assert_eq!(manifest.images[0].name, "Test");
        Ok(())
    }

    /// Tests WIM XML parsing errors.
    #[test]
    fn test_wim_xml_parse_errors() {
        // Empty bytes
        let err = WimManifest::parse(&[]).unwrap_err();
        assert!(matches!(err, MsiError::WimXmlParseError { .. }));

        // Invalid root node
        let xml_invalid_root = b"<ROOT><IMAGE INDEX=\"1\"></IMAGE></ROOT>";
        let err = WimManifest::parse(xml_invalid_root).unwrap_err();
        assert!(matches!(err, MsiError::WimXmlParseError { .. }));

        // Missing IMAGE index
        let xml_missing_index = b"<WIM><IMAGE><NAME>Test</NAME></IMAGE></WIM>";
        let err = WimManifest::parse(xml_missing_index).unwrap_err();
        assert!(matches!(err, MsiError::WimXmlParseError { .. }));

        // Invalid IMAGE index type
        let xml_bad_index = b"<WIM><IMAGE INDEX=\"ABC\"><NAME>Test</NAME></IMAGE></WIM>";
        let err = WimManifest::parse(xml_bad_index).unwrap_err();
        assert!(matches!(err, MsiError::WimXmlParseError { .. }));

        // Malformed XML structure
        let xml_malformed = b"<WIM><IMAGE INDEX=\"1\"><NAME>Unclosed</IMAGE></WIM>";
        let err = WimManifest::parse(xml_malformed).unwrap_err();
        assert!(matches!(err, MsiError::WimXmlParseError { .. }));
    }
    #[test]
    fn test_wim_xml_totalbytes_invalid() -> Result<()> {
        let xml = r#"<WIM><TOTALBYTES>INVALID</TOTALBYTES><IMAGE INDEX="1"><NAME>Windows 10 Pro</NAME></IMAGE></WIM>"#;
        let bytes = xml.as_bytes();
        let manifest = WimManifest::parse(bytes)?;
        assert_eq!(manifest.total_bytes, 0); // unwrap_or(0) logic fallback
        Ok(())
    }

    /// Tests trait implementations (`Debug`, `Clone`, `PartialEq`, `Default`).
    #[test]
    fn test_xml_traits() {
        let mut m1 = WimManifest::default();
        m1.total_bytes = 100;
        let m2 = m1.clone();
        assert_eq!(m1, m2);
        assert_eq!(format!("{m1:?}"), format!("{m2:?}"));

        let mut i1 = WimImage::default();
        i1.name = "Test".to_string();
        let i2 = i1.clone();
        assert_eq!(i1, i2);
        assert_eq!(format!("{i1:?}"), format!("{i2:?}"));
    }
}
#[cfg(test)]
mod additional_xml_tests {
    use super::*;

    #[test]
    fn test_wim_xml_windows_edge_cases() -> Result<()> {
        let xml1 = r#"<WIM><IMAGE INDEX="1"><WINDOWS></WINDOWS></IMAGE></WIM>"#;
        let manifest1 = WimManifest::parse(xml1.as_bytes())?;
        assert_eq!(manifest1.images[0].architecture, None);
        assert_eq!(manifest1.images[0].display_name, None);
        assert_eq!(manifest1.images[0].default_language, None);

        let xml2 = r#"<WIM><IMAGE INDEX="1"><WINDOWS><ARCH>INVALID</ARCH></WINDOWS></IMAGE></WIM>"#;
        let manifest2 = WimManifest::parse(xml2.as_bytes())?;
        assert_eq!(manifest2.images[0].architecture, None);
        Ok(())
    }
}

//! `WiX` localization (`.wxl`) XML parsing and string token expansion.
//!
//! Provides support for `WiX` localization files and `!(loc.StringId)` token resolution:
//! - Parsing `<WixLocalization>` XML documents with `Culture` and `Codepage` attributes.
//! - Parsing `<String>` elements with `Id` and `Overridable` attributes.
//! - Multi-culture fallback resolution (e.g. `-cultures:de-de;en-us`).
//! - Expanding `!(loc.StringId)` tokens across attribute values and text nodes.

use crate::error::{Error, Result};
use crate::wix::xml::{XmlNode, XmlParser};
use std::collections::HashMap;

/// An individual localized string definition from a `.wxl` file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WixLocString {
    /// String identifier.
    pub id: String,
    /// Localized string value.
    pub value: String,
    /// Whether this string can be overridden by a subsequent `.wxl` file.
    pub overridable: bool,
}

impl WixLocString {
    /// Creates a new [`WixLocString`].
    ///
    /// # Arguments
    ///
    /// * `id` - String identifier.
    /// * `value` - String value.
    /// * `overridable` - Whether this string is overridable.
    ///
    /// # Returns
    ///
    /// A new localized string entry.
    #[must_use]
    pub const fn new(id: String, value: String, overridable: bool) -> Self {
        Self {
            id,
            value,
            overridable,
        }
    }
}

/// A parsed `WiX` localization document (`.wxl`).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct WixLocalization {
    /// Primary culture for this document (e.g. `en-US`, `de-DE`).
    pub culture: Option<String>,
    /// Codepage for string encoding (e.g. `1252`, `65001`).
    pub codepage: Option<u16>,
    /// Localized strings mapped by string ID.
    pub strings: HashMap<String, WixLocString>,
}

impl WixLocalization {
    /// Creates a new empty [`WixLocalization`] document.
    ///
    /// # Returns
    ///
    /// A new empty localization document.
    #[must_use]
    pub fn new() -> Self {
        Self {
            culture: None,
            codepage: None,
            strings: HashMap::new(),
        }
    }

    /// Parses a `.wxl` XML string into a [`WixLocalization`] document.
    ///
    /// # Arguments
    ///
    /// * `xml_content` - The raw `.wxl` XML content.
    ///
    /// # Returns
    ///
    /// Parsed [`WixLocalization`] document.
    ///
    /// # Errors
    ///
    /// Returns [`Error::XmlParse`] on XML syntax error or [`Error::WixCompiler`]
    /// on schema validation errors.
    pub fn parse(xml_content: &str) -> Result<Self> {
        let parser = XmlParser::new();
        let root = parser.parse(xml_content)?;
        Self::from_xml_node(&root)
    }

    /// Builds a [`WixLocalization`] from a parsed [`XmlNode`].
    ///
    /// # Arguments
    ///
    /// * `root` - Root XML node, expecting `<WixLocalization>`.
    ///
    /// # Returns
    ///
    /// Parsed [`WixLocalization`].
    ///
    /// # Errors
    ///
    /// Returns [`Error::WixCompiler`] if root element is not `<WixLocalization>`.
    pub fn from_xml_node(root: &XmlNode) -> Result<Self> {
        if root.tag != "WixLocalization" {
            return Err(Error::WixCompiler {
                element: root.tag.clone(),
                message: "expected root element <WixLocalization>".to_string(),
            });
        }

        let culture = root.attribute("Culture").map(ToString::to_string);
        let codepage = root
            .attribute("Codepage")
            .and_then(|cp| cp.parse::<u16>().ok());

        let mut strings = HashMap::new();

        for child in &root.children {
            if child.tag == "String" {
                if let Some(id) = child.attribute("Id") {
                    let overridable = child
                        .attribute("Overridable")
                        .is_some_and(|s| s.eq_ignore_ascii_case("yes"));
                    let value = child.text.clone();
                    strings.insert(
                        id.to_string(),
                        WixLocString::new(id.to_string(), value, overridable),
                    );
                }
            }
        }

        Ok(Self {
            culture,
            codepage,
            strings,
        })
    }

    /// Adds or updates a string entry in this document.
    ///
    /// # Arguments
    ///
    /// * `entry` - Localized string to add or update.
    pub fn add_string(&mut self, entry: WixLocString) {
        self.strings.insert(entry.id.clone(), entry);
    }
}

/// Catalog aggregating multiple localization files across cultures with fallback resolution.
#[derive(Debug, Clone, Default)]
pub struct LocalizationCatalog {
    /// Documents mapped by lowercase culture identifier (or empty string for culture-neutral).
    documents: Vec<WixLocalization>,
}

impl LocalizationCatalog {
    /// Creates a new empty [`LocalizationCatalog`].
    ///
    /// # Returns
    ///
    /// A new empty catalog.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            documents: Vec::new(),
        }
    }

    /// Adds a parsed [`WixLocalization`] document to the catalog.
    ///
    /// # Arguments
    ///
    /// * `doc` - Localization document to add.
    pub fn add_document(&mut self, doc: WixLocalization) {
        self.documents.push(doc);
    }

    /// Resolves a localized string identifier against the configured culture priority chain.
    ///
    /// # Arguments
    ///
    /// * `id` - String identifier to look up.
    /// * `cultures` - Ordered culture priority list (e.g. `["de-de", "en-us"]`).
    ///
    /// # Returns
    ///
    /// The resolved localized string value, or `None` if not found.
    #[must_use]
    pub fn resolve_string(&self, id: &str, cultures: &[String]) -> Option<String> {
        // First check specified cultures in order
        for target_cult in cultures {
            for doc in self.documents.iter().rev() {
                if let Some(ref cult) = doc.culture {
                    if cult.eq_ignore_ascii_case(target_cult) {
                        if let Some(entry) = doc.strings.get(id) {
                            return Some(entry.value.clone());
                        }
                    }
                }
            }
        }

        // Then check any document without culture or any remaining document
        for doc in self.documents.iter().rev() {
            if let Some(entry) = doc.strings.get(id) {
                return Some(entry.value.clone());
            }
        }

        None
    }

    /// Expands all `!(loc.StringId)` tokens in the given text.
    ///
    /// # Arguments
    ///
    /// * `text` - Input text containing zero or more `!(loc.StringId)` tokens.
    /// * `cultures` - Ordered culture priority list.
    ///
    /// # Returns
    ///
    /// Text with all tokens expanded.
    ///
    /// # Errors
    ///
    /// Returns [`Error::WixLinker`] if any `!(loc.StringId)` token cannot be resolved.
    pub fn expand_loc_tokens(&self, text: &str, cultures: &[String]) -> Result<String> {
        if !text.contains("!(loc.") {
            return Ok(text.to_string());
        }

        let mut result = String::with_capacity(text.len());
        let mut rest = text;

        while let Some(start_idx) = rest.find("!(loc.") {
            result.push_str(&rest[..start_idx]);
            let after_prefix = &rest[start_idx + 6..];
            if let Some(end_idx) = after_prefix.find(')') {
                let id = &after_prefix[..end_idx];
                let resolved =
                    self.resolve_string(id, cultures)
                        .ok_or_else(|| Error::WixLinker {
                            message: format!("Unresolved localization string token: '!(loc.{id})'"),
                        })?;
                result.push_str(&resolved);
                rest = &after_prefix[end_idx + 1..];
            } else {
                result.push_str("!(loc.");
                rest = after_prefix;
            }
        }

        result.push_str(rest);
        Ok(result)
    }

    /// Returns the resolved primary codepage from the highest-priority localization document.
    ///
    /// # Arguments
    ///
    /// * `cultures` - Ordered culture priority list.
    ///
    /// # Returns
    ///
    /// Codepage integer if defined in any matching document.
    #[must_use]
    pub fn get_primary_codepage(&self, cultures: &[String]) -> Option<u16> {
        for target_cult in cultures {
            for doc in self.documents.iter().rev() {
                if let Some(ref cult) = doc.culture {
                    if cult.eq_ignore_ascii_case(target_cult) && doc.codepage.is_some() {
                        return doc.codepage;
                    }
                }
            }
        }
        for doc in self.documents.iter().rev() {
            if doc.codepage.is_some() {
                return doc.codepage;
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_wix_loc_string_creation() {
        let entry = WixLocString::new("Title".to_string(), "My Title".to_string(), true);
        assert_eq!(entry.id, "Title");
        assert_eq!(entry.value, "My Title");
        assert!(entry.overridable);
    }

    #[test]
    fn test_parse_wxl_document() -> Result<()> {
        let xml = r#"
<WixLocalization Culture="en-US" Codepage="1252" xmlns="http://schemas.microsoft.com/wix/2006/localization">
    <String Id="WelcomeTitle">Welcome to the Installer</String>
    <String Id="LicenseText" Overridable="yes">Standard EULA</String>
</WixLocalization>
"#;
        let mut doc = WixLocalization::parse(xml)?;
        assert_eq!(doc.culture.as_deref(), Some("en-US"));
        assert_eq!(doc.codepage, Some(1252));
        assert_eq!(doc.strings.len(), 2);
        assert_eq!(
            doc.strings.get("WelcomeTitle").map(|s| s.value.as_str()),
            Some("Welcome to the Installer")
        );
        assert!(doc
            .strings
            .get("LicenseText")
            .is_some_and(|s| s.overridable));

        // Add string directly
        doc.add_string(WixLocString::new(
            "Extra".to_string(),
            "Val".to_string(),
            false,
        ));
        assert_eq!(doc.strings.len(), 3);

        Ok(())
    }

    #[test]
    fn test_parse_wxl_invalid_root() {
        let xml = r#"<WrongTag><String Id="A">B</String></WrongTag>"#;
        let err = WixLocalization::parse(xml);
        assert!(err.is_err());
    }

    #[test]
    fn test_localization_catalog_resolution_and_expansion() -> Result<()> {
        let mut catalog = LocalizationCatalog::new();

        let en_xml = r#"
<WixLocalization Culture="en-US" Codepage="1252">
    <String Id="Greeting">Hello</String>
    <String Id="Goodbye">Farewell</String>
</WixLocalization>
"#;
        let de_xml = r#"
<WixLocalization Culture="de-DE" Codepage="1252">
    <String Id="Greeting">Guten Tag</String>
</WixLocalization>
"#;
        catalog.add_document(WixLocalization::parse(en_xml)?);
        catalog.add_document(WixLocalization::parse(de_xml)?);

        let cult_de = vec!["de-de".to_string(), "en-us".to_string()];
        let cult_en = vec!["en-us".to_string()];

        // In de-DE culture: Greeting is German, Goodbye falls back to English
        assert_eq!(
            catalog.resolve_string("Greeting", &cult_de).as_deref(),
            Some("Guten Tag")
        );
        assert_eq!(
            catalog.resolve_string("Goodbye", &cult_de).as_deref(),
            Some("Farewell")
        );

        // In en-US culture: Greeting is English
        assert_eq!(
            catalog.resolve_string("Greeting", &cult_en).as_deref(),
            Some("Hello")
        );

        // Missing ID returns None
        assert!(catalog.resolve_string("NonExistent", &cult_de).is_none());

        // expand_loc_tokens
        let expanded =
            catalog.expand_loc_tokens("Message: !(loc.Greeting)! Have a nice day.", &cult_de)?;
        assert_eq!(expanded, "Message: Guten Tag! Have a nice day.");

        // expand_loc_tokens with text containing no loc tokens
        let untouched = catalog.expand_loc_tokens("Plain text", &cult_de)?;
        assert_eq!(untouched, "Plain text");

        // expand_loc_tokens with malformed token (no closing paren)
        let malformed =
            catalog.expand_loc_tokens("Incomplete !(loc.Greeting without close", &cult_de)?;
        assert_eq!(malformed, "Incomplete !(loc.Greeting without close");

        // expand_loc_tokens with missing token errors
        let err = catalog.expand_loc_tokens("!(loc.UnknownToken)", &cult_de);
        assert!(err.is_err());

        // Primary codepage
        assert_eq!(catalog.get_primary_codepage(&cult_de), Some(1252));
        assert_eq!(catalog.get_primary_codepage(&[]), Some(1252));

        let empty_catalog = LocalizationCatalog::new();
        assert_eq!(empty_catalog.get_primary_codepage(&[]), None);

        Ok(())
    }

    #[test]
    fn test_wix_localization_edge_cases() -> Result<()> {
        let def_doc = WixLocalization::default();
        assert_eq!(def_doc, WixLocalization::new());

        let mixed_xml = r#"
<WixLocalization Codepage="932">
    <IgnoreTag />
    <String Id="NeutralKey">NeutralVal</String>
    <String>MissingIdTag</String>
</WixLocalization>
"#;
        let parsed = WixLocalization::parse(mixed_xml)?;
        assert_eq!(parsed.codepage, Some(932));
        assert_eq!(parsed.strings.len(), 1);

        let mut cat = LocalizationCatalog::default();
        cat.add_document(parsed);
        assert_eq!(
            cat.resolve_string("NeutralKey", &["fr-fr".to_string()]),
            Some("NeutralVal".to_string())
        );
        assert_eq!(cat.get_primary_codepage(&["fr-fr".to_string()]), Some(932));

        let doc_no_cp = WixLocalization {
            culture: Some("en-us".to_string()),
            codepage: None,
            strings: HashMap::new(),
        };
        let mut cat_no_cp = LocalizationCatalog::default();
        cat_no_cp.add_document(doc_no_cp);
        assert_eq!(cat_no_cp.get_primary_codepage(&["en-us".to_string()]), None);
        assert_eq!(cat_no_cp.get_primary_codepage(&["de-de".to_string()]), None);

        Ok(())
    }
}

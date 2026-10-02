#![deny(clippy::unwrap_used)]
#![deny(missing_docs)]

//! `WixBalExtension` backend implementation.
//!
//! Provides parsing and linker support for `http://schemas.microsoft.com/wix/BalExtension`.

use crate::database::tables::record::{FieldValue, Record};
use crate::error::{MsiError, Result};
use crate::wix::extensions::WixExtension;
use crate::wix::linker::LinkedDatabase;
use crate::wix::wixobj::{IntermediateSection, IntermediateTable};
use crate::wix::xml::XmlNode;
use std::collections::HashMap;

/// Namespace URI for `WiX` Bal Extension.
pub const WIX_BAL_EXTENSION_NAMESPACE: &str = "http://schemas.microsoft.com/wix/BalExtension";

/// Backend implementation for `WixBalExtension`.
#[derive(Debug, Default, Clone, Copy)]
pub struct BalExtension;

impl BalExtension {
    /// Creates a new `BalExtension` instance.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }

    /// Compiles a `<bal:WixStandardBootstrapperApplication>` node.
    ///
    /// # Errors
    /// Returns [`MsiError::WixCompiler`] if required attributes are missing.
    #[allow(clippy::unnecessary_wraps)]
    fn compile_standard_bootstrapper(
        node: &XmlNode,
        parent_id: Option<&str>,
        tables: &mut HashMap<String, IntermediateTable>,
    ) -> Result<()> {
        let license_url = node.attribute("LicenseUrl").unwrap_or("");
        let theme = node.attribute("Theme").unwrap_or("standard");
        let logo_file = node.attribute("LogoFile").unwrap_or("");
        let parent = parent_id.unwrap_or("");

        let row = Record::with_fields(vec![
            FieldValue::String(parent.to_string()),
            FieldValue::String(license_url.to_string()),
            FieldValue::String(theme.to_string()),
            FieldValue::String(logo_file.to_string()),
        ]);

        tables
            .entry("WixBalStdBootstrapperApp".to_string())
            .or_insert_with(|| IntermediateTable::new("WixBalStdBootstrapperApp"))
            .push_record(row);

        Ok(())
    }
}

impl WixExtension for BalExtension {
    fn id(&self) -> &'static str {
        "WixBalExtension"
    }

    fn supported_namespaces(&self) -> &[&'static str] {
        &[WIX_BAL_EXTENSION_NAMESPACE]
    }

    fn compile_node(
        &self,
        node: &XmlNode,
        parent_id: Option<&str>,
        _section: &mut IntermediateSection,
        tables: &mut HashMap<String, IntermediateTable>,
    ) -> Result<()> {
        let tag_name = node.tag.split(':').next_back().unwrap_or(&node.tag);

        match tag_name {
            "WixStandardBootstrapperApplication" => {
                Self::compile_standard_bootstrapper(node, parent_id, tables)
            }
            _ => Err(MsiError::WixExtension {
                extension: self.id().to_string(),
                message: format!("unsupported element: '{tag_name}'"),
            }),
        }
    }

    fn link_database(&self, db: &mut LinkedDatabase) -> Result<()> {
        let has_bal = db
            .tables
            .get("WixBalStdBootstrapperApp")
            .is_some_and(|t| !t.is_empty());

        if !has_bal {
            return Ok(());
        }

        // The BalExtension typically influences Burn bundle compilation, rather than adding standard MSI tables.
        // We ensure a Property is added if we're somehow compiling it into an MSI for debugging/parity tracking.
        let _ = db.add_or_merge_record(
            "Property",
            Record::with_fields(vec![
                FieldValue::String("WixBalStdBootstrapperAppConfigured".to_string()),
                FieldValue::String("1".to_string()),
            ]),
        );

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::wix::wixobj::SectionType;

    /// Tests `BalExtension` constructor, traits, identifiers, and supported namespaces.
    #[test]
    #[allow(clippy::clone_on_copy, clippy::default_constructed_unit_structs)]
    fn test_bal_extension_id_and_namespaces() {
        let ext = BalExtension::new();
        let ext_clone = ext;
        let _ = ext.clone();
        let _ = format!("{ext:?}");
        let _ = BalExtension::default();
        assert_eq!(ext.id(), "WixBalExtension");
        assert_eq!(ext_clone.id(), "WixBalExtension");
        assert_eq!(ext.supported_namespaces(), &[WIX_BAL_EXTENSION_NAMESPACE]);
    }

    /// Tests compiling `<bal:WixStandardBootstrapperApplication>` with explicit attributes.
    #[test]
    fn test_compile_standard_bootstrapper_success() {
        let ext = BalExtension::new();
        let xml = r#"<bal:WixStandardBootstrapperApplication LicenseUrl="http://example.com" Theme="hyperlinkSidebar" LogoFile="logo.png" />"#;
        let parser = crate::wix::xml::XmlParser::new();
        let node = parser.parse(xml).unwrap_or_default();

        let mut section = IntermediateSection::new(SectionType::Fragment, Some("bun".to_string()));
        let mut tables = HashMap::new();

        ext.compile_node(&node, Some("bun1"), &mut section, &mut tables)
            .unwrap_or_default();

        assert!(tables.contains_key("WixBalStdBootstrapperApp"));
        let table = &tables["WixBalStdBootstrapperApp"];
        assert_eq!(table.records.len(), 1);

        let fields = table.records[0].fields();
        assert_eq!(fields[0], FieldValue::String("bun1".to_string()));
        assert_eq!(
            fields[1],
            FieldValue::String("http://example.com".to_string())
        );
        assert_eq!(
            fields[2],
            FieldValue::String("hyperlinkSidebar".to_string())
        );
        assert_eq!(fields[3], FieldValue::String("logo.png".to_string()));
    }

    /// Tests compiling `<bal:WixStandardBootstrapperApplication>` with default attributes.
    #[test]
    fn test_compile_standard_bootstrapper_defaults() {
        let ext = BalExtension::new();
        let xml = "<bal:WixStandardBootstrapperApplication />";
        let parser = crate::wix::xml::XmlParser::new();
        let node = parser.parse(xml).unwrap_or_default();

        let mut section = IntermediateSection::new(SectionType::Fragment, None);
        let mut tables = HashMap::new();

        ext.compile_node(&node, None, &mut section, &mut tables)
            .unwrap_or_default();

        let fields = tables["WixBalStdBootstrapperApp"].records[0].fields();
        assert_eq!(fields[1], FieldValue::String(String::new())); // LicenseUrl
        assert_eq!(fields[2], FieldValue::String("standard".to_string())); // Theme
        assert_eq!(fields[3], FieldValue::String(String::new())); // LogoFile
    }

    /// Tests unsupported elements returning [`MsiError::WixExtension`].
    #[test]
    fn test_unsupported_element() {
        let ext = BalExtension::new();
        let xml = "<bal:Unsupported />";
        let parser = crate::wix::xml::XmlParser::new();
        let node = parser.parse(xml).unwrap_or_default();

        let mut section = IntermediateSection::new(SectionType::Fragment, None);
        let mut tables = HashMap::new();

        let res = ext.compile_node(&node, Some("cmp1"), &mut section, &mut tables);
        assert!(matches!(res, Err(MsiError::WixExtension { .. })));
    }

    /// Tests linking behavior of `BalExtension` on databases with and without bal tables.
    #[test]
    fn test_link_database_stub() {
        let ext = BalExtension::new();
        let mut db = LinkedDatabase::default();

        // Empty table test
        assert!(ext.link_database(&mut db).is_ok());

        // Add a bal entry
        let _ = db.add_or_merge_record(
            "WixBalStdBootstrapperApp",
            Record::with_fields(vec![
                FieldValue::String("bun1".to_string()),
                FieldValue::String("http".to_string()),
                FieldValue::String("theme".to_string()),
                FieldValue::String("logo".to_string()),
            ]),
        );

        assert!(ext.link_database(&mut db).is_ok());

        assert!(db.tables.contains_key("Property"));
        let prop_tbl = &db.tables["Property"];
        assert!(prop_tbl.iter().any(|r| r.fields()[0]
            == FieldValue::String("WixBalStdBootstrapperAppConfigured".to_string())));
    }
}

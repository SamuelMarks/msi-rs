#![deny(clippy::unwrap_used)]
#![deny(missing_docs)]

//! `WixDependencyExtension` backend implementation.
//!
//! Provides parsing and linker support for `http://schemas.microsoft.com/wix/DependencyExtension`.

use crate::database::tables::record::{FieldValue, Record};
use crate::error::{MsiError, Result};
use crate::wix::extensions::WixExtension;
use crate::wix::linker::LinkedDatabase;
use crate::wix::wixobj::{IntermediateSection, IntermediateTable};
use crate::wix::xml::XmlNode;
use std::collections::HashMap;

/// Namespace URI for `WiX` Dependency Extension.
pub const WIX_DEPENDENCY_EXTENSION_NAMESPACE: &str =
    "http://schemas.microsoft.com/wix/DependencyExtension";

/// Backend implementation for `WixDependencyExtension`.
#[derive(Debug, Default, Clone, Copy)]
pub struct DependencyExtension;

impl DependencyExtension {
    /// Creates a new `DependencyExtension` instance.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }

    /// Compiles a `<dep:Provides>` node into intermediate tables.
    ///
    /// # Errors
    ///
    /// Returns [`MsiError::WixCompiler`] if required attributes are missing.
    fn compile_provides(
        node: &XmlNode,
        parent_id: Option<&str>,
        tables: &mut HashMap<String, IntermediateTable>,
    ) -> Result<()> {
        let key = node.attribute("Key").ok_or_else(|| MsiError::WixCompiler {
            element: "dep:Provides".to_string(),
            message: "missing required 'Key' attribute".to_string(),
        })?;

        let version = node.attribute("Version").unwrap_or("");
        let display_name = node.attribute("DisplayName").unwrap_or("");
        let component = parent_id.unwrap_or(""); // Can be empty if authored elsewhere? Usually nested in Component.

        let row = Record::with_fields(vec![
            FieldValue::String(key.to_string()),
            FieldValue::String(version.to_string()),
            FieldValue::String(display_name.to_string()),
            FieldValue::String(component.to_string()),
        ]);

        tables
            .entry("WixDependencyProvider".to_string())
            .or_insert_with(|| IntermediateTable::new("WixDependencyProvider"))
            .push_record(row);

        Ok(())
    }

    /// Compiles a `<dep:Requires>` node into intermediate tables.
    ///
    /// # Errors
    ///
    /// Returns [`MsiError::WixCompiler`] if required attributes are missing.
    fn compile_requires(
        node: &XmlNode,
        parent_id: Option<&str>,
        tables: &mut HashMap<String, IntermediateTable>,
    ) -> Result<()> {
        let provider_key = node
            .attribute("ProviderKey")
            .ok_or_else(|| MsiError::WixCompiler {
                element: "dep:Requires".to_string(),
                message: "missing required 'ProviderKey' attribute".to_string(),
            })?;

        let version = node.attribute("Version").unwrap_or("");
        let min_version = node.attribute("MinVersion").unwrap_or("");
        let max_version = node.attribute("MaxVersion").unwrap_or("");
        let display_name = node.attribute("DisplayName").unwrap_or("");
        let component = parent_id.unwrap_or("");

        let row = Record::with_fields(vec![
            FieldValue::String(provider_key.to_string()),
            FieldValue::String(version.to_string()),
            FieldValue::String(min_version.to_string()),
            FieldValue::String(max_version.to_string()),
            FieldValue::String(display_name.to_string()),
            FieldValue::String(component.to_string()),
        ]);

        tables
            .entry("WixDependencyRequirement".to_string())
            .or_insert_with(|| IntermediateTable::new("WixDependencyRequirement"))
            .push_record(row);

        Ok(())
    }
}

impl WixExtension for DependencyExtension {
    fn id(&self) -> &'static str {
        "WixDependencyExtension"
    }

    fn supported_namespaces(&self) -> &[&'static str] {
        &[WIX_DEPENDENCY_EXTENSION_NAMESPACE]
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
            "Provides" => Self::compile_provides(node, parent_id, tables),
            "Requires" => Self::compile_requires(node, parent_id, tables),
            _ => Err(MsiError::WixExtension {
                extension: self.id().to_string(),
                message: format!("unsupported element: '{tag_name}'"),
            }),
        }
    }

    fn link_database(&self, db: &mut LinkedDatabase) -> Result<()> {
        // WiX dependency extension requires injecting custom actions to register/unregister
        // providers and requirements with the Windows Installer dependency keys in the registry.

        let has_providers = db
            .tables
            .get("WixDependencyProvider")
            .is_some_and(|t| !t.is_empty());
        let has_requires = db
            .tables
            .get("WixDependencyRequirement")
            .is_some_and(|t| !t.is_empty());

        if !has_providers && !has_requires {
            return Ok(());
        }

        let ca_name = "WixDependencyExec".to_string();

        db.add_or_merge_record(
            "CustomAction",
            Record::with_fields(vec![
                FieldValue::String(ca_name.clone()),
                FieldValue::Long(38), // Type 38 = VBScript inline placeholder for now
                FieldValue::String(String::new()),
                FieldValue::String("WScript.Quit 0".to_string()), // Placeholder action script
            ]),
        )?;

        db.add_or_merge_record(
            "InstallExecuteSequence",
            Record::with_fields(vec![
                FieldValue::String(ca_name),
                FieldValue::String(String::new()),
                FieldValue::Long(6605), // Usually scheduled after InstallFiles
            ]),
        )?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::wix::wixobj::SectionType;

    /// Tests `DependencyExtension` constructor, traits, identifiers, and supported namespaces.
    #[test]
    #[allow(clippy::clone_on_copy, clippy::default_constructed_unit_structs)]
    fn test_dependency_extension_id_and_namespaces() {
        let ext = DependencyExtension::new();
        let ext_clone = ext;
        let _ = ext.clone();
        let _ = format!("{ext:?}");
        let _ = DependencyExtension::default();
        assert_eq!(ext.id(), "WixDependencyExtension");
        assert_eq!(ext_clone.id(), "WixDependencyExtension");
        assert_eq!(
            ext.supported_namespaces(),
            &[WIX_DEPENDENCY_EXTENSION_NAMESPACE]
        );
    }

    /// Tests compiling `<dep:Provides>` with explicit attributes.
    #[test]
    fn test_compile_provides_success() {
        let ext = DependencyExtension::new();
        let xml = r#"<dep:Provides Key="MyKey" Version="1.0" DisplayName="My Dep" />"#;
        let parser = crate::wix::xml::XmlParser::new();
        let node = parser.parse(xml).unwrap_or_default();

        let mut section = IntermediateSection::new(SectionType::Product, Some("prod".to_string()));
        let mut tables = HashMap::new();

        ext.compile_node(&node, Some("cmp1"), &mut section, &mut tables)
            .unwrap_or_default();

        assert!(tables.contains_key("WixDependencyProvider"));
        let table = &tables["WixDependencyProvider"];
        assert_eq!(table.records.len(), 1);

        let fields = table.records[0].fields();
        assert_eq!(fields[0], FieldValue::String("MyKey".to_string()));
        assert_eq!(fields[1], FieldValue::String("1.0".to_string()));
        assert_eq!(fields[2], FieldValue::String("My Dep".to_string()));
        assert_eq!(fields[3], FieldValue::String("cmp1".to_string()));
    }

    /// Tests compiling `<dep:Provides>` when required Key attribute is missing.
    #[test]
    fn test_compile_provides_missing_key() {
        let ext = DependencyExtension::new();
        let xml = r#"<dep:Provides Version="1.0" />"#;
        let parser = crate::wix::xml::XmlParser::new();
        let node = parser.parse(xml).unwrap_or_default();

        let mut section = IntermediateSection::new(SectionType::Product, None);
        let mut tables = HashMap::new();

        let res = ext.compile_node(&node, Some("cmp1"), &mut section, &mut tables);
        assert!(matches!(res, Err(MsiError::WixCompiler { .. })));
    }

    /// Tests compiling `<dep:Requires>` with explicit attributes.
    #[test]
    fn test_compile_requires_success() {
        let ext = DependencyExtension::new();
        let xml = r#"<dep:Requires ProviderKey="MyKey" MinVersion="1.0" MaxVersion="2.0" />"#;
        let parser = crate::wix::xml::XmlParser::new();
        let node = parser.parse(xml).unwrap_or_default();

        let mut section = IntermediateSection::new(SectionType::Product, Some("prod".to_string()));
        let mut tables = HashMap::new();

        ext.compile_node(&node, Some("cmp1"), &mut section, &mut tables)
            .unwrap_or_default();

        assert!(tables.contains_key("WixDependencyRequirement"));
        let table = &tables["WixDependencyRequirement"];
        assert_eq!(table.records.len(), 1);

        let fields = table.records[0].fields();
        assert_eq!(fields[0], FieldValue::String("MyKey".to_string()));
        assert_eq!(fields[1], FieldValue::String(String::new())); // Version
        assert_eq!(fields[2], FieldValue::String("1.0".to_string())); // MinVersion
        assert_eq!(fields[3], FieldValue::String("2.0".to_string())); // MaxVersion
        assert_eq!(fields[4], FieldValue::String(String::new())); // DisplayName
        assert_eq!(fields[5], FieldValue::String("cmp1".to_string())); // component
    }

    /// Tests compiling `<dep:Requires>` when required `ProviderKey` is missing.
    #[test]
    fn test_compile_requires_missing_key() {
        let ext = DependencyExtension::new();
        let xml = "<dep:Requires />";
        let parser = crate::wix::xml::XmlParser::new();
        let node = parser.parse(xml).unwrap_or_default();

        let mut section = IntermediateSection::new(SectionType::Product, None);
        let mut tables = HashMap::new();

        let res = ext.compile_node(&node, Some("cmp1"), &mut section, &mut tables);
        assert!(matches!(res, Err(MsiError::WixCompiler { .. })));
    }

    /// Tests compiling unsupported elements returns [`MsiError::WixExtension`].
    #[test]
    fn test_unsupported_element() {
        let ext = DependencyExtension::new();
        let xml = "<dep:Unsupported />";
        let parser = crate::wix::xml::XmlParser::new();
        let node = parser.parse(xml).unwrap_or_default();

        let mut section = IntermediateSection::new(SectionType::Product, None);
        let mut tables = HashMap::new();

        let res = ext.compile_node(&node, Some("cmp1"), &mut section, &mut tables);
        assert!(matches!(res, Err(MsiError::WixExtension { .. })));
    }

    /// Tests linking dependency custom actions and sequences.
    #[test]
    fn test_link_database_stub() {
        let ext = DependencyExtension::new();
        let mut db = LinkedDatabase::default();

        // Empty table test
        assert!(ext.link_database(&mut db).is_ok());

        // Add a provider
        let _ = db.add_or_merge_record(
            "WixDependencyProvider",
            Record::with_fields(vec![
                FieldValue::String("MyKey".to_string()),
                FieldValue::String("1.0".to_string()),
                FieldValue::String("My Dep".to_string()),
                FieldValue::String("cmp1".to_string()),
            ]),
        );

        assert!(ext.link_database(&mut db).is_ok());

        // Verify CustomAction was injected
        assert!(db.tables.contains_key("CustomAction"));
        let ca_tbl = &db.tables["CustomAction"];
        assert!(ca_tbl
            .iter()
            .any(|r| r.fields()[0] == FieldValue::String("WixDependencyExec".to_string())));

        // Verify InstallExecuteSequence was injected
        assert!(db.tables.contains_key("InstallExecuteSequence"));
        let ies_tbl = &db.tables["InstallExecuteSequence"];
        assert!(ies_tbl
            .iter()
            .any(|r| r.fields()[0] == FieldValue::String("WixDependencyExec".to_string())));

        // Test with requirement only (covers !has_providers && !has_requires false branch where has_providers is false)
        let mut db_req_only = LinkedDatabase::default();
        let _ = db_req_only.add_or_merge_record(
            "WixDependencyRequirement",
            Record::with_fields(vec![
                FieldValue::String("MyKey".to_string()),
                FieldValue::String(String::new()),
                FieldValue::String("1.0".to_string()),
                FieldValue::String("2.0".to_string()),
                FieldValue::String(String::new()),
                FieldValue::String("cmp1".to_string()),
            ]),
        );
        assert!(ext.link_database(&mut db_req_only).is_ok());
        assert!(db_req_only.tables.contains_key("CustomAction"));
    }

    /// Tests linking error path when a conflicting `CustomAction` table record exists.
    #[test]
    fn test_link_database_duplicate_ca_failure() {
        let ext = DependencyExtension::new();
        let mut db = LinkedDatabase::default();

        let _ = db.add_or_merge_record(
            "WixDependencyProvider",
            Record::with_fields(vec![
                FieldValue::String("MyKey".to_string()),
                FieldValue::String("1.0".to_string()),
                FieldValue::String("My Dep".to_string()),
                FieldValue::String("cmp1".to_string()),
            ]),
        );

        // Add a conflicting CustomAction
        let _ = db.add_or_merge_record(
            "CustomAction",
            Record::with_fields(vec![
                FieldValue::String("WixDependencyExec".to_string()),
                FieldValue::Long(1), // Different type
                FieldValue::String(String::new()),
                FieldValue::String("Different".to_string()),
            ]),
        );

        assert!(ext.link_database(&mut db).is_err());
    }

    /// Tests linking error path when a conflicting `InstallExecuteSequence` record exists.
    #[test]
    fn test_link_database_duplicate_sequence_failure() {
        let ext = DependencyExtension::new();
        let mut db = LinkedDatabase::default();

        let _ = db.add_or_merge_record(
            "WixDependencyProvider",
            Record::with_fields(vec![
                FieldValue::String("MyKey".to_string()),
                FieldValue::String("1.0".to_string()),
                FieldValue::String("My Dep".to_string()),
                FieldValue::String("cmp1".to_string()),
            ]),
        );

        // Add a conflicting InstallExecuteSequence
        let _ = db.add_or_merge_record(
            "InstallExecuteSequence",
            Record::with_fields(vec![
                FieldValue::String("WixDependencyExec".to_string()),
                FieldValue::String(String::new()),
                FieldValue::Long(9999), // Different sequence
            ]),
        );

        assert!(ext.link_database(&mut db).is_err());
    }
}

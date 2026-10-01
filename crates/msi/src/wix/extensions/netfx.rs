#![deny(clippy::unwrap_used)]
#![deny(missing_docs)]

//! `WixNetFxExtension` backend implementation.
//!
//! Provides parsing and linker support for `http://schemas.microsoft.com/wix/NetFxExtension`.

use crate::database::tables::record::{FieldValue, Record};
use crate::error::{Error, Result};
use crate::wix::extensions::WixExtension;
use crate::wix::linker::LinkedDatabase;
use crate::wix::wixobj::{IntermediateSection, IntermediateTable};
use crate::wix::xml::XmlNode;
use std::collections::HashMap;

/// Namespace URI for `WiX` `NetFx` Extension.
pub const WIX_NETFX_EXTENSION_NAMESPACE: &str = "http://schemas.microsoft.com/wix/NetFxExtension";

/// Backend implementation for `WixNetFxExtension`.
#[derive(Debug, Default, Clone, Copy)]
pub struct NetFxExtension;

impl NetFxExtension {
    /// Creates a new `NetFxExtension` instance.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }

    /// Compiles a `<netfx:NativeImage>` node into intermediate tables.
    ///
    /// # Errors
    ///
    /// Returns [`Error::WixCompiler`] if required attributes are missing.
    fn compile_native_image(
        node: &XmlNode,
        parent_id: Option<&str>,
        tables: &mut HashMap<String, IntermediateTable>,
    ) -> Result<()> {
        let id = node.attribute("Id").ok_or_else(|| Error::WixCompiler {
            element: "netfx:NativeImage".to_string(),
            message: "missing required 'Id' attribute".to_string(),
        })?;

        let file = parent_id.ok_or_else(|| Error::WixCompiler {
            element: "netfx:NativeImage".to_string(),
            message: "NativeImage must be nested under a File element".to_string(),
        })?;

        let app_base = node.attribute("AppBaseDirectory").unwrap_or("");

        let priority_str = node.attribute("Priority").unwrap_or("3");
        let priority: i32 = priority_str.parse().unwrap_or(3);

        let platform_str = node.attribute("Platform").unwrap_or("all");
        let platform: i32 = match platform_str {
            "32bit" => 1,
            "64bit" => 2,
            _ => 0, // all
        };

        let row = Record::with_fields(vec![
            FieldValue::String(file.to_string()),
            FieldValue::String(id.to_string()),
            FieldValue::String(app_base.to_string()),
            FieldValue::Long(priority),
            FieldValue::Long(platform),
        ]);

        tables
            .entry("NetFxNativeImage".to_string())
            .or_insert_with(|| IntermediateTable::new("NetFxNativeImage"))
            .push_record(row);

        Ok(())
    }
}

impl WixExtension for NetFxExtension {
    fn id(&self) -> &'static str {
        "WixNetFxExtension"
    }

    fn supported_namespaces(&self) -> &[&'static str] {
        &[WIX_NETFX_EXTENSION_NAMESPACE]
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
            "NativeImage" => Self::compile_native_image(node, parent_id, tables),
            _ => Err(Error::WixExtension {
                extension: self.id().to_string(),
                message: format!("unsupported element: '{tag_name}'"),
            }),
        }
    }

    fn link_database(&self, db: &mut LinkedDatabase) -> Result<()> {
        let has_ni = db
            .tables
            .get("NetFxNativeImage")
            .is_some_and(|t| !t.is_empty());

        if !has_ni {
            return Ok(());
        }

        let ca_name = "NetFxExecuteNativeImage".to_string();

        let _ = db.add_or_merge_record(
            "CustomAction",
            Record::with_fields(vec![
                FieldValue::String(ca_name.clone()),
                FieldValue::Long(38), // Type 38 = VBScript inline placeholder for now
                FieldValue::String(String::new()),
                // Placeholder action script instead of DLL injection for now
                FieldValue::String("WScript.Quit 0".to_string()),
            ]),
        );

        let _ = db.add_or_merge_record(
            "InstallExecuteSequence",
            Record::with_fields(vec![
                FieldValue::String(ca_name),
                FieldValue::String(String::new()),
                FieldValue::Long(6606),
            ]),
        );

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::wix::wixobj::SectionType;

    /// Tests `NetFxExtension` constructor, traits, identifiers, and supported namespaces.
    #[test]
    #[allow(clippy::clone_on_copy, clippy::default_constructed_unit_structs)]
    fn test_netfx_extension_id_and_namespaces() {
        let ext = NetFxExtension::new();
        let ext_clone = ext;
        let _ = ext.clone();
        let _ = format!("{ext:?}");
        let _ = NetFxExtension::default();
        assert_eq!(ext.id(), "WixNetFxExtension");
        assert_eq!(ext_clone.id(), "WixNetFxExtension");
        assert_eq!(ext.supported_namespaces(), &[WIX_NETFX_EXTENSION_NAMESPACE]);
    }

    /// Tests compiling `<netfx:NativeImage>` with explicit attributes including 32bit and 64bit platforms.
    #[test]
    fn test_compile_native_image_success() {
        let ext = NetFxExtension::new();
        let xml = r#"<netfx:NativeImage Id="NI_1" AppBaseDirectory="BinDir" Priority="1" Platform="64bit" />"#;
        let parser = crate::wix::xml::XmlParser::new();
        let node = parser.parse(xml).unwrap_or_default();

        let mut section = IntermediateSection::new(SectionType::Product, Some("prod".to_string()));
        let mut tables = HashMap::new();

        ext.compile_node(&node, Some("file1"), &mut section, &mut tables)
            .unwrap_or_default();

        assert!(tables.contains_key("NetFxNativeImage"));
        let table = &tables["NetFxNativeImage"];
        assert_eq!(table.records.len(), 1);

        let fields = table.records[0].fields();
        assert_eq!(fields[0], FieldValue::String("file1".to_string()));
        assert_eq!(fields[1], FieldValue::String("NI_1".to_string()));
        assert_eq!(fields[2], FieldValue::String("BinDir".to_string()));
        assert_eq!(fields[3], FieldValue::Long(1)); // priority
        assert_eq!(fields[4], FieldValue::Long(2)); // platform 64bit

        // Also test 32bit platform and fallback invalid priority
        let xml_32 = r#"<netfx:NativeImage Id="NI_2" Priority="invalid" Platform="32bit" />"#;
        let node_32 = parser.parse(xml_32).unwrap_or_default();
        ext.compile_node(&node_32, Some("file2"), &mut section, &mut tables)
            .unwrap_or_default();
        let native_img_tbl = &tables["NetFxNativeImage"];
        assert_eq!(native_img_tbl.records.len(), 2);
        let fields2 = native_img_tbl.records[1].fields();
        assert_eq!(fields2[3], FieldValue::Long(3)); // fallback priority 3
        assert_eq!(fields2[4], FieldValue::Long(1)); // platform 32bit
    }

    /// Tests compiling `<netfx:NativeImage>` with default attributes.
    #[test]
    fn test_compile_native_image_defaults() {
        let ext = NetFxExtension::new();
        let xml = r#"<netfx:NativeImage Id="NI_1" />"#;
        let parser = crate::wix::xml::XmlParser::new();
        let node = parser.parse(xml).unwrap_or_default();

        let mut section = IntermediateSection::new(SectionType::Product, None);
        let mut tables = HashMap::new();

        ext.compile_node(&node, Some("file1"), &mut section, &mut tables)
            .unwrap_or_default();

        let fields = tables["NetFxNativeImage"].records[0].fields();
        assert_eq!(fields[3], FieldValue::Long(3)); // default priority
        assert_eq!(fields[4], FieldValue::Long(0)); // default platform
    }

    /// Tests compiling `<netfx:NativeImage>` when missing Id attribute returns error.
    #[test]
    fn test_compile_native_image_missing_id() {
        let ext = NetFxExtension::new();
        let xml = r#"<netfx:NativeImage AppBaseDirectory="BinDir" />"#;
        let parser = crate::wix::xml::XmlParser::new();
        let node = parser.parse(xml).unwrap_or_default();

        let mut section = IntermediateSection::new(SectionType::Product, None);
        let mut tables = HashMap::new();

        let res = ext.compile_node(&node, Some("file1"), &mut section, &mut tables);
        assert!(matches!(res, Err(Error::WixCompiler { .. })));
    }

    /// Tests compiling `<netfx:NativeImage>` when missing parent file returns error.
    #[test]
    fn test_compile_native_image_missing_parent() {
        let ext = NetFxExtension::new();
        let xml = r#"<netfx:NativeImage Id="NI_1" />"#;
        let parser = crate::wix::xml::XmlParser::new();
        let node = parser.parse(xml).unwrap_or_default();

        let mut section = IntermediateSection::new(SectionType::Product, None);
        let mut tables = HashMap::new();

        let res = ext.compile_node(&node, None, &mut section, &mut tables);
        assert!(matches!(res, Err(Error::WixCompiler { .. })));
    }

    /// Tests compiling unsupported elements returns [`Error::WixExtension`].
    #[test]
    fn test_unsupported_element() {
        let ext = NetFxExtension::new();
        let xml = "<netfx:Unsupported />";
        let parser = crate::wix::xml::XmlParser::new();
        let node = parser.parse(xml).unwrap_or_default();

        let mut section = IntermediateSection::new(SectionType::Product, None);
        let mut tables = HashMap::new();

        let res = ext.compile_node(&node, Some("file1"), &mut section, &mut tables);
        assert!(matches!(res, Err(Error::WixExtension { .. })));
    }

    /// Tests linking native image actions into database.
    #[test]
    fn test_link_database_stub() {
        let ext = NetFxExtension::new();
        let mut db = LinkedDatabase::default();

        // Empty table test
        assert!(ext.link_database(&mut db).is_ok());

        // Add a native image
        let _ = db.add_or_merge_record(
            "NetFxNativeImage",
            Record::with_fields(vec![
                FieldValue::String("file1".to_string()),
                FieldValue::String("NI_1".to_string()),
                FieldValue::String("BinDir".to_string()),
                FieldValue::Long(3),
                FieldValue::Long(0),
            ]),
        );

        assert!(ext.link_database(&mut db).is_ok());

        // Verify CustomAction was injected
        assert!(db.tables.contains_key("CustomAction"));
        let ca_tbl = &db.tables["CustomAction"];
        assert!(ca_tbl
            .iter()
            .any(|r| r.fields()[0] == FieldValue::String("NetFxExecuteNativeImage".to_string())));

        // Verify InstallExecuteSequence was injected
        assert!(db.tables.contains_key("InstallExecuteSequence"));
        let ies_tbl = &db.tables["InstallExecuteSequence"];
        assert!(ies_tbl
            .iter()
            .any(|r| r.fields()[0] == FieldValue::String("NetFxExecuteNativeImage".to_string())));
    }
}

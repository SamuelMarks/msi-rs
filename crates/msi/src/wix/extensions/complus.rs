#![deny(clippy::unwrap_used)]
#![deny(missing_docs)]

//! `WixComPlusExtension` backend implementation.
//!
//! Provides parsing and linker support for `http://schemas.microsoft.com/wix/ComPlusExtension`.

use crate::database::tables::record::{FieldValue, Record};
use crate::error::{Error, Result};
use crate::wix::extensions::WixExtension;
use crate::wix::linker::LinkedDatabase;
use crate::wix::wixobj::{IntermediateSection, IntermediateTable};
use crate::wix::xml::XmlNode;
use std::collections::HashMap;

/// Namespace URI for `WiX` COM+ Extension.
pub const WIX_COMPLUS_EXTENSION_NAMESPACE: &str =
    "http://schemas.microsoft.com/wix/ComPlusExtension";

/// Backend implementation for `WixComPlusExtension`.
#[derive(Debug, Default, Clone, Copy)]
pub struct ComPlusExtension;

impl ComPlusExtension {
    /// Creates a new `ComPlusExtension` instance.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }

    /// Compiles a `<complus:ComPlusApplication>` node into intermediate tables.
    ///
    /// # Errors
    /// Returns [`Error::WixCompiler`] if required attributes are missing.
    fn compile_complus_application(
        node: &XmlNode,
        parent_id: Option<&str>,
        tables: &mut HashMap<String, IntermediateTable>,
    ) -> Result<()> {
        let id = node.attribute("Id").ok_or_else(|| Error::WixCompiler {
            element: "complus:ComPlusApplication".to_string(),
            message: "missing required 'Id' attribute".to_string(),
        })?;

        let name = node.attribute("Name").unwrap_or(id);

        let run_forever_str = node.attribute("RunForever").unwrap_or("no");
        let run_forever = i32::from(run_forever_str == "yes");

        let component = parent_id.unwrap_or("");

        let row = Record::with_fields(vec![
            FieldValue::String(id.to_string()),
            FieldValue::String(name.to_string()),
            FieldValue::Long(run_forever),
            FieldValue::String(component.to_string()),
        ]);

        tables
            .entry("WixComPlusApplication".to_string())
            .or_insert_with(|| IntermediateTable::new("WixComPlusApplication"))
            .push_record(row);

        Ok(())
    }

    /// Compiles a `<complus:ComPlusAssembly>` node into intermediate tables.
    ///
    /// # Errors
    /// Returns [`Error::WixCompiler`] if required attributes are missing.
    fn compile_complus_assembly(
        node: &XmlNode,
        parent_id: Option<&str>,
        tables: &mut HashMap<String, IntermediateTable>,
    ) -> Result<()> {
        let id = node.attribute("Id").ok_or_else(|| Error::WixCompiler {
            element: "complus:ComPlusAssembly".to_string(),
            message: "missing required 'Id' attribute".to_string(),
        })?;

        let dll_path = node
            .attribute("DllPath")
            .ok_or_else(|| Error::WixCompiler {
                element: "complus:ComPlusAssembly".to_string(),
                message: "missing required 'DllPath' attribute".to_string(),
            })?;

        let register_in_commit =
            i32::from(node.attribute("RegisterInCommit").unwrap_or("no") == "yes");

        let component = parent_id.unwrap_or("");

        let row = Record::with_fields(vec![
            FieldValue::String(id.to_string()),
            FieldValue::String(dll_path.to_string()),
            FieldValue::Long(register_in_commit),
            FieldValue::String(component.to_string()),
        ]);

        tables
            .entry("WixComPlusAssembly".to_string())
            .or_insert_with(|| IntermediateTable::new("WixComPlusAssembly"))
            .push_record(row);

        Ok(())
    }
}

impl WixExtension for ComPlusExtension {
    fn id(&self) -> &'static str {
        "WixComPlusExtension"
    }

    fn supported_namespaces(&self) -> &[&'static str] {
        &[WIX_COMPLUS_EXTENSION_NAMESPACE]
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
            "ComPlusApplication" => Self::compile_complus_application(node, parent_id, tables),
            "ComPlusAssembly" => Self::compile_complus_assembly(node, parent_id, tables),
            _ => Err(Error::WixExtension {
                extension: self.id().to_string(),
                message: format!("unsupported element: '{tag_name}'"),
            }),
        }
    }

    fn link_database(&self, db: &mut LinkedDatabase) -> Result<()> {
        let has_app = db
            .tables
            .get("WixComPlusApplication")
            .is_some_and(|t| !t.is_empty());
        let has_asm = db
            .tables
            .get("WixComPlusAssembly")
            .is_some_and(|t| !t.is_empty());

        if !has_app && !has_asm {
            return Ok(());
        }

        let ca_install = "WixComPlusExecInstall".to_string();
        let ca_rollback = "WixComPlusExecRollback".to_string();

        for ca_name in [&ca_install, &ca_rollback] {
            let _ = db.add_or_merge_record(
                "CustomAction",
                Record::with_fields(vec![
                    FieldValue::String(ca_name.clone()),
                    FieldValue::Long(38),
                    FieldValue::String(String::new()),
                    FieldValue::String("WScript.Quit 0".to_string()),
                ]),
            );
        }

        let _ = db.add_or_merge_record(
            "InstallExecuteSequence",
            Record::with_fields(vec![
                FieldValue::String(ca_install),
                FieldValue::String(String::new()),
                FieldValue::Long(6611),
            ]),
        );

        let _ = db.add_or_merge_record(
            "InstallExecuteSequence",
            Record::with_fields(vec![
                FieldValue::String(ca_rollback),
                FieldValue::String(String::new()),
                FieldValue::Long(6610),
            ]),
        );

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::wix::wixobj::SectionType;

    /// Tests `ComPlusExtension` constructor, traits, identifiers, and supported namespaces.
    #[test]
    #[allow(clippy::clone_on_copy, clippy::default_constructed_unit_structs)]
    fn test_complus_extension_id_and_namespaces() {
        let ext = ComPlusExtension::new();
        let ext_clone = ext;
        let _ = ext.clone();
        let _ = format!("{ext:?}");
        let _ = ComPlusExtension::default();
        assert_eq!(ext.id(), "WixComPlusExtension");
        assert_eq!(ext_clone.id(), "WixComPlusExtension");
        assert_eq!(
            ext.supported_namespaces(),
            &[WIX_COMPLUS_EXTENSION_NAMESPACE]
        );
    }

    /// Tests compiling `<complus:ComPlusApplication>` with attributes.
    #[test]
    fn test_compile_complus_application_success() {
        let ext = ComPlusExtension::new();
        let xml = r#"<complus:ComPlusApplication Id="App1" Name="MyApp" RunForever="yes" />"#;
        let parser = crate::wix::xml::XmlParser::new();
        let node = parser.parse(xml).unwrap_or_default();

        let mut section = IntermediateSection::new(SectionType::Product, Some("prod".to_string()));
        let mut tables = HashMap::new();

        ext.compile_node(&node, Some("cmp1"), &mut section, &mut tables)
            .unwrap_or_default();

        assert!(tables.contains_key("WixComPlusApplication"));
        let table = &tables["WixComPlusApplication"];
        assert_eq!(table.records.len(), 1);

        let fields = table.records[0].fields();
        assert_eq!(fields[0], FieldValue::String("App1".to_string()));
        assert_eq!(fields[1], FieldValue::String("MyApp".to_string()));
        assert_eq!(fields[2], FieldValue::Long(1));
        assert_eq!(fields[3], FieldValue::String("cmp1".to_string()));
    }

    /// Tests compiling `<complus:ComPlusApplication>` errors when required Id is missing.
    #[test]
    fn test_compile_complus_application_errors() {
        let ext = ComPlusExtension::new();
        let parser = crate::wix::xml::XmlParser::new();
        let mut section = IntermediateSection::new(SectionType::Product, None);

        let node_no_id = parser
            .parse(r#"<complus:ComPlusApplication Name="N" />"#)
            .unwrap_or_default();
        assert!(ext
            .compile_node(&node_no_id, Some("c"), &mut section, &mut HashMap::new())
            .is_err());
    }

    /// Tests compiling `<complus:ComPlusAssembly>` with attributes.
    #[test]
    fn test_compile_complus_assembly_success() {
        let ext = ComPlusExtension::new();
        let xml =
            r#"<complus:ComPlusAssembly Id="Asm1" DllPath="C:\App.dll" RegisterInCommit="yes" />"#;
        let parser = crate::wix::xml::XmlParser::new();
        let node = parser.parse(xml).unwrap_or_default();

        let mut section = IntermediateSection::new(SectionType::Product, Some("prod".to_string()));
        let mut tables = HashMap::new();

        ext.compile_node(&node, Some("cmp1"), &mut section, &mut tables)
            .unwrap_or_default();

        assert!(tables.contains_key("WixComPlusAssembly"));
        let table = &tables["WixComPlusAssembly"];
        assert_eq!(table.records.len(), 1);

        let fields = table.records[0].fields();
        assert_eq!(fields[0], FieldValue::String("Asm1".to_string()));
        assert_eq!(fields[1], FieldValue::String(r"C:\App.dll".to_string()));
        assert_eq!(fields[2], FieldValue::Long(1));
        assert_eq!(fields[3], FieldValue::String("cmp1".to_string()));
    }

    /// Tests compiling `<complus:ComPlusAssembly>` errors when required attributes are missing.
    #[test]
    fn test_compile_complus_assembly_errors() {
        let ext = ComPlusExtension::new();
        let parser = crate::wix::xml::XmlParser::new();
        let mut section = IntermediateSection::new(SectionType::Product, None);

        let node_no_id = parser
            .parse(r#"<complus:ComPlusAssembly DllPath="D" />"#)
            .unwrap_or_default();
        assert!(ext
            .compile_node(&node_no_id, Some("c"), &mut section, &mut HashMap::new())
            .is_err());

        let node_no_dll = parser
            .parse(r#"<complus:ComPlusAssembly Id="I" />"#)
            .unwrap_or_default();
        assert!(ext
            .compile_node(&node_no_dll, Some("c"), &mut section, &mut HashMap::new())
            .is_err());
    }

    /// Tests compiling unsupported elements returns [`Error::WixExtension`].
    #[test]
    fn test_unsupported_element() {
        let ext = ComPlusExtension::new();
        let xml = "<complus:Unsupported />";
        let parser = crate::wix::xml::XmlParser::new();
        let node = parser.parse(xml).unwrap_or_default();

        let mut section = IntermediateSection::new(SectionType::Product, None);
        let mut tables = HashMap::new();

        let res = ext.compile_node(&node, Some("cmp1"), &mut section, &mut tables);
        assert!(matches!(res, Err(Error::WixExtension { .. })));
    }

    /// Tests linking behavior with application or assembly records.
    #[test]
    fn test_link_database_stub() {
        let ext = ComPlusExtension::new();
        let mut db = LinkedDatabase::default();

        // Empty table test
        assert!(ext.link_database(&mut db).is_ok());

        // Add a complus app
        let _ = db.add_or_merge_record(
            "WixComPlusApplication",
            Record::with_fields(vec![
                FieldValue::String("App1".to_string()),
                FieldValue::String("MyApp".to_string()),
                FieldValue::Long(0),
                FieldValue::String("cmp1".to_string()),
            ]),
        );

        assert!(ext.link_database(&mut db).is_ok());

        // Verify CustomActions were injected
        assert!(db.tables.contains_key("CustomAction"));
        let ca_tbl = &db.tables["CustomAction"];
        assert!(ca_tbl
            .iter()
            .any(|r| r.fields()[0] == FieldValue::String("WixComPlusExecInstall".to_string())));

        // Test with only assembly (covers !has_app && !has_asm false branch where has_app is false)
        let mut db_asm_only = LinkedDatabase::default();
        let _ = db_asm_only.add_or_merge_record(
            "WixComPlusAssembly",
            Record::with_fields(vec![
                FieldValue::String("Asm1".to_string()),
                FieldValue::String(r"C:\App.dll".to_string()),
                FieldValue::Long(1),
                FieldValue::String("cmp1".to_string()),
            ]),
        );
        assert!(ext.link_database(&mut db_asm_only).is_ok());
        assert!(db_asm_only.tables.contains_key("CustomAction"));
    }
}

#![deny(clippy::unwrap_used)]
#![deny(missing_docs)]

//! `WixSqlExtension` backend implementation.
//!
//! Provides parsing and linker support for `http://schemas.microsoft.com/wix/SqlExtension`.

use crate::database::tables::record::{FieldValue, Record};
use crate::error::{Error, Result};
use crate::wix::extensions::WixExtension;
use crate::wix::linker::LinkedDatabase;
use crate::wix::wixobj::{IntermediateSection, IntermediateTable};
use crate::wix::xml::XmlNode;
use std::collections::HashMap;

/// Namespace URI for `WiX` Sql Extension.
pub const WIX_SQL_EXTENSION_NAMESPACE: &str = "http://schemas.microsoft.com/wix/SqlExtension";

/// Backend implementation for `WixSqlExtension`.
#[derive(Debug, Default, Clone, Copy)]
pub struct SqlExtension;

impl SqlExtension {
    /// Creates a new `SqlExtension` instance.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }

    /// Compiles a `<sql:SqlDatabase>` node into intermediate tables.
    ///
    /// # Errors
    /// Returns [`Error::WixCompiler`] if required attributes are missing.
    fn compile_sql_database(
        node: &XmlNode,
        parent_id: Option<&str>,
        tables: &mut HashMap<String, IntermediateTable>,
    ) -> Result<()> {
        let id = node.attribute("Id").ok_or_else(|| Error::WixCompiler {
            element: "sql:SqlDatabase".to_string(),
            message: "missing required 'Id' attribute".to_string(),
        })?;

        let database = node
            .attribute("Database")
            .ok_or_else(|| Error::WixCompiler {
                element: "sql:SqlDatabase".to_string(),
                message: "missing required 'Database' attribute".to_string(),
            })?;

        let server = node.attribute("Server").unwrap_or("localhost");

        let component = parent_id.unwrap_or("");

        let row = Record::with_fields(vec![
            FieldValue::String(id.to_string()),
            FieldValue::String(server.to_string()),
            FieldValue::String(database.to_string()),
            FieldValue::String(component.to_string()),
        ]);

        tables
            .entry("WixSqlDatabase".to_string())
            .or_insert_with(|| IntermediateTable::new("WixSqlDatabase"))
            .push_record(row);

        Ok(())
    }

    /// Compiles a `<sql:SqlScript>` node into intermediate tables.
    ///
    /// # Errors
    /// Returns [`Error::WixCompiler`] if required attributes are missing.
    fn compile_sql_script(
        node: &XmlNode,
        parent_id: Option<&str>,
        tables: &mut HashMap<String, IntermediateTable>,
    ) -> Result<()> {
        let id = node.attribute("Id").ok_or_else(|| Error::WixCompiler {
            element: "sql:SqlScript".to_string(),
            message: "missing required 'Id' attribute".to_string(),
        })?;

        let sql_db = node.attribute("SqlDb").ok_or_else(|| Error::WixCompiler {
            element: "sql:SqlScript".to_string(),
            message: "missing required 'SqlDb' attribute".to_string(),
        })?;

        let execute_on_install =
            i32::from(node.attribute("ExecuteOnInstall").unwrap_or("no") == "yes");

        let component = parent_id.unwrap_or("");

        let row = Record::with_fields(vec![
            FieldValue::String(id.to_string()),
            FieldValue::String(sql_db.to_string()),
            FieldValue::Long(execute_on_install),
            FieldValue::String(component.to_string()),
        ]);

        tables
            .entry("WixSqlScript".to_string())
            .or_insert_with(|| IntermediateTable::new("WixSqlScript"))
            .push_record(row);

        Ok(())
    }
}

impl WixExtension for SqlExtension {
    fn id(&self) -> &'static str {
        "WixSqlExtension"
    }

    fn supported_namespaces(&self) -> &[&'static str] {
        &[WIX_SQL_EXTENSION_NAMESPACE]
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
            "SqlDatabase" => Self::compile_sql_database(node, parent_id, tables),
            "SqlScript" => Self::compile_sql_script(node, parent_id, tables),
            _ => Err(Error::WixExtension {
                extension: self.id().to_string(),
                message: format!("unsupported element: '{tag_name}'"),
            }),
        }
    }

    fn link_database(&self, db: &mut LinkedDatabase) -> Result<()> {
        let has_db = db
            .tables
            .get("WixSqlDatabase")
            .is_some_and(|t| !t.is_empty());
        let has_script = db.tables.get("WixSqlScript").is_some_and(|t| !t.is_empty());

        if !has_db && !has_script {
            return Ok(());
        }

        let ca_install = "WixSqlExecInstall".to_string();
        let ca_rollback = "WixSqlExecRollback".to_string();

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
                FieldValue::Long(6610),
            ]),
        );

        let _ = db.add_or_merge_record(
            "InstallExecuteSequence",
            Record::with_fields(vec![
                FieldValue::String(ca_rollback),
                FieldValue::String(String::new()),
                FieldValue::Long(6609),
            ]),
        );

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::wix::wixobj::SectionType;

    /// Tests `SqlExtension` constructor, traits, identifiers, and supported namespaces.
    #[test]
    #[allow(clippy::clone_on_copy, clippy::default_constructed_unit_structs)]
    fn test_sql_extension_id_and_namespaces() {
        let ext = SqlExtension::new();
        let ext_clone = ext;
        let _ = ext.clone();
        let _ = format!("{ext:?}");
        let _ = SqlExtension::default();
        assert_eq!(ext.id(), "WixSqlExtension");
        assert_eq!(ext_clone.id(), "WixSqlExtension");
        assert_eq!(ext.supported_namespaces(), &[WIX_SQL_EXTENSION_NAMESPACE]);
    }

    /// Tests compiling `<sql:SqlDatabase>` with explicit attributes.
    #[test]
    fn test_compile_sql_database_success() {
        let ext = SqlExtension::new();
        let xml = r#"<sql:SqlDatabase Id="DB1" Database="MyDB" Server="MyServer" />"#;
        let parser = crate::wix::xml::XmlParser::new();
        let node = parser.parse(xml).unwrap_or_default();

        let mut section = IntermediateSection::new(SectionType::Product, Some("prod".to_string()));
        let mut tables = HashMap::new();

        ext.compile_node(&node, Some("cmp1"), &mut section, &mut tables)
            .unwrap_or_default();

        assert!(tables.contains_key("WixSqlDatabase"));
        let table = &tables["WixSqlDatabase"];
        assert_eq!(table.records.len(), 1);

        let fields = table.records[0].fields();
        assert_eq!(fields[0], FieldValue::String("DB1".to_string()));
        assert_eq!(fields[1], FieldValue::String("MyServer".to_string()));
        assert_eq!(fields[2], FieldValue::String("MyDB".to_string()));
        assert_eq!(fields[3], FieldValue::String("cmp1".to_string()));
    }

    /// Tests compiling `<sql:SqlDatabase>` error cases when required attributes are missing.
    #[test]
    fn test_compile_sql_database_errors() {
        let ext = SqlExtension::new();
        let parser = crate::wix::xml::XmlParser::new();
        let mut section = IntermediateSection::new(SectionType::Product, None);

        let node_no_id = parser
            .parse(r#"<sql:SqlDatabase Database="D" />"#)
            .unwrap_or_default();
        assert!(ext
            .compile_node(&node_no_id, Some("c"), &mut section, &mut HashMap::new())
            .is_err());

        let node_no_db = parser
            .parse(r#"<sql:SqlDatabase Id="I" />"#)
            .unwrap_or_default();
        assert!(ext
            .compile_node(&node_no_db, Some("c"), &mut section, &mut HashMap::new())
            .is_err());
    }

    /// Tests compiling `<sql:SqlScript>` with explicit attributes.
    #[test]
    fn test_compile_sql_script_success() {
        let ext = SqlExtension::new();
        let xml = r#"<sql:SqlScript Id="S1" SqlDb="DB1" ExecuteOnInstall="yes" />"#;
        let parser = crate::wix::xml::XmlParser::new();
        let node = parser.parse(xml).unwrap_or_default();

        let mut section = IntermediateSection::new(SectionType::Product, Some("prod".to_string()));
        let mut tables = HashMap::new();

        ext.compile_node(&node, Some("cmp1"), &mut section, &mut tables)
            .unwrap_or_default();

        assert!(tables.contains_key("WixSqlScript"));
        let table = &tables["WixSqlScript"];
        assert_eq!(table.records.len(), 1);

        let fields = table.records[0].fields();
        assert_eq!(fields[0], FieldValue::String("S1".to_string()));
        assert_eq!(fields[1], FieldValue::String("DB1".to_string()));
        assert_eq!(fields[2], FieldValue::Long(1));
        assert_eq!(fields[3], FieldValue::String("cmp1".to_string()));
    }

    /// Tests compiling `<sql:SqlScript>` error cases when required attributes are missing.
    #[test]
    fn test_compile_sql_script_errors() {
        let ext = SqlExtension::new();
        let parser = crate::wix::xml::XmlParser::new();
        let mut section = IntermediateSection::new(SectionType::Product, None);

        let node_no_id = parser
            .parse(r#"<sql:SqlScript SqlDb="D" />"#)
            .unwrap_or_default();
        assert!(ext
            .compile_node(&node_no_id, Some("c"), &mut section, &mut HashMap::new())
            .is_err());

        let node_no_db = parser
            .parse(r#"<sql:SqlScript Id="I" />"#)
            .unwrap_or_default();
        assert!(ext
            .compile_node(&node_no_db, Some("c"), &mut section, &mut HashMap::new())
            .is_err());
    }

    /// Tests compiling unsupported elements returns [`Error::WixExtension`].
    #[test]
    fn test_unsupported_element() {
        let ext = SqlExtension::new();
        let xml = "<sql:Unsupported />";
        let parser = crate::wix::xml::XmlParser::new();
        let node = parser.parse(xml).unwrap_or_default();

        let mut section = IntermediateSection::new(SectionType::Product, None);
        let mut tables = HashMap::new();

        let res = ext.compile_node(&node, Some("cmp1"), &mut section, &mut tables);
        assert!(matches!(res, Err(Error::WixExtension { .. })));
    }

    /// Tests linking behavior with databases or scripts.
    #[test]
    fn test_link_database_stub() {
        let ext = SqlExtension::new();
        let mut db = LinkedDatabase::default();

        // Empty table test
        assert!(ext.link_database(&mut db).is_ok());

        // Add a sql database
        let _ = db.add_or_merge_record(
            "WixSqlDatabase",
            Record::with_fields(vec![
                FieldValue::String("DB1".to_string()),
                FieldValue::String("localhost".to_string()),
                FieldValue::String("MyDB".to_string()),
                FieldValue::String("cmp1".to_string()),
            ]),
        );

        assert!(ext.link_database(&mut db).is_ok());

        // Verify CustomActions were injected
        assert!(db.tables.contains_key("CustomAction"));
        let ca_tbl = &db.tables["CustomAction"];
        assert!(ca_tbl
            .iter()
            .any(|r| r.fields()[0] == FieldValue::String("WixSqlExecInstall".to_string())));

        // Test with script only (covers !has_db && !has_script false branch where has_db is false)
        let mut db_script_only = LinkedDatabase::default();
        let _ = db_script_only.add_or_merge_record(
            "WixSqlScript",
            Record::with_fields(vec![
                FieldValue::String("S1".to_string()),
                FieldValue::String("DB1".to_string()),
                FieldValue::Long(1),
                FieldValue::String("cmp1".to_string()),
            ]),
        );
        assert!(ext.link_database(&mut db_script_only).is_ok());
        assert!(db_script_only.tables.contains_key("CustomAction"));
    }
}

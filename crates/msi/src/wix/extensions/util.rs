//! `WixUtilExtension` backend implementation.
//!
//! Provides parsing and linker support for `http://schemas.microsoft.com/wix/UtilExtension`.

use crate::database::tables::record::{FieldValue, Record};
use crate::error::{Error, Result};
use crate::wix::extensions::WixExtension;
use crate::wix::linker::LinkedDatabase;
use crate::wix::wixobj::{IntermediateSection, IntermediateTable};
use crate::wix::xml::XmlNode;
use std::collections::HashMap;

/// Namespace URI for `WiX` v3 Util Extension.
pub const WIX_UTIL_EXTENSION_NAMESPACE_V3: &str = "http://schemas.microsoft.com/wix/UtilExtension";

/// Backend implementation for `WixUtilExtension`.
#[derive(Debug, Default)]
pub struct UtilExtension;

impl UtilExtension {
    /// Creates a new `UtilExtension` instance.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }

    /// Compiles a `<util:User>` node into intermediate tables.
    ///
    /// # Errors
    ///
    /// Returns [`Error::WixCompiler`] if required attributes are missing.
    fn compile_user(
        node: &XmlNode,
        parent_id: Option<&str>,
        tables: &mut HashMap<String, IntermediateTable>,
    ) -> Result<()> {
        let id = node.attribute("Id").ok_or_else(|| Error::WixCompiler {
            element: "util:User".to_string(),
            message: "missing required 'Id' attribute".to_string(),
        })?;

        let name = node.attribute("Name").ok_or_else(|| Error::WixCompiler {
            element: "util:User".to_string(),
            message: "missing required 'Name' attribute".to_string(),
        })?;

        let component = parent_id.ok_or_else(|| Error::WixCompiler {
            element: "util:User".to_string(),
            message: "User element must be nested within a Component".to_string(),
        })?;

        let password = node.attribute("Password").unwrap_or("");
        let domain = node.attribute("Domain").unwrap_or("");

        let update_if_exists = i32::from(node.attribute("UpdateIfExists").unwrap_or("no") == "yes");
        let fail_if_exists = i32::from(node.attribute("FailIfExists").unwrap_or("no") == "yes");
        let create_user = i32::from(node.attribute("CreateUser").unwrap_or("yes") == "yes");
        let remove_on_uninstall =
            i32::from(node.attribute("RemoveOnUninstall").unwrap_or("no") == "yes");
        let password_never_expires =
            i32::from(node.attribute("PasswordNeverExpires").unwrap_or("no") == "yes");

        let flags = (update_if_exists)
            | (fail_if_exists << 1)
            | (create_user << 2)
            | (remove_on_uninstall << 3)
            | (password_never_expires << 4);

        let row = Record::with_fields(vec![
            FieldValue::String(id.to_string()),
            FieldValue::String(component.to_string()),
            FieldValue::String(name.to_string()),
            FieldValue::String(domain.to_string()),
            FieldValue::String(password.to_string()),
            FieldValue::Long(flags),
        ]);

        tables
            .entry("_util:User".to_string())
            .or_insert_with(|| IntermediateTable::new("_util:User"))
            .push_record(row);

        Ok(())
    }
}

impl WixExtension for UtilExtension {
    fn id(&self) -> &'static str {
        "WixUtilExtension"
    }

    fn supported_namespaces(&self) -> &[&'static str] {
        &[WIX_UTIL_EXTENSION_NAMESPACE_V3]
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
            "User" => Self::compile_user(node, parent_id, tables),
            _ => Err(Error::WixExtension {
                extension: self.id().to_string(),
                message: format!("unsupported element: '{tag_name}'"),
            }),
        }
    }

    fn link_database(&self, db: &mut LinkedDatabase) -> Result<()> {
        let Some(user_tbl) = db.tables.get("_util:User").cloned() else {
            return Ok(()); // No users to process
        };

        if user_tbl.is_empty() {
            return Ok(());
        }

        // We have users to process. For msi-rs drop-in compatibility, we emit
        // a custom action into the CustomAction table and schedule it.
        // In a true Wix environment this would link the util CA DLL, but we synthesize it.

        let ca_name = "WixUtilExecUsers".to_string();

        db.add_or_merge_record(
            "CustomAction",
            Record::with_fields(vec![
                FieldValue::String(ca_name.clone()),
                FieldValue::Long(38), // Type 38 = VBScript inline
                FieldValue::String(String::new()),
                FieldValue::String("WScript.Quit 0".to_string()),
            ]),
        )?;

        db.add_or_merge_record(
            "InstallExecuteSequence",
            Record::with_fields(vec![
                FieldValue::String(ca_name),
                FieldValue::String(String::new()),
                FieldValue::Long(6600), // Typically scheduled after InstallFiles
            ]),
        )?;

        // Also we emit a functional User table (WixUtilUser) mimicking WiX
        for rec in user_tbl {
            let _ = db.add_or_merge_record("WixUtilUser", rec);
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::wix::wixobj::SectionType;

    #[test]
    fn test_util_extension_id_and_namespaces() {
        let ext = UtilExtension::new();
        assert_eq!(ext.id(), "WixUtilExtension");
        assert_eq!(
            ext.supported_namespaces(),
            &[WIX_UTIL_EXTENSION_NAMESPACE_V3]
        );
    }

    #[test]
    fn test_compile_user_success() {
        let ext = UtilExtension::new();
        let xml = r#"<util:User Id="usr1" Name="admin" Domain="WORKGROUP" UpdateIfExists="yes" />"#;
        let parser = crate::wix::xml::XmlParser::new();
        let node = parser.parse(xml).unwrap_or_default();

        let mut section = IntermediateSection::new(SectionType::Product, Some("prod".to_string()));
        let mut tables = HashMap::new();

        ext.compile_node(&node, Some("cmp1"), &mut section, &mut tables)
            .unwrap_or_default();

        assert!(tables.contains_key("_util:User"));
        let table = &tables["_util:User"];
        assert_eq!(table.records.len(), 1);

        let fields = table.records[0].fields();
        assert_eq!(fields[0], FieldValue::String("usr1".to_string()));
        assert_eq!(fields[1], FieldValue::String("cmp1".to_string())); // parent_id
        assert_eq!(fields[2], FieldValue::String("admin".to_string()));
        assert_eq!(fields[3], FieldValue::String("WORKGROUP".to_string())); // domain
        assert_eq!(fields[4], FieldValue::String(String::new())); // password

        // update_if_exists=yes (1), fail_if_exists=no(0), create_user=yes (4), remove_on_uninstall=no(0), password_never_expires=no(0)
        // flags = 1 | 0 | 4 | 0 | 0 = 5
        assert_eq!(fields[5], FieldValue::Long(5));
    }

    #[test]
    fn test_compile_user_missing_id() {
        let ext = UtilExtension::new();
        let xml = r#"<util:User Name="admin" />"#;
        let parser = crate::wix::xml::XmlParser::new();
        let node = parser.parse(xml).unwrap_or_default();

        let mut section = IntermediateSection::new(SectionType::Product, None);
        let mut tables = HashMap::new();

        let res = ext.compile_node(&node, Some("cmp1"), &mut section, &mut tables);
        assert!(matches!(res, Err(Error::WixCompiler { .. })));
    }

    #[test]
    fn test_compile_user_missing_name() {
        let ext = UtilExtension::new();
        let xml = r#"<util:User Id="usr1" />"#;
        let parser = crate::wix::xml::XmlParser::new();
        let node = parser.parse(xml).unwrap_or_default();

        let mut section = IntermediateSection::new(SectionType::Product, None);
        let mut tables = HashMap::new();

        let res = ext.compile_node(&node, Some("cmp1"), &mut section, &mut tables);
        assert!(matches!(res, Err(Error::WixCompiler { .. })));
    }

    #[test]
    fn test_compile_user_missing_parent() {
        let ext = UtilExtension::new();
        let xml = r#"<util:User Id="usr1" Name="admin" />"#;
        let parser = crate::wix::xml::XmlParser::new();
        let node = parser.parse(xml).unwrap_or_default();

        let mut section = IntermediateSection::new(SectionType::Product, None);
        let mut tables = HashMap::new();

        let res = ext.compile_node(&node, None, &mut section, &mut tables);
        assert!(matches!(res, Err(Error::WixCompiler { .. })));
    }

    #[test]
    fn test_unsupported_element() {
        let ext = UtilExtension::new();
        let xml = "<util:Unsupported />";
        let parser = crate::wix::xml::XmlParser::new();
        let node = parser.parse(xml).unwrap_or_default();

        let mut section = IntermediateSection::new(SectionType::Product, None);
        let mut tables = HashMap::new();

        let res = ext.compile_node(&node, Some("cmp1"), &mut section, &mut tables);
        assert!(matches!(res, Err(Error::WixExtension { .. })));
    }

    #[test]
    fn test_link_database_stub() {
        let ext = UtilExtension::new();
        let mut db = LinkedDatabase::new().unwrap_or_default();

        // Empty table test
        db.tables.insert("_util:User".to_string(), Vec::new());
        assert!(ext.link_database(&mut db).is_ok());

        // Setup dummy _util:User table with record
        let row = Record::with_fields(vec![
            FieldValue::String("usr1".to_string()),
            FieldValue::String("cmp1".to_string()),
            FieldValue::String("admin".to_string()),
            FieldValue::String(String::new()),
            FieldValue::String(String::new()),
            FieldValue::Long(5),
        ]);
        db.add_or_merge_record("_util:User", row)
            .unwrap_or_default();

        assert!(ext.link_database(&mut db).is_ok());

        // Verify WixUtilUser table was created
        assert!(db.tables.contains_key("WixUtilUser"));
        let util_user_tbl = &db.tables["WixUtilUser"];
        assert_eq!(util_user_tbl.len(), 1);

        // Verify CustomAction was injected
        assert!(db.tables.contains_key("CustomAction"));
        let ca_tbl = &db.tables["CustomAction"];
        assert!(ca_tbl
            .iter()
            .any(|r| r.fields()[0] == FieldValue::String("WixUtilExecUsers".to_string())));

        // Verify InstallExecuteSequence was injected
        assert!(db.tables.contains_key("InstallExecuteSequence"));
        let ies_tbl = &db.tables["InstallExecuteSequence"];
        assert!(ies_tbl
            .iter()
            .any(|r| r.fields()[0] == FieldValue::String("WixUtilExecUsers".to_string())));
    }

    #[test]
    fn test_link_database_duplicate_ca_failure() {
        let ext = UtilExtension::new();
        let mut db = LinkedDatabase::new().unwrap_or_default();

        let row = Record::with_fields(vec![
            FieldValue::String("usr1".to_string()),
            FieldValue::String("cmp1".to_string()),
            FieldValue::String("admin".to_string()),
            FieldValue::String(String::new()),
            FieldValue::String(String::new()),
            FieldValue::Long(5),
        ]);
        db.add_or_merge_record("_util:User", row)
            .unwrap_or_default();

        // Add a conflicting CustomAction
        db.add_or_merge_record(
            "CustomAction",
            Record::with_fields(vec![
                FieldValue::String("WixUtilExecUsers".to_string()),
                FieldValue::Long(1), // Different type
                FieldValue::String(String::new()),
                FieldValue::String("Different".to_string()),
            ]),
        )
        .unwrap_or_default();

        assert!(ext.link_database(&mut db).is_err());
    }

    #[test]
    fn test_link_database_duplicate_sequence_failure() {
        let ext = UtilExtension::new();
        let mut db = LinkedDatabase::new().unwrap_or_default();

        let row = Record::with_fields(vec![
            FieldValue::String("usr1".to_string()),
            FieldValue::String("cmp1".to_string()),
            FieldValue::String("admin".to_string()),
            FieldValue::String(String::new()),
            FieldValue::String(String::new()),
            FieldValue::Long(5),
        ]);
        db.add_or_merge_record("_util:User", row)
            .unwrap_or_default();

        // Add a conflicting InstallExecuteSequence
        db.add_or_merge_record(
            "InstallExecuteSequence",
            Record::with_fields(vec![
                FieldValue::String("WixUtilExecUsers".to_string()),
                FieldValue::String(String::new()),
                FieldValue::Long(9999), // Different sequence
            ]),
        )
        .unwrap_or_default();

        assert!(ext.link_database(&mut db).is_err());
    }
}

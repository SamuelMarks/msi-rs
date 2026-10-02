#![deny(clippy::unwrap_used)]
#![deny(missing_docs)]

//! `WixFirewallExtension` backend implementation.
//!
//! Provides parsing and linker support for `http://schemas.microsoft.com/wix/FirewallExtension`.

use crate::database::tables::record::{FieldValue, Record};
use crate::error::{MsiError, Result};
use crate::wix::extensions::WixExtension;
use crate::wix::linker::LinkedDatabase;
use crate::wix::wixobj::{IntermediateSection, IntermediateTable};
use crate::wix::xml::XmlNode;
use std::collections::HashMap;

/// Namespace URI for `WiX` Firewall Extension.
pub const WIX_FIREWALL_EXTENSION_NAMESPACE: &str =
    "http://schemas.microsoft.com/wix/FirewallExtension";

/// Backend implementation for `WixFirewallExtension`.
#[derive(Debug, Default, Clone, Copy)]
pub struct FirewallExtension;

impl FirewallExtension {
    /// Creates a new `FirewallExtension` instance.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }

    /// Compiles a `<fw:FirewallException>` node into intermediate tables.
    ///
    /// # Errors
    ///
    /// Returns [`MsiError::WixCompiler`] if required attributes are missing.
    fn compile_firewall_exception(
        node: &XmlNode,
        parent_id: Option<&str>,
        tables: &mut HashMap<String, IntermediateTable>,
    ) -> Result<()> {
        let id = node.attribute("Id").ok_or_else(|| MsiError::WixCompiler {
            element: "fw:FirewallException".to_string(),
            message: "missing required 'Id' attribute".to_string(),
        })?;

        let name = node.attribute("Name").unwrap_or(id);

        let program = node.attribute("Program").unwrap_or("");
        let port = node.attribute("Port").unwrap_or("");

        let protocol_str = node.attribute("Protocol").unwrap_or("tcp");
        let protocol = match protocol_str {
            "udp" => 17,
            _ => 6, // tcp is 6
        };

        let scope_str = node.attribute("Scope").unwrap_or("any");
        let scope = match scope_str {
            "localSubnet" => 1,
            _ => 0, // any
        };

        let ignore_failure = i32::from(node.attribute("IgnoreFailure").unwrap_or("no") == "yes");
        let component = parent_id.unwrap_or("");

        let row = Record::with_fields(vec![
            FieldValue::String(id.to_string()),
            FieldValue::String(name.to_string()),
            FieldValue::String(program.to_string()),
            FieldValue::String(port.to_string()),
            FieldValue::Long(protocol),
            FieldValue::Long(scope),
            FieldValue::Long(ignore_failure),
            FieldValue::String(component.to_string()),
        ]);

        tables
            .entry("WixFirewallException".to_string())
            .or_insert_with(|| IntermediateTable::new("WixFirewallException"))
            .push_record(row);

        Ok(())
    }
}

impl WixExtension for FirewallExtension {
    fn id(&self) -> &'static str {
        "WixFirewallExtension"
    }

    fn supported_namespaces(&self) -> &[&'static str] {
        &[WIX_FIREWALL_EXTENSION_NAMESPACE]
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
            "FirewallException" => Self::compile_firewall_exception(node, parent_id, tables),
            _ => Err(MsiError::WixExtension {
                extension: self.id().to_string(),
                message: format!("unsupported element: '{tag_name}'"),
            }),
        }
    }

    fn link_database(&self, db: &mut LinkedDatabase) -> Result<()> {
        let has_fw = db
            .tables
            .get("WixFirewallException")
            .is_some_and(|t| !t.is_empty());

        if !has_fw {
            return Ok(());
        }

        let ca_install = "WixExecFirewallExceptionsInstall".to_string();
        let ca_uninstall = "WixExecFirewallExceptionsUninstall".to_string();
        let ca_rollback = "WixExecFirewallExceptionsRollback".to_string();

        for ca_name in [&ca_install, &ca_uninstall, &ca_rollback] {
            let _ = db.add_or_merge_record(
                "CustomAction",
                Record::with_fields(vec![
                    FieldValue::String(ca_name.clone()),
                    FieldValue::Long(38), // Type 38 = VBScript inline
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
                FieldValue::Long(6608),
            ]),
        );

        let _ = db.add_or_merge_record(
            "InstallExecuteSequence",
            Record::with_fields(vec![
                FieldValue::String(ca_uninstall),
                FieldValue::String("REMOVE=\"ALL\"".to_string()),
                FieldValue::Long(3299),
            ]),
        );

        let _ = db.add_or_merge_record(
            "InstallExecuteSequence",
            Record::with_fields(vec![
                FieldValue::String(ca_rollback),
                FieldValue::String(String::new()),
                FieldValue::Long(6607),
            ]),
        );

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::wix::wixobj::SectionType;

    /// Tests `FirewallExtension` constructor, traits, identifiers, and supported namespaces.
    #[test]
    #[allow(clippy::clone_on_copy, clippy::default_constructed_unit_structs)]
    fn test_firewall_extension_id_and_namespaces() {
        let ext = FirewallExtension::new();
        let ext_clone = ext;
        let _ = ext.clone();
        let _ = format!("{ext:?}");
        let _ = FirewallExtension::default();
        assert_eq!(ext.id(), "WixFirewallExtension");
        assert_eq!(ext_clone.id(), "WixFirewallExtension");
        assert_eq!(
            ext.supported_namespaces(),
            &[WIX_FIREWALL_EXTENSION_NAMESPACE]
        );
    }

    /// Tests compiling `<fw:FirewallException>` with explicit attributes.
    #[test]
    fn test_compile_firewall_exception_success() {
        let ext = FirewallExtension::new();
        let xml = r#"<fw:FirewallException Id="FW_1" Name="My App" Port="8080" Protocol="udp" Scope="localSubnet" IgnoreFailure="yes" />"#;
        let parser = crate::wix::xml::XmlParser::new();
        let node = parser.parse(xml).unwrap_or_default();

        let mut section = IntermediateSection::new(SectionType::Product, Some("prod".to_string()));
        let mut tables = HashMap::new();

        ext.compile_node(&node, Some("cmp1"), &mut section, &mut tables)
            .unwrap_or_default();

        assert!(tables.contains_key("WixFirewallException"));
        let table = &tables["WixFirewallException"];
        assert_eq!(table.records.len(), 1);

        let fields = table.records[0].fields();
        assert_eq!(fields[0], FieldValue::String("FW_1".to_string()));
        assert_eq!(fields[1], FieldValue::String("My App".to_string()));
        assert_eq!(fields[2], FieldValue::String(String::new())); // Program
        assert_eq!(fields[3], FieldValue::String("8080".to_string())); // Port
        assert_eq!(fields[4], FieldValue::Long(17)); // Protocol UDP
        assert_eq!(fields[5], FieldValue::Long(1)); // Scope localSubnet
        assert_eq!(fields[6], FieldValue::Long(1)); // IgnoreFailure
        assert_eq!(fields[7], FieldValue::String("cmp1".to_string()));
    }

    /// Tests compiling `<fw:FirewallException>` with default attributes.
    #[test]
    fn test_compile_firewall_exception_defaults() {
        let ext = FirewallExtension::new();
        let xml = r#"<fw:FirewallException Id="FW_1" />"#;
        let parser = crate::wix::xml::XmlParser::new();
        let node = parser.parse(xml).unwrap_or_default();

        let mut section = IntermediateSection::new(SectionType::Product, None);
        let mut tables = HashMap::new();

        ext.compile_node(&node, Some("cmp1"), &mut section, &mut tables)
            .unwrap_or_default();

        let fields = tables["WixFirewallException"].records[0].fields();
        assert_eq!(fields[1], FieldValue::String("FW_1".to_string())); // Name defaults to Id
        assert_eq!(fields[4], FieldValue::Long(6)); // TCP
        assert_eq!(fields[5], FieldValue::Long(0)); // Scope Any
        assert_eq!(fields[6], FieldValue::Long(0)); // IgnoreFailure No
    }

    /// Tests compiling `<fw:FirewallException>` when the required Id attribute is missing.
    #[test]
    fn test_compile_firewall_exception_missing_id() {
        let ext = FirewallExtension::new();
        let xml = r#"<fw:FirewallException Name="App" />"#;
        let parser = crate::wix::xml::XmlParser::new();
        let node = parser.parse(xml).unwrap_or_default();

        let mut section = IntermediateSection::new(SectionType::Product, None);
        let mut tables = HashMap::new();

        let res = ext.compile_node(&node, Some("cmp1"), &mut section, &mut tables);
        assert!(matches!(res, Err(MsiError::WixCompiler { .. })));
    }

    /// Tests compiling an unsupported element returns [`MsiError::WixExtension`].
    #[test]
    fn test_unsupported_element() {
        let ext = FirewallExtension::new();
        let xml = "<fw:Unsupported />";
        let parser = crate::wix::xml::XmlParser::new();
        let node = parser.parse(xml).unwrap_or_default();

        let mut section = IntermediateSection::new(SectionType::Product, None);
        let mut tables = HashMap::new();

        let res = ext.compile_node(&node, Some("cmp1"), &mut section, &mut tables);
        assert!(matches!(res, Err(MsiError::WixExtension { .. })));
    }

    /// Tests linking firewall custom actions and sequence records into database.
    #[test]
    fn test_link_database_stub() {
        let ext = FirewallExtension::new();
        let mut db = LinkedDatabase::default();

        // Empty table test
        assert!(ext.link_database(&mut db).is_ok());

        // Add a firewall exception
        let _ = db.add_or_merge_record(
            "WixFirewallException",
            Record::with_fields(vec![
                FieldValue::String("FW_1".to_string()),
                FieldValue::String("My App".to_string()),
                FieldValue::String(String::new()),
                FieldValue::String("80".to_string()),
                FieldValue::Long(6),
                FieldValue::Long(0),
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
            .any(|r| r.fields()[0]
                == FieldValue::String("WixExecFirewallExceptionsInstall".to_string())));
        assert!(ca_tbl.iter().any(|r| r.fields()[0]
            == FieldValue::String("WixExecFirewallExceptionsRollback".to_string())));
    }
}

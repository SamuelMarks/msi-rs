#![deny(clippy::unwrap_used)]
#![deny(missing_docs)]

//! `WixIisExtension` backend implementation.
//!
//! Provides parsing and linker support for `http://schemas.microsoft.com/wix/IIsExtension`.

use crate::database::tables::record::{FieldValue, Record};
use crate::error::{MsiError, Result};
use crate::wix::extensions::WixExtension;
use crate::wix::linker::LinkedDatabase;
use crate::wix::wixobj::{IntermediateSection, IntermediateTable};
use crate::wix::xml::XmlNode;
use std::collections::HashMap;

/// Namespace URI for `WiX` Iis Extension.
pub const WIX_IIS_EXTENSION_NAMESPACE: &str = "http://schemas.microsoft.com/wix/IIsExtension";

/// Backend implementation for `WixIisExtension`.
#[derive(Debug, Default, Clone, Copy)]
pub struct IisExtension;

impl IisExtension {
    /// Creates a new `IisExtension` instance.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }

    /// Compiles an `<iis:WebAppPool>` node into intermediate tables.
    ///
    /// # Errors
    /// Returns [`MsiError::WixCompiler`] if required attributes are missing.
    fn compile_web_app_pool(
        node: &XmlNode,
        parent_id: Option<&str>,
        tables: &mut HashMap<String, IntermediateTable>,
    ) -> Result<()> {
        let id = node.attribute("Id").ok_or_else(|| MsiError::WixCompiler {
            element: "iis:WebAppPool".to_string(),
            message: "missing required 'Id' attribute".to_string(),
        })?;

        let name = node.attribute("Name").unwrap_or(id);

        let identity_str = node.attribute("Identity").unwrap_or("networkService");
        let identity = match identity_str {
            "localSystem" => 1,
            "localService" => 2,
            "networkService" => 3,
            "applicationPoolIdentity" => 4,
            _ => 0, // other
        };

        let runtime = node.attribute("ManagedRuntimeVersion").unwrap_or("v4.0");
        let component = parent_id.unwrap_or("");

        let row = Record::with_fields(vec![
            FieldValue::String(id.to_string()),
            FieldValue::String(name.to_string()),
            FieldValue::Long(identity),
            FieldValue::String(runtime.to_string()),
            FieldValue::String(component.to_string()),
        ]);

        tables
            .entry("WixIisWebAppPool".to_string())
            .or_insert_with(|| IntermediateTable::new("WixIisWebAppPool"))
            .push_record(row);

        Ok(())
    }

    /// Compiles an `<iis:WebSite>` node into intermediate tables.
    ///
    /// # Errors
    /// Returns [`MsiError::WixCompiler`] if required attributes are missing.
    fn compile_web_site(
        node: &XmlNode,
        parent_id: Option<&str>,
        tables: &mut HashMap<String, IntermediateTable>,
    ) -> Result<()> {
        let id = node.attribute("Id").ok_or_else(|| MsiError::WixCompiler {
            element: "iis:WebSite".to_string(),
            message: "missing required 'Id' attribute".to_string(),
        })?;

        let description = node.attribute("Description").unwrap_or(id);

        let directory = node
            .attribute("Directory")
            .ok_or_else(|| MsiError::WixCompiler {
                element: "iis:WebSite".to_string(),
                message: "missing required 'Directory' attribute".to_string(),
            })?;

        let auto_start = i32::from(node.attribute("AutoStart").unwrap_or("yes") == "yes");
        let component = parent_id.unwrap_or("");

        let row = Record::with_fields(vec![
            FieldValue::String(id.to_string()),
            FieldValue::String(description.to_string()),
            FieldValue::String(directory.to_string()),
            FieldValue::Long(auto_start),
            FieldValue::String(component.to_string()),
        ]);

        tables
            .entry("WixIisWebSite".to_string())
            .or_insert_with(|| IntermediateTable::new("WixIisWebSite"))
            .push_record(row);

        Ok(())
    }
}

impl WixExtension for IisExtension {
    fn id(&self) -> &'static str {
        "WixIisExtension"
    }

    fn supported_namespaces(&self) -> &[&'static str] {
        &[WIX_IIS_EXTENSION_NAMESPACE]
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
            "WebAppPool" => Self::compile_web_app_pool(node, parent_id, tables),
            "WebSite" => Self::compile_web_site(node, parent_id, tables),
            _ => Err(MsiError::WixExtension {
                extension: self.id().to_string(),
                message: format!("unsupported element: '{tag_name}'"),
            }),
        }
    }

    fn link_database(&self, db: &mut LinkedDatabase) -> Result<()> {
        let has_pool = db
            .tables
            .get("WixIisWebAppPool")
            .is_some_and(|t| !t.is_empty());
        let has_site = db
            .tables
            .get("WixIisWebSite")
            .is_some_and(|t| !t.is_empty());

        if !has_pool && !has_site {
            return Ok(());
        }

        let ca_install = "WixIisExecInstall".to_string();
        let ca_rollback = "WixIisExecRollback".to_string();
        let ca_uninstall = "WixIisExecUninstall".to_string();

        for ca_name in [&ca_install, &ca_rollback, &ca_uninstall] {
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
                FieldValue::Long(6609),
            ]),
        );

        let _ = db.add_or_merge_record(
            "InstallExecuteSequence",
            Record::with_fields(vec![
                FieldValue::String(ca_rollback),
                FieldValue::String(String::new()),
                FieldValue::Long(6608),
            ]),
        );

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::wix::wixobj::SectionType;

    /// Tests `IisExtension` constructor, traits, identifiers, and supported namespaces.
    #[test]
    #[allow(clippy::clone_on_copy, clippy::default_constructed_unit_structs)]
    fn test_iis_extension_id_and_namespaces() {
        let ext = IisExtension::new();
        let ext_clone = ext;
        let _ = ext.clone();
        let _ = format!("{ext:?}");
        let _ = IisExtension::default();
        assert_eq!(ext.id(), "WixIisExtension");
        assert_eq!(ext_clone.id(), "WixIisExtension");
        assert_eq!(ext.supported_namespaces(), &[WIX_IIS_EXTENSION_NAMESPACE]);
    }

    /// Tests compiling `<iis:WebAppPool>` with explicit attributes and all identity variants.
    #[test]
    fn test_compile_web_app_pool_success() {
        let ext = IisExtension::new();
        let xml = r#"<iis:WebAppPool Id="Pool1" Name="MyPool" Identity="networkService" ManagedRuntimeVersion="v4.0" />"#;
        let parser = crate::wix::xml::XmlParser::new();
        let node = parser.parse(xml).unwrap_or_default();

        let mut section = IntermediateSection::new(SectionType::Product, Some("prod".to_string()));
        let mut tables = HashMap::new();

        ext.compile_node(&node, Some("cmp1"), &mut section, &mut tables)
            .unwrap_or_default();

        assert!(tables.contains_key("WixIisWebAppPool"));
        let table = &tables["WixIisWebAppPool"];
        assert_eq!(table.records.len(), 1);

        let fields = table.records[0].fields();
        assert_eq!(fields[0], FieldValue::String("Pool1".to_string()));
        assert_eq!(fields[1], FieldValue::String("MyPool".to_string()));
        assert_eq!(fields[2], FieldValue::Long(3));
        assert_eq!(fields[3], FieldValue::String("v4.0".to_string()));
        assert_eq!(fields[4], FieldValue::String("cmp1".to_string()));

        // Test all other identity options
        for (id, val, code) in [
            ("P_sys", "localSystem", 1),
            ("P_svc", "localService", 2),
            ("P_app", "applicationPoolIdentity", 4),
            ("P_oth", "custom", 0),
        ] {
            let alt_xml = format!(r#"<iis:WebAppPool Id="{id}" Identity="{val}" />"#);
            let alt_node = parser.parse(&alt_xml).unwrap_or_default();
            ext.compile_node(&alt_node, None, &mut section, &mut tables)
                .unwrap_or_default();
            let rec = tables["WixIisWebAppPool"]
                .records
                .last()
                .cloned()
                .unwrap_or_default();
            assert_eq!(rec.fields()[2], FieldValue::Long(code));
        }
    }

    /// Tests compiling `<iis:WebAppPool>` with default attributes.
    #[test]
    fn test_compile_web_app_pool_defaults() {
        let ext = IisExtension::new();
        let xml = r#"<iis:WebAppPool Id="Pool1" />"#;
        let parser = crate::wix::xml::XmlParser::new();
        let node = parser.parse(xml).unwrap_or_default();

        let mut section = IntermediateSection::new(SectionType::Product, None);
        let mut tables = HashMap::new();

        ext.compile_node(&node, None, &mut section, &mut tables)
            .unwrap_or_default();

        let fields = tables["WixIisWebAppPool"].records[0].fields();
        assert_eq!(fields[1], FieldValue::String("Pool1".to_string())); // Name defaults to Id
        assert_eq!(fields[2], FieldValue::Long(3)); // default networkService
        assert_eq!(fields[3], FieldValue::String("v4.0".to_string())); // default runtime
    }

    /// Tests compiling `<iis:WebSite>` with explicit attributes.
    #[test]
    fn test_compile_web_site_success() {
        let ext = IisExtension::new();
        let xml = r#"<iis:WebSite Id="Site1" Description="My Site" Directory="INSTALLDIR" AutoStart="no" />"#;
        let parser = crate::wix::xml::XmlParser::new();
        let node = parser.parse(xml).unwrap_or_default();

        let mut section = IntermediateSection::new(SectionType::Product, Some("prod".to_string()));
        let mut tables = HashMap::new();

        ext.compile_node(&node, Some("cmp1"), &mut section, &mut tables)
            .unwrap_or_default();

        assert!(tables.contains_key("WixIisWebSite"));
        let table = &tables["WixIisWebSite"];
        assert_eq!(table.records.len(), 1);

        let fields = table.records[0].fields();
        assert_eq!(fields[0], FieldValue::String("Site1".to_string()));
        assert_eq!(fields[1], FieldValue::String("My Site".to_string()));
        assert_eq!(fields[2], FieldValue::String("INSTALLDIR".to_string()));
        assert_eq!(fields[3], FieldValue::Long(0)); // auto start no
        assert_eq!(fields[4], FieldValue::String("cmp1".to_string()));
    }

    /// Tests compiling `<iis:WebSite>` error conditions when required attributes are missing.
    #[test]
    fn test_compile_web_site_errors() {
        let ext = IisExtension::new();
        let parser = crate::wix::xml::XmlParser::new();
        let mut section = IntermediateSection::new(SectionType::Product, None);

        let node_no_id = parser
            .parse(r#"<iis:WebSite Directory="D" />"#)
            .unwrap_or_default();
        assert!(ext
            .compile_node(&node_no_id, Some("c"), &mut section, &mut HashMap::new())
            .is_err());

        let node_no_dir = parser
            .parse(r#"<iis:WebSite Id="I" />"#)
            .unwrap_or_default();
        assert!(ext
            .compile_node(&node_no_dir, Some("c"), &mut section, &mut HashMap::new())
            .is_err());
    }

    /// Tests compiling `<iis:WebAppPool>` error conditions when required Id is missing.
    #[test]
    fn test_compile_web_app_pool_errors() {
        let ext = IisExtension::new();
        let parser = crate::wix::xml::XmlParser::new();
        let mut section = IntermediateSection::new(SectionType::Product, None);

        let node_no_id = parser
            .parse(r#"<iis:WebAppPool Name="N" />"#)
            .unwrap_or_default();
        assert!(ext
            .compile_node(&node_no_id, Some("c"), &mut section, &mut HashMap::new())
            .is_err());
    }

    /// Tests compiling unsupported elements returns [`MsiError::WixExtension`].
    #[test]
    fn test_unsupported_element() {
        let ext = IisExtension::new();
        let xml = "<iis:Unsupported />";
        let parser = crate::wix::xml::XmlParser::new();
        let node = parser.parse(xml).unwrap_or_default();

        let mut section = IntermediateSection::new(SectionType::Product, None);
        let mut tables = HashMap::new();

        let res = ext.compile_node(&node, Some("cmp1"), &mut section, &mut tables);
        assert!(matches!(res, Err(MsiError::WixExtension { .. })));
    }

    /// Tests linking behavior with web sites or app pools.
    #[test]
    fn test_link_database_stub() {
        let ext = IisExtension::new();
        let mut db = LinkedDatabase::default();

        // Empty table test
        assert!(ext.link_database(&mut db).is_ok());

        // Add a web site
        let _ = db.add_or_merge_record(
            "WixIisWebSite",
            Record::with_fields(vec![
                FieldValue::String("Site1".to_string()),
                FieldValue::String("My Site".to_string()),
                FieldValue::String("DIR".to_string()),
                FieldValue::Long(1),
                FieldValue::String("cmp1".to_string()),
            ]),
        );

        assert!(ext.link_database(&mut db).is_ok());

        // Verify CustomActions were injected
        assert!(db.tables.contains_key("CustomAction"));
        let ca_tbl = &db.tables["CustomAction"];
        assert!(ca_tbl
            .iter()
            .any(|r| r.fields()[0] == FieldValue::String("WixIisExecInstall".to_string())));

        // Test with app pool only (covers !has_pool && !has_site false branch where has_pool is true)
        let mut db_pool_only = LinkedDatabase::default();
        let _ = db_pool_only.add_or_merge_record(
            "WixIisWebAppPool",
            Record::with_fields(vec![
                FieldValue::String("Pool1".to_string()),
                FieldValue::String("Pool1".to_string()),
                FieldValue::Long(3),
                FieldValue::String("v4.0".to_string()),
                FieldValue::String("cmp1".to_string()),
            ]),
        );
        assert!(ext.link_database(&mut db_pool_only).is_ok());
        assert!(db_pool_only.tables.contains_key("CustomAction"));
    }
}

#![deny(clippy::unwrap_used)]
#![deny(missing_docs)]

//! `WixHttpExtension` backend implementation.
//!
//! Provides parsing and linker support for `http://schemas.microsoft.com/wix/HttpExtension`.

use crate::database::tables::record::{FieldValue, Record};
use crate::error::{Error, Result};
use crate::wix::extensions::WixExtension;
use crate::wix::linker::LinkedDatabase;
use crate::wix::wixobj::{IntermediateSection, IntermediateTable};
use crate::wix::xml::XmlNode;
use std::collections::HashMap;

/// Namespace URI for `WiX` Http Extension.
pub const WIX_HTTP_EXTENSION_NAMESPACE: &str = "http://schemas.microsoft.com/wix/HttpExtension";

/// Backend implementation for `WixHttpExtension`.
#[derive(Debug, Default, Clone, Copy)]
pub struct HttpExtension;

impl HttpExtension {
    /// Creates a new `HttpExtension` instance.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }

    /// Compiles a `<http:UrlReservation>` node into intermediate tables.
    ///
    /// # Errors
    ///
    /// Returns [`Error::WixCompiler`] if required attributes are missing.
    fn compile_url_reservation(
        node: &XmlNode,
        parent_id: Option<&str>,
        tables: &mut HashMap<String, IntermediateTable>,
    ) -> Result<()> {
        let url = node.attribute("Url").ok_or_else(|| Error::WixCompiler {
            element: "http:UrlReservation".to_string(),
            message: "missing required 'Url' attribute".to_string(),
        })?;

        let id = node.attribute("Id").unwrap_or(url); // Often falls back to URL if missing

        let handle_existing_str = node.attribute("HandleExisting").unwrap_or("replace");
        let handle_existing = match handle_existing_str {
            "ignore" => 1,
            "fail" => 2,
            _ => 0, // replace
        };

        let rights_str = node.attribute("Rights").unwrap_or("all");
        let rights = match rights_str {
            "execute" => 1,
            "delegate" => 2,
            _ => 0, // all
        };

        let component = parent_id.unwrap_or("");

        let row = Record::with_fields(vec![
            FieldValue::String(id.to_string()),
            FieldValue::String(url.to_string()),
            FieldValue::Long(handle_existing),
            FieldValue::Long(rights),
            FieldValue::String(component.to_string()),
        ]);

        tables
            .entry("WixHttpUrlReservation".to_string())
            .or_insert_with(|| IntermediateTable::new("WixHttpUrlReservation"))
            .push_record(row);

        Ok(())
    }

    /// Compiles a `<http:CertificateRef>` node.
    ///
    /// # Errors
    ///
    /// Returns [`Error::WixCompiler`] if required attributes are missing.
    fn compile_certificate_ref(
        node: &XmlNode,
        parent_id: Option<&str>,
        tables: &mut HashMap<String, IntermediateTable>,
    ) -> Result<()> {
        let id = node.attribute("Id").ok_or_else(|| Error::WixCompiler {
            element: "http:CertificateRef".to_string(),
            message: "missing required 'Id' attribute".to_string(),
        })?;

        let component = parent_id.unwrap_or("");

        let row = Record::with_fields(vec![
            FieldValue::String(id.to_string()),
            FieldValue::String(component.to_string()),
        ]);

        tables
            .entry("WixHttpCertificateRef".to_string())
            .or_insert_with(|| IntermediateTable::new("WixHttpCertificateRef"))
            .push_record(row);

        Ok(())
    }
}

impl WixExtension for HttpExtension {
    fn id(&self) -> &'static str {
        "WixHttpExtension"
    }

    fn supported_namespaces(&self) -> &[&'static str] {
        &[WIX_HTTP_EXTENSION_NAMESPACE]
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
            "UrlReservation" => Self::compile_url_reservation(node, parent_id, tables),
            "CertificateRef" => Self::compile_certificate_ref(node, parent_id, tables),
            _ => Err(Error::WixExtension {
                extension: self.id().to_string(),
                message: format!("unsupported element: '{tag_name}'"),
            }),
        }
    }

    fn link_database(&self, db: &mut LinkedDatabase) -> Result<()> {
        let has_url = db
            .tables
            .get("WixHttpUrlReservation")
            .is_some_and(|t| !t.is_empty());
        let has_cert = db
            .tables
            .get("WixHttpCertificateRef")
            .is_some_and(|t| !t.is_empty());

        if !has_url && !has_cert {
            return Ok(());
        }

        let ca_name = "WixHttpExecConfig".to_string();

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

    /// Tests `HttpExtension` constructor, traits, identifiers, and supported namespaces.
    #[test]
    #[allow(clippy::clone_on_copy, clippy::default_constructed_unit_structs)]
    fn test_http_extension_id_and_namespaces() {
        let ext = HttpExtension::new();
        let ext_clone = ext;
        let _ = ext.clone();
        let _ = format!("{ext:?}");
        let _ = HttpExtension::default();
        assert_eq!(ext.id(), "WixHttpExtension");
        assert_eq!(ext_clone.id(), "WixHttpExtension");
        assert_eq!(ext.supported_namespaces(), &[WIX_HTTP_EXTENSION_NAMESPACE]);
    }

    /// Tests compiling `<http:UrlReservation>` with explicit attributes.
    #[test]
    fn test_compile_url_reservation_success() {
        let ext = HttpExtension::new();
        let xml = r#"<http:UrlReservation Id="Res1" Url="http://+:80/" HandleExisting="ignore" Rights="execute" />"#;
        let parser = crate::wix::xml::XmlParser::new();
        let node = parser.parse(xml).unwrap_or_default();

        let mut section = IntermediateSection::new(SectionType::Product, Some("prod".to_string()));
        let mut tables = HashMap::new();

        ext.compile_node(&node, Some("cmp1"), &mut section, &mut tables)
            .unwrap_or_default();

        assert!(tables.contains_key("WixHttpUrlReservation"));
        let table = &tables["WixHttpUrlReservation"];
        assert_eq!(table.records.len(), 1);

        let fields = table.records[0].fields();
        assert_eq!(fields[0], FieldValue::String("Res1".to_string()));
        assert_eq!(fields[1], FieldValue::String("http://+:80/".to_string()));
        assert_eq!(fields[2], FieldValue::Long(1)); // ignore
        assert_eq!(fields[3], FieldValue::Long(1)); // execute
        assert_eq!(fields[4], FieldValue::String("cmp1".to_string()));
    }

    /// Tests compiling `<http:UrlReservation>` with default attributes.
    #[test]
    fn test_compile_url_reservation_defaults() {
        let ext = HttpExtension::new();
        let xml = r#"<http:UrlReservation Url="http://+:8080/" />"#;
        let parser = crate::wix::xml::XmlParser::new();
        let node = parser.parse(xml).unwrap_or_default();

        let mut section = IntermediateSection::new(SectionType::Product, None);
        let mut tables = HashMap::new();

        ext.compile_node(&node, Some("cmp1"), &mut section, &mut tables)
            .unwrap_or_default();

        let fields = tables["WixHttpUrlReservation"].records[0].fields();
        assert_eq!(fields[0], FieldValue::String("http://+:8080/".to_string())); // id falls back to url
        assert_eq!(fields[2], FieldValue::Long(0)); // default handle existing
        assert_eq!(fields[3], FieldValue::Long(0)); // default rights

        // Also test HandleExisting="fail" and Rights="delegate"
        let xml_alt = r#"<http:UrlReservation Id="ResAlt" Url="http://+:8081/" HandleExisting="fail" Rights="delegate" />"#;
        let node_alt = parser.parse(xml_alt).unwrap_or_default();
        ext.compile_node(&node_alt, Some("cmp2"), &mut section, &mut tables)
            .unwrap_or_default();
        let fields_alt = tables["WixHttpUrlReservation"].records[1].fields();
        assert_eq!(fields_alt[2], FieldValue::Long(2)); // fail
        assert_eq!(fields_alt[3], FieldValue::Long(2)); // delegate
    }

    /// Tests compiling `<http:UrlReservation>` when missing required Url attribute returns error.
    #[test]
    fn test_compile_url_reservation_missing_url() {
        let ext = HttpExtension::new();
        let xml = r#"<http:UrlReservation Id="Res1" />"#;
        let parser = crate::wix::xml::XmlParser::new();
        let node = parser.parse(xml).unwrap_or_default();

        let mut section = IntermediateSection::new(SectionType::Product, None);
        let mut tables = HashMap::new();

        let res = ext.compile_node(&node, Some("cmp1"), &mut section, &mut tables);
        assert!(matches!(res, Err(Error::WixCompiler { .. })));
    }

    /// Tests compiling `<http:CertificateRef>` with explicit attributes.
    #[test]
    fn test_compile_certificate_ref_success() {
        let ext = HttpExtension::new();
        let xml = r#"<http:CertificateRef Id="Cert1" />"#;
        let parser = crate::wix::xml::XmlParser::new();
        let node = parser.parse(xml).unwrap_or_default();

        let mut section = IntermediateSection::new(SectionType::Product, None);
        let mut tables = HashMap::new();

        ext.compile_node(&node, Some("cmp1"), &mut section, &mut tables)
            .unwrap_or_default();

        let fields = tables["WixHttpCertificateRef"].records[0].fields();
        assert_eq!(fields[0], FieldValue::String("Cert1".to_string()));
        assert_eq!(fields[1], FieldValue::String("cmp1".to_string()));
    }

    /// Tests compiling `<http:CertificateRef>` when missing Id returns error.
    #[test]
    fn test_compile_certificate_ref_missing_id() {
        let ext = HttpExtension::new();
        let xml = "<http:CertificateRef />";
        let parser = crate::wix::xml::XmlParser::new();
        let node = parser.parse(xml).unwrap_or_default();

        let mut section = IntermediateSection::new(SectionType::Product, None);
        let mut tables = HashMap::new();

        let res = ext.compile_node(&node, Some("cmp1"), &mut section, &mut tables);
        assert!(matches!(res, Err(Error::WixCompiler { .. })));
    }

    /// Tests compiling unsupported elements returns [`Error::WixExtension`].
    #[test]
    fn test_unsupported_element() {
        let ext = HttpExtension::new();
        let xml = "<http:Unsupported />";
        let parser = crate::wix::xml::XmlParser::new();
        let node = parser.parse(xml).unwrap_or_default();

        let mut section = IntermediateSection::new(SectionType::Product, None);
        let mut tables = HashMap::new();

        let res = ext.compile_node(&node, Some("cmp1"), &mut section, &mut tables);
        assert!(matches!(res, Err(Error::WixExtension { .. })));
    }

    /// Tests linking behavior of `HttpExtension` with reservations and certificates.
    #[test]
    fn test_link_database_stub() {
        let ext = HttpExtension::new();
        let mut db = LinkedDatabase::default();

        // Empty table test
        assert!(ext.link_database(&mut db).is_ok());

        // Add a url reservation
        let _ = db.add_or_merge_record(
            "WixHttpUrlReservation",
            Record::with_fields(vec![
                FieldValue::String("Res1".to_string()),
                FieldValue::String("http://+:80/".to_string()),
                FieldValue::Long(0),
                FieldValue::Long(0),
                FieldValue::String("cmp1".to_string()),
            ]),
        );

        assert!(ext.link_database(&mut db).is_ok());

        // Verify CustomAction was injected
        assert!(db.tables.contains_key("CustomAction"));
        let ca_tbl = &db.tables["CustomAction"];
        assert!(ca_tbl
            .iter()
            .any(|r| r.fields()[0] == FieldValue::String("WixHttpExecConfig".to_string())));

        // Verify InstallExecuteSequence was injected
        assert!(db.tables.contains_key("InstallExecuteSequence"));
        let ies_tbl = &db.tables["InstallExecuteSequence"];
        assert!(ies_tbl
            .iter()
            .any(|r| r.fields()[0] == FieldValue::String("WixHttpExecConfig".to_string())));

        // Test with cert only (covers !has_url && !has_cert false branch where has_url is false)
        let mut db_cert_only = LinkedDatabase::default();
        let _ = db_cert_only.add_or_merge_record(
            "WixHttpCertificateRef",
            Record::with_fields(vec![
                FieldValue::String("Cert1".to_string()),
                FieldValue::String("cmp1".to_string()),
            ]),
        );
        assert!(ext.link_database(&mut db_cert_only).is_ok());
        assert!(db_cert_only.tables.contains_key("CustomAction"));
    }
}

//! `WiX` compiler pipeline (`candle` architecture).
//!
//! Compiles preprocessed `WiX` XML documents into intermediate object models (`.wixobj`),
//! generating symbols, unresolved references, and intermediate database tables.

use crate::database::tables::config::{EnvironmentRow, RegistryRow};
use crate::database::tables::core::{
    ComponentRow, DirectoryRow, FeatureComponentsRow, FeatureRow, FileRow, MediaRow, PropertyRow,
};
use crate::database::tables::file_mgmt::{CreateFolderRow, RemoveFileRow};
use crate::database::tables::record::{FieldValue, Record};
use crate::database::tables::sequence::SequenceRow;
use crate::database::tables::types::{
    ComponentGuid, ComponentName, DirectoryId, FeatureName, FileKey, PropertyName,
};
use crate::error::{Error, Result};
use crate::wix::schema::WixSchemaVersion;
use crate::wix::wixobj::{
    IntermediateSection, IntermediateTable, Reference, SectionType, Symbol, WixObject,
};
use crate::wix::xml::XmlNode;

/// Converts plaintext to well-formed RTF document with Unicode escape sequences.
///
/// If input already begins with `{\rtf`, returns the input unchanged.
///
/// # Arguments
///
/// * `input` - Raw plaintext or RTF string.
///
/// # Returns
///
/// Valid RTF string.
#[must_use]
#[allow(clippy::cast_possible_truncation, clippy::cast_possible_wrap)]
pub fn convert_text_to_rtf(input: &str) -> String {
    if input.trim_start().starts_with(r"{\rtf") {
        return input.to_string();
    }

    let mut out = String::from(
        r"{\rtf1\ansi\ansicpg1252\deff0\nouicompat{\fonttbl{\f0\fnil\fcharset0 Arial;}}{\colortbl ;\red0\green0\blue0;}\viewkind4\uc1\pard\cf1\f0\fs20 ",
    );
    for ch in input.chars() {
        match ch {
            '\\' => out.push_str(r"\\"),
            '{' => out.push_str(r"\{"),
            '}' => out.push_str(r"\}"),
            '\r' => {}
            '\n' => out.push_str(r"\par "),
            c if (c as u32) < 128 => out.push(c),
            c => {
                let code = c as u32;
                if code <= 0xFFFF {
                    let _ =
                        std::fmt::Write::write_fmt(&mut out, format_args!(r"\u{}?", code as i16));
                } else {
                    let high = 0xD800 + ((code - 0x10000) >> 10);
                    let low = 0xDC00 + ((code - 0x10000) & 0x3FF);
                    let _ = std::fmt::Write::write_fmt(
                        &mut out,
                        format_args!(r"\u{}?\u{}?", high as i16, low as i16),
                    );
                }
            }
        }
    }
    out.push_str(r"\par}");
    out
}

/// `WiX` compiler translating XML documents into [`WixObject`].
#[derive(Debug, Default)]
pub struct Compiler;

impl Compiler {
    /// Creates a new [`Compiler`].
    ///
    /// # Returns
    ///
    /// A new compiler.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }

    /// Compiles a root XML document into a [`WixObject`].
    ///
    /// # Arguments
    ///
    /// * `root` - The root [`XmlNode`] (typically `<Wix>`).
    ///
    /// # Returns
    ///
    /// Compiled [`WixObject`] containing sections, symbols, references, and table records.
    ///
    /// # Errors
    ///
    /// Returns [`Error::WixCompiler`] if any mandatory attribute or element structure is invalid.
    pub fn compile(&self, root: &XmlNode) -> Result<WixObject> {
        let mut obj = WixObject::new();

        // Check namespace if present
        if let Some(ns) = root.attribute("xmlns") {
            let _ = WixSchemaVersion::from_uri(ns)?;
        }

        // Process top-level sections
        for child in &root.children {
            match child.tag.as_str() {
                "Product" | "Package" => {
                    let sec = self.compile_section(child, SectionType::Product)?;
                    obj.add_section(sec);
                }
                "Module" => {
                    let sec = self.compile_section(child, SectionType::Module)?;
                    obj.add_section(sec);
                }
                "Fragment" => {
                    let sec = self.compile_section(child, SectionType::Fragment)?;
                    obj.add_section(sec);
                }
                "PatchCreation" => {
                    let sec = self.compile_section(child, SectionType::PatchCreation)?;
                    obj.add_section(sec);
                }
                "Patch" => {
                    let sec = self.compile_section(child, SectionType::Patch)?;
                    obj.add_section(sec);
                }
                _ => {}
            }
        }

        Ok(obj)
    }

    /// Compiles an individual section element (e.g. `<Product>`, `<Fragment>`).
    #[allow(clippy::too_many_lines)]
    fn compile_section(
        &self,
        node: &XmlNode,
        sec_type: SectionType,
    ) -> Result<IntermediateSection> {
        let id = node.attribute("Id").map(ToString::to_string);
        let mut section = IntermediateSection::new(sec_type, id.clone());

        if let Some(ref sec_id) = id {
            section.add_symbol(Symbol::new(format!("{sec_type}"), sec_id));
        }

        // Tables map
        let mut tables_map: std::collections::HashMap<String, IntermediateTable> =
            std::collections::HashMap::new();

        // Extract Product attributes as Property rows
        if sec_type == SectionType::Product {
            let prop_table = tables_map
                .entry("Property".to_string())
                .or_insert_with(|| IntermediateTable::new("Property"));

            if let Some(ver) = node.attribute("Version") {
                let p = PropertyRow {
                    property: PropertyName::new("ProductVersion")?,
                    value: ver.to_string(),
                };
                prop_table.push_record(p.to_record());
            }
            if let Some(name) = node.attribute("Name") {
                let p = PropertyRow {
                    property: PropertyName::new("ProductName")?,
                    value: name.to_string(),
                };
                prop_table.push_record(p.to_record());
            }
            if let Some(mfg) = node.attribute("Manufacturer") {
                let p = PropertyRow {
                    property: PropertyName::new("Manufacturer")?,
                    value: mfg.to_string(),
                };
                prop_table.push_record(p.to_record());
            }
            if let Some(code) = node
                .attribute("ProductCode")
                .or_else(|| node.attribute("Id"))
            {
                let p = PropertyRow {
                    property: PropertyName::new("ProductCode")?,
                    value: code.to_string(),
                };
                prop_table.push_record(p.to_record());
            }
            if let Some(upg) = node.attribute("UpgradeCode") {
                let p = PropertyRow {
                    property: PropertyName::new("UpgradeCode")?,
                    value: upg.to_string(),
                };
                prop_table.push_record(p.to_record());
            }
            if let Some(lang) = node
                .attribute("Language")
                .or_else(|| node.attribute("Languages"))
            {
                let p = PropertyRow {
                    property: PropertyName::new("ProductLanguage")?,
                    value: lang.to_string(),
                };
                prop_table.push_record(p.to_record());
            }
            if let Some(cp) = node
                .attribute("Codepage")
                .or_else(|| node.attribute("SummaryCodepage"))
            {
                let p = PropertyRow {
                    property: PropertyName::new("ProductCodepage")?,
                    value: cp.to_string(),
                };
                prop_table.push_record(p.to_record());
            }
            if let Some(iv) = node.attribute("InstallerVersion") {
                let p = PropertyRow {
                    property: PropertyName::new("InstallerVersion")?,
                    value: iv.to_string(),
                };
                prop_table.push_record(p.to_record());
            }
            if let Some(cmp) = node.attribute("Compressed") {
                let p = PropertyRow {
                    property: PropertyName::new("ProductCompressed")?,
                    value: cmp.to_string(),
                };
                prop_table.push_record(p.to_record());
            }
            if let Some(desc) = node.attribute("Description") {
                let p = PropertyRow {
                    property: PropertyName::new("ProductDescription")?,
                    value: desc.to_string(),
                };
                prop_table.push_record(p.to_record());
            }
            if let Some(comm) = node.attribute("Comments") {
                let p = PropertyRow {
                    property: PropertyName::new("ProductComments")?,
                    value: comm.to_string(),
                };
                prop_table.push_record(p.to_record());
            }
            if let Some(kw) = node.attribute("Keywords") {
                let p = PropertyRow {
                    property: PropertyName::new("ProductKeywords")?,
                    value: kw.to_string(),
                };
                prop_table.push_record(p.to_record());
            }
            if let Some(plt) = node.attribute("Platform") {
                let p = PropertyRow {
                    property: PropertyName::new("ProductPlatform")?,
                    value: plt.to_string(),
                };
                prop_table.push_record(p.to_record());
            }
        }

        // Extract Module attributes as Property rows and ModuleSignature
        if sec_type == SectionType::Module {
            let mod_id = node
                .attribute("Id")
                .or_else(|| node.attribute("Guid"))
                .unwrap_or("Module");
            let lang_val: i16 = node
                .attribute("Language")
                .and_then(|l| l.parse().ok())
                .unwrap_or(1033);
            let ver_val = node.attribute("Version").unwrap_or("1.0.0");

            let sig_row = crate::database::tables::core::ModuleSignatureRow {
                module_id: mod_id.to_string(),
                language: lang_val,
                version: ver_val.to_string(),
            };
            tables_map
                .entry("ModuleSignature".to_string())
                .or_insert_with(|| IntermediateTable::new("ModuleSignature"))
                .push_record(sig_row.to_record());

            let prop_table = tables_map
                .entry("Property".to_string())
                .or_insert_with(|| IntermediateTable::new("Property"));

            if let Some(ver) = node.attribute("Version") {
                let p = PropertyRow {
                    property: PropertyName::new("ModuleVersion")?,
                    value: ver.to_string(),
                };
                prop_table.push_record(p.to_record());
            }
            if let Some(lang) = node.attribute("Language") {
                let p = PropertyRow {
                    property: PropertyName::new("ModuleLanguage")?,
                    value: lang.to_string(),
                };
                prop_table.push_record(p.to_record());
            }
            if let Some(mfg) = node.attribute("Manufacturer") {
                let p = PropertyRow {
                    property: PropertyName::new("ModuleManufacturer")?,
                    value: mfg.to_string(),
                };
                prop_table.push_record(p.to_record());
            }
            if let Some(guid) = node.attribute("Guid").or_else(|| node.attribute("Id")) {
                let p = PropertyRow {
                    property: PropertyName::new("ModuleId")?,
                    value: guid.to_string(),
                };
                prop_table.push_record(p.to_record());
            }
            if let Some(cp) = node
                .attribute("Codepage")
                .or_else(|| node.attribute("SummaryCodepage"))
            {
                let p = PropertyRow {
                    property: PropertyName::new("ModuleCodepage")?,
                    value: cp.to_string(),
                };
                prop_table.push_record(p.to_record());
            }
        }

        // Recursively walk elements in section
        self.compile_element_tree(node, None, &mut section, &mut tables_map)?;

        // Attach ModuleComponents for all compiled components
        if sec_type == SectionType::Module {
            let mod_id = node
                .attribute("Id")
                .or_else(|| node.attribute("Guid"))
                .unwrap_or("Module");
            let lang_val: i16 = node
                .attribute("Language")
                .and_then(|l| l.parse().ok())
                .unwrap_or(1033);

            if let Some(comp_table) = tables_map.get("Component") {
                let comp_names: Vec<String> = comp_table
                    .records
                    .iter()
                    .map(|r| r.fields()[0].to_string().trim_matches('\'').to_string())
                    .collect();
                let mod_comp_table = tables_map
                    .entry("ModuleComponents".to_string())
                    .or_insert_with(|| IntermediateTable::new("ModuleComponents"));
                for c_name in comp_names {
                    let mc_row = crate::database::tables::core::ModuleComponentsRow {
                        component: c_name,
                        module_id: mod_id.to_string(),
                        language: lang_val,
                    };
                    mod_comp_table.push_record(mc_row.to_record());
                }
            }
        }

        // Attach tables to section
        for (_, tbl) in tables_map {
            section.add_table(tbl);
        }

        Ok(section)
    }

    /// Recursively walks and compiles an XML element subtree.
    #[allow(clippy::too_many_lines, clippy::self_only_used_in_recursion)]
    fn compile_element_tree(
        &self,
        node: &XmlNode,
        parent_id: Option<&str>,
        section: &mut IntermediateSection,
        tables: &mut std::collections::HashMap<String, IntermediateTable>,
    ) -> Result<()> {
        for child in &node.children {
            match child.tag.as_str() {
                "Directory" => {
                    let dir_id_str = child.attribute("Id").ok_or_else(|| Error::WixCompiler {
                        element: "Directory".to_string(),
                        message: "missing required 'Id' attribute".to_string(),
                    })?;
                    let dir_name = child.attribute("Name").unwrap_or(dir_id_str);

                    let dir_id = DirectoryId::new(dir_id_str)?;
                    let parent_dir = match parent_id {
                        Some(p) => Some(DirectoryId::new(p)?),
                        None => None,
                    };

                    section.add_symbol(Symbol::new("Directory", dir_id_str));
                    if let Some(p) = parent_id {
                        section.add_reference(Reference::new("Directory", p));
                    }

                    let short_name = child.attribute("ShortName");
                    let default_dir = short_name
                        .map_or_else(|| dir_name.to_string(), |sn| format!("{sn}|{dir_name}"));

                    let row = DirectoryRow {
                        directory: dir_id,
                        directory_parent: parent_dir,
                        default_dir,
                    };
                    tables
                        .entry("Directory".to_string())
                        .or_insert_with(|| IntermediateTable::new("Directory"))
                        .push_record(row.to_record());

                    self.compile_element_tree(child, Some(dir_id_str), section, tables)?;
                }
                "Component" => {
                    let comp_id_str = child.attribute("Id").ok_or_else(|| Error::WixCompiler {
                        element: "Component".to_string(),
                        message: "missing required 'Id' attribute".to_string(),
                    })?;
                    let current_dir = parent_id.unwrap_or("TARGETDIR");
                    let comp_name = ComponentName::new(comp_id_str)?;
                    let dir_id = DirectoryId::new(current_dir)?;

                    let guid_str = child.attribute("Guid");
                    let comp_guid = match guid_str {
                        Some(g) if !g.is_empty() && g != "*" && g != "?" => {
                            Some(ComponentGuid::parse(g)?)
                        }
                        Some("*" | "?") => Some(ComponentGuid::generate_deterministic(
                            current_dir,
                            comp_id_str,
                        )?),
                        _ => None,
                    };

                    let mut comp_attrs: i16 = 0;
                    if child
                        .attribute("Win64")
                        .is_some_and(|s| s.eq_ignore_ascii_case("yes"))
                    {
                        comp_attrs |= 0x0100;
                    }
                    if child
                        .attribute("Permanent")
                        .is_some_and(|s| s.eq_ignore_ascii_case("yes"))
                    {
                        comp_attrs |= 0x0040;
                    }
                    if child
                        .attribute("NeverOverwrite")
                        .is_some_and(|s| s.eq_ignore_ascii_case("yes"))
                    {
                        comp_attrs |= 0x0080;
                    }
                    if child
                        .attribute("Shared")
                        .or_else(|| child.attribute("SharedDllRefCount"))
                        .is_some_and(|s| s.eq_ignore_ascii_case("yes"))
                    {
                        comp_attrs |= 0x0020;
                    }
                    if child
                        .attribute("Transitive")
                        .is_some_and(|s| s.eq_ignore_ascii_case("yes"))
                    {
                        comp_attrs |= 0x0200;
                    }
                    if child
                        .attribute("UninstallWhenSuperseded")
                        .is_some_and(|s| s.eq_ignore_ascii_case("yes"))
                    {
                        comp_attrs |= 0x0400;
                    }
                    if child
                        .attribute("MultiInstance")
                        .is_some_and(|s| s.eq_ignore_ascii_case("yes"))
                    {
                        comp_attrs |= 0x0800;
                    }
                    match child.attribute("Location") {
                        Some("local") => comp_attrs |= 0x0001,
                        Some("source") => comp_attrs |= 0x0002,
                        Some("either") => comp_attrs |= 0x0004,
                        _ => {}
                    }

                    let comp_cond = child
                        .children
                        .iter()
                        .find(|c| c.tag == "Condition")
                        .map(|c| c.text.clone())
                        .or_else(|| child.attribute("Condition").map(ToString::to_string));

                    let comp_kp = child.attribute("KeyPath").map(ToString::to_string);

                    section.add_symbol(Symbol::new("Component", comp_id_str));
                    section.add_reference(Reference::new("Directory", current_dir));

                    let row = ComponentRow {
                        component: comp_name,
                        component_id: comp_guid,
                        directory: dir_id,
                        attributes: comp_attrs,
                        condition: comp_cond,
                        key_path: comp_kp,
                    };
                    tables
                        .entry("Component".to_string())
                        .or_insert_with(|| IntermediateTable::new("Component"))
                        .push_record(row.to_record());

                    self.compile_element_tree(child, Some(comp_id_str), section, tables)?;
                }
                "File" => {
                    let file_id_str = child.attribute("Id").ok_or_else(|| Error::WixCompiler {
                        element: "File".to_string(),
                        message: "missing required 'Id' attribute".to_string(),
                    })?;
                    let file_name = child
                        .attribute("Name")
                        .or_else(|| child.attribute("Source"))
                        .unwrap_or(file_id_str);
                    let current_comp = parent_id.unwrap_or("DefaultComp");

                    let file_key = FileKey::new(file_id_str)?;
                    let comp_name = ComponentName::new(current_comp)?;

                    section.add_symbol(Symbol::new("File", file_id_str));
                    section.add_reference(Reference::new("Component", current_comp));

                    let mut file_attrs: i16 = 0x0200; // Default vital in WiX
                    if child
                        .attribute("Vital")
                        .is_some_and(|s| s.eq_ignore_ascii_case("no"))
                    {
                        file_attrs &= !0x0200;
                    }
                    if child
                        .attribute("ReadOnly")
                        .is_some_and(|s| s.eq_ignore_ascii_case("yes"))
                    {
                        file_attrs |= 0x0001;
                    }
                    if child
                        .attribute("Hidden")
                        .is_some_and(|s| s.eq_ignore_ascii_case("yes"))
                    {
                        file_attrs |= 0x0002;
                    }
                    if child
                        .attribute("System")
                        .is_some_and(|s| s.eq_ignore_ascii_case("yes"))
                    {
                        file_attrs |= 0x0004;
                    }
                    if child
                        .attribute("Checksum")
                        .is_some_and(|s| s.eq_ignore_ascii_case("yes"))
                    {
                        file_attrs |= 0x0400;
                    }
                    if child
                        .attribute("Compressed")
                        .is_some_and(|s| s.eq_ignore_ascii_case("yes"))
                    {
                        file_attrs |= 0x4000;
                    } else if child
                        .attribute("Compressed")
                        .is_some_and(|s| s.eq_ignore_ascii_case("no"))
                    {
                        file_attrs |= 0x2000;
                    }

                    let mut file_size: i32 = child
                        .attribute("DefaultSize")
                        .or_else(|| child.attribute("Size"))
                        .and_then(|s| s.parse().ok())
                        .unwrap_or(0);
                    if file_size == 0 {
                        if let Some(src) = child.attribute("Source") {
                            if let Ok(meta) = std::fs::metadata(src) {
                                file_size = i32::try_from(meta.len()).unwrap_or(0);
                            }
                        }
                    }

                    let version = child
                        .attribute("DefaultVersion")
                        .or_else(|| child.attribute("Version"))
                        .map(ToString::to_string);
                    let language = child
                        .attribute("DefaultLanguage")
                        .or_else(|| child.attribute("Language"))
                        .map(ToString::to_string);

                    let short_name = child.attribute("ShortName");
                    let final_file_name = short_name.map_or_else(
                        || {
                            if file_name.contains(' ') || file_name.len() > 12 {
                                let parts: Vec<&str> = file_name.split('.').collect();
                                let base = parts[0];
                                let ext = if parts.len() > 1 { parts[1] } else { "" };
                                let short_base = if base.len() > 6 {
                                    format!("{}~1", base[..6].to_ascii_uppercase())
                                } else {
                                    base.to_ascii_uppercase()
                                };
                                let short_ext = if ext.len() > 3 {
                                    &ext[..3].to_ascii_uppercase()
                                } else {
                                    ext
                                };
                                if short_ext.is_empty() {
                                    format!("{short_base}|{file_name}")
                                } else {
                                    format!("{short_base}.{short_ext}|{file_name}")
                                }
                            } else {
                                file_name.to_string()
                            }
                        },
                        |sn| format!("{sn}|{file_name}"),
                    );

                    let row = FileRow {
                        file: file_key,
                        component: comp_name,
                        file_name: final_file_name,
                        file_size,
                        version,
                        language,
                        attributes: Some(file_attrs),
                        sequence: 1,
                    };
                    tables
                        .entry("File".to_string())
                        .or_insert_with(|| IntermediateTable::new("File"))
                        .push_record(row.to_record());

                    if let Some(src) = child.attribute("Source") {
                        tables
                            .entry("WixFile".to_string())
                            .or_insert_with(|| IntermediateTable::new("WixFile"))
                            .push_record(Record::with_fields(vec![
                                FieldValue::String(file_id_str.to_string()),
                                FieldValue::String(src.to_string()),
                            ]));
                    }

                    for sub in &child.children {
                        if sub.tag == "Font" {
                            let font_title = sub.attribute("Title").map(ToString::to_string);
                            let font_row = crate::database::tables::core::FontRow {
                                file: FileKey::new(file_id_str)?,
                                font_title,
                            };
                            tables
                                .entry("Font".to_string())
                                .or_insert_with(|| IntermediateTable::new("Font"))
                                .push_record(font_row.to_record());
                        }
                    }
                }
                "Feature" => {
                    let feat_id_str = child.attribute("Id").ok_or_else(|| Error::WixCompiler {
                        element: "Feature".to_string(),
                        message: "missing required 'Id' attribute".to_string(),
                    })?;
                    let title = child.attribute("Title").map(ToString::to_string);
                    let desc = child.attribute("Description").map(ToString::to_string);
                    let level: i16 = child
                        .attribute("Level")
                        .and_then(|l| l.parse().ok())
                        .unwrap_or(1);

                    let display: Option<i16> = match child.attribute("Display") {
                        Some("hidden") => Some(0),
                        Some("collapse") => Some(1),
                        Some("expand") => Some(2),
                        Some(s) => s.parse().ok(),
                        None => None,
                    };

                    let mut feat_attrs: i16 = 0;
                    if child
                        .attribute("AllowAdvertise")
                        .is_some_and(|s| s.eq_ignore_ascii_case("no"))
                        || child
                            .attribute("Absent")
                            .is_some_and(|s| s.eq_ignore_ascii_case("disallow"))
                    {
                        feat_attrs |= 0x0010;
                    }
                    match child.attribute("InstallDefault") {
                        Some("local") => feat_attrs |= 0x0001,
                        Some("source") => feat_attrs |= 0x0002,
                        Some("followParent") => feat_attrs |= 0x0004,
                        _ => {}
                    }
                    if child
                        .attribute("TypicalDefault")
                        .is_some_and(|s| s.eq_ignore_ascii_case("advertise"))
                    {
                        feat_attrs |= 0x0008;
                    }

                    let config_dir = child
                        .attribute("ConfigurableDirectory")
                        .and_then(|d| DirectoryId::new(d).ok());

                    let feat_name = FeatureName::new(feat_id_str)?;
                    let parent_feat = match parent_id {
                        Some(p) => Some(FeatureName::new(p)?),
                        None => None,
                    };

                    section.add_symbol(Symbol::new("Feature", feat_id_str));
                    if let Some(p) = parent_id {
                        section.add_reference(Reference::new("Feature", p));
                    }

                    let row = FeatureRow {
                        feature: feat_name,
                        feature_parent: parent_feat,
                        title,
                        description: desc,
                        display,
                        level,
                        directory: config_dir,
                        attributes: feat_attrs,
                    };
                    tables
                        .entry("Feature".to_string())
                        .or_insert_with(|| IntermediateTable::new("Feature"))
                        .push_record(row.to_record());

                    // Compile child elements inside Feature (e.g. ComponentRef, ComponentGroupRef, Condition, MergeRef)
                    for sub in &child.children {
                        if sub.tag == "ComponentRef" {
                            if let Some(comp_ref_id) = sub.attribute("Id") {
                                section.add_reference(Reference::new("Component", comp_ref_id));
                                let fc_row = FeatureComponentsRow {
                                    feature: FeatureName::new(feat_id_str)?,
                                    component: ComponentName::new(comp_ref_id)?,
                                };
                                tables
                                    .entry("FeatureComponents".to_string())
                                    .or_insert_with(|| IntermediateTable::new("FeatureComponents"))
                                    .push_record(fc_row.to_record());
                            }
                        } else if sub.tag == "ComponentGroupRef" {
                            let cg_ref_id = sub.attribute("Id").unwrap_or("");
                            section.add_reference(Reference::new("ComponentGroup", cg_ref_id));
                            tables
                                .entry("_FeatureComponentGroupRef".to_string())
                                .or_insert_with(|| {
                                    IntermediateTable::new("_FeatureComponentGroupRef")
                                })
                                .push_record(Record::with_fields(vec![
                                    FieldValue::String(feat_id_str.to_string()),
                                    FieldValue::String(cg_ref_id.to_string()),
                                ]));
                        } else if sub.tag == "Condition" {
                            let cond_level: i16 = sub
                                .attribute("Level")
                                .and_then(|l| l.parse().ok())
                                .unwrap_or(0);
                            let cond_text = if sub.text.is_empty() { "1" } else { &sub.text };
                            let cond_rec = Record::with_fields(vec![
                                FieldValue::String(feat_id_str.to_string()),
                                FieldValue::Short(cond_level),
                                FieldValue::String(cond_text.to_string()),
                            ]);
                            tables
                                .entry("Condition".to_string())
                                .or_insert_with(|| IntermediateTable::new("Condition"))
                                .push_record(cond_rec);
                        } else if sub.tag == "Merge" || sub.tag == "MergeRef" {
                            if let Some(m_id) = sub.attribute("Id") {
                                section.add_reference(Reference::new("Module", m_id));
                            }
                        }
                    }

                    self.compile_element_tree(child, Some(feat_id_str), section, tables)?;
                }
                "DirectoryRef" => {
                    let dir_id_str = child.attribute("Id").ok_or_else(|| Error::WixCompiler {
                        element: "DirectoryRef".to_string(),
                        message: "missing required 'Id' attribute".to_string(),
                    })?;
                    section.add_reference(Reference::new("Directory", dir_id_str));
                    self.compile_element_tree(child, Some(dir_id_str), section, tables)?;
                }
                "ComponentGroup" => {
                    let group_id_str = child.attribute("Id").ok_or_else(|| Error::WixCompiler {
                        element: "ComponentGroup".to_string(),
                        message: "missing required 'Id' attribute".to_string(),
                    })?;
                    section.add_symbol(Symbol::new("ComponentGroup", group_id_str));
                    let comp_dir = child.attribute("Directory").or(parent_id);
                    if let Some(dir) = child.attribute("Directory") {
                        section.add_reference(Reference::new("Directory", dir));
                    }

                    for sub in &child.children {
                        if sub.tag == "ComponentRef" {
                            if let Some(comp_ref_id) = sub.attribute("Id") {
                                section.add_reference(Reference::new("Component", comp_ref_id));
                                tables
                                    .entry("_ComponentGroupMember".to_string())
                                    .or_insert_with(|| {
                                        IntermediateTable::new("_ComponentGroupMember")
                                    })
                                    .push_record(Record::with_fields(vec![
                                        FieldValue::String(group_id_str.to_string()),
                                        FieldValue::String(comp_ref_id.to_string()),
                                    ]));
                            }
                        } else if sub.tag == "Component" {
                            let comp_id = sub.attribute("Id").unwrap_or("");
                            tables
                                .entry("_ComponentGroupMember".to_string())
                                .or_insert_with(|| IntermediateTable::new("_ComponentGroupMember"))
                                .push_record(Record::with_fields(vec![
                                    FieldValue::String(group_id_str.to_string()),
                                    FieldValue::String(comp_id.to_string()),
                                ]));
                        } else if sub.tag == "ComponentGroupRef" {
                            let cg_ref_id = sub.attribute("Id").unwrap_or("");
                            section.add_reference(Reference::new("ComponentGroup", cg_ref_id));
                            tables
                                .entry("_ComponentGroupNested".to_string())
                                .or_insert_with(|| IntermediateTable::new("_ComponentGroupNested"))
                                .push_record(Record::with_fields(vec![
                                    FieldValue::String(group_id_str.to_string()),
                                    FieldValue::String(cg_ref_id.to_string()),
                                ]));
                        }
                    }

                    self.compile_element_tree(child, comp_dir, section, tables)?;
                }
                "ComponentGroupRef" => {
                    let group_ref_id = child.attribute("Id").ok_or_else(|| Error::WixCompiler {
                        element: "ComponentGroupRef".to_string(),
                        message: "missing required 'Id' attribute".to_string(),
                    })?;
                    section.add_reference(Reference::new("ComponentGroup", group_ref_id));
                }
                "PackageGroup" => {
                    let group_id_str = child.attribute("Id").ok_or_else(|| Error::WixCompiler {
                        element: "PackageGroup".to_string(),
                        message: "missing required 'Id' attribute".to_string(),
                    })?;
                    section.add_symbol(Symbol::new("PackageGroup", group_id_str));
                    self.compile_element_tree(child, parent_id, section, tables)?;
                }
                "PackageGroupRef" => {
                    let group_ref_id = child.attribute("Id").ok_or_else(|| Error::WixCompiler {
                        element: "PackageGroupRef".to_string(),
                        message: "missing required 'Id' attribute".to_string(),
                    })?;
                    section.add_reference(Reference::new("PackageGroup", group_ref_id));
                }
                "FeatureGroup" => {
                    let group_id_str = child.attribute("Id").ok_or_else(|| Error::WixCompiler {
                        element: "FeatureGroup".to_string(),
                        message: "missing required 'Id' attribute".to_string(),
                    })?;
                    section.add_symbol(Symbol::new("FeatureGroup", group_id_str));
                    self.compile_element_tree(child, parent_id, section, tables)?;
                }
                "FeatureGroupRef" => {
                    let group_ref_id = child.attribute("Id").ok_or_else(|| Error::WixCompiler {
                        element: "FeatureGroupRef".to_string(),
                        message: "missing required 'Id' attribute".to_string(),
                    })?;
                    section.add_reference(Reference::new("FeatureGroup", group_ref_id));
                }
                "FeatureRef" => {
                    let feat_ref_id = child.attribute("Id").ok_or_else(|| Error::WixCompiler {
                        element: "FeatureRef".to_string(),
                        message: "missing required 'Id' attribute".to_string(),
                    })?;
                    section.add_reference(Reference::new("Feature", feat_ref_id));
                    self.compile_element_tree(child, Some(feat_ref_id), section, tables)?;
                }
                "Package" => {
                    let prop_table = tables
                        .entry("Property".to_string())
                        .or_insert_with(|| IntermediateTable::new("Property"));

                    if let Some(iv) = child.attribute("InstallerVersion") {
                        let p = PropertyRow {
                            property: PropertyName::new("InstallerVersion")?,
                            value: iv.to_string(),
                        };
                        prop_table.push_record(p.to_record());
                    }
                    if let Some(cmp) = child.attribute("Compressed") {
                        let p = PropertyRow {
                            property: PropertyName::new("ProductCompressed")?,
                            value: cmp.to_string(),
                        };
                        prop_table.push_record(p.to_record());
                    }
                    if let Some(desc) = child.attribute("Description") {
                        let p = PropertyRow {
                            property: PropertyName::new("ProductDescription")?,
                            value: desc.to_string(),
                        };
                        prop_table.push_record(p.to_record());
                    }
                    if let Some(comm) = child.attribute("Comments") {
                        let p = PropertyRow {
                            property: PropertyName::new("ProductComments")?,
                            value: comm.to_string(),
                        };
                        prop_table.push_record(p.to_record());
                    }
                    if let Some(kw) = child.attribute("Keywords") {
                        let p = PropertyRow {
                            property: PropertyName::new("ProductKeywords")?,
                            value: kw.to_string(),
                        };
                        prop_table.push_record(p.to_record());
                    }
                    if let Some(lang) = child
                        .attribute("Languages")
                        .or_else(|| child.attribute("Language"))
                    {
                        let p = PropertyRow {
                            property: PropertyName::new("ProductLanguage")?,
                            value: lang.to_string(),
                        };
                        prop_table.push_record(p.to_record());
                    }
                    if let Some(mfg) = child.attribute("Manufacturer") {
                        let p = PropertyRow {
                            property: PropertyName::new("Manufacturer")?,
                            value: mfg.to_string(),
                        };
                        prop_table.push_record(p.to_record());
                    }
                    if let Some(plt) = child.attribute("Platform") {
                        let p = PropertyRow {
                            property: PropertyName::new("ProductPlatform")?,
                            value: plt.to_string(),
                        };
                        prop_table.push_record(p.to_record());
                    }
                    if let Some(cp) = child
                        .attribute("SummaryCodepage")
                        .or_else(|| child.attribute("Codepage"))
                    {
                        let p = PropertyRow {
                            property: PropertyName::new("ProductCodepage")?,
                            value: cp.to_string(),
                        };
                        prop_table.push_record(p.to_record());
                    }

                    self.compile_element_tree(child, parent_id, section, tables)?;
                }
                "SetDirectory" => {
                    let dir_id_str = child.attribute("Id").ok_or_else(|| Error::WixCompiler {
                        element: "SetDirectory".to_string(),
                        message: "missing required 'Id' attribute".to_string(),
                    })?;
                    let value = child.attribute("Value").unwrap_or("");
                    let action_name = child
                        .attribute("Action")
                        .map_or_else(|| format!("SetDirectory_{dir_id_str}"), ToString::to_string);

                    section.add_symbol(Symbol::new("CustomAction", &action_name));
                    section.add_reference(Reference::new("Directory", dir_id_str));

                    let rec = Record::with_fields(vec![
                        FieldValue::String(action_name.clone()),
                        FieldValue::Short(35),
                        FieldValue::String(dir_id_str.to_string()),
                        FieldValue::String(value.to_string()),
                        FieldValue::Null,
                    ]);
                    tables
                        .entry("CustomAction".to_string())
                        .or_insert_with(|| IntermediateTable::new("CustomAction"))
                        .push_record(rec);

                    let seq_num: Option<i16> =
                        child.attribute("Sequence").and_then(|s| s.parse().ok());
                    let cond = child
                        .children
                        .iter()
                        .find(|c| c.tag == "Condition")
                        .map(|c| c.text.clone())
                        .or_else(|| child.attribute("Condition").map(ToString::to_string));
                    let seq_row = SequenceRow::new(&action_name, cond, seq_num)?;
                    tables
                        .entry("InstallExecuteSequence".to_string())
                        .or_insert_with(|| IntermediateTable::new("InstallExecuteSequence"))
                        .push_record(seq_row.to_record());
                }
                "CreateFolder" => {
                    let dir_id_str = child
                        .attribute("Directory")
                        .or(parent_id)
                        .unwrap_or("TARGETDIR");
                    let comp_str = parent_id.unwrap_or("DefaultComp");
                    let row = CreateFolderRow {
                        directory: DirectoryId::new(dir_id_str)?,
                        component: ComponentName::new(comp_str)?,
                    };
                    tables
                        .entry("CreateFolder".to_string())
                        .or_insert_with(|| IntermediateTable::new("CreateFolder"))
                        .push_record(row.to_record());
                }
                "RemoveFolder" => {
                    let rem_id = child.attribute("Id").ok_or_else(|| Error::WixCompiler {
                        element: "RemoveFolder".to_string(),
                        message: "missing required 'Id' attribute".to_string(),
                    })?;
                    let dir_prop = child
                        .attribute("Directory")
                        .or_else(|| child.attribute("Property"))
                        .or(parent_id)
                        .unwrap_or("TARGETDIR");
                    let comp_str = parent_id.unwrap_or("DefaultComp");
                    let on_mode = match child.attribute("On") {
                        Some("install") => 2,
                        Some("uninstall") => 1,
                        _ => 3,
                    };
                    let row = RemoveFileRow {
                        file_key: rem_id.to_string(),
                        component: ComponentName::new(comp_str)?,
                        file_name: None,
                        dir_property: dir_prop.to_string(),
                        install_mode: on_mode,
                    };
                    tables
                        .entry("RemoveFile".to_string())
                        .or_insert_with(|| IntermediateTable::new("RemoveFile"))
                        .push_record(row.to_record());
                }
                "Environment" => {
                    let env_id = child.attribute("Id").ok_or_else(|| Error::WixCompiler {
                        element: "Environment".to_string(),
                        message: "missing required 'Id' attribute".to_string(),
                    })?;
                    let raw_name = child.attribute("Name").unwrap_or("");
                    let value = child.attribute("Value").unwrap_or("");
                    let comp_str = parent_id.unwrap_or("DefaultComp");

                    let mut prefix = String::new();
                    if child
                        .attribute("System")
                        .is_some_and(|s| s.eq_ignore_ascii_case("yes"))
                    {
                        prefix.push('*');
                    }
                    match child.attribute("Action") {
                        Some("create") => prefix.push('+'),
                        Some("remove") => prefix.push('-'),
                        _ => prefix.push('='),
                    }
                    let formatted_name = format!("{prefix}{raw_name}");

                    let row = EnvironmentRow {
                        environment: env_id.to_string(),
                        name: formatted_name,
                        value: value.to_string(),
                        component: ComponentName::new(comp_str)?,
                    };
                    tables
                        .entry("Environment".to_string())
                        .or_insert_with(|| IntermediateTable::new("Environment"))
                        .push_record(row.to_record());
                }
                "Media" => {
                    let disk_id: i16 = child
                        .attribute("Id")
                        .and_then(|id| id.parse().ok())
                        .unwrap_or(1);
                    let cabinet = child.attribute("Cabinet").map(ToString::to_string);

                    section.add_symbol(Symbol::new("Media", format!("{disk_id}")));

                    let row = MediaRow {
                        disk_id,
                        last_sequence: 1,
                        disk_prompt: None,
                        cabinet,
                        volume_label: None,
                        source: None,
                    };
                    tables
                        .entry("Media".to_string())
                        .or_insert_with(|| IntermediateTable::new("Media"))
                        .push_record(row.to_record());
                }
                "Property" => {
                    let prop_id_str = child.attribute("Id").ok_or_else(|| Error::WixCompiler {
                        element: "Property".to_string(),
                        message: "missing required 'Id' attribute".to_string(),
                    })?;
                    let prop_val = child.attribute("Value").unwrap_or("");

                    let prop_name = PropertyName::new(prop_id_str)?;
                    section.add_symbol(Symbol::new("Property", prop_id_str));

                    let row = PropertyRow {
                        property: prop_name,
                        value: prop_val.to_string(),
                    };
                    tables
                        .entry("Property".to_string())
                        .or_insert_with(|| IntermediateTable::new("Property"))
                        .push_record(row.to_record());
                }
                "RegistryKey" => {
                    let key = child.attribute("Key").unwrap_or("Software\\Product");
                    let root = child.attribute("Root").unwrap_or("HKLM");
                    for sub in &child.children {
                        if sub.tag == "RegistryValue" {
                            Self::compile_registry_value(
                                sub,
                                parent_id,
                                Some(root),
                                Some(key),
                                section,
                                tables,
                            )?;
                        }
                    }
                    self.compile_element_tree(child, parent_id, section, tables)?;
                }
                "RegistryValue" => {
                    Self::compile_registry_value(child, parent_id, None, None, section, tables)?;
                }
                "Shortcut" => {
                    Self::compile_shortcut(child, parent_id, section, tables);
                }
                "ServiceInstall" => {
                    Self::compile_service_install(child, parent_id, section, tables);
                }
                "ServiceControl" => {
                    Self::compile_service_control(child, parent_id, section, tables);
                }
                "CustomAction" => {
                    Self::compile_custom_action(child, section, tables)?;
                }
                "Dialog" => {
                    Self::compile_dialog(child, section, tables)?;
                    self.compile_element_tree(child, child.attribute("Id"), section, tables)?;
                }
                "Control" => {
                    Self::compile_control(child, parent_id, section, tables)?;
                }
                "TextStyle" => {
                    Self::compile_text_style(child, section, tables);
                }
                "Upgrade" => {
                    Self::compile_upgrade(child, section, tables);
                }
                "MajorUpgrade" => {
                    Self::compile_major_upgrade(tables);
                }
                "posix:File" | "PosixFile" | "posix:Symlink" | "PosixSymlink" | "posix:Daemon"
                | "PosixDaemon" | "posix:Acl" | "PosixAcl" | "posix:Desktop" | "PosixDesktop" => {
                    Self::compile_posix_element(child, parent_id, section, tables);
                }
                "Configuration" => {
                    Self::compile_module_configuration(child, tables);
                }
                "Substitution" => {
                    Self::compile_module_substitution(child, tables);
                }
                "IgnoreModularization" => {
                    Self::compile_module_ignore_modularization(child, tables);
                }
                "Dependency" | "ModuleDependency" => {
                    Self::compile_module_dependency(child, tables);
                }
                "Exclusion" | "ModuleExclusion" => {
                    Self::compile_module_exclusion(child, tables);
                }
                "Symbol" | "PublishSymbol" => {
                    let sym_name = child
                        .attribute("Name")
                        .or_else(|| child.attribute("Id"))
                        .or_else(|| child.attribute("Path"))
                        .unwrap_or("Symbol1");
                    let sym_ns = child.attribute("Namespace").unwrap_or("Symbol");
                    section.add_symbol(Symbol::new(sym_ns, sym_name));
                }
                "MediaTemplate" => {
                    Self::compile_media_template(child, tables);
                }
                "CopyFile" => {
                    Self::compile_copy_file(child, parent_id, section, tables);
                }
                "MoveFile" => {
                    Self::compile_move_file(child, parent_id, section, tables);
                }
                "RemoveFile" => {
                    Self::compile_remove_file(child, parent_id, section, tables)?;
                }
                "SymbolicLink" | "Hardlink" => {
                    Self::compile_symlink(child, parent_id, section, tables)?;
                }
                "RemoveRegistryKey" | "RemoveRegistryValue" => {
                    Self::compile_remove_registry(child, parent_id, section, tables);
                }
                "IniFile" => {
                    Self::compile_ini_file(child, parent_id, section, tables);
                }
                "RemoveIniFile" => {
                    Self::compile_remove_ini_file(child, parent_id, section, tables);
                }
                "Icon" => {
                    Self::compile_icon(child, section, tables);
                }
                "ProgId" => {
                    Self::compile_prog_id(child, parent_id, section, tables);
                }
                "Extension" => {
                    Self::compile_extension(child, parent_id, section, tables);
                }
                "MIME" => {
                    Self::compile_mime(child, section, tables);
                }
                "Class" => {
                    Self::compile_class(child, parent_id, section, tables);
                }
                "TypeLib" => {
                    Self::compile_type_lib(child, parent_id, section, tables);
                }
                "AppId" => {
                    Self::compile_app_id(child, section, tables);
                }
                "PropertyRef" => {
                    if let Some(prop_ref) = child.attribute("Id") {
                        section.add_reference(Reference::new("Property", prop_ref));
                    }
                }
                "SetProperty" => {
                    Self::compile_set_property(child, section, tables)?;
                }
                "AppSearch" => {
                    Self::compile_app_search(child, section, tables);
                }
                "RegistrySearch" => {
                    Self::compile_registry_search(child, parent_id, section, tables);
                }
                "DirectorySearch" => {
                    Self::compile_directory_search(child, parent_id, section, tables);
                }
                "FileSearch" => {
                    Self::compile_file_search(child, parent_id, section, tables);
                }
                "IniFileSearch" => {
                    Self::compile_ini_file_search(child, parent_id, section, tables);
                }
                "ComponentSearch" => {
                    Self::compile_component_search(child, parent_id, section, tables);
                }
                "Launch" | "Condition" => {
                    Self::compile_launch_condition(child, tables);
                }
                "InstallExecuteSequence"
                | "InstallUISequence"
                | "AdminExecuteSequence"
                | "AdminUISequence"
                | "AdvtExecuteSequence" => {
                    Self::compile_sequence_table(child, section, tables)?;
                }
                "CustomActionRef" => {
                    if let Some(ca_ref) = child.attribute("Id") {
                        section.add_reference(Reference::new("CustomAction", ca_ref));
                    }
                }
                "PatchCreation" | "PatchInformation" | "PatchMetadata" | "PatchFamily"
                | "Patch" => {
                    Self::compile_patch_element(child, section, tables);
                }
                "UIRef" => {
                    Self::compile_ui_ref(child, section, tables);
                }
                "ControlEvent" | "Publish" => {
                    Self::compile_control_event(child, parent_id, tables);
                }
                "ControlCondition" => {
                    Self::compile_control_condition(child, parent_id, tables);
                }
                "Subscribe" => {
                    Self::compile_subscribe(child, parent_id, tables);
                }
                "Binary" => {
                    Self::compile_binary(child, section, tables);
                }
                "Billboard" | "BillboardControl" => {
                    Self::compile_billboard(child, section, tables);
                }
                "ProgressText" => {
                    Self::compile_progress_text(child, tables);
                }
                "Error" => {
                    Self::compile_error(child, tables);
                }
                "Merge" | "MergeRef" => {
                    if let Some(m_id) = child.attribute("Id") {
                        section.add_reference(Reference::new("Module", m_id));
                    }
                }
                _ => {
                    if child.tag.contains(':') {
                        Self::compile_extension_element(child, parent_id, section, tables);
                    }
                    self.compile_element_tree(child, parent_id, section, tables)?;
                }
            }
        }

        Ok(())
    }

    /// Compiles a `<RegistryValue>` element.
    fn compile_registry_value(
        child: &XmlNode,
        parent_id: Option<&str>,
        inherited_root: Option<&str>,
        inherited_key: Option<&str>,
        section: &mut IntermediateSection,
        tables: &mut std::collections::HashMap<String, IntermediateTable>,
    ) -> Result<()> {
        let comp_name = parent_id.unwrap_or("DefaultComp");
        let reg_id = child.attribute("Id").unwrap_or("Registry1");
        let root_str = child.attribute("Root").or(inherited_root).unwrap_or("HKLM");
        let root = parse_registry_root(root_str);
        let key = child
            .attribute("Key")
            .or(inherited_key)
            .unwrap_or("Software\\Product");
        let name = child.attribute("Name").map(ToString::to_string);
        let value = child.attribute("Value").map(ToString::to_string);

        section.add_symbol(Symbol::new("Registry", reg_id));
        section.add_reference(Reference::new("Component", comp_name));

        let row = RegistryRow {
            registry: reg_id.to_string(),
            root,
            key: key.to_string(),
            name,
            value,
            component: ComponentName::new(comp_name)?,
        };
        tables
            .entry("Registry".to_string())
            .or_insert_with(|| IntermediateTable::new("Registry"))
            .push_record(row.to_record());
        Ok(())
    }

    /// Compiles a `<Shortcut>` element.
    fn compile_shortcut(
        child: &XmlNode,
        parent_id: Option<&str>,
        section: &mut IntermediateSection,
        tables: &mut std::collections::HashMap<String, IntermediateTable>,
    ) {
        let sc_id = child.attribute("Id").unwrap_or("Shortcut1");
        let sc_name = child.attribute("Name").unwrap_or(sc_id);
        let sc_target = child.attribute("Target").unwrap_or("");
        let sc_dir = child.attribute("Directory").unwrap_or("ProgramMenuFolder");
        let comp_name = parent_id.unwrap_or("DefaultComp");
        let sc_desc = child.attribute("Description").map(ToString::to_string);
        let sc_args = child.attribute("Arguments").map(ToString::to_string);
        let sc_icon = child.attribute("Icon").map(ToString::to_string);

        section.add_symbol(Symbol::new("Shortcut", sc_id));
        section.add_reference(Reference::new("Component", comp_name));

        let rec = Record::with_fields(vec![
            FieldValue::String(sc_id.to_string()),
            FieldValue::String(sc_dir.to_string()),
            FieldValue::String(sc_name.to_string()),
            FieldValue::String(comp_name.to_string()),
            FieldValue::String(sc_target.to_string()),
            sc_args.map_or(FieldValue::Null, FieldValue::String),
            sc_desc.map_or(FieldValue::Null, FieldValue::String),
            FieldValue::Null,
            sc_icon.map_or(FieldValue::Null, FieldValue::String),
            FieldValue::Null,
            FieldValue::Null,
            FieldValue::Null,
        ]);
        tables
            .entry("Shortcut".to_string())
            .or_insert_with(|| IntermediateTable::new("Shortcut"))
            .push_record(rec);
    }

    /// Compiles a `<ServiceInstall>` element.
    fn compile_service_install(
        child: &XmlNode,
        parent_id: Option<&str>,
        section: &mut IntermediateSection,
        tables: &mut std::collections::HashMap<String, IntermediateTable>,
    ) {
        let svc_id = child
            .attribute("Id")
            .or_else(|| child.attribute("Name"))
            .unwrap_or("Service1");
        let svc_name = child.attribute("Name").unwrap_or(svc_id);
        let svc_display = child.attribute("DisplayName").map(ToString::to_string);
        let comp_name = parent_id.unwrap_or("DefaultComp");
        let svc_type = match child.attribute("Type").unwrap_or("ownProcess") {
            "shareProcess" => 32,
            _ => 16,
        };
        let start_type = match child.attribute("Start").unwrap_or("auto") {
            "demand" | "manual" => 3,
            "disabled" => 4,
            _ => 2,
        };
        let error_control = match child.attribute("ErrorControl").unwrap_or("normal") {
            "ignore" => 0,
            "critical" => 3,
            _ => 1,
        };

        section.add_symbol(Symbol::new("ServiceInstall", svc_id));
        section.add_reference(Reference::new("Component", comp_name));

        let rec = Record::with_fields(vec![
            FieldValue::String(svc_id.to_string()),
            FieldValue::String(svc_name.to_string()),
            svc_display.map_or(FieldValue::Null, FieldValue::String),
            FieldValue::Long(svc_type),
            FieldValue::Long(start_type),
            FieldValue::Long(error_control),
            FieldValue::Null,
            FieldValue::Null,
            FieldValue::Null,
            FieldValue::Null,
            FieldValue::String(comp_name.to_string()),
        ]);
        tables
            .entry("ServiceInstall".to_string())
            .or_insert_with(|| IntermediateTable::new("ServiceInstall"))
            .push_record(rec);
    }

    /// Compiles a `<ServiceControl>` element.
    fn compile_service_control(
        child: &XmlNode,
        parent_id: Option<&str>,
        section: &mut IntermediateSection,
        tables: &mut std::collections::HashMap<String, IntermediateTable>,
    ) {
        let ctrl_id = child.attribute("Id").unwrap_or("ServiceControl1");
        let ctrl_name = child.attribute("Name").unwrap_or(ctrl_id);
        let comp_name = parent_id.unwrap_or("DefaultComp");
        let mut event: i16 = 0;
        if child.attribute("Start") == Some("yes") {
            event |= 1;
        }
        if child.attribute("Stop") == Some("yes") {
            event |= 2;
        }
        if child.attribute("Remove") == Some("yes") {
            event |= 32;
        }

        section.add_symbol(Symbol::new("ServiceControl", ctrl_id));
        section.add_reference(Reference::new("Component", comp_name));

        let rec = Record::with_fields(vec![
            FieldValue::String(ctrl_id.to_string()),
            FieldValue::String(ctrl_name.to_string()),
            FieldValue::Short(event),
            FieldValue::Null,
            FieldValue::Null,
            FieldValue::String(comp_name.to_string()),
        ]);
        tables
            .entry("ServiceControl".to_string())
            .or_insert_with(|| IntermediateTable::new("ServiceControl"))
            .push_record(rec);
    }

    /// Compiles a `<CustomAction>` element.
    fn compile_custom_action(
        child: &XmlNode,
        section: &mut IntermediateSection,
        tables: &mut std::collections::HashMap<String, IntermediateTable>,
    ) -> Result<()> {
        let ca_id = child.attribute("Id").ok_or_else(|| Error::WixCompiler {
            element: "CustomAction".to_string(),
            message: "missing required 'Id' attribute".to_string(),
        })?;
        let (source, target, mut ca_type) = match (
            child.attribute("BinaryKey"),
            child.attribute("FileKey"),
            child.attribute("Property"),
            child.attribute("Directory"),
            child.attribute("Script"),
            child.attribute("Error"),
        ) {
            (Some(bin), _, _, _, _, _) => child.attribute("DllEntry").map_or_else(
                || {
                    child
                        .attribute("ExeCommand")
                        .map_or_else(|| (bin, "", 1), |exe| (bin, exe, 2))
                },
                |dll| (bin, dll, 1),
            ),
            (None, Some(file), _, _, _, _) => {
                (file, child.attribute("ExeCommand").unwrap_or(""), 18)
            }
            (None, None, Some(prop), _, _, _) => (prop, child.attribute("Value").unwrap_or(""), 51),
            (None, None, None, Some(dir), _, _) => child.attribute("Value").map_or_else(
                || (dir, child.attribute("ExeCommand").unwrap_or(""), 34),
                |val| (dir, val, 35),
            ),
            (None, None, None, None, Some("vbscript"), _) => ("", child.text.as_str(), 6),
            (None, None, None, None, Some("jscript"), _) => ("", child.text.as_str(), 5),
            (None, None, None, None, _, Some(err)) => ("", err, 19),
            _ => ("", "", 1),
        };

        // Execution mode
        match child.attribute("Execute") {
            Some("deferred") => ca_type |= 0x0400,
            Some("rollback") => ca_type |= 0x0500,
            Some("commit") => ca_type |= 0x0600,
            Some("firstSequence") => ca_type |= 0x0100,
            Some("oncePerProcess") => ca_type |= 0x0200,
            _ => {}
        }

        // Return mode
        match child.attribute("Return") {
            Some("ignore") => ca_type |= 0x0040,
            Some("asyncWait") => ca_type |= 0x0080,
            Some("asyncNoWait") => ca_type |= 0x00C0,
            _ => {}
        }

        // Impersonation
        if child
            .attribute("Impersonate")
            .is_some_and(|s| s.eq_ignore_ascii_case("no"))
        {
            ca_type |= 0x0800;
        }

        section.add_symbol(Symbol::new("CustomAction", ca_id));

        let rec = Record::with_fields(vec![
            FieldValue::String(ca_id.to_string()),
            FieldValue::Short(ca_type),
            FieldValue::String(source.to_string()),
            FieldValue::String(target.to_string()),
            FieldValue::Null,
        ]);
        tables
            .entry("CustomAction".to_string())
            .or_insert_with(|| IntermediateTable::new("CustomAction"))
            .push_record(rec);
        Ok(())
    }

    /// Compiles a `<Dialog>` element.
    fn compile_dialog(
        child: &XmlNode,
        section: &mut IntermediateSection,
        tables: &mut std::collections::HashMap<String, IntermediateTable>,
    ) -> Result<()> {
        let dlg_id = child.attribute("Id").ok_or_else(|| Error::WixCompiler {
            element: "Dialog".to_string(),
            message: "missing required 'Id' attribute".to_string(),
        })?;
        let width: i16 = child
            .attribute("Width")
            .and_then(|w| w.parse().ok())
            .unwrap_or(370);
        let height: i16 = child
            .attribute("Height")
            .and_then(|h| h.parse().ok())
            .unwrap_or(270);
        let title = child.attribute("Title").map(ToString::to_string);

        section.add_symbol(Symbol::new("Dialog", dlg_id));

        let rec = Record::with_fields(vec![
            FieldValue::String(dlg_id.to_string()),
            FieldValue::Short(50),
            FieldValue::Short(50),
            FieldValue::Short(width),
            FieldValue::Short(height),
            FieldValue::Long(3),
            title.map_or(FieldValue::Null, FieldValue::String),
            FieldValue::String("FirstControl".to_string()),
            FieldValue::Null,
            FieldValue::Null,
        ]);
        tables
            .entry("Dialog".to_string())
            .or_insert_with(|| IntermediateTable::new("Dialog"))
            .push_record(rec);
        Ok(())
    }

    /// Compiles a `<Control>` element.
    #[allow(clippy::too_many_lines)]
    fn compile_control(
        child: &XmlNode,
        parent_id: Option<&str>,
        section: &mut IntermediateSection,
        tables: &mut std::collections::HashMap<String, IntermediateTable>,
    ) -> Result<()> {
        let ctrl_id = child.attribute("Id").ok_or_else(|| Error::WixCompiler {
            element: "Control".to_string(),
            message: "missing required 'Id' attribute".to_string(),
        })?;
        let ctrl_type = child.attribute("Type").unwrap_or("PushButton");
        let x: i16 = child
            .attribute("X")
            .and_then(|v| v.parse().ok())
            .unwrap_or(0);
        let y: i16 = child
            .attribute("Y")
            .and_then(|v| v.parse().ok())
            .unwrap_or(0);
        let w: i16 = child
            .attribute("Width")
            .and_then(|v| v.parse().ok())
            .unwrap_or(56);
        let h: i16 = child
            .attribute("Height")
            .and_then(|v| v.parse().ok())
            .unwrap_or(17);
        let text = child.attribute("Text").map(ToString::to_string);
        let prop = child.attribute("Property").map(ToString::to_string);
        let dlg_name = parent_id.unwrap_or("DefaultDialog");

        section.add_symbol(Symbol::new("Control", format!("{dlg_name}.{ctrl_id}")));

        let rec = Record::with_fields(vec![
            FieldValue::String(dlg_name.to_string()),
            FieldValue::String(ctrl_id.to_string()),
            FieldValue::String(ctrl_type.to_string()),
            FieldValue::Short(x),
            FieldValue::Short(y),
            FieldValue::Short(w),
            FieldValue::Short(h),
            FieldValue::Long(3),
            prop.map_or(FieldValue::Null, FieldValue::String),
            text.map_or(FieldValue::Null, FieldValue::String),
            FieldValue::Null,
            FieldValue::Null,
        ]);
        tables
            .entry("Control".to_string())
            .or_insert_with(|| IntermediateTable::new("Control"))
            .push_record(rec);

        for sub in &child.children {
            match sub.tag.as_str() {
                "Publish" | "ControlEvent" => {
                    let event = sub.attribute("Event").unwrap_or("NewDialog");
                    let arg = sub
                        .attribute("Argument")
                        .or_else(|| sub.attribute("Value"))
                        .unwrap_or("");
                    let cond = sub
                        .attribute("Condition")
                        .map(ToString::to_string)
                        .or_else(|| {
                            if sub.text.is_empty() {
                                None
                            } else {
                                Some(sub.text.clone())
                            }
                        });
                    let order: Option<i16> = sub.attribute("Order").and_then(|o| o.parse().ok());
                    let ce_rec = Record::with_fields(vec![
                        FieldValue::String(dlg_name.to_string()),
                        FieldValue::String(ctrl_id.to_string()),
                        FieldValue::String(event.to_string()),
                        FieldValue::String(arg.to_string()),
                        cond.map_or(FieldValue::Null, FieldValue::String),
                        order.map_or(FieldValue::Null, FieldValue::Short),
                    ]);
                    tables
                        .entry("ControlEvent".to_string())
                        .or_insert_with(|| IntermediateTable::new("ControlEvent"))
                        .push_record(ce_rec);
                }
                "Condition" | "ControlCondition" => {
                    let action = sub.attribute("Action").unwrap_or("enable");
                    let cond = sub.attribute("Condition").map_or_else(
                        || {
                            if sub.text.is_empty() {
                                "1"
                            } else {
                                &sub.text
                            }
                        },
                        |c| c,
                    );
                    let cc_rec = Record::with_fields(vec![
                        FieldValue::String(dlg_name.to_string()),
                        FieldValue::String(ctrl_id.to_string()),
                        FieldValue::String(action.to_string()),
                        FieldValue::String(cond.to_string()),
                    ]);
                    tables
                        .entry("ControlCondition".to_string())
                        .or_insert_with(|| IntermediateTable::new("ControlCondition"))
                        .push_record(cc_rec);
                }
                "Subscribe" => {
                    let event = sub.attribute("Event").unwrap_or("SetProgress");
                    let attr = sub.attribute("Attribute").unwrap_or("Progress");
                    let em_rec = Record::with_fields(vec![
                        FieldValue::String(dlg_name.to_string()),
                        FieldValue::String(ctrl_id.to_string()),
                        FieldValue::String(event.to_string()),
                        FieldValue::String(attr.to_string()),
                    ]);
                    tables
                        .entry("EventMapping".to_string())
                        .or_insert_with(|| IntermediateTable::new("EventMapping"))
                        .push_record(em_rec);
                }
                _ => {}
            }
        }

        Ok(())
    }

    /// Compiles a `<TextStyle>` element.
    fn compile_text_style(
        child: &XmlNode,
        section: &mut IntermediateSection,
        tables: &mut std::collections::HashMap<String, IntermediateTable>,
    ) {
        let style_id = child.attribute("Id").unwrap_or("DefaultStyle");
        let face = child.attribute("FaceName").unwrap_or("Tahoma");
        let size: i16 = child
            .attribute("Size")
            .and_then(|s| s.parse().ok())
            .unwrap_or(9);

        section.add_symbol(Symbol::new("TextStyle", style_id));

        let rec = Record::with_fields(vec![
            FieldValue::String(style_id.to_string()),
            FieldValue::String(face.to_string()),
            FieldValue::Short(size),
            FieldValue::Null,
            FieldValue::Null,
        ]);
        tables
            .entry("TextStyle".to_string())
            .or_insert_with(|| IntermediateTable::new("TextStyle"))
            .push_record(rec);
    }

    /// Compiles an `<Upgrade>` element.
    fn compile_upgrade(
        child: &XmlNode,
        section: &mut IntermediateSection,
        tables: &mut std::collections::HashMap<String, IntermediateTable>,
    ) {
        let upg_id = child
            .attribute("Id")
            .unwrap_or("{00000000-0000-0000-0000-000000000000}");
        section.add_symbol(Symbol::new("Upgrade", upg_id));

        for sub in &child.children {
            if sub.tag == "UpgradeVersion" {
                let min = sub.attribute("Minimum").map(ToString::to_string);
                let max = sub.attribute("Maximum").map(ToString::to_string);
                let prop = sub.attribute("Property").unwrap_or("NEWPRODUCTFOUND");
                let rec = Record::with_fields(vec![
                    FieldValue::String(upg_id.to_string()),
                    min.map_or(FieldValue::Null, FieldValue::String),
                    max.map_or(FieldValue::Null, FieldValue::String),
                    FieldValue::Null,
                    FieldValue::Long(256),
                    FieldValue::Null,
                    FieldValue::String(prop.to_string()),
                ]);
                tables
                    .entry("Upgrade".to_string())
                    .or_insert_with(|| IntermediateTable::new("Upgrade"))
                    .push_record(rec);
            }
        }
    }

    /// Compiles a `<MajorUpgrade>` syntactic macro.
    fn compile_major_upgrade(tables: &mut std::collections::HashMap<String, IntermediateTable>) {
        let rec = Record::with_fields(vec![
            FieldValue::String("{00000000-0000-0000-0000-000000000000}".to_string()),
            FieldValue::Null,
            FieldValue::Null,
            FieldValue::Null,
            FieldValue::Long(256),
            FieldValue::Null,
            FieldValue::String("MAJORUPGRADE".to_string()),
        ]);
        tables
            .entry("Upgrade".to_string())
            .or_insert_with(|| IntermediateTable::new("Upgrade"))
            .push_record(rec);
    }

    /// Compiles cross-platform POSIX extension elements.
    fn compile_posix_element(
        child: &XmlNode,
        parent_id: Option<&str>,
        section: &mut IntermediateSection,
        tables: &mut std::collections::HashMap<String, IntermediateTable>,
    ) {
        match child.tag.as_str() {
            "posix:File" | "PosixFile" => {
                Self::compile_posix_file(child, parent_id, tables);
            }
            "posix:Symlink" | "PosixSymlink" => {
                Self::compile_posix_symlink(child, parent_id, section, tables);
            }
            "posix:Daemon" | "PosixDaemon" => {
                Self::compile_posix_daemon(child, section, tables);
            }
            "posix:Acl" | "PosixAcl" => {
                Self::compile_posix_acl(child, section, tables);
            }
            "posix:Desktop" | "PosixDesktop" => {
                Self::compile_posix_desktop(child, section, tables);
            }
            _ => {}
        }
    }

    /// Compiles `<posix:File>` element.
    fn compile_posix_file(
        child: &XmlNode,
        parent_id: Option<&str>,
        tables: &mut std::collections::HashMap<String, IntermediateTable>,
    ) {
        let file_key = child.attribute("File").or(parent_id).unwrap_or("AppFile");
        let mode: i32 = child
            .attribute("Mode")
            .and_then(|m| i32::from_str_radix(m.trim_start_matches("0o"), 8).ok())
            .unwrap_or(0o755);
        let owner = child.attribute("Owner").map(ToString::to_string);
        let group = child.attribute("Group").map(ToString::to_string);
        let rec = Record::with_fields(vec![
            FieldValue::String(file_key.to_string()),
            FieldValue::Long(mode),
            owner.map_or(FieldValue::Null, FieldValue::String),
            group.map_or(FieldValue::Null, FieldValue::String),
            FieldValue::Null,
        ]);
        tables
            .entry("PosixFile".to_string())
            .or_insert_with(|| IntermediateTable::new("PosixFile"))
            .push_record(rec);
    }

    /// Compiles `<posix:Symlink>` element.
    fn compile_posix_symlink(
        child: &XmlNode,
        parent_id: Option<&str>,
        section: &mut IntermediateSection,
        tables: &mut std::collections::HashMap<String, IntermediateTable>,
    ) {
        let sym_id = child.attribute("Id").unwrap_or("Symlink1");
        let target = child.attribute("Target").unwrap_or("");
        let dir = child.attribute("LinkDirectory").unwrap_or("TARGETDIR");
        let name = child.attribute("LinkName").unwrap_or(sym_id);
        let comp = parent_id.unwrap_or("DefaultComp");

        section.add_symbol(Symbol::new("PosixSymlink", sym_id));
        section.add_reference(Reference::new("Component", comp));

        let rec = Record::with_fields(vec![
            FieldValue::String(sym_id.to_string()),
            FieldValue::String(target.to_string()),
            FieldValue::String(dir.to_string()),
            FieldValue::String(name.to_string()),
            FieldValue::String(comp.to_string()),
        ]);
        tables
            .entry("PosixSymlink".to_string())
            .or_insert_with(|| IntermediateTable::new("PosixSymlink"))
            .push_record(rec);
    }

    /// Compiles `<posix:Daemon>` element.
    fn compile_posix_daemon(
        child: &XmlNode,
        section: &mut IntermediateSection,
        tables: &mut std::collections::HashMap<String, IntermediateTable>,
    ) {
        let svc = child.attribute("Service").unwrap_or("Service1");
        let sup_str = child.attribute("SupervisorType").unwrap_or("Systemd");
        let sup_code: i32 = match sup_str.to_ascii_lowercase().as_str() {
            "launchd" => 1,
            "rcd" | "rc.d" => 2,
            "smf" => 3,
            _ => 0,
        };
        let restart: i32 = match child.attribute("RestartPolicy").unwrap_or("always") {
            "on-failure" => 1,
            "no" => 2,
            _ => 0,
        };
        let user = child.attribute("RunAsUser").map(ToString::to_string);

        section.add_symbol(Symbol::new("PosixDaemon", svc));

        let rec = Record::with_fields(vec![
            FieldValue::String(svc.to_string()),
            FieldValue::Long(sup_code),
            FieldValue::Null,
            FieldValue::Long(restart),
            user.map_or(FieldValue::Null, FieldValue::String),
        ]);
        tables
            .entry("PosixDaemon".to_string())
            .or_insert_with(|| IntermediateTable::new("PosixDaemon"))
            .push_record(rec);
    }

    /// Compiles `<posix:Acl>` element.
    fn compile_posix_acl(
        child: &XmlNode,
        section: &mut IntermediateSection,
        tables: &mut std::collections::HashMap<String, IntermediateTable>,
    ) {
        let acl_id = child.attribute("Id").unwrap_or("Acl1");
        let file = child.attribute("File").unwrap_or("File1");
        let p_type = child.attribute("PrincipalType").unwrap_or("user");
        let p_name = child.attribute("PrincipalName").unwrap_or("root");
        let perms: i32 = child
            .attribute("Permissions")
            .and_then(|p| p.parse().ok())
            .unwrap_or(7);

        section.add_symbol(Symbol::new("PosixAcl", acl_id));

        let rec = Record::with_fields(vec![
            FieldValue::String(acl_id.to_string()),
            FieldValue::String(file.to_string()),
            FieldValue::String(p_type.to_string()),
            FieldValue::String(p_name.to_string()),
            FieldValue::Long(perms),
            FieldValue::Long(0),
        ]);
        tables
            .entry("PosixAcl".to_string())
            .or_insert_with(|| IntermediateTable::new("PosixAcl"))
            .push_record(rec);
    }

    /// Compiles `<posix:Desktop>` element.
    fn compile_posix_desktop(
        child: &XmlNode,
        section: &mut IntermediateSection,
        tables: &mut std::collections::HashMap<String, IntermediateTable>,
    ) {
        let sc = child.attribute("Shortcut").unwrap_or("Shortcut1");
        let cat = child.attribute("Categories").map(ToString::to_string);
        let mime = child.attribute("MimeTypes").map(ToString::to_string);
        let term: i16 = i16::from(
            child
                .attribute("Terminal")
                .is_some_and(|v| v == "yes" || v == "true"),
        );

        section.add_symbol(Symbol::new("PosixDesktop", sc));

        let rec = Record::with_fields(vec![
            FieldValue::String(sc.to_string()),
            cat.map_or(FieldValue::Null, FieldValue::String),
            mime.map_or(FieldValue::Null, FieldValue::String),
            FieldValue::Null,
            FieldValue::Short(term),
        ]);
        tables
            .entry("PosixDesktop".to_string())
            .or_insert_with(|| IntermediateTable::new("PosixDesktop"))
            .push_record(rec);
    }

    /// Compiles a `<Configuration>` element in a Merge Module.
    fn compile_module_configuration(
        child: &XmlNode,
        tables: &mut std::collections::HashMap<String, IntermediateTable>,
    ) {
        let name = child.attribute("Name").unwrap_or("Param1");
        let format: i32 = child
            .attribute("Format")
            .and_then(|f| f.parse().ok())
            .unwrap_or(0);
        let row = crate::database::tables::core::ModuleConfigurationRow {
            name: name.to_string(),
            format,
            type_: child.attribute("Type").map(ToString::to_string),
            context_data: child.attribute("ContextData").map(ToString::to_string),
            default_value: child.attribute("DefaultValue").map(ToString::to_string),
            attributes: child.attribute("Attributes").and_then(|a| a.parse().ok()),
            display_name: child.attribute("DisplayName").map(ToString::to_string),
            description: child.attribute("Description").map(ToString::to_string),
            help_keyword: child.attribute("HelpKeyword").map(ToString::to_string),
        };
        tables
            .entry("ModuleConfiguration".to_string())
            .or_insert_with(|| IntermediateTable::new("ModuleConfiguration"))
            .push_record(row.to_record());
    }

    /// Compiles a `<Substitution>` element in a Merge Module.
    fn compile_module_substitution(
        child: &XmlNode,
        tables: &mut std::collections::HashMap<String, IntermediateTable>,
    ) {
        let table = child.attribute("Table").unwrap_or("Property");
        let row_key = child.attribute("Row").unwrap_or("");
        let column = child.attribute("Column").unwrap_or("Value");
        let value = child.attribute("Value").map(ToString::to_string);
        let row = crate::database::tables::core::ModuleSubstitutionRow {
            table: table.to_string(),
            row: row_key.to_string(),
            column: column.to_string(),
            value,
        };
        tables
            .entry("ModuleSubstitution".to_string())
            .or_insert_with(|| IntermediateTable::new("ModuleSubstitution"))
            .push_record(row.to_record());
    }

    /// Compiles an `<IgnoreModularization>` element in a Merge Module.
    fn compile_module_ignore_modularization(
        child: &XmlNode,
        tables: &mut std::collections::HashMap<String, IntermediateTable>,
    ) {
        let name = child.attribute("Name").unwrap_or("");
        let type_val = child.attribute("Type").and_then(|t| t.parse().ok());
        let row = crate::database::tables::core::ModuleIgnoreModularizationRow {
            name: name.to_string(),
            type_: type_val,
        };
        tables
            .entry("ModuleIgnoreModularization".to_string())
            .or_insert_with(|| IntermediateTable::new("ModuleIgnoreModularization"))
            .push_record(row.to_record());
    }

    /// Compiles a `<Dependency>` or `<ModuleDependency>` element in a Merge Module.
    fn compile_module_dependency(
        child: &XmlNode,
        tables: &mut std::collections::HashMap<String, IntermediateTable>,
    ) {
        let req_id = child
            .attribute("RequiredId")
            .or_else(|| child.attribute("Id"))
            .unwrap_or("");
        let req_lang = child
            .attribute("RequiredLanguage")
            .or_else(|| child.attribute("Language"))
            .and_then(|l| l.parse().ok())
            .unwrap_or(1033);
        let req_ver = child
            .attribute("RequiredVersion")
            .or_else(|| child.attribute("Version"))
            .map(ToString::to_string);
        let mod_id = child.attribute("ModuleId").unwrap_or("Module");
        let mod_lang = child
            .attribute("ModuleLanguage")
            .and_then(|l| l.parse().ok())
            .unwrap_or(1033);

        let row = crate::database::tables::core::ModuleDependencyRow {
            module_id: mod_id.to_string(),
            module_language: mod_lang,
            required_id: req_id.to_string(),
            required_language: req_lang,
            required_version: req_ver,
        };
        tables
            .entry("ModuleDependency".to_string())
            .or_insert_with(|| IntermediateTable::new("ModuleDependency"))
            .push_record(row.to_record());
    }

    /// Compiles an `<Exclusion>` or `<ModuleExclusion>` element in a Merge Module.
    fn compile_module_exclusion(
        child: &XmlNode,
        tables: &mut std::collections::HashMap<String, IntermediateTable>,
    ) {
        let excl_id = child
            .attribute("ExcludedId")
            .or_else(|| child.attribute("Id"))
            .unwrap_or("");
        let excl_lang = child
            .attribute("ExcludedLanguage")
            .or_else(|| child.attribute("Language"))
            .and_then(|l| l.parse().ok())
            .unwrap_or(1033);
        let ver_min = child
            .attribute("ExcludedVersionMin")
            .or_else(|| child.attribute("VersionMin"))
            .map(ToString::to_string);
        let ver_max = child
            .attribute("ExcludedVersionMax")
            .or_else(|| child.attribute("VersionMax"))
            .map(ToString::to_string);
        let mod_id = child.attribute("ModuleId").unwrap_or("Module");
        let mod_lang = child
            .attribute("ModuleLanguage")
            .and_then(|l| l.parse().ok())
            .unwrap_or(1033);

        let row = crate::database::tables::core::ModuleExclusionRow {
            module_id: mod_id.to_string(),
            module_language: mod_lang,
            excluded_id: excl_id.to_string(),
            excluded_language: excl_lang,
            excluded_version_min: ver_min,
            excluded_version_max: ver_max,
        };
        tables
            .entry("ModuleExclusion".to_string())
            .or_insert_with(|| IntermediateTable::new("ModuleExclusion"))
            .push_record(row.to_record());
    }

    /// Compiles a `<CopyFile>` element into `DuplicateFile` or `MoveFile` table.
    fn compile_copy_file(
        child: &XmlNode,
        parent_id: Option<&str>,
        section: &mut IntermediateSection,
        tables: &mut std::collections::HashMap<String, IntermediateTable>,
    ) {
        let id = child.attribute("Id").unwrap_or("CopyFile1");
        let comp = parent_id.unwrap_or("DefaultComp");
        section.add_symbol(Symbol::new("CopyFile", id));
        section.add_reference(Reference::new("Component", comp));

        if let Some(file_id) = child.attribute("FileId") {
            let dest_name = child.attribute("DestinationName");
            let dest_folder = child
                .attribute("DestinationDirectory")
                .or_else(|| child.attribute("DestinationProperty"));
            let rec = Record::with_fields(vec![
                FieldValue::String(id.to_string()),
                FieldValue::String(comp.to_string()),
                FieldValue::String(file_id.to_string()),
                dest_name.map_or(FieldValue::Null, |s| FieldValue::String(s.to_string())),
                dest_folder.map_or(FieldValue::Null, |s| FieldValue::String(s.to_string())),
            ]);
            tables
                .entry("DuplicateFile".to_string())
                .or_insert_with(|| IntermediateTable::new("DuplicateFile"))
                .push_record(rec);
        } else {
            let src_name = child.attribute("SourceName").unwrap_or("*");
            let dest_name = child.attribute("DestinationName").unwrap_or(src_name);
            let src_folder = child
                .attribute("SourceDirectory")
                .or_else(|| child.attribute("SourceProperty"))
                .unwrap_or("TARGETDIR");
            let dest_folder = child
                .attribute("DestinationDirectory")
                .or_else(|| child.attribute("DestinationProperty"))
                .unwrap_or("TARGETDIR");
            let delete = child
                .attribute("Delete")
                .is_some_and(|s| s.eq_ignore_ascii_case("yes"));
            let options: i16 = i16::from(delete);
            let rec = Record::with_fields(vec![
                FieldValue::String(id.to_string()),
                FieldValue::String(comp.to_string()),
                FieldValue::String(src_name.to_string()),
                FieldValue::String(dest_name.to_string()),
                FieldValue::String(src_folder.to_string()),
                FieldValue::String(dest_folder.to_string()),
                FieldValue::Short(options),
            ]);
            tables
                .entry("MoveFile".to_string())
                .or_insert_with(|| IntermediateTable::new("MoveFile"))
                .push_record(rec);
        }
    }

    /// Compiles a `<MoveFile>` element into `MoveFile` table.
    fn compile_move_file(
        child: &XmlNode,
        parent_id: Option<&str>,
        section: &mut IntermediateSection,
        tables: &mut std::collections::HashMap<String, IntermediateTable>,
    ) {
        let id = child.attribute("Id").unwrap_or("MoveFile1");
        let comp = parent_id.unwrap_or("DefaultComp");
        let src_name = child.attribute("SourceName").unwrap_or("*");
        let dest_name = child.attribute("DestName").unwrap_or(src_name);
        let src_folder = child.attribute("SourceFolder").unwrap_or("TARGETDIR");
        let dest_folder = child.attribute("DestFolder").unwrap_or("TARGETDIR");

        section.add_symbol(Symbol::new("MoveFile", id));
        section.add_reference(Reference::new("Component", comp));

        let rec = Record::with_fields(vec![
            FieldValue::String(id.to_string()),
            FieldValue::String(comp.to_string()),
            FieldValue::String(src_name.to_string()),
            FieldValue::String(dest_name.to_string()),
            FieldValue::String(src_folder.to_string()),
            FieldValue::String(dest_folder.to_string()),
            FieldValue::Short(1),
        ]);
        tables
            .entry("MoveFile".to_string())
            .or_insert_with(|| IntermediateTable::new("MoveFile"))
            .push_record(rec);
    }

    /// Compiles a `<RemoveFile>` element into `RemoveFile` table.
    fn compile_remove_file(
        child: &XmlNode,
        parent_id: Option<&str>,
        section: &mut IntermediateSection,
        tables: &mut std::collections::HashMap<String, IntermediateTable>,
    ) -> Result<()> {
        let id = child.attribute("Id").unwrap_or("RemoveFile1");
        let comp = parent_id.unwrap_or("DefaultComp");
        let name = child.attribute("Name");
        let dir = child
            .attribute("Directory")
            .or_else(|| child.attribute("Property"))
            .unwrap_or("TARGETDIR");
        let on = child.attribute("On").unwrap_or("uninstall");
        let mode: i16 = match on {
            "install" => 1,
            "both" => 3,
            _ => 2,
        };

        section.add_symbol(Symbol::new("RemoveFile", id));
        section.add_reference(Reference::new("Component", comp));

        let row = RemoveFileRow {
            file_key: id.to_string(),
            component: ComponentName::new(comp)?,
            file_name: name.map(ToString::to_string),
            dir_property: dir.to_string(),
            install_mode: mode,
        };
        tables
            .entry("RemoveFile".to_string())
            .or_insert_with(|| IntermediateTable::new("RemoveFile"))
            .push_record(row.to_record());
        Ok(())
    }

    /// Compiles a `<SymbolicLink>` or `<Hardlink>` element into `PosixSymlink` table.
    fn compile_symlink(
        child: &XmlNode,
        parent_id: Option<&str>,
        section: &mut IntermediateSection,
        tables: &mut std::collections::HashMap<String, IntermediateTable>,
    ) -> Result<()> {
        let id = child.attribute("Id").unwrap_or("Symlink1");
        let target = child
            .attribute("Target")
            .or_else(|| child.attribute("TargetPath"))
            .unwrap_or("");
        let dir = child
            .attribute("Directory")
            .or(parent_id)
            .unwrap_or("TARGETDIR");
        let name = child.attribute("Name").unwrap_or(id);
        let comp = parent_id.unwrap_or("DefaultComp");

        section.add_symbol(Symbol::new("SymbolicLink", id));
        section.add_reference(Reference::new("Component", comp));

        let row = crate::database::tables::posix::PosixSymlinkRow {
            symlink_key: id.to_string(),
            target_path: target.to_string(),
            link_directory: DirectoryId::new(dir)?,
            link_name: name.to_string(),
            component: ComponentName::new(comp)?,
        };
        tables
            .entry("PosixSymlink".to_string())
            .or_insert_with(|| IntermediateTable::new("PosixSymlink"))
            .push_record(row.to_record());
        Ok(())
    }

    /// Compiles a `<MediaTemplate>` element into a synthesized `Media` table entry.
    fn compile_media_template(
        child: &XmlNode,
        tables: &mut std::collections::HashMap<String, IntermediateTable>,
    ) {
        let cab_template = child.attribute("CabinetTemplate").unwrap_or("cab1.cab");
        let cab_name = cab_template.replace("{0}", "1");
        let disk_prompt = child.attribute("DiskPrompt").map(ToString::to_string);
        let row = MediaRow {
            disk_id: 1,
            last_sequence: 1,
            disk_prompt,
            cabinet: Some(cab_name),
            volume_label: None,
            source: None,
        };
        tables
            .entry("Media".to_string())
            .or_insert_with(|| IntermediateTable::new("Media"))
            .push_record(row.to_record());
    }

    /// Compiles `<RemoveRegistryKey>` or `<RemoveRegistryValue>` into `RemoveRegistry` table.
    fn compile_remove_registry(
        child: &XmlNode,
        parent_id: Option<&str>,
        section: &mut IntermediateSection,
        tables: &mut std::collections::HashMap<String, IntermediateTable>,
    ) {
        let id = child.attribute("Id").unwrap_or("RemoveReg1");
        let root_str = child.attribute("Root").unwrap_or("HKLM");
        let root = parse_registry_root(root_str);
        let key = child.attribute("Key").unwrap_or("");
        let name = child.attribute("Name").map(ToString::to_string);
        let comp = parent_id.unwrap_or("DefaultComp");

        section.add_symbol(Symbol::new("RemoveRegistry", id));
        section.add_reference(Reference::new("Component", comp));

        let rec = Record::with_fields(vec![
            FieldValue::String(id.to_string()),
            FieldValue::Short(root),
            FieldValue::String(key.to_string()),
            name.map_or(FieldValue::Null, FieldValue::String),
            FieldValue::String(comp.to_string()),
        ]);
        tables
            .entry("RemoveRegistry".to_string())
            .or_insert_with(|| IntermediateTable::new("RemoveRegistry"))
            .push_record(rec);
    }

    /// Compiles an `<IniFile>` element into `IniFile` table.
    fn compile_ini_file(
        child: &XmlNode,
        parent_id: Option<&str>,
        section: &mut IntermediateSection,
        tables: &mut std::collections::HashMap<String, IntermediateTable>,
    ) {
        let id = child.attribute("Id").unwrap_or("Ini1");
        let file_name = child.attribute("Name").unwrap_or("config.ini");
        let dir = child.attribute("Directory");
        let section_name = child.attribute("Section").unwrap_or("Settings");
        let key = child.attribute("Key").unwrap_or("Key");
        let value = child.attribute("Value").unwrap_or("");
        let action_str = child.attribute("Action").unwrap_or("addLine");
        let action: i16 = match action_str {
            "createLine" => 1,
            "replaceLine" => 2,
            "addTag" => 3,
            "removeTag" => 4,
            _ => 0,
        };
        let comp = parent_id.unwrap_or("DefaultComp");

        section.add_symbol(Symbol::new("IniFile", id));
        section.add_reference(Reference::new("Component", comp));

        let rec = Record::with_fields(vec![
            FieldValue::String(id.to_string()),
            FieldValue::String(file_name.to_string()),
            dir.map_or(FieldValue::Null, |d| FieldValue::String(d.to_string())),
            FieldValue::String(section_name.to_string()),
            FieldValue::String(key.to_string()),
            FieldValue::String(value.to_string()),
            FieldValue::Short(action),
            FieldValue::String(comp.to_string()),
        ]);
        tables
            .entry("IniFile".to_string())
            .or_insert_with(|| IntermediateTable::new("IniFile"))
            .push_record(rec);
    }

    /// Compiles a `<RemoveIniFile>` element into `RemoveIniFile` table.
    fn compile_remove_ini_file(
        child: &XmlNode,
        parent_id: Option<&str>,
        section: &mut IntermediateSection,
        tables: &mut std::collections::HashMap<String, IntermediateTable>,
    ) {
        let id = child.attribute("Id").unwrap_or("RemoveIni1");
        let file_name = child.attribute("Name").unwrap_or("config.ini");
        let dir = child.attribute("Directory");
        let section_name = child.attribute("Section").unwrap_or("Settings");
        let key = child.attribute("Key").unwrap_or("Key");
        let value = child.attribute("Value");
        let comp = parent_id.unwrap_or("DefaultComp");

        section.add_symbol(Symbol::new("RemoveIniFile", id));
        section.add_reference(Reference::new("Component", comp));

        let rec = Record::with_fields(vec![
            FieldValue::String(id.to_string()),
            FieldValue::String(file_name.to_string()),
            dir.map_or(FieldValue::Null, |d| FieldValue::String(d.to_string())),
            FieldValue::String(section_name.to_string()),
            FieldValue::String(key.to_string()),
            value.map_or(FieldValue::Null, |v| FieldValue::String(v.to_string())),
            FieldValue::Short(4),
            FieldValue::String(comp.to_string()),
        ]);
        tables
            .entry("RemoveIniFile".to_string())
            .or_insert_with(|| IntermediateTable::new("RemoveIniFile"))
            .push_record(rec);
    }

    /// Compiles an `<Icon>` element into `Icon` table.
    fn compile_icon(
        child: &XmlNode,
        section: &mut IntermediateSection,
        tables: &mut std::collections::HashMap<String, IntermediateTable>,
    ) {
        let id = child.attribute("Id").unwrap_or("AppIcon");
        section.add_symbol(Symbol::new("Icon", id));
        let rec = Record::with_fields(vec![
            FieldValue::String(id.to_string()),
            FieldValue::Stream(crate::database::tables::types::StringPoolId::new(1)),
        ]);
        tables
            .entry("Icon".to_string())
            .or_insert_with(|| IntermediateTable::new("Icon"))
            .push_record(rec);
    }

    /// Compiles a `<ProgId>` element into `ProgId` table.
    fn compile_prog_id(
        child: &XmlNode,
        parent_id: Option<&str>,
        section: &mut IntermediateSection,
        tables: &mut std::collections::HashMap<String, IntermediateTable>,
    ) {
        let id = child.attribute("Id").unwrap_or("App.Document");
        let desc = child.attribute("Description").map(ToString::to_string);
        let icon = child.attribute("Icon").map(ToString::to_string);
        let icon_idx: Option<i16> = child.attribute("IconIndex").and_then(|i| i.parse().ok());
        let class_id = parent_id.map(ToString::to_string);

        section.add_symbol(Symbol::new("ProgId", id));

        let rec = Record::with_fields(vec![
            FieldValue::String(id.to_string()),
            FieldValue::Null,
            class_id.map_or(FieldValue::Null, FieldValue::String),
            desc.map_or(FieldValue::Null, FieldValue::String),
            icon.map_or(FieldValue::Null, FieldValue::String),
            icon_idx.map_or(FieldValue::Null, FieldValue::Short),
        ]);
        tables
            .entry("ProgId".to_string())
            .or_insert_with(|| IntermediateTable::new("ProgId"))
            .push_record(rec);
    }

    /// Compiles an `<Extension>` element into `Extension` and `Verb` tables.
    fn compile_extension(
        child: &XmlNode,
        parent_id: Option<&str>,
        section: &mut IntermediateSection,
        tables: &mut std::collections::HashMap<String, IntermediateTable>,
    ) {
        let ext = child.attribute("Id").unwrap_or("txt");
        let comp = parent_id.unwrap_or("DefaultComp");
        let prog_id = child.attribute("ProgId");
        let mime = child
            .attribute("MIME")
            .or_else(|| child.attribute("ContentType"));

        section.add_symbol(Symbol::new("Extension", ext));
        section.add_reference(Reference::new("Component", comp));

        let rec = Record::with_fields(vec![
            FieldValue::String(ext.to_string()),
            FieldValue::String(comp.to_string()),
            prog_id.map_or(FieldValue::Null, |p| FieldValue::String(p.to_string())),
            mime.map_or(FieldValue::Null, |m| FieldValue::String(m.to_string())),
            FieldValue::Null,
        ]);
        tables
            .entry("Extension".to_string())
            .or_insert_with(|| IntermediateTable::new("Extension"))
            .push_record(rec);

        for sub in &child.children {
            if sub.tag == "Verb" {
                let verb_id = sub.attribute("Id").unwrap_or("open");
                let cmd = sub.attribute("Command").map(ToString::to_string);
                let arg = sub.attribute("Argument").map(ToString::to_string);
                let seq: Option<i16> = sub.attribute("Sequence").and_then(|s| s.parse().ok());

                let verb_rec = Record::with_fields(vec![
                    FieldValue::String(ext.to_string()),
                    FieldValue::String(verb_id.to_string()),
                    seq.map_or(FieldValue::Null, FieldValue::Short),
                    cmd.map_or(FieldValue::Null, FieldValue::String),
                    arg.map_or(FieldValue::Null, FieldValue::String),
                ]);
                tables
                    .entry("Verb".to_string())
                    .or_insert_with(|| IntermediateTable::new("Verb"))
                    .push_record(verb_rec);
            }
        }
    }

    /// Compiles a `<MIME>` element into `MIME` table.
    fn compile_mime(
        child: &XmlNode,
        section: &mut IntermediateSection,
        tables: &mut std::collections::HashMap<String, IntermediateTable>,
    ) {
        let content_type = child
            .attribute("ContentType")
            .unwrap_or("application/octet-stream");
        let ext = child
            .attribute("DefaultExtension")
            .or_else(|| child.attribute("Extension"))
            .unwrap_or("");
        let clsid = child
            .attribute("CLSID")
            .or_else(|| child.attribute("Class"));

        section.add_symbol(Symbol::new("MIME", content_type));

        let rec = Record::with_fields(vec![
            FieldValue::String(content_type.to_string()),
            FieldValue::String(ext.to_string()),
            clsid.map_or(FieldValue::Null, |c| FieldValue::String(c.to_string())),
        ]);
        tables
            .entry("MIME".to_string())
            .or_insert_with(|| IntermediateTable::new("MIME"))
            .push_record(rec);
    }

    /// Compiles a `<Class>` element into `Class` table.
    fn compile_class(
        child: &XmlNode,
        parent_id: Option<&str>,
        section: &mut IntermediateSection,
        tables: &mut std::collections::HashMap<String, IntermediateTable>,
    ) {
        let clsid = child
            .attribute("Id")
            .unwrap_or("{00000000-0000-0000-0000-000000000000}");
        let context = child.attribute("Context").unwrap_or("InprocServer32");
        let comp = parent_id.unwrap_or("DefaultComp");
        let desc = child.attribute("Description").map(ToString::to_string);
        let prog_id = child.attribute("ProgId");
        let app_id = child.attribute("AppId");

        section.add_symbol(Symbol::new("Class", clsid));
        section.add_reference(Reference::new("Component", comp));

        let rec = Record::with_fields(vec![
            FieldValue::String(clsid.to_string()),
            FieldValue::String(context.to_string()),
            FieldValue::String(comp.to_string()),
            prog_id.map_or(FieldValue::Null, |p| FieldValue::String(p.to_string())),
            desc.map_or(FieldValue::Null, FieldValue::String),
            app_id.map_or(FieldValue::Null, |a| FieldValue::String(a.to_string())),
            FieldValue::Null,
            FieldValue::Null,
            FieldValue::Null,
            FieldValue::Null,
            FieldValue::Null,
            FieldValue::Null,
        ]);
        tables
            .entry("Class".to_string())
            .or_insert_with(|| IntermediateTable::new("Class"))
            .push_record(rec);
    }

    /// Compiles a `<TypeLib>` element into `TypeLib` table.
    fn compile_type_lib(
        child: &XmlNode,
        parent_id: Option<&str>,
        section: &mut IntermediateSection,
        tables: &mut std::collections::HashMap<String, IntermediateTable>,
    ) {
        let lib_id = child
            .attribute("Id")
            .unwrap_or("{00000000-0000-0000-0000-000000000000}");
        let lang: i16 = child
            .attribute("Language")
            .and_then(|l| l.parse().ok())
            .unwrap_or(0);
        let comp = parent_id.unwrap_or("DefaultComp");
        let desc = child.attribute("Description").map(ToString::to_string);
        let dir = child.attribute("Directory");

        section.add_symbol(Symbol::new("TypeLib", lib_id));
        section.add_reference(Reference::new("Component", comp));

        let rec = Record::with_fields(vec![
            FieldValue::String(lib_id.to_string()),
            FieldValue::Short(lang),
            FieldValue::String(comp.to_string()),
            FieldValue::Null,
            desc.map_or(FieldValue::Null, FieldValue::String),
            dir.map_or(FieldValue::Null, |d| FieldValue::String(d.to_string())),
            FieldValue::Null,
            FieldValue::Null,
        ]);
        tables
            .entry("TypeLib".to_string())
            .or_insert_with(|| IntermediateTable::new("TypeLib"))
            .push_record(rec);
    }

    /// Compiles an `<AppId>` element into `AppId` table.
    fn compile_app_id(
        child: &XmlNode,
        section: &mut IntermediateSection,
        tables: &mut std::collections::HashMap<String, IntermediateTable>,
    ) {
        let app_id = child
            .attribute("Id")
            .unwrap_or("{00000000-0000-0000-0000-000000000000}");
        let remote_server = child.attribute("RemoteServerName").map(ToString::to_string);
        let local_service = child.attribute("LocalService").map(ToString::to_string);

        section.add_symbol(Symbol::new("AppId", app_id));

        let rec = Record::with_fields(vec![
            FieldValue::String(app_id.to_string()),
            remote_server.map_or(FieldValue::Null, FieldValue::String),
            local_service.map_or(FieldValue::Null, FieldValue::String),
            FieldValue::Null,
            FieldValue::Null,
            FieldValue::Null,
            FieldValue::Null,
        ]);
        tables
            .entry("AppId".to_string())
            .or_insert_with(|| IntermediateTable::new("AppId"))
            .push_record(rec);
    }

    /// Compiles a `<SetProperty>` element (syntactic sugar for Type 51 custom action).
    fn compile_set_property(
        child: &XmlNode,
        section: &mut IntermediateSection,
        tables: &mut std::collections::HashMap<String, IntermediateTable>,
    ) -> Result<()> {
        let prop = child
            .attribute("Id")
            .or_else(|| child.attribute("Property"))
            .unwrap_or("PROP");
        let val = child.attribute("Value").unwrap_or("");
        let action_name = child
            .attribute("Action")
            .map_or_else(|| format!("Set_{prop}"), ToString::to_string);

        section.add_symbol(Symbol::new("CustomAction", &action_name));

        let ca_rec = Record::with_fields(vec![
            FieldValue::String(action_name.clone()),
            FieldValue::Short(51),
            FieldValue::String(prop.to_string()),
            FieldValue::String(val.to_string()),
            FieldValue::Null,
        ]);
        tables
            .entry("CustomAction".to_string())
            .or_insert_with(|| IntermediateTable::new("CustomAction"))
            .push_record(ca_rec);

        let seq_num = child.attribute("Sequence").and_then(|s| s.parse().ok());
        let cond = child
            .children
            .iter()
            .find(|c| c.tag == "Condition")
            .map(|c| c.text.clone())
            .or_else(|| child.attribute("Condition").map(ToString::to_string));
        let seq_row = SequenceRow::new(&action_name, cond, seq_num)?;
        tables
            .entry("InstallExecuteSequence".to_string())
            .or_insert_with(|| IntermediateTable::new("InstallExecuteSequence"))
            .push_record(seq_row.to_record());
        Ok(())
    }

    /// Compiles `<AppSearch>` element into `AppSearch` table.
    fn compile_app_search(
        child: &XmlNode,
        section: &mut IntermediateSection,
        tables: &mut std::collections::HashMap<String, IntermediateTable>,
    ) {
        let prop = child.attribute("Property").unwrap_or("PROP");
        let sig = child.attribute("Id").unwrap_or(prop);
        section.add_symbol(Symbol::new("AppSearch", prop));
        let rec = Record::with_fields(vec![
            FieldValue::String(prop.to_string()),
            FieldValue::String(sig.to_string()),
        ]);
        tables
            .entry("AppSearch".to_string())
            .or_insert_with(|| IntermediateTable::new("AppSearch"))
            .push_record(rec);
    }

    /// Compiles `<RegistrySearch>` element into `RegLocator` table.
    fn compile_registry_search(
        child: &XmlNode,
        parent_id: Option<&str>,
        section: &mut IntermediateSection,
        tables: &mut std::collections::HashMap<String, IntermediateTable>,
    ) {
        let sig = child.attribute("Id").unwrap_or("RegSearch1");
        let root_str = child.attribute("Root").unwrap_or("HKLM");
        let root = parse_registry_root(root_str);
        let key = child.attribute("Key").unwrap_or("");
        let name = child.attribute("Name").map(ToString::to_string);
        let type_str = child.attribute("Type").unwrap_or("raw");
        let type_num: i16 = match type_str {
            "directory" => 0,
            "file" => 1,
            _ => 2,
        };

        section.add_symbol(Symbol::new("Signature", sig));
        if let Some(parent_prop) = parent_id {
            let app_search_rec = Record::with_fields(vec![
                FieldValue::String(parent_prop.to_string()),
                FieldValue::String(sig.to_string()),
            ]);
            tables
                .entry("AppSearch".to_string())
                .or_insert_with(|| IntermediateTable::new("AppSearch"))
                .push_record(app_search_rec);
        }

        let rec = Record::with_fields(vec![
            FieldValue::String(sig.to_string()),
            FieldValue::Short(root),
            FieldValue::String(key.to_string()),
            name.map_or(FieldValue::Null, FieldValue::String),
            FieldValue::Short(type_num),
        ]);
        tables
            .entry("RegLocator".to_string())
            .or_insert_with(|| IntermediateTable::new("RegLocator"))
            .push_record(rec);
    }

    /// Compiles `<DirectorySearch>` element into `DrLocator` table.
    fn compile_directory_search(
        child: &XmlNode,
        parent_id: Option<&str>,
        section: &mut IntermediateSection,
        tables: &mut std::collections::HashMap<String, IntermediateTable>,
    ) {
        let sig = child.attribute("Id").unwrap_or("DirSearch1");
        let path = child.attribute("Path").map(ToString::to_string);
        let depth: Option<i16> = child.attribute("Depth").and_then(|d| d.parse().ok());
        let parent = child.attribute("Parent").or(parent_id);

        section.add_symbol(Symbol::new("Signature", sig));

        let rec = Record::with_fields(vec![
            FieldValue::String(sig.to_string()),
            parent.map_or(FieldValue::Null, |p| FieldValue::String(p.to_string())),
            path.map_or(FieldValue::Null, FieldValue::String),
            depth.map_or(FieldValue::Null, FieldValue::Short),
        ]);
        tables
            .entry("DrLocator".to_string())
            .or_insert_with(|| IntermediateTable::new("DrLocator"))
            .push_record(rec);
    }

    /// Compiles `<FileSearch>` element into `FileSearch` and `Signature` tables.
    fn compile_file_search(
        child: &XmlNode,
        parent_id: Option<&str>,
        section: &mut IntermediateSection,
        tables: &mut std::collections::HashMap<String, IntermediateTable>,
    ) {
        let sig = child.attribute("Id").unwrap_or("FileSearch1");
        let name = child.attribute("Name").unwrap_or("target.exe");
        let min_ver = child.attribute("MinVersion").map(ToString::to_string);
        let max_ver = child.attribute("MaxVersion").map(ToString::to_string);

        section.add_symbol(Symbol::new("Signature", sig));

        let rec = Record::with_fields(vec![
            FieldValue::String(sig.to_string()),
            FieldValue::String(name.to_string()),
            min_ver.map_or(FieldValue::Null, FieldValue::String),
            max_ver.map_or(FieldValue::Null, FieldValue::String),
            FieldValue::Null,
            FieldValue::Null,
            FieldValue::Null,
            FieldValue::Null,
            FieldValue::Null,
        ]);
        tables
            .entry("Signature".to_string())
            .or_insert_with(|| IntermediateTable::new("Signature"))
            .push_record(rec);

        if let Some(prop) = parent_id {
            let app_rec = Record::with_fields(vec![
                FieldValue::String(prop.to_string()),
                FieldValue::String(sig.to_string()),
            ]);
            tables
                .entry("AppSearch".to_string())
                .or_insert_with(|| IntermediateTable::new("AppSearch"))
                .push_record(app_rec);
        }
    }

    /// Compiles `<IniFileSearch>` element into `IniLocator` table.
    fn compile_ini_file_search(
        child: &XmlNode,
        parent_id: Option<&str>,
        section: &mut IntermediateSection,
        tables: &mut std::collections::HashMap<String, IntermediateTable>,
    ) {
        let sig = child.attribute("Id").unwrap_or("IniSearch1");
        let file = child
            .attribute("Name")
            .or_else(|| child.attribute("FileName"))
            .unwrap_or("app.ini");
        let section_name = child.attribute("Section").unwrap_or("Config");
        let key = child.attribute("Key").unwrap_or("Key");
        let field: Option<i16> = child.attribute("Field").and_then(|f| f.parse().ok());
        let type_num: Option<i16> = child.attribute("Type").and_then(|t| t.parse().ok());

        section.add_symbol(Symbol::new("Signature", sig));

        let rec = Record::with_fields(vec![
            FieldValue::String(sig.to_string()),
            FieldValue::String(file.to_string()),
            FieldValue::String(section_name.to_string()),
            FieldValue::String(key.to_string()),
            field.map_or(FieldValue::Null, FieldValue::Short),
            type_num.map_or(FieldValue::Null, FieldValue::Short),
        ]);
        tables
            .entry("IniLocator".to_string())
            .or_insert_with(|| IntermediateTable::new("IniLocator"))
            .push_record(rec);

        if let Some(prop) = parent_id {
            let app_rec = Record::with_fields(vec![
                FieldValue::String(prop.to_string()),
                FieldValue::String(sig.to_string()),
            ]);
            tables
                .entry("AppSearch".to_string())
                .or_insert_with(|| IntermediateTable::new("AppSearch"))
                .push_record(app_rec);
        }
    }

    /// Compiles `<ComponentSearch>` element into `CompLocator` table.
    fn compile_component_search(
        child: &XmlNode,
        parent_id: Option<&str>,
        section: &mut IntermediateSection,
        tables: &mut std::collections::HashMap<String, IntermediateTable>,
    ) {
        let sig = child.attribute("Id").unwrap_or("CompSearch1");
        let guid = child
            .attribute("Guid")
            .unwrap_or("{00000000-0000-0000-0000-000000000000}");
        let type_num: Option<i16> = child.attribute("Type").and_then(|t| t.parse().ok());

        section.add_symbol(Symbol::new("Signature", sig));

        let rec = Record::with_fields(vec![
            FieldValue::String(sig.to_string()),
            FieldValue::String(guid.to_string()),
            type_num.map_or(FieldValue::Null, FieldValue::Short),
        ]);
        tables
            .entry("CompLocator".to_string())
            .or_insert_with(|| IntermediateTable::new("CompLocator"))
            .push_record(rec);

        if let Some(prop) = parent_id {
            let app_rec = Record::with_fields(vec![
                FieldValue::String(prop.to_string()),
                FieldValue::String(sig.to_string()),
            ]);
            tables
                .entry("AppSearch".to_string())
                .or_insert_with(|| IntermediateTable::new("AppSearch"))
                .push_record(app_rec);
        }
    }

    /// Compiles a `<Launch>` or top-level `<Condition>` element into `LaunchCondition` table.
    fn compile_launch_condition(
        child: &XmlNode,
        tables: &mut std::collections::HashMap<String, IntermediateTable>,
    ) {
        let message = child
            .attribute("Message")
            .unwrap_or("System requirements not met.");
        let condition = child
            .attribute("Condition")
            .or_else(|| child.attribute("Message"))
            .map_or_else(
                || {
                    if child.text.is_empty() {
                        "1"
                    } else {
                        &child.text
                    }
                },
                |c| c,
            );
        let rec = Record::with_fields(vec![
            FieldValue::String(condition.to_string()),
            FieldValue::String(message.to_string()),
        ]);
        tables
            .entry("LaunchCondition".to_string())
            .or_insert_with(|| IntermediateTable::new("LaunchCondition"))
            .push_record(rec);
    }

    /// Compiles a sequence table container (e.g. `<InstallExecuteSequence>`, `<InstallUISequence>`).
    fn compile_sequence_table(
        child: &XmlNode,
        section: &mut IntermediateSection,
        tables: &mut std::collections::HashMap<String, IntermediateTable>,
    ) -> Result<()> {
        let table_name = &child.tag;
        for sub in &child.children {
            let action_name = sub
                .attribute("Action")
                .or_else(|| sub.attribute("Id"))
                .unwrap_or(&sub.tag);
            let cond = sub
                .attribute("Condition")
                .map(ToString::to_string)
                .or_else(|| {
                    if sub.text.is_empty() {
                        None
                    } else {
                        Some(sub.text.clone())
                    }
                });
            let seq: Option<i16> = sub.attribute("Sequence").and_then(|s| s.parse().ok());
            section.add_reference(Reference::new("Action", action_name));
            let seq_row = SequenceRow::new(action_name, cond, seq)?;
            tables
                .entry(table_name.clone())
                .or_insert_with(|| IntermediateTable::new(table_name))
                .push_record(seq_row.to_record());
        }
        Ok(())
    }

    /// Compiles patch-related authoring elements.
    fn compile_patch_element(
        child: &XmlNode,
        section: &mut IntermediateSection,
        tables: &mut std::collections::HashMap<String, IntermediateTable>,
    ) {
        let id = child.attribute("Id").unwrap_or("Patch1");
        section.add_symbol(Symbol::new("Patch", id));
        let row = crate::database::tables::core::PatchPackageRow {
            patch_id: id.to_string(),
            media: 1,
        };
        tables
            .entry("PatchPackage".to_string())
            .or_insert_with(|| IntermediateTable::new("PatchPackage"))
            .push_record(row.to_record());
    }

    /// Compiles a `<UIRef>` element, adding a reference to the target UI dialog set.
    fn compile_ui_ref(
        child: &XmlNode,
        section: &mut IntermediateSection,
        _tables: &mut std::collections::HashMap<String, IntermediateTable>,
    ) {
        let id = child.attribute("Id").unwrap_or("WixUI_InstallDir");
        section.add_reference(Reference::new("UI", id));
    }

    /// Compiles `<ControlEvent>` or `<Publish>` element into `ControlEvent` table.
    fn compile_control_event(
        child: &XmlNode,
        parent_id: Option<&str>,
        tables: &mut std::collections::HashMap<String, IntermediateTable>,
    ) {
        let dialog = child.attribute("Dialog").unwrap_or("Dialog1");
        let control = parent_id
            .or_else(|| child.attribute("Control"))
            .unwrap_or("BtnNext");
        let event = child.attribute("Event").unwrap_or("NewDialog");
        let arg = child
            .attribute("Argument")
            .or_else(|| child.attribute("Value"))
            .unwrap_or("");
        let cond = child
            .attribute("Condition")
            .map(ToString::to_string)
            .or_else(|| {
                if child.text.is_empty() {
                    None
                } else {
                    Some(child.text.clone())
                }
            });
        let order: Option<i16> = child.attribute("Order").and_then(|o| o.parse().ok());

        let rec = Record::with_fields(vec![
            FieldValue::String(dialog.to_string()),
            FieldValue::String(control.to_string()),
            FieldValue::String(event.to_string()),
            FieldValue::String(arg.to_string()),
            cond.map_or(FieldValue::Null, FieldValue::String),
            order.map_or(FieldValue::Null, FieldValue::Short),
        ]);
        tables
            .entry("ControlEvent".to_string())
            .or_insert_with(|| IntermediateTable::new("ControlEvent"))
            .push_record(rec);
    }

    /// Compiles `<ControlCondition>` into `ControlCondition` table.
    fn compile_control_condition(
        child: &XmlNode,
        parent_id: Option<&str>,
        tables: &mut std::collections::HashMap<String, IntermediateTable>,
    ) {
        let dialog = child.attribute("Dialog").unwrap_or("Dialog1");
        let control = parent_id
            .or_else(|| child.attribute("Control"))
            .unwrap_or("Control1");
        let action = child.attribute("Action").unwrap_or("enable");
        let cond = child.attribute("Condition").map_or_else(
            || {
                if child.text.is_empty() {
                    "1"
                } else {
                    &child.text
                }
            },
            |c| c,
        );

        let rec = Record::with_fields(vec![
            FieldValue::String(dialog.to_string()),
            FieldValue::String(control.to_string()),
            FieldValue::String(action.to_string()),
            FieldValue::String(cond.to_string()),
        ]);
        tables
            .entry("ControlCondition".to_string())
            .or_insert_with(|| IntermediateTable::new("ControlCondition"))
            .push_record(rec);
    }

    /// Compiles `<Subscribe>` element into `EventMapping` table.
    fn compile_subscribe(
        child: &XmlNode,
        parent_id: Option<&str>,
        tables: &mut std::collections::HashMap<String, IntermediateTable>,
    ) {
        let dialog = child.attribute("Dialog").unwrap_or("Dialog1");
        let control = parent_id
            .or_else(|| child.attribute("Control"))
            .unwrap_or("Control1");
        let event = child.attribute("Event").unwrap_or("SetProgress");
        let attr = child.attribute("Attribute").unwrap_or("Progress");

        let rec = Record::with_fields(vec![
            FieldValue::String(dialog.to_string()),
            FieldValue::String(control.to_string()),
            FieldValue::String(event.to_string()),
            FieldValue::String(attr.to_string()),
        ]);
        tables
            .entry("EventMapping".to_string())
            .or_insert_with(|| IntermediateTable::new("EventMapping"))
            .push_record(rec);
    }

    /// Compiles a `<Binary>` element into `Binary` table.
    fn compile_binary(
        child: &XmlNode,
        section: &mut IntermediateSection,
        tables: &mut std::collections::HashMap<String, IntermediateTable>,
    ) {
        let id = child.attribute("Id").unwrap_or("Binary1");
        section.add_symbol(Symbol::new("Binary", id));
        let row = crate::database::tables::core::BinaryRow {
            name: id.to_string(),
            data: crate::database::tables::types::StringPoolId::new(1),
        };
        tables
            .entry("Binary".to_string())
            .or_insert_with(|| IntermediateTable::new("Binary"))
            .push_record(row.to_record());
    }

    /// Compiles `<Billboard>` and `<BillboardAction>` elements.
    fn compile_billboard(
        child: &XmlNode,
        section: &mut IntermediateSection,
        tables: &mut std::collections::HashMap<String, IntermediateTable>,
    ) {
        let id = child.attribute("Id").unwrap_or("BB1");
        let feat = child.attribute("Feature").unwrap_or("Main");
        let action = child.attribute("Action").unwrap_or("InstallFiles");
        let ordering: Option<i16> = child.attribute("Ordering").and_then(|o| o.parse().ok());

        section.add_symbol(Symbol::new("Billboard", id));

        let rec = Record::with_fields(vec![
            FieldValue::String(id.to_string()),
            FieldValue::String(feat.to_string()),
            FieldValue::String(action.to_string()),
            ordering.map_or(FieldValue::Null, FieldValue::Short),
        ]);
        tables
            .entry("Billboard".to_string())
            .or_insert_with(|| IntermediateTable::new("Billboard"))
            .push_record(rec);
    }

    /// Compiles a `<ProgressText>` element into `ActionText` table.
    fn compile_progress_text(
        child: &XmlNode,
        tables: &mut std::collections::HashMap<String, IntermediateTable>,
    ) {
        let action = child.attribute("Action").unwrap_or("InstallFiles");
        let template = child.attribute("Template").map(ToString::to_string);
        let desc = if child.text.is_empty() {
            action.to_string()
        } else {
            child.text.clone()
        };
        let rec = Record::with_fields(vec![
            FieldValue::String(action.to_string()),
            FieldValue::String(desc),
            template.map_or(FieldValue::Null, FieldValue::String),
        ]);
        tables
            .entry("ActionText".to_string())
            .or_insert_with(|| IntermediateTable::new("ActionText"))
            .push_record(rec);
    }

    /// Compiles an `<Error>` element into `Error` table.
    fn compile_error(
        child: &XmlNode,
        tables: &mut std::collections::HashMap<String, IntermediateTable>,
    ) {
        let id: i16 = child
            .attribute("Id")
            .and_then(|i| i.parse().ok())
            .unwrap_or(1001);
        let message = child.attribute("Message").map_or_else(
            || {
                if child.text.is_empty() {
                    "Error occurred"
                } else {
                    &child.text
                }
            },
            |m| m,
        );
        let rec = Record::with_fields(vec![
            FieldValue::Short(id),
            FieldValue::String(message.to_string()),
        ]);
        tables
            .entry("Error".to_string())
            .or_insert_with(|| IntermediateTable::new("Error"))
            .push_record(rec);
    }

    /// Compiles common `WiX` extension elements (e.g. `util:*`, `netfx:*`, `firewall:*`, `sql:*`, `iis:*`, `dep:*`, `bal:*`, `http:*`).
    fn compile_extension_element(
        child: &XmlNode,
        parent_id: Option<&str>,
        section: &mut IntermediateSection,
        tables: &mut std::collections::HashMap<String, IntermediateTable>,
    ) {
        let id = child.attribute("Id").unwrap_or("ExtensionItem");
        section.add_symbol(Symbol::new("Extension", id));
        if let Some(parent) = parent_id {
            section.add_reference(Reference::new("Component", parent));
        }

        let tag = &child.tag;
        let mut fields = vec![
            FieldValue::String(id.to_string()),
            FieldValue::String(tag.clone()),
        ];
        if let Some(val) = child
            .attribute("Value")
            .or_else(|| child.attribute("Name"))
            .or_else(|| child.attribute("Target"))
        {
            fields.push(FieldValue::String(val.to_string()));
        }
        let rec = Record::with_fields(fields);
        tables
            .entry(format!("_{tag}"))
            .or_insert_with(|| IntermediateTable::new(format!("_{tag}")))
            .push_record(rec);
    }
}

/// Helper to parse registry root strings to standard integers (`0` HKCR, `1` HKCU, `2` HKLM, `3` HKU).
fn parse_registry_root(root_str: &str) -> i16 {
    match root_str.to_ascii_uppercase().as_str() {
        "HKCR" | "HKEY_CLASSES_ROOT" => 0,
        "HKCU" | "HKEY_CURRENT_USER" => 1,
        "HKU" | "HKEY_USERS" => 3,
        _ => 2,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::wix::xml::XmlParser;

    #[test]
    fn test_compiler_basic() -> Result<()> {
        let xml = r#"
<Wix xmlns="http://schemas.microsoft.com/wix/2006/wi">
    <Product Id="{12345678-1234-1234-1234-1234567890AB}" Name="MyApp" Version="2.0.0" Manufacturer="Acme">
        <Package Description="Installer" />
        <Media Id="1" Cabinet="Data1.cab" />
        <Directory Id="TARGETDIR" Name="SourceDir">
            <Directory Id="ProgramFilesFolder" Name="PFiles">
                <Component Id="MainComp" Guid="{11111111-1111-1111-1111-111111111111}">
                    <File Id="AppExe" Source="app.exe" />
                </Component>
            </Directory>
        </Directory>
        <Feature Id="MainFeature" Title="Complete" Level="1">
            <ComponentRef Id="MainComp" />
        </Feature>
        <Property Id="CUSTOM_PROP" Value="Hello" />
    </Product>
</Wix>
"#;

        let parser = XmlParser::new();
        let root = parser.parse(xml)?;

        let compiler = Compiler::new();
        let obj = compiler.compile(&root)?;

        assert_eq!(obj.sections.len(), 1);
        let sec = &obj.sections[0];
        assert_eq!(sec.section_type, SectionType::Product);

        // Verify symbols
        assert!(sec.symbols.contains(&Symbol::new(
            "Product",
            "{12345678-1234-1234-1234-1234567890AB}"
        )));
        assert!(sec.symbols.contains(&Symbol::new("Directory", "TARGETDIR")));
        assert!(sec
            .symbols
            .contains(&Symbol::new("Directory", "ProgramFilesFolder")));
        assert!(sec.symbols.contains(&Symbol::new("Component", "MainComp")));
        assert!(sec.symbols.contains(&Symbol::new("File", "AppExe")));
        assert!(sec.symbols.contains(&Symbol::new("Feature", "MainFeature")));
        assert!(sec.symbols.contains(&Symbol::new("Media", "1")));
        assert!(sec
            .symbols
            .contains(&Symbol::new("Property", "CUSTOM_PROP")));

        // Verify references
        assert!(sec
            .references
            .contains(&Reference::new("Directory", "TARGETDIR")));
        assert!(sec
            .references
            .contains(&Reference::new("Component", "MainComp")));

        // Verify tables
        assert!(sec.tables.iter().any(|t| t.name == "Directory"));
        assert!(sec.tables.iter().any(|t| t.name == "Component"));
        assert!(sec.tables.iter().any(|t| t.name == "File"));
        assert!(sec.tables.iter().any(|t| t.name == "Feature"));
        assert!(sec.tables.iter().any(|t| t.name == "FeatureComponents"));
        assert!(sec.tables.iter().any(|t| t.name == "Property"));
        assert!(sec.tables.iter().any(|t| t.name == "Media"));

        Ok(())
    }

    #[test]
    fn test_compiler_sections_variety() -> Result<()> {
        let xml = r#"
<Wix xmlns="http://schemas.microsoft.com/wix/2006/wi">
    <Module Id="Mod1" Version="1.0" />
    <Fragment Id="Frag1" />
    <PatchCreation Id="PC1" />
    <Patch Id="P1" />
</Wix>
"#;
        let parser = XmlParser::new();
        let root = parser.parse(xml)?;
        let compiler = Compiler::new();
        let obj = compiler.compile(&root)?;

        assert_eq!(obj.sections.len(), 4);
        assert_eq!(obj.sections[0].section_type, SectionType::Module);
        assert_eq!(obj.sections[1].section_type, SectionType::Fragment);
        assert_eq!(obj.sections[2].section_type, SectionType::PatchCreation);
        assert_eq!(obj.sections[3].section_type, SectionType::Patch);
        Ok(())
    }

    #[test]
    fn test_compiler_errors() -> Result<()> {
        let compiler = Compiler::new();
        let parser = XmlParser::new();

        // Missing Directory Id
        let bad_dir = parser.parse("<Wix><Product Id=\"{11111111-1111-1111-1111-111111111111}\"><Directory /></Product></Wix>")?;
        assert!(compiler.compile(&bad_dir).is_err());

        // Missing Component Id
        let bad_comp = parser.parse("<Wix><Product Id=\"{11111111-1111-1111-1111-111111111111}\"><Component /></Product></Wix>")?;
        assert!(compiler.compile(&bad_comp).is_err());

        // Missing File Id
        let bad_file = parser.parse(
            "<Wix><Product Id=\"{11111111-1111-1111-1111-111111111111}\"><File /></Product></Wix>",
        )?;
        assert!(compiler.compile(&bad_file).is_err());

        // Missing Feature Id
        let bad_feat = parser.parse("<Wix><Product Id=\"{11111111-1111-1111-1111-111111111111}\"><Feature /></Product></Wix>")?;
        assert!(compiler.compile(&bad_feat).is_err());

        // Missing Property Id
        let bad_prop = parser.parse("<Wix><Product Id=\"{11111111-1111-1111-1111-111111111111}\"><Property /></Product></Wix>")?;
        assert!(compiler.compile(&bad_prop).is_err());

        Ok(())
    }

    #[test]
    fn test_compiler_extended_elements() -> Result<()> {
        let xml = r#"
<Wix xmlns="http://schemas.microsoft.com/wix/2006/wi" xmlns:posix="http://schemas.msi-rs.org/wix/posix/v1">
    <Product Id="{12345678-1234-1234-1234-1234567890AB}" Name="ExtendedApp" Version="1.0.0" Manufacturer="Acme">
        <Package Description="Installer" />
        <MajorUpgrade />
        <Upgrade Id="{87654321-4321-4321-4321-BA0987654321}">
            <UpgradeVersion Minimum="1.0.0" Maximum="2.0.0" Property="OLDVERSIONFOUND" />
        </Upgrade>
        <Directory Id="TARGETDIR" Name="SourceDir">
            <Component Id="ExtComp" Guid="{22222222-2222-2222-2222-222222222222}">
                <File Id="AppBinary" Source="bin/app" />
                <RegistryKey Root="HKLM" Key="Software\Acme\App">
                    <RegistryValue Id="RegVal1" Name="Installed" Value="1" Type="integer" />
                </RegistryKey>
                <RegistryValue Id="StandaloneVal" Root="HKCU" Key="Software\Acme" Name="UserPref" Value="Dark" />
                <Shortcut Id="AppShortcut" Name="Extended App" Target="[#AppBinary]" Directory="ProgramMenuFolder" Description="Launch app" Arguments="--run" />
                <ServiceInstall Id="AppSvc" Name="AppService" DisplayName="Extended Service" Type="ownProcess" Start="auto" ErrorControl="normal" />
                <ServiceControl Id="AppSvcCtrl" Name="AppService" Start="yes" Stop="yes" Remove="yes" />
                <posix:File File="AppBinary" Mode="0o755" Owner="root" Group="wheel" />
                <posix:Symlink Id="AppSymlink" Target="/usr/local/bin/app" LinkDirectory="TARGETDIR" LinkName="app" />
                <posix:Daemon Service="AppSvc" SupervisorType="Systemd" RestartPolicy="always" RunAsUser="appuser" />
                <posix:Acl Id="AppAcl" File="AppBinary" PrincipalType="user" PrincipalName="appuser" Permissions="7" />
                <posix:Desktop Shortcut="AppShortcut" Categories="Utility;Development;" Terminal="no" />
            </Component>
        </Directory>
        <CustomAction Id="CA_Dll" BinaryKey="CustomDll" DllEntry="RunAction" />
        <CustomAction Id="CA_Exe" FileKey="AppBinary" ExeCommand="--init" />
        <CustomAction Id="CA_Prop" Property="INSTALLDIR" Value="/opt/acme" />
        <UI>
            <TextStyle Id="TitleStyle" FaceName="Arial" Size="12" />
            <Dialog Id="WelcomeDlg" Width="370" Height="270" Title="Welcome Wizard">
                <Control Id="NextBtn" Type="PushButton" X="236" Y="243" Width="56" Height="17" Text="Next" />
            </Dialog>
        </UI>
    </Product>
</Wix>
"#;

        let parser = XmlParser::new();
        let root = parser.parse(xml)?;
        let compiler = Compiler::new();
        let obj = compiler.compile(&root)?;

        assert_eq!(obj.sections.len(), 1);
        let sec = &obj.sections[0];

        // Verify symbols generated
        assert!(sec.symbols.contains(&Symbol::new("Registry", "RegVal1")));
        assert!(sec
            .symbols
            .contains(&Symbol::new("Registry", "StandaloneVal")));
        assert!(sec
            .symbols
            .contains(&Symbol::new("Shortcut", "AppShortcut")));
        assert!(sec
            .symbols
            .contains(&Symbol::new("ServiceInstall", "AppSvc")));
        assert!(sec
            .symbols
            .contains(&Symbol::new("ServiceControl", "AppSvcCtrl")));
        assert!(sec.symbols.contains(&Symbol::new("CustomAction", "CA_Dll")));
        assert!(sec.symbols.contains(&Symbol::new("CustomAction", "CA_Exe")));
        assert!(sec
            .symbols
            .contains(&Symbol::new("CustomAction", "CA_Prop")));
        assert!(sec.symbols.contains(&Symbol::new("Dialog", "WelcomeDlg")));
        assert!(sec
            .symbols
            .contains(&Symbol::new("Control", "WelcomeDlg.NextBtn")));
        assert!(sec
            .symbols
            .contains(&Symbol::new("TextStyle", "TitleStyle")));
        assert!(sec.symbols.contains(&Symbol::new(
            "Upgrade",
            "{87654321-4321-4321-4321-BA0987654321}"
        )));
        assert!(sec
            .symbols
            .contains(&Symbol::new("PosixSymlink", "AppSymlink")));
        assert!(sec.symbols.contains(&Symbol::new("PosixDaemon", "AppSvc")));
        assert!(sec.symbols.contains(&Symbol::new("PosixAcl", "AppAcl")));
        assert!(sec
            .symbols
            .contains(&Symbol::new("PosixDesktop", "AppShortcut")));

        // Verify tables generated
        assert!(sec.tables.iter().any(|t| t.name == "Registry"));
        assert!(sec.tables.iter().any(|t| t.name == "Shortcut"));
        assert!(sec.tables.iter().any(|t| t.name == "ServiceInstall"));
        assert!(sec.tables.iter().any(|t| t.name == "ServiceControl"));
        assert!(sec.tables.iter().any(|t| t.name == "CustomAction"));
        assert!(sec.tables.iter().any(|t| t.name == "Dialog"));
        assert!(sec.tables.iter().any(|t| t.name == "Control"));
        assert!(sec.tables.iter().any(|t| t.name == "TextStyle"));
        assert!(sec.tables.iter().any(|t| t.name == "Upgrade"));
        assert!(sec.tables.iter().any(|t| t.name == "PosixFile"));
        assert!(sec.tables.iter().any(|t| t.name == "PosixSymlink"));
        assert!(sec.tables.iter().any(|t| t.name == "PosixDaemon"));
        assert!(sec.tables.iter().any(|t| t.name == "PosixAcl"));
        assert!(sec.tables.iter().any(|t| t.name == "PosixDesktop"));

        Ok(())
    }

    #[test]
    #[allow(clippy::too_many_lines)]
    fn test_compiler_wix_section11_features() -> Result<()> {
        let xml = r#"
<Wix xmlns="http://schemas.microsoft.com/wix/2006/wi">
    <Product Id="{12345678-1234-1234-1234-1234567890AB}" Name="Section11App" Version="1.2.3" Manufacturer="Acme" UpgradeCode="{87654321-4321-4321-4321-BA0987654321}" Language="1033" Codepage="1252" InstallerVersion="500" Compressed="yes" Description="Product Desc" Comments="Product Comm" Keywords="MSI;Test;" Platform="x64">
        <Package Description="Pkg Desc" Comments="Pkg Comm" Keywords="Pkg Kw" Languages="1033" Manufacturer="Pkg Mfg" Platform="x64" SummaryCodepage="1252" InstallerVersion="500" Compressed="yes" />
        <DirectoryRef Id="TARGETDIR">
            <Directory Id="ProgramFilesFolder" Name="PFiles">
                <Directory Id="INSTALLDIR" Name="MyApp" />
            </Directory>
        </DirectoryRef>
        <ComponentGroup Id="ProductComponents" Directory="INSTALLDIR">
            <Component Id="CompWithAutoGuid" Guid="*" Win64="yes" Permanent="yes" NeverOverwrite="yes" Shared="yes" Transitive="yes" UninstallWhenSuperseded="yes" MultiInstance="yes" Location="local">
                <Condition>VersionNT &gt;= 600</Condition>
                <CreateFolder Directory="INSTALLDIR" />
                <RemoveFolder Id="RemInstallDir" Directory="INSTALLDIR" On="both" />
                <Environment Id="EnvPath" Name="PATH" Value="[INSTALLDIR]" Action="set" System="yes" />
                <File Id="AppExe" Name="app.exe" Vital="yes" ReadOnly="yes" Hidden="yes" System="yes" Checksum="yes" Compressed="yes" DefaultSize="4096" DefaultVersion="1.2.3.4" DefaultLanguage="1033" KeyPath="yes" />
            </Component>
        </ComponentGroup>
        <Feature Id="MainFeat" Title="Main Feature" Level="1">
            <ComponentGroupRef Id="ProductComponents" />
        </Feature>
        <FeatureGroup Id="AllFeatures">
            <FeatureRef Id="MainFeat" />
        </FeatureGroup>
        <FeatureGroupRef Id="AllFeatures" />
        <PackageGroup Id="PkgGroup" />
        <PackageGroupRef Id="PkgGroup" />
        <SetDirectory Id="INSTALLDIR" Value="[ProgramFilesFolder]MyApp" Sequence="1">
            <Condition>NOT Installed</Condition>
        </SetDirectory>
    </Product>
</Wix>
"#;

        let parser = XmlParser::new();
        let root = parser.parse(xml)?;
        let compiler = Compiler::new();
        let obj = compiler.compile(&root)?;

        assert_eq!(obj.sections.len(), 1);
        let sec = &obj.sections[0];

        // Symbols
        assert!(sec
            .symbols
            .contains(&Symbol::new("ComponentGroup", "ProductComponents")));
        assert!(sec
            .symbols
            .contains(&Symbol::new("FeatureGroup", "AllFeatures")));
        assert!(sec
            .symbols
            .contains(&Symbol::new("PackageGroup", "PkgGroup")));
        assert!(sec
            .symbols
            .contains(&Symbol::new("CustomAction", "SetDirectory_INSTALLDIR")));

        // References
        assert!(sec
            .references
            .contains(&Reference::new("Directory", "TARGETDIR")));
        assert!(sec
            .references
            .contains(&Reference::new("ComponentGroup", "ProductComponents")));
        assert!(sec
            .references
            .contains(&Reference::new("FeatureGroup", "AllFeatures")));
        assert!(sec
            .references
            .contains(&Reference::new("PackageGroup", "PkgGroup")));
        assert!(sec
            .references
            .contains(&Reference::new("Feature", "MainFeat")));

        // Tables
        let mut found_create_folder = false;
        let mut found_remove_file = false;
        let mut found_env = false;
        let mut found_comp = false;
        let mut found_file = false;

        for t in &sec.tables {
            if t.name == "CreateFolder" {
                assert_eq!(t.records.len(), 1);
                found_create_folder = true;
            } else if t.name == "RemoveFile" {
                assert_eq!(t.records.len(), 1);
                found_remove_file = true;
            } else if t.name == "Environment" {
                assert_eq!(t.records.len(), 1);
                found_env = true;
            } else if t.name == "Component" {
                let parsed_comp = ComponentRow::from_record(&t.records[0])?;
                assert!(parsed_comp.component_id.is_some());
                assert_ne!(parsed_comp.attributes, 0);
                assert_eq!(parsed_comp.condition, Some("VersionNT >= 600".to_string()));
                found_comp = true;
            } else if t.name == "File" {
                let parsed_file = FileRow::from_record(&t.records[0])?;
                assert_eq!(parsed_file.file_size, 4096);
                assert_eq!(parsed_file.version, Some("1.2.3.4".to_string()));
                assert_eq!(parsed_file.language, Some("1033".to_string()));
                assert_ne!(parsed_file.attributes, Some(0));
                found_file = true;
            }
        }

        assert!(found_create_folder);
        assert!(found_remove_file);
        assert!(found_env);
        assert!(found_comp);
        assert!(found_file);

        Ok(())
    }

    /// Tests compiling module attributes, signatures, dependencies, exclusions, and configurations.
    #[test]
    fn test_compiler_module_attributes() -> Result<()> {
        let parser = XmlParser::new();
        let compiler = Compiler::new();

        let mod_xml = r#"
<Wix xmlns="http://schemas.microsoft.com/wix/2006/wi">
    <Module Id="TestMod" Version="2.3.4" Language="1033" Manufacturer="ModMfg" Guid="{11111111-2222-3333-4444-555555555555}" Codepage="65001">
        <Component Id="ModComp1" Guid="{22222222-3333-4444-5555-666666666666}" />
        <Dependency RequiredId="OtherMod" RequiredLanguage="1033" RequiredVersion="1.0.0" />
        <Dependency Id="OtherMod2" Language="1033" Version="2.0.0" ModuleLanguage="1033" />
        <Exclusion ExcludedId="BadMod" ExcludedLanguage="1033" ExcludedVersionMin="0.1.0" ExcludedVersionMax="0.9.0" />
        <Exclusion Id="BadMod2" Language="1033" VersionMin="1.0.0" VersionMax="2.0.0" ModuleLanguage="1033" />
        <Configuration Name="ParamA" Format="0" />
        <Substitution Table="Property" Row="ParamA" Column="Value" Value="[=PARAM_A]" />
        <IgnoreModularization Name="StandardSym" />
    </Module>
</Wix>
"#;
        let mod_root = parser.parse(mod_xml)?;
        let mod_obj = compiler.compile(&mod_root)?;
        assert_eq!(mod_obj.sections.len(), 1);
        let mod_sec = &mod_obj.sections[0];
        let table_names: Vec<&str> = mod_sec.tables.iter().map(|t| t.name.as_str()).collect();
        assert!(table_names.contains(&"Property"));
        assert!(table_names.contains(&"ModuleSignature"));
        assert!(table_names.contains(&"ModuleComponents"));
        assert!(table_names.contains(&"ModuleDependency"));
        assert!(table_names.contains(&"ModuleExclusion"));
        assert!(table_names.contains(&"ModuleConfiguration"));
        assert!(table_names.contains(&"ModuleSubstitution"));
        assert!(table_names.contains(&"ModuleIgnoreModularization"));

        Ok(())
    }

    #[test]
    #[allow(clippy::too_many_lines)]
    fn test_compiler_wix_section11_extended_tags() -> Result<()> {
        let parser = XmlParser::new();
        let compiler = Compiler::new();

        let xml = r#"
<Wix xmlns="http://schemas.microsoft.com/wix/2006/wi" xmlns:util="http://schemas.microsoft.com/wix/UtilExtension">
    <Product Id="{11111111-1111-1111-1111-111111111111}" Name="Sec11App" Version="1.0.0" Manufacturer="Acme">
        <Package Description="Test" />
        <Symbol Name="PublishedSym" Namespace="Export" />
        <MediaTemplate CabinetTemplate="cab{0}.cab" DiskPrompt="Disk 1" />
        <PropertyRef Id="EXISTING_PROP" />
        <Property Id="SECURE_PROP" Value="Secret" Secure="yes" />
        <SetProperty Id="CONFIG_DIR" Value="C:\App" Before="CostInitialize" Sequence="800" Condition="NOT Installed" />
        <Launch Message="OS unsupported" Condition="VersionNT &gt;= 600" />
        <Directory Id="TARGETDIR" Name="SourceDir" ShortName="SRCDIR">
            <Component Id="C1" Guid="*">
                <File Id="F1" Source="main.exe" ShortName="MAIN~1.EXE" Vital="yes" ReadOnly="yes" Hidden="yes" System="yes" Checksum="yes" Compressed="yes">
                    <Font Title="MainFont" />
                </File>
                <CopyFile Id="Copy1" FileId="F1" DestinationDirectory="TARGETDIR" DestinationName="main_backup.exe" />
                <CopyFile Id="Copy2" SourceName="temp.dat" DestinationDirectory="TARGETDIR" DestinationName="temp_dest.dat" Delete="yes" />
                <MoveFile Id="Move1" SourceName="old.log" DestName="new.log" />
                <RemoveFile Id="Rem1" Name="*.tmp" On="both" />
                <SymbolicLink Id="Sym1" Target="/usr/bin/app" Name="app_link" />
                <RemoveRegistryKey Id="RemKey1" Root="HKLM" Key="Software\OldAcme" />
                <RemoveRegistryValue Id="RemVal1" Root="HKCU" Key="Software\Acme" Name="OldVal" />
                <IniFile Id="Ini1" Name="test.ini" Section="General" Key="Port" Value="8080" Action="addLine" Directory="TARGETDIR" />
                <RemoveIniFile Id="RemIni1" Name="test.ini" Section="General" Key="OldPort" Value="OldVal" Directory="TARGETDIR" />
                <util:User Id="AppUser" Name="appuser" />
            </Component>
        </Directory>
        <Feature Id="MainFeat" Title="Main Feature" Display="expand" AllowAdvertise="no" InstallDefault="local" TypicalDefault="advertise" Absent="disallow" ConfigurableDirectory="TARGETDIR">
            <ComponentRef Id="C1" />
            <Condition Level="0">NOT ALLOWED</Condition>
            <MergeRef Id="HelperMod" />
        </Feature>
        <Icon Id="AppIco" SourceFile="icon.ico" />
        <ProgId Id="Acme.Doc" Description="Acme Document" Icon="AppIco" IconIndex="0" />
        <Extension Id="acme" ProgId="Acme.Doc" MIME="application/x-acme">
            <Verb Id="open" Command="Open" Argument="&quot;%1&quot;" Sequence="1" />
        </Extension>
        <MIME ContentType="application/x-acme" DefaultExtension="acme" CLSID="{22222222-2222-2222-2222-222222222222}" />
        <Class Id="{22222222-2222-2222-2222-222222222222}" Context="InprocServer32" Description="Acme Class" ProgId="Acme.Doc" AppId="{33333333-3333-3333-3333-333333333333}" />
        <TypeLib Id="{44444444-4444-4444-4444-444444444444}" Language="1033" Description="Acme TypeLib" Directory="TARGETDIR" />
        <AppId Id="{33333333-3333-3333-3333-333333333333}" Description="Acme AppId" />
        <AppSearch Property="NET_FRAMEWORK" Id="AppSearch1" />
        <RegistrySearch Id="Reg1" Root="HKLM" Key="Software\DotNet" Name="Version" Type="raw" />
        <DirectorySearch Id="Dir1" Path="C:\Windows" Depth="2" />
        <FileSearch Id="File1" Name="kernel32.dll" MinVersion="6.0.0.0" />
        <IniFileSearch Id="IniSearch1" Name="boot.ini" Section="boot loader" Key="timeout" />
        <ComponentSearch Id="CompSearch1" Guid="{55555555-5555-5555-5555-555555555555}" />
        <InstallExecuteSequence>
            <Custom Action="Set_CONFIG_DIR" Before="CostInitialize" Sequence="800">NOT Installed</Custom>
        </InstallExecuteSequence>
        <CustomActionRef Id="CA_Ext" />
        <PatchCreation Id="PC1" />
        <UIRef Id="WixUI_InstallDir" />
        <Binary Id="BinHelper" SourceFile="helper.dll" />
        <Billboard Id="BB1" Feature="MainFeat" Action="InstallFiles" Ordering="1" />
        <ProgressText Action="InstallFiles" Template="Copying: [1]">Copying files</ProgressText>
        <Error Id="1001" Message="Fatal error occurred" />
        <UI>
            <Dialog Id="CustomDlg" Width="370" Height="270" Title="Dialog Title">
                <Control Id="Btn" Type="PushButton" X="10" Y="10" Width="50" Height="17" Text="Click">
                    <Publish Event="NewDialog" Value="ExitDlg" Order="1" />
                    <Condition Action="disable">DISABLE_BTN = 1</Condition>
                    <Subscribe Event="SetProgress" Attribute="Progress" />
                </Control>
            </Dialog>
        </UI>
    </Product>
</Wix>
"#;
        let root = parser.parse(xml)?;
        let obj = compiler.compile(&root)?;
        assert_eq!(obj.sections.len(), 1);
        let sec = &obj.sections[0];

        // Check symbols and references
        assert!(sec.symbols.contains(&Symbol::new("Export", "PublishedSym")));
        assert!(sec
            .references
            .contains(&Reference::new("Property", "EXISTING_PROP")));
        assert!(sec
            .references
            .contains(&Reference::new("Module", "HelperMod")));
        assert!(sec
            .references
            .contains(&Reference::new("CustomAction", "CA_Ext")));

        // Check generated tables
        let table_names: Vec<&str> = sec.tables.iter().map(|t| t.name.as_str()).collect();
        assert!(table_names.contains(&"Font"));
        assert!(table_names.contains(&"DuplicateFile"));
        assert!(table_names.contains(&"MoveFile"));
        assert!(table_names.contains(&"RemoveFile"));
        assert!(table_names.contains(&"PosixSymlink"));
        assert!(table_names.contains(&"RemoveRegistry"));
        assert!(table_names.contains(&"IniFile"));
        assert!(table_names.contains(&"RemoveIniFile"));
        assert!(table_names.contains(&"Icon"));
        assert!(table_names.contains(&"ProgId"));
        assert!(table_names.contains(&"Extension"));
        assert!(table_names.contains(&"Verb"));
        assert!(table_names.contains(&"MIME"));
        assert!(table_names.contains(&"Class"));
        assert!(table_names.contains(&"TypeLib"));
        assert!(table_names.contains(&"AppId"));
        assert!(table_names.contains(&"AppSearch"));
        assert!(table_names.contains(&"RegLocator"));
        assert!(table_names.contains(&"DrLocator"));
        assert!(table_names.contains(&"Signature"));
        assert!(table_names.contains(&"IniLocator"));
        assert!(table_names.contains(&"CompLocator"));
        assert!(table_names.contains(&"LaunchCondition"));
        assert!(table_names.contains(&"PatchPackage"));
        assert!(table_names.contains(&"Binary"));
        assert!(table_names.contains(&"Billboard"));
        assert!(table_names.contains(&"ActionText"));
        assert!(table_names.contains(&"Error"));
        assert!(table_names.contains(&"ControlEvent"));
        assert!(table_names.contains(&"ControlCondition"));
        assert!(table_names.contains(&"EventMapping"));
        assert!(table_names.contains(&"_util:User"));

        // Verify Module elements
        let mod_xml = r#"
<Wix xmlns="http://schemas.microsoft.com/wix/2006/wi">
    <Module Id="ModConfig" Version="1.0" Language="1033">
        <Configuration Name="ParamA" Format="0" Type="string" DefaultValue="default" Description="Config param" DisplayName="Param A" Attributes="1" />
        <Substitution Table="Property" Row="PROP" Column="Value" Value="[=ParamA]" />
        <IgnoreModularization Name="ActionA" Type="1" />
    </Module>
</Wix>
"#;
        let mod_root = parser.parse(mod_xml)?;
        let mod_obj = compiler.compile(&mod_root)?;
        let mod_sec = &mod_obj.sections[0];
        let mod_table_names: Vec<&str> = mod_sec.tables.iter().map(|t| t.name.as_str()).collect();
        assert!(mod_table_names.contains(&"ModuleConfiguration"));
        assert!(mod_table_names.contains(&"ModuleSubstitution"));
        assert!(mod_table_names.contains(&"ModuleIgnoreModularization"));

        Ok(())
    }

    /// Tests compiling top-level Wix root variations and section attributes.
    ///
    /// # Errors
    ///
    /// Returns [`Error`] if XML parsing or compilation fails.
    #[test]
    fn test_compiler_top_level_and_section_variations() -> Result<()> {
        let parser = XmlParser::new();
        let compiler = Compiler::new();

        // 1. Root with unknown tag
        let xml_unknown = r#"
<Wix xmlns="http://schemas.microsoft.com/wix/2006/wi">
    <UnknownRootChild Tag="Ignored" />
</Wix>
"#;
        let root_unknown = parser.parse(xml_unknown)?;
        let obj_unknown = compiler.compile(&root_unknown)?;
        assert_eq!(obj_unknown.sections.len(), 0);

        // 2. Minimal Product without attributes
        let xml_min_prod = r#"
<Wix xmlns="http://schemas.microsoft.com/wix/2006/wi">
    <Product />
</Wix>
"#;
        let root_min_prod = parser.parse(xml_min_prod)?;
        let obj_min_prod = compiler.compile(&root_min_prod)?;
        assert_eq!(obj_min_prod.sections.len(), 1);
        assert_eq!(obj_min_prod.sections[0].id, None);

        // 3. Product with all attributes and alternative attribute names
        let xml_full_prod = r#"
<Wix xmlns="http://schemas.microsoft.com/wix/2006/wi">
    <Product Id="{11111111-1111-1111-1111-111111111111}"
             ProductCode="{22222222-2222-2222-2222-222222222222}"
             Name="FullApp"
             Version="1.2.3"
             Manufacturer="Acme Inc"
             UpgradeCode="{33333333-3333-3333-3333-333333333333}"
             Language="1033"
             Codepage="1252"
             InstallerVersion="500"
             Compressed="yes"
             Description="Full Application"
             Comments="Package Comments"
             Keywords="App, Installer"
             Platform="x64">
        <Package InstallerVersion="500"
                 Compressed="yes"
                 Description="Pkg Description"
                 Comments="Pkg Comments"
                 Keywords="Pkg, Tags"
                 Languages="1033"
                 Manufacturer="Pkg Mfg"
                 Platform="x64"
                 SummaryCodepage="1252" />
    </Product>
</Wix>
"#;
        let root_full_prod = parser.parse(xml_full_prod)?;
        let obj_full_prod = compiler.compile(&root_full_prod)?;
        assert_eq!(obj_full_prod.sections.len(), 1);

        // 4. Product with Languages and SummaryCodepage alternatives
        let xml_alt_prod = r#"
<Wix xmlns="http://schemas.microsoft.com/wix/2006/wi">
    <Product Id="{44444444-4444-4444-4444-444444444444}"
             Languages="1031"
             SummaryCodepage="65001">
        <Package Language="1031" Codepage="65001" />
    </Product>
</Wix>
"#;
        let root_alt_prod = parser.parse(xml_alt_prod)?;
        let obj_alt_prod = compiler.compile(&root_alt_prod)?;
        assert_eq!(obj_alt_prod.sections.len(), 1);

        // 5. Minimal Module (all attributes None)
        let xml_min_mod = r#"
<Wix xmlns="http://schemas.microsoft.com/wix/2006/wi">
    <Module />
</Wix>
"#;
        let root_min_mod = parser.parse(xml_min_mod)?;
        let obj_min_mod = compiler.compile(&root_min_mod)?;
        assert_eq!(obj_min_mod.sections.len(), 1);

        // 6. Module with Guid and SummaryCodepage
        let xml_alt_mod = r#"
<Wix xmlns="http://schemas.microsoft.com/wix/2006/wi">
    <Module Guid="{55555555-5555-5555-5555-555555555555}" SummaryCodepage="1252" />
</Wix>
"#;
        let root_alt_mod = parser.parse(xml_alt_mod)?;
        let obj_alt_mod = compiler.compile(&root_alt_mod)?;
        assert_eq!(obj_alt_mod.sections.len(), 1);

        Ok(())
    }

    /// Tests compiling features, components, and files with various attribute branches.
    ///
    /// # Errors
    ///
    /// Returns [`Error`] if XML parsing or compilation fails.
    #[test]
    #[allow(clippy::too_many_lines)]
    fn test_compiler_features_and_components_branches() -> Result<()> {
        let parser = XmlParser::new();
        let compiler = Compiler::new();

        let xml = format!(
            r#"
<Wix xmlns="http://schemas.microsoft.com/wix/2006/wi" xmlns:posix="http://schemas.msi-rs.org/wix/posix/v1">
    <Product Id="{{11111111-1111-1111-1111-111111111111}}" Name="App" Version="1.0.0" Manufacturer="Acme">
        <Directory Id="TARGETDIR" Name="SourceDir">
            <Component Id="CompSource" Location="source" />
            <Component Id="CompEither" Location="either" />
            <Component Id="CompOtherLoc" Location="unknown" />
            <Component Id="CompEmptyGuid" Guid="" />
            <Component Id="CompQGuid" Guid="?" />
            <Component Id="CompFiles" Guid="{{22222222-2222-2222-2222-222222222222}}">
                <File Id="FileCargo" Source="{manifest_dir}/Cargo.toml" Vital="no" Compressed="no" />
                <File Id="FileWithFontChild" Name="custom.ttf">
                    <Font Title="My Custom Font" />
                    <UnknownChildInFile />
                </File>
                <File Id="FileFont1" Name="font1.ttf" TrueType="yes" FontTitle="Arial Font" />
                <File Id="FileFont2" Name="font2.ttf" TrueType="yes" />
                <File Id="FileFontNoName" TrueType="yes" />
                <File Id="FileLong" Name="verylongfilename withspace.longextension" />
                <File Id="FileNoExt" Name="verylongfilenamewithoutanextension" />
                <File Id="FileShortSpace" Name="a b.c" />
                <File Id="FilePlain" Name="test.txt" />
                <CopyFile Id="CopyDestProp" FileId="FilePlain" DestinationProperty="DEST_DIR" />
                <CopyFile Id="CopySrcProp" SourceProperty="SRC_DIR" DestinationProperty="DEST_DIR" Delete="no" />
                <RemoveFile Id="RemInst" Name="*.bak" On="install" />
                <RemoveFile Id="RemUninst" Name="*.old" On="uninstall" />
                <RemoveFile Id="RemDef" Name="*.tmp" />
                <SymbolicLink Id="SymTargPath" TargetPath="/usr/local/bin/link" LinkDirectory="TARGETDIR" LinkName="symlink" />
                <IniFile Id="Ini1" Name="app.ini" Section="S" Key="K1" Value="V" Action="createLine" />
                <IniFile Id="Ini2" Name="app.ini" Section="S" Key="K2" Value="V" Action="replaceLine" />
                <IniFile Id="Ini3" Name="app.ini" Section="S" Key="K3" Value="V" Action="addTag" />
                <IniFile Id="Ini4" Name="app.ini" Section="S" Key="K4" Value="V" Action="removeTag" />
                <IniFile Id="Ini5" Name="app.ini" Section="S" Key="K5" Value="V" Action="other" />
                <Environment Id="EnvCreate" Name="VAR1" Value="VAL1" System="yes" Action="create" />
                <Environment Id="EnvRemove" Name="VAR2" Value="VAL2" Action="remove" />
                <Environment Id="EnvSet" Name="VAR3" Value="VAL3" Action="set" />
                <ServiceInstall Name="SvcShared" Type="shareProcess" Start="demand" ErrorControl="ignore" />
                <ServiceInstall Id="SvcManual" Start="manual" ErrorControl="normal" />
                <ServiceInstall Id="SvcDisabled" Start="disabled" ErrorControl="critical" />
                <ServiceControl Id="SvcCtrlNo" Name="SvcCtrlNo" Start="no" Stop="no" Remove="no" />
                <posix:Daemon Service="SvcLaunchd" SupervisorType="launchd" RestartPolicy="on-failure" />
                <posix:Daemon Service="SvcRcd" SupervisorType="rcd" RestartPolicy="no" />
                <posix:Daemon Service="SvcSmf" SupervisorType="smf" />
                <posix:Desktop Shortcut="S_Yes" Terminal="yes" />
                <posix:Desktop Shortcut="S_True" Terminal="true" />
                <posix:UnknownTag />
            </Component>
        </Directory>
        <ComponentGroup Id="CG_NoIds">
            <ComponentRef />
            <Component Id="CompInCG_Valid" />
            <ComponentGroupRef Id="CG_Nested_Valid" />
        </ComponentGroup>
        <ComponentGroup Id="CG_Nested_Valid" />
        <Feature Id="ParentFeat">
            <ComponentRef />
            <MergeRef />
            <Merge Id="ModMerge" />
            <Condition Level="1" />
            <ComponentGroupRef Id="CG1" />
            <Feature Id="ChildFeat" Display="collapse" InstallDefault="source" />
        </Feature>
        <Feature Id="FeatHidden" Display="hidden" InstallDefault="followParent" />
        <Feature Id="FeatNumericDisplay" Display="3" InstallDefault="local" />
        <Feature Id="FeatAbsentDisallow" Absent="disallow" TypicalDefault="advertise" />
        <Feature Id="FeatWithCG"><ComponentGroupRef Id="CG1" /></Feature>
        <Feature Id="FeatDefault" />
    </Product>
</Wix>
"#,
            manifest_dir = env!("CARGO_MANIFEST_DIR")
        );
        let root = parser.parse(&xml)?;
        let obj = compiler.compile(&root)?;
        assert_eq!(obj.sections.len(), 1);

        Ok(())
    }

    /// Tests custom action execution modes, return modes, and types.
    ///
    /// # Errors
    ///
    /// Returns [`Error`] if XML parsing or compilation fails.
    #[test]
    fn test_compiler_custom_actions_all_branches() -> Result<()> {
        let parser = XmlParser::new();
        let compiler = Compiler::new();

        let xml = r#"
<Wix xmlns="http://schemas.microsoft.com/wix/2006/wi">
    <Product Id="{11111111-1111-1111-1111-111111111111}" Name="App" Version="1.0.0" Manufacturer="Acme">
        <CustomAction Id="CA1" BinaryKey="Bin" ExeCommand="--init" Execute="deferred" Return="ignore" Impersonate="no" />
        <CustomAction Id="CA2" BinaryKey="Bin" Execute="rollback" Return="asyncWait" />
        <CustomAction Id="CA3" Directory="TARGETDIR" Value="C:\Dir" Execute="commit" Return="asyncNoWait" />
        <CustomAction Id="CA4" Directory="TARGETDIR" ExeCommand="echo 1" Execute="firstSequence" />
        <CustomAction Id="CA5" Script="vbscript" Execute="oncePerProcess">x = 1</CustomAction>
        <CustomAction Id="CA6" Script="jscript">var y = 2;</CustomAction>
        <CustomAction Id="CA7" Error="Fatal err" />
        <CustomAction Id="CA8" />
    </Product>
</Wix>
"#;
        let root = parser.parse(xml)?;
        let obj = compiler.compile(&root)?;
        assert_eq!(obj.sections.len(), 1);
        let sec = &obj.sections[0];
        let mut found_ca = false;
        for t in &sec.tables {
            if t.name == "CustomAction" {
                assert_eq!(t.records.len(), 8);
                found_ca = true;
            }
        }
        assert!(found_ca);

        Ok(())
    }

    /// Tests locator search types nested under components and products.
    ///
    /// # Errors
    ///
    /// Returns [`Error`] if XML parsing or compilation fails.
    #[test]
    #[allow(clippy::too_many_lines)]
    fn test_compiler_searches_nested_and_types() -> Result<()> {
        let parser = XmlParser::new();
        let compiler = Compiler::new();

        let xml = r#"
<Wix xmlns="http://schemas.microsoft.com/wix/2006/wi">
    <Product Id="{11111111-1111-1111-1111-111111111111}" Name="App" Version="1.0.0" Manufacturer="Acme">
        <Directory Id="TARGETDIR" Name="SourceDir">
            <Component Id="CompSearches" Guid="{11111111-2222-3333-4444-555555555555}">
                <RegistrySearch Id="RS_Dir" Root="HKLM" Key="Software\Acme" Type="directory" />
                <RegistrySearch Id="RS_File" Root="HKCU" Key="Software\Acme" Type="file" />
                <DirectorySearch Id="DS_Nested" Path="C:\Windows" />
                <FileSearch Id="FS_Nested" Name="target.exe" />
                <IniFileSearch Id="IS_Nested" FileName="setup.ini" Section="General" Key="Opt" Field="1" Type="0" />
                <ComponentSearch Id="CS_Nested" Guid="{11111111-2222-3333-4444-555555555555}" Type="1" />
            </Component>
        </Directory>
    </Product>
</Wix>
"#;
        let root = parser.parse(xml)?;
        let obj = compiler.compile(&root)?;
        assert_eq!(obj.sections.len(), 1);
        let sec = &obj.sections[0];
        let mut found_app_search = false;
        for t in &sec.tables {
            if t.name == "AppSearch" {
                assert_eq!(t.records.len(), 5);
                found_app_search = true;
            }
        }
        assert!(found_app_search);

        // Test standalone searches in separate products to trigger or_insert_with
        let xml_ini_alone = r#"
<Wix xmlns="http://schemas.microsoft.com/wix/2006/wi">
    <Product Id="{22222222-2222-2222-2222-222222222222}" Name="App2" Version="1.0.0" Manufacturer="Acme">
        <Directory Id="TARGETDIR">
            <Component Id="CompIni">
                <IniFileSearch Id="IS_Alone" FileName="my.ini" Section="S" Key="K" />
            </Component>
        </Directory>
    </Product>
</Wix>
"#;
        let obj_ini = compiler.compile(&parser.parse(xml_ini_alone)?)?;
        assert_eq!(obj_ini.sections.len(), 1);

        let xml_comp_alone = r#"
<Wix xmlns="http://schemas.microsoft.com/wix/2006/wi">
    <Product Id="{33333333-3333-3333-3333-333333333333}" Name="App3" Version="1.0.0" Manufacturer="Acme">
        <Directory Id="TARGETDIR">
            <Component Id="CompSearchOnly">
                <ComponentSearch Id="CS_Alone" Guid="{11111111-2222-3333-4444-555555555555}" />
            </Component>
        </Directory>
    </Product>
</Wix>
"#;
        let obj_comp = compiler.compile(&parser.parse(xml_comp_alone)?)?;
        assert_eq!(obj_comp.sections.len(), 1);

        let xml_fs_alone = r#"
<Wix xmlns="http://schemas.microsoft.com/wix/2006/wi">
    <Product Id="{44444444-4444-4444-4444-444444444444}" Name="App4" Version="1.0.0" Manufacturer="Acme">
        <Directory Id="TARGETDIR">
            <Component Id="CompFileOnly">
                <FileSearch Id="FS_Alone" Name="standalone.exe" />
            </Component>
        </Directory>
    </Product>
</Wix>
"#;
        let obj_fs = compiler.compile(&parser.parse(xml_fs_alone)?)?;
        assert_eq!(obj_fs.sections.len(), 1);

        Ok(())
    }

    /// Tests UI publish, control condition, and subscribe elements.
    ///
    /// # Errors
    ///
    /// Returns [`Error`] if XML parsing or compilation fails.
    #[test]
    fn test_compiler_ui_controls_publish_and_events() -> Result<()> {
        let parser = XmlParser::new();
        let compiler = Compiler::new();

        let xml = r#"
<Wix xmlns="http://schemas.microsoft.com/wix/2006/wi">
    <Product Id="{11111111-1111-1111-1111-111111111111}" Name="App" Version="1.0.0" Manufacturer="Acme">
        <UI>
            <Publish Dialog="Dlg1" Control="BtnNext" Event="NewDialog" Value="Dlg2" Order="1">NOT Installed</Publish>
            <Publish Dialog="Dlg1" Control="BtnCancel" Event="EndDialog" Argument="Exit" />
            <ControlCondition Dialog="Dlg1" Control="BtnNext" Action="disable" Condition="VersionNT &lt; 600" />
            <ControlCondition Dialog="Dlg1" Control="BtnNext" Action="hide">VersionNT &lt; 500</ControlCondition>
            <ControlCondition Dialog="Dlg1" Control="BtnNext" Action="enable" />
            <Subscribe Dialog="Dlg1" Control="Prog" Event="SetProgress" Attribute="Progress" />
            <Subscribe />
            <Dialog Id="Dlg1" Width="300" Height="200" Title="Dlg1">
                <Control Id="BtnNext" Type="PushButton" X="10" Y="10" Width="50" Height="20">
                    <Publish Event="DoAction" Value="CA1">1</Publish>
                    <Condition Action="enable" />
                    <UnknownControlChild />
                </Control>
            </Dialog>
        </UI>
    </Product>
</Wix>
"#;
        let root = parser.parse(xml)?;
        let obj = compiler.compile(&root)?;
        assert_eq!(obj.sections.len(), 1);
        let sec = &obj.sections[0];
        let mut found_control_event = false;
        let mut found_control_condition = false;
        let mut found_event_mapping = false;
        for t in &sec.tables {
            if t.name == "ControlEvent" {
                assert_eq!(t.records.len(), 3);
                found_control_event = true;
            } else if t.name == "ControlCondition" {
                assert_eq!(t.records.len(), 4);
                found_control_condition = true;
            } else if t.name == "EventMapping" {
                assert_eq!(t.records.len(), 2);
                found_event_mapping = true;
            }
        }
        assert!(found_control_event);
        assert!(found_control_condition);
        assert!(found_event_mapping);

        Ok(())
    }

    /// Tests component groups, package groups, sequences, and helper methods.
    ///
    /// # Errors
    ///
    /// Returns [`Error`] if XML parsing or compilation fails.
    #[test]
    #[allow(clippy::too_many_lines)]
    fn test_compiler_groups_sequences_and_helpers() -> Result<()> {
        let parser = XmlParser::new();
        let compiler = Compiler::new();

        let xml = r#"
<Wix xmlns="http://schemas.microsoft.com/wix/2006/wi" xmlns:util="http://schemas.microsoft.com/wix/UtilExtension">
    <Product Id="{11111111-1111-1111-1111-111111111111}" Name="App" Version="1.0.0" Manufacturer="Acme">
        <Symbol Id="SymIdOnly" />
        <Symbol Path="SymPathOnly" />
        <ComponentGroup Id="CG_Outer" Directory="TARGETDIR">
            <ComponentRef Id="CompRef1" />
            <Component Id="CompInCG" Guid="*" />
            <ComponentGroupRef Id="CG_Inner" />
            <UnknownInCG />
        </ComponentGroup>
        <ComponentGroup Id="CG_Inner" />
        <PackageGroup Id="PG1">
            <PackageGroupRef Id="PG2" />
        </PackageGroup>
        <PackageGroup Id="PG2" />
        <FeatureGroup Id="FG1">
            <FeatureGroupRef Id="FG2" />
        </FeatureGroup>
        <FeatureGroup Id="FG2" />
        <FeatureRef Id="FeatRef1" />
        <SetDirectory Id="TARGETDIR" Value="C:\Target" Condition="NOT Installed" />
        <SetDirectory Id="TARGETDIR" Value="C:\Target">
            <Condition>NOT Installed</Condition>
        </SetDirectory>
        <RemoveFolder Id="RemF_Prop" Property="MY_PROP" On="install" />
        <RemoveFolder Id="RemF_Uninst" On="uninstall" />
        <PropertyRef />
        <CustomActionRef />
        <MergeRef />
        <Extension Id="xyz" ContentType="application/x-xyz">
            <UnknownSubElementInExt />
        </Extension>
        <MIME ContentType="application/x-xyz" Extension="xyz" Class="{22222222-2222-2222-2222-222222222222}" />
        <SetProperty Property="SET_PROP" Value="1">
            <Condition>NOT Installed</Condition>
        </SetProperty>
        <Launch />
        <Launch>VersionNT &gt;= 600</Launch>
        <InstallUISequence>
            <Custom Id="ActId" Sequence="100" />
            <Custom Action="ActCond" Sequence="101" Condition="NOT Installed" />
            <Custom Action="ActEmpty" Sequence="102" />
            <Custom Action="ActText" Sequence="103">VersionNT</Custom>
        </InstallUISequence>
        <ProgressText Action="CustomAction1" Template="[1]" />
        <Error Id="2001" />
        <Error Id="2002">Custom error message</Error>
        <util:TopLevel Target="Everywhere" />
        <util:ValueElem Value="Val1" />
        <util:Empty />
        <Upgrade Id="{11111111-2222-3333-4444-555555555555}">
            <UnknownInUpgrade />
        </Upgrade>
        <RegistryKey Root="HKLM" Key="Software\Test">
            <UnknownInRegKey />
        </RegistryKey>
    </Product>
</Wix>
"#;
        let root = parser.parse(xml)?;
        let obj = compiler.compile(&root)?;
        assert_eq!(obj.sections.len(), 1);

        // Standalone fragments to exercise initial table creations
        let xml_fragments = r#"
<Wix xmlns="http://schemas.microsoft.com/wix/2006/wi">
    <Fragment>
        <Package Description="PkgInFrag" />
    </Fragment>
    <Fragment>
        <Property Id="PropInFrag" Value="1" />
    </Fragment>
    <Fragment>
        <Upgrade Id="{00000000-1111-2222-3333-444444444444}">
            <UpgradeVersion Minimum="1.0" Property="UPG" />
        </Upgrade>
    </Fragment>
    <Fragment>
        <Directory Id="TARGETDIR">
            <Component Id="C_MoveAlone">
                <MoveFile Id="MoveAlone" SourceName="a" DestName="b" />
            </Component>
        </Directory>
    </Fragment>
</Wix>
"#;
        let obj_frag = compiler.compile(&parser.parse(xml_fragments)?)?;
        assert_eq!(obj_frag.sections.len(), 4);

        // Verify direct call to compile_posix_element with unknown tag
        let dummy_node = parser.parse("<posix:UnknownTag />")?;
        let mut dummy_sec = IntermediateSection::new(SectionType::Product, None);
        let mut dummy_tbls = std::collections::HashMap::new();
        Compiler::compile_posix_element(&dummy_node, None, &mut dummy_sec, &mut dummy_tbls);

        // Verify parse_registry_root helpers
        assert_eq!(parse_registry_root("HKEY_CLASSES_ROOT"), 0);
        assert_eq!(parse_registry_root("HKCR"), 0);
        assert_eq!(parse_registry_root("HKEY_CURRENT_USER"), 1);
        assert_eq!(parse_registry_root("HKCU"), 1);
        assert_eq!(parse_registry_root("HKEY_USERS"), 3);
        assert_eq!(parse_registry_root("HKU"), 3);
        assert_eq!(parse_registry_root("HKLM"), 2);
        assert_eq!(parse_registry_root("HKEY_LOCAL_MACHINE"), 2);
        assert_eq!(parse_registry_root("UNKNOWN"), 2);

        Ok(())
    }

    /// Tests compiler error handling for elements missing mandatory Id attributes.
    ///
    /// # Errors
    ///
    /// Returns [`Error`] if XML parsing fails unexpectedly.
    #[test]
    fn test_compiler_all_missing_id_errors() -> Result<()> {
        let parser = XmlParser::new();
        let compiler = Compiler::new();

        let test_cases = [
            "<Wix><Product Id=\"{11111111-1111-1111-1111-111111111111}\"><DirectoryRef /></Product></Wix>",
            "<Wix><Product Id=\"{11111111-1111-1111-1111-111111111111}\"><ComponentGroup /></Product></Wix>",
            "<Wix><Product Id=\"{11111111-1111-1111-1111-111111111111}\"><ComponentGroupRef /></Product></Wix>",
            "<Wix><Product Id=\"{11111111-1111-1111-1111-111111111111}\"><PackageGroup /></Product></Wix>",
            "<Wix><Product Id=\"{11111111-1111-1111-1111-111111111111}\"><PackageGroupRef /></Product></Wix>",
            "<Wix><Product Id=\"{11111111-1111-1111-1111-111111111111}\"><FeatureGroup /></Product></Wix>",
            "<Wix><Product Id=\"{11111111-1111-1111-1111-111111111111}\"><FeatureGroupRef /></Product></Wix>",
            "<Wix><Product Id=\"{11111111-1111-1111-1111-111111111111}\"><FeatureRef /></Product></Wix>",
            "<Wix><Product Id=\"{11111111-1111-1111-1111-111111111111}\"><SetDirectory /></Product></Wix>",
            "<Wix><Product Id=\"{11111111-1111-1111-1111-111111111111}\"><RemoveFolder /></Product></Wix>",
            "<Wix><Product Id=\"{11111111-1111-1111-1111-111111111111}\"><Environment /></Product></Wix>",
            "<Wix><Product Id=\"{11111111-1111-1111-1111-111111111111}\"><CustomAction /></Product></Wix>",
            "<Wix><Product Id=\"{11111111-1111-1111-1111-111111111111}\"><UI><Dialog /></UI></Product></Wix>",
            "<Wix><Product Id=\"{11111111-1111-1111-1111-111111111111}\"><UI><Dialog Id=\"D1\"><Control /></Dialog></UI></Product></Wix>",
        ];

        for xml in test_cases {
            let root = parser.parse(xml)?;
            assert!(compiler.compile(&root).is_err());
        }

        Ok(())
    }

    /// Tests converting plain text to RTF format, handling braces, slashes, CRLF, and Unicode.
    #[test]
    fn test_convert_text_to_rtf() {
        // Plain text with newlines, CRLF, special characters, and Unicode
        let plain =
            "Line 1\r\nLine 2 with {braces} and \\backslashes\\ and non-ascii: ä and emoji: 🦀";
        let rtf = convert_text_to_rtf(plain);
        assert!(rtf.starts_with(r"{\rtf1"));
        assert!(rtf.contains(r"\par "));
        assert!(rtf.contains(r"\{braces\}"));
        assert!(rtf.contains(r"\\backslashes\\"));
        assert!(rtf.contains(r"\u"));

        // Already RTF
        let already = r"{\rtf1\ansi Some text}";
        let rtf2 = convert_text_to_rtf(already);
        assert_eq!(rtf2, already);
    }
}

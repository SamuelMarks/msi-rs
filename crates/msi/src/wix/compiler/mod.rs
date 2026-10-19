//! `WiX` compiler pipeline (`candle` architecture).
//!
//! Compiles preprocessed `WiX` XML documents into intermediate object models (`.wixobj`),
//! generating symbols, unresolved references, and intermediate database tables.

mod elements;
mod ui_and_search;

use crate::database::tables::config::EnvironmentRow;
use crate::database::tables::core::{
    ComponentRow, DirectoryRow, FeatureComponentsRow, FeatureRow, FileRow, MediaRow, PropertyRow,
};
use crate::database::tables::file_mgmt::{CreateFolderRow, RemoveFileRow};
use crate::database::tables::record::{FieldValue, Record};
use crate::database::tables::sequence::SequenceRow;
use crate::database::tables::types::{
    ComponentGuid, ComponentName, DirectoryId, FeatureName, FileKey, PropertyName,
};
use crate::error::{MsiError, Result};
use crate::wix::schema::WixSchemaVersion;
use crate::wix::wixobj::{
    IntermediateSection, IntermediateTable, Reference, SectionType, SourceSpan, Symbol, WixObject,
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
#[allow(clippy::cast_possible_truncation)]
pub fn convert_text_to_rtf(input: &str) -> String {
    if input.trim_start().starts_with(r"{\rtf") {
        return input.to_string();
    }

    let normalized = input.replace("\r\n", "\n");
    let mut out = String::from(
        r"{\rtf1\ansi\ansicpg1252\deff0\nouicompat{\fonttbl{\f0\fnil\fcharset0 Tahoma;}}{\colortbl ;\red0\green0\blue0;}\viewkind4\uc1\pard\cf1\f0\fs16 ",
    );
    for ch in normalized.chars() {
        match ch {
            '\\' => out.push_str(r"\\"),
            '{' => out.push_str(r"\{"),
            '}' => out.push_str(r"\}"),
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

use crate::wix::extensions::ExtensionRegistry;
use std::cell::RefCell;
use std::collections::HashMap;
use std::path::PathBuf;

/// `WiX` compiler translating XML documents into [`WixObject`].
#[derive(Debug, Default)]
pub struct Compiler {
    /// Registry of loaded extensions.
    extension_registry: ExtensionRegistry,
    /// Namespace prefixes declared on the root element.
    prefixes: RefCell<HashMap<String, String>>,
    /// Optional source file parent directory for resolving relative paths during compilation.
    pub(super) source_dir: Option<PathBuf>,
}

impl Compiler {
    /// Creates a new [`Compiler`].
    ///
    /// # Returns
    ///
    /// A new compiler.
    #[must_use]
    pub fn new() -> Self {
        Self {
            extension_registry: ExtensionRegistry::with_builtin_extensions(),
            prefixes: RefCell::new(HashMap::new()),
            source_dir: None,
        }
    }

    /// Creates a new [`Compiler`] with the given extension registry.
    ///
    /// # Arguments
    ///
    /// * `registry` - The extension registry to use.
    ///
    /// # Returns
    ///
    /// A newly constructed instance of `Self`.
    #[must_use]
    pub fn with_extensions(registry: ExtensionRegistry) -> Self {
        Self {
            extension_registry: registry,
            prefixes: RefCell::new(HashMap::new()),
            source_dir: None,
        }
    }

    /// Sets the source file's parent directory for resolving relative paths.
    ///
    /// # Arguments
    ///
    /// * `dir` - Parent directory path.
    ///
    /// # Returns
    ///
    /// Updated [`Compiler`] instance.
    #[must_use]
    pub fn with_source_dir(mut self, dir: impl Into<PathBuf>) -> Self {
        self.source_dir = Some(dir.into());
        self
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
    /// Returns [`MsiError::WixCompiler`] if any mandatory attribute or element structure is invalid.
    pub fn compile(&self, root: &XmlNode) -> Result<WixObject> {
        let mut obj = WixObject::new();

        self.prefixes.borrow_mut().clear();

        // Check namespace if present
        for (k, v) in &root.attributes {
            if k == "xmlns" {
                let _ = WixSchemaVersion::from_uri(v)?;
            } else if let Some(prefix) = k.strip_prefix("xmlns:") {
                let mut map = self.prefixes.borrow_mut();
                map.insert(prefix.to_string(), v.clone());
            }
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
    ///
    /// # Arguments
    ///
    /// * `node` - Parsed XML element node being compiled.
    /// * `sec_type` - The sec type value.
    ///
    /// # Returns
    ///
    /// `Ok(IntermediateSection)` on success, or an error if the operation fails.
    #[allow(clippy::cognitive_complexity)]
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
        let mut tables_map: HashMap<String, IntermediateTable> = HashMap::new();

        // Extract Product attributes as Property rows
        if sec_type == SectionType::Product {
            let prop_table = tables_map
                .entry("Property".to_string())
                .or_insert_with(|| IntermediateTable::new("Property"));

            if let Some(ver) = node.attribute("Version") {
                let p = PropertyRow {
                    property: PropertyName::from_static("ProductVersion"),
                    value: ver.to_string(),
                };
                prop_table.push_record(p.to_record());
            }
            if let Some(name) = node.attribute("Name") {
                let p = PropertyRow {
                    property: PropertyName::from_static("ProductName"),
                    value: name.to_string(),
                };
                prop_table.push_record(p.to_record());
            }
            if let Some(mfg) = node.attribute("Manufacturer") {
                let p = PropertyRow {
                    property: PropertyName::from_static("Manufacturer"),
                    value: mfg.to_string(),
                };
                prop_table.push_record(p.to_record());
            }
            if let Some(code) = node
                .attribute("ProductCode")
                .or_else(|| node.attribute("Id"))
            {
                let effective_code = if code == "*" || code == "?" {
                    let name_seed = node.attribute("Name").unwrap_or("Product");
                    let mfg_seed = node.attribute("Manufacturer").unwrap_or("Manufacturer");
                    let ver_seed = node.attribute("Version").unwrap_or("1.0.0");
                    let seed = format!("{name_seed}_{mfg_seed}_{ver_seed}");
                    ComponentGuid::generate_deterministic("ProductCode", &seed).to_string()
                } else {
                    code.to_string()
                };
                let p = PropertyRow {
                    property: PropertyName::from_static("ProductCode"),
                    value: effective_code,
                };
                prop_table.push_record(p.to_record());
            }
            if let Some(upg) = node.attribute("UpgradeCode") {
                let p = PropertyRow {
                    property: PropertyName::from_static("UpgradeCode"),
                    value: upg.to_string(),
                };
                prop_table.push_record(p.to_record());
            }
            if let Some(lang) = node
                .attribute("Language")
                .or_else(|| node.attribute("Languages"))
            {
                let p = PropertyRow {
                    property: PropertyName::from_static("ProductLanguage"),
                    value: lang.to_string(),
                };
                prop_table.push_record(p.to_record());
            }
            if let Some(cp) = node
                .attribute("Codepage")
                .or_else(|| node.attribute("SummaryCodepage"))
            {
                let p = PropertyRow {
                    property: PropertyName::from_static("ProductCodepage"),
                    value: cp.to_string(),
                };
                prop_table.push_record(p.to_record());
            }
            if let Some(iv) = node.attribute("InstallerVersion") {
                let p = PropertyRow {
                    property: PropertyName::from_static("InstallerVersion"),
                    value: iv.to_string(),
                };
                prop_table.push_record(p.to_record());
            }
            if let Some(cmp) = node.attribute("Compressed") {
                let p = PropertyRow {
                    property: PropertyName::from_static("ProductCompressed"),
                    value: cmp.to_string(),
                };
                prop_table.push_record(p.to_record());
            }
            if let Some(desc) = node.attribute("Description") {
                let p = PropertyRow {
                    property: PropertyName::from_static("ProductDescription"),
                    value: desc.to_string(),
                };
                prop_table.push_record(p.to_record());
            }
            if let Some(comm) = node.attribute("Comments") {
                let p = PropertyRow {
                    property: PropertyName::from_static("ProductComments"),
                    value: comm.to_string(),
                };
                prop_table.push_record(p.to_record());
            }
            if let Some(kw) = node.attribute("Keywords") {
                let p = PropertyRow {
                    property: PropertyName::from_static("ProductKeywords"),
                    value: kw.to_string(),
                };
                prop_table.push_record(p.to_record());
            }
            if let Some(plt) = node.attribute("Platform") {
                let p = PropertyRow {
                    property: PropertyName::from_static("ProductPlatform"),
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
                    property: PropertyName::from_static("ModuleVersion"),
                    value: ver.to_string(),
                };
                prop_table.push_record(p.to_record());
            }
            if let Some(lang) = node.attribute("Language") {
                let p = PropertyRow {
                    property: PropertyName::from_static("ModuleLanguage"),
                    value: lang.to_string(),
                };
                prop_table.push_record(p.to_record());
            }
            if let Some(mfg) = node.attribute("Manufacturer") {
                let p = PropertyRow {
                    property: PropertyName::from_static("ModuleManufacturer"),
                    value: mfg.to_string(),
                };
                prop_table.push_record(p.to_record());
            }
            if let Some(guid) = node.attribute("Guid").or_else(|| node.attribute("Id")) {
                let p = PropertyRow {
                    property: PropertyName::from_static("ModuleId"),
                    value: guid.to_string(),
                };
                prop_table.push_record(p.to_record());
            }
            if let Some(cp) = node
                .attribute("Codepage")
                .or_else(|| node.attribute("SummaryCodepage"))
            {
                let p = PropertyRow {
                    property: PropertyName::from_static("ModuleCodepage"),
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
    ///
    /// # Arguments
    ///
    /// * `node` - Parsed XML element node being compiled.
    /// * `parent_id` - Optional identifier of the enclosing parent element or component.
    /// * `section` - Intermediate object section receiving symbols and table rows.
    /// * `tables` - Map of intermediate MSI tables being populated during compilation.
    ///
    /// # Returns
    ///
    /// `Ok(())` on success, or an [`MsiError`] if the operation fails.
    #[allow(clippy::cognitive_complexity)]
    #[allow(clippy::too_many_lines)]
    #[allow(clippy::self_only_used_in_recursion)]
    fn compile_element_tree(
        &self,
        node: &XmlNode,
        parent_id: Option<&str>,
        section: &mut IntermediateSection,
        tables: &mut HashMap<String, IntermediateTable>,
    ) -> Result<()> {
        for child in &node.children {
            match child.tag.as_str() {
                "Directory" => {
                    let dir_id_str =
                        child.attribute("Id").ok_or_else(|| MsiError::WixCompiler {
                            element: "Directory".to_string(),
                            message: "missing required 'Id' attribute".to_string(),
                        })?;
                    let dir_name = child.attribute("Name").unwrap_or(dir_id_str);

                    let dir_id = DirectoryId::new(dir_id_str)?;
                    let parent_dir = parent_id.map(DirectoryId::from_validated);

                    section.add_symbol(Symbol::new("Directory", dir_id.as_str()));
                    if let Some(ref p) = parent_dir {
                        section.add_reference(Reference::new("Directory", p.as_str()));
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
                    let comp_id_str =
                        child.attribute("Id").ok_or_else(|| MsiError::WixCompiler {
                            element: "Component".to_string(),
                            message: "missing required 'Id' attribute".to_string(),
                        })?;
                    let comp_name = ComponentName::new(comp_id_str)?;
                    let dir_id = parent_id.map_or_else(
                        || DirectoryId::from_static("TARGETDIR"),
                        DirectoryId::from_validated,
                    );

                    let guid_str = child.attribute("Guid");
                    let comp_guid = match guid_str {
                        Some(g) if !g.is_empty() && g != "*" && g != "?" => {
                            Some(ComponentGuid::parse(g)?)
                        }
                        Some("*" | "?") => Some(ComponentGuid::generate_deterministic(
                            dir_id.as_str(),
                            comp_id_str,
                        )),
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

                    section.add_symbol(Symbol::new("Component", comp_name.as_str()));
                    section.add_reference(Reference::new("Directory", dir_id.as_str()));

                    let row = ComponentRow {
                        component: comp_name.clone(),
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

                    self.compile_element_tree(child, Some(comp_name.as_str()), section, tables)?;
                }
                "File" => {
                    let file_id_str =
                        child.attribute("Id").ok_or_else(|| MsiError::WixCompiler {
                            element: "File".to_string(),
                            message: "missing required 'Id' attribute".to_string(),
                        })?;
                    let file_name = child
                        .attribute("Name")
                        .or_else(|| {
                            child
                                .attribute("Source")
                                .map(|src| src.rsplit(['/', '\\']).next().unwrap_or(src))
                        })
                        .unwrap_or(file_id_str);
                    let file_key = FileKey::new(file_id_str)?;
                    let comp_name = parent_id.map_or_else(
                        || ComponentName::from_static("DefaultComp"),
                        ComponentName::from_validated,
                    );

                    section.add_symbol(Symbol::new("File", file_key.as_str()));
                    section.add_reference(Reference::new("Component", comp_name.as_str()));

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
                    let existing_file_names: Vec<String> = tables
                        .get("File")
                        .map(|t| {
                            t.records
                                .iter()
                                .filter_map(|r| r.get(2).map(ToString::to_string))
                                .collect()
                        })
                        .unwrap_or_default();
                    let final_file_name =
                        format_msi_filename(short_name, file_name, &existing_file_names);

                    let row = FileRow {
                        file: file_key.clone(),
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

                    let file_disk_id: i16 = child
                        .attribute("DiskId")
                        .and_then(|d| d.parse().ok())
                        .unwrap_or(1);

                    if let Some(src) = child.attribute("Source") {
                        tables
                            .entry("WixFile".to_string())
                            .or_insert_with(|| IntermediateTable::new("WixFile"))
                            .push_record(Record::with_fields(vec![
                                FieldValue::String(file_id_str.to_string()),
                                FieldValue::String(src.to_string()),
                                FieldValue::Short(file_disk_id),
                            ]));
                    }

                    for sub in &child.children {
                        if sub.tag == "Font" {
                            let font_title = sub.attribute("Title").map(ToString::to_string);
                            let font_row = crate::database::tables::core::FontRow {
                                file: file_key.clone(),
                                font_title,
                            };
                            tables
                                .entry("Font".to_string())
                                .or_insert_with(|| IntermediateTable::new("Font"))
                                .push_record(font_row.to_record());
                        } else if sub.tag == "Shortcut" {
                            let target = format!("[#{file_id_str}]");
                            let mut sc_node = sub.clone();
                            if sc_node.attribute("Target").is_none() {
                                sc_node.attributes.insert("Target".to_string(), target);
                            }
                            elements::compile_shortcut(&sc_node, parent_id, section, tables);
                        }
                    }
                }
                "Feature" => {
                    let feat_id_str =
                        child.attribute("Id").ok_or_else(|| MsiError::WixCompiler {
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
                    let parent_feat = parent_id.map(FeatureName::from_validated);

                    section.add_symbol(Symbol::new("Feature", feat_id_str));
                    if let Some(p) = parent_id {
                        section.add_reference(Reference::new("Feature", p));
                    }

                    let row = FeatureRow {
                        feature: feat_name.clone(),
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
                        let sub_span = SourceSpan::new(sub.line, sub.column);
                        if sub.tag == "ComponentRef" {
                            if let Some(comp_ref_id) = sub.attribute("Id") {
                                let comp_name = ComponentName::new(comp_ref_id)?;
                                section.add_reference(Reference::with_span(
                                    "Component",
                                    comp_name.as_str(),
                                    sub_span,
                                ));
                                let fc_row = FeatureComponentsRow {
                                    feature: feat_name.clone(),
                                    component: comp_name,
                                };
                                tables
                                    .entry("FeatureComponents".to_string())
                                    .or_insert_with(|| IntermediateTable::new("FeatureComponents"))
                                    .push_record(fc_row.to_record());
                            }
                        } else if sub.tag == "ComponentGroupRef" {
                            let cg_ref_id = sub.attribute("Id").unwrap_or("");
                            section.add_reference(Reference::with_span(
                                "ComponentGroup",
                                cg_ref_id,
                                sub_span,
                            ));
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
                                section
                                    .add_reference(Reference::with_span("Module", m_id, sub_span));
                            }
                        }
                    }

                    self.compile_element_tree(child, Some(feat_id_str), section, tables)?;
                }
                "DirectoryRef" => {
                    let dir_id_str =
                        child.attribute("Id").ok_or_else(|| MsiError::WixCompiler {
                            element: "DirectoryRef".to_string(),
                            message: "missing required 'Id' attribute".to_string(),
                        })?;
                    let dir_id = DirectoryId::new(dir_id_str)?;
                    let span = SourceSpan::new(child.line, child.column);
                    section.add_reference(Reference::with_span("Directory", dir_id.as_str(), span));
                    self.compile_element_tree(child, Some(dir_id.as_str()), section, tables)?;
                }
                "ComponentGroup" => {
                    let group_id_str =
                        child.attribute("Id").ok_or_else(|| MsiError::WixCompiler {
                            element: "ComponentGroup".to_string(),
                            message: "missing required 'Id' attribute".to_string(),
                        })?;
                    let span = SourceSpan::new(child.line, child.column);
                    section.add_symbol(Symbol::with_span("ComponentGroup", group_id_str, span));
                    let comp_dir = child.attribute("Directory").or(parent_id);
                    if let Some(dir) = child.attribute("Directory") {
                        let dir_id = DirectoryId::new(dir)?;
                        section.add_reference(Reference::with_span(
                            "Directory",
                            dir_id.as_str(),
                            span,
                        ));
                    }

                    for sub in &child.children {
                        let sub_span = SourceSpan::new(sub.line, sub.column);
                        if sub.tag == "ComponentRef" {
                            if let Some(comp_ref_id) = sub.attribute("Id") {
                                let comp_name = ComponentName::new(comp_ref_id)?;
                                section.add_reference(Reference::with_span(
                                    "Component",
                                    comp_name.as_str(),
                                    sub_span,
                                ));
                                tables
                                    .entry("_ComponentGroupMember".to_string())
                                    .or_insert_with(|| {
                                        IntermediateTable::new("_ComponentGroupMember")
                                    })
                                    .push_record(Record::with_fields(vec![
                                        FieldValue::String(group_id_str.to_string()),
                                        FieldValue::String(comp_name.as_str().to_string()),
                                    ]));
                            }
                        } else if sub.tag == "Component" {
                            let comp_id = sub.attribute("Id").unwrap_or("");
                            let comp_name = ComponentName::new(comp_id)?;
                            tables
                                .entry("_ComponentGroupMember".to_string())
                                .or_insert_with(|| IntermediateTable::new("_ComponentGroupMember"))
                                .push_record(Record::with_fields(vec![
                                    FieldValue::String(group_id_str.to_string()),
                                    FieldValue::String(comp_name.as_str().to_string()),
                                ]));
                        } else if sub.tag == "ComponentGroupRef" {
                            let cg_ref_id = sub.attribute("Id").unwrap_or("");
                            section.add_reference(Reference::with_span(
                                "ComponentGroup",
                                cg_ref_id,
                                sub_span,
                            ));
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
                    let group_ref_id =
                        child.attribute("Id").ok_or_else(|| MsiError::WixCompiler {
                            element: "ComponentGroupRef".to_string(),
                            message: "missing required 'Id' attribute".to_string(),
                        })?;
                    let span = SourceSpan::new(child.line, child.column);
                    section.add_reference(Reference::with_span(
                        "ComponentGroup",
                        group_ref_id,
                        span,
                    ));
                }
                "PackageGroup" => {
                    let group_id_str =
                        child.attribute("Id").ok_or_else(|| MsiError::WixCompiler {
                            element: "PackageGroup".to_string(),
                            message: "missing required 'Id' attribute".to_string(),
                        })?;
                    let span = SourceSpan::new(child.line, child.column);
                    section.add_symbol(Symbol::with_span("PackageGroup", group_id_str, span));
                    self.compile_element_tree(child, parent_id, section, tables)?;
                }
                "PackageGroupRef" => {
                    let group_ref_id =
                        child.attribute("Id").ok_or_else(|| MsiError::WixCompiler {
                            element: "PackageGroupRef".to_string(),
                            message: "missing required 'Id' attribute".to_string(),
                        })?;
                    let span = SourceSpan::new(child.line, child.column);
                    section.add_reference(Reference::with_span("PackageGroup", group_ref_id, span));
                }
                "FeatureGroup" => {
                    let group_id_str =
                        child.attribute("Id").ok_or_else(|| MsiError::WixCompiler {
                            element: "FeatureGroup".to_string(),
                            message: "missing required 'Id' attribute".to_string(),
                        })?;
                    let span = SourceSpan::new(child.line, child.column);
                    section.add_symbol(Symbol::with_span("FeatureGroup", group_id_str, span));
                    self.compile_element_tree(child, parent_id, section, tables)?;
                }
                "FeatureGroupRef" => {
                    let group_ref_id =
                        child.attribute("Id").ok_or_else(|| MsiError::WixCompiler {
                            element: "FeatureGroupRef".to_string(),
                            message: "missing required 'Id' attribute".to_string(),
                        })?;
                    let span = SourceSpan::new(child.line, child.column);
                    section.add_reference(Reference::with_span("FeatureGroup", group_ref_id, span));
                }
                "FeatureRef" => {
                    let feat_ref_id =
                        child.attribute("Id").ok_or_else(|| MsiError::WixCompiler {
                            element: "FeatureRef".to_string(),
                            message: "missing required 'Id' attribute".to_string(),
                        })?;
                    let feat_name = FeatureName::new(feat_ref_id)?;
                    let feat_span = SourceSpan::new(child.line, child.column);
                    section.add_reference(Reference::with_span("Feature", feat_ref_id, feat_span));

                    for sub in &child.children {
                        let sub_span = SourceSpan::new(sub.line, sub.column);
                        if sub.tag == "ComponentRef" {
                            if let Some(comp_ref_id) = sub.attribute("Id") {
                                let comp_name = ComponentName::new(comp_ref_id)?;
                                section.add_reference(Reference::with_span(
                                    "Component",
                                    comp_name.as_str(),
                                    sub_span,
                                ));
                                let fc_row = FeatureComponentsRow {
                                    feature: feat_name.clone(),
                                    component: comp_name,
                                };
                                tables
                                    .entry("FeatureComponents".to_string())
                                    .or_insert_with(|| IntermediateTable::new("FeatureComponents"))
                                    .push_record(fc_row.to_record());
                            }
                        } else if sub.tag == "ComponentGroupRef" {
                            let cg_ref_id = sub.attribute("Id").unwrap_or("");
                            section.add_reference(Reference::with_span(
                                "ComponentGroup",
                                cg_ref_id,
                                sub_span,
                            ));
                            tables
                                .entry("_FeatureComponentGroupRef".to_string())
                                .or_insert_with(|| {
                                    IntermediateTable::new("_FeatureComponentGroupRef")
                                })
                                .push_record(Record::with_fields(vec![
                                    FieldValue::String(feat_ref_id.to_string()),
                                    FieldValue::String(cg_ref_id.to_string()),
                                ]));
                        }
                    }

                    self.compile_element_tree(child, Some(feat_ref_id), section, tables)?;
                }
                "Package" => {
                    let prop_table = tables
                        .entry("Property".to_string())
                        .or_insert_with(|| IntermediateTable::new("Property"));

                    if let Some(iv) = child.attribute("InstallerVersion") {
                        let p = PropertyRow {
                            property: PropertyName::from_static("InstallerVersion"),
                            value: iv.to_string(),
                        };
                        prop_table.push_record(p.to_record());
                    }
                    if let Some(cmp) = child.attribute("Compressed") {
                        let p = PropertyRow {
                            property: PropertyName::from_static("ProductCompressed"),
                            value: cmp.to_string(),
                        };
                        prop_table.push_record(p.to_record());
                    }
                    if let Some(desc) = child.attribute("Description") {
                        let p = PropertyRow {
                            property: PropertyName::from_static("ProductDescription"),
                            value: desc.to_string(),
                        };
                        prop_table.push_record(p.to_record());
                    }
                    if let Some(comm) = child.attribute("Comments") {
                        let p = PropertyRow {
                            property: PropertyName::from_static("ProductComments"),
                            value: comm.to_string(),
                        };
                        prop_table.push_record(p.to_record());
                    }
                    if let Some(kw) = child.attribute("Keywords") {
                        let p = PropertyRow {
                            property: PropertyName::from_static("ProductKeywords"),
                            value: kw.to_string(),
                        };
                        prop_table.push_record(p.to_record());
                    }
                    if let Some(lang) = child
                        .attribute("Languages")
                        .or_else(|| child.attribute("Language"))
                    {
                        let p = PropertyRow {
                            property: PropertyName::from_static("ProductLanguage"),
                            value: lang.to_string(),
                        };
                        prop_table.push_record(p.to_record());
                    }
                    if let Some(mfg) = child.attribute("Manufacturer") {
                        let p = PropertyRow {
                            property: PropertyName::from_static("Manufacturer"),
                            value: mfg.to_string(),
                        };
                        prop_table.push_record(p.to_record());
                    }
                    if let Some(plt) = child.attribute("Platform") {
                        let p = PropertyRow {
                            property: PropertyName::from_static("ProductPlatform"),
                            value: plt.to_string(),
                        };
                        prop_table.push_record(p.to_record());
                    }
                    if let Some(cp) = child
                        .attribute("SummaryCodepage")
                        .or_else(|| child.attribute("Codepage"))
                    {
                        let p = PropertyRow {
                            property: PropertyName::from_static("ProductCodepage"),
                            value: cp.to_string(),
                        };
                        prop_table.push_record(p.to_record());
                    }
                    if let Some(scope) = child.attribute("InstallScope") {
                        if scope.eq_ignore_ascii_case("perMachine") {
                            let p = PropertyRow {
                                property: PropertyName::from_static("ALLUSERS"),
                                value: "1".to_string(),
                            };
                            prop_table.push_record(p.to_record());
                        } else if scope.eq_ignore_ascii_case("perUser") {
                            let p_user = PropertyRow {
                                property: PropertyName::from_static("MSIINSTALLPERUSER"),
                                value: "1".to_string(),
                            };
                            prop_table.push_record(p_user.to_record());
                        }
                    }
                    if let Some(privileges) = child.attribute("InstallPrivileges") {
                        let p = PropertyRow {
                            property: PropertyName::from_static("InstallPrivileges"),
                            value: privileges.to_string(),
                        };
                        prop_table.push_record(p.to_record());
                    }

                    self.compile_element_tree(child, parent_id, section, tables)?;
                }
                "SetDirectory" => {
                    let dir_id_str =
                        child.attribute("Id").ok_or_else(|| MsiError::WixCompiler {
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
                    let comp_name = parent_id.map_or_else(
                        || ComponentName::from_static("DefaultComp"),
                        ComponentName::from_validated,
                    );
                    let row = CreateFolderRow {
                        directory: DirectoryId::new(dir_id_str)?,
                        component: comp_name,
                    };
                    tables
                        .entry("CreateFolder".to_string())
                        .or_insert_with(|| IntermediateTable::new("CreateFolder"))
                        .push_record(row.to_record());
                }
                "RemoveFolder" => {
                    let rem_id = child.attribute("Id").ok_or_else(|| MsiError::WixCompiler {
                        element: "RemoveFolder".to_string(),
                        message: "missing required 'Id' attribute".to_string(),
                    })?;
                    let dir_prop = child
                        .attribute("Directory")
                        .or_else(|| child.attribute("Property"))
                        .or(parent_id)
                        .unwrap_or("TARGETDIR");
                    let comp_name = parent_id.map_or_else(
                        || ComponentName::from_static("DefaultComp"),
                        ComponentName::from_validated,
                    );
                    let on_mode = match child.attribute("On") {
                        Some("install") => 2,
                        Some("uninstall") => 1,
                        _ => 3,
                    };
                    let row = RemoveFileRow {
                        file_key: rem_id.to_string(),
                        component: comp_name,
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
                    let env_id = child.attribute("Id").ok_or_else(|| MsiError::WixCompiler {
                        element: "Environment".to_string(),
                        message: "missing required 'Id' attribute".to_string(),
                    })?;
                    let raw_name = child.attribute("Name").unwrap_or("");
                    let value = child.attribute("Value").unwrap_or("");
                    let comp_name = parent_id.map_or_else(
                        || ComponentName::from_static("DefaultComp"),
                        ComponentName::from_validated,
                    );

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

                    let part = child.attribute("Part").unwrap_or("all");
                    let mut val_str = value.to_string();
                    if part.eq_ignore_ascii_case("last") && !val_str.contains("[~]") {
                        val_str = format!("[~];{val_str}");
                    } else if part.eq_ignore_ascii_case("first") && !val_str.contains("[~]") {
                        val_str = format!("{val_str};[~]");
                    }

                    let row = EnvironmentRow {
                        environment: env_id.to_string(),
                        name: formatted_name,
                        value: val_str,
                        component: comp_name,
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
                    let mut cabinet = child.attribute("Cabinet").map(ToString::to_string);
                    let is_embedded = child
                        .attribute("EmbedCab")
                        .is_some_and(|v| v.eq_ignore_ascii_case("yes"));
                    if is_embedded {
                        if let Some(ref mut c) = cabinet {
                            if !c.starts_with('#') {
                                *c = format!("#{c}");
                            }
                        }
                    }
                    let disk_prompt = child.attribute("DiskPrompt").map(ToString::to_string);
                    let volume_label = child.attribute("VolumeLabel").map(ToString::to_string);
                    let source = child.attribute("Source").map(ToString::to_string);
                    let compression_level = child.attribute("CompressionLevel");

                    section.add_symbol(Symbol::new("Media", format!("{disk_id}")));

                    let row = MediaRow {
                        disk_id,
                        last_sequence: 1,
                        disk_prompt,
                        cabinet,
                        volume_label,
                        source,
                    };
                    tables
                        .entry("Media".to_string())
                        .or_insert_with(|| IntermediateTable::new("Media"))
                        .push_record(row.to_record());

                    if let Some(comp_lvl) = compression_level {
                        tables
                            .entry("WixMediaCompression".to_string())
                            .or_insert_with(|| IntermediateTable::new("WixMediaCompression"))
                            .push_record(Record::with_fields(vec![
                                FieldValue::Short(disk_id),
                                FieldValue::String(comp_lvl.to_string()),
                            ]));
                    }

                    if child
                        .attribute("EmbedCab")
                        .is_some_and(|v| v.eq_ignore_ascii_case("no"))
                    {
                        tables
                            .entry("WixExternalMedia".to_string())
                            .or_insert_with(|| IntermediateTable::new("WixExternalMedia"))
                            .push_record(Record::with_fields(vec![FieldValue::Short(disk_id)]));
                    }
                }
                "WixVariable" => {
                    let var_id = child.attribute("Id").ok_or_else(|| MsiError::WixCompiler {
                        element: "WixVariable".to_string(),
                        message: "missing required 'Id' attribute".to_string(),
                    })?;
                    let val = child
                        .attribute("Value")
                        .map_or_else(|| child.text.as_str(), |v| v);
                    let overridable = child
                        .attribute("Overridable")
                        .is_some_and(|v| v.eq_ignore_ascii_case("yes"));

                    section.add_symbol(Symbol::new("WixVariable", var_id));

                    let rec = Record::with_fields(vec![
                        FieldValue::String(var_id.to_string()),
                        FieldValue::String(val.to_string()),
                        FieldValue::Short(i16::from(overridable)),
                    ]);
                    tables
                        .entry("WixVariable".to_string())
                        .or_insert_with(|| IntermediateTable::new("WixVariable"))
                        .push_record(rec);
                }
                "Property" => {
                    let prop_id_str =
                        child.attribute("Id").ok_or_else(|| MsiError::WixCompiler {
                            element: "Property".to_string(),
                            message: "missing required 'Id' attribute".to_string(),
                        })?;
                    let prop_val = child.attribute("Value").unwrap_or("");

                    let prop_name = PropertyName::new(prop_id_str)?;
                    section.add_symbol(Symbol::new("Property", prop_id_str));

                    let has_search_children = child.children.iter().any(|c| {
                        matches!(
                            c.tag.as_str(),
                            "RegistrySearch"
                                | "DirectorySearch"
                                | "FileSearch"
                                | "IniFileSearch"
                                | "ComponentSearch"
                        )
                    });

                    if !prop_val.is_empty() || !has_search_children {
                        let row = PropertyRow {
                            property: prop_name,
                            value: prop_val.to_string(),
                        };
                        tables
                            .entry("Property".to_string())
                            .or_insert_with(|| IntermediateTable::new("Property"))
                            .push_record(row.to_record());
                    }

                    self.compile_element_tree(child, Some(prop_id_str), section, tables)?;
                }
                "RegistryKey" => {
                    let key = child.attribute("Key").unwrap_or("Software\\Product");
                    let root = child.attribute("Root").unwrap_or("HKLM");
                    for sub in &child.children {
                        if sub.tag == "RegistryValue" {
                            elements::compile_registry_value(
                                sub,
                                parent_id,
                                Some(root),
                                Some(key),
                                section,
                                tables,
                            );
                        }
                    }
                    self.compile_element_tree(child, parent_id, section, tables)?;
                }
                "RegistryValue" => {
                    if node.tag != "RegistryKey" {
                        elements::compile_registry_value(
                            child, parent_id, None, None, section, tables,
                        );
                    }
                }
                "Shortcut" => {
                    elements::compile_shortcut(child, parent_id, section, tables);
                }
                "ServiceInstall" => {
                    elements::compile_service_install(child, parent_id, section, tables);
                }
                "ServiceControl" => {
                    elements::compile_service_control(child, parent_id, section, tables);
                }
                "EmbeddedChainer" => {
                    elements::compile_embedded_chainer(child, section, tables)?;
                }
                "sql:SqlDatabase" | "SqlDatabase" => {
                    elements::compile_sql_database(child, parent_id, section, tables)?;
                    self.compile_element_tree(child, child.attribute("Id"), section, tables)?;
                }
                "sql:SqlString" | "SqlString" => {
                    elements::compile_sql_string(child, parent_id, section, tables)?;
                }
                "sql:SqlScript" | "SqlScript" => {
                    elements::compile_sql_script(child, parent_id, section, tables)?;
                }
                "CustomAction" => {
                    ui_and_search::compile_custom_action(child, section, tables)?;
                }
                "Dialog" => {
                    ui_and_search::compile_dialog(child, section, tables)?;
                    self.compile_element_tree(child, child.attribute("Id"), section, tables)?;
                }
                "Control" => {
                    ui_and_search::compile_control(self, child, parent_id, section, tables)?;
                }
                "TextStyle" => {
                    ui_and_search::compile_text_style(child, section, tables);
                }
                "Upgrade" => {
                    ui_and_search::compile_upgrade(child, section, tables);
                }
                "MajorUpgrade" => {
                    ui_and_search::compile_major_upgrade(child, tables);
                }
                "posix:File" | "PosixFile" | "posix:Symlink" | "PosixSymlink" | "posix:Daemon"
                | "PosixDaemon" | "posix:Acl" | "PosixAcl" | "posix:Desktop" | "PosixDesktop" => {
                    elements::compile_posix_element(child, parent_id, section, tables);
                }
                "Configuration" => {
                    elements::compile_module_configuration(child, tables);
                }
                "Substitution" => {
                    elements::compile_module_substitution(child, tables);
                }
                "IgnoreModularization" => {
                    elements::compile_module_ignore_modularization(child, tables);
                }
                "Dependency" | "ModuleDependency" => {
                    elements::compile_module_dependency(child, tables);
                }
                "Exclusion" | "ModuleExclusion" => {
                    elements::compile_module_exclusion(child, tables);
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
                    elements::compile_media_template(child, tables);
                }
                "CopyFile" => {
                    elements::compile_copy_file(child, parent_id, section, tables);
                }
                "MoveFile" => {
                    elements::compile_move_file(child, parent_id, section, tables);
                }
                "RemoveFile" => {
                    elements::compile_remove_file(child, parent_id, section, tables);
                }
                "SymbolicLink" | "Hardlink" => {
                    elements::compile_symlink(child, parent_id, section, tables)?;
                }
                "RemoveRegistryKey" | "RemoveRegistryValue" => {
                    elements::compile_remove_registry(child, parent_id, section, tables);
                }
                "IniFile" => {
                    elements::compile_ini_file(child, parent_id, section, tables);
                }
                "RemoveIniFile" => {
                    elements::compile_remove_ini_file(child, parent_id, section, tables);
                }
                "Icon" => {
                    elements::compile_icon(child, section, tables);
                }
                "ProgId" => {
                    elements::compile_prog_id(child, parent_id, section, tables);
                }
                "Extension" => {
                    elements::compile_extension(child, parent_id, section, tables);
                }
                "MIME" => {
                    elements::compile_mime(child, section, tables);
                }
                "Class" => {
                    elements::compile_class(child, parent_id, section, tables);
                }
                "TypeLib" => {
                    elements::compile_type_lib(child, parent_id, section, tables);
                }
                "AppId" => {
                    elements::compile_app_id(child, section, tables);
                }
                "PropertyRef" => {
                    if let Some(prop_ref) = child.attribute("Id") {
                        section.add_reference(Reference::new("Property", prop_ref));
                    }
                }
                "SetProperty" => {
                    ui_and_search::compile_set_property(child, section, tables)?;
                }
                "AppSearch" => {
                    ui_and_search::compile_app_search(child, section, tables);
                }
                "RegistrySearch" => {
                    ui_and_search::compile_registry_search(child, parent_id, section, tables);
                }
                "DirectorySearch" => {
                    ui_and_search::compile_directory_search(child, parent_id, section, tables);
                }
                "FileSearch" => {
                    ui_and_search::compile_file_search(child, parent_id, section, tables);
                }
                "IniFileSearch" => {
                    ui_and_search::compile_ini_file_search(child, parent_id, section, tables);
                }
                "ComponentSearch" => {
                    ui_and_search::compile_component_search(child, parent_id, section, tables);
                }
                "Launch" | "Condition" => {
                    ui_and_search::compile_launch_condition(child, tables);
                }
                "InstallExecuteSequence"
                | "InstallUISequence"
                | "AdminExecuteSequence"
                | "AdminUISequence"
                | "AdvtExecuteSequence" => {
                    ui_and_search::compile_sequence_table(child, section, tables)?;
                }
                "CustomActionRef" => {
                    if let Some(ca_ref) = child.attribute("Id") {
                        section.add_reference(Reference::new("CustomAction", ca_ref));
                    }
                }
                "PatchCreation" | "PatchInformation" | "PatchMetadata" | "PatchFamily"
                | "Patch" => {
                    ui_and_search::compile_patch_element(child, section, tables);
                }
                "UIRef" => {
                    ui_and_search::compile_ui_ref(child, section, tables);
                }
                "ControlEvent" | "Publish" => {
                    ui_and_search::compile_control_event(child, parent_id, tables);
                }
                "ControlCondition" => {
                    ui_and_search::compile_control_condition(child, parent_id, tables);
                }
                "Subscribe" => {
                    ui_and_search::compile_subscribe(child, parent_id, tables);
                }
                "Binary" => {
                    ui_and_search::compile_binary(child, section, tables);
                }
                "Billboard" | "BillboardControl" => {
                    ui_and_search::compile_billboard(child, section, tables);
                }
                "ProgressText" => {
                    ui_and_search::compile_progress_text(child, tables);
                }
                "Error" => {
                    ui_and_search::compile_error(child, tables);
                }
                "Merge" | "MergeRef" => {
                    if let Some(m_id) = child.attribute("Id") {
                        section.add_reference(Reference::new("Module", m_id));
                    }
                }
                _ => {
                    if let Some((prefix, _)) = child.tag.split_once(':') {
                        let mut handled = false;
                        if let Some(ns) = self.prefixes.borrow().get(prefix) {
                            if let Some(ext) = self.extension_registry.get_by_namespace(ns) {
                                ext.compile_node(child, parent_id, section, tables)?;
                                handled = true;
                            }
                        }

                        if !handled {
                            ui_and_search::compile_extension_element(
                                child, parent_id, section, tables,
                            );
                            self.compile_element_tree(child, parent_id, section, tables)?;
                        }
                    } else {
                        self.compile_element_tree(child, parent_id, section, tables)?;
                    }
                }
            }
        }

        Ok(())
    }
}

/// Formats an MSI `Filename` column value (`ShortName|LongName` or `Name`) with 8.3 sanitization
/// and collision disambiguation (`~1`, `~2`, ...).
///
/// # Arguments
///
/// * `short_name` - Optional explicit short name.
/// * `long_name` - The long file or shortcut name.
/// * `existing` - Existing formatted `Filename` entries in the table/directory for collision tracking.
///
/// # Returns
///
/// Formatted MSI `Filename` string.
#[must_use]
pub fn format_msi_filename(
    short_name: Option<&str>,
    long_name: &str,
    existing: &[String],
) -> String {
    if let Some(sn) = short_name {
        return format!("{sn}|{long_name}");
    }
    if long_name.contains('|') {
        return long_name.to_string();
    }

    let is_invalid_83_char = |c: char| -> bool {
        matches!(
            c,
            '+' | '='
                | '['
                | ']'
                | ';'
                | ','
                | '/'
                | '\\'
                | ':'
                | '*'
                | '?'
                | '"'
                | '<'
                | '>'
                | '|'
                | '.'
                | ' '
        ) || !c.is_ascii()
    };

    let (raw_base, raw_ext) = match long_name.rsplit_once('.') {
        Some((b, e)) if !b.is_empty() => (b, e),
        _ => (long_name, ""),
    };

    let needs_short = long_name.len() > 12
        || raw_base.len() > 8
        || raw_ext.len() > 3
        || raw_base.chars().any(is_invalid_83_char)
        || raw_ext.chars().any(is_invalid_83_char);

    if !needs_short {
        return long_name.to_string();
    }

    let clean_base: String = raw_base
        .chars()
        .filter(|&c| !is_invalid_83_char(c))
        .map(|c| c.to_ascii_uppercase())
        .collect();
    let clean_ext: String = raw_ext
        .chars()
        .filter(|&c| !is_invalid_83_char(c))
        .take(3)
        .map(|c| c.to_ascii_uppercase())
        .collect();

    let base_prefix = if clean_base.is_empty() {
        "FILE".to_string()
    } else if clean_base.len() > 6 {
        clean_base[..6].to_string()
    } else {
        clean_base
    };

    let mut idx = 1usize;
    loop {
        let suffix = format!("~{idx}");
        let max_prefix = 8usize.saturating_sub(suffix.len()).min(base_prefix.len());
        let pfx = &base_prefix[..max_prefix];
        let candidate_short = if clean_ext.is_empty() {
            format!("{pfx}{suffix}")
        } else {
            format!("{pfx}{suffix}.{clean_ext}")
        };
        let collides = existing.iter().any(|ex| {
            let ex_short = ex.split('|').next().unwrap_or(ex);
            ex_short.eq_ignore_ascii_case(&candidate_short)
        });
        if !collides {
            return format!("{candidate_short}|{long_name}");
        }
        idx += 1;
    }
}

/// Helper to parse registry root strings to standard integers (`0` HKCR, `1` HKCU, `2` HKLM, `3` HKU).
///
/// # Arguments
///
/// * `root_str` - The root str value.
///
/// # Returns
///
/// Computed `i16` numeric result or status code.
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
    use crate::wix::extensions::WixExtension;
    use crate::wix::linker::LinkedDatabase;
    use crate::wix::xml::XmlParser;
    use std::fs;

    #[test]
    fn test_compiler_with_extensions_and_util_namespace() {
        let xml = r#"
<Wix xmlns="http://schemas.microsoft.com/wix/2006/wi"
     xmlns:util="http://schemas.microsoft.com/wix/UtilExtension"
     xmlns:unreg="http://schemas.microsoft.com/wix/UnregisteredExtension"
     OtherAttr="ignored">
    <Product Id="{12345678-1234-1234-1234-1234567890AB}" Name="MyApp" Version="2.0.0" Manufacturer="Acme">
        <Package Description="Installer" />
        <Directory Id="TARGETDIR" Name="SourceDir">
            <Component Id="MainComp" Guid="{11111111-1111-1111-1111-111111111111}">
                <util:User Id="usr1" Name="admin" Domain="WORKGROUP" />
                <unreg:UnknownCustomTag Id="c1" />
                <unknownprefix:Tag Id="c2" />
            </Component>
        </Directory>
    </Product>
</Wix>
"#;
        let parser = XmlParser::new();
        let root = parser.parse(xml).unwrap_or_default();

        let mut registry = ExtensionRegistry::new();
        registry.register(std::sync::Arc::new(
            crate::wix::extensions::util::UtilExtension::new(),
        ));
        let compiler = Compiler::with_extensions(registry);
        let obj = compiler.compile(&root).unwrap_or_default();

        assert_eq!(obj.sections.len(), 1);
        let sec = &obj.sections[0];
        assert!(sec.tables.iter().any(|t| t.name == "_util:User"));
    }

    #[test]
    fn test_compiler_basic() {
        let xml = r#"
<Wix xmlns="http://schemas.microsoft.com/wix/2006/wi">
    <Product Id="{12345678-1234-1234-1234-1234567890AB}" Name="MyApp" Version="2.0.0" Manufacturer="Acme">
        <Package Description="Installer" InstallScope="perMachine" InstallPrivileges="limited" />
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
        let root = parser.parse(xml).unwrap_or_default();

        let compiler = Compiler::new();
        let obj = compiler.compile(&root).unwrap_or_default();

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

        // Verify property translation
        let prop_table = sec.tables.iter().find(|t| t.name == "Property");
        assert!(prop_table.is_some());
        let allusers_val = prop_table.and_then(|pt| {
            pt.records
                .iter()
                .find(|r| r.get(0) == Some(&FieldValue::String("ALLUSERS".to_string())))
                .and_then(|r| r.get(1))
        });
        assert_eq!(allusers_val, Some(&FieldValue::String("1".to_string())));

        let privs_val = prop_table.and_then(|pt| {
            pt.records
                .iter()
                .find(|r| r.get(0) == Some(&FieldValue::String("InstallPrivileges".to_string())))
                .and_then(|r| r.get(1))
        });
        assert_eq!(privs_val, Some(&FieldValue::String("limited".to_string())));

        // Verify non-perMachine scope (e.g., perUser) does not emit ALLUSERS=1
        let user_xml = r#"
<Wix xmlns="http://schemas.microsoft.com/wix/2006/wi">
    <Product Id="{12345678-1234-1234-1234-1234567890AC}" Name="UserApp" Version="1.0.0" Manufacturer="Acme">
        <Package Description="UserInstaller" InstallScope="perUser" />
        <Directory Id="TARGETDIR" Name="SourceDir" />
    </Product>
</Wix>"#;
        let user_parser = XmlParser::new();
        let user_node = user_parser.parse(user_xml).unwrap_or_default();
        let user_compiler = Compiler::new();
        let user_obj = user_compiler.compile(&user_node).unwrap_or_default();
        let user_sec = &user_obj.sections[0];
        let has_allusers = user_sec
            .tables
            .iter()
            .find(|t| t.name == "Property")
            .is_some_and(|up| {
                up.records
                    .iter()
                    .any(|r| r.get(0) == Some(&FieldValue::String("ALLUSERS".to_string())))
            });
        assert!(!has_allusers);
    }

    #[test]
    fn test_compiler_sections_variety() {
        let xml = r#"
<Wix xmlns="http://schemas.microsoft.com/wix/2006/wi">
    <Module Id="Mod1" Version="1.0" />
    <Fragment Id="Frag1" />
    <PatchCreation Id="PC1" />
    <Patch Id="P1" />
</Wix>
"#;
        let parser = XmlParser::new();
        let root = parser.parse(xml).unwrap_or_default();
        let compiler = Compiler::new();
        let obj = compiler.compile(&root).unwrap_or_default();

        assert_eq!(obj.sections.len(), 4);
        assert_eq!(obj.sections[0].section_type, SectionType::Module);
        assert_eq!(obj.sections[1].section_type, SectionType::Fragment);
        assert_eq!(obj.sections[2].section_type, SectionType::PatchCreation);
        assert_eq!(obj.sections[3].section_type, SectionType::Patch);
    }

    #[test]
    fn test_compiler_errors() {
        let compiler = Compiler::new();
        let parser = XmlParser::new();

        // Missing Directory Id
        let bad_dir = parser.parse("<Wix><Product Id=\"{11111111-1111-1111-1111-111111111111}\"><Directory /></Product></Wix>").unwrap_or_default();
        assert!(compiler.compile(&bad_dir).is_err());

        // Missing Component Id
        let bad_comp = parser.parse("<Wix><Product Id=\"{11111111-1111-1111-1111-111111111111}\"><Component /></Product></Wix>").unwrap_or_default();
        assert!(compiler.compile(&bad_comp).is_err());

        // Missing File Id
        let bad_file = parser.parse(
            "<Wix><Product Id=\"{11111111-1111-1111-1111-111111111111}\"><File /></Product></Wix>",
        ).unwrap_or_default();
        assert!(compiler.compile(&bad_file).is_err());

        // Missing Feature Id
        let bad_feat = parser.parse("<Wix><Product Id=\"{11111111-1111-1111-1111-111111111111}\"><Feature /></Product></Wix>").unwrap_or_default();
        assert!(compiler.compile(&bad_feat).is_err());

        // Missing Property Id
        let bad_prop = parser.parse("<Wix><Product Id=\"{11111111-1111-1111-1111-111111111111}\"><Property /></Product></Wix>").unwrap_or_default();
        assert!(compiler.compile(&bad_prop).is_err());
    }

    #[allow(clippy::cognitive_complexity)]
    #[test]
    fn test_compiler_extended_elements() {
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
        let root = parser.parse(xml).unwrap_or_default();
        let compiler = Compiler::new();
        let obj = compiler.compile(&root).unwrap_or_default();

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
    }

    #[allow(clippy::cognitive_complexity)]
    #[test]
    #[allow(clippy::too_many_lines)]
    fn test_compiler_wix_section11_features() {
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
        let root = parser.parse(xml).unwrap_or_default();
        let compiler = Compiler::new();
        let obj = compiler.compile(&root).unwrap_or_default();

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
                for parsed_comp in into_vec(ComponentRow::from_record(&t.records[0])) {
                    assert!(parsed_comp.component_id.is_some());
                    assert_ne!(parsed_comp.attributes, 0);
                    assert_eq!(parsed_comp.condition, Some("VersionNT >= 600".to_string()));
                    found_comp = true;
                }
            } else if t.name == "File" {
                for parsed_file in into_vec(FileRow::from_record(&t.records[0])) {
                    assert_eq!(parsed_file.file_size, 4096);
                    assert_eq!(parsed_file.version, Some("1.2.3.4".to_string()));
                    assert_eq!(parsed_file.language, Some("1033".to_string()));
                    assert_ne!(parsed_file.attributes, Some(0));
                    found_file = true;
                }
            }
        }

        assert!(found_create_folder);
        assert!(found_remove_file);
        assert!(found_env);
        assert!(found_comp);
        assert!(found_file);
    }

    /// Tests compiling module attributes, signatures, dependencies, exclusions, and configurations.
    #[test]
    fn test_compiler_module_attributes() {
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
        let mod_root = parser.parse(mod_xml).unwrap_or_default();
        let mod_obj = compiler.compile(&mod_root).unwrap_or_default();
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
    }

    struct FailingMockExtension;
    impl WixExtension for FailingMockExtension {
        fn id(&self) -> &'static str {
            "FailingMockExtension"
        }
        fn supported_namespaces(&self) -> &[&'static str] {
            &["http://fail"]
        }
        fn compile_node(
            &self,
            _node: &XmlNode,
            _parent_id: Option<&str>,
            _section: &mut IntermediateSection,
            _tables: &mut HashMap<String, IntermediateTable>,
        ) -> Result<()> {
            Err(MsiError::WixExtension {
                extension: "FailingMockExtension".to_string(),
                message: "fail".to_string(),
            })
        }
        fn link_database(&self, _db: &mut LinkedDatabase) -> Result<()> {
            Ok(())
        }
    }

    #[test]
    fn test_compiler_extension_failure() {
        let xml = r#"
<Wix xmlns="http://schemas.microsoft.com/wix/2006/wi" xmlns:fail="http://fail">
    <Product Id="*" Name="P" Version="1" Language="1033" Manufacturer="M" UpgradeCode="12345678-1234-1234-1234-123456789012">
        <fail:Element />
    </Product>
</Wix>
"#;
        let parser = XmlParser::new();
        let root = parser.parse(xml).unwrap_or_default();
        let mut registry = ExtensionRegistry::new();
        registry.register(std::sync::Arc::new(FailingMockExtension));
        let compiler = Compiler::with_extensions(registry);
        assert!(compiler.compile(&root).is_err());

        // Cover the unused link_database
        let ext = FailingMockExtension;
        let mut db = LinkedDatabase::default();
        let _ = ext.link_database(&mut db);
    }

    #[test]
    fn test_compiler_unregistered_prefix_failure() {
        let xml = r#"
<Wix xmlns="http://schemas.microsoft.com/wix/2006/wi" xmlns:unknown="http://unknown">
    <Product Id="*" Name="P" Version="1" Language="1033" Manufacturer="M" UpgradeCode="12345678-1234-1234-1234-123456789012">
        <unknown:Element>
            <Component />
        </unknown:Element>
    </Product>
</Wix>
"#;
        let parser = XmlParser::new();
        let root = parser.parse(xml).unwrap_or_default();
        let compiler = Compiler::new();
        assert!(compiler.compile(&root).is_err());
    }

    #[allow(clippy::cognitive_complexity)]
    #[test]
    #[allow(clippy::too_many_lines)]
    fn test_compiler_wix_section11_extended_tags() {
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
        <Binary Id="BinNoSrc" />
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
        let root = parser.parse(xml).unwrap_or_default();
        let obj = compiler.compile(&root).unwrap_or_default();
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
        let mod_root = parser.parse(mod_xml).unwrap_or_default();
        let mod_obj = compiler.compile(&mod_root).unwrap_or_default();
        let mod_sec = &mod_obj.sections[0];
        let mod_table_names: Vec<&str> = mod_sec.tables.iter().map(|t| t.name.as_str()).collect();
        assert!(mod_table_names.contains(&"ModuleConfiguration"));
        assert!(mod_table_names.contains(&"ModuleSubstitution"));
        assert!(mod_table_names.contains(&"ModuleIgnoreModularization"));
    }

    /// Tests compiling top-level Wix root variations and section attributes.
    ///
    /// # Errors
    ///
    /// Returns [`crate::MsiError`] if XML parsing or compilation fails.
    #[test]
    fn test_compiler_top_level_and_section_variations() {
        let parser = XmlParser::new();
        let compiler = Compiler::new();

        // 1. Root with unknown tag
        let xml_unknown = r#"
<Wix xmlns="http://schemas.microsoft.com/wix/2006/wi">
    <UnknownRootChild Tag="Ignored" />
</Wix>
"#;
        let root_unknown = parser.parse(xml_unknown).unwrap_or_default();
        let obj_unknown = compiler.compile(&root_unknown).unwrap_or_default();
        assert_eq!(obj_unknown.sections.len(), 0);

        // 2. Minimal Product without attributes
        let xml_min_prod = r#"
<Wix xmlns="http://schemas.microsoft.com/wix/2006/wi">
    <Product />
</Wix>
"#;
        let root_min_prod = parser.parse(xml_min_prod).unwrap_or_default();
        let obj_min_prod = compiler.compile(&root_min_prod).unwrap_or_default();
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
        let root_full_prod = parser.parse(xml_full_prod).unwrap_or_default();
        let obj_full_prod = compiler.compile(&root_full_prod).unwrap_or_default();
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
        let root_alt_prod = parser.parse(xml_alt_prod).unwrap_or_default();
        let obj_alt_prod = compiler.compile(&root_alt_prod).unwrap_or_default();
        assert_eq!(obj_alt_prod.sections.len(), 1);

        // 5. Minimal Module (all attributes None)
        let xml_min_mod = r#"
<Wix xmlns="http://schemas.microsoft.com/wix/2006/wi">
    <Module />
</Wix>
"#;
        let root_min_mod = parser.parse(xml_min_mod).unwrap_or_default();
        let obj_min_mod = compiler.compile(&root_min_mod).unwrap_or_default();
        assert_eq!(obj_min_mod.sections.len(), 1);

        // 6. Module with Guid and SummaryCodepage
        let xml_alt_mod = r#"
<Wix xmlns="http://schemas.microsoft.com/wix/2006/wi">
    <Module Guid="{55555555-5555-5555-5555-555555555555}" SummaryCodepage="1252" />
</Wix>
"#;
        let root_alt_mod = parser.parse(xml_alt_mod).unwrap_or_default();
        let obj_alt_mod = compiler.compile(&root_alt_mod).unwrap_or_default();
        assert_eq!(obj_alt_mod.sections.len(), 1);
    }

    /// Tests compiling features, components, and files with various attribute branches.
    ///
    /// # Errors
    ///
    /// Returns [`crate::MsiError`] if XML parsing or compilation fails.
    #[test]
    fn test_compiler_features_and_components_branches() {
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
        let root = parser.parse(&xml).unwrap_or_default();
        let obj = compiler.compile(&root).unwrap_or_default();
        assert_eq!(obj.sections.len(), 1);
    }

    /// Tests custom action execution modes, return modes, and types.
    ///
    /// # Errors
    ///
    /// Returns [`crate::MsiError`] if XML parsing or compilation fails.
    #[test]
    fn test_compiler_custom_actions_all_branches() {
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
        let root = parser.parse(xml).unwrap_or_default();
        let obj = compiler.compile(&root).unwrap_or_default();
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
    }

    /// Tests locator search types nested under components and products.
    ///
    /// # Errors
    ///
    /// Returns [`crate::MsiError`] if XML parsing or compilation fails.
    #[test]
    fn test_compiler_searches_nested_and_types() {
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
        let root = parser.parse(xml).unwrap_or_default();
        let obj = compiler.compile(&root).unwrap_or_default();
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
        let obj_ini = compiler
            .compile(&parser.parse(xml_ini_alone).unwrap_or_default())
            .unwrap_or_default();
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
        let obj_comp = compiler
            .compile(&parser.parse(xml_comp_alone).unwrap_or_default())
            .unwrap_or_default();
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
        let obj_fs = compiler
            .compile(&parser.parse(xml_fs_alone).unwrap_or_default())
            .unwrap_or_default();
        assert_eq!(obj_fs.sections.len(), 1);
    }

    /// Tests UI publish, control condition, and subscribe elements.
    ///
    /// # Errors
    ///
    /// Returns [`crate::MsiError`] if XML parsing or compilation fails.
    #[test]
    fn test_compiler_ui_controls_publish_and_events() {
        let parser = XmlParser::new();
        let compiler = Compiler::new();

        let xml = r#"
<Wix xmlns="http://schemas.microsoft.com/wix/2006/wi">
    <Product Id="{11111111-1111-1111-1111-111111111111}" Name="App" Version="1.0.0" Manufacturer="Acme">
        <UI>
            <Publish Dialog="Dlg1" Control="BtnNext" Event="NewDialog" Value="Dlg2" Order="1">NOT Installed</Publish>
            <Publish Dialog="Dlg1" Control="BtnCancel" Event="EndDialog" Argument="Exit" />
            <Publish Dialog="Dlg1" Control="BtnNext" Property="CUSTOM_PROP" Value="VAL">1</Publish>
            <ControlCondition Dialog="Dlg1" Control="BtnNext" Action="disable" Condition="VersionNT &lt; 600" />
            <ControlCondition Dialog="Dlg1" Control="BtnNext" Action="hide">VersionNT &lt; 500</ControlCondition>
            <ControlCondition Dialog="Dlg1" Control="BtnNext" Action="enable" />
            <Subscribe Dialog="Dlg1" Control="Prog" Event="SetProgress" Attribute="Progress" />
            <Subscribe />
            <Dialog Id="Dlg1" Width="300" Height="200" Title="Dlg1">
                <Control Id="BtnNext" Type="PushButton" X="10" Y="10" Width="50" Height="20">
                    <Publish Event="DoAction" Value="CA1">1</Publish>
                    <Publish Property="_BrowseProperty" Value="INSTALLFOLDER">1</Publish>
                    <Condition Action="enable" />
                    <UnknownControlChild />
                </Control>
                <Control Id="TxtPass" Type="Edit" X="10" Y="40" Width="100" Height="20" Hidden="yes" Disabled="yes" Multiline="yes" Password="yes" NoPrefix="yes" Cancel="yes" Sunken="yes" Default="yes" />
                <Control Id="TxtNorm" Type="Edit" X="10" Y="70" Width="100" Height="20" Hidden="no" Disabled="no" Multiline="no" Password="no" NoPrefix="no" Cancel="no" Sunken="no" Default="no" />
            </Dialog>
        </UI>
        <Directory Id="TARGETDIR" Name="SourceDir">
            <Directory Id="INSTALLFOLDER" Name="App">
                <Component Id="CmpGuidQuestion" Guid="?"><CreateFolder /></Component>
                <Component Id="CmpGuidStar" Guid="*"><CreateFolder /></Component>
                <Component Id="CmpGuidEmpty" Guid=""><CreateFolder /></Component>
                <Component Id="CmpGuidNone"><CreateFolder /></Component>
            </Directory>
        </Directory>
    </Product>
</Wix>
"#;
        let root = parser.parse(xml).unwrap_or_default();
        let obj = compiler.compile(&root).unwrap_or_default();
        assert_eq!(obj.sections.len(), 1);
        let sec = &obj.sections[0];
        let mut found_control_event = false;
        let mut found_control_condition = false;
        let mut found_event_mapping = false;
        for t in &sec.tables {
            if t.name == "ControlEvent" {
                assert_eq!(t.records.len(), 5);
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
    }

    /// Tests component groups, package groups, sequences, and helper methods.
    ///
    /// # Errors
    ///
    /// Returns [`crate::MsiError`] if XML parsing or compilation fails.
    #[test]
    #[allow(clippy::too_many_lines)]
    fn test_compiler_groups_sequences_and_helpers() {
        let parser = XmlParser::new();
        let compiler = Compiler::new();

        let xml = r#"
<Wix xmlns="http://schemas.microsoft.com/wix/2006/wi" xmlns:mockext="http://schemas.example.com/mockext">
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
        <mockext:TopLevel Target="Everywhere" />
        <mockext:ValueElem Value="Val1" />
        <mockext:Empty />
        <Upgrade Id="{11111111-2222-3333-4444-555555555555}">
            <UnknownInUpgrade />
        </Upgrade>
        <RegistryKey Root="HKLM" Key="Software\Test">
            <UnknownInRegKey />
        </RegistryKey>
    </Product>
</Wix>
"#;
        let root = parser.parse(xml).unwrap_or_default();
        let obj = compiler.compile(&root).unwrap_or_default();
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
        let obj_frag = compiler
            .compile(&parser.parse(xml_fragments).unwrap_or_default())
            .unwrap_or_default();
        assert_eq!(obj_frag.sections.len(), 4);

        // Verify direct call to compile_posix_element with unknown tag
        let dummy_node = parser.parse("<posix:UnknownTag />").unwrap_or_default();
        let mut dummy_sec = IntermediateSection::new(SectionType::Product, None);
        let mut dummy_tbls = HashMap::new();
        elements::compile_posix_element(&dummy_node, None, &mut dummy_sec, &mut dummy_tbls);

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
    }

    /// Tests compiler error handling for elements missing mandatory Id attributes.
    ///
    /// # Errors
    ///
    /// Returns [`crate::MsiError`] if XML parsing fails unexpectedly.
    #[test]
    fn test_compiler_all_missing_id_errors() {
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
            "<Wix><Product Id=\"{11111111-1111-1111-1111-111111111111}\"><WixVariable /></Product></Wix>",
            "<Wix><Product Id=\"{11111111-1111-1111-1111-111111111111}\"><UI><Dialog /></UI></Product></Wix>",
            "<Wix><Product Id=\"{11111111-1111-1111-1111-111111111111}\"><UI><Dialog Id=\"D1\"><Control /></Dialog></UI></Product></Wix>",
        ];

        for xml in test_cases {
            let root = parser.parse(xml).unwrap_or_default();
            assert!(compiler.compile(&root).is_err());
        }
    }

    /// Tests libscript `WiX` feature parity in compiler: multi-cab media, `DiskId` on files,
    /// `WixVariable`, nested Property searches, `RadioButtonGroup`/`RadioButton`, `ScrollableText`,
    /// VBScript/JScript custom actions, relative sequencing, and `MajorUpgrade` attributes.
    ///
    /// # Errors
    ///
    /// Returns [`crate::MsiError`] if XML parsing or compilation fails.
    #[allow(clippy::cognitive_complexity)]
    #[test]
    #[allow(clippy::too_many_lines)]
    #[allow(clippy::similar_names)]
    fn test_compiler_libscript_parity_features() {
        let parser = XmlParser::new();
        let compiler = Compiler::new();

        let xml = r##"
<Wix xmlns="http://schemas.microsoft.com/wix/2006/wi">
    <Product Id="{11111111-2222-3333-4444-555555555555}" Name="ParityApp" Version="2.0.0" Manufacturer="TestCorp" UpgradeCode="{AAAAAAAA-BBBB-CCCC-DDDD-EEEEEEEEEEEE}">
        <Package Description="Parity Test" />
        <MajorUpgrade DowngradeErrorMessage="A newer version is installed." Schedule="afterInstallExecute" AllowSameVersionUpgrades="yes" />
        <Media Id="1" Cabinet="engine.cab" EmbedCab="yes" DiskPrompt="Disk 1" VolumeLabel="VOL1" Source="SRC1" />
        <Media Id="2" Cabinet="#runtimes.cab" EmbedCab="yes" />
        <WixVariable Id="WixUIBannerBmp" Value="banner.bmp" Overridable="yes" />
        <WixVariable Id="WixUILicenseRtf">{\rtf1 Inline EULA}</WixVariable>

        <Property Id="FOUND_PYTHON_EXE">
            <RegistrySearch Id="SearchPy64" Root="HKLM" Key="SOFTWARE\Python\PythonCore\3.12\InstallPath" Type="raw" Win64="yes">
                <FileSearch Id="SearchPyExe64" Name="python.exe" MinVersion="3.12.0" MaxVersion="3.13.0" />
            </RegistrySearch>
        </Property>

        <Binary Id="Bin_Val_mysql" SourceFile="validate_mysql.vbs" />
        <Binary Id="Bin_Val_js" SourceFile="validate.js" />
        <CustomAction Id="CA_Val_mysql" BinaryKey="Bin_Val_mysql" VBScriptCall="CheckPorts_mysql" Return="check" />
        <CustomAction Id="CA_Val_js" BinaryKey="Bin_Val_js" JScriptCall="CheckPorts_js" Return="ignore" />

        <Directory Id="TARGETDIR" Name="SourceDir">
            <Directory Id="ProgramFilesFolder" Name="PFiles">
                <Directory Id="INSTALLDIR" Name="App">
                    <Component Id="C_Engine" Guid="{11111111-1111-1111-1111-111111111111}">
                        <File Id="F_Engine" Source="engine.exe" DiskId="1" KeyPath="yes" />
                    </Component>
                    <Component Id="C_Runtime" Guid="{22222222-2222-2222-2222-222222222222}">
                        <File Id="F_Runtime" Source="python.dll" DiskId="2" KeyPath="yes" />
                    </Component>
                </Directory>
            </Directory>
        </Directory>

        <UI Id="CustomUI">
            <Dialog Id="Dlg_Mode" Width="370" Height="270" Title="Setup Mode">
                <Control Id="ModeRadio" Type="RadioButtonGroup" X="20" Y="50" Width="300" Height="60" Property="SETUP_MODE">
                    <RadioButtonGroup Property="SETUP_MODE">
                        <RadioButton Value="Simple" X="0" Y="0" Width="280" Height="14" Text="Simple (Express)" />
                        <RadioButton Value="Advanced" X="0" Y="20" Width="280" Height="14" Text="Advanced (Custom)" Help="Choose features" />
                    </RadioButtonGroup>
                </Control>
                <Control Id="DirectRadio" Type="RadioButtonGroup" X="20" Y="120" Width="300" Height="40" Property="SUB_MODE">
                    <RadioButton Value="Val1" Order="1" X="0" Y="0" Width="280" Height="14" Text="Option 1" />
                    <RadioButton Value="Val2" Order="2" X="0" Y="20" Width="280" Height="14" Text="Option 2" />
                </Control>
                <Control Id="LicenseBox" Type="ScrollableText" X="20" Y="170" Width="300" Height="60" Sunken="yes">
                    <Text>{\rtf1 EULA content here}</Text>
                </Control>
            </Dialog>
        </UI>

        <InstallUISequence>
            <Show Dialog="Dlg_Mode" After="CostFinalize">NOT Installed</Show>
            <Show Dialog="Dlg_Exit" OnExit="success">NOT Installed</Show>
        </InstallUISequence>

        <InstallExecuteSequence>
            <Custom Action="CA_Val_mysql" Before="InstallInitialize">NOT Installed</Custom>
            <Custom Action="CA_Val_js" After="InstallInitialize">NOT Installed</Custom>
        </InstallExecuteSequence>
    </Product>
</Wix>
"##;

        let root = parser.parse(xml).unwrap_or_default();
        let obj = compiler.compile(&root).unwrap_or_default();
        let sec = &obj.sections[0];

        let def_tbl = IntermediateTable::new("Default");

        // 1. Verify WixFile table contains DiskId
        let wix_file_tbl = sec
            .tables
            .iter()
            .find(|t| t.name == "WixFile")
            .unwrap_or(&def_tbl);
        assert_eq!(wix_file_tbl.name, "WixFile");
        let wf_records = &wix_file_tbl.records;
        assert_eq!(wf_records.len(), 2);
        assert_eq!(wf_records[0].get(2), Some(&FieldValue::Short(1)));
        assert_eq!(wf_records[1].get(2), Some(&FieldValue::Short(2)));

        // 2. Verify Media records (embedding prefix #, DiskPrompt, VolumeLabel, Source)
        let media_tbl = sec
            .tables
            .iter()
            .find(|t| t.name == "Media")
            .unwrap_or(&def_tbl);
        assert_eq!(media_tbl.name, "Media");
        let m_records = &media_tbl.records;
        assert_eq!(m_records.len(), 2);
        assert_eq!(
            m_records[0].get(3),
            Some(&FieldValue::String("#engine.cab".to_string()))
        );
        assert_eq!(
            m_records[0].get(2),
            Some(&FieldValue::String("Disk 1".to_string()))
        );
        assert_eq!(
            m_records[0].get(4),
            Some(&FieldValue::String("VOL1".to_string()))
        );
        assert_eq!(
            m_records[0].get(5),
            Some(&FieldValue::String("SRC1".to_string()))
        );
        assert_eq!(
            m_records[1].get(3),
            Some(&FieldValue::String("#runtimes.cab".to_string()))
        );

        // 3. Verify WixVariable table
        let wix_var_tbl = sec
            .tables
            .iter()
            .find(|t| t.name == "WixVariable")
            .unwrap_or(&def_tbl);
        assert_eq!(wix_var_tbl.name, "WixVariable");
        let wv_records = &wix_var_tbl.records;
        assert_eq!(wv_records.len(), 2);
        assert_eq!(
            wv_records[0].get(0),
            Some(&FieldValue::String("WixUIBannerBmp".to_string()))
        );
        assert_eq!(
            wv_records[0].get(1),
            Some(&FieldValue::String("banner.bmp".to_string()))
        );
        assert_eq!(wv_records[0].get(2), Some(&FieldValue::Short(1))); // overridable = 1

        // 4. Verify nested Property -> RegistrySearch -> FileSearch
        let sig_tbl = sec.tables.iter().find(|t| t.name == "Signature");
        assert!(sig_tbl.is_some());
        let reg_loc_tbl = sec.tables.iter().find(|t| t.name == "RegLocator");
        assert!(reg_loc_tbl.is_some());
        let app_search_tbl = sec.tables.iter().find(|t| t.name == "AppSearch");
        assert!(app_search_tbl.is_some());

        // 5. Verify CustomAction Type 6 (VBScript) and Type 5 (JScript)
        let ca_tbl = sec
            .tables
            .iter()
            .find(|t| t.name == "CustomAction")
            .unwrap_or(&def_tbl);
        assert_eq!(ca_tbl.name, "CustomAction");
        let ca_records = &ca_tbl.records;
        assert_eq!(
            ca_records[0].get(0),
            Some(&FieldValue::String("CA_Val_mysql".to_string()))
        );
        assert_eq!(ca_records[0].get(1), Some(&FieldValue::Short(6))); // Type 6 VBScript
        assert_eq!(
            ca_records[1].get(0),
            Some(&FieldValue::String("CA_Val_js".to_string()))
        );
        assert_eq!(ca_records[1].get(1), Some(&FieldValue::Short(5 | 0x0040))); // Type 5 JScript + Return="ignore"

        // 6. Verify RadioButton table
        let rb_tbl = sec
            .tables
            .iter()
            .find(|t| t.name == "RadioButton")
            .unwrap_or(&def_tbl);
        assert_eq!(rb_tbl.name, "RadioButton");
        let rb_records = &rb_tbl.records;
        assert_eq!(rb_records.len(), 4);
        assert_eq!(
            rb_records[0].get(0),
            Some(&FieldValue::String("SETUP_MODE".to_string()))
        );
        assert_eq!(
            rb_records[0].get(2),
            Some(&FieldValue::String("Simple".to_string()))
        );
        assert_eq!(
            rb_records[1].get(2),
            Some(&FieldValue::String("Advanced".to_string()))
        );
        assert_eq!(
            rb_records[1].get(8),
            Some(&FieldValue::String("Choose features".to_string()))
        );
        assert_eq!(
            rb_records[2].get(0),
            Some(&FieldValue::String("SUB_MODE".to_string()))
        );
        assert_eq!(
            rb_records[2].get(2),
            Some(&FieldValue::String("Val1".to_string()))
        );

        // 7. Verify ScrollableText resolved text
        let ctrl_tbl = sec
            .tables
            .iter()
            .find(|t| t.name == "Control")
            .unwrap_or(&def_tbl);
        assert_eq!(ctrl_tbl.name, "Control");
        let ctrl_records = &ctrl_tbl.records;
        let license_rec = ctrl_records
            .iter()
            .find(|r| r.get(1) == Some(&FieldValue::String("LicenseBox".to_string())));
        assert!(license_rec.is_some());
        assert_eq!(
            license_rec.and_then(|r| r.get(9)),
            Some(&FieldValue::String(
                r"{\rtf1 EULA content here}".to_string()
            ))
        );

        // 8. Verify Relative Sequence Table (_WixSequenceRelative)
        let rel_tbl = sec
            .tables
            .iter()
            .find(|t| t.name == "_WixSequenceRelative")
            .unwrap_or(&def_tbl);
        assert_eq!(rel_tbl.name, "_WixSequenceRelative");
        let rel_records = &rel_tbl.records;
        assert_eq!(rel_records.len(), 4);

        // 9. Verify MajorUpgrade records
        let upg_tbl = sec.tables.iter().find(|t| t.name == "Upgrade");
        assert!(upg_tbl.is_some());
        let lc_tbl = sec.tables.iter().find(|t| t.name == "LaunchCondition");
        assert!(lc_tbl.is_some());
        let prop_tbl = sec
            .tables
            .iter()
            .find(|t| t.name == "Property")
            .unwrap_or(&def_tbl);
        assert_eq!(prop_tbl.name, "Property");
        let p_records = &prop_tbl.records;
        let sec_prop = p_records
            .iter()
            .find(|r| r.get(0) == Some(&FieldValue::String("SecureCustomProperties".to_string())));
        assert!(sec_prop.is_some());
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

    /// Tests compiling system, shell, and COM metadata elements including `Shortcut`, `SymbolicLink`,
    /// `IniFile`, `Icon`, `ProgId`, `MIME`, `Class`, `AppId`, `Error`, and control text resolution.
    ///
    /// # Errors
    ///
    /// Returns [`crate::MsiError`] if XML parsing or compilation fails.
    #[test]
    #[allow(clippy::too_many_lines)]
    fn test_compiler_system_and_com_elements() {
        let temp_dir = std::env::temp_dir().join("msi_test_compiler_system_elements");
        let _ = fs::remove_dir_all(&temp_dir);
        assert!(fs::create_dir_all(&temp_dir).is_ok());

        let rtf_path = temp_dir.join("license.rtf");
        assert!(fs::write(&rtf_path, "{\\rtf1 RTF sample}").is_ok());
        let txt_path = temp_dir.join("license.txt");
        assert!(fs::write(&txt_path, "Plain text sample").is_ok());

        let rtf_str = rtf_path.to_str().unwrap_or("license.rtf");
        let txt_str = txt_path.to_str().unwrap_or("license.txt");

        let xml = format!(
            r#"
<Wix xmlns="http://schemas.microsoft.com/wix/2006/wi">
    <Product Id="{{11111111-2222-3333-4444-555555555555}}" Name="ComApp" Version="1.0.0" Manufacturer="Vendor" UpgradeCode="{{AAAAAAAA-BBBB-CCCC-DDDD-EEEEEEEEEEEE}}">
        <Package Description="COM Test" />
        <MajorUpgrade Schedule="afterInstallValidate" DowngradeErrorMessage="Downgrade forbidden" />
        <Media Id="1" Cabinet="app.cab" EmbedCab="yes" />
        <Media Id="3" EmbedCab="yes" />

        <Directory Id="TARGETDIR" Name="SourceDir">
            <Directory Id="INSTALLDIR" Name="App">
                <Component Id="C_Main" Guid="{{11111111-1111-1111-1111-111111111111}}">
                    <File Id="F_Main" Source="app.exe" KeyPath="yes" />
                    <Shortcut Id="SC_Main" Name="App Shortcut" Target="[#F_Main]" Directory="ProgramMenuFolder" Description="App Description" Arguments="--test" Icon="AppIcon" />
                    <SymbolicLink Id="Sym_Main" Name="app_link" Target="app.exe" Directory="INSTALLDIR" />
                    <IniFile Id="Ini_1" Name="app.ini" Section="Config" Key="Created" Value="1" Action="createLine" Directory="INSTALLDIR" />
                    <IniFile Id="Ini_2" Name="app.ini" Section="Config" Key="Replaced" Value="2" Action="replaceLine" />
                    <IniFile Id="Ini_3" Name="app.ini" Section="Config" Key="Tag" Value="3" Action="addTag" />
                    <IniFile Id="Ini_4" Name="app.ini" Section="Config" Key="Tag" Value="4" Action="removeTag" />
                    <IniFile Id="Ini_5" Name="app.ini" Section="Config" Key="Other" Value="5" Action="other" />
                    <Class Id="{{22222222-2222-2222-2222-222222222222}}" Context="InprocServer32" Description="Main Class" ProgId="App.Doc" AppId="{{33333333-3333-3333-3333-333333333333}}" />
                </Component>
            </Directory>
        </Directory>

        <Icon Id="AppIcon" />
        <ProgId Id="App.Doc" Description="App Document" Icon="AppIcon" IconIndex="0" />
        <MIME ContentType="application/x-app" DefaultExtension=".app" CLSID="{{22222222-2222-2222-2222-222222222222}}" />
        <AppId Id="{{33333333-3333-3333-3333-333333333333}}" RemoteServerName="server1" LocalService="svc1" />
        <Error Id="1001" Message="Fatal Error 1001" />
        <Error Id="1002">Inline message 1002</Error>
        <Property Id="EMPTY_PROP" Value="" />
        <RegistrySearch Id="TopLevelSearch" Root="HKLM" Key="Software\App">
            <DirectorySearch Id="NestedDir" />
        </RegistrySearch>

        <UI Id="DlgUI">
            <Dialog Id="Dlg_Text" Width="300" Height="200" Title="Dialog">
                <Control Id="C_Rtf" Type="ScrollableText" X="0" Y="0" Width="100" Height="50">
                    <Text SourceFile="{rtf_str}" />
                </Control>
                <Control Id="C_Txt" Type="ScrollableText" X="0" Y="50" Width="100" Height="50">
                    <Text SourceFile="{txt_str}" />
                </Control>
                <Control Id="C_Missing" Type="ScrollableText" X="0" Y="100" Width="100" Height="50">
                    <Text SourceFile="/nonexistent_source_file_9999.txt" />
                </Control>
                <Control Id="C_Empty" Type="ScrollableText" X="0" Y="150" Width="100" Height="20">
                    <Text></Text>
                </Control>
                <Control Id="C_Direct_Inner" Type="RadioButtonGroup" X="0" Y="170" Width="100" Height="20" Property="PROP_DIRECT">
                    <RadioButton Value="V2" Help="Help Tip">Direct text inside</RadioButton>
                </Control>
                <Control Id="C_Rbg_PropFallback" Type="RadioButtonGroup" X="0" Y="190" Width="100" Height="20" Property="PROP_FROM_CTRL">
                    <RadioButtonGroup>
                        <RadioButton Value="V1">Text From Body</RadioButton>
                    </RadioButtonGroup>
                </Control>
            </Dialog>
        </UI>

        <InstallExecuteSequence>
            <Custom Action="CA_Exit" OnExit="success">NOT Installed</Custom>
        </InstallExecuteSequence>
    </Product>
</Wix>
"#
        );

        let parser = XmlParser::new();
        let compiler = Compiler::new();
        let root = parser.parse(&xml).unwrap_or_default();
        let obj = compiler.compile(&root).unwrap_or_default();
        let sec = &obj.sections[0];

        let def_tbl = IntermediateTable::new("Default");

        let sc_tbl = sec
            .tables
            .iter()
            .find(|t| t.name == "Shortcut")
            .unwrap_or(&def_tbl);
        assert_eq!(sc_tbl.name, "Shortcut");
        assert_eq!(sc_tbl.records.len(), 1);

        let sym_tbl = sec
            .tables
            .iter()
            .find(|t| t.name == "PosixSymlink")
            .unwrap_or(&def_tbl);
        assert_eq!(sym_tbl.name, "PosixSymlink");
        assert_eq!(sym_tbl.records.len(), 1);

        let ini_tbl = sec
            .tables
            .iter()
            .find(|t| t.name == "IniFile")
            .unwrap_or(&def_tbl);
        assert_eq!(ini_tbl.name, "IniFile");
        assert_eq!(ini_tbl.records.len(), 5);

        let icon_tbl = sec
            .tables
            .iter()
            .find(|t| t.name == "Icon")
            .unwrap_or(&def_tbl);
        assert_eq!(icon_tbl.name, "Icon");
        assert_eq!(icon_tbl.records.len(), 1);

        let prog_tbl = sec
            .tables
            .iter()
            .find(|t| t.name == "ProgId")
            .unwrap_or(&def_tbl);
        assert_eq!(prog_tbl.name, "ProgId");
        assert_eq!(prog_tbl.records.len(), 1);

        let mime_tbl = sec
            .tables
            .iter()
            .find(|t| t.name == "MIME")
            .unwrap_or(&def_tbl);
        assert_eq!(mime_tbl.name, "MIME");
        assert_eq!(mime_tbl.records.len(), 1);

        let class_tbl = sec
            .tables
            .iter()
            .find(|t| t.name == "Class")
            .unwrap_or(&def_tbl);
        assert_eq!(class_tbl.name, "Class");
        assert_eq!(class_tbl.records.len(), 1);

        let app_tbl = sec
            .tables
            .iter()
            .find(|t| t.name == "AppId")
            .unwrap_or(&def_tbl);
        assert_eq!(app_tbl.name, "AppId");
        assert_eq!(app_tbl.records.len(), 1);

        let err_tbl = sec
            .tables
            .iter()
            .find(|t| t.name == "Error")
            .unwrap_or(&def_tbl);
        assert_eq!(err_tbl.name, "Error");
        assert_eq!(err_tbl.records.len(), 2);

        // Also test MajorUpgrade with afterInstallFinalize and other schedules
        let xml_upg = r#"
<Wix xmlns="http://schemas.microsoft.com/wix/2006/wi">
    <Product Id="{11111111-2222-3333-4444-555555555555}" Name="UpgApp" Version="1.0.0" Manufacturer="Vendor" UpgradeCode="{AAAAAAAA-BBBB-CCCC-DDDD-EEEEEEEEEEEE}">
        <Package Description="Upg Test" />
        <MajorUpgrade Schedule="afterInstallFinalize" />
    </Product>
</Wix>
"#;
        let root_upg = parser.parse(xml_upg).unwrap_or_default();
        assert!(compiler.compile(&root_upg).is_ok());

        let xml_upg_other = r#"
<Wix xmlns="http://schemas.microsoft.com/wix/2006/wi">
    <Product Id="{11111111-2222-3333-4444-555555555555}" Name="UpgApp" Version="1.0.0" Manufacturer="Vendor" UpgradeCode="{AAAAAAAA-BBBB-CCCC-DDDD-EEEEEEEEEEEE}">
        <Package Description="Upg Test" />
        <MajorUpgrade Schedule="otherSchedule" />
    </Product>
</Wix>
"#;
        let root_upg_other = parser.parse(xml_upg_other).unwrap_or_default();
        assert!(compiler.compile(&root_upg_other).is_ok());

        // Test compiling Fragment with MajorUpgrade on fresh tables and top-level RegistrySearch (parent_id = None)
        let xml_frag = r#"
<Wix xmlns="http://schemas.microsoft.com/wix/2006/wi">
    <Fragment>
        <MajorUpgrade Schedule="afterInstallValidate" DowngradeErrorMessage="Downgrade blocked" />
        <RegistrySearch Id="FragSearch" Root="HKLM" Key="Software\App" />
    </Fragment>
</Wix>
"#;
        let root_frag = parser.parse(xml_frag).unwrap_or_default();
        assert!(compiler.compile(&root_frag).is_ok());

        let _ = fs::remove_dir_all(&temp_dir);
    }

    /// Tests compiling `Media` elements with `CompressionLevel` attribute creating `WixMediaCompression` table.
    ///
    /// # Errors
    ///
    /// Returns [`crate::MsiError`] if XML parsing fails.
    #[test]
    fn test_compiler_media_compression_level() {
        let xml = r#"
<Wix xmlns="http://schemas.microsoft.com/wix/2006/wi">
    <Product Id="{11111111-2222-3333-4444-555555555555}" Name="MediaApp" Version="1.0.0" Manufacturer="Vendor">
        <Package Description="Media Test" />
        <Media Id="1" Cabinet="app.cab" EmbedCab="yes" CompressionLevel="high" />
        <Media Id="2" Cabinet="none.cab" EmbedCab="no" CompressionLevel="none" />
        <Media Id="3" Cabinet="default.cab" />
    </Product>
</Wix>
"#;
        let parser = XmlParser::new();
        let root = parser.parse(xml).unwrap_or_default();
        let compiler = Compiler::new();
        let obj = compiler.compile(&root).unwrap_or_default();
        assert_eq!(obj.sections.len(), 1);
        let sec = &obj.sections[0];
        let mut count = 0;
        for tbl in sec
            .tables
            .iter()
            .filter(|t| t.name == "WixMediaCompression")
        {
            assert_eq!(tbl.records.len(), 2);
            assert_eq!(tbl.records[0].get(0), Some(&FieldValue::Short(1)));
            assert_eq!(
                tbl.records[0].get(1),
                Some(&FieldValue::String("high".to_string()))
            );
            assert_eq!(tbl.records[1].get(0), Some(&FieldValue::Short(2)));
            assert_eq!(
                tbl.records[1].get(1),
                Some(&FieldValue::String("none".to_string()))
            );
            count += 1;
        }
        assert_eq!(count, 1);
    }

    static EMPTY_TABLE: std::sync::LazyLock<IntermediateTable> =
        std::sync::LazyLock::new(|| IntermediateTable::new(""));

    /// Retrieves an intermediate table by name.
    fn get_table<'a>(sec: &'a IntermediateSection, name: &str) -> &'a IntermediateTable {
        sec.tables
            .iter()
            .find(|t| t.name == name)
            .unwrap_or(&EMPTY_TABLE)
    }

    /// Helper converting a [`Result` of `T`] into a vector of items.
    fn into_vec<T>(res: Result<T>) -> Vec<T> {
        res.ok().into_iter().collect()
    }

    /// Tests compiling `<EmbeddedChainer>` element into `MsiEmbeddedChainer` table and references.
    #[test]
    fn test_compiler_embedded_chainer() {
        let xml = r#"
<Wix xmlns="http://schemas.microsoft.com/wix/2006/wi">
    <Product Id="{11111111-2222-3333-4444-555555555555}" Name="ChainerApp" Version="1.0.0" Manufacturer="Vendor">
        <Package Description="Chainer Test" />
        <EmbeddedChainer Id="LibScriptChainer1" BinaryKey="MyChainerDll" CommandLine="/quiet" Condition="NOT Installed" />
        <EmbeddedChainer Id="LibScriptChainer2" FileKey="MyChainerExe" CommandLine="/verbose" />
        <EmbeddedChainer Id="LibScriptChainer3" SourceFile="binary\chainer.dll" />
    </Product>
</Wix>
"#;
        let parser = XmlParser::new();
        let root = parser.parse(xml).unwrap_or_default();
        let compiler = Compiler::new();
        let obj = compiler.compile(&root).unwrap_or_default();
        let sec = &obj.sections[0];

        assert_eq!(get_table(sec, "NoSuchTable").name, "");
        let chainer_tbl = get_table(sec, "MsiEmbeddedChainer");
        assert_eq!(chainer_tbl.records.len(), 3);

        // Chainer 1 (BinaryKey)
        let r1 = &chainer_tbl.records[0];
        assert_eq!(
            r1.get(0),
            Some(&FieldValue::String("LibScriptChainer1".to_string()))
        );
        assert_eq!(
            r1.get(1),
            Some(&FieldValue::String("NOT Installed".to_string()))
        );
        assert_eq!(r1.get(2), Some(&FieldValue::String("/quiet".to_string())));
        assert_eq!(
            r1.get(3),
            Some(&FieldValue::String("MyChainerDll".to_string()))
        );
        assert_eq!(r1.get(4), Some(&FieldValue::Long(1)));

        // Chainer 2 (FileKey)
        let r2 = &chainer_tbl.records[1];
        assert_eq!(
            r2.get(0),
            Some(&FieldValue::String("LibScriptChainer2".to_string()))
        );
        assert_eq!(r2.get(1), Some(&FieldValue::Null));
        assert_eq!(r2.get(2), Some(&FieldValue::String("/verbose".to_string())));
        assert_eq!(
            r2.get(3),
            Some(&FieldValue::String("MyChainerExe".to_string()))
        );
        assert_eq!(r2.get(4), Some(&FieldValue::Long(2)));

        // Chainer 3 (SourceFile -> auto synthesized Binary)
        let r3 = &chainer_tbl.records[2];
        assert_eq!(
            r3.get(0),
            Some(&FieldValue::String("LibScriptChainer3".to_string()))
        );
        assert_eq!(
            r3.get(3),
            Some(&FieldValue::String("LibScriptChainer3_Binary".to_string()))
        );
        assert_eq!(r3.get(4), Some(&FieldValue::Long(1)));

        // Verify missing Id error
        let bad_xml = r#"
<Wix xmlns="http://schemas.microsoft.com/wix/2006/wi">
    <Product Id="{11111111-2222-3333-4444-555555555555}" Name="App" Version="1.0.0" Manufacturer="Vendor">
        <EmbeddedChainer BinaryKey="Bin1" />
    </Product>
</Wix>
"#;
        let bad_root = parser.parse(bad_xml).unwrap_or_default();
        assert!(compiler.compile(&bad_root).is_err());
    }

    /// Tests compiling `<ServiceInstall>` and `<ServiceControl>` with full fidelity attributes.
    #[allow(clippy::cognitive_complexity)]
    #[test]
    #[allow(clippy::too_many_lines)]
    fn test_compiler_service_install_and_control_full_fidelity() {
        let xml = r#"
<Wix xmlns="http://schemas.microsoft.com/wix/2006/wi">
    <Product Id="{11111111-2222-3333-4444-555555555555}" Name="ServiceApp" Version="1.0.0" Manufacturer="Vendor">
        <Directory Id="TARGETDIR" Name="SourceDir">
            <Directory Id="ProgramFilesFolder" Name="PFiles">
                <Component Id="MySQLServiceComp" Guid="{22222222-3333-4444-5555-666666666666}">
                    <File Id="MySQLDaemon" Source="mysqld.exe" />
                    <ServiceInstall
                        Id="InstallMySQL"
                        Name="LibScript_MySQL"
                        DisplayName="LibScript MySQL 8.0 Server"
                        Type="ownProcess"
                        Start="auto"
                        ErrorControl="normal"
                        Account="NT AUTHORITY\NetworkService"
                        Password="secretPassword"
                        LoadOrderGroup="BaseGroup"
                        Dependencies="Tcpip"
                        Description="MySQL Relational Database Service"
                    />
                    <ServiceControl
                        Id="ControlMySQL1"
                        Name="LibScript_MySQL"
                        Start="both"
                        Stop="both"
                        Remove="both"
                        Wait="yes"
                        Arguments="--console"
                    />
                    <ServiceControl
                        Id="ControlMySQL2"
                        Name="LibScript_MySQL"
                        Start="install"
                        Stop="install"
                        Remove="install"
                        Wait="no"
                    />
                    <ServiceControl
                        Id="ControlMySQL3"
                        Name="LibScript_MySQL"
                        Start="uninstall"
                        Stop="uninstall"
                        Remove="uninstall"
                    />
                </Component>
            </Directory>
        </Directory>
    </Product>
</Wix>
"#;
        let parser = XmlParser::new();
        let root = parser.parse(xml).unwrap_or_default();
        let compiler = Compiler::new();
        let obj = compiler.compile(&root).unwrap_or_default();
        let sec = &obj.sections[0];

        // 1. Check ServiceInstall
        let svc_tbl = get_table(sec, "ServiceInstall");
        assert_eq!(svc_tbl.records.len(), 1);
        let s_rec = &svc_tbl.records[0];
        assert_eq!(
            s_rec.get(0),
            Some(&FieldValue::String("InstallMySQL".to_string()))
        );
        assert_eq!(
            s_rec.get(1),
            Some(&FieldValue::String("LibScript_MySQL".to_string()))
        );
        assert_eq!(
            s_rec.get(2),
            Some(&FieldValue::String(
                "LibScript MySQL 8.0 Server".to_string()
            ))
        );
        assert_eq!(s_rec.get(3), Some(&FieldValue::Long(16))); // ownProcess
        assert_eq!(s_rec.get(4), Some(&FieldValue::Long(2))); // auto
        assert_eq!(s_rec.get(5), Some(&FieldValue::Long(1))); // normal
        assert_eq!(
            s_rec.get(6),
            Some(&FieldValue::String("BaseGroup".to_string()))
        );
        assert_eq!(s_rec.get(7), Some(&FieldValue::String("Tcpip".to_string())));
        assert_eq!(
            s_rec.get(8),
            Some(&FieldValue::String(
                r"NT AUTHORITY\NetworkService".to_string()
            ))
        );
        assert_eq!(
            s_rec.get(9),
            Some(&FieldValue::String("secretPassword".to_string()))
        );
        assert_eq!(
            s_rec.get(10),
            Some(&FieldValue::String("MySQLServiceComp".to_string()))
        );

        // 2. Check MsiServiceConfig generated from Description
        let cfg_tbl = get_table(sec, "MsiServiceConfig");
        assert_eq!(cfg_tbl.records.len(), 1);
        let c_rec = &cfg_tbl.records[0];
        assert_eq!(
            c_rec.get(0),
            Some(&FieldValue::String("InstallMySQL_Config".to_string()))
        );
        assert_eq!(
            c_rec.get(1),
            Some(&FieldValue::String("LibScript_MySQL".to_string()))
        );
        assert_eq!(
            c_rec.get(4),
            Some(&FieldValue::String(
                "MySQL Relational Database Service".to_string()
            ))
        );

        // 3. Check ServiceControl
        let ctrl_tbl = get_table(sec, "ServiceControl");
        assert_eq!(ctrl_tbl.records.len(), 3);

        // Control 1: both (Start: 0x0011, Stop: 0x0022, Remove: 0x0088 -> total: 0x00BB = 187)
        let c1 = &ctrl_tbl.records[0];
        assert_eq!(
            c1.get(2),
            Some(&FieldValue::Short(0x0011 | 0x0022 | 0x0088))
        );
        assert_eq!(
            c1.get(3),
            Some(&FieldValue::String("--console".to_string()))
        );
        assert_eq!(c1.get(4), Some(&FieldValue::Short(1))); // Wait="yes"

        // Control 2: install (Start: 0x0001, Stop: 0x0002, Remove: 0x0008 -> total: 0x000B = 11)
        let c2 = &ctrl_tbl.records[1];
        assert_eq!(
            c2.get(2),
            Some(&FieldValue::Short(0x0001 | 0x0002 | 0x0008))
        );
        assert_eq!(c2.get(3), Some(&FieldValue::Null));
        assert_eq!(c2.get(4), Some(&FieldValue::Short(0))); // Wait="no"

        // Control 3: uninstall (Start: 0x0010, Stop: 0x0020, Remove: 0x0080 -> total: 0x00B0 = 176)
        let c3 = &ctrl_tbl.records[2];
        assert_eq!(
            c3.get(2),
            Some(&FieldValue::Short(0x0010 | 0x0020 | 0x0080))
        );
        assert_eq!(c3.get(4), Some(&FieldValue::Null)); // Wait unspecified
    }

    /// Tests compiling declarative SQL elements (`SqlDatabase`, `SqlString`, `SqlScript`).
    #[test]
    #[allow(clippy::too_many_lines)]
    #[allow(clippy::similar_names)]
    fn test_compiler_sql_extension_elements() {
        let xml = r#"
<Wix xmlns="http://schemas.microsoft.com/wix/2006/wi" xmlns:sql="http://schemas.microsoft.com/wix/SqlExtension">
    <Product Id="{11111111-2222-3333-4444-555555555555}" Name="SqlApp" Version="1.0.0" Manufacturer="Vendor">
        <Directory Id="TARGETDIR" Name="SourceDir">
            <Component Id="SqlProvisionComp" Guid="{33333333-4444-5555-6666-777777777777}">
                <sql:SqlDatabase
                    Id="OpenEdXDatabase"
                    Server="127.0.0.1"
                    Database="openedx"
                    CreateOnInstall="yes"
                    DropOnUninstall="yes"
                    User="root_user"
                >
                    <sql:SqlString
                        Id="CreateSchema"
                        SQL="CREATE DATABASE IF NOT EXISTS `openedx`;"
                        ExecuteOnInstall="yes"
                        Sequence="1"
                    />
                </sql:SqlDatabase>
                <sql:SqlScript
                    Id="InitTables"
                    SqlDb="OpenEdXDatabase"
                    BinaryKey="InitSqlBinary"
                    ExecuteOnInstall="yes"
                    Sequence="2"
                />
            </Component>
        </Directory>
    </Product>
</Wix>
"#;
        let parser = XmlParser::new();
        let root = parser.parse(xml).unwrap_or_default();
        let compiler = Compiler::new();
        let obj = compiler.compile(&root).unwrap_or_default();
        let sec = &obj.sections[0];

        // 1. SqlDatabase table
        let db_tbl = get_table(sec, "SqlDatabase");
        assert_eq!(db_tbl.records.len(), 1);
        let db_rec = &db_tbl.records[0];
        assert_eq!(
            db_rec.get(0),
            Some(&FieldValue::String("OpenEdXDatabase".to_string()))
        );
        assert_eq!(
            db_rec.get(1),
            Some(&FieldValue::String("127.0.0.1".to_string()))
        );
        assert_eq!(
            db_rec.get(3),
            Some(&FieldValue::String("openedx".to_string()))
        );
        assert_eq!(
            db_rec.get(4),
            Some(&FieldValue::String("SqlProvisionComp".to_string()))
        );
        assert_eq!(
            db_rec.get(5),
            Some(&FieldValue::String("root_user".to_string()))
        );
        assert_eq!(db_rec.get(6), Some(&FieldValue::Long(0x0001 | 0x0002))); // CreateOnInstall | DropOnUninstall

        // 2. SqlString table
        let str_tbl = get_table(sec, "SqlString");
        assert_eq!(str_tbl.records.len(), 1);
        let str_rec = &str_tbl.records[0];
        assert_eq!(
            str_rec.get(0),
            Some(&FieldValue::String("CreateSchema".to_string()))
        );
        assert_eq!(
            str_rec.get(1),
            Some(&FieldValue::String("OpenEdXDatabase".to_string()))
        );
        assert_eq!(
            str_rec.get(2),
            Some(&FieldValue::String(
                "CREATE DATABASE IF NOT EXISTS `openedx`;".to_string()
            ))
        );
        assert_eq!(str_rec.get(4), Some(&FieldValue::Long(1))); // ExecuteOnInstall
        assert_eq!(str_rec.get(5), Some(&FieldValue::Long(1))); // Sequence

        // 3. SqlScript table
        let scr_tbl = get_table(sec, "SqlScript");
        assert_eq!(scr_tbl.records.len(), 1);
        let scr_rec = &scr_tbl.records[0];
        assert_eq!(
            scr_rec.get(0),
            Some(&FieldValue::String("InitTables".to_string()))
        );
        assert_eq!(
            scr_rec.get(1),
            Some(&FieldValue::String("OpenEdXDatabase".to_string()))
        );
        assert_eq!(
            scr_rec.get(2),
            Some(&FieldValue::String("SqlProvisionComp".to_string()))
        );
        assert_eq!(
            scr_rec.get(3),
            Some(&FieldValue::String("InitSqlBinary".to_string()))
        );
        assert_eq!(scr_rec.get(5), Some(&FieldValue::Long(1))); // ExecuteOnInstall
        assert_eq!(scr_rec.get(6), Some(&FieldValue::Long(2))); // Sequence

        // 4. Test missing Id errors and missing SQL text
        let no_id_db = r#"<Wix xmlns="http://schemas.microsoft.com/wix/2006/wi"><Product Id="P" Name="N" Version="1" Manufacturer="M"><Component Id="C"><SqlDatabase Database="db" /></Component></Product></Wix>"#;
        assert!(compiler
            .compile(&parser.parse(no_id_db).unwrap_or_default())
            .is_err());

        let no_id_str = r#"<Wix xmlns="http://schemas.microsoft.com/wix/2006/wi"><Product Id="P" Name="N" Version="1" Manufacturer="M"><Component Id="C"><SqlString SQL="select 1" /></Component></Product></Wix>"#;
        assert!(compiler
            .compile(&parser.parse(no_id_str).unwrap_or_default())
            .is_err());

        let no_sql_text = r#"<Wix xmlns="http://schemas.microsoft.com/wix/2006/wi"><Product Id="P" Name="N" Version="1" Manufacturer="M"><Component Id="C"><SqlString Id="S1" /></Component></Product></Wix>"#;
        assert!(compiler
            .compile(&parser.parse(no_sql_text).unwrap_or_default())
            .is_err());

        let no_id_scr = r#"<Wix xmlns="http://schemas.microsoft.com/wix/2006/wi"><Product Id="P" Name="N" Version="1" Manufacturer="M"><Component Id="C"><SqlScript ScriptFile="f.sql" /></Component></Product></Wix>"#;
        assert!(compiler
            .compile(&parser.parse(no_id_scr).unwrap_or_default())
            .is_err());
    }

    /// Tests extended service, SQL, and embedded chainer attributes for 100% coverage.
    #[test]
    fn test_compiler_services_sql_and_chainer_full_coverage() {
        let xml = r#"
<Wix xmlns="http://schemas.microsoft.com/wix/2006/wi">
    <Product Id="{11111111-2222-3333-4444-555555555555}" Name="EdgeAttrs" Version="1.0.0" Manufacturer="Test">
        <Package Description="Edge Test" />
        <Directory Id="TARGETDIR" Name="SourceDir">
            <Component Id="C1">
                <File Id="F1" Source="test.sys" KeyPath="yes" />
                <ServiceInstall Id="Drv1" Name="Drv1" Type="kernelDriver" Start="boot" ErrorControl="severe" />
                <ServiceInstall Id="Drv2" Name="Drv2" Type="systemDriver" Start="system" />
                <ServiceControl Id="CtrlOther" Name="SvcOther" Start="other" Stop="other" Remove="other" />
                <ServiceControl Id="CtrlNone" Name="SvcNone" />
                <SqlDatabase Id="DbFull" Server="127.0.0.1" Database="appdb" CreateOnInstall="yes" DropOnUninstall="yes" ContinueOnError="yes" DropOnInstall="yes" CreateOnUninstall="yes" />
                <SqlDatabase Id="DbMinimal" />
                <SqlString Id="SqlInner" SqlDb="DbFull" ExecuteOnInstall="yes" ExecuteOnUninstall="yes" Rollback="yes" ContinueOnError="yes">
                    CREATE TABLE users (id INT PRIMARY KEY);
                </SqlString>
                <SqlString Id="SqlNoInstall" SqlDb="DbFull" ExecuteOnInstall="no">
                    SELECT 1;
                </SqlString>
                <SqlString Id="SqlOmittedInstall" SqlDb="DbFull">
                    SELECT 2;
                </SqlString>
                <SqlScript Id="ScriptFull" SqlDb="DbFull" ExecuteOnInstall="yes" ExecuteOnUninstall="yes" Rollback="yes" ContinueOnError="yes" ScriptFile="setup.sql" />
                <SqlScript Id="ScriptNoInstall" SqlDb="DbFull" ExecuteOnInstall="no" ScriptFile="setup2.sql" />
                <SqlScript Id="ScriptOmittedInstall" SqlDb="DbFull" ScriptFile="setup3.sql" />
            </Component>
        </Directory>
        <EmbeddedChainer Id="DefaultChainer" />
    </Product>
</Wix>
"#;
        let parser = XmlParser::new();
        let root = parser.parse(xml).unwrap_or_default();
        let compiler = Compiler::new();
        let obj = compiler.compile(&root).unwrap_or_default();
        let sec = &obj.sections[0];

        let svc_tbl = get_table(sec, "ServiceInstall");
        assert_eq!(svc_tbl.records.len(), 2);
        // Drv1: kernelDriver (1), boot (0), severe (2)
        assert_eq!(svc_tbl.records[0].get(3), Some(&FieldValue::Long(1)));
        assert_eq!(svc_tbl.records[0].get(4), Some(&FieldValue::Long(0)));
        assert_eq!(svc_tbl.records[0].get(5), Some(&FieldValue::Long(2)));
        // Drv2: systemDriver (2), system (1)
        assert_eq!(svc_tbl.records[1].get(3), Some(&FieldValue::Long(2)));
        assert_eq!(svc_tbl.records[1].get(4), Some(&FieldValue::Long(1)));

        let ctrl_tbl = get_table(sec, "ServiceControl");
        assert_eq!(ctrl_tbl.records.len(), 2);
        // Event should be 0 because "other" didn't set any bit
        assert_eq!(ctrl_tbl.records[0].get(2), Some(&FieldValue::Short(0)));
        assert_eq!(ctrl_tbl.records[1].get(2), Some(&FieldValue::Short(0)));

        let db_tbl = get_table(sec, "SqlDatabase");
        assert_eq!(db_tbl.records.len(), 2);
        // Attrs: 0x01 | 0x02 | 0x04 | 0x08 | 0x10 = 0x1F = 31
        assert_eq!(db_tbl.records[0].get(6), Some(&FieldValue::Long(31)));
        assert_eq!(db_tbl.records[1].get(6), Some(&FieldValue::Long(0)));

        let str_tbl = get_table(sec, "SqlString");
        assert_eq!(str_tbl.records.len(), 3);
        assert_eq!(
            str_tbl.records[0].get(2),
            Some(&FieldValue::String(
                "CREATE TABLE users (id INT PRIMARY KEY);".to_string()
            ))
        );
        // Attrs: 0x01 | 0x02 | 0x04 | 0x08 = 15
        assert_eq!(str_tbl.records[0].get(4), Some(&FieldValue::Long(15)));
        // SqlNoInstall: ExecuteOnInstall="no", attrs = 0
        assert_eq!(str_tbl.records[1].get(4), Some(&FieldValue::Long(0)));
        // SqlOmittedInstall: ExecuteOnInstall omitted (defaults to 1)
        assert_eq!(str_tbl.records[2].get(4), Some(&FieldValue::Long(1)));

        let script_tbl = get_table(sec, "SqlScript");
        assert_eq!(script_tbl.records.len(), 3);
        // Attrs: 0x01 | 0x02 | 0x04 | 0x08 = 15
        assert_eq!(script_tbl.records[0].get(5), Some(&FieldValue::Long(15)));
        // ScriptNoInstall: ExecuteOnInstall="no", attrs = 0
        assert_eq!(script_tbl.records[1].get(5), Some(&FieldValue::Long(0)));
        // ScriptOmittedInstall: ExecuteOnInstall omitted (defaults to 1)
        assert_eq!(script_tbl.records[2].get(5), Some(&FieldValue::Long(1)));

        let chainer_tbl = get_table(sec, "MsiEmbeddedChainer");
        assert_eq!(chainer_tbl.records.len(), 1);
        assert_eq!(
            chainer_tbl.records[0].get(3),
            Some(&FieldValue::String("DefaultChainer_Binary".to_string()))
        );
    }

    /// Tests comprehensive error branches and invalid input handling across compiler elements.
    #[allow(clippy::cognitive_complexity)]
    #[test]
    #[allow(clippy::too_many_lines)]
    fn test_compiler_comprehensive_error_regions() {
        let compiler = Compiler::new();
        let parser = XmlParser::new();

        // 1. Invalid XML schema xmlns (L 105)
        let invalid_xmlns = r#"<Wix xmlns="http://invalid.uri"><Product Id="{11111111-1111-1111-1111-111111111111}" Language="1033" /></Wix>"#;
        assert!(compiler
            .compile(&parser.parse(invalid_xmlns).unwrap_or_default())
            .is_err());

        // 2. Child section error propagations (L 116, 120, 124, 128)
        let bad_module = r#"<Wix><Module Id="{11111111-1111-1111-1111-111111111111}"><Directory /></Module></Wix>"#;
        assert!(compiler
            .compile(&parser.parse(bad_module).unwrap_or_default())
            .is_err());

        let bad_fragment = "<Wix><Fragment><Directory /></Fragment></Wix>";
        assert!(compiler
            .compile(&parser.parse(bad_fragment).unwrap_or_default())
            .is_err());

        let bad_patch_creation =
            r#"<Wix><PatchCreation Id="PC"><Directory /></PatchCreation></Wix>"#;
        assert!(compiler
            .compile(&parser.parse(bad_patch_creation).unwrap_or_default())
            .is_err());

        let bad_patch = r#"<Wix><Patch Id="P"><Directory /></Patch></Wix>"#;
        assert!(compiler
            .compile(&parser.parse(bad_patch).unwrap_or_default())
            .is_err());

        // 3. DirectoryId and parent error branches (L 390, 392, 415)
        let bad_dir_id = r#"<Wix><Product Id="{11111111-1111-1111-1111-111111111111}" Language="1033"><Directory Id="" /></Product></Wix>"#;
        assert!(compiler
            .compile(&parser.parse(bad_dir_id).unwrap_or_default())
            .is_err());

        let bad_dir_parent = r#"<Wix><Product Id="{11111111-1111-1111-1111-111111111111}" Language="1033"><Directory Id="D1"><Directory Id="D2"><Component /></Directory></Directory></Product></Wix>"#;
        assert!(compiler
            .compile(&parser.parse(bad_dir_parent).unwrap_or_default())
            .is_err());

        // 4. Component errors (L 423, 424, 429)
        let bad_comp_id = r#"<Wix><Product Id="{11111111-1111-1111-1111-111111111111}" Language="1033"><Component Id="" /></Product></Wix>"#;
        assert!(compiler
            .compile(&parser.parse(bad_comp_id).unwrap_or_default())
            .is_err());

        let bad_comp_guid = r#"<Wix><Product Id="{11111111-1111-1111-1111-111111111111}" Language="1033"><Component Id="C1" Guid="not-a-valid-guid" /></Product></Wix>"#;
        assert!(compiler
            .compile(&parser.parse(bad_comp_guid).unwrap_or_default())
            .is_err());

        // 5. File errors and Font (L 527, 528, 662)
        let bad_file_id = r#"<Wix><Product Id="{11111111-1111-1111-1111-111111111111}"><Component Id="C1"><File Id="" /></Component></Product></Wix>"#;
        assert!(compiler
            .compile(&parser.parse(bad_file_id).unwrap_or_default())
            .is_err());

        // 6. Feature and FeatureRef errors (L 719, 721, 749, 753, 795, 902)
        let bad_feat_id = r#"<Wix><Product Id="{11111111-1111-1111-1111-111111111111}"><Feature Id="" /></Product></Wix>"#;
        assert!(compiler
            .compile(&parser.parse(bad_feat_id).unwrap_or_default())
            .is_err());

        let bad_feat_comp_ref = r#"<Wix><Product Id="{11111111-1111-1111-1111-111111111111}"><Feature Id="F1"><ComponentRef Id="" /></Feature></Product></Wix>"#;
        assert!(compiler
            .compile(&parser.parse(bad_feat_comp_ref).unwrap_or_default())
            .is_err());

        let bad_feat_child = r#"<Wix><Product Id="{11111111-1111-1111-1111-111111111111}"><Feature Id="F1"><Component /></Feature></Product></Wix>"#;
        assert!(compiler
            .compile(&parser.parse(bad_feat_child).unwrap_or_default())
            .is_err());

        let bad_feat_ref_child = r#"<Wix><Product Id="{11111111-1111-1111-1111-111111111111}"><FeatureRef Id="FR1"><Component /></FeatureRef></Product></Wix>"#;
        assert!(compiler
            .compile(&parser.parse(bad_feat_ref_child).unwrap_or_default())
            .is_err());

        // 7. DirectoryRef and ComponentGroup errors (L 802, 804, 814, 821, 836, 857)
        let bad_dir_ref = r#"<Wix><Product Id="{11111111-1111-1111-1111-111111111111}"><DirectoryRef Id="" /></Product></Wix>"#;
        assert!(compiler
            .compile(&parser.parse(bad_dir_ref).unwrap_or_default())
            .is_err());

        let bad_dir_ref_child = r#"<Wix><Product Id="{11111111-1111-1111-1111-111111111111}"><DirectoryRef Id="TARGETDIR"><Component /></DirectoryRef></Product></Wix>"#;
        assert!(compiler
            .compile(&parser.parse(bad_dir_ref_child).unwrap_or_default())
            .is_err());

        let bad_cg_dir = r#"<Wix><Product Id="{11111111-1111-1111-1111-111111111111}"><ComponentGroup Id="CG1" Directory="" /></Product></Wix>"#;
        assert!(compiler
            .compile(&parser.parse(bad_cg_dir).unwrap_or_default())
            .is_err());

        let bad_cg_comp_ref = r#"<Wix><Product Id="{11111111-1111-1111-1111-111111111111}"><ComponentGroup Id="CG1"><ComponentRef Id="" /></ComponentGroup></Product></Wix>"#;
        assert!(compiler
            .compile(&parser.parse(bad_cg_comp_ref).unwrap_or_default())
            .is_err());

        let bad_cg_comp = r#"<Wix><Product Id="{11111111-1111-1111-1111-111111111111}"><ComponentGroup Id="CG1"><Component Id="" /></ComponentGroup></Product></Wix>"#;
        assert!(compiler
            .compile(&parser.parse(bad_cg_comp).unwrap_or_default())
            .is_err());

        let bad_cg_child = r#"<Wix><Product Id="{11111111-1111-1111-1111-111111111111}"><ComponentGroup Id="CG1"><Directory /></ComponentGroup></Product></Wix>"#;
        assert!(compiler
            .compile(&parser.parse(bad_cg_child).unwrap_or_default())
            .is_err());

        // 8. PackageGroup and FeatureGroup child errors (L 872, 887, 979)
        let bad_pkg_grp_child = r#"<Wix><Product Id="{11111111-1111-1111-1111-111111111111}"><PackageGroup Id="PGR1"><Component /></PackageGroup></Product></Wix>"#;
        assert!(compiler
            .compile(&parser.parse(bad_pkg_grp_child).unwrap_or_default())
            .is_err());

        let bad_feat_grp_child = r#"<Wix><Product Id="{11111111-1111-1111-1111-111111111111}"><FeatureGroup Id="FGR1"><Component /></FeatureGroup></Product></Wix>"#;
        assert!(compiler
            .compile(&parser.parse(bad_feat_grp_child).unwrap_or_default())
            .is_err());

        let bad_pkg_child = r#"<Wix><Product Id="{11111111-1111-1111-1111-111111111111}"><Package><Component /></Package></Product></Wix>"#;
        assert!(compiler
            .compile(&parser.parse(bad_pkg_child).unwrap_or_default())
            .is_err());

        // 9. Sequence and table errors (L 1014, 1027, 1028, 1053, 1090)
        let bad_seq_action = r#"<Wix><Product Id="{11111111-1111-1111-1111-111111111111}"><InstallUISequence><Custom Action="" After="Cost" /></InstallUISequence></Product></Wix>"#;
        assert!(compiler
            .compile(&parser.parse(bad_seq_action).unwrap_or_default())
            .is_err());

        let bad_custom_action_id = r#"<Wix><Product Id="{11111111-1111-1111-1111-111111111111}"><SetDirectory Id="TARGETDIR" Action="" Value="C:\" /></Product></Wix>"#;
        assert!(compiler
            .compile(&parser.parse(bad_custom_action_id).unwrap_or_default())
            .is_err());

        let bad_create_folder_dir = r#"<Wix><Product Id="{11111111-1111-1111-1111-111111111111}"><CreateFolder Directory="" /></Product></Wix>"#;
        assert!(compiler
            .compile(&parser.parse(bad_create_folder_dir).unwrap_or_default())
            .is_err());

        let bad_env = r#"<Wix><Product Id="{11111111-1111-1111-1111-111111111111}"><Environment /></Product></Wix>"#;
        assert!(compiler
            .compile(&parser.parse(bad_env).unwrap_or_default())
            .is_err());

        // 10. Property and Registry errors (L 1174, 1199, 1216, 1235)
        let bad_prop_id = r#"<Wix><Product Id="{11111111-1111-1111-1111-111111111111}"><Property Id="" Value="V" /></Product></Wix>"#;
        assert!(compiler
            .compile(&parser.parse(bad_prop_id).unwrap_or_default())
            .is_err());

        let bad_prop_child = r#"<Wix><Product Id="{11111111-1111-1111-1111-111111111111}"><Property Id="P1"><Component /></Property></Product></Wix>"#;
        assert!(compiler
            .compile(&parser.parse(bad_prop_child).unwrap_or_default())
            .is_err());

        let bad_reg_key_child = r#"<Wix><Product Id="{11111111-1111-1111-1111-111111111111}"><Component Id="C1" Directory="TARGETDIR"><RegistryKey Root="HKLM" Key="Software\App"><Component /></RegistryKey></Component></Product></Wix>"#;
        assert!(compiler
            .compile(&parser.parse(bad_reg_key_child).unwrap_or_default())
            .is_err());

        let bad_sql_db_child = r#"<Wix><Product Id="{11111111-1111-1111-1111-111111111111}"><SqlDatabase Id="DB1"><Component /></SqlDatabase></Product></Wix>"#;
        assert!(compiler
            .compile(&parser.parse(bad_sql_db_child).unwrap_or_default())
            .is_err());

        // 11. SymbolicLink, SetProperty, ExecuteSequence (L 1303, 1341, 3362, 3708)
        let bad_symlink_dir = r#"<Wix><Product Id="{11111111-1111-1111-1111-111111111111}"><SymbolicLink Directory="" Target="t" /></Product></Wix>"#;
        assert!(compiler
            .compile(&parser.parse(bad_symlink_dir).unwrap_or_default())
            .is_err());

        let bad_set_property = r#"<Wix><Product Id="{11111111-1111-1111-1111-111111111111}"><SetProperty Action="" Value="V" /></Product></Wix>"#;
        assert!(compiler
            .compile(&parser.parse(bad_set_property).unwrap_or_default())
            .is_err());

        let bad_execute_seq = r#"<Wix><Product Id="{11111111-1111-1111-1111-111111111111}"><InstallExecuteSequence><Custom Action="" OnExit="success" /></InstallExecuteSequence></Product></Wix>"#;
        assert!(compiler
            .compile(&parser.parse(bad_execute_seq).unwrap_or_default())
            .is_err());
    }

    /// Tests top-level fallback component attribution for elements outside explicit Component tags.
    #[test]
    fn test_compiler_top_level_fallback_components() {
        let compiler = Compiler::new();
        let parser = XmlParser::new();
        let xml = r#"
<Wix xmlns="http://schemas.microsoft.com/wix/2006/wi">
    <Product Id="{11111111-2222-3333-4444-555555555555}" Name="FallbackApp" Version="1.0.0" Manufacturer="Vendor">
        <File Id="TopFile" Source="test.txt" />
        <Environment Id="TopEnv" Name="ENV" Value="1" />
        <RegistryValue Id="TopReg" Key="Software\App" Value="1" />
        <RemoveFile Id="TopRem" On="install" />
    </Product>
</Wix>
"#;
        let root = parser.parse(xml).unwrap_or_default();
        let obj = compiler.compile(&root).unwrap_or_default();
        assert_eq!(obj.sections.len(), 1);
    }

    /// Tests compiling `<Shortcut>`, `<Environment>` (with Part="last" and Part="first"),
    /// nested `<RegistrySearch>` and `<FileSearch>`, and `<Condition Message="...">`.
    #[allow(clippy::cognitive_complexity)]
    #[test]
    #[allow(clippy::too_many_lines)]
    fn test_compiler_shortcuts_environment_searches_and_launch_conditions() {
        let compiler = Compiler::new();
        let parser = XmlParser::new();
        let xml = r#"
<Wix xmlns="http://schemas.microsoft.com/wix/2006/wi">
    <Product Id="{11111111-2222-3333-4444-555555555555}" Name="SearchApp" Version="1.0.0" Manufacturer="Vendor">
        <Package Description="Search and Shortcut Test" />

        <Condition Message="Windows 10 or later is required.">VersionNT &gt;= 603</Condition>

        <Property Id="PYTHON_EXE">
            <RegistrySearch Id="SearchPython" Root="HKLM" Key="Software\Python\PythonCore\3.11\InstallPath" Type="file" Win64="yes">
                <FileSearch Id="SearchPythonExe" Name="python.exe" MinVersion="3.11.0.0" />
            </RegistrySearch>
        </Property>

        <Directory Id="TARGETDIR" Name="SourceDir">
            <Directory Id="ProgramFilesFolder" Name="PFiles">
                <Directory Id="INSTALLFOLDER" Name="AppFolder">
                    <Component Id="CompApp" Guid="{22222222-3333-4444-5555-666666666666}">
                        <File Id="FileCli" Source="app.exe" KeyPath="yes" />
                        <Shortcut
                            Id="DesktopShortcut"
                            Directory="DesktopFolder"
                            Name="Open edX"
                            Target="[#FileCli]"
                            Arguments="--console"
                            Description="Open edX CLI Management Console"
                            Icon="AppIcon.ico"
                            IconIndex="0"
                            Show="normal"
                            WorkingDirectory="INSTALLFOLDER"
                        />
                        <Shortcut
                            Id="DesktopShortcutMin"
                            Directory="DesktopFolder"
                            Name="Open edX Min"
                            Target="[#FileCli]"
                            Show="minimized"
                        />
                        <Shortcut
                            Id="DesktopShortcutMax"
                            Directory="DesktopFolder"
                            Name="Open edX Max"
                            Target="[#FileCli]"
                            Show="maximized"
                        />
                        <Environment
                            Id="EnvPath"
                            Name="PATH"
                            Value="[INSTALLFOLDER]bin"
                            Part="last"
                            System="yes"
                            Action="set"
                        />
                        <Environment
                            Id="EnvRoot"
                            Name="LIBSCRIPT_ROOT_DIR"
                            Value="[INSTALLFOLDER]"
                            Part="all"
                            System="yes"
                            Action="create"
                        />
                        <Environment
                            Id="EnvPrepend"
                            Name="CUSTOM_PATH"
                            Value="[INSTALLFOLDER]lib"
                            Part="first"
                            System="no"
                            Action="set"
                        />
                        <Environment
                            Id="EnvLastWithTilde"
                            Name="EXISTING_LAST"
                            Value="[~];custom"
                            Part="last"
                            System="no"
                            Action="set"
                        />
                        <Environment
                            Id="EnvFirstWithTilde"
                            Name="EXISTING_FIRST"
                            Value="custom;[~]"
                            Part="first"
                            System="no"
                            Action="set"
                        />
                    </Component>
                </Directory>
            </Directory>
        </Directory>
    </Product>
</Wix>
"#;
        let root = parser.parse(xml).unwrap_or_default();
        let obj = compiler.compile(&root).unwrap_or_default();
        let sec = &obj.sections[0];

        // 1. Verify LaunchCondition table
        let lc_tbl = get_table(sec, "LaunchCondition");
        assert_eq!(lc_tbl.records.len(), 1);
        assert_eq!(
            lc_tbl.records[0].get(0),
            Some(&FieldValue::String("VersionNT >= 603".to_string()))
        );
        assert_eq!(
            lc_tbl.records[0].get(1),
            Some(&FieldValue::String(
                "Windows 10 or later is required.".to_string()
            ))
        );

        // 2. Verify Shortcut table
        let sc_tbl = get_table(sec, "Shortcut");
        assert_eq!(sc_tbl.records.len(), 3);
        let sc_rec = &sc_tbl.records[0];
        assert_eq!(
            sc_rec.get(0),
            Some(&FieldValue::String("DesktopShortcut".to_string()))
        );
        assert_eq!(
            sc_rec.get(1),
            Some(&FieldValue::String("DesktopFolder".to_string()))
        );
        assert_eq!(
            sc_rec.get(2),
            Some(&FieldValue::String("OPENED~1|Open edX".to_string()))
        );
        assert_eq!(
            sc_rec.get(3),
            Some(&FieldValue::String("CompApp".to_string()))
        );
        assert_eq!(
            sc_rec.get(4),
            Some(&FieldValue::String("[#FileCli]".to_string()))
        );
        assert_eq!(
            sc_rec.get(5),
            Some(&FieldValue::String("--console".to_string()))
        );
        assert_eq!(
            sc_rec.get(6),
            Some(&FieldValue::String(
                "Open edX CLI Management Console".to_string()
            ))
        );
        assert_eq!(
            sc_rec.get(8),
            Some(&FieldValue::String("AppIcon.ico".to_string()))
        );
        assert_eq!(sc_rec.get(9), Some(&FieldValue::Short(0))); // IconIndex
        assert_eq!(sc_rec.get(10), Some(&FieldValue::Short(1))); // Show = normal (1)
        assert_eq!(
            sc_rec.get(11),
            Some(&FieldValue::String("INSTALLFOLDER".to_string()))
        );

        // Minimized (Show = 7)
        let sc_min = &sc_tbl.records[1];
        assert_eq!(
            sc_min.get(0),
            Some(&FieldValue::String("DesktopShortcutMin".to_string()))
        );
        assert_eq!(sc_min.get(10), Some(&FieldValue::Short(7)));

        // Maximized (Show = 3)
        let sc_max = &sc_tbl.records[2];
        assert_eq!(
            sc_max.get(0),
            Some(&FieldValue::String("DesktopShortcutMax".to_string()))
        );
        assert_eq!(sc_max.get(10), Some(&FieldValue::Short(3)));

        // 3. Verify Environment table
        let env_tbl = get_table(sec, "Environment");
        assert_eq!(env_tbl.records.len(), 5);
        // EnvPath: System="yes" (*), Action="set" (=) -> "*=PATH", Part="last" -> "[~];[INSTALLFOLDER]bin"
        assert_eq!(
            env_tbl.records[0].get(0),
            Some(&FieldValue::String("EnvPath".to_string()))
        );
        assert_eq!(
            env_tbl.records[0].get(1),
            Some(&FieldValue::String("*=PATH".to_string()))
        );
        assert_eq!(
            env_tbl.records[0].get(2),
            Some(&FieldValue::String("[~];[INSTALLFOLDER]bin".to_string()))
        );
        // EnvRoot: System="yes" (*), Action="create" (+) -> "*+LIBSCRIPT_ROOT_DIR", Part="all" -> "[INSTALLFOLDER]"
        assert_eq!(
            env_tbl.records[1].get(0),
            Some(&FieldValue::String("EnvRoot".to_string()))
        );
        assert_eq!(
            env_tbl.records[1].get(1),
            Some(&FieldValue::String("*+LIBSCRIPT_ROOT_DIR".to_string()))
        );
        assert_eq!(
            env_tbl.records[1].get(2),
            Some(&FieldValue::String("[INSTALLFOLDER]".to_string()))
        );
        // EnvPrepend: System="no" (), Action="set" (=) -> "=CUSTOM_PATH", Part="first" -> "[INSTALLFOLDER]lib;[~]"
        assert_eq!(
            env_tbl.records[2].get(0),
            Some(&FieldValue::String("EnvPrepend".to_string()))
        );
        assert_eq!(
            env_tbl.records[2].get(1),
            Some(&FieldValue::String("=CUSTOM_PATH".to_string()))
        );
        assert_eq!(
            env_tbl.records[2].get(2),
            Some(&FieldValue::String("[INSTALLFOLDER]lib;[~]".to_string()))
        );
        // EnvLastWithTilde: Part="last" with existing "[~]" -> preserves value "[~];custom"
        assert_eq!(
            env_tbl.records[3].get(0),
            Some(&FieldValue::String("EnvLastWithTilde".to_string()))
        );
        assert_eq!(
            env_tbl.records[3].get(2),
            Some(&FieldValue::String("[~];custom".to_string()))
        );
        // EnvFirstWithTilde: Part="first" with existing "[~]" -> preserves value "custom;[~]"
        assert_eq!(
            env_tbl.records[4].get(0),
            Some(&FieldValue::String("EnvFirstWithTilde".to_string()))
        );
        assert_eq!(
            env_tbl.records[4].get(2),
            Some(&FieldValue::String("custom;[~]".to_string()))
        );

        // 4. Verify RegLocator & AppSearch & Signature
        let app_tbl = get_table(sec, "AppSearch");
        assert_eq!(app_tbl.records.len(), 1);
        assert_eq!(
            app_tbl.records[0].get(0),
            Some(&FieldValue::String("PYTHON_EXE".to_string()))
        );
        assert_eq!(
            app_tbl.records[0].get(1),
            Some(&FieldValue::String("SearchPython".to_string()))
        );

        let reg_tbl = get_table(sec, "RegLocator");
        assert_eq!(reg_tbl.records.len(), 1);
        assert_eq!(
            reg_tbl.records[0].get(0),
            Some(&FieldValue::String("SearchPython".to_string()))
        );
        // Type 0 (directory search locating FileSearch) | 0x0010 (Win64) = 16
        assert_eq!(reg_tbl.records[0].get(4), Some(&FieldValue::Short(16)));

        let sig_tbl = get_table(sec, "Signature");
        assert_eq!(sig_tbl.records.len(), 1);
        assert_eq!(
            sig_tbl.records[0].get(0),
            Some(&FieldValue::String("SearchPythonExe".to_string()))
        );
        assert_eq!(
            sig_tbl.records[0].get(1),
            Some(&FieldValue::String("python.exe".to_string()))
        );
        assert_eq!(
            sig_tbl.records[0].get(2),
            Some(&FieldValue::String("3.11.0.0".to_string()))
        );
    }

    /// Tests `FeatureRef` compilation across all child variations and error branches.
    #[test]
    fn test_compiler_feature_ref_all_branches() {
        let parser = XmlParser::new();
        let compiler = Compiler::new();

        let valid_wxs = r#"
        <Wix xmlns="http://schemas.microsoft.com/wix/2006/wi">
            <Product Id="{11111111-1111-1111-1111-111111111111}">
                <FeatureRef Id="MainFeature">
                    <ComponentRef Id="Comp1" />
                    <ComponentRef />
                    <ComponentGroupRef Id="CG1" />
                    <MergeRef Id="Mod1" />
                    <Feature Id="SubFeature" />
                </FeatureRef>
            </Product>
        </Wix>
        "#;
        for (xml, is_valid) in [
            (valid_wxs, true),
            (r#"<Wix xmlns="invalid_ns"/>"#, false),
            ("invalid <xml", false),
        ] {
            if let Ok(doc) = parser.parse(xml) {
                let res = compiler.compile(&doc);
                assert_eq!(res.is_ok(), is_valid);
                if let Ok(obj) = res {
                    let sec = &obj.sections[0];
                    assert!(sec
                        .references
                        .contains(&Reference::new("Feature", "MainFeature")));
                    assert!(sec
                        .references
                        .contains(&Reference::new("Component", "Comp1")));
                    assert!(sec
                        .references
                        .contains(&Reference::new("ComponentGroup", "CG1")));
                    let fc_tbl = get_table(sec, "FeatureComponents");
                    assert_eq!(fc_tbl.records.len(), 1);
                    let cg_tbl = get_table(sec, "_FeatureComponentGroupRef");
                    assert_eq!(cg_tbl.records.len(), 1);
                }
            }
        }

        // Error: invalid FeatureRef Id
        let bad_feat_ref_id = r#"<Wix><Product Id="{11111111-1111-1111-1111-111111111111}"><FeatureRef Id="" /></Product></Wix>"#;
        for xml in [bad_feat_ref_id, "invalid <xml"] {
            if let Ok(doc_bad) = parser.parse(xml) {
                assert!(compiler.compile(&doc_bad).is_err());
            }
        }

        // Error: invalid ComponentRef Id within FeatureRef
        let bad_comp_ref = r#"<Wix><Product Id="{11111111-1111-1111-1111-111111111111}"><FeatureRef Id="MainFeature"><ComponentRef Id="" /></FeatureRef></Product></Wix>"#;
        for xml in [bad_comp_ref, "invalid <xml"] {
            if let Ok(doc_bad) = parser.parse(xml) {
                assert!(compiler.compile(&doc_bad).is_err());
            }
        }
    }

    /// Tests deterministic identifier generation for `<RegistryValue>`, `<Shortcut>`,
    /// `<ServiceControl>`, and search elements when `Id` is omitted, verifying collision-free linking.
    #[test]
    fn test_compiler_deterministic_identifiers_and_collision_free() {
        let parser = XmlParser::new();
        let compiler = Compiler::new();

        let xml = r#"
<Wix xmlns="http://schemas.microsoft.com/wix/2006/wi">
    <Product Id="{11111111-2222-3333-4444-555555555555}" Name="MultiRegApp" Version="1.0.0" Manufacturer="Vendor">
        <Package Description="Multi Registry Test" />
        <Directory Id="TARGETDIR" Name="SourceDir">
            <Directory Id="ProgramFilesFolder" Name="PFiles">
                <Component Id="CompMulti" Guid="{22222222-3333-4444-5555-666666666666}">
                    <RegistryKey Root="HKLM" Key="Software\LibScript\MySQL">
                        <RegistryValue Name="Installed" Type="integer" Value="1" KeyPath="yes" />
                        <RegistryValue Name="Variant" Type="string" Value="online" />
                        <RegistryValue Name="Version" Type="string" Value="8.0.39" />
                        <RegistryValue Name="InstallDir" Type="string" Value="C:\Program Files\MySQL" />
                        <RegistryValue Name="Port" Type="string" Value="3306" />
                    </RegistryKey>
                    <Shortcut Name="App" Target="[TARGETDIR]app.exe" />
                    <ServiceControl Name="MySQLService" Start="install" Stop="both" Remove="uninstall" />
                </Component>
            </Directory>
        </Directory>
        <Feature Id="Main" Level="1">
            <ComponentRef Id="CompMulti" />
        </Feature>
        <Property Id="FOUND_EXE">
            <RegistrySearch Root="HKLM" Key="Software\App" Name="Path">
                <FileSearch Name="app.exe" />
            </RegistrySearch>
        </Property>
        <Property Id="FOUND_FILE_DIRECT">
            <FileSearch Name="app_direct.exe" />
        </Property>
        <Property Id="FOUND_DIR">
            <DirectorySearch Path="C:\App" />
        </Property>
        <Property Id="FOUND_INI">
            <IniFileSearch Name="app.ini" Section="Config" Key="Port" />
        </Property>
        <Property Id="FOUND_COMP">
            <ComponentSearch Guid="{22222222-3333-4444-5555-666666666666}" />
        </Property>
    </Product>
</Wix>
"#;
        let doc = parser.parse(xml).unwrap_or_default();
        let obj = compiler.compile(&doc).unwrap_or_default();
        let sec = &obj.sections[0];
        let reg_table = get_table(sec, "Registry");
        assert_eq!(reg_table.records.len(), 5);
        let mut ids = std::collections::HashSet::new();
        for rec in &reg_table.records {
            let id = format!("{}", rec.fields()[0]);
            assert!(id.starts_with("reg_"));
            ids.insert(id);
        }
        assert_eq!(ids.len(), 5);

        let sc_table = get_table(sec, "Shortcut");
        assert_eq!(sc_table.records.len(), 1);
        let sc_id = format!("{}", sc_table.records[0].fields()[0]);
        assert!(sc_id.starts_with("sc_"));

        let svc_ctrl_table = get_table(sec, "ServiceControl");
        assert_eq!(svc_ctrl_table.records.len(), 1);
        let ctrl_id = format!("{}", svc_ctrl_table.records[0].fields()[0]);
        assert!(ctrl_id.starts_with("svc_ctrl_"));

        let reg_loc_table = get_table(sec, "RegLocator");
        assert_eq!(reg_loc_table.records.len(), 1);
        let reg_sig = format!("{}", reg_loc_table.records[0].fields()[0]);
        assert!(reg_sig.starts_with("reg_search_"));

        let dr_loc_table = get_table(sec, "DrLocator");
        assert_eq!(dr_loc_table.records.len(), 1);
        let dr_sig = format!("{}", dr_loc_table.records[0].fields()[0]);
        assert!(dr_sig.starts_with("dir_search_"));

        let ini_loc_table = get_table(sec, "IniLocator");
        assert_eq!(ini_loc_table.records.len(), 1);
        let ini_sig = format!("{}", ini_loc_table.records[0].fields()[0]);
        assert!(ini_sig.starts_with("ini_search_"));

        let comp_loc_table = get_table(sec, "CompLocator");
        assert_eq!(comp_loc_table.records.len(), 1);
        let comp_sig = format!("{}", comp_loc_table.records[0].fields()[0]);
        assert!(comp_sig.starts_with("comp_search_"));

        let sig_table = get_table(sec, "Signature");
        assert_eq!(sig_table.records.len(), 2);
    }

    /// Tests `Dialog` element compilation and automatic detection of first, default, and cancel controls.
    #[test]
    fn test_compile_dialog_controls_first_default_cancel() {
        let xml = r#"<Wix xmlns="http://schemas.microsoft.com/wix/2006/wi">
    <Product Id="*" Name="T" Version="1.0" Manufacturer="M" UpgradeCode="{11111111-1111-1111-1111-111111111111}">
        <Package Description="D" />
        <UI>
            <Dialog Id="CustomDlg" Width="370" Height="270" Title="Title">
                <Control Id="NextBtn" Type="PushButton" X="0" Y="0" Width="50" Height="20" Default="yes" />
                <Control Id="CancelBtn" Type="PushButton" X="60" Y="0" Width="50" Height="20" Cancel="yes" />
            </Dialog>
        </UI>
    </Product>
</Wix>"#;
        let parser = XmlParser::new();
        let root = parser.parse(xml).unwrap_or_default();
        let compiler = Compiler::new();
        let obj = compiler.compile(&root).unwrap_or_default();
        let sec = &obj.sections[0];
        let dlg_tbl = get_table(sec, "Dialog");
        assert_eq!(dlg_tbl.records.len(), 1);
        let rec = &dlg_tbl.records[0];
        assert_eq!(rec.get(7), Some(&FieldValue::String("NextBtn".to_string())));
        assert_eq!(rec.get(8), Some(&FieldValue::String("NextBtn".to_string())));
        assert_eq!(
            rec.get(9),
            Some(&FieldValue::String("CancelBtn".to_string()))
        );

        // Test dialog with a child Control lacking an Id attribute and a non-Control child
        let mut dlg_node = XmlNode {
            tag: "Dialog".to_string(),
            ..Default::default()
        };
        dlg_node
            .attributes
            .insert("Id".to_string(), "DlgNoCtrlId".to_string());
        dlg_node.children.push(XmlNode {
            tag: "Control".to_string(),
            ..Default::default()
        });
        dlg_node.children.push(XmlNode {
            tag: "NonControl".to_string(),
            ..Default::default()
        });
        let mut dialog_sec =
            IntermediateSection::new(SectionType::Product, Some("Prod".to_string()));
        let mut tables = HashMap::new();
        assert!(ui_and_search::compile_dialog(&dlg_node, &mut dialog_sec, &mut tables).is_ok());
    }

    /// Tests `Product` with `Id="?"`, `InstallScope="perUser"`, nested file shortcut without explicit target, and full `UpgradeVersion` attribute flags.
    #[test]
    fn test_compiler_remaining_uncovered_paths() {
        let xml = r#"<Wix xmlns="http://schemas.microsoft.com/wix/2006/wi">
    <Product Id="?" Name="TestPerUser" Version="2.0.0" Manufacturer="TestMfg" UpgradeCode="{22222222-2222-2222-2222-222222222222}">
        <Package Description="Test" InstallScope="perUser" />
        <Upgrade Id="{22222222-2222-2222-2222-222222222222}">
            <UpgradeVersion Minimum="1.0.0" Maximum="2.0.0"
                IncludeMinimum="no"
                IncludeMaximum="yes"
                OnlyDetect="yes"
                MigrateFeatures="yes"
                IgnoreRemoveFailure="yes"
                Property="PREVIOUSVERSIONSINSTALLED" />
        </Upgrade>
        <Directory Id="TARGETDIR" Name="SourceDir">
            <Directory Id="ProgramFilesFolder" Name="PFiles">
                <Component Id="Comp1" Guid="{33333333-3333-3333-3333-333333333333}">
                    <File Id="File1" Source="main.exe">
                        <Shortcut Id="ScNoTarget" Directory="ProgramFilesFolder" Name="ShortcutNoTarget" />
                        <Shortcut Id="ScWithTarget" Directory="ProgramFilesFolder" Name="ShortcutWithTarget" Target="[#File1]" />
                    </File>
                </Component>
            </Directory>
        </Directory>
    </Product>
</Wix>"#;
        let parser = XmlParser::new();
        let root = parser.parse(xml).unwrap_or_default();
        let compiler = Compiler::new();
        let obj = compiler.compile(&root).unwrap_or_default();
        let sec = &obj.sections[0];

        // Also test Package with other InstallScope to exercise the fallthrough branch of InstallScope
        let xml_other_scope = r#"<Wix xmlns="http://schemas.microsoft.com/wix/2006/wi">
    <Product Id="*" Name="OtherScope" Version="1.0.0" Manufacturer="TestMfg">
        <Package Description="Test" InstallScope="customScope" />
    </Product>
</Wix>"#;
        let root_other = parser.parse(xml_other_scope).unwrap_or_default();
        assert!(compiler.compile(&root_other).is_ok());

        // Verify MSIINSTALLPERUSER property
        let prop_tbl = get_table(sec, "Property");
        let has_per_user = prop_tbl.records.iter().any(|r| {
            r.get(0) == Some(&FieldValue::String("MSIINSTALLPERUSER".to_string()))
                && r.get(1) == Some(&FieldValue::String("1".to_string()))
        });
        assert!(has_per_user);

        // Verify ProductCode generated deterministically from "?"
        let has_prod_code = prop_tbl
            .records
            .iter()
            .any(|r| r.get(0) == Some(&FieldValue::String("ProductCode".to_string())));
        assert!(has_prod_code);

        // Verify Shortcuts
        let sc_tbl = get_table(sec, "Shortcut");
        assert_eq!(sc_tbl.records.len(), 2);
        let sc_rec = &sc_tbl.records[0];
        assert_eq!(
            sc_rec.get(4),
            Some(&FieldValue::String("[#File1]".to_string()))
        );

        // Verify Upgrade table attributes:
        // IncludeMinimum="no" -> 0x100 not set
        // IncludeMaximum="yes" -> 0x200
        // OnlyDetect="yes" -> 0x002
        // MigrateFeatures="yes" -> 0x001
        // IgnoreRemoveFailure="yes" -> 0x004
        // Expected attrs = 0x200 | 0x002 | 0x001 | 0x004 = 0x207 = 519
        let upg_tbl = get_table(sec, "Upgrade");
        assert_eq!(upg_tbl.records.len(), 1);
        let upg_rec = &upg_tbl.records[0];
        assert_eq!(upg_rec.get(4), Some(&FieldValue::Long(519)));
    }

    /// Tests `<File Source="a/b/c/my_long_script_name.sh" />` basename extraction without slashes,
    /// `Compiler::with_source_dir`, and `ScrollableText` RTF conversion branches.
    #[test]
    #[allow(clippy::too_many_lines)]
    fn test_file_source_basename_and_scrollable_text_branches() {
        let temp_dir = std::env::temp_dir().join("msi_test_compiler_src_dir_scroll");
        let _ = fs::create_dir_all(&temp_dir);
        let rtf_path = temp_dir.join("rel_lic.rtf");
        let txt_path = temp_dir.join("rel_lic.txt");
        let _ = fs::write(&rtf_path, r"{\rtf1\ansi Relative RTF}");
        let _ = fs::write(&txt_path, "Relative Plain Text");

        let xml = r#"<Wix xmlns="http://schemas.microsoft.com/wix/2006/wi">
    <Product Id="*" Name="T" Version="1.0.0" Manufacturer="M">
        <Package Description="D" SummaryCodepage="1252" />
        <Directory Id="TARGETDIR" Name="SourceDir">
            <Component Id="C1" Guid="{11111111-2222-3333-4444-555555555555}">
                <File Id="F1" Source="a/b/c/my_long_script_name.sh" />
                <File Id="F2" Source="x\y\z\my_long_script_two.sh" />
            </Component>
        </Directory>
        <UI>
            <Dialog Id="D1" Width="370" Height="270" Title="T">
                <Control Id="S1" Type="ScrollableText" X="0" Y="0" Width="100" Height="50">
                    <Text SourceFile="rel_lic.rtf" />
                </Control>
                <Control Id="S2" Type="ScrollableText" X="0" Y="0" Width="100" Height="50">
                    <Text SourceFile="rel_lic.txt" />
                </Control>
                <Control Id="S3" Type="ScrollableText" X="0" Y="0" Width="100" Height="50">
                    <Text SourceFile="missing_deferred.txt" />
                </Control>
                <Control Id="S4" Type="Text" X="0" Y="0" Width="100" Height="50">
                    <Text SourceFile="missing_non_scroll.txt" />
                </Control>
                <Control Id="S5" Type="ScrollableText" X="0" Y="0" Width="100" Height="50">
                    <Text>Inline Plain License Text</Text>
                </Control>
                <Control Id="S6" Type="ScrollableText" X="0" Y="0" Width="100" Height="50">
                    <Text>{\rtf1 Already RTF}</Text>
                </Control>
                <Control Id="S7" Type="ScrollableText" X="0" Y="0" Width="100" Height="50">
                    <Text>!(wix.WixUILicenseRtf)</Text>
                </Control>
                <Control Id="S8" Type="ScrollableText" X="0" Y="0" Width="100" Height="50" Text="Attr Plain Text" />
                <Control Id="S9" Type="ScrollableText" X="0" Y="0" Width="100" Height="50" Text="{\rtf1 Attr RTF}" />
                <Control Id="S10" Type="ScrollableText" X="0" Y="0" Width="100" Height="50" Text="!(wix.WixUILicenseRtf)" />
                <Control Id="S11" Type="ScrollableText" X="0" Y="0" Width="100" Height="50" Text="!(bindpath.rtf:lic.rtf)" />
                <Control Id="S12" Type="ScrollableText" X="0" Y="0" Width="100" Height="50" Text="" />
            </Dialog>
        </UI>
    </Product>
</Wix>"#;
        let parser = XmlParser::new();
        let root = parser.parse(xml).unwrap_or_default();
        let compiler = Compiler::new().with_source_dir(&temp_dir);
        let obj = compiler.compile(&root).unwrap_or_default();
        let sec = &obj.sections[0];

        assert_eq!(
            format_msi_filename(None, "SHORT~1.SH|already_formatted.sh", &[]),
            "SHORT~1.SH|already_formatted.sh"
        );
        assert_eq!(
            format_msi_filename(None, "   +++   .txt", &[]),
            "FILE~1.TXT|   +++   .txt"
        );

        let file_tbl = get_table(sec, "File");
        assert_eq!(file_tbl.records.len(), 2);
        for rec in &file_tbl.records {
            let fname = rec.get(2).map(ToString::to_string).unwrap_or_default();
            assert!(!fname.contains('/'));
            assert!(!fname.contains('\\'));
        }
        assert_eq!(
            file_tbl.records[0].get(2),
            Some(&FieldValue::String(
                "MY_LON~1.SH|my_long_script_name.sh".to_string()
            ))
        );
        assert_eq!(
            file_tbl.records[1].get(2),
            Some(&FieldValue::String(
                "MY_LON~2.SH|my_long_script_two.sh".to_string()
            ))
        );

        let ctrl_tbl = get_table(sec, "Control");
        assert_eq!(ctrl_tbl.records.len(), 12);
        assert_eq!(
            ctrl_tbl.records[2].get(9),
            Some(&FieldValue::String(
                "!(bindpath.rtf:missing_deferred.txt)".to_string()
            ))
        );
        assert_eq!(
            ctrl_tbl.records[3].get(9),
            Some(&FieldValue::String("missing_non_scroll.txt".to_string()))
        );
        let s5 = ctrl_tbl.records[4]
            .get(9)
            .map(ToString::to_string)
            .unwrap_or_default();
        assert!(s5.starts_with(r"{\rtf1\ansi\ansicpg1252"));
        let s8 = ctrl_tbl.records[7]
            .get(9)
            .map(ToString::to_string)
            .unwrap_or_default();
        assert!(s8.starts_with(r"{\rtf1\ansi\ansicpg1252"));

        let _ = fs::remove_dir_all(&temp_dir);
    }
}

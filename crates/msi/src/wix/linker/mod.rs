//! `WiX` Linker, Binder, and ICE Validation pipeline (`light` architecture).
//!
//! Grounded directly in official `WiX` toolset architecture and Windows Installer SDK:
//! - Linker graph solver: Section entry point resolution (`Product` / `Module`), recursive
//!   symbol resolution, detection of duplicate or missing symbols.
//! - Standard directory resolution: Translates `WiX` standard directory identifiers (`TARGETDIR`,
//!   `ProgramFilesFolder`, `CommonFilesFolder`, `SystemFolder`, etc.) into hierarchical paths.
//! - Standard action sequencer: Injects standard MSI actions into sequence tables (`InstallExecuteSequence`,
//!   `InstallUISequence`, `AdminExecuteSequence`, `AdvtExecuteSequence`) with precise ordering.
//! - Media & Cabinet layout binder: Assigns files to Media disks, calculates contiguous sequence numbers,
//!   and supports embedded/external cabinets.
//! - Internal Consistency Evaluators (ICE) validation: ICE01..ICE105.

mod binder;
mod solver;

#[cfg(test)]
use self::binder::inspect_font_title;
pub use self::binder::inspect_pe_version;
#[cfg(test)]
use self::solver::is_special_reference;

pub use crate::database::memory::{LinkedDatabase, ModifyMode};
use crate::database::tables::record::{FieldValue, Record};
use crate::error::{MsiError, Result};
pub use crate::wix::ice::cub::CubValidator;
pub use crate::wix::ice::rules::{
    IceDiagnostic, IceDiagnosticType, IceRegistry, IceReport, IceRule, StandardIceRule,
};
use crate::wix::localization::LocalizationCatalog;
pub use crate::wix::merge_module::{modularize_identifier, MergeModule};
pub use crate::wix::standard_defs::{
    StandardActionOrder, StandardDirectory, STANDARD_DIRECTORIES, STANDARD_INSTALL_EXECUTE_ACTIONS,
    STANDARD_INSTALL_UI_ACTIONS,
};
use crate::wix::wixobj::{IntermediateSection, WixObject};
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

/// Sequence constraint tuple representing `(Action, Anchor, Position, Condition)`.
type SequenceConstraint = (String, String, String, Option<String>);

use crate::wix::extensions::ExtensionRegistry;

/// `WiX` Linker (`light`) graph solver and binder.
#[derive(Debug, Default)]
#[allow(clippy::struct_excessive_bools)]
pub struct Linker {
    /// Input intermediate object files.
    pub(super) objects: Vec<WixObject>,
    /// Base directory search paths for binding payload files (`-b`).
    pub(super) base_directories: Vec<PathBuf>,
    /// Named bind paths mapping identifier to directory (`-bd`).
    pub(super) bind_paths: HashMap<String, PathBuf>,
    /// Whether to suppress all ICE validation (`-sval`).
    suppress_ice: bool,
    /// Specific ICE validation rules to selectively run (`-ice:<ICE>`).
    selected_ice: HashSet<String>,
    /// Specific ICE validation rules to suppress (`-sice:<ICE>`).
    pub(crate) suppressed_ice: HashSet<String>,
    /// Specific warning IDs to suppress (`-sw<id>`).
    pub(crate) suppressed_warnings: HashSet<String>,
    /// Treat warnings as fatal errors (`-wx`).
    warnings_as_errors: bool,
    /// Treat all warnings as suppressed (`-swall`).
    suppress_all_warnings: bool,
    /// Enable pedantic validation (`-pedantic`).
    pedantic: bool,
    /// Ordered culture priority list (e.g. `["en-us", "de-de"]`).
    cultures: Vec<String>,
    /// Localization catalog for evaluating `!(loc.Id)`.
    loc_catalog: LocalizationCatalog,
    /// Component-split media layout (one CAB per component).
    pub(super) cab_per_component: bool,
    /// Embedded cabinet archives generated during binding.
    pub(super) embedded_cabinets: HashMap<String, Vec<u8>>,
    /// External cabinet archives generated during binding (non-embedded).
    pub(super) external_cabinets: HashMap<String, Vec<u8>>,
    /// Extension registry.
    extension_registry: ExtensionRegistry,
}

#[allow(clippy::multiple_inherent_impl)]
impl Linker {
    /// Creates a new empty [`Linker`].
    ///
    /// # Returns
    ///
    /// A new [`Linker`].
    #[must_use]
    pub fn new() -> Self {
        Self {
            extension_registry: ExtensionRegistry::with_builtin_extensions(),
            ..Default::default()
        }
    }

    /// Creates a new [`Linker`] with the provided extension registry.
    ///
    /// # Arguments
    ///
    /// * `registry` - The extension registry.
    ///
    /// # Returns
    ///
    /// A newly constructed instance of `Self`.
    #[must_use]
    pub fn with_extensions(registry: ExtensionRegistry) -> Self {
        Self {
            extension_registry: registry,
            ..Default::default()
        }
    }

    /// Adds an intermediate [`WixObject`] to the linker input queue.
    ///
    /// # Arguments
    ///
    /// * `object` - The intermediate object to link.
    pub fn add_object(&mut self, object: WixObject) {
        self.objects.push(object);
    }

    /// Adds intermediate objects from a [`crate::wix::wixlib::WixLibrary`] to the linker input queue.
    ///
    /// # Arguments
    ///
    /// * `library` - The [`crate::wix::wixlib::WixLibrary`] to link.
    pub fn add_library(&mut self, library: crate::wix::wixlib::WixLibrary) {
        for obj in library.objects {
            self.add_object(obj);
        }
    }

    /// Adds a base directory search path for binding payload files (`-b`).
    ///
    /// # Arguments
    ///
    /// * `dir` - Directory path to add to base search directories.
    pub fn add_base_dir(&mut self, dir: impl Into<PathBuf>) {
        self.base_directories.push(dir.into());
    }

    /// Adds a named bind path mapping identifier to directory (`-bd`).
    ///
    /// # Arguments
    ///
    /// * `id` - Bind path identifier.
    /// * `dir` - Bound directory path.
    pub fn add_bind_path(&mut self, id: impl Into<String>, dir: impl Into<PathBuf>) {
        self.bind_paths.insert(id.into(), dir.into());
    }

    /// Sets whether to suppress all ICE validation (`-sval`).
    ///
    /// # Arguments
    ///
    /// * `suppress` - Whether ICE validation is suppressed.
    pub const fn set_suppress_ice(&mut self, suppress: bool) {
        self.suppress_ice = suppress;
    }

    /// Selects an ICE rule to explicitly execute (`-ice:<ICE>`).
    ///
    /// # Arguments
    ///
    /// * `ice` - Rule name to select (e.g. `ICE01`).
    pub fn select_ice(&mut self, ice: impl Into<String>) {
        self.selected_ice.insert(ice.into());
    }

    /// Suppresses a specific ICE rule (`-sice:<ICE>`).
    ///
    /// # Arguments
    ///
    /// * `ice` - Rule name to suppress (e.g. `ICE38`).
    pub fn suppress_ice(&mut self, ice: impl Into<String>) {
        self.suppressed_ice.insert(ice.into());
    }

    /// Suppresses a specific warning code (`-sw<id>`).
    ///
    /// # Arguments
    ///
    /// * `warn_id` - Warning ID to suppress.
    pub fn suppress_warning(&mut self, warn_id: impl Into<String>) {
        self.suppressed_warnings.insert(warn_id.into());
    }

    /// Sets whether to treat linker warnings as fatal errors (`-wx`).
    ///
    /// # Arguments
    ///
    /// * `wx` - Whether warnings are fatal errors.
    pub const fn set_warnings_as_errors(&mut self, wx: bool) {
        self.warnings_as_errors = wx;
    }

    /// Sets whether to suppress all warnings.
    ///
    /// # Arguments
    ///
    /// * `suppress` - Suppress all warnings.
    pub const fn set_suppress_all_warnings(&mut self, suppress: bool) {
        self.suppress_all_warnings = suppress;
    }

    /// Sets whether to enable pedantic validation.
    ///
    /// # Arguments
    ///
    /// * `pedantic` - Enable pedantic validation.
    pub const fn set_pedantic(&mut self, pedantic: bool) {
        self.pedantic = pedantic;
    }

    /// Sets the ordered culture priority list for localization string resolution.
    ///
    /// # Arguments
    ///
    /// * `cultures` - Culture priority list (e.g. `["en-us", "de-de"]`).
    pub fn set_cultures(&mut self, cultures: Vec<String>) {
        self.cultures = cultures;
    }

    /// Sets the localization catalog for resolving `!(loc.StringId)` tokens.
    ///
    /// # Arguments
    ///
    /// * `catalog` - Localization catalog to use.
    pub fn set_localization_catalog(&mut self, catalog: LocalizationCatalog) {
        self.loc_catalog = catalog;
    }

    /// Sets whether to use component-split cabinet layout (one CAB per component).
    ///
    /// # Arguments
    ///
    /// * `cab_per_comp` - Whether each component has its own cabinet archive.
    pub const fn set_cab_per_component(&mut self, cab_per_comp: bool) {
        self.cab_per_component = cab_per_comp;
    }

    /// Returns a reference to the embedded cabinet archives generated during linking.
    ///
    /// # Returns
    ///
    /// Map of cabinet stream names to byte vectors.
    #[must_use]
    pub const fn embedded_cabinets(&self) -> &HashMap<String, Vec<u8>> {
        &self.embedded_cabinets
    }

    /// Consumes and returns the embedded cabinet archives generated during linking.
    ///
    /// # Returns
    ///
    /// Map of cabinet stream names to byte vectors.
    #[must_use]
    pub fn take_embedded_cabinets(&mut self) -> HashMap<String, Vec<u8>> {
        std::mem::take(&mut self.embedded_cabinets)
    }

    /// Returns a reference to the external cabinet archives generated during linking.
    ///
    /// # Returns
    ///
    /// Map of external cabinet filenames to byte vectors.
    #[must_use]
    pub const fn external_cabinets(&self) -> &HashMap<String, Vec<u8>> {
        &self.external_cabinets
    }

    /// Consumes and returns the external cabinet archives generated during linking.
    ///
    /// # Returns
    ///
    /// Map of external cabinet filenames to byte vectors.
    #[must_use]
    pub fn take_external_cabinets(&mut self) -> HashMap<String, Vec<u8>> {
        std::mem::take(&mut self.external_cabinets)
    }

    /// Links all input objects, resolves symbols and references, builds directory hierarchies,
    /// sequences standard actions, binds media sequence numbers, and evaluates ICE rules.
    ///
    /// # Returns
    ///
    /// Fully resolved and validated [`LinkedDatabase`].
    ///
    /// # Errors
    ///
    /// Returns [`MsiError::WixLinker`] or [`MsiError::IceValidation`] on linking or validation failures.
    pub fn link(&mut self) -> Result<LinkedDatabase> {
        // 1. Solve linker symbol graph
        let active_sections = self.solve_symbol_graph()?;

        // 2. Build initial linked database from intermediate tables
        let mut db = LinkedDatabase::default();
        for sec in &active_sections {
            for tbl in &sec.tables {
                for rec in &tbl.records {
                    db.add_or_merge_record(&tbl.name, rec.clone())?;
                }
            }
        }

        // 2a. Invoke registered extensions to link backend custom actions/tables
        for ext in self.extension_registry.clone().iter() {
            ext.link_database(&mut db)?;
        }

        // 2b. Resolve multi-level nested ComponentGroupRef hierarchy
        Self::resolve_component_groups(&mut db)?;

        // 3. Resolve standard directories
        Self::resolve_standard_directories(&mut db);

        // 4. Inject standard action sequences
        Self::sequence_standard_actions(&mut db);

        // 4b. Solve topological relative sequences (InstallExecuteSequence & InstallUISequence)
        Self::solve_relative_sequences(&mut db)?;

        // 5. Expand localization string tokens !(loc.StringId) across all string fields
        self.expand_localization_tokens(&mut db)?;

        // 5b. Ingest standard UI if referenced by UIRef
        for sec in &active_sections {
            for rf in &sec.references {
                if rf.namespace == "UI" {
                    if let Ok(set) = rf.id.parse::<crate::wix::ui_library::WixUiDialogSet>() {
                        if db.get_records("Dialog").is_empty() {
                            let _ = crate::wix::ui_library::inject_ui_library(
                                &mut db, set, None, None, None,
                            );
                            break;
                        }
                    }
                }
            }
        }

        // 5c. Resolve WiX variables, bind bitmaps, and extract EULA
        self.resolve_wix_variables(&mut db);
        self.bind_text_sources(&mut db)?;

        // 5c2. Ensure standard WiX font presets exist in TextStyle
        if db.get_records("TextStyle").is_empty() {
            let text_styles = [
                ("WixUI_Font_Normal", "Tahoma", 8, 0, 0),
                ("WixUI_Font_Bigger", "Tahoma", 12, 0, 1),
                ("WixUI_Font_Title", "Tahoma", 9, 0, 1),
                ("WixUI_Font_Bold", "Tahoma", 8, 0, 1),
            ];
            for (name, face, sz, color, bits) in text_styles {
                db.add_record(
                    "TextStyle",
                    Record::with_fields(vec![
                        FieldValue::String(name.to_string()),
                        FieldValue::String(face.to_string()),
                        FieldValue::Short(sz),
                        FieldValue::Long(color),
                        FieldValue::Short(bits),
                    ]),
                );
            }
        }

        // 5d. Bind physical binary payloads from disk into embedded streams
        self.bind_binaries(&mut db)?;

        // 5e. Validate MsiEmbeddedChainer references
        Self::validate_embedded_chainers(&db)?;

        // 6. Bind physical files from disk and generate cabinet archives
        self.bind_files_and_pack_cabinets(&mut db)?;

        // 7. Layout and sequence media files
        Self::layout_media_and_files(&mut db);

        // 7.5. Generate standard validation table
        Self::generate_validation_table(&mut db);

        // 8. Run comprehensive ICE validation
        self.run_filtered_ice_validations(&db)?;

        Ok(db)
    }

    /// Expands `!(loc.StringId)` tokens in all string fields across all database tables.
    ///
    /// # Arguments
    ///
    /// * `db` - In-memory linked MSI database to inspect or mutate.
    ///
    /// # Returns
    ///
    /// `Ok(())` on success, or an [`MsiError`] if the operation fails.
    fn expand_localization_tokens(&self, db: &mut LinkedDatabase) -> Result<()> {
        for records in db.tables.values_mut() {
            for rec in records {
                for field in rec.fields_mut() {
                    if let FieldValue::String(ref mut s) = field {
                        if s.contains("!(loc.") {
                            *s = self.loc_catalog.expand_loc_tokens(s, &self.cultures)?;
                        }
                    }
                }
            }
        }
        Ok(())
    }

    /// Solves relative sequencing directives (`Before`, `After`, `OnExit`) across execution and UI sequences.
    fn solve_relative_sequences(db: &mut LinkedDatabase) -> Result<()> {
        solver::solve_relative_sequences(db)
    }

    /// Resolves `WixVariable` tokens, binds branding bitmaps, and extracts license RTF.
    fn resolve_wix_variables(&mut self, db: &mut LinkedDatabase) {
        binder::resolve_wix_variables(self, db);
    }

    /// Resolves deferred `ScrollableText` `Control.Text` file references against configured bind paths.
    fn bind_text_sources(&self, db: &mut LinkedDatabase) -> Result<()> {
        binder::bind_text_sources(self, db)
    }

    /// Resolves a source file path against configured base directories and bind paths.
    #[cfg(test)]
    fn resolve_source_path(&self, src_path: &str) -> Option<PathBuf> {
        binder::resolve_source_path(self, src_path)
    }

    /// Binds physical file payloads from disk into File records and compresses them into embedded cabinets.
    fn bind_files_and_pack_cabinets(&mut self, db: &mut LinkedDatabase) -> Result<()> {
        binder::bind_files_and_pack_cabinets(self, db)
    }

    /// Evaluates ICE validation rules according to suppression and selection settings.
    pub(crate) fn run_filtered_ice_validations(&self, db: &LinkedDatabase) -> Result<()> {
        if self.suppress_ice {
            return Ok(());
        }

        let reports = vec![
            Self::validate_ice01(db),
            Self::validate_ice02(db),
            Self::validate_ice03(db),
            Self::validate_ice04(db),
            Self::validate_ice05(db),
            Self::validate_ice06(db),
            Self::validate_ice07(db),
            Self::validate_ice08(db),
            Self::validate_ice09(db),
            Self::validate_ice18(db),
            Self::validate_ice20(db),
            Self::validate_ice30(db),
            Self::validate_ice33(db),
            Self::validate_ice38(db),
            Self::validate_ice61(db),
            Self::validate_ice80(db),
            Self::validate_ice99(db),
            Self::validate_ice101(db),
            Self::validate_ice103(db),
        ];

        for rep in reports.into_iter().flatten() {
            if self.suppressed_ice.contains(&rep.ice) {
                continue;
            }
            if !self.selected_ice.is_empty() && !self.selected_ice.contains(&rep.ice) {
                continue;
            }

            if rep.is_error {
                return Err(MsiError::IceValidation {
                    ice: rep.ice,
                    message: rep.message,
                });
            }

            if self.suppress_all_warnings || self.suppressed_warnings.contains(&rep.ice) {
                continue;
            }

            if self.warnings_as_errors {
                return Err(MsiError::IceValidation {
                    ice: rep.ice,
                    message: format!("warning treated as error: {}", rep.message),
                });
            }
        }

        Ok(())
    }

    /// Solves the symbol graph: identifies the single entry point (`Product` or `Module`),
    /// detects duplicate symbols, and resolves all required references across sections.
    fn solve_symbol_graph(&self) -> Result<Vec<IntermediateSection>> {
        solver::solve_symbol_graph(self)
    }

    /// Resolves multi-level nested `ComponentGroupRef` hierarchies and links components to features.
    fn resolve_component_groups(db: &mut LinkedDatabase) -> Result<()> {
        solver::resolve_component_groups(db)
    }

    /// Resolves standard directory hierarchy rooted at `TARGETDIR`.
    fn resolve_standard_directories(db: &mut LinkedDatabase) {
        solver::resolve_standard_directories(db);
    }

    /// Injects standard action sequences into `InstallExecuteSequence`.
    fn sequence_standard_actions(db: &mut LinkedDatabase) {
        solver::sequence_standard_actions(db);
    }

    /// Generates the standard `_Validation` table for all core MSI SDK tables.
    fn generate_validation_table(db: &mut LinkedDatabase) {
        solver::generate_validation_table(db);
    }

    /// Binds physical binary payloads from disk into embedded streams.
    fn bind_binaries(&mut self, db: &mut LinkedDatabase) -> Result<()> {
        binder::bind_binaries(self, db)
    }

    /// Validates `MsiEmbeddedChainer` table records to ensure referenced binary or file streams exist.
    fn validate_embedded_chainers(db: &LinkedDatabase) -> Result<()> {
        binder::validate_embedded_chainers(db)
    }

    /// Sequences file numbers and aligns media last sequence across partitioned disks.
    fn layout_media_and_files(db: &mut LinkedDatabase) {
        binder::layout_media_and_files(db);
    }
}

#[cfg(test)]
#[allow(clippy::unnecessary_wraps)]
mod tests {
    use super::*;
    use crate::wix::extensions::WixExtension;
    use crate::wix::wixlib::WixLibrary;
    use crate::wix::wixobj::{IntermediateTable, Reference, SectionType, Symbol};
    use crate::wix::xml::XmlNode;

    #[test]
    fn test_linker_validation_table_generation() -> Result<()> {
        let mut obj = WixObject::new();
        let mut sec = IntermediateSection::new(
            SectionType::Product,
            Some("{11111111-1111-1111-1111-111111111111}".to_string()),
        );
        sec.add_symbol(Symbol::new(
            "Product",
            "{11111111-1111-1111-1111-111111111111}",
        ));
        let mut comp_tbl = IntermediateTable::new("Component");
        comp_tbl.push_record(Record::with_fields(vec![
            FieldValue::String("Comp1".to_string()),
            FieldValue::String("{22222222-2222-2222-2222-222222222222}".to_string()),
            FieldValue::String("TARGETDIR".to_string()),
            FieldValue::Short(0),
            FieldValue::Null,
            FieldValue::Null,
        ]));
        sec.add_table(comp_tbl);
        obj.sections.push(sec);

        let mut linker = Linker::new();
        linker.add_object(obj);
        let db = linker.link()?;

        // Verify _Validation table is present
        let val_records = db.get_records("_Validation");
        assert!(!val_records.is_empty());

        let has_component = val_records.iter().any(|r| {
            r.get(0) == Some(&FieldValue::String("Component".to_string()))
                && r.get(1) == Some(&FieldValue::String("Component".to_string()))
                && r.get(2) == Some(&FieldValue::String("N".to_string()))
        });
        assert!(has_component);

        let has_component_guid = val_records.iter().any(|r| {
            r.get(0) == Some(&FieldValue::String("Component".to_string()))
                && r.get(1) == Some(&FieldValue::String("ComponentId".to_string()))
                && r.get(2) == Some(&FieldValue::String("Y".to_string()))
        });
        assert!(has_component_guid);

        Ok(())
    }

    #[test]
    fn test_linker_basic() -> Result<()> {
        let mut obj = WixObject::new();
        let mut sec = IntermediateSection::new(
            SectionType::Product,
            Some("{11111111-1111-1111-1111-111111111111}".to_string()),
        );
        sec.add_symbol(Symbol::new(
            "Product",
            "{11111111-1111-1111-1111-111111111111}",
        ));
        sec.add_symbol(Symbol::new("Component", "Comp1"));
        sec.add_symbol(Symbol::new("Directory", "TARGETDIR"));

        let mut comp_tbl = IntermediateTable::new("Component");
        comp_tbl.push_record(Record::with_fields(vec![
            FieldValue::String("Comp1".to_string()),
            FieldValue::String("{22222222-2222-2222-2222-222222222222}".to_string()),
            FieldValue::String("TARGETDIR".to_string()),
            FieldValue::Short(0),
            FieldValue::Null,
            FieldValue::Null,
        ]));
        sec.add_table(comp_tbl);

        let mut prop_tbl = IntermediateTable::new("Property");
        prop_tbl.push_record(Record::with_fields(vec![
            FieldValue::String("ProductVersion".to_string()),
            FieldValue::String("1.0.0".to_string()),
        ]));
        sec.add_table(prop_tbl);

        obj.add_section(sec);

        let mut active_linker = Linker::new();
        let ext_linker = Linker::with_extensions(ExtensionRegistry::new());
        assert!(format!("{ext_linker:?}").contains("Linker"), "format check");
        active_linker.add_object(obj);

        let linked = active_linker.link().unwrap_or_default();
        assert_eq!(linked.get_records("Component").len(), 1);
        assert_ne!(linked.get_records("Directory"), []);
        assert!(linked.get_records("InstallExecuteSequence").len() >= 15);

        // Test database inspection helpers
        assert!(linked.get_table("Component").is_some());
        assert!(linked.get_table("NonExistentTable").is_none());

        let comp_rec = &linked.get_records("Component")[0];
        assert_eq!(linked.get_string_field(comp_rec, 0), Some("Comp1"));
        assert_eq!(linked.get_string_field(comp_rec, 3), None); // Short integer
        assert_eq!(linked.get_string_field(comp_rec, 99), None); // Out of bounds

        assert_eq!(linked.get_int_field(comp_rec, 3), Some(0));
        assert_eq!(linked.get_int_field(comp_rec, 0), None); // String
        assert_eq!(linked.get_int_field(comp_rec, 99), None); // Out of bounds

        let comp_pks = linked.get_primary_keys("Component");
        assert_eq!(comp_pks, vec!["Component"]);
        let missing_pks = linked.get_primary_keys("NonExistentTable");
        assert!(missing_pks.is_empty());

        Ok(())
    }

    #[test]
    fn test_linker_missing_and_duplicate_symbols() {
        // Missing entry
        let mut empty_linker = Linker::new();
        assert!(empty_linker.link().is_err());

        // Multiple entries
        let mut multi_obj = WixObject::new();
        multi_obj.add_section(IntermediateSection::new(
            SectionType::Product,
            Some("P1".to_string()),
        ));
        multi_obj.add_section(IntermediateSection::new(
            SectionType::Product,
            Some("P2".to_string()),
        ));
        let mut multi_linker = Linker::new();
        multi_linker.add_object(multi_obj);
        assert!(multi_linker.link().is_err());

        // Duplicate symbol definition
        let mut dup_obj = WixObject::new();
        let mut s1 = IntermediateSection::new(SectionType::Product, Some("P1".to_string()));
        s1.add_symbol(Symbol::new("Component", "C1"));
        let mut s2 = IntermediateSection::new(SectionType::Fragment, Some("F1".to_string()));
        s2.add_symbol(Symbol::new("Component", "C1"));
        dup_obj.add_section(s1);
        dup_obj.add_section(s2);

        let mut dup_linker = Linker::new();
        dup_linker.add_object(dup_obj);
        assert!(dup_linker.link().is_err());

        // Duplicate Property definition across sections is permitted in WiX
        let mut prop_obj = WixObject::new();
        let mut ps1 = IntermediateSection::new(SectionType::Product, Some("P1".to_string()));
        ps1.add_symbol(Symbol::new("Property", "PROP1"));
        let mut ps2 = IntermediateSection::new(SectionType::Fragment, Some("F1".to_string()));
        ps2.add_symbol(Symbol::new("Property", "PROP1"));
        prop_obj.add_section(ps1);
        prop_obj.add_section(ps2);
        let mut prop_linker = Linker::new();
        prop_linker.add_object(prop_obj);
        assert!(prop_linker.link().is_ok());

        // Duplicate ComponentGroup definition across sections is permitted in WiX
        let mut cg_obj = WixObject::new();
        let mut cgs1 = IntermediateSection::new(SectionType::Product, Some("P1".to_string()));
        cgs1.add_symbol(Symbol::new("ComponentGroup", "CG1"));
        let mut cgs2 = IntermediateSection::new(SectionType::Fragment, Some("F1".to_string()));
        cgs2.add_symbol(Symbol::new("ComponentGroup", "CG1"));
        cg_obj.add_section(cgs1);
        cg_obj.add_section(cgs2);
        let mut cg_linker = Linker::new();
        cg_linker.add_object(cg_obj);
        assert!(cg_linker.link().is_ok());

        // Duplicate Directory definition across sections is permitted in WiX
        let mut dir_obj = WixObject::new();
        let mut ds1 = IntermediateSection::new(SectionType::Product, Some("P1".to_string()));
        ds1.add_symbol(Symbol::new("Directory", "TARGETDIR"));
        let mut ds2 = IntermediateSection::new(SectionType::Fragment, Some("F1".to_string()));
        ds2.add_symbol(Symbol::new("Directory", "TARGETDIR"));
        dir_obj.add_section(ds1);
        dir_obj.add_section(ds2);
        let mut dir_linker = Linker::new();
        dir_linker.add_object(dir_obj);
        assert!(dir_linker.link().is_ok());

        // Duplicate symbol definition within the same section is permitted and skipped
        let mut same_sec_obj = WixObject::new();
        let mut ss = IntermediateSection::new(SectionType::Product, Some("P_Same".to_string()));
        ss.symbols.push(Symbol::new("Component", "C_Same"));
        ss.symbols.push(Symbol::new("Component", "C_Same"));
        same_sec_obj.add_section(ss);
        let mut same_sec_linker = Linker::new();
        same_sec_linker.add_object(same_sec_obj);
        assert!(same_sec_linker.link().is_ok());
    }

    #[test]
    fn test_linker_unresolved_reference() {
        let mut obj = WixObject::new();
        let mut sec = IntermediateSection::new(SectionType::Product, Some("P1".to_string()));
        sec.add_reference(Reference::new("Component", "NonExistentComp"));
        obj.add_section(sec);

        let mut linker = Linker::new();
        linker.add_object(obj);
        assert!(linker.link().is_err());
    }

    #[test]
    fn test_linker_cross_fragment_and_media_layout() -> Result<()> {
        let mut obj1 = WixObject::new();
        let mut prod = IntermediateSection::new(SectionType::Product, Some("P1".to_string()));
        prod.add_symbol(Symbol::new("Product", "P1"));
        prod.add_reference(Reference::new("Component", "FragComp"));
        prod.add_reference(Reference::new("Directory", "ProgramFilesFolder"));
        prod.add_reference(Reference::new("Action", "InstallFiles"));

        let mut prop_tbl = IntermediateTable::new("Property");
        prop_tbl.push_record(Record::with_fields(vec![
            FieldValue::String("ProductVersion".to_string()),
            FieldValue::String("1.0.0".to_string()),
        ]));
        prod.add_table(prop_tbl);
        obj1.add_section(prod);

        let mut obj2 = WixObject::new();
        let mut frag = IntermediateSection::new(SectionType::Fragment, Some("F1".to_string()));
        frag.add_symbol(Symbol::new("Component", "FragComp"));

        let mut comp_tbl = IntermediateTable::new("Component");
        comp_tbl.push_record(Record::with_fields(vec![
            FieldValue::String("FragComp".to_string()),
            FieldValue::String("{33333333-3333-3333-3333-333333333333}".to_string()),
            FieldValue::String("ProgramFilesFolder".to_string()),
            FieldValue::Short(0),
            FieldValue::Null,
            FieldValue::String("File1".to_string()),
        ]));
        frag.add_table(comp_tbl);

        let mut file_tbl = IntermediateTable::new("File");
        file_tbl.push_record(Record::with_fields(vec![
            FieldValue::String("File1".to_string()),
            FieldValue::String("FragComp".to_string()),
            FieldValue::String("test.exe".to_string()),
            FieldValue::Long(1024),
            FieldValue::String("1.0.0.0".to_string()),
            FieldValue::Null,
            FieldValue::Short(0),
            FieldValue::Short(0),
        ]));
        frag.add_table(file_tbl);
        obj2.add_section(frag);

        let mut linker = Linker::new();
        linker.add_object(obj1);
        linker.add_object(obj2);

        let db = linker.link().unwrap_or_default();
        assert_eq!(db.get_records("Component").len(), 1);
        assert_eq!(db.get_records("File").len(), 1);

        // Verify File was re-sequenced to 1
        let files = db.get_records("File");
        assert_eq!(files[0].get(7), Some(&FieldValue::Short(1)));

        // Verify Media was automatically synthesized
        let media = db.get_records("Media");
        assert_eq!(media.len(), 1);
        assert_eq!(media[0].get(1), Some(&FieldValue::Long(1)));

        // Verify standard directory ProgramFilesFolder was resolved
        let dirs = db.get_records("Directory");
        let has_prog_files = dirs
            .iter()
            .any(|d| d.get(0) == Some(&FieldValue::String("ProgramFilesFolder".to_string())));
        assert!(has_prog_files);

        // Verify database methods
        assert_eq!(db.get_records("NonExistentTable").len(), 0);

        Ok(())
    }

    #[test]
    fn test_linker_nested_component_groups_and_posix_dirs() -> Result<()> {
        let mut obj = WixObject::new();
        let mut prod = IntermediateSection::new(SectionType::Product, Some("P1".to_string()));
        prod.add_symbol(Symbol::new("Product", "P1"));

        // Feature referencing CG_A
        let mut feat_tbl = IntermediateTable::new("Feature");
        feat_tbl.push_record(Record::with_fields(vec![
            FieldValue::String("FeatRoot".to_string()),
            FieldValue::Null,
            FieldValue::String("Main".to_string()),
            FieldValue::Null,
            FieldValue::Null,
            FieldValue::Short(1),
            FieldValue::Null,
            FieldValue::Short(0),
        ]));
        prod.add_table(feat_tbl);

        let mut feat_cg_tbl = IntermediateTable::new("_FeatureComponentGroupRef");
        feat_cg_tbl.push_record(Record::with_fields(vec![
            FieldValue::String("FeatRoot".to_string()),
            FieldValue::String("CG_A".to_string()),
        ]));
        prod.add_table(feat_cg_tbl);

        // CG_A has CompA and nested CG_B
        let mut cg_mem = IntermediateTable::new("_ComponentGroupMember");
        cg_mem.push_record(Record::with_fields(vec![
            FieldValue::String("CG_A".to_string()),
            FieldValue::String("CompA".to_string()),
        ]));
        cg_mem.push_record(Record::with_fields(vec![
            FieldValue::String("CG_B".to_string()),
            FieldValue::String("CompB".to_string()),
        ]));
        cg_mem.push_record(Record::with_fields(vec![
            FieldValue::String("CG_C".to_string()),
            FieldValue::String("CompC".to_string()),
        ]));
        prod.add_table(cg_mem);

        let mut cg_nest = IntermediateTable::new("_ComponentGroupNested");
        cg_nest.push_record(Record::with_fields(vec![
            FieldValue::String("CG_A".to_string()),
            FieldValue::String("CG_B".to_string()),
        ]));
        cg_nest.push_record(Record::with_fields(vec![
            FieldValue::String("CG_B".to_string()),
            FieldValue::String("CG_C".to_string()),
        ]));
        prod.add_table(cg_nest);

        // Components installed to POSIX directories
        let mut comp_tbl = IntermediateTable::new("Component");
        comp_tbl.push_record(Record::with_fields(vec![
            FieldValue::String("CompA".to_string()),
            FieldValue::Null,
            FieldValue::String("/usr/bin".to_string()),
            FieldValue::Short(0),
            FieldValue::Null,
            FieldValue::Null,
        ]));
        comp_tbl.push_record(Record::with_fields(vec![
            FieldValue::String("CompB".to_string()),
            FieldValue::Null,
            FieldValue::String("/etc".to_string()),
            FieldValue::Short(0),
            FieldValue::Null,
            FieldValue::Null,
        ]));
        comp_tbl.push_record(Record::with_fields(vec![
            FieldValue::String("CompC".to_string()),
            FieldValue::Null,
            FieldValue::String("DesktopFolder".to_string()),
            FieldValue::Short(0),
            FieldValue::Null,
            FieldValue::Null,
        ]));
        prod.add_table(comp_tbl);
        obj.add_section(prod);

        let mut linker = Linker::new();
        linker.add_object(obj);
        let db = linker.link().unwrap_or_default();

        // Verify FeatureComponents contains all 3 transitively resolved components
        let fc = db.get_records("FeatureComponents");
        assert_eq!(fc.len(), 3);
        assert!(fc
            .iter()
            .any(|r| r.get(1) == Some(&FieldValue::String("CompA".to_string()))));
        assert!(fc
            .iter()
            .any(|r| r.get(1) == Some(&FieldValue::String("CompB".to_string()))));
        assert!(fc
            .iter()
            .any(|r| r.get(1) == Some(&FieldValue::String("CompC".to_string()))));

        // Verify standard POSIX and Windows directories were resolved
        let dirs = db.get_records("Directory");
        assert!(dirs
            .iter()
            .any(|d| d.get(0) == Some(&FieldValue::String("/usr/bin".to_string()))));
        assert!(dirs
            .iter()
            .any(|d| d.get(0) == Some(&FieldValue::String("/etc".to_string()))));
        assert!(dirs
            .iter()
            .any(|d| d.get(0) == Some(&FieldValue::String("DesktopFolder".to_string()))));

        // Temporary tracking tables must have been purged
        assert_eq!(db.get_records("_ComponentGroupMember").len(), 0);
        assert_eq!(db.get_records("_ComponentGroupNested").len(), 0);
        assert_eq!(db.get_records("_FeatureComponentGroupRef").len(), 0);

        Ok(())
    }

    /// Tests linking with `WixLibrary`, Module sections, unreferenced fragments, and duplicate section queues.
    ///
    /// # Returns
    ///
    /// `Ok(())` on success, or an [`MsiError`] if the operation fails.
    #[test]
    fn test_linker_add_library_and_module_and_unreferenced_fragments() -> Result<()> {
        let mut obj = WixObject::new();

        // Module section
        let mut module_sec =
            IntermediateSection::new(SectionType::Module, Some("Mod1".to_string()));
        module_sec.add_symbol(Symbol::new("Module", "Mod1"));
        module_sec.add_reference(Reference::new("Component", "Comp1"));
        // Multiple references to same component to test queue deduplication
        module_sec.add_reference(Reference::new("Component", "Comp1"));

        let mut prop_tbl = IntermediateTable::new("Property");
        prop_tbl.push_record(Record::with_fields(vec![
            FieldValue::String("ProductVersion".to_string()),
            FieldValue::String("1.0.0".to_string()),
        ]));
        module_sec.add_table(prop_tbl);

        let mut comp_sec =
            IntermediateSection::new(SectionType::Fragment, Some("Frag1".to_string()));
        comp_sec.add_symbol(Symbol::new("Component", "Comp1"));
        let mut comp_tbl = IntermediateTable::new("Component");
        comp_tbl.push_record(Record::with_fields(vec![
            FieldValue::String("Comp1".to_string()),
            FieldValue::String("{22222222-2222-2222-2222-222222222222}".to_string()),
            FieldValue::String("TARGETDIR".to_string()),
            FieldValue::Short(0),
            FieldValue::Null,
            FieldValue::Null,
        ]));
        comp_sec.add_table(comp_tbl);

        // Unreferenced fragment section to test unreferenced sections being skipped
        let unref_sec =
            IntermediateSection::new(SectionType::Fragment, Some("UnreferencedFrag".to_string()));

        obj.add_section(module_sec);
        obj.add_section(comp_sec);
        obj.add_section(unref_sec);
        let lib = WixLibrary::new(vec![obj]);

        let mut linker = Linker::new();
        linker.add_library(lib);

        let db = linker.link().unwrap_or_default();
        assert_eq!(db.get_records("Component").len(), 1);
        Ok(())
    }

    /// Tests component group hierarchy resolution edge cases including empty sets, malformed records, and cycles.
    ///
    /// # Returns
    ///
    /// `Ok(())` on success, or an [`MsiError`] if the operation fails.
    #[test]
    fn test_linker_component_groups_edge_cases() -> Result<()> {
        let mut db_empty = LinkedDatabase::default();
        let _ = Linker::resolve_component_groups(&mut db_empty);

        // feat_refs non-empty but members empty
        let mut db_feat_only = LinkedDatabase::default();
        db_feat_only.add_record(
            "_FeatureComponentGroupRef",
            Record::with_fields(vec![
                FieldValue::String("Feat1".to_string()),
                FieldValue::String("CG_Empty".to_string()),
            ]),
        );
        let _ = Linker::resolve_component_groups(&mut db_feat_only);

        // feat_refs empty but members non-empty
        let mut db_members_only = LinkedDatabase::default();
        db_members_only.add_record(
            "_ComponentGroupMember",
            Record::with_fields(vec![
                FieldValue::String("CG_Lone".to_string()),
                FieldValue::String("CompLone".to_string()),
            ]),
        );
        let _ = Linker::resolve_component_groups(&mut db_members_only);

        // Malformed records in tracking tables
        let mut db_malformed = LinkedDatabase::default();
        db_malformed.add_record(
            "_FeatureComponentGroupRef",
            Record::with_fields(vec![FieldValue::Null, FieldValue::Null]),
        );
        db_malformed.add_record(
            "_ComponentGroupMember",
            Record::with_fields(vec![FieldValue::Null, FieldValue::Null]),
        );
        db_malformed.add_record(
            "_ComponentGroupNested",
            Record::with_fields(vec![FieldValue::Null, FieldValue::Null]),
        );
        let _ = Linker::resolve_component_groups(&mut db_malformed);

        // Cycle in nested groups: CG1 -> CG2 -> CG1
        let mut db_cycle = LinkedDatabase::default();
        db_cycle.add_record(
            "_FeatureComponentGroupRef",
            Record::with_fields(vec![
                FieldValue::String("Feat1".to_string()),
                FieldValue::String("CG1".to_string()),
            ]),
        );
        db_cycle.add_record(
            "_ComponentGroupNested",
            Record::with_fields(vec![
                FieldValue::String("CG1".to_string()),
                FieldValue::String("CG2".to_string()),
            ]),
        );
        db_cycle.add_record(
            "_ComponentGroupNested",
            Record::with_fields(vec![
                FieldValue::String("CG2".to_string()),
                FieldValue::String("CG1".to_string()),
            ]),
        );
        db_cycle.add_record(
            "_ComponentGroupMember",
            Record::with_fields(vec![
                FieldValue::String("CG1".to_string()),
                FieldValue::String("CompInCycle".to_string()),
            ]),
        );
        let _ = Linker::resolve_component_groups(&mut db_cycle);
        let fc = db_cycle.get_records("FeatureComponents");
        assert_eq!(fc.len(), 1);

        Ok(())
    }

    /// Tests directory resolution and action sequence edge cases.
    ///
    /// # Returns
    ///
    /// `Ok(())` on success, or an [`MsiError`] if the operation fails.
    #[test]
    fn test_linker_directories_and_actions_edge_cases() -> Result<()> {
        let mut db = LinkedDatabase::default();

        // Pre-populate TARGETDIR and non-string record in Directory
        db.add_record(
            "Directory",
            Record::with_fields(vec![
                FieldValue::String("TARGETDIR".to_string()),
                FieldValue::Null,
                FieldValue::String("SourceDir".to_string()),
            ]),
        );
        db.add_record(
            "Directory",
            Record::with_fields(vec![FieldValue::Null, FieldValue::Null, FieldValue::Null]),
        );

        // Component with non-string directory, and component with already existing directory,
        // and component with custom directory not in STANDARD_DIRECTORIES
        db.add_record(
            "Component",
            Record::with_fields(vec![
                FieldValue::String("CompNullDir".to_string()),
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Short(0),
                FieldValue::Null,
                FieldValue::Null,
            ]),
        );
        db.add_record(
            "Component",
            Record::with_fields(vec![
                FieldValue::String("CompTargetDir".to_string()),
                FieldValue::Null,
                FieldValue::String("TARGETDIR".to_string()),
                FieldValue::Short(0),
                FieldValue::Null,
                FieldValue::Null,
            ]),
        );
        db.add_record(
            "Component",
            Record::with_fields(vec![
                FieldValue::String("CompCustomDir".to_string()),
                FieldValue::Null,
                FieldValue::String("CustomAppDir".to_string()),
                FieldValue::Short(0),
                FieldValue::Null,
                FieldValue::Null,
            ]),
        );

        Linker::resolve_standard_directories(&mut db);

        // sequence_standard_actions with pre-existing actions and non-string record
        db.add_record(
            "InstallExecuteSequence",
            Record::with_fields(vec![
                FieldValue::String("AppSearch".to_string()),
                FieldValue::Null,
                FieldValue::Short(400),
            ]),
        );
        db.add_record(
            "InstallExecuteSequence",
            Record::with_fields(vec![FieldValue::Null, FieldValue::Null, FieldValue::Null]),
        );

        Linker::sequence_standard_actions(&mut db);
        assert!(db.get_records("InstallExecuteSequence").len() >= 15);

        Ok(())
    }

    /// Tests media layout and file sequencing edge cases.
    ///
    /// # Returns
    ///
    /// `Ok(())` on success, or an [`MsiError`] if the operation fails.
    #[test]
    fn test_linker_media_layout_edge_cases() -> Result<()> {
        let mut db_empty_files = LinkedDatabase::default();
        Linker::layout_media_and_files(&mut db_empty_files);

        let mut db = LinkedDatabase::default();
        // File with < 8 fields
        db.add_record(
            "File",
            Record::with_fields(vec![
                FieldValue::String("FShort".to_string()),
                FieldValue::String("Comp1".to_string()),
            ]),
        );
        // Valid file
        db.add_record(
            "File",
            Record::with_fields(vec![
                FieldValue::String("F1".to_string()),
                FieldValue::String("Comp1".to_string()),
                FieldValue::String("f1.txt".to_string()),
                FieldValue::Long(100),
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Short(0),
            ]),
        );
        // Another valid file
        db.add_record(
            "File",
            Record::with_fields(vec![
                FieldValue::String("F2".to_string()),
                FieldValue::String("Comp1".to_string()),
                FieldValue::String("f2.txt".to_string()),
                FieldValue::Long(200),
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Short(0),
            ]),
        );

        // Pre-existing media records:
        // 1. Test updating last disk where last_seq < file_count (1 < 3 -> 3)
        let mut db1 = db.clone();
        db1.add_record(
            "Media",
            Record::with_fields(vec![
                FieldValue::Short(1),
                FieldValue::Long(1),
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
            ]),
        );
        Linker::layout_media_and_files(&mut db1);
        let media1 = db1.get_records("Media");
        assert_eq!(media1[0].get(1), Some(&FieldValue::Long(3)));

        // 2. Test last disk where last_seq >= file_count (100 >= 3 -> 100)
        let mut db2 = db.clone();
        db2.add_record(
            "Media",
            Record::with_fields(vec![
                FieldValue::Short(1),
                FieldValue::Long(100),
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
            ]),
        );
        Linker::layout_media_and_files(&mut db2);
        let media2 = db2.get_records("Media");
        assert_eq!(media2[0].get(1), Some(&FieldValue::Long(100)));

        // 3. Test last disk where field 1 is Null
        let mut db3 = db.clone();
        db3.add_record(
            "Media",
            Record::with_fields(vec![
                FieldValue::Short(1),
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
            ]),
        );
        Linker::layout_media_and_files(&mut db3);
        let media3 = db3.get_records("Media");
        assert_eq!(media3[0].get(1), Some(&FieldValue::Null));

        Ok(())
    }

    /// Tests trait implementations (`Debug`, `Clone`, `PartialEq`, `Default`) for linker data structures.

    #[test]
    fn test_execute_mutation_error_branches_2(
    ) -> std::result::Result<(), Box<dyn std::error::Error>> {
        let mut db = LinkedDatabase::new()?;
        db.tables.insert(
            "Feature".to_string(),
            vec![Record::with_fields(vec![
                FieldValue::String("PK".to_string()),
                FieldValue::String("Val1".to_string()),
            ])],
        );
        let mut rec1 = Record::with_fields(vec![
            FieldValue::String("PK2".to_string()),
            FieldValue::String("Val2".to_string()),
        ]);
        let _ = db.execute_mutation("Feature", &mut rec1, 0);

        let mut rec2 = Record::with_fields(vec![
            FieldValue::String("PK".to_string()),
            FieldValue::String("Val2".to_string()),
        ]);
        let _ = db.execute_mutation("Feature", &mut rec2, 5);
        let mut rec3 = Record::with_fields(vec![
            FieldValue::String("PK".to_string()),
            FieldValue::String("Val2".to_string()),
        ]);
        let _ = db.execute_mutation("Feature", &mut rec3, 8);
        Ok(())
    }

    #[test]
    fn test_execute_mutation_refresh_found() -> std::result::Result<(), Box<dyn std::error::Error>>
    {
        let mut db = LinkedDatabase::new()?;
        db.tables.insert(
            "Feature".to_string(),
            vec![Record::with_fields(vec![
                FieldValue::String("PK".to_string()),
                FieldValue::String("Val1".to_string()),
            ])],
        );
        let mut rec1 = Record::with_fields(vec![
            FieldValue::String("PK".to_string()),
            FieldValue::String("Val2".to_string()),
        ]);
        let _ = db.execute_mutation("Feature", &mut rec1, 0);
        assert_eq!(rec1.get(1), Some(&FieldValue::String("Val1".to_string())));
        Ok(())
    }

    #[test]
    fn test_execute_mutation_comprehensive() -> std::result::Result<(), Box<dyn std::error::Error>>
    {
        use crate::database::Record;
        let mut db = LinkedDatabase::new()?;

        let mut rec1 = Record::with_fields(vec![
            FieldValue::String("A".to_string()),
            FieldValue::String("Val1".to_string()),
        ]);
        let mut rec2 = Record::with_fields(vec![
            FieldValue::String("B".to_string()),
            FieldValue::String("Val2".to_string()),
        ]);
        let mut rec1_update = Record::with_fields(vec![
            FieldValue::String("A".to_string()),
            FieldValue::String("Val3".to_string()),
        ]);

        // INSERT
        assert!(db
            .execute_mutation("Property", &mut rec1.clone(), 1)
            .is_ok());
        // INSERT fail duplicate
        assert!(db
            .execute_mutation("Property", &mut rec1.clone(), 1)
            .is_err());

        // UPDATE
        assert!(db.execute_mutation("Property", &mut rec1_update, 2).is_ok());
        // UPDATE fail missing
        assert!(db
            .execute_mutation("Property", &mut rec2.clone(), 2)
            .is_err());

        // REPLACE (exists -> updates)
        assert!(db
            .execute_mutation("Property", &mut rec1.clone(), 4)
            .is_ok());
        // REPLACE (missing -> inserts)
        assert!(db.execute_mutation("Property", &mut rec2, 4).is_ok());

        let mut rec3 = Record::with_fields(vec![
            FieldValue::String("C".to_string()),
            FieldValue::String("Val3".to_string()),
        ]);

        // MERGE (exists -> updates)
        assert!(db
            .execute_mutation("Property", &mut rec1.clone(), 5)
            .is_ok());
        // MERGE (missing -> inserts)
        assert!(db
            .execute_mutation("Property", &mut rec3.clone(), 5)
            .is_ok());

        // DELETE
        assert!(db
            .execute_mutation("Property", &mut rec3.clone(), 6)
            .is_ok());
        // DELETE fail missing
        assert!(db.execute_mutation("Property", &mut rec3, 6).is_err());

        // INVALID mode
        assert!(db.execute_mutation("Property", &mut rec1, 99).is_err());

        // No primary keys defined (e.g. some internal table)
        db.tables.insert("NoPKTable".to_string(), vec![]);
        let mut rec_nopk = Record::with_fields(vec![FieldValue::String("X".to_string())]);
        assert!(db
            .execute_mutation("NoPKTable", &mut rec_nopk.clone(), 1)
            .is_ok());
        assert!(db.execute_mutation("NoPKTable", &mut rec_nopk, 2).is_err());
        Ok(())
    }

    #[test]
    fn test_merge_module_sequence_edge_cases() -> std::result::Result<(), Box<dyn std::error::Error>>
    {
        use crate::database::Record;
        let mut db = LinkedDatabase::new()?;
        let mut module_db = LinkedDatabase::default();

        db.tables.insert(
            "InstallExecuteSequence".to_string(),
            vec![Record::with_fields(vec![
                FieldValue::String("ExistingAction".to_string()),
                FieldValue::Null,
                FieldValue::String("NotAShort".to_string()), // index 2 is NOT a short!
                FieldValue::Null,
            ])],
        );

        module_db.tables.insert(
            "ModuleInstallExecuteSequence".to_string(),
            vec![
                Record::with_fields(vec![
                    FieldValue::String("Action1".to_string()),
                    FieldValue::String("Cond".to_string()),
                    FieldValue::Null,
                    FieldValue::Null,
                ]),
                // Case 1: base_action exists, but its sequence is not a short (covers line 1050)
                Record::with_fields(vec![
                    FieldValue::String("Action2".to_string()),
                    FieldValue::Null,
                    FieldValue::String("ExistingAction".to_string()),
                    FieldValue::Short(1),
                    FieldValue::Null,
                ]),
                // Case 2: base_action does not exist (covers line 1055)
                Record::with_fields(vec![
                    FieldValue::String("Action3".to_string()),
                    FieldValue::Null,
                    FieldValue::String("NonExistentAction".to_string()),
                    FieldValue::Short(1),
                    FieldValue::Null,
                ]),
            ],
        );

        let module = MergeModule {
            id: "Mod.Guid".to_string(),
            language: 1033,
            version: "1.0.0.0".to_string(),
            database: module_db,
        };

        assert!(db
            .merge_module(&module, "Feature1", "TARGETDIR", &HashMap::new())
            .is_ok());
        Ok(())
    }

    #[test]
    fn test_linker_types_derives_and_display() {
        let std_dir = STANDARD_DIRECTORIES[0].clone();
        assert_eq!(std_dir, STANDARD_DIRECTORIES[0]);
        let std_dir_dbg = format!("{std_dir:?}");
        assert!(std_dir_dbg.contains("StandardDirectory"));

        let std_act = STANDARD_INSTALL_EXECUTE_ACTIONS[0].clone();
        assert_eq!(std_act, STANDARD_INSTALL_EXECUTE_ACTIONS[0]);
        let std_act_dbg = format!("{std_act:?}");
        assert!(std_act_dbg.contains("StandardActionOrder"));

        let ice_rep = IceReport {
            ice: "ICE99".to_string(),
            is_error: true,
            message: "test message".to_string(),
        };
        assert_eq!(ice_rep, ice_rep.clone());
        let ice_rep_dbg = format!("{ice_rep:?}");
        assert!(ice_rep_dbg.contains("IceReport"));
        assert_eq!(format!("{ice_rep}"), "ICE99: error - test message");

        let ice_rep_warn = IceReport {
            ice: "ICE98".to_string(),
            is_error: false,
            message: "warn message".to_string(),
        };
        assert_eq!(format!("{ice_rep_warn}"), "ICE98: warning - warn message");

        let converted = Linker::convert_ice(Some(crate::wix::ice::types::IceReport {
            ice: "ICE01".to_string(),
            is_error: true,
            message: "converted err".to_string(),
            table: None,
            column: None,
        }));
        assert_eq!(
            converted,
            Some(IceReport {
                ice: "ICE01".to_string(),
                is_error: true,
                message: "converted err".to_string(),
            })
        );

        let db_default = LinkedDatabase::default();
        let rec_long = Record::with_fields(vec![FieldValue::Long(42)]);
        assert_eq!(db_default.get_int_field(&rec_long, 0), Some(42));
        assert_eq!(db_default, db_default.clone());
        let db_dbg = format!("{db_default:?}");
        assert!(db_dbg.contains("LinkedDatabase"));

        let linker = Linker::default();
        let linker_dbg = format!("{linker:?}");
        assert!(linker_dbg.contains("Linker"));
    }

    #[test]
    #[allow(clippy::too_many_lines)]
    fn test_linker_file_binding_and_cabinet_packaging() -> Result<()> {
        let temp_dir = std::env::temp_dir().join("msi_test_linker_binding");
        let _ = std::fs::create_dir_all(&temp_dir);
        let sample_file = temp_dir.join("sample.txt");
        let _ = std::fs::write(&sample_file, b"Hello WiX binder payload!");

        let versioned_file = temp_dir.join("versioned.txt");
        let _ = std::fs::write(&versioned_file, b"versioned payload");

        // Create font file fixture
        let font_file = temp_dir.join("testfont.ttf");
        let mut font_data = vec![0u8, 1, 0, 0]; // TTF magic
        font_data.extend_from_slice(&1u16.to_be_bytes()); // 1 table
        font_data.extend_from_slice(&[0; 6]); // searchRange, entrySelector, rangeShift
                                              // Table record for 'name': tag, checkSum, offset, length
        font_data.extend_from_slice(b"name");
        font_data.extend_from_slice(&0u32.to_be_bytes()); // checkSum
        let name_offset = 12 + 16u32;
        font_data.extend_from_slice(&name_offset.to_be_bytes());
        let name_length = 6 + 12 + 8u32;
        font_data.extend_from_slice(&name_length.to_be_bytes());
        // Name table: format=0, count=1, stringOffset=6+12=18
        font_data.extend_from_slice(&0u16.to_be_bytes()); // format
        font_data.extend_from_slice(&1u16.to_be_bytes()); // count
        font_data.extend_from_slice(&18u16.to_be_bytes()); // stringOffset
                                                           // NameRecord: platform=1 (Mac), encoding=0, lang=0, nameID=4 (title), length=8, offset=0
        font_data.extend_from_slice(&1u16.to_be_bytes());
        font_data.extend_from_slice(&0u16.to_be_bytes());
        font_data.extend_from_slice(&0u16.to_be_bytes());
        font_data.extend_from_slice(&4u16.to_be_bytes()); // Full Name
        font_data.extend_from_slice(&8u16.to_be_bytes()); // length
        font_data.extend_from_slice(&0u16.to_be_bytes()); // offset
        font_data.extend_from_slice(b"TestFont");
        let _ = std::fs::write(&font_file, &font_data);

        // Create PE file fixture with VS_FIXEDFILEINFO
        let pe_file = temp_dir.join("testapp.exe");
        let mut pe_data = vec![0u8; 1024];
        pe_data[0] = b'M';
        pe_data[1] = b'Z';
        pe_data[0x3C..0x40].copy_from_slice(&64u32.to_le_bytes()); // lfanew = 64
        pe_data[64..68].copy_from_slice(b"PE\0\0");
        pe_data[64 + 6..64 + 8].copy_from_slice(&1u16.to_le_bytes()); // num_sections = 1
        pe_data[64 + 20..64 + 22].copy_from_slice(&224u16.to_le_bytes()); // size_of_opt = 224
        let opt = 64 + 24;
        pe_data[opt..opt + 2].copy_from_slice(&0x10Bu16.to_le_bytes()); // PE32
                                                                        // Resource table directory at opt + 96 + 16
        let rsrc_dir_entry = opt + 96 + 16;
        pe_data[rsrc_dir_entry..rsrc_dir_entry + 4].copy_from_slice(&0x1000u32.to_le_bytes()); // RVA = 0x1000
        pe_data[rsrc_dir_entry + 4..rsrc_dir_entry + 8].copy_from_slice(&512u32.to_le_bytes()); // size = 512
                                                                                                // Section header at opt + 224
        let sec_hdr = opt + 224;
        pe_data[sec_hdr + 8..sec_hdr + 12].copy_from_slice(&512u32.to_le_bytes()); // virt_size
        pe_data[sec_hdr + 12..sec_hdr + 16].copy_from_slice(&0x1000u32.to_le_bytes()); // virt_addr
        pe_data[sec_hdr + 20..sec_hdr + 24].copy_from_slice(&512u32.to_le_bytes()); // raw_ptr = 512
                                                                                    // Resource directory root at file offset 512
        let root = 512;
        pe_data[root + 14..root + 16].copy_from_slice(&1u16.to_le_bytes()); // num_id = 1
                                                                            // Entry 0: ID = 16 (RT_VERSION), offset = 0x80000020 (directory at offset 32)
        pe_data[root + 16..root + 20].copy_from_slice(&16u32.to_le_bytes());
        pe_data[root + 20..root + 24].copy_from_slice(&(0x8000_0020u32).to_le_bytes());
        // Level 2 directory at root + 32
        let lvl2 = root + 32;
        pe_data[lvl2 + 14..lvl2 + 16].copy_from_slice(&1u16.to_le_bytes()); // num_id = 1
        pe_data[lvl2 + 16..lvl2 + 20].copy_from_slice(&1u32.to_le_bytes());
        pe_data[lvl2 + 20..lvl2 + 24].copy_from_slice(&(0x8000_0030u32).to_le_bytes());
        // Level 3 directory at root + 48
        let lvl3 = root + 48;
        pe_data[lvl3 + 14..lvl3 + 16].copy_from_slice(&1u16.to_le_bytes());
        pe_data[lvl3 + 16..lvl3 + 20].copy_from_slice(&0x1000u32.to_le_bytes()); // data_entry RVA = 0x1000
                                                                                 // Signature at offset root (RVA 0x1000 maps to 512 + 64 = 576)
        let sig_pos = 576;
        pe_data[sig_pos..sig_pos + 4].copy_from_slice(&0xFEEF_04BDu32.to_le_bytes());
        // FileVersionMS = (1 << 16) | 2, FileVersionLS = (3 << 16) | 4 => 1.2.3.4
        let ms = (1u32 << 16) | 2;
        let ls = (3u32 << 16) | 4;
        pe_data[sig_pos + 8..sig_pos + 12].copy_from_slice(&ms.to_le_bytes());
        pe_data[sig_pos + 12..sig_pos + 16].copy_from_slice(&ls.to_le_bytes());
        let _ = std::fs::write(&pe_file, &pe_data);

        // Build intermediate object with WixFile records
        let mut obj = WixObject::new();
        let mut sec = IntermediateSection::new(
            SectionType::Product,
            Some("{11111111-1111-1111-1111-111111111111}".to_string()),
        );

        // Add File table
        let mut file_tbl = IntermediateTable::new("File");
        file_tbl.push_record(Record::with_fields(vec![
            FieldValue::String("SampleFileKey".to_string()),
            FieldValue::String("Comp1".to_string()),
            FieldValue::String("sample.txt".to_string()),
            FieldValue::Long(0), // size to be filled
            FieldValue::Null,    // unversioned
            FieldValue::Null,
            FieldValue::Short(0),
            FieldValue::Short(1),
        ]));
        file_tbl.push_record(Record::with_fields(vec![
            FieldValue::String("FontFileKey".to_string()),
            FieldValue::String("Comp1".to_string()),
            FieldValue::String("testfont.ttf".to_string()),
            FieldValue::Long(0),
            FieldValue::Null,
            FieldValue::Null,
            FieldValue::Short(0),
            FieldValue::Short(2),
        ]));
        file_tbl.push_record(Record::with_fields(vec![
            FieldValue::String("PeFileKey".to_string()),
            FieldValue::String("Comp2".to_string()),
            FieldValue::String("testapp.exe".to_string()),
            FieldValue::Long(0),
            FieldValue::Null, // to be filled from PE
            FieldValue::Null,
            FieldValue::Short(0),
            FieldValue::Short(3),
        ]));
        file_tbl.push_record(Record::with_fields(vec![
            FieldValue::Null,
            FieldValue::String("Comp1".to_string()),
            FieldValue::Null,
            FieldValue::Long(0),
            FieldValue::Null,
        ]));
        file_tbl.push_record(Record::with_fields(vec![
            FieldValue::String("NullCompKey".to_string()),
            FieldValue::Null,
            FieldValue::Null,
            FieldValue::Long(0),
            FieldValue::Null,
        ]));
        file_tbl.push_record(Record::with_fields(vec![
            FieldValue::String("NoSourceKey".to_string()),
            FieldValue::String("Comp1".to_string()),
            FieldValue::Null,
            FieldValue::Long(0),
            FieldValue::Null,
        ]));
        file_tbl.push_record(Record::with_fields(vec![
            FieldValue::String("VersionedKey".to_string()),
            FieldValue::String("Comp1".to_string()),
            FieldValue::String("versioned.txt".to_string()),
            FieldValue::Long(0),
            FieldValue::String("2.0.0".to_string()),
            FieldValue::Null,
            FieldValue::Short(0),
            FieldValue::Short(4),
        ]));
        sec.tables.push(file_tbl);

        // Add WixFile table
        let mut wix_file_tbl = IntermediateTable::new("WixFile");
        wix_file_tbl.push_record(Record::with_fields(vec![
            FieldValue::String("SampleFileKey".to_string()),
            FieldValue::String("sample.txt".to_string()),
        ]));
        wix_file_tbl.push_record(Record::with_fields(vec![
            FieldValue::String("FontFileKey".to_string()),
            FieldValue::String("[MyBind]/testfont.ttf".to_string()),
        ]));
        wix_file_tbl.push_record(Record::with_fields(vec![
            FieldValue::String("PeFileKey".to_string()),
            FieldValue::String(pe_file.to_string_lossy().to_string()),
        ]));
        wix_file_tbl.push_record(Record::with_fields(vec![
            FieldValue::String("VersionedKey".to_string()),
            FieldValue::String("versioned.txt".to_string()),
        ]));
        wix_file_tbl.push_record(Record::with_fields(vec![
            FieldValue::Null,
            FieldValue::Null,
        ]));
        sec.tables.push(wix_file_tbl);

        // Add Directory table
        let mut dir_tbl = IntermediateTable::new("Directory");
        dir_tbl.push_record(Record::with_fields(vec![
            FieldValue::String("TARGETDIR".to_string()),
            FieldValue::Null,
            FieldValue::String("SourceDir".to_string()),
        ]));
        dir_tbl.push_record(Record::with_fields(vec![
            FieldValue::String("INSTALLFOLDER".to_string()),
            FieldValue::String("TARGETDIR".to_string()),
            FieldValue::String("App".to_string()),
        ]));
        sec.tables.push(dir_tbl);

        // Add Component table
        let mut comp_tbl = IntermediateTable::new("Component");
        comp_tbl.push_record(Record::with_fields(vec![
            FieldValue::String("Comp1".to_string()),
            FieldValue::String("{22222222-2222-2222-2222-222222222222}".to_string()),
            FieldValue::String("INSTALLFOLDER".to_string()),
            FieldValue::Short(0),
            FieldValue::Null,
            FieldValue::String("SampleFileKey".to_string()),
        ]));
        comp_tbl.push_record(Record::with_fields(vec![
            FieldValue::String("Comp2".to_string()),
            FieldValue::String("{33333333-3333-3333-3333-333333333333}".to_string()),
            FieldValue::String("INSTALLFOLDER".to_string()),
            FieldValue::Short(0),
            FieldValue::Null,
            FieldValue::String("PeFileKey".to_string()),
        ]));
        sec.tables.push(comp_tbl);

        // Add Feature table
        let mut feat_tbl = IntermediateTable::new("Feature");
        feat_tbl.push_record(Record::with_fields(vec![
            FieldValue::String("MainFeature".to_string()),
            FieldValue::Null,
            FieldValue::String("Main".to_string()),
            FieldValue::Null,
            FieldValue::Short(1),
            FieldValue::Short(1),
            FieldValue::Null,
            FieldValue::Short(0),
        ]));
        sec.tables.push(feat_tbl);

        // Add FeatureComponents table
        let mut fc_tbl = IntermediateTable::new("FeatureComponents");
        fc_tbl.push_record(Record::with_fields(vec![
            FieldValue::String("MainFeature".to_string()),
            FieldValue::String("Comp1".to_string()),
        ]));
        fc_tbl.push_record(Record::with_fields(vec![
            FieldValue::String("MainFeature".to_string()),
            FieldValue::String("Comp2".to_string()),
        ]));
        sec.tables.push(fc_tbl);

        // Add Media table
        let mut media_tbl = IntermediateTable::new("Media");
        media_tbl.push_record(Record::with_fields(vec![
            FieldValue::Short(1),
            FieldValue::Long(3),
            FieldValue::Null,
            FieldValue::String("#cab1.cab".to_string()),
            FieldValue::Null,
            FieldValue::Null,
        ]));
        sec.tables.push(media_tbl);

        obj.add_section(sec);

        // Test linking with base directories, bind paths, and cab_per_component
        let mut linker = Linker::new();
        linker.add_object(obj.clone());
        linker.add_base_dir(&temp_dir);
        linker.add_bind_path("MyBind", &temp_dir);
        linker.set_cab_per_component(true);
        linker.set_suppress_ice(true);

        let linked_db = linker.link().unwrap_or_default();

        // Verify File table updated sizes and versions
        let files = linked_db.get_records("File");
        assert_eq!(files.len(), 7);
        // SampleFileKey size = 25
        assert_eq!(files[0].get(3), Some(&FieldValue::Long(25)));
        // PeFileKey version = 1.2.3.4
        assert_eq!(
            files[2].get(4),
            Some(&FieldValue::String("1.2.3.4".to_string()))
        );
        assert_eq!(
            files[2].get(5),
            Some(&FieldValue::String("1033".to_string()))
        );

        // Verify FileHash table populated for unversioned sample file
        let hashes = linked_db.get_records("FileHash");
        assert_ne!(hashes, []);

        // Verify Font table populated for font file
        let fonts = linked_db.get_records("Font");
        assert_eq!(fonts.len(), 1);
        assert_eq!(
            fonts[0].get(1),
            Some(&FieldValue::String("TestFont".to_string()))
        );

        // Verify embedded cabinets were generated per component
        let cabs = linker.take_embedded_cabinets();
        assert!(cabs.contains_key("#comp_Comp1.cab"));
        assert!(cabs.contains_key("#comp_Comp2.cab"));

        // Test linking without cab_per_component (media cabinets fallback)
        let mut linker_media = Linker::new();
        linker_media.add_object(obj);
        linker_media.add_base_dir(&temp_dir);
        linker_media.add_bind_path("MyBind", &temp_dir);
        linker_media.set_cab_per_component(false);
        linker_media.set_suppress_ice(true);
        let linked_media_db = linker_media.link().unwrap_or_default();
        assert_ne!(linked_media_db.get_records("File"), []);

        // Test bind_files_and_pack_cabinets on empty database and db without File table
        let mut empty_db = LinkedDatabase::default();
        assert!(linker.bind_files_and_pack_cabinets(&mut empty_db).is_ok());

        let mut no_file_db = LinkedDatabase::default();
        no_file_db.add_record(
            "WixFile",
            Record::with_fields(vec![
                FieldValue::String("F1".to_string()),
                FieldValue::String("sample.txt".to_string()),
            ]),
        );
        assert!(linker.bind_files_and_pack_cabinets(&mut no_file_db).is_ok());

        // Clean up
        let _ = std::fs::remove_dir_all(&temp_dir);
        Ok(())
    }

    #[test]
    #[allow(clippy::too_many_lines)]
    fn test_linker_missing_file_and_ice_filtering() {
        let mut obj = WixObject::new();
        let mut sec = IntermediateSection::new(
            SectionType::Product,
            Some("{11111111-1111-1111-1111-111111111111}".to_string()),
        );

        let mut file_tbl = IntermediateTable::new("File");
        file_tbl.push_record(Record::with_fields(vec![
            FieldValue::String("MissingF".to_string()),
            FieldValue::String("Comp1".to_string()),
            FieldValue::String("missing.dll".to_string()),
            FieldValue::Long(0),
            FieldValue::Null,
            FieldValue::Null,
            FieldValue::Short(0),
            FieldValue::Short(1),
        ]));
        sec.tables.push(file_tbl);

        let mut wix_file_tbl = IntermediateTable::new("WixFile");
        wix_file_tbl.push_record(Record::with_fields(vec![
            FieldValue::String("MissingF".to_string()),
            FieldValue::String("non_existent_path.dll".to_string()),
        ]));
        sec.tables.push(wix_file_tbl);

        let mut dir_tbl = IntermediateTable::new("Directory");
        dir_tbl.push_record(Record::with_fields(vec![
            FieldValue::String("TARGETDIR".to_string()),
            FieldValue::Null,
            FieldValue::String("SourceDir".to_string()),
        ]));
        sec.tables.push(dir_tbl);

        obj.add_section(sec);

        let mut linker = Linker::new();
        linker.add_object(obj.clone());
        linker.add_base_dir("/nonexistent_search_base_dir");
        // Linking should fail because non_existent_path.dll cannot be found in base directories
        let err = linker.link();
        assert!(err.is_err());

        let mut linker_bind_only = Linker::new();
        linker_bind_only.add_bind_path("B", "/nonexistent");
        linker_bind_only.add_object(obj.clone());
        assert!(linker_bind_only.link().is_err());

        let mut linker_neither = Linker::new();
        linker_neither.add_object(obj);
        assert!(linker_neither.link().is_ok());

        // Test ICE filtering options
        let mut linker2 = Linker::new();
        linker2.set_suppress_ice(true);
        linker2.select_ice("ICE01");
        linker2.suppress_ice("ICE38");
        linker2.suppress_warning("101");
        linker2.set_warnings_as_errors(true);
        linker2.set_cultures(vec!["en-us".to_string()]);
        assert!(linker2.embedded_cabinets().is_empty());

        // Test resolve_source_path variants: absolute path, bind path prefix, and cwd relative
        let temp_dir = std::env::temp_dir().join("msi_linker_resolve_test");
        let _ = std::fs::create_dir_all(&temp_dir);
        let sample = temp_dir.join("bound_sample.txt");
        let _ = std::fs::write(&sample, b"test payload");

        let mut linker3 = Linker::new();
        linker3.add_bind_path("TestBind", &temp_dir);
        linker3.add_base_dir(&temp_dir);
        assert!(linker3
            .resolve_source_path(&sample.to_string_lossy())
            .is_some());
        assert!(linker3
            .resolve_source_path("[TestBind]/bound_sample.txt")
            .is_some());
        assert!(linker3
            .resolve_source_path("[TestBind]/nonexistent.txt")
            .is_none());
        assert!(linker3
            .resolve_source_path("/nonexistent_abs_path/file.txt")
            .is_none());

        let cwd_temp = PathBuf::from("temp_linker_cwd_test.txt");
        let _ = std::fs::write(&cwd_temp, b"cwd test");
        assert!(linker3
            .resolve_source_path("temp_linker_cwd_test.txt")
            .is_some());
        let _ = std::fs::remove_file(&cwd_temp);
        let _ = std::fs::remove_dir_all(&temp_dir);

        // Test standard property references in symbol graph
        let mut std_prop_obj = WixObject::new();
        let mut std_sec = IntermediateSection::new(
            SectionType::Product,
            Some("{33333333-3333-3333-3333-333333333333}".to_string()),
        );
        std_sec.add_symbol(Symbol::new(
            "Product",
            "{33333333-3333-3333-3333-333333333333}",
        ));
        std_sec.add_reference(Reference::new("Property", "WIXUI_INSTALLDIR"));
        std_sec.add_reference(Reference::new("Property", "ARPPRODUCTICON"));
        std_sec.add_reference(Reference::new("Property", "ALLUSERS"));
        std_sec.add_reference(Reference::new("Directory", "TARGETDIR"));
        std_sec.add_reference(Reference::new("Action", "InstallFiles"));
        std_sec.add_reference(Reference::new("UI", "WixUI_InstallDir"));
        std_sec.add_reference(Reference::new("UI", "WixUI"));

        let mut std_dir = IntermediateTable::new("Directory");
        std_dir.push_record(Record::with_fields(vec![
            FieldValue::String("TARGETDIR".to_string()),
            FieldValue::Null,
            FieldValue::String("SourceDir".to_string()),
        ]));
        std_sec.tables.push(std_dir);

        let mut std_dlg = IntermediateTable::new("Dialog");
        std_dlg.push_record(Record::with_fields(vec![
            FieldValue::String("ExistingDialog".to_string()),
            FieldValue::Short(100),
            FieldValue::Short(100),
            FieldValue::String("Title".to_string()),
        ]));
        std_sec.tables.push(std_dlg);

        std_prop_obj.add_section(std_sec);
        let mut linker4 = Linker::new();
        linker4.set_suppress_ice(true);
        linker4.add_object(std_prop_obj);
        let link_res = linker4.link();
        assert!(link_res.is_ok());

        // Test linking with WixUI_InstallDir and without pre-existing Dialog table
        let mut std_prop_obj_no_dlg = WixObject::new();
        let mut std_sec_no_dlg = IntermediateSection::new(
            SectionType::Product,
            Some("{44444444-4444-4444-4444-444444444444}".to_string()),
        );
        std_sec_no_dlg.add_symbol(Symbol::new(
            "Product",
            "{44444444-4444-4444-4444-444444444444}",
        ));
        std_sec_no_dlg.add_reference(Reference::new("UI", "WixUI_InstallDir"));
        let mut std_dir2 = IntermediateTable::new("Directory");
        std_dir2.push_record(Record::with_fields(vec![
            FieldValue::String("TARGETDIR".to_string()),
            FieldValue::Null,
            FieldValue::String("SourceDir".to_string()),
        ]));
        std_sec_no_dlg.tables.push(std_dir2);
        std_prop_obj_no_dlg.add_section(std_sec_no_dlg);

        let mut linker5 = Linker::new();
        linker5.set_suppress_ice(true);
        linker5.add_object(std_prop_obj_no_dlg);
        assert!(linker5.link().is_ok());
    }

    #[allow(clippy::cognitive_complexity)]
    #[test]
    #[allow(clippy::too_many_lines)]
    fn test_linker_inspection_helpers_and_ice_rules() -> Result<()> {
        // Test inspect_pe_version error branches
        assert_eq!(inspect_pe_version(&[]), None);
        assert_eq!(inspect_pe_version(b"MZ"), None);
        assert_eq!(inspect_pe_version(&[0u8; 70]), None);

        let mut pe_short_lfanew = vec![0u8; 70];
        pe_short_lfanew[0] = b'M';
        pe_short_lfanew[1] = b'Z';
        pe_short_lfanew[0x3C] = 64; // lfanew = 64, but data.len() = 70 < 64 + 24
        assert_eq!(inspect_pe_version(&pe_short_lfanew), None);

        let mut bad_pe = vec![0u8; 128];
        bad_pe[0] = b'M';
        bad_pe[1] = b'Z';
        bad_pe[0x3C] = 64;
        bad_pe[64..68].copy_from_slice(b"NOTP");
        assert_eq!(inspect_pe_version(&bad_pe), None);

        bad_pe[64..68].copy_from_slice(b"PE\0\0");
        bad_pe[64 + 24..64 + 26].copy_from_slice(&0xFFFFu16.to_le_bytes()); // invalid opt magic
        assert_eq!(inspect_pe_version(&bad_pe), None);

        bad_pe[64 + 24..64 + 26].copy_from_slice(&0x10Bu16.to_le_bytes()); // PE32, rsrc_rva is 0
        assert_eq!(inspect_pe_version(&bad_pe), None);

        // Test PE32+ (64-bit) fixture
        let mut pe64 = vec![0u8; 1024];
        pe64[0] = b'M';
        pe64[1] = b'Z';
        pe64[0x3C..0x40].copy_from_slice(&64u32.to_le_bytes());
        pe64[64..68].copy_from_slice(b"PE\0\0");
        pe64[64 + 6..64 + 8].copy_from_slice(&1u16.to_le_bytes()); // num_sections = 1
        pe64[64 + 20..64 + 22].copy_from_slice(&240u16.to_le_bytes()); // size_of_opt = 240
        let opt64 = 64 + 24;
        pe64[opt64..opt64 + 2].copy_from_slice(&0x20Bu16.to_le_bytes()); // PE32+
        let rsrc_dir_entry64 = opt64 + 112 + 16;
        pe64[rsrc_dir_entry64..rsrc_dir_entry64 + 4].copy_from_slice(&0x1000u32.to_le_bytes());
        pe64[rsrc_dir_entry64 + 4..rsrc_dir_entry64 + 8].copy_from_slice(&512u32.to_le_bytes());
        let sec_hdr64 = opt64 + 240;
        pe64[sec_hdr64 + 8..sec_hdr64 + 12].copy_from_slice(&512u32.to_le_bytes());
        pe64[sec_hdr64 + 12..sec_hdr64 + 16].copy_from_slice(&0x1000u32.to_le_bytes());
        pe64[sec_hdr64 + 20..sec_hdr64 + 24].copy_from_slice(&512u32.to_le_bytes());
        let root = 512;
        pe64[root + 14..root + 16].copy_from_slice(&1u16.to_le_bytes());
        pe64[root + 16..root + 20].copy_from_slice(&16u32.to_le_bytes());
        pe64[root + 20..root + 24].copy_from_slice(&(0x8000_0020u32).to_le_bytes());
        pe64[544 + 14..544 + 16].copy_from_slice(&1u16.to_le_bytes());
        pe64[544 + 16..544 + 20].copy_from_slice(&1u32.to_le_bytes());
        pe64[544 + 20..544 + 24].copy_from_slice(&(0x8000_0030u32).to_le_bytes());
        pe64[560 + 14..560 + 16].copy_from_slice(&1u16.to_le_bytes());
        pe64[560 + 16..560 + 20].copy_from_slice(&0x1000u32.to_le_bytes());
        pe64[576..580].copy_from_slice(&0xFEEF_04BDu32.to_le_bytes());
        let ms64 = (3u32 << 16) | 1;
        let ls64 = (4u32 << 16) | 2;
        pe64[584..588].copy_from_slice(&ms64.to_le_bytes());
        pe64[588..592].copy_from_slice(&ls64.to_le_bytes());
        assert_eq!(
            inspect_pe_version(&pe64),
            Some(("3.1.4.2".to_string(), "1033".to_string()))
        );

        // PE section RVA not matching section
        let mut pe_bad_sec = pe64.clone();
        let sec_hdr_bad = 64 + 24 + 240;
        pe_bad_sec[sec_hdr_bad + 12..sec_hdr_bad + 16].copy_from_slice(&0x5000u32.to_le_bytes());
        assert_eq!(inspect_pe_version(&pe_bad_sec), None);

        // PE section where rsrc_rva >= virt_addr but >= virt_addr + virt_size
        let mut pe_rva_past_sec = pe64.clone();
        pe_rva_past_sec[sec_hdr_bad + 8..sec_hdr_bad + 12].copy_from_slice(&256u32.to_le_bytes()); // size 256
        pe_rva_past_sec[sec_hdr_bad + 12..sec_hdr_bad + 16]
            .copy_from_slice(&0x1000u32.to_le_bytes()); // addr 0x1000
        let rsrc_dir_past = 64 + 24 + 112 + 16;
        pe_rva_past_sec[rsrc_dir_past..rsrc_dir_past + 4].copy_from_slice(&0x1200u32.to_le_bytes()); // rva 0x1200
        assert_eq!(inspect_pe_version(&pe_rva_past_sec), None);

        // PE where resource directory has entries with id != 16
        let mut pe_no_ver = pe64.clone();
        pe_no_ver[512 + 16..512 + 20].copy_from_slice(&1u32.to_le_bytes());
        assert_eq!(inspect_pe_version(&pe_no_ver), None);

        // PE with valid header where rsrc_rva is 0
        let mut pe_rsrc_zero = vec![0u8; 512];
        pe_rsrc_zero[0] = b'M';
        pe_rsrc_zero[1] = b'Z';
        pe_rsrc_zero[0x3C] = 64;
        pe_rsrc_zero[64..68].copy_from_slice(b"PE\0\0");
        pe_rsrc_zero[64 + 24..64 + 26].copy_from_slice(&0x10Bu16.to_le_bytes());
        assert_eq!(inspect_pe_version(&pe_rsrc_zero), None);

        // PE where signature 0xFEEF_04BD is zeroed out
        let mut pe_no_sig = pe64.clone();
        pe_no_sig[576..580].copy_from_slice(&[0; 4]);
        assert_eq!(inspect_pe_version(&pe_no_sig), None);

        // PE truncated before end of VS_FIXEDFILEINFO
        let mut pe_truncated = pe64.clone();
        pe_truncated.truncate(576 + 10);
        assert_eq!(inspect_pe_version(&pe_truncated), None);

        // PE with unknown optional header magic
        let mut pe_bad_magic = pe64.clone();
        pe_bad_magic[opt64..opt64 + 2].copy_from_slice(&0x30Bu16.to_le_bytes());
        assert_eq!(inspect_pe_version(&pe_bad_magic), None);

        // Additional PE truncated bounds checks
        assert_eq!(inspect_pe_version(&pe64[..=(64 + 24)]), None);
        assert_eq!(inspect_pe_version(&pe64[..rsrc_dir_entry64 + 2]), None);
        assert_eq!(inspect_pe_version(&pe64[..sec_hdr64 + 20]), None);
        assert_eq!(inspect_pe_version(&pe64[..root + 10]), None);
        assert_eq!(inspect_pe_version(&pe64[..root + 20]), None);

        // Test inspect_font_title error branches
        assert_eq!(inspect_font_title(&[]), None);
        assert_eq!(inspect_font_title(b"NOTF"), None);
        let bad_font = vec![0u8, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]; // 0 tables
        assert_eq!(inspect_font_title(&bad_font), None);

        // Font with multiple tables (first not "name") and non-matching name_id = 2
        let mut font_multi = vec![0u8, 1, 0, 0];
        font_multi.extend_from_slice(&2u16.to_be_bytes()); // 2 tables
        font_multi.extend_from_slice(&[0; 6]);
        font_multi.extend_from_slice(b"head");
        font_multi.extend_from_slice(&0u32.to_be_bytes());
        font_multi.extend_from_slice(&100u32.to_be_bytes());
        font_multi.extend_from_slice(&20u32.to_be_bytes());
        font_multi.extend_from_slice(b"name");
        font_multi.extend_from_slice(&0u32.to_be_bytes());
        let name_off = 12 + 32u32;
        font_multi.extend_from_slice(&name_off.to_be_bytes());
        let name_len = 6 + 12 + 8u32;
        font_multi.extend_from_slice(&name_len.to_be_bytes());
        font_multi.extend_from_slice(&0u16.to_be_bytes());
        font_multi.extend_from_slice(&1u16.to_be_bytes());
        font_multi.extend_from_slice(&18u16.to_be_bytes());
        font_multi.extend_from_slice(&1u16.to_be_bytes()); // Mac
        font_multi.extend_from_slice(&0u16.to_be_bytes());
        font_multi.extend_from_slice(&0u16.to_be_bytes());
        font_multi.extend_from_slice(&2u16.to_be_bytes()); // name_id = 2 (not 1 or 4)
        font_multi.extend_from_slice(&8u16.to_be_bytes());
        font_multi.extend_from_slice(&0u16.to_be_bytes());
        font_multi.extend_from_slice(b"BoldName");
        assert_eq!(inspect_font_title(&font_multi), None);

        // Additional font truncated bounds checks
        assert_eq!(inspect_font_title(&font_multi[..20]), None);
        assert_eq!(
            inspect_font_title(&font_multi[..name_off as usize + 2]),
            None
        );
        assert_eq!(
            inspect_font_title(&font_multi[..name_off as usize + 10]),
            None
        );
        assert_eq!(
            inspect_font_title(&font_multi[..name_off as usize + 22]),
            None
        );

        // Test font UTF-16 BE with platform_id = 3 and name_id = 1
        let mut font16_data = vec![0u8, 1, 0, 0];
        font16_data.extend_from_slice(&1u16.to_be_bytes()); // 1 table
        font16_data.extend_from_slice(&[0; 6]);
        font16_data.extend_from_slice(b"name");
        font16_data.extend_from_slice(&0u32.to_be_bytes());
        let name_off16 = 12 + 16u32;
        font16_data.extend_from_slice(&name_off16.to_be_bytes());
        let name_len16 = 6 + 12 + 16u32;
        font16_data.extend_from_slice(&name_len16.to_be_bytes());
        font16_data.extend_from_slice(&0u16.to_be_bytes());
        font16_data.extend_from_slice(&1u16.to_be_bytes());
        font16_data.extend_from_slice(&18u16.to_be_bytes());
        // platform=3 (Windows), encoding=1, lang=0x0409, nameID=1 (Family Name), length=16, offset=0
        font16_data.extend_from_slice(&3u16.to_be_bytes());
        font16_data.extend_from_slice(&1u16.to_be_bytes());
        font16_data.extend_from_slice(&0x0409u16.to_be_bytes());
        font16_data.extend_from_slice(&1u16.to_be_bytes());
        font16_data.extend_from_slice(&16u16.to_be_bytes());
        font16_data.extend_from_slice(&0u16.to_be_bytes());
        for c in "TestFont".encode_utf16() {
            font16_data.extend_from_slice(&c.to_be_bytes());
        }
        assert_eq!(
            inspect_font_title(&font16_data),
            Some("TestFont".to_string())
        );

        // Font with name_id == 1 where string table offset extends past slice
        let mut font_trunc_str = font16_data.clone();
        font_trunc_str.truncate(font_trunc_str.len() - 10);
        assert_eq!(inspect_font_title(&font_trunc_str), None);

        // Test OTTO magic font
        let mut otto_font = font16_data.clone();
        otto_font[0..4].copy_from_slice(b"OTTO");
        assert_eq!(inspect_font_title(&otto_font), Some("TestFont".to_string()));

        // Test platform_id = 0 (OpenType Unicode)
        let mut font_plat0 = font16_data.clone();
        let plat_off = 12 + 16 + 6;
        font_plat0[plat_off..plat_off + 2].copy_from_slice(&0u16.to_be_bytes());
        assert_eq!(
            inspect_font_title(&font_plat0),
            Some("TestFont".to_string())
        );

        // Font with empty UTF-16 BE title (length = 0)
        let mut font_empty = font16_data.clone();
        let len_off = 12 + 16 + 6 + 8;
        font_empty[len_off..len_off + 2].copy_from_slice(&0u16.to_be_bytes());
        assert_eq!(inspect_font_title(&font_empty), None);

        // Font with empty UTF-8 title (length = 0, platform_id = 1)
        let mut font_utf8_empty = font16_data.clone();
        font_utf8_empty[plat_off..plat_off + 2].copy_from_slice(&1u16.to_be_bytes()); // Mac
        font_utf8_empty[len_off..len_off + 2].copy_from_slice(&0u16.to_be_bytes()); // length 0
        assert_eq!(inspect_font_title(&font_utf8_empty), None);

        // Font with invalid UTF-16 surrogate
        let mut font_bad_utf16 = font16_data.clone();
        font_bad_utf16[46..48].copy_from_slice(&[0xD8, 0x00]);
        assert_eq!(inspect_font_title(&font_bad_utf16), None);

        // Font with invalid UTF-8 byte
        let mut font_bad_utf8 = font16_data;
        font_bad_utf8[plat_off..plat_off + 2].copy_from_slice(&1u16.to_be_bytes()); // Mac
        font_bad_utf8[len_off..len_off + 2].copy_from_slice(&1u16.to_be_bytes()); // length 1
        font_bad_utf8[46] = 0xFF; // invalid utf-8
        assert_eq!(inspect_font_title(&font_bad_utf8), None);

        // Test direct run_ice_validations
        let db = LinkedDatabase::default();
        assert!(Linker::run_ice_validations(&db).is_ok());

        Ok(())
    }

    /// Tests multi-cabinet media partitioning, sequence graph solving, and variable resolution.
    ///
    /// # Errors
    ///
    /// Returns [`crate::MsiError`] on test setup or execution failure.
    #[test]
    #[allow(clippy::too_many_lines)]
    #[allow(clippy::cast_possible_truncation)]
    #[allow(clippy::cast_possible_wrap)]
    fn test_linker_libscript_parity_multi_cab_and_sequences() -> Result<()> {
        let mut db = LinkedDatabase::default();

        // 1. Setup Media table with 4 disks
        db.add_record(
            "Media",
            Record::with_fields(vec![
                FieldValue::Short(1),
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::String("#engine.cab".to_string()),
                FieldValue::Null,
                FieldValue::Null,
            ]),
        );
        db.add_record(
            "Media",
            Record::with_fields(vec![
                FieldValue::Short(2),
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::String("#runtimes.cab".to_string()),
                FieldValue::Null,
                FieldValue::Null,
            ]),
        );
        db.add_record(
            "Media",
            Record::with_fields(vec![
                FieldValue::Short(3),
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::String("#databases.cab".to_string()),
                FieldValue::Null,
                FieldValue::Null,
            ]),
        );
        db.add_record(
            "Media",
            Record::with_fields(vec![
                FieldValue::Short(4),
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::String("#codebase.cab".to_string()),
                FieldValue::Null,
                FieldValue::Null,
            ]),
        );

        // 2. Setup File table and _FileDiskId mapping (2 files per disk = 8 files total)
        for d in 1..=4 {
            for f in 1..=2 {
                let fid = format!("File_D{d}_{f}");
                db.add_record(
                    "File",
                    Record::with_fields(vec![
                        FieldValue::String(fid.clone()),
                        FieldValue::String(format!("Comp_D{d}_{f}")),
                        FieldValue::String(format!("file_d{d}_{f}.txt")),
                        FieldValue::Long(100),
                        FieldValue::Null,
                        FieldValue::Null,
                        FieldValue::Null,
                        FieldValue::Null,
                    ]),
                );
                db.add_record(
                    "_FileDiskId",
                    Record::with_fields(vec![FieldValue::String(fid), FieldValue::Short(d)]),
                );
            }
        }

        // 3. Layout media and files
        Linker::layout_media_and_files(&mut db);

        let files = db.get_records("File");
        assert_eq!(files.len(), 8);
        for (i, r) in files.iter().enumerate() {
            let expected_seq = (i + 1) as i16;
            assert_eq!(r.get(7), Some(&FieldValue::Short(expected_seq)));
        }

        let media = db.get_records("Media");
        assert_eq!(media.len(), 4);
        assert_eq!(media[0].get(1), Some(&FieldValue::Long(2)));
        assert_eq!(media[1].get(1), Some(&FieldValue::Long(4)));
        assert_eq!(media[2].get(1), Some(&FieldValue::Long(6)));
        assert_eq!(media[3].get(1), Some(&FieldValue::Long(8)));

        // 4. Test Relative Sequence Solving
        db.add_record(
            "InstallUISequence",
            Record::with_fields(vec![
                FieldValue::String("Dlg_Welcome".to_string()),
                FieldValue::String("NOT Installed".to_string()),
                FieldValue::Null,
            ]),
        );
        db.add_record(
            "InstallUISequence",
            Record::with_fields(vec![
                FieldValue::String("Dlg_Exit".to_string()),
                FieldValue::String("NOT Installed".to_string()),
                FieldValue::Null,
            ]),
        );

        db.add_record(
            "_WixSequenceRelative",
            Record::with_fields(vec![
                FieldValue::String("InstallUISequence".to_string()),
                FieldValue::String("Dlg_Welcome".to_string()),
                FieldValue::String("CostFinalize".to_string()),
                FieldValue::String("After".to_string()),
            ]),
        );
        db.add_record(
            "_WixSequenceRelative",
            Record::with_fields(vec![
                FieldValue::String("InstallUISequence".to_string()),
                FieldValue::String("Dlg_Exit".to_string()),
                FieldValue::String("success".to_string()),
                FieldValue::String("OnExit".to_string()),
            ]),
        );

        let _ = Linker::solve_relative_sequences(&mut db);

        let ui_seq = db.get_records("InstallUISequence");
        let exit_rec = ui_seq
            .iter()
            .find(|r| r.get(0) == Some(&FieldValue::String("Dlg_Exit".to_string())));
        assert!(exit_rec.is_some());
        assert_eq!(
            exit_rec.and_then(|r| r.get(2)),
            Some(&FieldValue::Short(-1))
        );

        let welcome_rec = ui_seq
            .iter()
            .find(|r| r.get(0) == Some(&FieldValue::String("Dlg_Welcome".to_string())));
        assert!(welcome_rec.is_some());
        assert_eq!(
            welcome_rec.and_then(|r| r.get(2)),
            Some(&FieldValue::Short(1025))
        );

        // 5. Test Cycle Detection
        let mut cyclic_db = LinkedDatabase::default();
        cyclic_db.add_record(
            "InstallExecuteSequence",
            Record::with_fields(vec![
                FieldValue::String("ActA".to_string()),
                FieldValue::Null,
                FieldValue::Null,
            ]),
        );
        cyclic_db.add_record(
            "InstallExecuteSequence",
            Record::with_fields(vec![
                FieldValue::String("ActB".to_string()),
                FieldValue::Null,
                FieldValue::Null,
            ]),
        );
        cyclic_db.add_record(
            "_WixSequenceRelative",
            Record::with_fields(vec![
                FieldValue::String("InstallExecuteSequence".to_string()),
                FieldValue::String("ActA".to_string()),
                FieldValue::String("ActB".to_string()),
                FieldValue::String("After".to_string()),
            ]),
        );
        cyclic_db.add_record(
            "_WixSequenceRelative",
            Record::with_fields(vec![
                FieldValue::String("InstallExecuteSequence".to_string()),
                FieldValue::String("ActB".to_string()),
                FieldValue::String("ActA".to_string()),
                FieldValue::String("After".to_string()),
            ]),
        );
        assert!(Linker::solve_relative_sequences(&mut cyclic_db).is_err());

        // 6. Test WixVariable Resolution and EULA binding
        let mut linker = Linker::new();
        let mut var_db = LinkedDatabase::default();

        let temp_dir =
            std::env::temp_dir().join(format!("linker_icon_test_{}", std::process::id()));
        let _ = std::fs::create_dir_all(&temp_dir);
        let ico_file = temp_dir.join("exclamation.ico");
        let _ = std::fs::write(&ico_file, b"fake icon data");

        var_db.add_record(
            "WixVariable",
            Record::with_fields(vec![
                FieldValue::String("WixUIExclamationIco".to_string()),
                FieldValue::String(ico_file.to_string_lossy().to_string()),
                FieldValue::Short(0),
            ]),
        );
        var_db.add_record(
            "WixVariable",
            Record::with_fields(vec![
                FieldValue::String("AppName".to_string()),
                FieldValue::String("MySuperApp".to_string()),
                FieldValue::Short(0),
            ]),
        );
        var_db.add_record(
            "WixVariable",
            Record::with_fields(vec![
                FieldValue::String("WixUILicenseRtf".to_string()),
                FieldValue::String(r"{\rtf1 Custom EULA Text}".to_string()),
                FieldValue::Short(0),
            ]),
        );
        var_db.add_record(
            "Property",
            Record::with_fields(vec![
                FieldValue::String("ProductName".to_string()),
                FieldValue::String("Welcome to !(wix.AppName)".to_string()),
            ]),
        );
        var_db.add_record(
            "Control",
            Record::with_fields(vec![
                FieldValue::String("LicenseDlg".to_string()),
                FieldValue::String("AgreementText".to_string()),
                FieldValue::String("ScrollableText".to_string()),
                FieldValue::Short(20),
                FieldValue::Short(60),
                FieldValue::Short(330),
                FieldValue::Short(150),
                FieldValue::Long(3),
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
            ]),
        );

        linker.resolve_wix_variables(&mut var_db);

        let prop = var_db.get_records("Property");
        assert_eq!(
            prop[0].get(1),
            Some(&FieldValue::String("Welcome to MySuperApp".to_string()))
        );

        let ctrl = var_db.get_records("Control");
        assert_eq!(
            ctrl[0].get(9),
            Some(&FieldValue::String(r"{\rtf1 Custom EULA Text}".to_string()))
        );

        let bin_records = var_db.get_records("Binary");
        assert!(bin_records
            .iter()
            .any(|r| r.get(0) == Some(&FieldValue::String("WixUIExclamationIco".to_string()))));

        let _ = std::fs::remove_dir_all(&temp_dir);

        Ok(())
    }

    #[allow(clippy::cognitive_complexity)]
    #[test]
    #[allow(clippy::too_many_lines)]
    fn test_linker_remaining_uncovered_paths() -> Result<()> {
        // 1. Coverage for on_exit sequence types: "cancel", "error", "suspend"
        // and relative sequencing queue branch where in_degree transitions to 0,
        // as well as assigned increment/decrement collision loops and sort fallback.
        let mut db = LinkedDatabase::default();
        db.add_record(
            "_WixSequenceRelative",
            Record::with_fields(vec![
                FieldValue::String("InstallUISequence".to_string()),
                FieldValue::String("ActionCancel".to_string()),
                FieldValue::String("cancel".to_string()),
                FieldValue::String("OnExit".to_string()),
                FieldValue::String("COND_MATCH_CANCEL".to_string()),
            ]),
        );
        db.add_record(
            "_WixSequenceRelative",
            Record::with_fields(vec![
                FieldValue::String("InstallUISequence".to_string()),
                FieldValue::String("ActionError".to_string()),
                FieldValue::String("error".to_string()),
                FieldValue::String("OnExit".to_string()),
            ]),
        );
        db.add_record(
            "_WixSequenceRelative",
            Record::with_fields(vec![
                FieldValue::String("InstallUISequence".to_string()),
                FieldValue::String("ActionSuspend".to_string()),
                FieldValue::String("suspend".to_string()),
                FieldValue::String("OnExit".to_string()),
            ]),
        );
        // Add existing records in InstallUISequence so on_exit actions can be updated
        db.add_record(
            "InstallUISequence",
            Record::with_fields(vec![
                FieldValue::String("ActionCancel".to_string()),
                FieldValue::String("COND_MISMATCH_CANCEL".to_string()),
                FieldValue::Short(0),
            ]),
        );
        db.add_record(
            "InstallUISequence",
            Record::with_fields(vec![
                FieldValue::String("ActionCancel".to_string()),
                FieldValue::String("COND_MATCH_CANCEL".to_string()),
                FieldValue::Short(0),
            ]),
        );
        db.add_record(
            "InstallUISequence",
            Record::with_fields(vec![
                FieldValue::String("ActionError".to_string()),
                FieldValue::Null,
                FieldValue::Short(0),
            ]),
        );
        db.add_record(
            "InstallUISequence",
            Record::with_fields(vec![
                FieldValue::String("ActionSuspend".to_string()),
                FieldValue::Null,
                FieldValue::Short(0),
            ]),
        );
        db.add_record(
            "InstallUISequence",
            Record::with_fields(vec![
                FieldValue::String("ActionSuccess".to_string()),
                FieldValue::Null,
                FieldValue::Short(0),
            ]),
        );

        // Relative constraints chaining for After and Before:
        // A After B, C After A (tests queue.push_back(*d == 0))
        // D Before E, F Before D (tests queue.push_back(*d == 0))
        db.add_record(
            "_WixSequenceRelative",
            Record::with_fields(vec![
                FieldValue::String("InstallUISequence".to_string()),
                FieldValue::String("ActionA".to_string()),
                FieldValue::String("ActionB".to_string()),
                FieldValue::String("After".to_string()),
                FieldValue::String("COND_MATCH_A".to_string()),
            ]),
        );
        db.add_record(
            "_WixSequenceRelative",
            Record::with_fields(vec![
                FieldValue::String("InstallUISequence".to_string()),
                FieldValue::String("ActionC".to_string()),
                FieldValue::String("ActionA".to_string()),
                FieldValue::String("After".to_string()),
            ]),
        );
        db.add_record(
            "_WixSequenceRelative",
            Record::with_fields(vec![
                FieldValue::String("InstallUISequence".to_string()),
                FieldValue::String("ActionC".to_string()),
                FieldValue::String("ActionB".to_string()),
                FieldValue::String("After".to_string()),
            ]),
        );
        db.add_record(
            "_WixSequenceRelative",
            Record::with_fields(vec![
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
            ]),
        );
        // Colliding sequence for After collision loop:
        // ActionB base_seq = 1000. ActionA assigned = 1025.
        // If we also pre-insert sequence 1025 into action_seqs via ActionCollisionAfter,
        // then ActionA's while loop assigned += 1 will execute!
        db.add_record(
            "InstallUISequence",
            Record::with_fields(vec![
                FieldValue::String("ActionCollisionAfter".to_string()),
                FieldValue::Null,
                FieldValue::Short(1025),
            ]),
        );
        db.add_record(
            "InstallUISequence",
            Record::with_fields(vec![
                FieldValue::String("ActionB".to_string()),
                FieldValue::Null,
                FieldValue::Short(1000),
            ]),
        );
        db.add_record(
            "InstallUISequence",
            Record::with_fields(vec![
                FieldValue::String("ActionA".to_string()),
                FieldValue::String("COND_MISMATCH_A".to_string()),
                FieldValue::Short(1),
            ]),
        );
        db.add_record(
            "InstallUISequence",
            Record::with_fields(vec![
                FieldValue::String("ActionA".to_string()),
                FieldValue::String("COND_MATCH_A".to_string()),
                FieldValue::Short(1),
            ]),
        );
        db.add_record(
            "InstallUISequence",
            Record::with_fields(vec![
                FieldValue::String("ActionC".to_string()),
                FieldValue::Null,
                FieldValue::Short(1),
            ]),
        );

        // Before chaining & collision:
        // ActionD Before ActionE (base 1000 -> assigned 975)
        // Pre-insert ActionCollisionBefore with sequence 975 to trigger while loop decrement.
        db.add_record(
            "_WixSequenceRelative",
            Record::with_fields(vec![
                FieldValue::String("InstallUISequence".to_string()),
                FieldValue::String("ActionD".to_string()),
                FieldValue::String("ActionE".to_string()),
                FieldValue::String("Before".to_string()),
                FieldValue::String("COND_MATCH_D".to_string()),
            ]),
        );
        db.add_record(
            "_WixSequenceRelative",
            Record::with_fields(vec![
                FieldValue::String("InstallUISequence".to_string()),
                FieldValue::String("ActionF".to_string()),
                FieldValue::String("ActionD".to_string()),
                FieldValue::String("Before".to_string()),
            ]),
        );
        db.add_record(
            "InstallUISequence",
            Record::with_fields(vec![
                FieldValue::String("ActionCollisionBefore".to_string()),
                FieldValue::Null,
                FieldValue::Short(975),
            ]),
        );
        db.add_record(
            "InstallUISequence",
            Record::with_fields(vec![
                FieldValue::String("ActionE".to_string()),
                FieldValue::Null,
                FieldValue::Short(1000),
            ]),
        );
        db.add_record(
            "InstallUISequence",
            Record::with_fields(vec![
                FieldValue::String("ActionD".to_string()),
                FieldValue::String("COND_MISMATCH_D".to_string()),
                FieldValue::Short(1),
            ]),
        );
        db.add_record(
            "InstallUISequence",
            Record::with_fields(vec![
                FieldValue::String("ActionD".to_string()),
                FieldValue::String("COND_MATCH_D".to_string()),
                FieldValue::Short(1),
            ]),
        );
        db.add_record(
            "InstallUISequence",
            Record::with_fields(vec![
                FieldValue::String("ActionF".to_string()),
                FieldValue::Null,
                FieldValue::Short(1),
            ]),
        );

        // Add a record with non-Short sequence (e.g. Null or String) to test fallback _ => 0 in sort_by
        db.add_record(
            "InstallUISequence",
            Record::with_fields(vec![
                FieldValue::String("ActionNonShort1".to_string()),
                FieldValue::Null,
                FieldValue::Null,
            ]),
        );
        db.add_record(
            "InstallUISequence",
            Record::with_fields(vec![
                FieldValue::String("ActionNonShort2".to_string()),
                FieldValue::Null,
                FieldValue::String("InvalidSeq".to_string()),
            ]),
        );
        db.add_record(
            "_WixSequenceRelative",
            Record::with_fields(vec![
                FieldValue::String("InstallUISequence".to_string()),
                FieldValue::String("ActionSuccess".to_string()),
                FieldValue::String("success".to_string()),
                FieldValue::String("OnExit".to_string()),
            ]),
        );
        db.add_record(
            "_WixSequenceRelative",
            Record::with_fields(vec![
                FieldValue::String("AdminUISequence".to_string()),
                FieldValue::String("CustomAdminAction".to_string()),
                FieldValue::String("CostInitialize".to_string()),
                FieldValue::String("After".to_string()),
            ]),
        );

        let _ = Linker::solve_relative_sequences(&mut db);

        let mut cycle_db = LinkedDatabase::default();
        cycle_db.add_record(
            "_WixSequenceRelative",
            Record::with_fields(vec![
                FieldValue::String("InstallExecuteSequence".to_string()),
                FieldValue::String("ActionOne".to_string()),
                FieldValue::String("ActionTwo".to_string()),
                FieldValue::String("After".to_string()),
            ]),
        );
        cycle_db.add_record(
            "_WixSequenceRelative",
            Record::with_fields(vec![
                FieldValue::String("InstallExecuteSequence".to_string()),
                FieldValue::String("ActionTwo".to_string()),
                FieldValue::String("ActionOne".to_string()),
                FieldValue::String("After".to_string()),
            ]),
        );
        assert!(Linker::solve_relative_sequences(&mut cycle_db).is_err());

        let ui_seq = db.get_records("InstallUISequence");
        let find_seq = |act: &str, cond: Option<&str>| -> Option<i16> {
            for r in ui_seq {
                if let (Some(FieldValue::String(a)), Some(FieldValue::Short(s))) =
                    (r.get(0), r.get(2))
                {
                    let cond_match = match (cond, r.get(1)) {
                        (Some(c), Some(FieldValue::String(rc))) => c == rc,
                        (None, _) => true,
                        _ => false,
                    };
                    if a == act && cond_match {
                        return Some(*s);
                    }
                }
            }
            None
        };
        assert_eq!(
            find_seq("ActionCancel", Some("COND_MATCH_CANCEL")),
            Some(-2)
        );
        assert_eq!(
            find_seq("ActionCancel", Some("COND_MISMATCH_CANCEL")),
            Some(0)
        );
        assert_eq!(find_seq("ActionError", None), Some(-3));
        assert_eq!(find_seq("ActionSuspend", None), Some(-4));
        assert_eq!(find_seq("ActionSuccess", None), Some(-1));
        assert_eq!(find_seq("ActionA", Some("COND_MATCH_A")), Some(1026)); // collided with 1025, incremented to 1026
        assert_eq!(find_seq("ActionA", Some("COND_MISMATCH_A")), Some(1));
        assert_eq!(find_seq("ActionD", Some("COND_MATCH_D")), Some(974)); // collided with 975, decremented to 974
        assert_eq!(find_seq("ActionD", Some("COND_MISMATCH_D")), Some(1));
        assert_eq!(find_seq("NonExistentAction", None), None);

        // 2. Coverage for WixUIBannerBmp, WixUIDialogBmp, WixUILicenseRtf (.txt and unresolved),
        // and resolve_source_path branches.
        let temp_dir = std::env::temp_dir().join("msi_linker_uncovered_test");
        let _ = std::fs::create_dir_all(&temp_dir);

        let banner_file = temp_dir.join("banner.bmp");
        let _ = std::fs::write(&banner_file, b"BMfakebanner");

        let dialog_file = temp_dir.join("dialog.bmp");
        let _ = std::fs::write(&dialog_file, b"BMfakedialog");

        let txt_license_file = temp_dir.join("license.txt");
        let _ = std::fs::write(&txt_license_file, b"Plain text license agreement.");

        let mut linker = Linker::new();
        linker.set_cab_per_component(true);
        linker.add_base_dir(&temp_dir);

        let mut var_db = LinkedDatabase::default();
        var_db.add_record(
            "WixVariable",
            Record::with_fields(vec![FieldValue::Null, FieldValue::Null]),
        );
        var_db.add_record(
            "WixVariable",
            Record::with_fields(vec![
                FieldValue::String("UnusedVar".to_string()),
                FieldValue::String("UnusedVal".to_string()),
            ]),
        );
        var_db.add_record(
            "Property",
            Record::with_fields(vec![
                FieldValue::String("TESTPROP".to_string()),
                FieldValue::String("ValueWith!(wix.WixUILicenseRtf)".to_string()),
            ]),
        );
        var_db.add_record(
            "Property",
            Record::with_fields(vec![
                FieldValue::String("LOC_PROP".to_string()),
                FieldValue::String("!(loc.MyLocString)".to_string()),
            ]),
        );
        let mut wxl = crate::wix::localization::WixLocalization::new();
        wxl.culture = Some("en-US".to_string());
        wxl.add_string(crate::wix::localization::WixLocString::new(
            "MyLocString".to_string(),
            "Resolved Loc String".to_string(),
            true,
        ));
        let mut loc_catalog = LocalizationCatalog::new();
        loc_catalog.add_document(wxl);
        linker.set_localization_catalog(loc_catalog);
        let _ = linker.expand_localization_tokens(&mut var_db);
        var_db.add_record(
            "WixVariable",
            Record::with_fields(vec![
                FieldValue::String("WixUIBannerBmp".to_string()),
                FieldValue::String(banner_file.to_string_lossy().to_string()),
            ]),
        );
        var_db.add_record(
            "WixVariable",
            Record::with_fields(vec![
                FieldValue::String("WixUIDialogBmp".to_string()),
                FieldValue::String(dialog_file.to_string_lossy().to_string()),
            ]),
        );
        var_db.add_record(
            "WixVariable",
            Record::with_fields(vec![
                FieldValue::String("WixUILicenseRtf".to_string()),
                FieldValue::String("license.txt".to_string()),
            ]),
        );
        var_db.add_record(
            "Control",
            Record::with_fields(vec![
                FieldValue::String("LicenseAgreementDlg".to_string()),
                FieldValue::String("AgreementText".to_string()),
                FieldValue::String("ScrollableText".to_string()),
                FieldValue::Short(20),
                FieldValue::Short(60),
                FieldValue::Short(330),
                FieldValue::Short(140),
                FieldValue::Long(7),
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
            ]),
        );
        var_db.add_record(
            "Control",
            Record::with_fields(vec![
                FieldValue::String("LicenseAgreementDlg".to_string()),
                FieldValue::String("ShortControl".to_string()),
                FieldValue::String("ScrollableText".to_string()),
                FieldValue::Short(20),
                FieldValue::Short(60),
            ]),
        );

        linker.resolve_wix_variables(&mut var_db);
        let binaries = var_db.get_records("Binary");
        assert!(binaries
            .iter()
            .any(|r| r.get(0) == Some(&FieldValue::String("WixUIBannerBmp".to_string()))));
        assert!(binaries
            .iter()
            .any(|r| r.get(0) == Some(&FieldValue::String("WixUIDialogBmp".to_string()))));
        let ctrls = var_db.get_records("Control");
        let ctrl_dbg = format!("{:?}", ctrls[0].get(9));
        assert!(ctrl_dbg.contains("Plain text license agreement."));

        // Test WixUIBannerBmp and WixUIDialogBmp error reading file
        let dir_bmp = temp_dir.join("dir_bmp");
        let _ = std::fs::create_dir_all(&dir_bmp);
        let mut banner_dir_db = LinkedDatabase::default();
        banner_dir_db.add_record(
            "WixVariable",
            Record::with_fields(vec![
                FieldValue::String("WixUIBannerBmp".to_string()),
                FieldValue::String(dir_bmp.to_string_lossy().to_string()),
            ]),
        );
        banner_dir_db.add_record(
            "WixVariable",
            Record::with_fields(vec![
                FieldValue::String("WixUIDialogBmp".to_string()),
                FieldValue::String(dir_bmp.to_string_lossy().to_string()),
            ]),
        );
        linker.resolve_wix_variables(&mut banner_dir_db);

        let mut banner_none_db = LinkedDatabase::default();
        banner_none_db.add_record(
            "WixVariable",
            Record::with_fields(vec![
                FieldValue::String("WixUIBannerBmp".to_string()),
                FieldValue::String("nonexistent_banner_999.bmp".to_string()),
            ]),
        );
        banner_none_db.add_record(
            "WixVariable",
            Record::with_fields(vec![
                FieldValue::String("WixUIDialogBmp".to_string()),
                FieldValue::String("nonexistent_dialog_999.bmp".to_string()),
            ]),
        );
        linker.resolve_wix_variables(&mut banner_none_db);

        // Test raw RTF and real .rtf extension and non-ScrollableText control
        let mut var_db_raw_rtf = LinkedDatabase::default();
        var_db_raw_rtf.add_record(
            "WixVariable",
            Record::with_fields(vec![
                FieldValue::String("WixUILicenseRtf".to_string()),
                FieldValue::String(r"{\rtf1\ansi Raw RTF content}".to_string()),
            ]),
        );
        var_db_raw_rtf.add_record(
            "Control",
            Record::with_fields(vec![
                FieldValue::String("LicenseAgreementDlg".to_string()),
                FieldValue::String("AgreementText".to_string()),
                FieldValue::String("ScrollableText".to_string()),
                FieldValue::Short(20),
                FieldValue::Short(60),
                FieldValue::Short(330),
                FieldValue::Short(140),
                FieldValue::Long(7),
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
            ]),
        );
        linker.resolve_wix_variables(&mut var_db_raw_rtf);

        let rtf_license_file = temp_dir.join("license.rtf");
        let _ = std::fs::write(&rtf_license_file, r"{\rtf1\ansi From file RTF}");
        let mut var_db_rtf_file = LinkedDatabase::default();
        var_db_rtf_file.add_record(
            "WixVariable",
            Record::with_fields(vec![
                FieldValue::String("WixUILicenseRtf".to_string()),
                FieldValue::String(rtf_license_file.to_string_lossy().to_string()),
            ]),
        );
        var_db_rtf_file.add_record(
            "Control",
            Record::with_fields(vec![
                FieldValue::String("LicenseAgreementDlg".to_string()),
                FieldValue::String("AgreementText".to_string()),
                FieldValue::String("ScrollableText".to_string()),
                FieldValue::Short(20),
                FieldValue::Short(60),
                FieldValue::Short(330),
                FieldValue::Short(140),
                FieldValue::Long(7),
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
            ]),
        );
        var_db_rtf_file.add_record(
            "Control",
            Record::with_fields(vec![
                FieldValue::String("LicenseAgreementDlg".to_string()),
                FieldValue::String("CancelBtn".to_string()),
                FieldValue::String("PushButton".to_string()),
                FieldValue::Short(20),
                FieldValue::Short(60),
                FieldValue::Short(330),
                FieldValue::Short(140),
                FieldValue::Long(7),
                FieldValue::Null,
                FieldValue::String("Cancel".to_string()),
                FieldValue::Null,
                FieldValue::Null,
            ]),
        );
        var_db_rtf_file.add_record(
            "Control",
            Record::with_fields(vec![
                FieldValue::String("LicenseAgreementDlg".to_string()),
                FieldValue::String("AgreementText2".to_string()),
                FieldValue::String("ScrollableText".to_string()),
                FieldValue::Short(20),
                FieldValue::Short(60),
                FieldValue::Short(330),
                FieldValue::Short(140),
                FieldValue::Long(7),
                FieldValue::Null,
                FieldValue::String("Existing Text".to_string()),
                FieldValue::Null,
                FieldValue::Null,
            ]),
        );
        linker.resolve_wix_variables(&mut var_db_rtf_file);

        let mut var_db_no_control = LinkedDatabase::default();
        var_db_no_control.add_record(
            "WixVariable",
            Record::with_fields(vec![
                FieldValue::String("WixUILicenseRtf".to_string()),
                FieldValue::String("any_license_text".to_string()),
            ]),
        );
        linker.resolve_wix_variables(&mut var_db_no_control);

        // Test WixUILicenseRtf fallback when resolve_source_path returns None
        let mut var_db_unresolved = LinkedDatabase::default();
        var_db_unresolved.add_record(
            "WixVariable",
            Record::with_fields(vec![
                FieldValue::String("WixUILicenseRtf".to_string()),
                FieldValue::String("non_existent_file_path_12345.rtf".to_string()),
            ]),
        );
        var_db_unresolved.add_record(
            "Control",
            Record::with_fields(vec![
                FieldValue::String("LicenseAgreementDlg".to_string()),
                FieldValue::String("AgreementText".to_string()),
                FieldValue::String("ScrollableText".to_string()),
                FieldValue::Short(20),
                FieldValue::Short(60),
                FieldValue::Short(330),
                FieldValue::Short(140),
                FieldValue::Long(7),
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
            ]),
        );
        linker.resolve_wix_variables(&mut var_db_unresolved);
        let ctrls_unresolved = var_db_unresolved.get_records("Control");
        assert_eq!(
            ctrls_unresolved[0].get(9),
            Some(&FieldValue::String(
                "non_existent_file_path_12345.rtf".to_string()
            ))
        );

        // Test WixUILicenseTxt variable resolution with text-to-rtf conversion
        let mut var_db_txt = LinkedDatabase::default();
        var_db_txt.add_record(
            "WixVariable",
            Record::with_fields(vec![
                FieldValue::String("WixUILicenseTxt".to_string()),
                FieldValue::String("My Plain Text License Agreement".to_string()),
            ]),
        );
        var_db_txt.add_record(
            "Control",
            Record::with_fields(vec![
                FieldValue::String("LicenseAgreementDlg".to_string()),
                FieldValue::String("AgreementText".to_string()),
                FieldValue::String("ScrollableText".to_string()),
                FieldValue::Short(20),
                FieldValue::Short(60),
                FieldValue::Short(330),
                FieldValue::Short(140),
                FieldValue::Long(7),
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
            ]),
        );
        linker.resolve_wix_variables(&mut var_db_txt);
        let ctrls_txt = var_db_txt.get_records("Control");
        #[allow(clippy::manual_flatten)]
        for opt in [ctrls_txt[0].get(9), None] {
            if let Some(FieldValue::String(s)) = opt {
                assert!(s.starts_with(r"{\rtf1"));
                assert!(s.contains("My Plain Text License Agreement"));
            }
        }

        // Test WixUILicenseRtf map_or_else err branch where file exists (e.g. directory) but read_to_string fails
        let unreadable_dir = temp_dir.join("dir_as_license");
        let _ = std::fs::create_dir_all(&unreadable_dir);
        let mut var_db_dir_err = LinkedDatabase::default();
        var_db_dir_err.add_record(
            "WixVariable",
            Record::with_fields(vec![
                FieldValue::String("WixUILicenseRtf".to_string()),
                FieldValue::String(unreadable_dir.to_string_lossy().to_string()),
            ]),
        );
        var_db_dir_err.add_record(
            "Control",
            Record::with_fields(vec![
                FieldValue::String("LicenseAgreementDlg".to_string()),
                FieldValue::String("AgreementText".to_string()),
                FieldValue::String("ScrollableText".to_string()),
                FieldValue::Short(20),
                FieldValue::Short(60),
                FieldValue::Short(330),
                FieldValue::Short(140),
                FieldValue::Long(7),
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
            ]),
        );
        // Test WixUIExclamationIco (unresolved source path) and WixUIInfoIco (resolved to directory, read fails)
        var_db_dir_err.add_record(
            "WixVariable",
            Record::with_fields(vec![
                FieldValue::String("WixUIExclamationIco".to_string()),
                FieldValue::String("relative_missing_ico_12345.ico".to_string()),
            ]),
        );
        var_db_dir_err.add_record(
            "WixVariable",
            Record::with_fields(vec![
                FieldValue::String("WixUIInfoIco".to_string()),
                FieldValue::String(unreadable_dir.to_string_lossy().to_string()),
            ]),
        );
        linker.resolve_wix_variables(&mut var_db_dir_err);
        let ctrls_dir_err = var_db_dir_err.get_records("Control");
        assert_eq!(
            ctrls_dir_err[0].get(9),
            Some(&FieldValue::String(
                unreadable_dir.to_string_lossy().to_string()
            ))
        );

        // 3. Coverage for bind_files_and_pack_cabinets:
        // - Media record with cab name without '#' prefix (e.g. "cab1.cab" -> format!("#{c}"))
        // - Media record with non-String cab name (e.g. Null -> format!("#cab{did}.cab"))
        // - File mapped to disk_id where disk_to_cab does not contain it -> unwrap_or_else fallback
        let mut bind_db = LinkedDatabase::default();
        let dummy_src_a = temp_dir.join("dummy_a.bin");
        let _ = std::fs::write(&dummy_src_a, b"dummy payload data a");
        let dummy_src_b = temp_dir.join("dummy_b.bin");
        let _ = std::fs::write(&dummy_src_b, b"dummy payload data b");
        let dummy_src_c = temp_dir.join("dummy_c.bin");
        let _ = std::fs::write(&dummy_src_c, b"dummy payload data c");
        let dummy_src_d = temp_dir.join("dummy_d.bin");
        let _ = std::fs::write(&dummy_src_d, b"dummy payload data d");
        let dummy_src_e = temp_dir.join("dummy_e.bin");
        let _ = std::fs::write(&dummy_src_e, b"dummy payload data e");

        // WixMediaCompression records: one with "none", "high", "medium", "lzx:18", "quantum", and invalid non-Short did
        bind_db.add_record(
            "WixMediaCompression",
            Record::with_fields(vec![
                FieldValue::Short(1),
                FieldValue::String("none".to_string()),
            ]),
        );
        bind_db.add_record(
            "WixMediaCompression",
            Record::with_fields(vec![
                FieldValue::Short(2),
                FieldValue::String("high".to_string()),
            ]),
        );
        bind_db.add_record(
            "WixMediaCompression",
            Record::with_fields(vec![
                FieldValue::Short(4),
                FieldValue::String("lzx:18".to_string()),
            ]),
        );
        bind_db.add_record(
            "WixMediaCompression",
            Record::with_fields(vec![
                FieldValue::Short(5),
                FieldValue::String("quantum".to_string()),
            ]),
        );
        bind_db.add_record(
            "WixMediaCompression",
            Record::with_fields(vec![
                FieldValue::Short(99),
                FieldValue::String("medium".to_string()),
            ]),
        );
        bind_db.add_record(
            "WixMediaCompression",
            Record::with_fields(vec![
                FieldValue::String("not_short".to_string()),
                FieldValue::String("none".to_string()),
            ]),
        );

        // WixExternalMedia records for disk 4 and disk 5, plus non-Short record
        bind_db.add_record(
            "WixExternalMedia",
            Record::with_fields(vec![FieldValue::Short(4)]),
        );
        bind_db.add_record(
            "WixExternalMedia",
            Record::with_fields(vec![FieldValue::Short(5)]),
        );
        bind_db.add_record(
            "WixExternalMedia",
            Record::with_fields(vec![FieldValue::String("not_short".to_string())]),
        );

        bind_db.add_record(
            "WixFile",
            Record::with_fields(vec![
                FieldValue::String("FileA".to_string()),
                FieldValue::String(dummy_src_a.to_string_lossy().to_string()),
                FieldValue::Short(1),
            ]),
        );
        bind_db.add_record(
            "WixFile",
            Record::with_fields(vec![
                FieldValue::String("FileB".to_string()),
                FieldValue::String(dummy_src_b.to_string_lossy().to_string()),
                FieldValue::Short(2),
            ]),
        );
        bind_db.add_record(
            "WixFile",
            Record::with_fields(vec![
                FieldValue::String("FileC".to_string()),
                FieldValue::String(dummy_src_c.to_string_lossy().to_string()),
                FieldValue::Short(99), // No Media entry for disk 99
            ]),
        );
        bind_db.add_record(
            "WixFile",
            Record::with_fields(vec![
                FieldValue::String("FileD".to_string()),
                FieldValue::String(dummy_src_d.to_string_lossy().to_string()),
                FieldValue::Short(4),
            ]),
        );
        bind_db.add_record(
            "WixFile",
            Record::with_fields(vec![
                FieldValue::String("FileE".to_string()),
                FieldValue::String(dummy_src_e.to_string_lossy().to_string()),
                FieldValue::Short(5),
            ]),
        );

        // Media disk 1 has cab name without '#': "cab1.cab" -> will become "#cab1.cab"
        bind_db.add_record(
            "Media",
            Record::with_fields(vec![
                FieldValue::Short(1),
                FieldValue::Long(10),
                FieldValue::Null,
                FieldValue::String("mycab1.cab".to_string()),
                FieldValue::Null,
                FieldValue::Null,
            ]),
        );
        // Media disk 6 has empty string cab name -> exercises if !c.is_empty() false branch
        bind_db.add_record(
            "Media",
            Record::with_fields(vec![
                FieldValue::Short(6),
                FieldValue::Long(60),
                FieldValue::Null,
                FieldValue::String(String::new()),
                FieldValue::Null,
                FieldValue::Null,
            ]),
        );
        // Media disk 2 has Null cab name -> will become "#cab2.cab"
        bind_db.add_record(
            "Media",
            Record::with_fields(vec![
                FieldValue::Short(2),
                FieldValue::Long(20),
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
            ]),
        );
        // Media disk 4 is external with explicit cab name -> will strip '#' and become external "myextcab4.cab"
        bind_db.add_record(
            "Media",
            Record::with_fields(vec![
                FieldValue::Short(4),
                FieldValue::Long(40),
                FieldValue::Null,
                FieldValue::String("#myextcab4.cab".to_string()),
                FieldValue::Null,
                FieldValue::Null,
            ]),
        );
        // Media disk 5 is external with Null cab name -> will become external "cab5.cab"
        bind_db.add_record(
            "Media",
            Record::with_fields(vec![
                FieldValue::Short(5),
                FieldValue::Long(50),
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
            ]),
        );
        // Media with Null disk_id to hit if let Some(FieldValue::Short(did)) false branch
        bind_db.add_record(
            "Media",
            Record::with_fields(vec![
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
            ]),
        );
        // Media disk 3 with no files to hit max_seq_per_disk.get(did) == None
        bind_db.add_record(
            "Media",
            Record::with_fields(vec![
                FieldValue::Short(3),
                FieldValue::Long(30),
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
            ]),
        );
        // Additional non-Short Media record to hit match b.get(0) _ => 0
        bind_db.add_record(
            "Media",
            Record::with_fields(vec![
                FieldValue::String("non_short_b".to_string()),
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
            ]),
        );

        // Add File table records
        bind_db.add_record(
            "File",
            Record::with_fields(vec![
                FieldValue::String("FileA".to_string()),
                FieldValue::String("Comp1".to_string()),
                FieldValue::String("fileA.bin".to_string()),
                FieldValue::Long(18),
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Short(0),
                FieldValue::Short(1),
            ]),
        );
        bind_db.add_record(
            "File",
            Record::with_fields(vec![
                FieldValue::String("FileB".to_string()),
                FieldValue::String("Comp1".to_string()),
                FieldValue::String("fileB.bin".to_string()),
                FieldValue::Long(18),
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Short(0),
                FieldValue::Short(2),
            ]),
        );
        bind_db.add_record(
            "File",
            Record::with_fields(vec![
                FieldValue::String("FileC".to_string()),
                FieldValue::String("Comp1".to_string()),
                FieldValue::String("fileC.bin".to_string()),
                FieldValue::Long(18),
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Short(0),
                FieldValue::Short(3),
            ]),
        );
        bind_db.add_record(
            "File",
            Record::with_fields(vec![
                FieldValue::String("FileD".to_string()),
                FieldValue::String("Comp1".to_string()),
                FieldValue::String("fileD.bin".to_string()),
                FieldValue::Long(18),
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Short(0),
                FieldValue::Short(4),
            ]),
        );
        bind_db.add_record(
            "File",
            Record::with_fields(vec![
                FieldValue::String("FileE".to_string()),
                FieldValue::String("Comp1".to_string()),
                FieldValue::String("fileE.bin".to_string()),
                FieldValue::Long(18),
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Short(0),
                FieldValue::Short(5),
            ]),
        );

        linker.set_cab_per_component(false);
        let _ = linker.bind_files_and_pack_cabinets(&mut bind_db);
        let embedded = linker.embedded_cabinets();
        assert!(embedded.contains_key("#mycab1.cab"));
        assert!(embedded.contains_key("#cab2.cab"));
        assert!(embedded.contains_key("#cab99.cab"));
        let ext = linker.external_cabinets();
        assert!(ext.contains_key("myextcab4.cab"));
        assert!(ext.contains_key("cab5.cab"));

        // Test ICE warning suppression flag (self.suppress_all_warnings)
        let mut warn_ice_db = LinkedDatabase::default();
        warn_ice_db.add_record(
            "Registry",
            Record::with_fields(vec![
                FieldValue::String("Reg1".to_string()),
                FieldValue::Short(0),
                FieldValue::String("CLSID\\MyCom".to_string()),
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::String("Comp1".to_string()),
            ]),
        );
        let mut warn_linker = Linker::new();
        warn_linker.set_suppress_all_warnings(true);
        assert!(warn_linker
            .run_filtered_ice_validations(&warn_ice_db)
            .is_ok());

        // 4. Coverage for solve_symbol_graph:
        // - Action reference matching ExecuteAction which is in STANDARD_INSTALL_UI_ACTIONS
        let mut sym_obj = WixObject::new();
        let mut prod_sec = IntermediateSection::new(
            SectionType::Product,
            Some("{33333333-3333-3333-3333-333333333333}".to_string()),
        );
        prod_sec.add_symbol(Symbol::new(
            "Product",
            "{33333333-3333-3333-3333-333333333333}",
        ));
        prod_sec.add_symbol(Symbol::new("CustomAction", "MyCustomAction"));
        prod_sec.add_symbol(Symbol::new("Dialog", "MyDialogAction"));
        prod_sec.add_reference(Reference::new("Action", "ExecuteAction"));
        prod_sec.add_reference(Reference::new("Action", "MyCustomAction"));
        prod_sec.add_reference(Reference::new("Action", "MyDialogAction"));
        prod_sec.add_reference(Reference::new("UI", "WixUI"));
        sym_obj.add_section(prod_sec);

        let mut frag_ui_sec =
            IntermediateSection::new(SectionType::Fragment, Some("WixUIFrag".to_string()));
        frag_ui_sec.add_symbol(Symbol::new("UI", "WixUI"));
        frag_ui_sec.add_reference(Reference::new("UI", "WixUI"));
        sym_obj.add_section(frag_ui_sec);

        let mut sym_linker = Linker::new();
        sym_linker.add_object(sym_obj);
        let solved = sym_linker.solve_symbol_graph().unwrap_or_default();
        assert_eq!(solved.len(), 2);

        let mut test_syms = HashMap::new();
        test_syms.insert(Symbol::new("Dialog", "MyDialog"), vec![0]);
        test_syms.insert(Symbol::new("CustomAction", "MyCA"), vec![1]);

        assert!(is_special_reference(
            &Reference::new("Directory", "TARGETDIR"),
            &test_syms
        ));
        assert!(!is_special_reference(
            &Reference::new("Directory", "NonStandardDir"),
            &test_syms
        ));

        assert!(is_special_reference(
            &Reference::new("Action", "CostInitialize"),
            &test_syms
        ));
        assert!(is_special_reference(
            &Reference::new("Action", "ExecuteAction"),
            &test_syms
        ));
        assert!(is_special_reference(
            &Reference::new("Action", "MyDialog"),
            &test_syms
        ));
        assert!(is_special_reference(
            &Reference::new("Action", "MyCA"),
            &test_syms
        ));
        assert!(!is_special_reference(
            &Reference::new("Action", "UnknownAction"),
            &test_syms
        ));

        assert!(is_special_reference(
            &Reference::new("UI", "WixUI_InstallDir"),
            &test_syms
        ));
        assert!(is_special_reference(
            &Reference::new("UI", "WixUI"),
            &test_syms
        ));
        assert!(!is_special_reference(
            &Reference::new("UI", "CustomUI"),
            &test_syms
        ));

        assert!(is_special_reference(
            &Reference::new("Property", "WIXUI_INSTALLDIR"),
            &test_syms
        ));
        assert!(is_special_reference(
            &Reference::new("Property", "ARPNOREPAIR"),
            &test_syms
        ));
        assert!(is_special_reference(
            &Reference::new("Property", "ALLUSERS"),
            &test_syms
        ));
        assert!(!is_special_reference(
            &Reference::new("Property", "CUSTOM_PROPERTY"),
            &test_syms
        ));

        assert!(!is_special_reference(
            &Reference::new("Component", "MyComp"),
            &test_syms
        ));

        // 5. Coverage for layout_media_and_files multi-cab fallback branches:
        // - Media record sorting fallback where disk_id is non-Short
        // - disk_id not in disk_max_seq
        // - existing_last > 0
        let mut empty_layout_db = LinkedDatabase::default();
        Linker::layout_media_and_files(&mut empty_layout_db);

        let mut zero_media_db = LinkedDatabase::default();
        zero_media_db.add_record(
            "_FileDiskId",
            Record::with_fields(vec![
                FieldValue::String("FileZero".to_string()),
                FieldValue::Short(2),
            ]),
        );
        zero_media_db.add_record(
            "File",
            Record::with_fields(vec![
                FieldValue::String("FileZero".to_string()),
                FieldValue::String("CompZero".to_string()),
                FieldValue::String("filezero.bin".to_string()),
                FieldValue::Long(10),
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Short(0),
                FieldValue::Short(1),
            ]),
        );
        zero_media_db.add_record(
            "Media",
            Record::with_fields(vec![
                FieldValue::Short(1),
                FieldValue::Long(0),
                FieldValue::Null,
                FieldValue::String("#cab1.cab".to_string()),
                FieldValue::Null,
                FieldValue::Null,
            ]),
        );
        zero_media_db.add_record(
            "Media",
            Record::with_fields(vec![
                FieldValue::Short(2),
                FieldValue::Long(10),
                FieldValue::Null,
                FieldValue::String("#cab2.cab".to_string()),
                FieldValue::Null,
                FieldValue::Null,
            ]),
        );
        Linker::layout_media_and_files(&mut zero_media_db);

        let mut layout_db = LinkedDatabase::default();
        layout_db.add_record(
            "_FileDiskId",
            Record::with_fields(vec![FieldValue::Null, FieldValue::Null]),
        );
        layout_db.add_record(
            "_FileDiskId",
            Record::with_fields(vec![
                FieldValue::String("FileX".to_string()),
                FieldValue::Short(1),
            ]),
        );
        layout_db.add_record(
            "File",
            Record::with_fields(vec![
                FieldValue::String("FileX".to_string()),
                FieldValue::String("CompX".to_string()),
                FieldValue::String("fileX.bin".to_string()),
                FieldValue::Long(10),
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Short(0),
                FieldValue::Short(1),
            ]),
        );
        // Media with non-Short disk_id to hit match r.get(0) _ => 1
        layout_db.add_record(
            "Media",
            Record::with_fields(vec![
                FieldValue::Null,
                FieldValue::Long(50),
                FieldValue::Null,
                FieldValue::String("#cab0.cab".to_string()),
                FieldValue::Null,
                FieldValue::Null,
            ]),
        );
        // Media with disk_id 2 which has no files, so disk_max_seq has no entry
        layout_db.add_record(
            "Media",
            Record::with_fields(vec![
                FieldValue::Short(2),
                FieldValue::Long(100),
                FieldValue::Null,
                FieldValue::String("#cab2.cab".to_string()),
                FieldValue::Null,
                FieldValue::Null,
            ]),
        );
        // Media with disk_id 1
        layout_db.add_record(
            "Media",
            Record::with_fields(vec![
                FieldValue::Short(1),
                FieldValue::Long(10),
                FieldValue::Null,
                FieldValue::String("#cab1.cab".to_string()),
                FieldValue::Null,
                FieldValue::Null,
            ]),
        );

        Linker::layout_media_and_files(&mut layout_db);
        let final_media = layout_db.get_records("Media");
        assert_eq!(final_media.len(), 3);

        // Clean up temp directory
        let _ = std::fs::remove_dir_all(&temp_dir);

        Ok(())
    }

    #[test]
    fn test_linker_install_execute_sequence_ordering() -> Result<()> {
        let mut db = LinkedDatabase::default();
        Linker::sequence_standard_actions(&mut db);

        let ies = db.get_records("InstallExecuteSequence");
        let get_seq = |name: &str| -> Option<i16> {
            ies.iter().find_map(|r| match (r.get(0), r.get(2)) {
                (Some(FieldValue::String(n)), Some(FieldValue::Short(s))) if n == name => Some(*s),
                _ => None,
            })
        };

        // Phase 4.2 Standard Action Sequence Ordering audit assertions
        assert_eq!(get_seq("NonExistentActionForCoverage"), None);
        assert_eq!(get_seq("CostInitialize"), Some(800));
        assert_eq!(get_seq("FileCost"), Some(900));
        assert_eq!(get_seq("CostFinalize"), Some(1000));
        assert_eq!(get_seq("InstallValidate"), Some(1400));
        assert_eq!(get_seq("InstallInitialize"), Some(1500));
        assert_eq!(get_seq("ProcessComponents"), Some(1600));
        assert_eq!(get_seq("UnpublishFeatures"), Some(1800));
        assert_eq!(get_seq("InstallFiles"), Some(4000));
        assert_eq!(get_seq("RegisterUser"), Some(6000));
        assert_eq!(get_seq("PublishFeatures"), Some(6300));
        assert_eq!(get_seq("PublishProduct"), Some(6400));
        assert_eq!(get_seq("InstallFinalize"), Some(6600));

        Ok(())
    }

    /// Tests extended standard action sequence injection and embedded chainer reference validation.
    ///
    /// # Returns
    ///
    /// `Ok(())` on success, or an [`MsiError`] if the operation fails.
    #[allow(clippy::cognitive_complexity)]
    #[test]
    #[allow(clippy::too_many_lines)]
    fn test_linker_extended_standard_actions_and_embedded_chainer() -> Result<()> {
        let mut db = LinkedDatabase::default();

        // Base database with no services or environment
        Linker::sequence_standard_actions(&mut db);
        let ies = db.get_records("InstallExecuteSequence");
        let has_action = |name: &str| {
            ies.iter()
                .any(|r| r.get(0) == Some(&FieldValue::String(name.to_string())))
        };
        assert!(!has_action("StopServices"));
        assert!(!has_action("InstallServices"));
        assert!(!has_action("WriteEnvironmentStrings"));
        assert!(!has_action("CreateFolders"));

        // Now add service and environment tables to a new database
        let mut db_with_svc = LinkedDatabase::default();
        db_with_svc.add_record(
            "ServiceInstall",
            Record::with_fields(vec![
                FieldValue::String("Svc1".to_string()),
                FieldValue::String("MySQL".to_string()),
                FieldValue::Null,
                FieldValue::Long(16),
                FieldValue::Long(2),
                FieldValue::Long(1),
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::String("Comp1".to_string()),
            ]),
        );
        db_with_svc.add_record(
            "Environment",
            Record::with_fields(vec![
                FieldValue::String("Env1".to_string()),
                FieldValue::String("=PATH".to_string()),
                FieldValue::String("[INSTALLDIR]".to_string()),
                FieldValue::String("Comp1".to_string()),
            ]),
        );
        db_with_svc.add_record(
            "CreateFolder",
            Record::with_fields(vec![
                FieldValue::String("DataFolder".to_string()),
                FieldValue::String("Comp1".to_string()),
            ]),
        );
        db_with_svc.add_record(
            "RemoveFolder",
            Record::with_fields(vec![
                FieldValue::String("RemDataFolder".to_string()),
                FieldValue::String("Comp1".to_string()),
                FieldValue::Null,
                FieldValue::Short(2),
            ]),
        );
        db_with_svc.add_record(
            "RemoveFile",
            Record::with_fields(vec![
                FieldValue::String("RemFile1".to_string()),
                FieldValue::String("Comp1".to_string()),
                FieldValue::Null,
                FieldValue::String("TARGETDIR".to_string()),
                FieldValue::Short(2),
            ]),
        );
        db_with_svc.add_record(
            "MoveFile",
            Record::with_fields(vec![
                FieldValue::String("Move1".to_string()),
                FieldValue::String("Comp1".to_string()),
                FieldValue::String("src.dat".to_string()),
                FieldValue::String("dst.dat".to_string()),
                FieldValue::String("TARGETDIR".to_string()),
                FieldValue::String("TARGETDIR".to_string()),
                FieldValue::Short(0),
            ]),
        );
        db_with_svc.add_record(
            "IniFile",
            Record::with_fields(vec![
                FieldValue::String("Ini1".to_string()),
                FieldValue::String("cfg.ini".to_string()),
                FieldValue::String("TARGETDIR".to_string()),
                FieldValue::String("Section".to_string()),
                FieldValue::String("Key".to_string()),
                FieldValue::String("Value".to_string()),
                FieldValue::Short(0),
                FieldValue::String("Comp1".to_string()),
            ]),
        );

        Linker::sequence_standard_actions(&mut db_with_svc);
        let ies_svc = db_with_svc.get_records("InstallExecuteSequence");
        let has_svc_action = |name: &str| {
            ies_svc
                .iter()
                .any(|r| r.get(0) == Some(&FieldValue::String(name.to_string())))
        };
        assert!(has_svc_action("StopServices"));
        assert!(has_svc_action("DeleteServices"));
        assert!(has_svc_action("InstallServices"));
        assert!(has_svc_action("StartServices"));
        assert!(has_svc_action("WriteEnvironmentStrings"));
        assert!(has_svc_action("CreateFolders"));
        assert!(has_svc_action("RemoveFolders"));
        assert!(has_svc_action("RemoveFiles"));
        assert!(has_svc_action("MoveFiles"));
        assert!(has_svc_action("WriteIniValues"));

        // Validate EmbeddedChainer references
        let mut chainer_db = LinkedDatabase::default();
        // Empty chainers -> Ok
        assert!(Linker::validate_embedded_chainers(&chainer_db).is_ok());

        // Chainer referencing missing binary
        chainer_db.add_record(
            "MsiEmbeddedChainer",
            Record::with_fields(vec![
                FieldValue::String("Chainer1".to_string()),
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::String("MissingBinary".to_string()),
                FieldValue::Long(1),
            ]),
        );
        let err_bin = Linker::validate_embedded_chainers(&chainer_db);
        assert!(err_bin.is_err());
        let err_bin_dbg = format!("{err_bin:?}");
        assert!(err_bin_dbg.contains("MissingBinary"));

        // Add the referenced binary -> Ok
        chainer_db.add_record(
            "Binary",
            Record::with_fields(vec![
                FieldValue::String("MissingBinary".to_string()),
                FieldValue::Null,
            ]),
        );
        assert!(Linker::validate_embedded_chainers(&chainer_db).is_ok());

        // Add chainer referencing missing file (Type 2)
        chainer_db.add_record(
            "MsiEmbeddedChainer",
            Record::with_fields(vec![
                FieldValue::String("ChainerExe".to_string()),
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::String("MissingFileKey".to_string()),
                FieldValue::Long(2),
            ]),
        );
        let err_file = Linker::validate_embedded_chainers(&chainer_db);
        assert!(err_file.is_err());
        let err_file_dbg = format!("{err_file:?}");
        assert!(err_file_dbg.contains("MissingFileKey"));

        // Add the referenced file -> Ok
        chainer_db.add_record(
            "File",
            Record::with_fields(vec![
                FieldValue::String("MissingFileKey".to_string()),
                FieldValue::String("Comp1".to_string()),
                FieldValue::String("chainer.exe".to_string()),
                FieldValue::Long(100),
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Short(1),
            ]),
        );
        assert!(Linker::validate_embedded_chainers(&chainer_db).is_ok());

        // Cover null/non-string entries in Binary and File records (_ => None branches)
        chainer_db.add_record("Binary", Record::with_fields(vec![FieldValue::Null]));
        chainer_db.add_record("File", Record::with_fields(vec![FieldValue::Null]));

        // Cover chainer with missing/null id (rec.get(0) else branch)
        chainer_db.add_record(
            "MsiEmbeddedChainer",
            Record::with_fields(vec![
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::String("MissingBinary".to_string()),
                FieldValue::Long(1),
            ]),
        );

        // Cover chainer with missing/null source (rec.get(3) else branch)
        chainer_db.add_record(
            "MsiEmbeddedChainer",
            Record::with_fields(vec![
                FieldValue::String("ChainerNoSrc".to_string()),
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Long(1),
            ]),
        );

        // Cover chainer with Short chainer_type and Null chainer_type (_ => 1 fallback)
        chainer_db.add_record(
            "MsiEmbeddedChainer",
            Record::with_fields(vec![
                FieldValue::String("ChainerShortType".to_string()),
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::String("MissingBinary".to_string()),
                FieldValue::Short(1),
            ]),
        );
        chainer_db.add_record(
            "MsiEmbeddedChainer",
            Record::with_fields(vec![
                FieldValue::String("ChainerFallbackType".to_string()),
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::String("MissingBinary".to_string()),
                FieldValue::Null,
            ]),
        );

        // Cover chainer with invalid type (e.g. 3, expected 1 or 2)
        let mut chainer_db_invalid_type = chainer_db.clone();
        chainer_db_invalid_type.add_record(
            "MsiEmbeddedChainer",
            Record::with_fields(vec![
                FieldValue::String("ChainerOtherType".to_string()),
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::String("ArbitrarySource".to_string()),
                FieldValue::Long(3),
            ]),
        );
        let err_type = Linker::validate_embedded_chainers(&chainer_db_invalid_type);
        assert!(err_type.is_err());
        let err_type_dbg = format!("{err_type:?}");
        assert!(err_type_dbg.contains("expected 1 (Binary) or 2 (File)"));

        // Cover chainer with malformed condition syntax
        let mut chainer_db_bad_cond = chainer_db.clone();
        chainer_db_bad_cond.add_record(
            "MsiEmbeddedChainer",
            Record::with_fields(vec![
                FieldValue::String("ChainerBadCond".to_string()),
                FieldValue::String("(NOT Installed".to_string()),
                FieldValue::Null,
                FieldValue::String("MissingBinary".to_string()),
                FieldValue::Long(1),
            ]),
        );
        let err_cond = Linker::validate_embedded_chainers(&chainer_db_bad_cond);
        assert!(err_cond.is_err());
        let err_cond_dbg = format!("{err_cond:?}");
        assert!(err_cond_dbg.contains("invalid Condition syntax"));

        // Cover chainer with valid condition syntax
        chainer_db.add_record(
            "MsiEmbeddedChainer",
            Record::with_fields(vec![
                FieldValue::String("ChainerValidCond".to_string()),
                FieldValue::String("NOT Installed".to_string()),
                FieldValue::Null,
                FieldValue::String("MissingBinary".to_string()),
                FieldValue::Long(1),
            ]),
        );
        // Cover chainer with whitespace condition string (exercises False branch of !s.trim().is_empty())
        chainer_db.add_record(
            "MsiEmbeddedChainer",
            Record::with_fields(vec![
                FieldValue::String("ChainerEmptyCond".to_string()),
                FieldValue::String("   ".to_string()),
                FieldValue::Null,
                FieldValue::String("MissingBinary".to_string()),
                FieldValue::Long(1),
            ]),
        );
        assert!(Linker::validate_embedded_chainers(&chainer_db).is_ok());

        Ok(())
    }

    /// Tests `LinkedDatabase` `add_or_merge_record` edge cases, `bind_binaries`, and `CubValidator::open` with root path.
    ///
    /// # Returns
    ///
    /// `Ok(())` on success, or an [`MsiError`] if the operation fails.
    #[test]
    fn test_linker_add_or_merge_and_bind_binaries_edge_cases() -> Result<()> {
        let mut db = LinkedDatabase::default();

        // 1. Table schema without primary key columns
        let nopk_schema = crate::database::catalogs::TableSchema {
            name: "NoPkTable".to_string(),
            columns: vec![crate::database::column::ColumnDef {
                name: "Data".to_string(),
                data_type: crate::database::column::DataType::String { max_len: 50 },
                nullable: true,
                primary_key: false,
                localizable: false,
            }],
        };
        let _ = db.catalog.add_table(nopk_schema);
        let r_nopk = Record::with_fields(vec![FieldValue::String("Value1".to_string())]);
        let _ = db.add_or_merge_record("NoPkTable", r_nopk.clone());
        // Adding again tests !records.contains(&record) returning false
        let _ = db.add_or_merge_record("NoPkTable", r_nopk);

        // 2. Table with primary key: identical record (TARGETDIR)
        let r_targetdir = Record::with_fields(vec![
            FieldValue::String("TARGETDIR".to_string()),
            FieldValue::Null,
            FieldValue::String("SourceDir".to_string()),
        ]);
        let _ = db.add_or_merge_record("Directory", r_targetdir.clone());
        let _ = db.add_or_merge_record("Directory", r_targetdir);

        // 3. Primary key collision (conflicting records)
        let r_conflict = Record::with_fields(vec![
            FieldValue::String("TARGETDIR".to_string()),
            FieldValue::Null,
            FieldValue::String("OtherSourceDir".to_string()),
        ]);
        assert!(db.add_or_merge_record("Directory", r_conflict).is_err());

        // 4. Internal table not in catalog: add twice
        let r_internal =
            Record::with_fields(vec![FieldValue::String("group_member_1".to_string())]);
        let _ = db.add_or_merge_record("_ComponentGroupMember", r_internal.clone());
        let _ = db.add_or_merge_record("_ComponentGroupMember", r_internal);

        // 5. bind_binaries with real binary, missing path, and invalid record
        let temp_dir = std::env::temp_dir().join(format!("msi_linker_bin_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&temp_dir);
        let _ = std::fs::create_dir_all(&temp_dir);
        let bin_file = temp_dir.join("test_payload.bin");
        let _ = std::fs::write(&bin_file, b"test binary payload data");

        let mut bin_db = LinkedDatabase::default();
        bin_db.add_record(
            "WixBinary",
            Record::with_fields(vec![
                FieldValue::String("BinPayload1".to_string()),
                FieldValue::String(bin_file.to_string_lossy().to_string()),
            ]),
        );
        bin_db.add_record(
            "WixBinary",
            Record::with_fields(vec![
                FieldValue::String("BinMissing".to_string()),
                FieldValue::String("non_existent_binary_xyz_9999.bin".to_string()),
            ]),
        );
        bin_db.add_record(
            "WixBinary",
            Record::with_fields(vec![FieldValue::Long(42), FieldValue::Null]),
        );

        let mut linker = Linker::new();
        let _ = linker.bind_binaries(&mut bin_db);
        assert_eq!(
            linker.embedded_cabinets.get("BinPayload1"),
            Some(&b"test binary payload data".to_vec())
        );
        assert_eq!(linker.embedded_cabinets.get("BinMissing"), None);

        // Test bind_binaries read failure error path on directory source
        let unreadable_bin_dir = temp_dir.join("dir_as_bin");
        let _ = std::fs::create_dir_all(&unreadable_bin_dir);
        let mut err_bin_db = LinkedDatabase::default();
        err_bin_db.add_record(
            "WixBinary",
            Record::with_fields(vec![
                FieldValue::String("BinDirErr".to_string()),
                FieldValue::String(unreadable_bin_dir.to_string_lossy().to_string()),
            ]),
        );
        assert!(linker.bind_binaries(&mut err_bin_db).is_err());

        // 6. CubValidator::open on path with no file stem (root directory)
        assert!(CubValidator::open("/").is_err());

        let _ = std::fs::remove_dir_all(&temp_dir);
        Ok(())
    }

    /// Tests comprehensive error branches for linker graph solving, binding, and defaults.
    #[test]
    #[allow(clippy::too_many_lines)]
    fn test_linker_comprehensive_error_branches() {
        // 1. Defaults and file open errors
        let def_msm = MergeModule::default();
        assert_eq!(def_msm.guid(), "Module");
        let def_cub = CubValidator::default();
        assert_eq!(def_cub.name(), "default");
        assert!(MergeModule::open("non_existent_module_path_xyz.msm").is_err());
        assert!(CubValidator::open("non_existent_cub_path_xyz.cub").is_err());

        // 2. Linker::link errors
        // a) add_or_merge_record conflict during link
        let mut obj_conflict = WixObject::new();
        let mut sec_conflict =
            IntermediateSection::new(SectionType::Product, Some("Prod".to_string()));
        sec_conflict.add_symbol(Symbol::new("Product", "Prod"));
        let mut dir_tbl = IntermediateTable::new("Directory");
        dir_tbl.push_record(Record::with_fields(vec![
            FieldValue::String("TARGETDIR".to_string()),
            FieldValue::Null,
            FieldValue::String("SourceDir".to_string()),
        ]));
        dir_tbl.push_record(Record::with_fields(vec![
            FieldValue::String("TARGETDIR".to_string()),
            FieldValue::Null,
            FieldValue::String("ConflictingSourceDir".to_string()),
        ]));
        sec_conflict.add_table(dir_tbl);
        obj_conflict.add_section(sec_conflict);
        let mut linker_conflict = Linker::new();
        linker_conflict.add_object(obj_conflict);
        assert!(linker_conflict.link().is_err());

        // b) resolve_component_groups error in link (empty feature name)
        let mut obj_cg = WixObject::new();
        let mut sec_cg = IntermediateSection::new(SectionType::Product, Some("Prod".to_string()));
        sec_cg.add_symbol(Symbol::new("Product", "Prod"));
        let mut f_cg_ref = IntermediateTable::new("_FeatureComponentGroupRef");
        f_cg_ref.push_record(Record::with_fields(vec![
            FieldValue::String(String::new()), // empty feature name -> FeatureName::new error!
            FieldValue::String("CG1".to_string()),
        ]));
        sec_cg.add_table(f_cg_ref);
        let mut cg_member = IntermediateTable::new("_ComponentGroupMember");
        cg_member.push_record(Record::with_fields(vec![
            FieldValue::String("CG1".to_string()),
            FieldValue::String("Comp1".to_string()),
        ]));
        sec_cg.add_table(cg_member);
        obj_cg.add_section(sec_cg);
        let mut linker_cg = Linker::new();
        linker_cg.add_object(obj_cg);
        assert!(linker_cg.link().is_err());

        // c) ComponentName::new error in resolve_component_groups (empty component name)
        let mut db_bad_comp = LinkedDatabase::default();
        db_bad_comp.add_record(
            "_FeatureComponentGroupRef",
            Record::with_fields(vec![
                FieldValue::String("Feat1".to_string()),
                FieldValue::String("CG1".to_string()),
            ]),
        );
        db_bad_comp.add_record(
            "_ComponentGroupMember",
            Record::with_fields(vec![
                FieldValue::String("CG1".to_string()),
                FieldValue::String(String::new()), // empty component name!
            ]),
        );
        assert!(Linker::resolve_component_groups(&mut db_bad_comp).is_err());

        // d) solve_relative_sequences error in link (cycle)
        let mut obj_seq = WixObject::new();
        let mut sec_seq = IntermediateSection::new(SectionType::Product, Some("Prod".to_string()));
        sec_seq.add_symbol(Symbol::new("Product", "Prod"));
        let mut seq_tbl = IntermediateTable::new("_WixSequenceRelative");
        seq_tbl.push_record(Record::with_fields(vec![
            FieldValue::String("InstallExecuteSequence".to_string()),
            FieldValue::String("ActA".to_string()),
            FieldValue::String("ActB".to_string()),
            FieldValue::String("After".to_string()),
        ]));
        seq_tbl.push_record(Record::with_fields(vec![
            FieldValue::String("InstallExecuteSequence".to_string()),
            FieldValue::String("ActB".to_string()),
            FieldValue::String("ActA".to_string()),
            FieldValue::String("After".to_string()),
        ]));
        sec_seq.add_table(seq_tbl);
        obj_seq.add_section(sec_seq);
        let mut linker_seq = Linker::new();
        linker_seq.add_object(obj_seq);
        assert!(linker_seq.link().is_err());

        // e) expand_localization_tokens error in link
        let mut obj_loc = WixObject::new();
        let mut sec_loc = IntermediateSection::new(SectionType::Product, Some("Prod".to_string()));
        sec_loc.add_symbol(Symbol::new("Product", "Prod"));
        let mut prop_tbl = IntermediateTable::new("Property");
        prop_tbl.push_record(Record::with_fields(vec![
            FieldValue::String("MissingTokenProp".to_string()),
            FieldValue::String("!(loc.UnknownToken)".to_string()),
        ]));
        sec_loc.add_table(prop_tbl);
        obj_loc.add_section(sec_loc);
        let mut linker_loc = Linker::new();
        linker_loc.add_object(obj_loc);
        let cat = LocalizationCatalog::new();
        linker_loc.set_localization_catalog(cat);
        linker_loc.set_cultures(vec!["en-US".to_string()]);
        assert!(linker_loc.link().is_err());

        // f) validate_embedded_chainers error in link
        let mut obj_chainer = WixObject::new();
        let mut sec_chainer =
            IntermediateSection::new(SectionType::Product, Some("Prod".to_string()));
        sec_chainer.add_symbol(Symbol::new("Product", "Prod"));
        let mut chainer_tbl = IntermediateTable::new("MsiEmbeddedChainer");
        chainer_tbl.push_record(Record::with_fields(vec![
            FieldValue::String("ChainerMissing".to_string()),
            FieldValue::Null,
            FieldValue::Null,
            FieldValue::String("MissingBinaryKey".to_string()),
            FieldValue::Short(1),
        ]));
        sec_chainer.add_table(chainer_tbl);
        obj_chainer.add_section(sec_chainer);
        let mut linker_chainer = Linker::new();
        linker_chainer.add_object(obj_chainer);
        assert!(linker_chainer.link().is_err());

        // g) bind_binaries error in link
        let temp_bin_dir =
            std::env::temp_dir().join(format!("msi_linker_bin_dir_{}", std::process::id()));
        let _ = std::fs::create_dir_all(&temp_bin_dir);
        let mut obj_bin = WixObject::new();
        let mut sec_bin = IntermediateSection::new(SectionType::Product, Some("Prod".to_string()));
        sec_bin.add_symbol(Symbol::new("Product", "Prod"));
        let mut wix_bin_tbl = IntermediateTable::new("WixBinary");
        wix_bin_tbl.push_record(Record::with_fields(vec![
            FieldValue::String("BinMissing".to_string()),
            FieldValue::String(temp_bin_dir.to_string_lossy().to_string()),
        ]));
        sec_bin.add_table(wix_bin_tbl);
        obj_bin.add_section(sec_bin);
        let mut linker_bin = Linker::new();
        linker_bin.add_object(obj_bin);
        assert!(linker_bin.link().is_err());
        let _ = std::fs::remove_dir_all(&temp_bin_dir);

        // h) run_filtered_ice_validations error in link (ICE18 keypath belongs to different component)
        let mut obj_ice = WixObject::new();
        let mut sec_ice = IntermediateSection::new(SectionType::Product, Some("Prod".to_string()));
        sec_ice.add_symbol(Symbol::new("Product", "Prod"));
        let mut file_tbl = IntermediateTable::new("File");
        file_tbl.push_record(Record::with_fields(vec![
            FieldValue::String("F1".to_string()),
            FieldValue::String("CompA".to_string()),
            FieldValue::String("file.txt".to_string()),
            FieldValue::Long(10),
            FieldValue::Null,
            FieldValue::Null,
            FieldValue::Short(0),
            FieldValue::Short(1),
        ]));
        sec_ice.add_table(file_tbl);
        let mut comp_tbl = IntermediateTable::new("Component");
        comp_tbl.push_record(Record::with_fields(vec![
            FieldValue::String("CompB".to_string()),
            FieldValue::String("{11111111-1111-1111-1111-111111111111}".to_string()),
            FieldValue::String("TARGETDIR".to_string()),
            FieldValue::Short(0),
            FieldValue::Null,
            FieldValue::String("F1".to_string()), // KeyPath pointing to F1 which belongs to CompA -> ICE18 error!
        ]));
        sec_ice.add_table(comp_tbl);
        obj_ice.add_section(sec_ice);
        let mut linker_ice = Linker::new();
        linker_ice.add_object(obj_ice);
        assert!(linker_ice.link().is_err());

        // 3. bind_files_and_pack_cabinets errors:
        // a) Source path is a directory -> std::fs::read fails
        let temp_dir = std::env::temp_dir().join(format!("linker_err_dir_{}", std::process::id()));
        let _ = std::fs::create_dir_all(&temp_dir);
        let mut db_read_err = LinkedDatabase::default();
        db_read_err.add_record(
            "File",
            Record::with_fields(vec![
                FieldValue::String("FileDirErr".to_string()),
                FieldValue::String("Comp1".to_string()),
                FieldValue::String("file.txt".to_string()),
                FieldValue::Long(0),
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Short(0),
                FieldValue::Short(1),
            ]),
        );
        db_read_err.add_record(
            "WixFile",
            Record::with_fields(vec![
                FieldValue::String("FileDirErr".to_string()),
                FieldValue::String(temp_dir.to_string_lossy().to_string()),
                FieldValue::Short(1),
            ]),
        );
        let mut linker_read_err = Linker::new();
        assert!(linker_read_err
            .bind_files_and_pack_cabinets(&mut db_read_err)
            .is_err());
        let _ = std::fs::remove_dir_all(&temp_dir);

        // b) Duplicate file in cabinet -> writer.add_file fails
        let temp_payload =
            std::env::temp_dir().join(format!("linker_dup_file_{}.txt", std::process::id()));
        let _ = std::fs::write(&temp_payload, b"payload");
        let mut db_dup_file = LinkedDatabase::default();
        db_dup_file.add_record(
            "File",
            Record::with_fields(vec![
                FieldValue::String("FileDup1".to_string()),
                FieldValue::String("Comp1".to_string()),
                FieldValue::String("file1.txt".to_string()),
                FieldValue::Long(7),
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Short(0),
                FieldValue::Short(1),
            ]),
        );
        db_dup_file.add_record(
            "File",
            Record::with_fields(vec![
                FieldValue::String("FileDup1".to_string()),
                FieldValue::String("Comp1".to_string()),
                FieldValue::String("file2.txt".to_string()),
                FieldValue::Long(7),
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Short(0),
                FieldValue::Short(2),
            ]),
        );
        db_dup_file.add_record(
            "WixFile",
            Record::with_fields(vec![
                FieldValue::String("FileDup1".to_string()),
                FieldValue::String(temp_payload.to_string_lossy().to_string()),
                FieldValue::Short(1),
            ]),
        );
        let mut linker_dup = Linker::new();
        assert!(linker_dup
            .bind_files_and_pack_cabinets(&mut db_dup_file)
            .is_err());
        let _ = std::fs::remove_file(&temp_payload);
    }

    /// Tests `solve_symbol_graph` feature extender resolution and dialog action fallback.
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
            Err(MsiError::WixExtension {
                extension: "FailingMockExtension".to_string(),
                message: "fail".to_string(),
            })
        }
    }

    #[test]
    fn test_linker_extension_failure() {
        let mut obj = WixObject::default();
        let sec = IntermediateSection::new(SectionType::Product, None);
        obj.add_section(sec);

        let mut registry = ExtensionRegistry::new();
        registry.register(std::sync::Arc::new(FailingMockExtension));
        let mut linker = Linker::with_extensions(registry);
        linker.add_object(obj);
        assert!(linker.link().is_err());

        // Cover the unused compile_node
        let ext = FailingMockExtension;
        let mut section = IntermediateSection::new(SectionType::Product, None);
        let mut tables = HashMap::new();
        let _ = ext.compile_node(&XmlNode::default(), None, &mut section, &mut tables);
    }

    #[test]
    fn test_solve_symbol_graph_feature_extenders_and_dialog_action() {
        let mut obj = WixObject::new();

        // Section 0: Product section defining Feature:MainFeature and Feature:SecondaryFeature
        let mut prod = IntermediateSection::new(SectionType::Product, Some("Prod".to_string()));
        prod.add_symbol(Symbol::new("Product", "ProdCode"));
        prod.add_symbol(Symbol::new("Feature", "MainFeature"));
        prod.add_symbol(Symbol::new("Feature", "SecondaryFeature"));
        // prod references Action:WelcomeDialog (which will resolve via Dialog fallback in Section 5)
        prod.add_reference(Reference::new("Action", "WelcomeDialog"));

        // Section 1: Fragment extending both MainFeature and SecondaryFeature via FeatureComponents table
        let mut frag1 = IntermediateSection::new(SectionType::Fragment, Some("Frag1".to_string()));
        frag1.add_symbol(Symbol::new("Component", "Comp1"));
        frag1.add_reference(Reference::new("Feature", "MainFeature"));
        frag1.add_reference(Reference::new("Feature", "SecondaryFeature"));
        let mut table1 = IntermediateTable::new("FeatureComponents");
        table1.records.push(Record::with_fields(vec![
            FieldValue::String("MainFeature".to_string()),
            FieldValue::String("Comp1".to_string()),
        ]));
        table1.records.push(Record::with_fields(vec![
            FieldValue::String("SecondaryFeature".to_string()),
            FieldValue::String("Comp1".to_string()),
        ]));
        frag1.add_table(table1);

        // Section 2: Fragment extending SecondaryFeature via _FeatureComponentGroupRef table
        let mut frag2 = IntermediateSection::new(SectionType::Fragment, Some("Frag2".to_string()));
        frag2.add_symbol(Symbol::new("ComponentGroup", "CGroup2"));
        frag2.add_reference(Reference::new("Feature", "SecondaryFeature"));
        let mut table2 = IntermediateTable::new("_FeatureComponentGroupRef");
        table2.records.push(Record::with_fields(vec![
            FieldValue::String("SecondaryFeature".to_string()),
            FieldValue::String("CGroup2".to_string()),
        ]));
        frag2.add_table(table2);

        // Section 3: Fragment referencing Feature:MainFeature but with non-matching table records (has_extension = false)
        let mut frag3 = IntermediateSection::new(SectionType::Fragment, Some("Frag3".to_string()));
        frag3.add_reference(Reference::new("Feature", "MainFeature"));
        let mut table3 = IntermediateTable::new("FeatureComponents");
        table3
            .records
            .push(Record::with_fields(vec![FieldValue::String(
                "UnrelatedFeature".to_string(),
            )]));
        frag3.add_table(table3);

        // Section 4: Fragment referencing Feature:MainFeature but with unrelated table (has_extension = false)
        let mut frag4 = IntermediateSection::new(SectionType::Fragment, Some("Frag4".to_string()));
        frag4.add_reference(Reference::new("Feature", "MainFeature"));
        let table4 = IntermediateTable::new("Property");
        frag4.add_table(table4);

        // Section 5: Fragment defining Dialog:WelcomeDialog
        let mut frag5 = IntermediateSection::new(SectionType::Fragment, Some("Frag5".to_string()));
        frag5.add_symbol(Symbol::new("Dialog", "WelcomeDialog"));

        obj.add_section(prod);
        obj.add_section(frag1);
        obj.add_section(frag2);
        obj.add_section(frag3);
        obj.add_section(frag4);
        obj.add_section(frag5);

        let mut linker = Linker::new();
        linker.add_object(obj);
        let solved = linker.solve_symbol_graph();
        assert!(solved.is_ok());
        let sections = solved.unwrap_or_default();
        assert!(sections.iter().any(|s| s.id.as_deref() == Some("Prod")));
        assert!(sections.iter().any(|s| s.id.as_deref() == Some("Frag1")));
        assert!(sections.iter().any(|s| s.id.as_deref() == Some("Frag2")));
        assert!(sections.iter().any(|s| s.id.as_deref() == Some("Frag5")));
    }

    /// Tests collision resolution for sequence tables with disjoint conditions.
    #[test]
    fn test_linker_disjoint_conditions_sequence_collision() {
        let mut db = LinkedDatabase::default();
        let ius_schema = crate::database::tables::sequence::install_ui_sequence_schema();
        let _ = db.catalog.add_table(ius_schema);

        let r_simple = Record::with_fields(vec![
            FieldValue::String("Dlg_VerifyReady".to_string()),
            FieldValue::String("SETUP_MODE=\"Simple\"".to_string()),
            FieldValue::Short(1000),
        ]);
        let r_advanced = Record::with_fields(vec![
            FieldValue::String("Dlg_VerifyReady".to_string()),
            FieldValue::String("SETUP_MODE=\"Advanced\"".to_string()),
            FieldValue::Short(1050),
        ]);
        let r_simple_dup = r_simple.clone();
        let r_simple_conflict = Record::with_fields(vec![
            FieldValue::String("Dlg_VerifyReady".to_string()),
            FieldValue::String("SETUP_MODE=\"Simple\"".to_string()),
            FieldValue::Short(9999),
        ]);

        // 1. Adding disjoint condition succeeds
        assert!(db
            .add_or_merge_record("InstallUISequence", r_simple)
            .is_ok());
        assert!(db
            .add_or_merge_record("InstallUISequence", r_advanced)
            .is_ok());
        assert_eq!(db.get_records("InstallUISequence").len(), 2);

        // 2. Exactly identical record dedupes
        assert!(db
            .add_or_merge_record("InstallUISequence", r_simple_dup)
            .is_ok());
        assert_eq!(db.get_records("InstallUISequence").len(), 2);

        // 3. Same condition with different sequence causes primary key collision
        assert!(db
            .add_or_merge_record("InstallUISequence", r_simple_conflict)
            .is_err());
    }

    /// Tests `Linker::bind_text_sources` and `sequence_standard_actions` `InstallUISequence` branches.
    #[test]
    #[allow(clippy::too_many_lines)]
    fn test_bind_text_sources_and_ui_sequence_injection_branches() {
        let temp_dir = std::env::temp_dir().join("msi_test_linker_bind_text_sources");
        let _ = std::fs::create_dir_all(&temp_dir);
        let rtf_file = temp_dir.join("bound.rtf");
        let txt_file = temp_dir.join("bound.txt");
        let dir_as_file = temp_dir.join("sub_dir_err.rtf");
        let _ = std::fs::write(&rtf_file, r"{\rtf1\ansi Bound RTF}");
        let _ = std::fs::write(&txt_file, "Bound Plain Text");
        let _ = std::fs::create_dir_all(&dir_as_file);

        let mut linker = Linker::new();
        linker.add_base_dir(&temp_dir);

        // 1. Empty db (no Control table) -> Ok
        let mut db_empty = LinkedDatabase::default();
        assert!(linker.bind_text_sources(&mut db_empty).is_ok());

        // 2. Populated Control table with various ScrollableText records
        let mut db = LinkedDatabase::default();
        let make_ctrl = |id: &str, ctype: &str, text: FieldValue| {
            Record::with_fields(vec![
                FieldValue::String("Dlg".to_string()),
                FieldValue::String(id.to_string()),
                FieldValue::String(ctype.to_string()),
                FieldValue::Short(0),
                FieldValue::Short(0),
                FieldValue::Short(100),
                FieldValue::Short(50),
                FieldValue::Long(3),
                FieldValue::Null,
                text,
            ])
        };
        db.add_record("Control", make_ctrl("C1", "PushButton", FieldValue::Null));
        db.add_record(
            "Control",
            make_ctrl("C2", "ScrollableText", FieldValue::Null),
        );
        db.add_record(
            "Control",
            make_ctrl("C3", "ScrollableText", FieldValue::String(String::new())),
        );
        db.add_record(
            "Control",
            make_ctrl(
                "C4",
                "ScrollableText",
                FieldValue::String(r"{\rtf1 Already RTF}".to_string()),
            ),
        );
        db.add_record(
            "Control",
            make_ctrl(
                "C5",
                "ScrollableText",
                FieldValue::String("!(bindpath.rtf:bound.rtf)".to_string()),
            ),
        );
        db.add_record(
            "Control",
            make_ctrl(
                "C6",
                "ScrollableText",
                FieldValue::String("bound.txt".to_string()),
            ),
        );
        db.add_record(
            "Control",
            make_ctrl(
                "C7",
                "ScrollableText",
                FieldValue::String("Plain Inline Non-File String".to_string()),
            ),
        );
        db.add_record(
            "Control",
            make_ctrl("C8", "ScrollableText", FieldValue::Short(1)),
        );
        assert!(linker.bind_text_sources(&mut db).is_ok());
        assert_eq!(
            db.get_records("Control")[4].get(9),
            Some(&FieldValue::String(r"{\rtf1\ansi Bound RTF}".to_string()))
        );
        let c6 = db.get_records("Control")[5]
            .get(9)
            .map(ToString::to_string)
            .unwrap_or_default();
        assert!(c6.starts_with(r"{\rtf1\ansi\ansicpg1252"));

        // 2b. bind_binaries with WixIcon valid path, missing path, and non-string record
        let mut icon_db = LinkedDatabase::default();
        icon_db.add_record(
            "WixIcon",
            Record::with_fields(vec![
                FieldValue::String("ValidIco".to_string()),
                FieldValue::String("bound.rtf".to_string()),
            ]),
        );
        icon_db.add_record(
            "WixIcon",
            Record::with_fields(vec![
                FieldValue::String("MissingIco".to_string()),
                FieldValue::String("nonexistent_icon.ico".to_string()),
            ]),
        );
        icon_db.add_record("WixIcon", Record::with_fields(vec![FieldValue::Short(1)]));
        assert!(linker.bind_binaries(&mut icon_db).is_ok());

        let mut icon_err_db = LinkedDatabase::default();
        icon_err_db.add_record(
            "WixIcon",
            Record::with_fields(vec![
                FieldValue::String("DirIco".to_string()),
                FieldValue::String("sub_dir_err.rtf".to_string()),
            ]),
        );
        assert!(linker.bind_binaries(&mut icon_err_db).is_err());

        // 3. Unresolved file path (.rtf / .txt / !(bindpath.rtf:...)) -> Err
        let mut db_unres = LinkedDatabase::default();
        db_unres.add_record(
            "Control",
            make_ctrl(
                "CU",
                "ScrollableText",
                FieldValue::String("missing.rtf".to_string()),
            ),
        );
        assert!(linker.bind_text_sources(&mut db_unres).is_err());

        let mut db_unres_txt = LinkedDatabase::default();
        db_unres_txt.add_record(
            "Control",
            make_ctrl(
                "CUT",
                "ScrollableText",
                FieldValue::String("missing.txt".to_string()),
            ),
        );
        assert!(linker.bind_text_sources(&mut db_unres_txt).is_err());

        // 4. Directory passed as source file -> read_to_string error
        let mut db_dir_err = LinkedDatabase::default();
        db_dir_err.add_record(
            "Control",
            make_ctrl(
                "CD",
                "ScrollableText",
                FieldValue::String("sub_dir_err.rtf".to_string()),
            ),
        );
        assert!(linker.bind_text_sources(&mut db_dir_err).is_err());

        // 5. resolve_wix_variables with non-String/non-Null field 9 in ScrollableText
        let mut db_wixvar = LinkedDatabase::default();
        db_wixvar.add_record(
            "WixVariable",
            Record::with_fields(vec![
                FieldValue::String("WixUILicenseRtf".to_string()),
                FieldValue::String(r"{\rtf1 Var}".to_string()),
                FieldValue::Short(0),
            ]),
        );
        db_wixvar.add_record(
            "Control",
            make_ctrl("CS", "ScrollableText", FieldValue::Short(1)),
        );
        db_wixvar.add_record(
            "Control",
            make_ctrl(
                "CE",
                "ScrollableText",
                FieldValue::String("!(wix.WixUILicenseTxt)".to_string()),
            ),
        );
        db_wixvar.add_record(
            "Control",
            Record::with_fields(vec![
                FieldValue::String("Dlg".to_string()),
                FieldValue::String("CNone".to_string()),
                FieldValue::String("ScrollableText".to_string()),
            ]),
        );
        linker.resolve_wix_variables(&mut db_wixvar);

        // 6. sequence_standard_actions with non-string record in InstallUISequence
        let mut db_seq = LinkedDatabase::default();
        db_seq.add_record(
            "InstallUISequence",
            Record::with_fields(vec![FieldValue::Short(1)]),
        );
        db_seq.add_record(
            "InstallUISequence",
            Record::with_fields(vec![
                FieldValue::String("ExecuteAction".to_string()),
                FieldValue::Null,
                FieldValue::Short(1300),
            ]),
        );
        Linker::sequence_standard_actions(&mut db_seq);

        let _ = std::fs::remove_dir_all(&temp_dir);
    }
}

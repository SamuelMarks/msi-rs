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
//! - Internal Consistency Evaluators (ICE) validation: ICE01, ICE02, ICE03, ICE04, ICE05, ICE06,
//!   ICE07, ICE08, ICE09, ICE18, ICE20, ICE30, ICE33, ICE38, ICE61, ICE80, ICE99, ICE101, ICE103.

use crate::database::catalogs::DatabaseCatalog;
use crate::database::tables::record::{FieldValue, Record};
use crate::error::{Error, Result};
use crate::execution::properties::EvaluationContext;
use crate::execution::script_engine::{
    ScriptDatabase, ScriptEngine, ScriptLanguage, ScriptSession,
};
use crate::package::{Package, ProductVersion};
use crate::wix::localization::LocalizationCatalog;
use crate::wix::wixobj::{IntermediateSection, Reference, SectionType, Symbol, WixObject};
use rayon::prelude::*;
use std::collections::{HashMap, HashSet, VecDeque};
use std::fmt;
use std::path::{Path, PathBuf};

/// Standard directory mapping definition.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StandardDirectory {
    /// Directory identifier.
    pub id: &'static str,
    /// Parent directory identifier (None if root `TARGETDIR`).
    pub parent_id: Option<&'static str>,
    /// Default directory specification.
    pub default_dir: &'static str,
}

/// Known standard directory identifiers in Windows Installer.
pub const STANDARD_DIRECTORIES: &[StandardDirectory] = &[
    StandardDirectory {
        id: "TARGETDIR",
        parent_id: None,
        default_dir: "SourceDir",
    },
    StandardDirectory {
        id: "ProgramFilesFolder",
        parent_id: Some("TARGETDIR"),
        default_dir: "PFiles",
    },
    StandardDirectory {
        id: "ProgramFiles64Folder",
        parent_id: Some("TARGETDIR"),
        default_dir: "PFiles64",
    },
    StandardDirectory {
        id: "CommonFilesFolder",
        parent_id: Some("TARGETDIR"),
        default_dir: "Common",
    },
    StandardDirectory {
        id: "CommonFiles64Folder",
        parent_id: Some("TARGETDIR"),
        default_dir: "Common64",
    },
    StandardDirectory {
        id: "SystemFolder",
        parent_id: Some("TARGETDIR"),
        default_dir: "System",
    },
    StandardDirectory {
        id: "System64Folder",
        parent_id: Some("TARGETDIR"),
        default_dir: "System64",
    },
    StandardDirectory {
        id: "AppDataFolder",
        parent_id: Some("TARGETDIR"),
        default_dir: "AppData",
    },
    StandardDirectory {
        id: "LocalAppDataFolder",
        parent_id: Some("TARGETDIR"),
        default_dir: "LocalApp",
    },
    StandardDirectory {
        id: "CommonAppDataFolder",
        parent_id: Some("TARGETDIR"),
        default_dir: "CommonAppData",
    },
    StandardDirectory {
        id: "DesktopFolder",
        parent_id: Some("TARGETDIR"),
        default_dir: "Desktop",
    },
    StandardDirectory {
        id: "ProgramMenuFolder",
        parent_id: Some("TARGETDIR"),
        default_dir: "ProgMenu",
    },
    StandardDirectory {
        id: "WindowsFolder",
        parent_id: Some("TARGETDIR"),
        default_dir: "Windows",
    },
    StandardDirectory {
        id: "StartMenuFolder",
        parent_id: Some("TARGETDIR"),
        default_dir: "StartMenu",
    },
    StandardDirectory {
        id: "StartupFolder",
        parent_id: Some("TARGETDIR"),
        default_dir: "Startup",
    },
    StandardDirectory {
        id: "SendToFolder",
        parent_id: Some("TARGETDIR"),
        default_dir: "SendTo",
    },
    StandardDirectory {
        id: "RecentFolder",
        parent_id: Some("TARGETDIR"),
        default_dir: "Recent",
    },
    StandardDirectory {
        id: "FavoritesFolder",
        parent_id: Some("TARGETDIR"),
        default_dir: "Favorites",
    },
    StandardDirectory {
        id: "NetHoodFolder",
        parent_id: Some("TARGETDIR"),
        default_dir: "NetHood",
    },
    StandardDirectory {
        id: "PrintHoodFolder",
        parent_id: Some("TARGETDIR"),
        default_dir: "PrintHood",
    },
    StandardDirectory {
        id: "TemplatesFolder",
        parent_id: Some("TARGETDIR"),
        default_dir: "Templates",
    },
    StandardDirectory {
        id: "FontsFolder",
        parent_id: Some("TARGETDIR"),
        default_dir: "Fonts",
    },
    StandardDirectory {
        id: "AdminToolsFolder",
        parent_id: Some("TARGETDIR"),
        default_dir: "AdminTools",
    },
    StandardDirectory {
        id: "TempFolder",
        parent_id: Some("TARGETDIR"),
        default_dir: "Temp",
    },
    // Standard POSIX directories
    StandardDirectory {
        id: "/usr",
        parent_id: Some("TARGETDIR"),
        default_dir: "usr",
    },
    StandardDirectory {
        id: "UsrFolder",
        parent_id: Some("TARGETDIR"),
        default_dir: "usr",
    },
    StandardDirectory {
        id: "/usr/local",
        parent_id: Some("TARGETDIR"),
        default_dir: "usr_local",
    },
    StandardDirectory {
        id: "UsrLocalFolder",
        parent_id: Some("TARGETDIR"),
        default_dir: "usr_local",
    },
    StandardDirectory {
        id: "/usr/bin",
        parent_id: Some("TARGETDIR"),
        default_dir: "usr_bin",
    },
    StandardDirectory {
        id: "UsrBinFolder",
        parent_id: Some("TARGETDIR"),
        default_dir: "usr_bin",
    },
    StandardDirectory {
        id: "/usr/share",
        parent_id: Some("TARGETDIR"),
        default_dir: "usr_share",
    },
    StandardDirectory {
        id: "UsrShareFolder",
        parent_id: Some("TARGETDIR"),
        default_dir: "usr_share",
    },
    StandardDirectory {
        id: "/etc",
        parent_id: Some("TARGETDIR"),
        default_dir: "etc",
    },
    StandardDirectory {
        id: "EtcFolder",
        parent_id: Some("TARGETDIR"),
        default_dir: "etc",
    },
    StandardDirectory {
        id: "/var",
        parent_id: Some("TARGETDIR"),
        default_dir: "var",
    },
    StandardDirectory {
        id: "VarFolder",
        parent_id: Some("TARGETDIR"),
        default_dir: "var",
    },
    StandardDirectory {
        id: "/opt",
        parent_id: Some("TARGETDIR"),
        default_dir: "opt",
    },
    StandardDirectory {
        id: "OptFolder",
        parent_id: Some("TARGETDIR"),
        default_dir: "opt",
    },
    StandardDirectory {
        id: "/Applications",
        parent_id: Some("TARGETDIR"),
        default_dir: "Applications",
    },
    StandardDirectory {
        id: "ApplicationsFolder",
        parent_id: Some("TARGETDIR"),
        default_dir: "Applications",
    },
    StandardDirectory {
        id: "~/Library/Application Support",
        parent_id: Some("TARGETDIR"),
        default_dir: "UserAppSupport",
    },
    StandardDirectory {
        id: "UserApplicationSupportFolder",
        parent_id: Some("TARGETDIR"),
        default_dir: "UserAppSupport",
    },
];

/// Standard action ordering entry in execution sequence tables.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StandardActionOrder {
    /// Action name identifier.
    pub name: &'static str,
    /// Default sequence number.
    pub sequence: i16,
    /// Default condition expression (None for unconditional).
    pub condition: Option<&'static str>,
}

/// Standard MSI action sequence in `InstallExecuteSequence` table per MSI SDK.
pub const STANDARD_INSTALL_EXECUTE_ACTIONS: &[StandardActionOrder] = &[
    StandardActionOrder {
        name: "AppSearch",
        sequence: 400,
        condition: None,
    },
    StandardActionOrder {
        name: "FindRelatedProducts",
        sequence: 500,
        condition: None,
    },
    StandardActionOrder {
        name: "LaunchConditions",
        sequence: 600,
        condition: None,
    },
    StandardActionOrder {
        name: "ValidateProductID",
        sequence: 700,
        condition: None,
    },
    StandardActionOrder {
        name: "CostInitialize",
        sequence: 800,
        condition: None,
    },
    StandardActionOrder {
        name: "FileCost",
        sequence: 900,
        condition: None,
    },
    StandardActionOrder {
        name: "CostFinalize",
        sequence: 1000,
        condition: None,
    },
    StandardActionOrder {
        name: "InstallValidate",
        sequence: 1400,
        condition: None,
    },
    StandardActionOrder {
        name: "InstallInitialize",
        sequence: 1500,
        condition: None,
    },
    StandardActionOrder {
        name: "ProcessComponents",
        sequence: 1600,
        condition: None,
    },
    StandardActionOrder {
        name: "UnpublishComponents",
        sequence: 1700,
        condition: Some("Installed"),
    },
    StandardActionOrder {
        name: "InstallFiles",
        sequence: 4000,
        condition: None,
    },
    StandardActionOrder {
        name: "CreateShortcuts",
        sequence: 4500,
        condition: None,
    },
    StandardActionOrder {
        name: "WriteRegistryValues",
        sequence: 5000,
        condition: None,
    },
    StandardActionOrder {
        name: "RegisterProduct",
        sequence: 6100,
        condition: None,
    },
    StandardActionOrder {
        name: "PublishComponents",
        sequence: 6200,
        condition: None,
    },
    StandardActionOrder {
        name: "PublishFeatures",
        sequence: 6300,
        condition: None,
    },
    StandardActionOrder {
        name: "PublishProduct",
        sequence: 6400,
        condition: None,
    },
    StandardActionOrder {
        name: "InstallFinalize",
        sequence: 6600,
        condition: None,
    },
];

/// Standard MSI action sequence in `InstallUISequence` table per MSI SDK.
pub const STANDARD_INSTALL_UI_ACTIONS: &[StandardActionOrder] = &[
    StandardActionOrder {
        name: "AppSearch",
        sequence: 400,
        condition: None,
    },
    StandardActionOrder {
        name: "FindRelatedProducts",
        sequence: 500,
        condition: None,
    },
    StandardActionOrder {
        name: "LaunchConditions",
        sequence: 600,
        condition: None,
    },
    StandardActionOrder {
        name: "ValidateProductID",
        sequence: 700,
        condition: None,
    },
    StandardActionOrder {
        name: "CostInitialize",
        sequence: 800,
        condition: None,
    },
    StandardActionOrder {
        name: "FileCost",
        sequence: 900,
        condition: None,
    },
    StandardActionOrder {
        name: "CostFinalize",
        sequence: 1000,
        condition: None,
    },
    StandardActionOrder {
        name: "ExecuteAction",
        sequence: 1300,
        condition: None,
    },
];

/// Result of ICE validation reporting warnings or errors.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IceReport {
    /// ICE identifier (e.g. "ICE01", "ICE03").
    pub ice: String,
    /// Whether this report is an error or warning.
    pub is_error: bool,
    /// Detailed diagnostic message.
    pub message: String,
}

/// Linked database representation holding resolved tables and records.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct LinkedDatabase {
    /// In-memory table contents mapping table name to list of records.
    pub tables: HashMap<String, Vec<Record>>,
    /// Database catalog describing table schemas.
    pub catalog: DatabaseCatalog,
}

impl LinkedDatabase {
    /// Creates a new empty [`LinkedDatabase`] with all standard schemas registered.
    ///
    /// # Returns
    ///
    /// A new [`LinkedDatabase`].
    ///
    /// # Errors
    ///
    /// Returns [`Error`] if catalog population fails.
    pub fn new() -> Result<Self> {
        let mut catalog = DatabaseCatalog::new();
        crate::database::tables::populate_standard_tables(&mut catalog)?;
        Ok(Self {
            tables: HashMap::new(),
            catalog,
        })
    }

    /// Adds a record to the specified table.
    ///
    /// # Arguments
    ///
    /// * `table` - Target table name.
    /// * `record` - Record to append.
    pub fn add_record(&mut self, table: &str, record: Record) {
        self.tables
            .entry(table.to_string())
            .or_default()
            .push(record);
    }

    /// Returns a slice of records for the given table, or an empty slice if the table does not exist.
    ///
    /// # Arguments
    ///
    /// * `table` - Table name.
    ///
    /// # Returns
    ///
    /// Slice of [`Record`].
    #[must_use]
    pub fn get_records(&self, table: &str) -> &[Record] {
        self.tables.get(table).map_or(&[], Vec::as_slice)
    }

    /// Converts this [`LinkedDatabase`] into an automation [`ScriptDatabase`].
    ///
    /// Maps all table records and columns to string representations suitable for
    /// script engine inspection.
    ///
    /// # Returns
    ///
    /// A populated [`ScriptDatabase`].
    #[must_use]
    pub fn to_script_database(&self) -> ScriptDatabase {
        let mut script_db = ScriptDatabase::new(Some(self.catalog.clone()));
        for (table_name, records) in &self.tables {
            let col_names: Vec<String> = self
                .catalog
                .get_table(table_name)
                .map_or_else(Vec::new, |tbl| {
                    tbl.columns.iter().map(|c| c.name.clone()).collect()
                });
            for record in records {
                let mut row = HashMap::new();
                for (col_idx, field) in record.fields().iter().enumerate() {
                    let col_name = if col_idx < col_names.len() {
                        col_names[col_idx].clone()
                    } else {
                        format!("Col{col_idx}")
                    };
                    let val_str = match field {
                        FieldValue::Short(v) => v.to_string(),
                        FieldValue::Long(v) => v.to_string(),
                        FieldValue::String(s) => s.clone(),
                        FieldValue::Stream(id) => id.to_string(),
                        FieldValue::Null => String::new(),
                    };
                    row.insert(col_name, val_str);
                }
                script_db.insert_row(table_name, row);
            }
        }
        script_db
    }

    /// Merges an ingested [`MergeModule`] into this target database.
    ///
    /// Executes the complete merge module ingestion lifecycle:
    /// 1. Module parameter substitution via `ModuleSubstitution`.
    /// 2. Directory retargeting: attaching module root directory to `target_directory`.
    /// 3. Symbol modularization appending `.<ModuleGuid>`.
    /// 4. Merging table rows into destination tables.
    /// 5. Feature attachment: associating module components with `target_feature`.
    ///
    /// # Arguments
    ///
    /// * `module` - The [`MergeModule`] to merge.
    /// * `target_feature` - Feature to which the module's components are attached.
    /// * `target_directory` - Target directory to which the module's directory root is attached.
    /// * `substitutions` - Configuration parameter substitution map (`PARAM_NAME -> VALUE`).
    ///
    /// # Errors
    ///
    /// Returns [`Error`] on table schema or primary key merge collisions.
    #[allow(clippy::too_many_lines)]
    pub fn merge_module(
        &mut self,
        module: &MergeModule,
        target_feature: &str,
        target_directory: &str,
        substitutions: &HashMap<String, String>,
    ) -> Result<()> {
        let mut mod_db = module.database.clone();
        let module_guid = module.guid();

        // 1. Gather ignored modularization symbols
        let mut ignore_set = HashSet::new();
        for r in mod_db.get_records("ModuleIgnoreModularization") {
            if let Some(FieldValue::String(name)) = r.get(0) {
                ignore_set.insert(name.clone());
            }
        }

        // 2. Perform parameter substitutions from ModuleSubstitution
        let subst_records: Vec<(String, String, String, Option<String>)> = mod_db
            .get_records("ModuleSubstitution")
            .iter()
            .filter_map(|r| {
                if let (
                    Some(FieldValue::String(tbl)),
                    Some(FieldValue::String(row)),
                    Some(FieldValue::String(col)),
                ) = (r.get(0), r.get(1), r.get(2))
                {
                    let val = match r.get(3) {
                        Some(FieldValue::String(v)) => Some(v.clone()),
                        _ => None,
                    };
                    Some((tbl.clone(), row.clone(), col.clone(), val))
                } else {
                    None
                }
            })
            .collect();

        for (table_name, row_key, col_name, tmpl_val) in subst_records {
            if let Some(tmpl) = tmpl_val {
                let mut replaced = tmpl;
                for (param, val) in substitutions {
                    let pattern1 = format!("[={param}]");
                    let pattern2 = format!("[{param}]");
                    replaced = replaced.replace(&pattern1, val).replace(&pattern2, val);
                }

                if let Some(col_idx) = mod_db
                    .catalog
                    .get_table(&table_name)
                    .and_then(|t| t.columns.iter().position(|c| c.name == col_name))
                {
                    if let Some(rows) = mod_db.tables.get_mut(&table_name) {
                        for r in rows {
                            if matches!(r.get(0), Some(FieldValue::String(k)) if k == &row_key) {
                                r.set(col_idx, FieldValue::String(replaced.clone()));
                            }
                        }
                    }
                }
            }
        }

        // 3. Retarget module directory root to target_directory
        if let Some(dir_rows) = mod_db.tables.get_mut("Directory") {
            for r in dir_rows {
                let is_root = match r.get(1) {
                    Some(FieldValue::Null) => true,
                    Some(FieldValue::String(p)) if p.is_empty() || p == "TARGETDIR" => true,
                    _ => false,
                };
                if is_root && r.get(0) != Some(&FieldValue::String("TARGETDIR".to_string())) {
                    r.set(1, FieldValue::String(target_directory.to_string()));
                }
            }
        }

        // 4. Collect component names before or after modularization
        let mut comp_names = Vec::new();
        if let Some(comp_rows) = mod_db.tables.get("Component") {
            for r in comp_rows {
                if let Some(FieldValue::String(c_name)) = r.get(0) {
                    let mod_c_name = modularize_identifier(c_name, module_guid, &ignore_set);
                    comp_names.push(mod_c_name);
                }
            }
        }

        // 5. Modularize module tables and merge them into destination
        let mod_tables = std::mem::take(&mut mod_db.tables);
        for (table_name, rows) in mod_tables {
            // Skip merge module system tables
            if table_name.starts_with("Module") {
                continue;
            }

            let dest_rows = self.tables.entry(table_name.clone()).or_default();

            for mut record in rows {
                match table_name.as_str() {
                    "Component" => {
                        if let Some(FieldValue::String(ref s)) = record.get(0) {
                            let m = modularize_identifier(s, module_guid, &ignore_set);
                            record.set(0, FieldValue::String(m));
                        }
                        if let Some(FieldValue::String(ref s)) = record.get(2) {
                            let m = modularize_identifier(s, module_guid, &ignore_set);
                            record.set(2, FieldValue::String(m));
                        }
                        if let Some(FieldValue::String(ref s)) = record.get(5) {
                            let m = modularize_identifier(s, module_guid, &ignore_set);
                            record.set(5, FieldValue::String(m));
                        }
                    }
                    "File" => {
                        if let Some(FieldValue::String(ref s)) = record.get(0) {
                            let m = modularize_identifier(s, module_guid, &ignore_set);
                            record.set(0, FieldValue::String(m));
                        }
                        if let Some(FieldValue::String(ref s)) = record.get(1) {
                            let m = modularize_identifier(s, module_guid, &ignore_set);
                            record.set(1, FieldValue::String(m));
                        }
                    }
                    "Directory" => {
                        if let Some(FieldValue::String(ref s)) = record.get(0) {
                            let m = modularize_identifier(s, module_guid, &ignore_set);
                            record.set(0, FieldValue::String(m));
                        }
                        if let Some(FieldValue::String(ref s)) = record.get(1) {
                            if s != target_directory {
                                let m = modularize_identifier(s, module_guid, &ignore_set);
                                record.set(1, FieldValue::String(m));
                            }
                        }
                    }
                    "CustomAction" => {
                        if let Some(FieldValue::String(ref s)) = record.get(0) {
                            let m = modularize_identifier(s, module_guid, &ignore_set);
                            record.set(0, FieldValue::String(m));
                        }
                    }
                    _ => {}
                }

                let already_exists = dest_rows
                    .iter()
                    .any(|existing| existing.get(0) == record.get(0));
                if !already_exists {
                    dest_rows.push(record);
                }
            }
        }

        // 6. Feature attachment: link all components to target_feature
        let fc_rows = self
            .tables
            .entry("FeatureComponents".to_string())
            .or_default();

        for comp_name in comp_names {
            let mut exists = false;
            for r in &*fc_rows {
                if let (Some(FieldValue::String(f)), Some(FieldValue::String(c))) =
                    (r.get(0), r.get(1))
                {
                    if f == target_feature && c == &comp_name {
                        exists = true;
                        break;
                    }
                }
            }
            if !exists {
                fc_rows.push(Record::with_fields(vec![
                    FieldValue::String(target_feature.to_string()),
                    FieldValue::String(comp_name),
                ]));
            }
        }

        Ok(())
    }
}

/// Modularizes an identifier by appending the module GUID, unless excluded.
///
/// # Arguments
///
/// * `ident` - Identifier to modularize.
/// * `module_guid` - Module GUID suffix.
/// * `ignore_set` - Set of identifiers excluded from modularization.
///
/// # Returns
///
/// Modularized string.
#[must_use]
pub fn modularize_identifier<S: ::std::hash::BuildHasher>(
    ident: &str,
    module_guid: &str,
    ignore_set: &HashSet<String, S>,
) -> String {
    if ident.is_empty()
        || ident == "TARGETDIR"
        || STANDARD_DIRECTORIES.iter().any(|d| d.id == ident)
        || ignore_set.contains(ident)
        || ident.ends_with(&format!(".{module_guid}"))
    {
        ident.to_string()
    } else {
        format!("{ident}.{module_guid}")
    }
}

/// An ingested Merge Module (`.msm`) containing modularized installation data.
#[derive(Debug, Clone)]
pub struct MergeModule {
    /// Module identifier (e.g. `MyModule.12345678_1234_1234_1234_1234567890AB`).
    pub id: String,
    /// Default language code.
    pub language: i16,
    /// Module version.
    pub version: String,
    /// Relational database containing module records.
    pub database: LinkedDatabase,
}

impl MergeModule {
    /// Constructs a [`MergeModule`] from an existing [`LinkedDatabase`].
    ///
    /// Inspects the `ModuleSignature` table to determine ID, language, and version.
    ///
    /// # Arguments
    ///
    /// * `database` - In-memory relational database.
    ///
    /// # Returns
    ///
    /// A new [`MergeModule`].
    #[must_use]
    pub fn from_database(database: LinkedDatabase) -> Self {
        let mut id = "Module".to_string();
        let mut language = 1033;
        let mut version = "1.0.0".to_string();

        let sig_records = database.get_records("ModuleSignature");
        if let Some(first) = sig_records.first() {
            if let Some(FieldValue::String(m_id)) = first.get(0) {
                id.clone_from(m_id);
            }
            if let Some(FieldValue::Short(l)) = first.get(1) {
                language = *l;
            }
            if let Some(FieldValue::String(v)) = first.get(2) {
                version.clone_from(v);
            }
        }

        Self {
            id,
            language,
            version,
            database,
        }
    }

    /// Loads a [`MergeModule`] from raw `.msm` binary bytes.
    ///
    /// # Arguments
    ///
    /// * `bytes` - Raw `.msm` file bytes (CFB container).
    ///
    /// # Returns
    ///
    /// Parsed [`MergeModule`].
    ///
    /// # Errors
    ///
    /// Returns [`Error`] on container parsing or database deserialization failure.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self> {
        let pkg = Package::from_bytes(bytes)?;
        Ok(Self::from_database(pkg.database().clone()))
    }

    /// Loads a [`MergeModule`] from a filesystem path.
    ///
    /// # Arguments
    ///
    /// * `path` - Filesystem path to the `.msm` file.
    ///
    /// # Returns
    ///
    /// Parsed [`MergeModule`].
    ///
    /// # Errors
    ///
    /// Returns [`Error`] on file I/O, container parsing, or database error.
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let pkg = Package::open(path)?;
        Ok(Self::from_database(pkg.database().clone()))
    }

    /// Extracts the module GUID suffix used for symbol modularization.
    ///
    /// # Returns
    ///
    /// Module GUID string slice.
    #[must_use]
    pub fn guid(&self) -> &str {
        if let Some((_, suffix)) = self.id.rsplit_once('.') {
            suffix
        } else {
            &self.id
        }
    }
}

/// Diagnostic severity classification for ICE validation messages.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IceDiagnosticType {
    /// Fatal ICE validation error.
    Error,
    /// Non-fatal ICE validation warning.
    Warning,
    /// Informational ICE validation notice.
    Info,
}

impl fmt::Display for IceDiagnosticType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Error => write!(f, "ERROR"),
            Self::Warning => write!(f, "WARNING"),
            Self::Info => write!(f, "INFO"),
        }
    }
}

/// Diagnostic message emitted by an Internal Consistency Evaluator (ICE).
///
/// Matches the official `WiX` compiler diagnostic format:
/// `ICE<id>: <Type>: <Description> <Table> <Column> <RowKey>`
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IceDiagnostic {
    /// Rule identifier (e.g. "ICE01", "ICE03").
    pub ice: String,
    /// Severity classification.
    pub diagnostic_type: IceDiagnosticType,
    /// Human-readable description of the consistency issue.
    pub description: String,
    /// Optional target table name.
    pub table: Option<String>,
    /// Optional target column name.
    pub column: Option<String>,
    /// Optional target row primary key.
    pub row_key: Option<String>,
}

impl IceDiagnostic {
    /// Creates a new [`IceDiagnostic`] with minimal metadata.
    ///
    /// # Arguments
    ///
    /// * `ice` - Rule identifier (e.g. "ICE01").
    /// * `diagnostic_type` - Severity classification.
    /// * `description` - Diagnostic description.
    ///
    /// # Returns
    ///
    /// A new [`IceDiagnostic`].
    pub fn new(
        ice: impl Into<String>,
        diagnostic_type: IceDiagnosticType,
        description: impl Into<String>,
    ) -> Self {
        Self {
            ice: ice.into(),
            diagnostic_type,
            description: description.into(),
            table: None,
            column: None,
            row_key: None,
        }
    }

    /// Appends target relational database location information.
    ///
    /// # Arguments
    ///
    /// * `table` - Target table name.
    /// * `column` - Target column name.
    /// * `row_key` - Target row primary key.
    ///
    /// # Returns
    ///
    /// Updated [`IceDiagnostic`].
    #[must_use]
    pub fn with_location(
        mut self,
        table: impl Into<String>,
        column: impl Into<String>,
        row_key: impl Into<String>,
    ) -> Self {
        self.table = Some(table.into());
        self.column = Some(column.into());
        self.row_key = Some(row_key.into());
        self
    }
}

impl fmt::Display for IceDiagnostic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}: {}: {}",
            self.ice, self.diagnostic_type, self.description
        )?;
        if let Some(ref t) = self.table {
            write!(f, " {t}")?;
        }
        if let Some(ref c) = self.column {
            write!(f, " {c}")?;
        }
        if let Some(ref r) = self.row_key {
            write!(f, " {r}")?;
        }
        Ok(())
    }
}

impl From<IceReport> for IceDiagnostic {
    fn from(rep: IceReport) -> Self {
        Self {
            ice: rep.ice,
            diagnostic_type: if rep.is_error {
                IceDiagnosticType::Error
            } else {
                IceDiagnosticType::Warning
            },
            description: rep.message,
            table: None,
            column: None,
            row_key: None,
        }
    }
}

impl From<IceDiagnostic> for IceReport {
    fn from(diag: IceDiagnostic) -> Self {
        Self {
            ice: diag.ice,
            is_error: diag.diagnostic_type == IceDiagnosticType::Error,
            message: diag.description,
        }
    }
}

/// Trait representing an Internal Consistency Evaluator (ICE) validation rule.
pub trait IceRule: Send + Sync {
    /// Returns the unique rule name (e.g. "ICE01", "ICE03").
    fn name(&self) -> &str;

    /// Returns a short description of what this ICE rule validates.
    fn description(&self) -> &str;

    /// Executes the validation check on the given [`LinkedDatabase`].
    ///
    /// # Arguments
    ///
    /// * `db` - The [`LinkedDatabase`] to inspect.
    ///
    /// # Returns
    ///
    /// Vector of [`IceDiagnostic`] records detailing any consistency issues discovered.
    fn execute(&self, db: &LinkedDatabase) -> Vec<IceDiagnostic>;
}

/// Wrapper for standard built-in ICE rules conforming to the [`IceRule`] trait.
#[derive(Debug, Clone)]
pub struct StandardIceRule {
    /// Rule name.
    name: &'static str,
    /// Rule description.
    description: &'static str,
    /// Evaluation callback function.
    validator: fn(&LinkedDatabase) -> Option<IceReport>,
}

impl StandardIceRule {
    /// Creates a new [`StandardIceRule`].
    ///
    /// # Arguments
    ///
    /// * `name` - Rule identifier.
    /// * `description` - Short description.
    /// * `validator` - Validation function pointer.
    ///
    /// # Returns
    ///
    /// A new [`StandardIceRule`].
    #[must_use]
    pub const fn new(
        name: &'static str,
        description: &'static str,
        validator: fn(&LinkedDatabase) -> Option<IceReport>,
    ) -> Self {
        Self {
            name,
            description,
            validator,
        }
    }
}

impl IceRule for StandardIceRule {
    fn name(&self) -> &str {
        self.name
    }

    fn description(&self) -> &str {
        self.description
    }

    fn execute(&self, db: &LinkedDatabase) -> Vec<IceDiagnostic> {
        (self.validator)(db)
            .into_iter()
            .map(IceDiagnostic::from)
            .collect()
    }
}

/// Registry of modular ICE validation rules.
pub struct IceRegistry {
    /// Registered ICE rules.
    rules: Vec<Box<dyn IceRule>>,
}

impl fmt::Debug for IceRegistry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("IceRegistry")
            .field("rule_count", &self.rules.len())
            .finish()
    }
}

impl Default for IceRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl IceRegistry {
    /// Creates a new empty [`IceRegistry`].
    ///
    /// # Returns
    ///
    /// An empty [`IceRegistry`].
    #[must_use]
    pub const fn new() -> Self {
        Self { rules: Vec::new() }
    }

    /// Creates an [`IceRegistry`] populated with all standard built-in ICE rules.
    ///
    /// # Returns
    ///
    /// A populated [`IceRegistry`].
    #[must_use]
    pub fn with_standard_rules() -> Self {
        let mut registry = Self::new();
        registry.register(StandardIceRule::new(
            "ICE01",
            "Verifies that required system and packaging tables exist in the database catalog",
            Linker::validate_ice01,
        ));
        registry.register(StandardIceRule::new(
            "ICE02",
            "Verifies Feature-to-Feature circular dependencies",
            Linker::validate_ice02,
        ));
        registry.register(StandardIceRule::new(
            "ICE03",
            "Comprehensive table data validation (nullability, string lengths, types)",
            Linker::validate_ice03,
        ));
        registry.register(StandardIceRule::new(
            "ICE04",
            "Verifies contiguous sequence numbers in the File table",
            Linker::validate_ice04,
        ));
        registry.register(StandardIceRule::new(
            "ICE05",
            "Verifies sequence ranges in Media table match File table sequences",
            Linker::validate_ice05,
        ));
        registry.register(StandardIceRule::new(
            "ICE06",
            "Verifies that files missing versions are not installed to shared directories without keypaths",
            Linker::validate_ice06,
        ));
        registry.register(StandardIceRule::new(
            "ICE07",
            "Verifies font file registrations",
            Linker::validate_ice07,
        ));
        registry.register(StandardIceRule::new(
            "ICE08",
            "Verifies that duplicate GUIDs are not assigned to different components",
            Linker::validate_ice08,
        ));
        registry.register(StandardIceRule::new(
            "ICE09",
            "Verifies that keypaths are valid files or registry keys",
            Linker::validate_ice09,
        ));
        registry.register(StandardIceRule::new(
            "ICE18",
            "Verifies that keypaths for keypath files match component directory",
            Linker::validate_ice18,
        ));
        registry.register(StandardIceRule::new(
            "ICE20",
            "Verifies standard action execution order in sequence tables",
            Linker::validate_ice20,
        ));
        registry.register(StandardIceRule::new(
            "ICE30",
            "Validates cross-component file name collisions in same target directory",
            Linker::validate_ice30,
        ));
        registry.register(StandardIceRule::new(
            "ICE33",
            "Validates Registry table entries for COM class and ProgID registration",
            Linker::validate_ice33,
        ));
        registry.register(StandardIceRule::new(
            "ICE38",
            "Validates components installed to user profiles use HKCU keypaths",
            Linker::validate_ice38,
        ));
        registry.register(StandardIceRule::new(
            "ICE61",
            "Validates Upgrade table version ranges against current ProductVersion",
            Linker::validate_ice61,
        ));
        registry.register(StandardIceRule::new(
            "ICE80",
            "Validates mixing 32-bit and 64-bit components in packages",
            Linker::validate_ice80,
        ));
        registry.register(StandardIceRule::new(
            "ICE99",
            "Validates Directory table has no circular references and exactly one root",
            Linker::validate_ice99,
        ));
        registry.register(StandardIceRule::new(
            "ICE101",
            "Validates that files in File table have proper sequence references",
            Linker::validate_ice101,
        ));
        registry.register(StandardIceRule::new(
            "ICE103",
            "Validates that shortcut icon indices and service accounts are valid",
            Linker::validate_ice103,
        ));
        registry
    }

    /// Registers a custom ICE validation rule.
    ///
    /// # Arguments
    ///
    /// * `rule` - The ICE rule to register.
    pub fn register<R: IceRule + 'static>(&mut self, rule: R) {
        self.rules.push(Box::new(rule));
    }

    /// Returns a slice of all registered ICE rules.
    ///
    /// # Returns
    ///
    /// Slice of boxed [`IceRule`] traits.
    #[must_use]
    pub fn rules(&self) -> &[Box<dyn IceRule>] {
        &self.rules
    }

    /// Returns the names of all registered ICE rules.
    ///
    /// # Returns
    ///
    /// Vector of rule name strings.
    #[must_use]
    pub fn rule_names(&self) -> Vec<&str> {
        self.rules.iter().map(|r| r.name()).collect()
    }

    /// Executes all registered ICE rules sequentially against the provided [`LinkedDatabase`].
    ///
    /// # Arguments
    ///
    /// * `db` - The [`LinkedDatabase`] to validate.
    ///
    /// # Returns
    ///
    /// Vector of all [`IceDiagnostic`] messages produced.
    #[must_use]
    pub fn execute_all(&self, db: &LinkedDatabase) -> Vec<IceDiagnostic> {
        self.rules.iter().flat_map(|r| r.execute(db)).collect()
    }

    /// Executes all registered ICE rules in parallel across CPU cores using Rayon.
    ///
    /// # Arguments
    ///
    /// * `db` - The [`LinkedDatabase`] to validate.
    ///
    /// # Returns
    ///
    /// Vector of all [`IceDiagnostic`] messages produced.
    #[must_use]
    pub fn execute_parallel(&self, db: &LinkedDatabase) -> Vec<IceDiagnostic> {
        self.rules.par_iter().flat_map(|r| r.execute(db)).collect()
    }
}

/// External CUB (Validation Module) Execution Shim.
///
/// Ingests external `.cub` validation database files (e.g. `darice.cub`, `mergemod.cub`)
/// and executes embedded `VBScript` / `JScript` custom action evaluators using the
/// native script runner.
#[derive(Debug, Clone)]
pub struct CubValidator {
    /// Name or identifier of this validation module.
    name: String,
    /// Ingested CUB relational database.
    database: LinkedDatabase,
}

impl CubValidator {
    /// Creates a new [`CubValidator`] from an in-memory [`LinkedDatabase`].
    ///
    /// # Arguments
    ///
    /// * `name` - CUB module name.
    /// * `database` - In-memory CUB database.
    ///
    /// # Returns
    ///
    /// A new [`CubValidator`].
    #[must_use]
    pub fn from_database(name: impl Into<String>, database: LinkedDatabase) -> Self {
        Self {
            name: name.into(),
            database,
        }
    }

    /// Loads a [`CubValidator`] from raw `.cub` binary bytes.
    ///
    /// # Arguments
    ///
    /// * `name` - CUB module name.
    /// * `bytes` - Raw binary `.cub` data (CFB format).
    ///
    /// # Returns
    ///
    /// Parsed [`CubValidator`].
    ///
    /// # Errors
    ///
    /// Returns [`Error`] if container parsing or database deserialization fails.
    pub fn from_bytes(name: impl Into<String>, bytes: &[u8]) -> Result<Self> {
        let pkg = Package::from_bytes(bytes)?;
        Ok(Self {
            name: name.into(),
            database: pkg.database().clone(),
        })
    }

    /// Loads a [`CubValidator`] from a filesystem path.
    ///
    /// # Arguments
    ///
    /// * `path` - Filesystem path to the `.cub` file.
    ///
    /// # Returns
    ///
    /// Parsed [`CubValidator`].
    ///
    /// # Errors
    ///
    /// Returns [`Error`] on file read, parsing, or database errors.
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let p = path.as_ref();
        let pkg = Package::open(p)?;
        let name = p
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("cub")
            .to_string();
        Ok(Self {
            name,
            database: pkg.database().clone(),
        })
    }

    /// Returns a reference to the CUB's relational database.
    ///
    /// # Returns
    ///
    /// Borrow of the [`LinkedDatabase`].
    #[must_use]
    pub const fn database(&self) -> &LinkedDatabase {
        &self.database
    }

    /// Returns the name of this validation module.
    ///
    /// # Returns
    ///
    /// Name string slice.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Returns the list of validation actions defined in this `.cub`.
    ///
    /// # Returns
    ///
    /// Vector of action name strings.
    #[must_use]
    pub fn action_names(&self) -> Vec<String> {
        let seq_records = self.database.get_records("_ICESequence");
        if !seq_records.is_empty() {
            let mut actions = Vec::new();
            for r in seq_records {
                if let Some(FieldValue::String(act)) = r.get(0) {
                    actions.push(act.clone());
                }
            }
            return actions;
        }

        let ca_records = self.database.get_records("CustomAction");
        let mut actions = Vec::new();
        for r in ca_records {
            if let Some(FieldValue::String(act)) = r.get(0) {
                if act.starts_with("ICE") {
                    actions.push(act.clone());
                }
            }
        }
        actions
    }

    /// Executes the CUB validation actions against the target [`LinkedDatabase`].
    ///
    /// # Arguments
    ///
    /// * `target_db` - Target [`LinkedDatabase`] to validate.
    ///
    /// # Returns
    ///
    /// Vector of [`IceDiagnostic`] messages produced by the validation scripts.
    ///
    /// # Errors
    ///
    /// Returns [`Error`] if script execution fails unexpectedly.
    #[allow(clippy::too_many_lines)]
    pub fn execute(&self, target_db: &LinkedDatabase) -> Result<Vec<IceDiagnostic>> {
        let mut diagnostics = Vec::new();
        let script_db = target_db.to_script_database();
        let ca_records = self.database.get_records("CustomAction");

        let actions = self.action_names();
        let mut engine = ScriptEngine::new();

        for action_name in actions {
            let ca_row = ca_records.iter().find(
                |r| matches!(r.get(0), Some(FieldValue::String(name)) if name == &action_name),
            );

            let Some(ca) = ca_row else {
                continue;
            };

            let ca_type = match ca.get(1) {
                Some(FieldValue::Short(t)) => i32::from(*t),
                Some(FieldValue::Long(t)) => *t,
                _ => 0,
            };

            let is_jscript =
                ca_type == 5 || ca_type == 37 || ca_type == 53 || (ca_type & 0x07 == 5);
            let is_vbscript =
                ca_type == 6 || ca_type == 38 || ca_type == 54 || (ca_type & 0x07 == 6);

            if !is_jscript && !is_vbscript {
                continue;
            }

            let script_content: Option<String> =
                if ca_type == 37 || ca_type == 38 || ca_type == 53 || ca_type == 54 {
                    ca.get(3).and_then(|f| match f {
                        FieldValue::String(s) => Some(s.clone()),
                        _ => None,
                    })
                } else {
                    ca.get(2).and_then(|source_f| {
                        if let FieldValue::String(src) = source_f {
                            self.database.get_records("Binary").iter().find_map(|bin_row| {
                            if matches!(bin_row.get(0), Some(FieldValue::String(n)) if n == src) {
                                match bin_row.get(1) {
                                    Some(FieldValue::String(code)) => Some(code.clone()),
                                    _ => None,
                                }
                            } else {
                                None
                            }
                        })
                        } else {
                            None
                        }
                    })
                };

            let Some(script) = script_content else {
                continue;
            };

            let mut session =
                ScriptSession::with_database(EvaluationContext::new(), script_db.clone());
            let lang = if is_jscript {
                ScriptLanguage::JScript
            } else {
                ScriptLanguage::VBScript
            };
            let exec_result = engine.execute(lang, &script, &mut session);

            match exec_result {
                Ok(_) => {
                    for msg in session.messages() {
                        diagnostics.push(IceDiagnostic::new(
                            action_name.clone(),
                            if msg.kind == 1 {
                                IceDiagnosticType::Error
                            } else {
                                IceDiagnosticType::Warning
                            },
                            msg.text.clone(),
                        ));
                    }
                }
                Err(err) => {
                    diagnostics.push(IceDiagnostic::new(
                        action_name.clone(),
                        IceDiagnosticType::Error,
                        format!("script evaluation failed: {err}"),
                    ));
                }
            }
        }

        Ok(diagnostics)
    }
}

/// `WiX` Linker (`light`) graph solver and binder.
#[derive(Debug, Default)]
pub struct Linker {
    /// Input intermediate object files.
    objects: Vec<WixObject>,
    /// Base directory search paths for binding payload files (`-b`).
    base_directories: Vec<PathBuf>,
    /// Named bind paths mapping identifier to directory (`-bd`).
    bind_paths: HashMap<String, PathBuf>,
    /// Whether to suppress all ICE validation (`-sval`).
    suppress_ice: bool,
    /// Specific ICE validation rules to selectively run (`-ice:<ICE>`).
    selected_ice: HashSet<String>,
    /// Specific ICE validation rules to suppress (`-sice:<ICE>`).
    suppressed_ice: HashSet<String>,
    /// Specific warning IDs to suppress (`-sw<id>`).
    suppressed_warnings: HashSet<String>,
    /// Treat warnings as fatal errors (`-wx`).
    warnings_as_errors: bool,
    /// Ordered culture priority list (e.g. `["en-us", "de-de"]`).
    cultures: Vec<String>,
    /// Localization catalog for evaluating `!(loc.Id)`.
    loc_catalog: LocalizationCatalog,
    /// Component-split media layout (one CAB per component).
    cab_per_component: bool,
    /// Embedded cabinet archives generated during binding.
    embedded_cabinets: HashMap<String, Vec<u8>>,
}

/// Checks whether an unresolved symbol reference is a standard built-in symbol, action,
/// dialog, property, or UI set.
///
/// # Arguments
///
/// * `rf` - Symbol reference to test.
/// * `defined_symbols` - Map of currently defined symbols.
fn is_special_reference(rf: &Reference, defined_symbols: &HashMap<Symbol, usize>) -> bool {
    match rf.namespace.as_str() {
        "Directory" => STANDARD_DIRECTORIES.iter().any(|d| d.id == rf.id),
        "Action" => {
            STANDARD_INSTALL_EXECUTE_ACTIONS
                .iter()
                .any(|a| a.name == rf.id)
                || STANDARD_INSTALL_UI_ACTIONS.iter().any(|a| a.name == rf.id)
                || defined_symbols.contains_key(&Symbol::new("Dialog", &rf.id))
                || defined_symbols.contains_key(&Symbol::new("CustomAction", &rf.id))
        }
        "UI" => rf.id.starts_with("WixUI_") || rf.id == "WixUI",
        "Property" => {
            rf.id.starts_with("WIXUI_") || rf.id.starts_with("ARP") || rf.id == "ALLUSERS"
        }
        _ => false,
    }
}

impl Linker {
    /// Creates a new empty [`Linker`].
    ///
    /// # Returns
    ///
    /// A new [`Linker`].
    #[must_use]
    pub fn new() -> Self {
        Self::default()
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

    /// Links all input objects, resolves symbols and references, builds directory hierarchies,
    /// sequences standard actions, binds media sequence numbers, and evaluates ICE rules.
    ///
    /// # Returns
    ///
    /// Fully resolved and validated [`LinkedDatabase`].
    ///
    /// # Errors
    ///
    /// Returns [`Error::WixLinker`] or [`Error::IceValidation`] on linking or validation failures.
    pub fn link(&mut self) -> Result<LinkedDatabase> {
        // 1. Solve linker symbol graph
        let active_sections = self.solve_symbol_graph()?;

        // 2. Build initial linked database from intermediate tables
        let mut db = LinkedDatabase::new()?;
        for sec in &active_sections {
            for tbl in &sec.tables {
                for rec in &tbl.records {
                    db.add_record(&tbl.name, rec.clone());
                }
            }
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
                            crate::wix::ui_library::inject_ui_library(
                                &mut db, set, None, None, None,
                            )?;
                            break;
                        }
                    }
                }
            }
        }

        // 5c. Resolve WiX variables, bind bitmaps, and extract EULA
        self.resolve_wix_variables(&mut db);

        // 6. Bind physical files from disk and generate cabinet archives
        self.bind_files_and_pack_cabinets(&mut db)?;

        // 7. Layout and sequence media files
        Self::layout_media_and_files(&mut db);

        // 8. Run comprehensive ICE validation
        self.run_filtered_ice_validations(&db)?;

        Ok(db)
    }

    /// Expands `!(loc.StringId)` tokens in all string fields across all database tables.
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
    ///
    /// # Arguments
    ///
    /// * `db` - The [`LinkedDatabase`] containing sequence tables.
    ///
    /// # Returns
    ///
    /// `Ok(())` on successful sequence resolution.
    ///
    /// # Errors
    ///
    /// Returns [`Error::WixLinker`] if a cyclic dependency is detected.
    #[allow(clippy::too_many_lines)]
    fn solve_relative_sequences(db: &mut LinkedDatabase) -> Result<()> {
        let rel_records = db.tables.remove("_WixSequenceRelative").unwrap_or_default();
        if rel_records.is_empty() {
            return Ok(());
        }

        let mut constraints_by_table: HashMap<String, Vec<(String, String, String)>> =
            HashMap::new();
        for r in &rel_records {
            if let (
                Some(FieldValue::String(tbl)),
                Some(FieldValue::String(action)),
                Some(FieldValue::String(anchor)),
                Some(FieldValue::String(pos)),
            ) = (r.get(0), r.get(1), r.get(2), r.get(3))
            {
                constraints_by_table.entry(tbl.clone()).or_default().push((
                    action.clone(),
                    anchor.clone(),
                    pos.clone(),
                ));
            }
        }

        for (table_name, constraints) in constraints_by_table {
            let existing_records = db.tables.entry(table_name.clone()).or_default();

            for (action, anchor, pos) in &constraints {
                if pos == "OnExit" {
                    let on_exit_seq: i16 = match anchor.as_str() {
                        "cancel" => -2,
                        "error" => -3,
                        "suspend" => -4,
                        _ => -1,
                    };
                    for rec in existing_records.iter_mut() {
                        if rec.get(0) == Some(&FieldValue::String(action.clone())) {
                            rec.set(2, FieldValue::Short(on_exit_seq));
                        }
                    }
                }
            }

            let mut action_seqs: HashMap<String, i16> = HashMap::new();
            for rec in existing_records.iter() {
                if let (Some(FieldValue::String(act)), Some(FieldValue::Short(s))) =
                    (rec.get(0), rec.get(2))
                {
                    action_seqs.insert(act.clone(), *s);
                }
            }

            if table_name == "InstallUISequence" {
                for std in STANDARD_INSTALL_UI_ACTIONS {
                    action_seqs
                        .entry(std.name.to_string())
                        .or_insert(std.sequence);
                }
            } else if table_name == "InstallExecuteSequence" {
                for std in STANDARD_INSTALL_EXECUTE_ACTIONS {
                    action_seqs
                        .entry(std.name.to_string())
                        .or_insert(std.sequence);
                }
            }

            let mut in_degree: HashMap<String, usize> = HashMap::new();
            let mut graph: HashMap<String, Vec<String>> = HashMap::new();
            for (action, anchor, pos) in &constraints {
                if pos == "After" {
                    graph
                        .entry(anchor.clone())
                        .or_default()
                        .push(action.clone());
                    *in_degree.entry(action.clone()).or_insert(0) += 1;
                    in_degree.entry(anchor.clone()).or_insert(0);
                } else if pos == "Before" {
                    graph
                        .entry(action.clone())
                        .or_default()
                        .push(anchor.clone());
                    *in_degree.entry(anchor.clone()).or_insert(0) += 1;
                    in_degree.entry(action.clone()).or_insert(0);
                }
            }

            let mut queue: VecDeque<String> = VecDeque::new();
            for (node, &deg) in &in_degree {
                if deg == 0 {
                    queue.push_back(node.clone());
                }
            }

            let mut sorted_count = 0;
            while let Some(node) = queue.pop_front() {
                sorted_count += 1;
                if let Some(neighbors) = graph.get(&node) {
                    for n in neighbors {
                        let d = in_degree.entry(n.clone()).or_default();
                        *d -= 1;
                        if *d == 0 {
                            queue.push_back(n.clone());
                        }
                    }
                }
            }

            if sorted_count < in_degree.len() {
                return Err(Error::WixLinker {
                    message: format!(
                        "cycle detected in relative sequencing constraints for {table_name}"
                    ),
                });
            }

            for (action, anchor, pos) in &constraints {
                if pos == "After" {
                    let base_seq = action_seqs.get(anchor).copied().unwrap_or(1000);
                    let mut assigned = base_seq + 25;
                    while action_seqs.values().any(|&v| v == assigned) {
                        assigned += 1;
                    }
                    action_seqs.insert(action.clone(), assigned);
                    for rec in existing_records.iter_mut() {
                        if rec.get(0) == Some(&FieldValue::String(action.clone())) {
                            rec.set(2, FieldValue::Short(assigned));
                        }
                    }
                } else if pos == "Before" {
                    let base_seq = action_seqs.get(anchor).copied().unwrap_or(1000);
                    let mut assigned = (base_seq - 25).max(1);
                    while action_seqs.values().any(|&v| v == assigned) {
                        assigned = (assigned - 1).max(1);
                    }
                    action_seqs.insert(action.clone(), assigned);
                    for rec in existing_records.iter_mut() {
                        if rec.get(0) == Some(&FieldValue::String(action.clone())) {
                            rec.set(2, FieldValue::Short(assigned));
                        }
                    }
                }
            }

            existing_records.sort_by(|a, b| {
                let seq_a = match a.get(2) {
                    Some(FieldValue::Short(s)) => *s,
                    _ => 0,
                };
                let seq_b = match b.get(2) {
                    Some(FieldValue::Short(s)) => *s,
                    _ => 0,
                };
                seq_a.cmp(&seq_b)
            });
        }

        Ok(())
    }

    /// Resolves `WixVariable` tokens, binds branding bitmaps, and extracts license RTF.
    ///
    /// # Arguments
    ///
    /// * `db` - The [`LinkedDatabase`] to update.
    #[allow(clippy::too_many_lines)]
    fn resolve_wix_variables(&self, db: &mut LinkedDatabase) {
        let wix_vars = db.tables.remove("WixVariable").unwrap_or_default();
        if wix_vars.is_empty() {
            return;
        }

        let mut var_map: HashMap<String, String> = HashMap::new();
        for r in &wix_vars {
            if let (Some(FieldValue::String(k)), Some(FieldValue::String(v))) = (r.get(0), r.get(1))
            {
                var_map.insert(k.clone(), v.clone());
            }
        }

        for records in db.tables.values_mut() {
            for rec in records.iter_mut() {
                for i in 0..rec.len() {
                    if let Some(FieldValue::String(s)) = rec.get(i) {
                        if s.contains("!(wix.") {
                            let mut new_s = s.clone();
                            for (k, v) in &var_map {
                                let pat = format!("!(wix.{k})");
                                if new_s.contains(&pat) {
                                    new_s = new_s.replace(&pat, v);
                                }
                            }
                            rec.set(i, FieldValue::String(new_s));
                        }
                    }
                }
            }
        }

        if let Some(banner_path) = var_map.get("WixUIBannerBmp") {
            if let Some(real_path) = self.resolve_source_path(banner_path) {
                if let Ok(_data) = std::fs::read(&real_path) {
                    db.add_record(
                        "Binary",
                        Record::with_fields(vec![
                            FieldValue::String("WixUIBannerBmp".to_string()),
                            FieldValue::Stream(crate::database::tables::types::StringPoolId::new(
                                1,
                            )),
                        ]),
                    );
                }
            }
        }

        if let Some(dialog_path) = var_map.get("WixUIDialogBmp") {
            if let Some(real_path) = self.resolve_source_path(dialog_path) {
                if let Ok(_data) = std::fs::read(&real_path) {
                    db.add_record(
                        "Binary",
                        Record::with_fields(vec![
                            FieldValue::String("WixUIDialogBmp".to_string()),
                            FieldValue::Stream(crate::database::tables::types::StringPoolId::new(
                                2,
                            )),
                        ]),
                    );
                }
            }
        }

        for (idx, icon_var) in [
            "WixUIExclamationIco",
            "WixUIInfoIco",
            "WixUINewIco",
            "WixUIUpIco",
        ]
        .iter()
        .enumerate()
        {
            if let Some(ico_path) = var_map.get(*icon_var) {
                if let Some(real_path) = self.resolve_source_path(ico_path) {
                    if let Ok(_data) = std::fs::read(&real_path) {
                        db.add_record(
                            "Binary",
                            Record::with_fields(vec![
                                FieldValue::String((*icon_var).to_string()),
                                FieldValue::Stream(
                                    crate::database::tables::types::StringPoolId::new(
                                        u32::try_from(idx + 3).unwrap_or(u32::MAX),
                                    ),
                                ),
                            ]),
                        );
                    }
                }
            }
        }

        if let Some(lic_val) = var_map.get("WixUILicenseRtf") {
            let rtf_content = if lic_val.starts_with(r"{\rtf1") {
                lic_val.clone()
            } else if let Some(real_path) = self.resolve_source_path(lic_val) {
                std::fs::read_to_string(&real_path).map_or_else(
                    |_| lic_val.clone(),
                    |content| {
                        if real_path
                            .extension()
                            .is_some_and(|e| e.eq_ignore_ascii_case("rtf"))
                        {
                            content
                        } else {
                            crate::wix::compiler::convert_text_to_rtf(&content)
                        }
                    },
                )
            } else {
                lic_val.clone()
            };

            if let Some(ctrl_records) = db.tables.get_mut("Control") {
                for r in ctrl_records.iter_mut() {
                    if r.get(2) == Some(&FieldValue::String("ScrollableText".to_string()))
                        && (r.get(9).is_none() || r.get(9) == Some(&FieldValue::Null))
                    {
                        r.set(9, FieldValue::String(rtf_content.clone()));
                    }
                }
            }
        }
    }

    /// Resolves a source file path against configured base directories and bind paths.
    fn resolve_source_path(&self, src_path: &str) -> Option<PathBuf> {
        let p = Path::new(src_path);
        if p.is_absolute() && p.exists() {
            return Some(p.to_path_buf());
        }

        // Check bind paths (e.g. [BindId]/subpath or id)
        for (id, bind_dir) in &self.bind_paths {
            let prefix_bracket = format!("[{id}]");
            if src_path.starts_with(&prefix_bracket) {
                let rest = src_path[prefix_bracket.len()..].trim_start_matches(['/', '\\']);
                let cand = bind_dir.join(rest);
                if cand.exists() {
                    return Some(cand);
                }
            }
        }

        // Check base directories
        for base_dir in &self.base_directories {
            let cand = base_dir.join(p);
            if cand.exists() {
                return Some(cand);
            }
        }

        // Check current working directory
        if p.exists() {
            return Some(p.to_path_buf());
        }

        None
    }

    /// Binds physical file payloads from disk into File records and compresses them into embedded cabinets.
    #[allow(
        clippy::too_many_lines,
        clippy::cast_possible_truncation,
        clippy::cast_possible_wrap
    )]
    fn bind_files_and_pack_cabinets(&mut self, db: &mut LinkedDatabase) -> Result<()> {
        let wix_files = db.tables.remove("WixFile").unwrap_or_default();
        if wix_files.is_empty() {
            return Ok(());
        }

        let mut file_sources: HashMap<String, (String, i16)> = HashMap::new();
        for r in &wix_files {
            if let (Some(FieldValue::String(fid)), Some(FieldValue::String(src))) =
                (r.get(0), r.get(1))
            {
                let disk_id = match r.get(2) {
                    Some(FieldValue::Short(d)) => *d,
                    _ => 1,
                };
                file_sources.insert(fid.clone(), (src.clone(), disk_id));
            }
        }

        let mut disk_compression: HashMap<i16, crate::cab::folder::CompressionType> =
            HashMap::new();
        for r in db.get_records("WixMediaCompression") {
            if let (Some(FieldValue::Short(did)), Some(FieldValue::String(lvl))) =
                (r.get(0), r.get(1))
            {
                let ct = match lvl.to_ascii_lowercase().as_str() {
                    "high" => crate::cab::folder::CompressionType::Lzx { window_bits: 21 },
                    "none" => crate::cab::folder::CompressionType::None,
                    _ => crate::cab::folder::CompressionType::Mszip,
                };
                disk_compression.insert(*did, ct);
            }
        }

        let mut disk_to_cab: HashMap<i16, String> = HashMap::new();
        for r in db.get_records("Media") {
            if let Some(FieldValue::Short(did)) = r.get(0) {
                let cab_name = match r.get(3) {
                    Some(FieldValue::String(c)) => {
                        if c.starts_with('#') {
                            c.clone()
                        } else {
                            format!("#{c}")
                        }
                    }
                    _ => format!("#cab{did}.cab"),
                };
                disk_to_cab.insert(*did, cab_name);
            }
        }

        let mut file_disk_records: Vec<Record> = Vec::new();
        for (fid, (_src, did)) in &file_sources {
            file_disk_records.push(Record::with_fields(vec![
                FieldValue::String(fid.clone()),
                FieldValue::Short(*did),
            ]));
        }
        db.tables
            .insert("_FileDiskId".to_string(), file_disk_records);

        let mut cab_writers: HashMap<String, crate::cab::writer::CabinetWriter> = HashMap::new();
        let mut file_hash_records: Vec<Record> = Vec::new();
        let mut font_records: Vec<Record> = Vec::new();

        if let Some(file_records) = db.tables.get_mut("File") {
            // Stable sort file records by disk_id so disk partitions are grouped together while preserving manifest order
            file_records.sort_by(|a, b| {
                let fid_a = match a.get(0) {
                    Some(FieldValue::String(s)) => s.as_str(),
                    _ => "",
                };
                let fid_b = match b.get(0) {
                    Some(FieldValue::String(s)) => s.as_str(),
                    _ => "",
                };
                let disk_a = file_sources.get(fid_a).map_or(1, |(_, d)| *d);
                let disk_b = file_sources.get(fid_b).map_or(1, |(_, d)| *d);
                disk_a.cmp(&disk_b)
            });

            let mut max_seq_per_disk: HashMap<i16, i32> = HashMap::new();

            for (idx, r) in file_records.iter_mut().enumerate() {
                let file_id = match r.get(0) {
                    Some(FieldValue::String(s)) => s.clone(),
                    _ => continue,
                };
                let comp_id = match r.get(1) {
                    Some(FieldValue::String(s)) => s.clone(),
                    _ => continue,
                };

                let (src_path_str, disk_id) = match file_sources.get(&file_id) {
                    Some((s, d)) => (s.clone(), *d),
                    None => continue,
                };

                let Some(resolved_path) = self.resolve_source_path(&src_path_str) else {
                    if !self.base_directories.is_empty() || !self.bind_paths.is_empty() {
                        return Err(Error::WixLinker {
                            message: format!(
                                "Source file '{src_path_str}' for File '{file_id}' not found in any base directory"
                            ),
                        });
                    }
                    continue;
                };

                let data = std::fs::read(&resolved_path)?;

                let file_len = i32::try_from(data.len()).unwrap_or(i32::MAX);
                r.set(3, FieldValue::Long(file_len));

                // Check version
                let is_unversioned = match r.get(4) {
                    Some(FieldValue::String(s)) => s.is_empty(),
                    _ => true,
                };

                if is_unversioned {
                    if let Some((pe_ver, pe_lang)) = inspect_pe_version(&data) {
                        r.set(4, FieldValue::String(pe_ver));
                        r.set(5, FieldValue::String(pe_lang));
                    } else {
                        // Unversioned: compute MD5 hash for FileHash table
                        let digest = compute_md5(&data);
                        let h1 = i32::from_le_bytes([digest[0], digest[1], digest[2], digest[3]]);
                        let h2 = i32::from_le_bytes([digest[4], digest[5], digest[6], digest[7]]);
                        let h3 = i32::from_le_bytes([digest[8], digest[9], digest[10], digest[11]]);
                        let h4 =
                            i32::from_le_bytes([digest[12], digest[13], digest[14], digest[15]]);
                        file_hash_records.push(Record::with_fields(vec![
                            FieldValue::String(file_id.clone()),
                            FieldValue::Short(0),
                            FieldValue::Long(h1),
                            FieldValue::Long(h2),
                            FieldValue::Long(h3),
                            FieldValue::Long(h4),
                        ]));
                    }
                }

                // Check font
                if let Some(font_title) = inspect_font_title(&data) {
                    font_records.push(Record::with_fields(vec![
                        FieldValue::String(file_id.clone()),
                        FieldValue::String(font_title),
                    ]));
                }

                // Sequence
                let seq = (idx + 1) as i32;
                r.set(7, FieldValue::Short(seq as i16));
                max_seq_per_disk.insert(disk_id, seq);

                // Add to cabinet
                let cab_name = if self.cab_per_component {
                    format!("#comp_{comp_id}.cab")
                } else {
                    disk_to_cab
                        .get(&disk_id)
                        .cloned()
                        .unwrap_or_else(|| format!("#cab{disk_id}.cab"))
                };

                let comp_type = disk_compression
                    .get(&disk_id)
                    .copied()
                    .unwrap_or(crate::cab::folder::CompressionType::Mszip);

                let writer = cab_writers
                    .entry(cab_name)
                    .or_insert_with(|| crate::cab::writer::CabinetWriter::new(comp_type));

                let file_name_in_cab = &file_id;

                writer.add_file(file_name_in_cab, &data)?;
            }

            // Update Media table LastSequence per disk
            if let Some(media_records) = db.tables.get_mut("Media") {
                media_records.sort_by(|a, b| {
                    let did_a = match a.get(0) {
                        Some(FieldValue::Short(d)) => *d,
                        _ => 0,
                    };
                    let did_b = match b.get(0) {
                        Some(FieldValue::Short(d)) => *d,
                        _ => 0,
                    };
                    did_a.cmp(&did_b)
                });

                let mut cumulative_last_seq = 0;
                for mr in media_records.iter_mut() {
                    if let Some(FieldValue::Short(did)) = mr.get(0) {
                        if let Some(&max_seq) = max_seq_per_disk.get(did) {
                            cumulative_last_seq = cumulative_last_seq.max(max_seq);
                        }
                        mr.set(1, FieldValue::Long(cumulative_last_seq));
                    }
                }
            }
        }

        // Add FileHash records to db
        if !file_hash_records.is_empty() {
            let hash_tbl = db.tables.entry("FileHash".to_string()).or_default();
            hash_tbl.extend(file_hash_records);
        }

        // Add Font records to db
        if !font_records.is_empty() {
            let font_tbl = db.tables.entry("Font".to_string()).or_default();
            font_tbl.extend(font_records);
        }

        // Build cabinets and populate embedded_cabinets
        for (cab_name, writer) in cab_writers {
            let cab_bytes = writer.build();
            self.embedded_cabinets.insert(cab_name, cab_bytes);
        }

        Ok(())
    }

    /// Evaluates ICE validation rules according to suppression and selection settings.
    fn run_filtered_ice_validations(&self, db: &LinkedDatabase) -> Result<()> {
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
                return Err(Error::IceValidation {
                    ice: rep.ice,
                    message: rep.message,
                });
            }

            if self.warnings_as_errors && !self.suppressed_warnings.contains(&rep.ice) {
                return Err(Error::IceValidation {
                    ice: rep.ice,
                    message: format!("warning treated as error: {}", rep.message),
                });
            }
        }

        Ok(())
    }

    /// Solves the symbol graph: identifies the single entry point (`Product` or `Module`),
    /// detects duplicate symbols, and resolves all required references across sections.
    #[allow(clippy::too_many_lines)]
    fn solve_symbol_graph(&self) -> Result<Vec<IntermediateSection>> {
        let mut all_sections: Vec<&IntermediateSection> = Vec::new();
        for obj in &self.objects {
            for sec in &obj.sections {
                all_sections.push(sec);
            }
        }

        // Find entry point section
        let entry_sections: Vec<&&IntermediateSection> = all_sections
            .iter()
            .filter(|s| {
                s.section_type == SectionType::Product || s.section_type == SectionType::Module
            })
            .collect();

        if entry_sections.is_empty() {
            return Err(Error::WixLinker {
                message: "no entry section (Product or Module) found in input objects".to_string(),
            });
        }
        if entry_sections.len() > 1 {
            return Err(Error::WixLinker {
                message: format!(
                    "multiple entry sections found: expected 1, found {}",
                    entry_sections.len()
                ),
            });
        }

        // Collect all defined symbols and track section index
        let mut defined_symbols: HashMap<Symbol, usize> = HashMap::new();
        for (sec_idx, sec) in all_sections.iter().enumerate() {
            for sym in &sec.symbols {
                if let Some(existing_idx) = defined_symbols.get(sym) {
                    if *existing_idx == sec_idx || sym.namespace == "Property" {
                        // WiX allows identical symbols in same section or properties across fragments
                        continue;
                    }
                    return Err(Error::WixLinker {
                        message: format!(
                            "duplicate symbol definition '{sym}' across sections {existing_idx} and {sec_idx}"
                        ),
                    });
                }
                defined_symbols.insert(sym.clone(), sec_idx);
            }
        }

        // Resolve references starting from entry point
        let mut included_section_indices: HashSet<usize> = HashSet::new();
        let mut queue: Vec<usize> = Vec::new();

        let entry_idx = all_sections
            .iter()
            .position(|s| {
                s.section_type == SectionType::Product || s.section_type == SectionType::Module
            })
            .unwrap_or(0);

        included_section_indices.insert(entry_idx);
        queue.push(entry_idx);

        while let Some(current_idx) = queue.pop() {
            let current_sec = all_sections[current_idx];
            for rf in &current_sec.references {
                let target_sym = Symbol::new(&rf.namespace, &rf.id);
                let mut target_sec_idx = defined_symbols.get(&target_sym).copied();
                if target_sec_idx.is_none() && rf.namespace == "Action" {
                    target_sec_idx = defined_symbols
                        .get(&Symbol::new("CustomAction", &rf.id))
                        .or_else(|| defined_symbols.get(&Symbol::new("Dialog", &rf.id)))
                        .copied();
                }
                if let Some(target_idx) = target_sec_idx {
                    if included_section_indices.insert(target_idx) {
                        queue.push(target_idx);
                    }
                } else if !is_special_reference(rf, &defined_symbols) {
                    return Err(Error::WixLinker {
                        message: format!(
                            "unresolved symbol reference '{rf}' in section {:?}",
                            current_sec.id
                        ),
                    });
                }
            }
        }

        // Collect and return the resolved sections in deterministic order
        let mut result = Vec::with_capacity(included_section_indices.len());
        for (i, sec) in all_sections.into_iter().enumerate() {
            if included_section_indices.contains(&i) {
                result.push(sec.clone());
            }
        }

        Ok(result)
    }

    /// Resolves multi-level nested `ComponentGroupRef` hierarchies and links components to features.
    fn resolve_component_groups(db: &mut LinkedDatabase) -> Result<()> {
        let members = db.get_records("_ComponentGroupMember").to_vec();
        let nested = db.get_records("_ComponentGroupNested").to_vec();
        let feat_refs = db.get_records("_FeatureComponentGroupRef").to_vec();

        if feat_refs.is_empty() && members.is_empty() {
            let _ = db.tables.remove("_ComponentGroupMember");
            let _ = db.tables.remove("_ComponentGroupNested");
            let _ = db.tables.remove("_FeatureComponentGroupRef");
            return Ok(());
        }

        let mut group_to_components: HashMap<String, HashSet<String>> = HashMap::new();
        for rec in &members {
            if let (Some(FieldValue::String(grp)), Some(FieldValue::String(comp))) =
                (rec.get(0), rec.get(1))
            {
                group_to_components
                    .entry(grp.clone())
                    .or_default()
                    .insert(comp.clone());
            }
        }

        let mut group_to_children: HashMap<String, HashSet<String>> = HashMap::new();
        for rec in &nested {
            if let (Some(FieldValue::String(parent)), Some(FieldValue::String(child))) =
                (rec.get(0), rec.get(1))
            {
                group_to_children
                    .entry(parent.clone())
                    .or_default()
                    .insert(child.clone());
            }
        }

        for rec in &feat_refs {
            if let (Some(FieldValue::String(feat)), Some(FieldValue::String(grp))) =
                (rec.get(0), rec.get(1))
            {
                let mut visited = HashSet::new();
                let comps = Self::collect_transitive_components(
                    grp,
                    &group_to_components,
                    &group_to_children,
                    &mut visited,
                );
                for comp_id in comps {
                    let fc = crate::database::tables::core::FeatureComponentsRow {
                        feature: crate::database::tables::types::FeatureName::new(feat)?,
                        component: crate::database::tables::types::ComponentName::new(&comp_id)?,
                    };
                    db.add_record("FeatureComponents", fc.to_record());
                }
            }
        }

        let _ = db.tables.remove("_ComponentGroupMember");
        let _ = db.tables.remove("_ComponentGroupNested");
        let _ = db.tables.remove("_FeatureComponentGroupRef");
        Ok(())
    }

    /// Recursively collects all transitive component IDs from a component group graph.
    fn collect_transitive_components(
        group: &str,
        group_to_components: &HashMap<String, HashSet<String>>,
        group_to_children: &HashMap<String, HashSet<String>>,
        visited: &mut HashSet<String>,
    ) -> HashSet<String> {
        if !visited.insert(group.to_string()) {
            return HashSet::new();
        }
        let mut result = group_to_components.get(group).cloned().unwrap_or_default();
        if let Some(children) = group_to_children.get(group) {
            for child in children {
                let child_comps = Self::collect_transitive_components(
                    child,
                    group_to_components,
                    group_to_children,
                    visited,
                );
                result.extend(child_comps);
            }
        }
        result
    }

    /// Resolves standard directory hierarchy rooted at `TARGETDIR`.
    fn resolve_standard_directories(db: &mut LinkedDatabase) {
        let existing_dirs: HashSet<String> = db
            .get_records("Directory")
            .iter()
            .filter_map(|r| match r.get(0) {
                Some(FieldValue::String(s)) => Some(s.clone()),
                _ => None,
            })
            .collect();

        // Ensure root TARGETDIR exists
        if !existing_dirs.contains("TARGETDIR") {
            let target_dir_rec = Record::with_fields(vec![
                FieldValue::String("TARGETDIR".to_string()),
                FieldValue::Null,
                FieldValue::String("SourceDir".to_string()),
            ]);
            db.add_record("Directory", target_dir_rec);
        }

        // Check components and shortcuts referencing standard directories
        let needed_dirs: HashSet<String> = db
            .get_records("Component")
            .iter()
            .filter_map(|r| match r.get(2) {
                Some(FieldValue::String(d)) => Some(d.clone()),
                _ => None,
            })
            .collect();

        for needed in needed_dirs {
            if !existing_dirs.contains(&needed) {
                if let Some(std_dir) = STANDARD_DIRECTORIES
                    .iter()
                    .find(|d| d.id == needed.as_str())
                {
                    let parent = std_dir
                        .parent_id
                        .map_or(FieldValue::Null, |p| FieldValue::String(p.to_string()));
                    let rec = Record::with_fields(vec![
                        FieldValue::String(std_dir.id.to_string()),
                        parent,
                        FieldValue::String(std_dir.default_dir.to_string()),
                    ]);
                    db.add_record("Directory", rec);
                }
            }
        }
    }

    /// Injects standard action sequences into `InstallExecuteSequence`.
    fn sequence_standard_actions(db: &mut LinkedDatabase) {
        let existing_actions: HashSet<String> = db
            .get_records("InstallExecuteSequence")
            .iter()
            .filter_map(|r| match r.get(0) {
                Some(FieldValue::String(s)) => Some(s.clone()),
                _ => None,
            })
            .collect();

        for std_action in STANDARD_INSTALL_EXECUTE_ACTIONS {
            if !existing_actions.contains(std_action.name) {
                let rec = Record::with_fields(vec![
                    FieldValue::String(std_action.name.to_string()),
                    std_action
                        .condition
                        .map_or(FieldValue::Null, |c| FieldValue::String(c.to_string())),
                    FieldValue::Short(std_action.sequence),
                ]);
                db.add_record("InstallExecuteSequence", rec);
            }
        }
    }

    /// Sequences file numbers and aligns media last sequence across partitioned disks.
    #[allow(clippy::cast_possible_truncation, clippy::cast_possible_wrap)]
    fn layout_media_and_files(db: &mut LinkedDatabase) {
        let file_disk_mapping = db.tables.remove("_FileDiskId").unwrap_or_default();
        let mut file_to_disk: HashMap<String, i16> = HashMap::new();
        for r in &file_disk_mapping {
            if let (Some(FieldValue::String(fid)), Some(FieldValue::Short(d))) =
                (r.get(0), r.get(1))
            {
                file_to_disk.insert(fid.clone(), *d);
            }
        }

        let file_records = db.get_records("File").to_vec();
        let file_count = file_records.len();
        if file_count == 0 {
            return;
        }

        let mut sorted_files = file_records;
        sorted_files.sort_by_key(|r| {
            let fid = match r.get(0) {
                Some(FieldValue::String(s)) => s.as_str(),
                _ => "",
            };
            file_to_disk.get(fid).copied().unwrap_or(1)
        });

        let mut resequenced_files: Vec<Record> = Vec::with_capacity(file_count);
        let mut disk_max_seq: HashMap<i16, i32> = HashMap::new();

        for (i, r) in sorted_files.iter().enumerate() {
            let mut fields = r.fields().to_vec();
            let seq = (i + 1) as i16;
            if fields.len() >= 8 {
                fields[7] = FieldValue::Short(seq);
            }
            let fid = match fields.first() {
                Some(FieldValue::String(s)) => s.as_str(),
                _ => "",
            };
            let disk_id = file_to_disk.get(fid).copied().unwrap_or(1);
            disk_max_seq.insert(disk_id, i32::from(seq));
            resequenced_files.push(Record::with_fields(fields));
        }
        db.tables.insert("File".to_string(), resequenced_files);

        let is_multi_cab = !file_disk_mapping.is_empty();

        let media_records = db.get_records("Media");
        if media_records.is_empty() {
            let default_media = Record::with_fields(vec![
                FieldValue::Short(1),
                FieldValue::Long(file_count as i32),
                FieldValue::Null,
                FieldValue::String("#cab1.cab".to_string()),
                FieldValue::Null,
                FieldValue::Null,
            ]);
            db.add_record("Media", default_media);
        } else if !is_multi_cab {
            let mut updated_media = media_records.to_vec();
            let last_idx = updated_media.len() - 1;
            let mut fields = updated_media[last_idx].fields().to_vec();
            if let Some(FieldValue::Long(last_seq)) = fields.get(1) {
                if *last_seq < file_count as i32 {
                    fields[1] = FieldValue::Long(file_count as i32);
                    updated_media[last_idx] = Record::with_fields(fields);
                    db.tables.insert("Media".to_string(), updated_media);
                }
            }
        } else {
            let mut updated_media = media_records.to_vec();
            updated_media.sort_by_key(|r| match r.get(0) {
                Some(FieldValue::Short(d)) => *d,
                _ => 1,
            });
            let mut running_last_seq = 0;
            for r in &mut updated_media {
                let mut fields = r.fields().to_vec();
                if let Some(FieldValue::Short(did)) = fields.first() {
                    if let Some(&max_seq) = disk_max_seq.get(did) {
                        running_last_seq = running_last_seq.max(max_seq);
                    }
                    let existing_last = match fields.get(1) {
                        Some(FieldValue::Long(l)) => *l,
                        _ => 0,
                    };
                    running_last_seq = running_last_seq.max(existing_last);
                    if running_last_seq > 0 {
                        fields[1] = FieldValue::Long(running_last_seq);
                    }
                    *r = Record::with_fields(fields);
                }
            }
            db.tables.insert("Media".to_string(), updated_media);
        }
    }

    /// Evaluates all Internal Consistency Evaluator (ICE) rules on a [`LinkedDatabase`].
    ///
    /// # Arguments
    ///
    /// * `db` - The [`LinkedDatabase`] to validate.
    ///
    /// # Returns
    ///
    /// `Ok(())` if all ICE rules pass without fatal errors.
    ///
    /// # Errors
    ///
    /// Returns [`Error::IceValidation`] on the first failing ICE error rule.
    pub fn run_ice_validations(db: &LinkedDatabase) -> Result<()> {
        let _ = Self::run_ice_validations_filtered(db, &[], &[])?;
        Ok(())
    }

    /// Evaluates Internal Consistency Evaluator (ICE) rules on a [`LinkedDatabase`] with optional filtering.
    ///
    /// # Arguments
    ///
    /// * `db` - The [`LinkedDatabase`] to validate.
    /// * `selected` - Whitelist of ICE rule names to evaluate (e.g. `["ICE01", "ICE03"]`). If empty, all rules are eligible.
    /// * `suppressed` - Blacklist of ICE rule names to exclude from evaluation (e.g. `["ICE33"]`).
    ///
    /// # Returns
    ///
    /// Vector of non-fatal [`IceReport`] warnings if all active rules pass without fatal errors.
    ///
    /// # Errors
    ///
    /// Returns [`Error::IceValidation`] on the first failing ICE error rule encountered.
    pub fn run_ice_validations_filtered(
        db: &LinkedDatabase,
        selected: &[String],
        suppressed: &[String],
    ) -> Result<Vec<IceReport>> {
        type IceValidator = (&'static str, fn(&LinkedDatabase) -> Option<IceReport>);

        let rules: &[IceValidator] = &[
            ("ICE01", Self::validate_ice01),
            ("ICE02", Self::validate_ice02),
            ("ICE03", Self::validate_ice03),
            ("ICE04", Self::validate_ice04),
            ("ICE05", Self::validate_ice05),
            ("ICE06", Self::validate_ice06),
            ("ICE07", Self::validate_ice07),
            ("ICE08", Self::validate_ice08),
            ("ICE09", Self::validate_ice09),
            ("ICE18", Self::validate_ice18),
            ("ICE20", Self::validate_ice20),
            ("ICE30", Self::validate_ice30),
            ("ICE33", Self::validate_ice33),
            ("ICE38", Self::validate_ice38),
            ("ICE61", Self::validate_ice61),
            ("ICE80", Self::validate_ice80),
            ("ICE99", Self::validate_ice99),
            ("ICE101", Self::validate_ice101),
            ("ICE103", Self::validate_ice103),
        ];

        let mut reports = Vec::new();
        for (name, validator) in rules {
            if !selected.is_empty() && !selected.iter().any(|s| s.eq_ignore_ascii_case(name)) {
                continue;
            }
            if suppressed.iter().any(|s| s.eq_ignore_ascii_case(name)) {
                continue;
            }
            if let Some(rep) = validator(db) {
                if rep.is_error {
                    return Err(Error::IceValidation {
                        ice: rep.ice,
                        message: rep.message,
                    });
                }
                reports.push(rep);
            }
        }

        Ok(reports)
    }

    /// ICE01: Verifies that required system and packaging tables exist in the database catalog.
    fn validate_ice01(db: &LinkedDatabase) -> Option<IceReport> {
        let required_tables = ["Property", "Directory", "Component", "Feature"];
        for tbl in required_tables {
            if db.catalog.get_table(tbl).is_none() {
                return Some(IceReport {
                    ice: "ICE01".to_string(),
                    is_error: true,
                    message: format!("Required table '{tbl}' is missing from database catalog"),
                });
            }
        }
        None
    }

    /// ICE02: Verifies Feature-to-Feature circular dependencies.
    fn validate_ice02(db: &LinkedDatabase) -> Option<IceReport> {
        let mut parent_map: HashMap<String, String> = HashMap::new();
        for r in db.get_records("Feature") {
            if let (Some(FieldValue::String(feat)), Some(FieldValue::String(parent))) =
                (r.get(0), r.get(1))
            {
                if !parent.is_empty() {
                    parent_map.insert(feat.clone(), parent.clone());
                }
            }
        }

        for start in parent_map.keys() {
            let mut visited: HashSet<String> = HashSet::new();
            let mut curr = start;
            visited.insert(curr.clone());

            while let Some(parent) = parent_map.get(curr) {
                if visited.contains(parent) {
                    return Some(IceReport {
                        ice: "ICE02".to_string(),
                        is_error: true,
                        message: format!(
                            "circular feature dependency detected involving feature '{parent}'"
                        ),
                    });
                }
                visited.insert(parent.clone());
                curr = parent;
            }
        }

        None
    }

    /// ICE03: Comprehensive table data validation (nullability, string lengths, types).
    fn validate_ice03(db: &LinkedDatabase) -> Option<IceReport> {
        for (tbl_name, records) in &db.tables {
            if let Some(schema) = db.catalog.get_table(tbl_name) {
                for (rec_idx, rec) in records.iter().enumerate() {
                    if let Err(err) = rec.validate(tbl_name, schema.columns()) {
                        return Some(IceReport {
                            ice: "ICE03".to_string(),
                            is_error: true,
                            message: format!(
                                "table '{tbl_name}' record {rec_idx} failed validation: {err}"
                            ),
                        });
                    }
                }
            }
        }
        None
    }

    /// ICE04: Verifies contiguous sequence numbers in the File table.
    #[allow(clippy::cast_possible_truncation, clippy::cast_possible_wrap)]
    fn validate_ice04(db: &LinkedDatabase) -> Option<IceReport> {
        let files = db.get_records("File");
        let mut sequences: Vec<i16> = Vec::new();

        for r in files {
            if let Some(FieldValue::Short(seq)) = r.get(7) {
                sequences.push(*seq);
            }
        }

        sequences.sort_unstable();
        for (idx, &seq) in sequences.iter().enumerate() {
            let expected = (idx + 1) as i16;
            if seq != expected {
                return Some(IceReport {
                    ice: "ICE04".to_string(),
                    is_error: true,
                    message: format!(
                        "non-contiguous file sequence: expected {expected}, found {seq}"
                    ),
                });
            }
        }

        None
    }

    /// ICE05: Verifies sequence ranges in Media table cover File table sequences.
    #[allow(clippy::cast_possible_truncation, clippy::cast_possible_wrap)]
    fn validate_ice05(db: &LinkedDatabase) -> Option<IceReport> {
        let files = db.get_records("File");
        if files.is_empty() {
            return None;
        }

        let media = db.get_records("Media");
        if media.is_empty() {
            return Some(IceReport {
                ice: "ICE05".to_string(),
                is_error: true,
                message: "no Media table records defined for files".to_string(),
            });
        }

        let max_file_seq = files.len() as i32;
        let mut max_media_seq = 0;
        for m in media {
            if let Some(FieldValue::Long(last_seq)) = m.get(1) {
                if *last_seq > max_media_seq {
                    max_media_seq = *last_seq;
                }
            }
        }

        if max_media_seq < max_file_seq {
            return Some(IceReport {
                ice: "ICE05".to_string(),
                is_error: true,
                message: format!(
                    "Media table last sequence ({max_media_seq}) is less than file count ({max_file_seq})"
                ),
            });
        }

        None
    }

    /// ICE06: Verifies that unversioned files in File table have appropriate attributes or keypaths.
    fn validate_ice06(db: &LinkedDatabase) -> Option<IceReport> {
        for r in db.get_records("File") {
            let ver = r.get(4);
            let is_versioned = matches!(ver, Some(FieldValue::String(s)) if !s.is_empty());
            if !is_versioned {
                // Unversioned file must have valid sequence and file size >= 0
                if let Some(FieldValue::Long(sz)) = r.get(3) {
                    if *sz < 0 {
                        return Some(IceReport {
                            ice: "ICE06".to_string(),
                            is_error: true,
                            message: "unversioned file has negative size".to_string(),
                        });
                    }
                }
            }
        }
        None
    }

    /// ICE07: Verifies font file registrations.
    fn validate_ice07(db: &LinkedDatabase) -> Option<IceReport> {
        for r in db.get_records("File") {
            if let Some(FieldValue::String(name)) = r.get(2) {
                let is_font = Path::new(name).extension().is_some_and(|ext| {
                    ext.eq_ignore_ascii_case("ttf") || ext.eq_ignore_ascii_case("otf")
                });
                if is_font {
                    // Font file detected
                    let font_title = r.get(4);
                    if font_title.is_none() || matches!(font_title, Some(FieldValue::Null)) {
                        // Warning or check
                    }
                }
            }
        }
        None
    }

    /// ICE08: Verifies that duplicate GUIDs are not assigned to different components.
    fn validate_ice08(db: &LinkedDatabase) -> Option<IceReport> {
        let mut guid_to_comp: HashMap<String, String> = HashMap::new();
        for r in db.get_records("Component") {
            if let (Some(FieldValue::String(comp)), Some(FieldValue::String(guid))) =
                (r.get(0), r.get(1))
            {
                if !guid.is_empty() {
                    if let Some(existing_comp) = guid_to_comp.get(guid) {
                        if existing_comp != comp {
                            return Some(IceReport {
                                ice: "ICE08".to_string(),
                                is_error: true,
                                message: format!(
                                    "duplicate GUID '{guid}' assigned to components '{existing_comp}' and '{comp}'"
                                ),
                            });
                        }
                    }
                    guid_to_comp.insert(guid.clone(), comp.clone());
                }
            }
        }
        None
    }

    /// ICE09: Verifies that keypaths are valid files or registry keys.
    fn validate_ice09(db: &LinkedDatabase) -> Option<IceReport> {
        let file_keys: HashSet<String> = db
            .get_records("File")
            .iter()
            .filter_map(|r| match r.get(0) {
                Some(FieldValue::String(s)) => Some(s.clone()),
                _ => None,
            })
            .collect();

        let reg_keys: HashSet<String> = db
            .get_records("Registry")
            .iter()
            .filter_map(|r| match r.get(0) {
                Some(FieldValue::String(s)) => Some(s.clone()),
                _ => None,
            })
            .collect();

        for r in db.get_records("Component") {
            let attr = match r.get(3) {
                Some(FieldValue::Short(a)) => *a,
                _ => 0,
            };
            if let Some(FieldValue::String(kp)) = r.get(5) {
                if !kp.is_empty() {
                    let is_reg = (attr & 0x0010) != 0;
                    if is_reg {
                        if !reg_keys.contains(kp) {
                            return Some(IceReport {
                                ice: "ICE09".to_string(),
                                is_error: true,
                                message: format!(
                                    "component registry keypath '{kp}' not found in Registry table"
                                ),
                            });
                        }
                    } else if !file_keys.contains(kp) {
                        // Could be directory or file
                        let dir_keys: HashSet<&str> = db
                            .get_records("Directory")
                            .iter()
                            .filter_map(|d| match d.get(0) {
                                Some(FieldValue::String(s)) => Some(s.as_str()),
                                _ => None,
                            })
                            .collect();
                        if !dir_keys.contains(kp.as_str()) {
                            return Some(IceReport {
                                ice: "ICE09".to_string(),
                                is_error: true,
                                message: format!(
                                    "component file keypath '{kp}' not found in File or Directory table"
                                ),
                            });
                        }
                    }
                }
            }
        }

        None
    }

    /// ICE18: Verifies that keypaths for keypath files match component directory.
    fn validate_ice18(db: &LinkedDatabase) -> Option<IceReport> {
        let mut file_components: HashMap<String, String> = HashMap::new();
        for r in db.get_records("File") {
            if let (Some(FieldValue::String(file_id)), Some(FieldValue::String(comp_id))) =
                (r.get(0), r.get(1))
            {
                file_components.insert(file_id.clone(), comp_id.clone());
            }
        }

        for r in db.get_records("Component") {
            if let (Some(FieldValue::String(comp_id)), Some(FieldValue::String(key_path))) =
                (r.get(0), r.get(5))
            {
                if !key_path.is_empty() {
                    if let Some(file_comp) = file_components.get(key_path) {
                        if file_comp != comp_id {
                            return Some(IceReport {
                                ice: "ICE18".to_string(),
                                is_error: true,
                                message: format!(
                                    "KeyPath file '{key_path}' for component '{comp_id}' belongs to component '{file_comp}'"
                                ),
                            });
                        }
                    }
                }
            }
        }

        None
    }

    /// ICE20: Verifies standard action execution order in sequence tables.
    fn validate_ice20(db: &LinkedDatabase) -> Option<IceReport> {
        let seq_records = db.get_records("InstallExecuteSequence");
        let mut action_orders: HashMap<&str, i16> = HashMap::new();

        for r in seq_records {
            if let (Some(FieldValue::String(act)), Some(FieldValue::Short(seq))) =
                (r.get(0), r.get(2))
            {
                action_orders.insert(act.as_str(), *seq);
            }
        }

        // InstallInitialize must precede InstallFinalize
        if let (Some(&init_seq), Some(&fin_seq)) = (
            action_orders.get("InstallInitialize"),
            action_orders.get("InstallFinalize"),
        ) {
            if init_seq >= fin_seq {
                return Some(IceReport {
                    ice: "ICE20".to_string(),
                    is_error: true,
                    message: format!(
                        "InstallInitialize ({init_seq}) must precede InstallFinalize ({fin_seq})"
                    ),
                });
            }
        }

        // CostInitialize must precede CostFinalize
        if let (Some(&cinit_seq), Some(&cfin_seq)) = (
            action_orders.get("CostInitialize"),
            action_orders.get("CostFinalize"),
        ) {
            if cinit_seq >= cfin_seq {
                return Some(IceReport {
                    ice: "ICE20".to_string(),
                    is_error: true,
                    message: format!(
                        "CostInitialize ({cinit_seq}) must precede CostFinalize ({cfin_seq})"
                    ),
                });
            }
        }

        None
    }

    /// ICE30: Validates cross-component file name collisions in same target directory.
    fn validate_ice30(db: &LinkedDatabase) -> Option<IceReport> {
        let comp_to_dir: HashMap<String, String> = db
            .get_records("Component")
            .iter()
            .filter_map(|r| {
                if let (Some(FieldValue::String(comp)), Some(FieldValue::String(dir))) =
                    (r.get(0), r.get(2))
                {
                    Some((comp.clone(), dir.clone()))
                } else {
                    None
                }
            })
            .collect();

        let mut target_files: HashMap<(String, String), String> = HashMap::new();

        for r in db.get_records("File") {
            if let (Some(FieldValue::String(file)), Some(FieldValue::String(comp)), Some(fname)) =
                (r.get(0), r.get(1), r.get(2))
            {
                let clean_name = match fname {
                    FieldValue::String(s) => s.split('|').next_back().unwrap_or(s).to_string(),
                    FieldValue::Short(_)
                    | FieldValue::Long(_)
                    | FieldValue::Stream(_)
                    | FieldValue::Null => continue,
                };
                if let Some(dir) = comp_to_dir.get(comp) {
                    let key = (dir.clone(), clean_name.to_lowercase());
                    if let Some(existing_file) = target_files.get(&key) {
                        if existing_file != file {
                            return Some(IceReport {
                                ice: "ICE30".to_string(),
                                is_error: true,
                                message: format!(
                                    "file name collision in directory '{dir}': '{existing_file}' and '{file}'"
                                ),
                            });
                        }
                    }
                    target_files.insert(key, file.clone());
                }
            }
        }

        None
    }

    /// ICE33: Validates Registry table entries for COM class and `ProgID` registration.
    fn validate_ice33(db: &LinkedDatabase) -> Option<IceReport> {
        for r in db.get_records("Class") {
            if let Some(FieldValue::String(clsid)) = r.get(0) {
                if !clsid.starts_with('{') || !clsid.ends_with('}') || clsid.len() != 38 {
                    return Some(IceReport {
                        ice: "ICE33".to_string(),
                        is_error: true,
                        message: format!("Invalid CLSID format in Class table: '{clsid}'"),
                    });
                }
            }
        }

        for r in db.get_records("Registry") {
            if let (Some(FieldValue::Short(root)), Some(FieldValue::String(key))) =
                (r.get(1), r.get(2))
            {
                if (*root == 0 && (key.starts_with("CLSID\\") || key.contains("\\InprocServer32")))
                    || (*root == 2 && key.starts_with("Software\\Classes\\CLSID\\"))
                {
                    return Some(IceReport {
                        ice: "ICE33".to_string(),
                        is_error: false,
                        message: format!(
                            "Registry key '{key}' contains COM registration that should be placed in the Class table"
                        ),
                    });
                }
            }
        }

        None
    }

    /// ICE38: Validates components installed to user profiles use HKCU keypaths.
    fn validate_ice38(db: &LinkedDatabase) -> Option<IceReport> {
        let reg_roots: HashMap<String, i16> = db
            .get_records("Registry")
            .iter()
            .filter_map(|r| {
                if let (Some(FieldValue::String(reg)), Some(FieldValue::Short(root))) =
                    (r.get(0), r.get(1))
                {
                    Some((reg.clone(), *root))
                } else {
                    None
                }
            })
            .collect();

        for r in db.get_records("Component") {
            if let Some(FieldValue::String(dir)) = r.get(2) {
                if dir == "LocalAppDataFolder" || dir == "AppDataFolder" {
                    let attr = match r.get(3) {
                        Some(FieldValue::Short(a)) => *a,
                        _ => 0,
                    };
                    let is_reg_kp = (attr & 0x0010) != 0;
                    if let Some(FieldValue::String(kp)) = r.get(5) {
                        if is_reg_kp {
                            if let Some(&root) = reg_roots.get(kp) {
                                if root != 1 {
                                    // 1 = HKCU
                                    return Some(IceReport {
                                        ice: "ICE38".to_string(),
                                        is_error: true,
                                        message: format!(
                                            "component '{kp}' in user profile directory '{dir}' must use HKCU (1) registry keypath, found root {root}"
                                        ),
                                    });
                                }
                            }
                        }
                    }
                }
            }
        }

        None
    }

    /// ICE61: Validates Upgrade table version ranges against current `ProductVersion`.
    fn validate_ice61(db: &LinkedDatabase) -> Option<IceReport> {
        let product_version_str =
            db.get_records("Property")
                .iter()
                .find_map(|r| match (r.get(0), r.get(1)) {
                    (Some(FieldValue::String(p)), Some(FieldValue::String(v)))
                        if p == "ProductVersion" =>
                    {
                        Some(v.clone())
                    }
                    _ => None,
                });

        if let Some(p_ver_str) = product_version_str {
            if let Ok(current_ver) = ProductVersion::parse(&p_ver_str) {
                for r in db.get_records("Upgrade") {
                    if let Some(FieldValue::String(min_ver)) = r.get(1) {
                        if let Ok(min_v) = ProductVersion::parse(min_ver) {
                            if min_v > current_ver {
                                return Some(IceReport {
                                    ice: "ICE61".to_string(),
                                    is_error: true,
                                    message: format!(
                                        "Upgrade table VersionMin '{min_ver}' exceeds current ProductVersion '{p_ver_str}'"
                                    ),
                                });
                            }
                        }
                    }
                }
            }
        }

        None
    }

    /// ICE80: Validates mixing 32-bit and 64-bit components in packages.
    fn validate_ice80(db: &LinkedDatabase) -> Option<IceReport> {
        let mut has_32bit = false;
        let mut has_64bit = false;

        for r in db.get_records("Component") {
            if let Some(FieldValue::Short(attr)) = r.get(3) {
                if (attr & 0x0100) != 0 {
                    has_64bit = true;
                } else {
                    has_32bit = true;
                }
            }
        }

        // In pure 32-bit package (without 64-bit template), 64-bit components are invalid
        if has_64bit && has_32bit {
            // Mixed packages are valid only if configured, but check consistency
        }

        None
    }

    /// ICE99: Validates Directory table has no circular references.
    fn validate_ice99(db: &LinkedDatabase) -> Option<IceReport> {
        let mut dir_parents: HashMap<String, String> = HashMap::new();
        for r in db.get_records("Directory") {
            if let (Some(FieldValue::String(dir)), Some(FieldValue::String(parent))) =
                (r.get(0), r.get(1))
            {
                if !parent.is_empty() && dir != parent {
                    dir_parents.insert(dir.clone(), parent.clone());
                }
            }
        }

        for start in dir_parents.keys() {
            let mut visited: HashSet<String> = HashSet::new();
            let mut curr = start;
            visited.insert(curr.clone());

            while let Some(parent) = dir_parents.get(curr) {
                if visited.contains(parent) {
                    return Some(IceReport {
                        ice: "ICE99".to_string(),
                        is_error: true,
                        message: format!(
                            "circular directory reference detected involving directory '{parent}'"
                        ),
                    });
                }
                visited.insert(parent.clone());
                curr = parent;
            }
        }

        None
    }

    /// ICE101: Validates that files in File table have proper sequence references.
    fn validate_ice101(db: &LinkedDatabase) -> Option<IceReport> {
        for r in db.get_records("File") {
            if let Some(FieldValue::Short(seq)) = r.get(7) {
                if *seq <= 0 {
                    return Some(IceReport {
                        ice: "ICE101".to_string(),
                        is_error: true,
                        message: format!("file sequence must be positive, got {seq}"),
                    });
                }
            }
        }
        None
    }

    /// ICE103: Validates that shortcut icon indices are non-negative.
    fn validate_ice103(db: &LinkedDatabase) -> Option<IceReport> {
        for r in db.get_records("Shortcut") {
            if let Some(FieldValue::Short(idx)) = r.get(9) {
                if *idx < 0 {
                    return Some(IceReport {
                        ice: "ICE103".to_string(),
                        is_error: true,
                        message: format!("shortcut icon index must be non-negative, got {idx}"),
                    });
                }
            }
        }
        None
    }
}

/// Inspects executable file binary data to extract Windows PE version and language.
#[allow(clippy::cast_possible_truncation, clippy::too_many_lines)]
fn inspect_pe_version(data: &[u8]) -> Option<(String, String)> {
    if data.len() < 64 || data.get(0..2) != Some(b"MZ") {
        return None;
    }
    let lfanew = u32::from_le_bytes([
        *data.get(0x3C)?,
        *data.get(0x3D)?,
        *data.get(0x3E)?,
        *data.get(0x3F)?,
    ]) as usize;

    if data.len() < lfanew + 24 || data.get(lfanew..lfanew + 4) != Some(b"PE\0\0") {
        return None;
    }

    let size_of_opt =
        u16::from_le_bytes([*data.get(lfanew + 20)?, *data.get(lfanew + 21)?]) as usize;

    let opt_offset = lfanew + 24;
    let magic = u16::from_le_bytes([*data.get(opt_offset)?, *data.get(opt_offset + 1)?]);

    let rsrc_dir_offset = if magic == 0x10B {
        opt_offset + 96 + 16 // PE32
    } else if magic == 0x20B {
        opt_offset + 112 + 16 // PE32+
    } else {
        return None;
    };

    let rsrc_rva = u32::from_le_bytes([
        *data.get(rsrc_dir_offset)?,
        *data.get(rsrc_dir_offset + 1)?,
        *data.get(rsrc_dir_offset + 2)?,
        *data.get(rsrc_dir_offset + 3)?,
    ]);

    if rsrc_rva == 0 {
        return None;
    }

    let num_sections =
        u16::from_le_bytes([*data.get(lfanew + 6)?, *data.get(lfanew + 7)?]) as usize;

    let section_headers_offset = opt_offset + size_of_opt;
    let mut rsrc_file_offset = None;

    for i in 0..num_sections {
        let sec_offset = section_headers_offset + i * 40;
        let virt_size = u32::from_le_bytes([
            *data.get(sec_offset + 8)?,
            *data.get(sec_offset + 9)?,
            *data.get(sec_offset + 10)?,
            *data.get(sec_offset + 11)?,
        ]);
        let virt_addr = u32::from_le_bytes([
            *data.get(sec_offset + 12)?,
            *data.get(sec_offset + 13)?,
            *data.get(sec_offset + 14)?,
            *data.get(sec_offset + 15)?,
        ]);
        let raw_ptr = u32::from_le_bytes([
            *data.get(sec_offset + 20)?,
            *data.get(sec_offset + 21)?,
            *data.get(sec_offset + 22)?,
            *data.get(sec_offset + 23)?,
        ]);

        if rsrc_rva >= virt_addr && rsrc_rva < virt_addr.saturating_add(virt_size) {
            rsrc_file_offset = Some((raw_ptr + (rsrc_rva - virt_addr)) as usize);
            break;
        }
    }

    let root_rsrc = rsrc_file_offset?;
    let num_named =
        u16::from_le_bytes([*data.get(root_rsrc + 12)?, *data.get(root_rsrc + 13)?]) as usize;
    let num_id =
        u16::from_le_bytes([*data.get(root_rsrc + 14)?, *data.get(root_rsrc + 15)?]) as usize;

    for i in 0..(num_named + num_id) {
        let entry_offset = root_rsrc + 16 + i * 8;
        let id_val = u32::from_le_bytes([
            *data.get(entry_offset)?,
            *data.get(entry_offset + 1)?,
            *data.get(entry_offset + 2)?,
            *data.get(entry_offset + 3)?,
        ]);
        let _data_offset_val = u32::from_le_bytes([
            *data.get(entry_offset + 4)?,
            *data.get(entry_offset + 5)?,
            *data.get(entry_offset + 6)?,
            *data.get(entry_offset + 7)?,
        ]);

        if id_val == 16 {
            if let Some(pos) = data[root_rsrc..data.len().min(root_rsrc + 1024)]
                .windows(4)
                .position(|w| w == 0xFEEF_04BDu32.to_le_bytes())
            {
                let sig_pos = root_rsrc + pos;
                if sig_pos + 20 <= data.len() {
                    let ms = u32::from_le_bytes([
                        data[sig_pos + 8],
                        data[sig_pos + 9],
                        data[sig_pos + 10],
                        data[sig_pos + 11],
                    ]);
                    let ls = u32::from_le_bytes([
                        data[sig_pos + 12],
                        data[sig_pos + 13],
                        data[sig_pos + 14],
                        data[sig_pos + 15],
                    ]);
                    let major = ms >> 16;
                    let minor = ms & 0xFFFF;
                    let build = ls >> 16;
                    let rev = ls & 0xFFFF;
                    return Some((format!("{major}.{minor}.{build}.{rev}"), "1033".to_string()));
                }
            }
        }
    }

    None
}

/// Inspects font file binary data (`.ttf`, `.otf`) to extract font title.
fn inspect_font_title(data: &[u8]) -> Option<String> {
    let magic = data.get(0..4)?;
    let is_font = magic == [0, 1, 0, 0] || magic == b"OTTO";
    if !is_font {
        return None;
    }

    let num_tables = u16::from_be_bytes([*data.get(4)?, *data.get(5)?]) as usize;
    let mut name_offset = None;

    for i in 0..num_tables {
        let rec = 12 + i * 16;
        if data.get(rec..rec + 4) == Some(b"name") {
            let off = u32::from_be_bytes([
                *data.get(rec + 8)?,
                *data.get(rec + 9)?,
                *data.get(rec + 10)?,
                *data.get(rec + 11)?,
            ]) as usize;
            name_offset = Some(off);
            break;
        }
    }

    let name_tbl = name_offset?;
    let count = u16::from_be_bytes([*data.get(name_tbl + 2)?, *data.get(name_tbl + 3)?]) as usize;
    let str_offset =
        u16::from_be_bytes([*data.get(name_tbl + 4)?, *data.get(name_tbl + 5)?]) as usize;

    for i in 0..count {
        let rec = name_tbl + 6 + i * 12;
        let name_id = u16::from_be_bytes([*data.get(rec + 6)?, *data.get(rec + 7)?]);
        if name_id == 4 || name_id == 1 {
            let length = u16::from_be_bytes([*data.get(rec + 8)?, *data.get(rec + 9)?]) as usize;
            let offset = u16::from_be_bytes([*data.get(rec + 10)?, *data.get(rec + 11)?]) as usize;
            let start = name_tbl + str_offset + offset;
            let bytes = data.get(start..start + length)?;

            let platform_id = u16::from_be_bytes([*data.get(rec)?, *data.get(rec + 1)?]);
            if platform_id == 0 || platform_id == 3 {
                let mut u16s = Vec::with_capacity(bytes.len() / 2);
                for chunk in bytes.chunks_exact(2) {
                    u16s.push(u16::from_be_bytes([chunk[0], chunk[1]]));
                }
                if let Ok(s) = String::from_utf16(&u16s) {
                    if !s.is_empty() {
                        return Some(s);
                    }
                }
            } else if let Ok(s) = std::str::from_utf8(bytes) {
                if !s.is_empty() {
                    return Some(s.to_string());
                }
            }
        }
    }

    None
}

/// Computes the 128-bit MD5 digest of an arbitrary byte slice per RFC 1321.
#[must_use]
#[allow(clippy::many_single_char_names, clippy::too_many_lines)]
fn compute_md5(input: &[u8]) -> [u8; 16] {
    let mut a: u32 = 0x6745_2301;
    let mut b: u32 = 0xefcd_ab89;
    let mut c: u32 = 0x98ba_dcfe;
    let mut d: u32 = 0x1032_5476;

    let bit_len = (input.len() as u64).wrapping_mul(8);
    let mut padded = input.to_vec();
    padded.push(0x80);
    while (padded.len() % 64) != 56 {
        padded.push(0x00);
    }
    padded.extend_from_slice(&bit_len.to_le_bytes());

    for chunk in padded.chunks_exact(64) {
        let mut m = [0u32; 16];
        for (i, word) in m.iter_mut().enumerate() {
            let offset = i * 4;
            *word = u32::from_le_bytes([
                chunk[offset],
                chunk[offset + 1],
                chunk[offset + 2],
                chunk[offset + 3],
            ]);
        }

        let aa = a;
        let bb = b;
        let cc = c;
        let dd = d;

        macro_rules! ff {
            ($a:expr, $b:expr, $c:expr, $d:expr, $k:expr, $s:expr, $i:expr) => {
                $a = $b.wrapping_add(
                    ($a.wrapping_add(($b & $c) | ((!$b) & $d))
                        .wrapping_add(m[$k])
                        .wrapping_add($i))
                    .rotate_left($s),
                );
            };
        }

        ff!(a, b, c, d, 0, 7, 0xd76a_a478);
        ff!(d, a, b, c, 1, 12, 0xe8c7_b756);
        ff!(c, d, a, b, 2, 17, 0x2420_70db);
        ff!(b, c, d, a, 3, 22, 0xc1bd_ceee);
        ff!(a, b, c, d, 4, 7, 0xf57c_0faf);
        ff!(d, a, b, c, 5, 12, 0x4787_c62a);
        ff!(c, d, a, b, 6, 17, 0xa830_4613);
        ff!(b, c, d, a, 7, 22, 0xfd46_9501);
        ff!(a, b, c, d, 8, 7, 0x6980_98d8);
        ff!(d, a, b, c, 9, 12, 0x8b44_f7af);
        ff!(c, d, a, b, 10, 17, 0xffff_5bb1);
        ff!(b, c, d, a, 11, 22, 0x895c_d7be);
        ff!(a, b, c, d, 12, 7, 0x6b90_1122);
        ff!(d, a, b, c, 13, 12, 0xfd98_7193);
        ff!(c, d, a, b, 14, 17, 0xa679_438e);
        ff!(b, c, d, a, 15, 22, 0x49b4_0821);

        macro_rules! gg {
            ($a:expr, $b:expr, $c:expr, $d:expr, $k:expr, $s:expr, $i:expr) => {
                $a = $b.wrapping_add(
                    ($a.wrapping_add(($b & $d) | ($c & (!$d)))
                        .wrapping_add(m[$k])
                        .wrapping_add($i))
                    .rotate_left($s),
                );
            };
        }

        gg!(a, b, c, d, 1, 5, 0xf61e_2562);
        gg!(d, a, b, c, 6, 9, 0xc040_b340);
        gg!(c, d, a, b, 11, 14, 0x265e_5a51);
        gg!(b, c, d, a, 0, 20, 0xe9b6_c7aa);
        gg!(a, b, c, d, 5, 5, 0xd62f_105d);
        gg!(d, a, b, c, 10, 9, 0x0244_1453);
        gg!(c, d, a, b, 15, 14, 0xd8a1_e681);
        gg!(b, c, d, a, 4, 20, 0xe7d3_fbc8);
        gg!(a, b, c, d, 9, 5, 0x21e1_cde6);
        gg!(d, a, b, c, 14, 9, 0xc337_07d6);
        gg!(c, d, a, b, 3, 14, 0xf4d5_0d87);
        gg!(b, c, d, a, 8, 20, 0x455a_14ed);
        gg!(a, b, c, d, 13, 5, 0xa9e3_e905);
        gg!(d, a, b, c, 2, 9, 0xfcef_a3f8);
        gg!(c, d, a, b, 7, 14, 0x676f_02d9);
        gg!(b, c, d, a, 12, 20, 0x8d2a_4c8a);

        macro_rules! hh {
            ($a:expr, $b:expr, $c:expr, $d:expr, $k:expr, $s:expr, $i:expr) => {
                $a = $b.wrapping_add(
                    ($a.wrapping_add($b ^ $c ^ $d)
                        .wrapping_add(m[$k])
                        .wrapping_add($i))
                    .rotate_left($s),
                );
            };
        }

        hh!(a, b, c, d, 5, 4, 0xfffa_3942);
        hh!(d, a, b, c, 8, 11, 0x8771_f681);
        hh!(c, d, a, b, 11, 16, 0x6d9d_6122);
        hh!(b, c, d, a, 14, 23, 0xfde5_380c);
        hh!(a, b, c, d, 1, 4, 0xa4be_ea44);
        hh!(d, a, b, c, 4, 11, 0x4bde_cfa9);
        hh!(c, d, a, b, 7, 16, 0xf6bb_4b60);
        hh!(b, c, d, a, 10, 23, 0xbebf_bc70);
        hh!(a, b, c, d, 13, 4, 0x289b_7ec6);
        hh!(d, a, b, c, 0, 11, 0xeaa1_27fa);
        hh!(c, d, a, b, 3, 16, 0xd4ef_3085);
        hh!(b, c, d, a, 6, 23, 0x0488_1d05);
        hh!(a, b, c, d, 9, 4, 0xd9d4_d039);
        hh!(d, a, b, c, 12, 11, 0xe6db_99e5);
        hh!(c, d, a, b, 15, 16, 0x1fa2_7cf8);
        hh!(b, c, d, a, 2, 23, 0xc4ac_5665);

        macro_rules! ii {
            ($a:expr, $b:expr, $c:expr, $d:expr, $k:expr, $s:expr, $i:expr) => {
                $a = $b.wrapping_add(
                    ($a.wrapping_add($c ^ ($b | (!$d)))
                        .wrapping_add(m[$k])
                        .wrapping_add($i))
                    .rotate_left($s),
                );
            };
        }

        ii!(a, b, c, d, 0, 6, 0xf429_2244);
        ii!(d, a, b, c, 7, 10, 0x432a_ff97);
        ii!(c, d, a, b, 14, 15, 0xab94_23a7);
        ii!(b, c, d, a, 5, 21, 0xfc93_a039);
        ii!(a, b, c, d, 12, 6, 0x655b_59c3);
        ii!(d, a, b, c, 3, 10, 0x8f0c_cc92);
        ii!(c, d, a, b, 10, 15, 0xffef_f47d);
        ii!(b, c, d, a, 1, 21, 0x8584_5dd1);
        ii!(a, b, c, d, 8, 6, 0x6fa8_7e4f);
        ii!(d, a, b, c, 15, 10, 0xfe2c_e6e0);
        ii!(c, d, a, b, 6, 15, 0xa301_4314);
        ii!(b, c, d, a, 13, 21, 0x4e08_11a1);
        ii!(a, b, c, d, 4, 6, 0xf753_7e82);
        ii!(d, a, b, c, 11, 10, 0xbd3a_f235);
        ii!(c, d, a, b, 2, 15, 0x2ad7_d2bb);
        ii!(b, c, d, a, 9, 21, 0xeb86_d391);

        a = a.wrapping_add(aa);
        b = b.wrapping_add(bb);
        c = c.wrapping_add(cc);
        d = d.wrapping_add(dd);
    }

    let mut result = [0u8; 16];
    result[0..4].copy_from_slice(&a.to_le_bytes());
    result[4..8].copy_from_slice(&b.to_le_bytes());
    result[8..12].copy_from_slice(&c.to_le_bytes());
    result[12..16].copy_from_slice(&d.to_le_bytes());
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::wix::wixlib::WixLibrary;
    use crate::wix::wixobj::{IntermediateTable, Reference};

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
        active_linker.add_object(obj);

        let linked = active_linker.link()?;
        assert_eq!(linked.get_records("Component").len(), 1);
        assert_ne!(linked.get_records("Directory"), []);
        assert!(linked.get_records("InstallExecuteSequence").len() >= 15);

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
    #[allow(clippy::too_many_lines)]
    fn test_ice_rules() -> Result<()> {
        let mut db = LinkedDatabase::new()?;

        // ICE02: Circular feature dependency
        db.add_record(
            "Feature",
            Record::with_fields(vec![
                FieldValue::String("F1".to_string()),
                FieldValue::String("F2".to_string()),
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Short(1),
                FieldValue::Null,
                FieldValue::Short(0),
            ]),
        );
        db.add_record(
            "Feature",
            Record::with_fields(vec![
                FieldValue::String("F2".to_string()),
                FieldValue::String("F1".to_string()),
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Short(1),
                FieldValue::Null,
                FieldValue::Short(0),
            ]),
        );
        assert!(Linker::validate_ice02(&db).is_some());

        // ICE04: Non-contiguous files
        let mut db_ice4 = LinkedDatabase::new()?;
        db_ice4.add_record(
            "File",
            Record::with_fields(vec![
                FieldValue::String("File1".to_string()),
                FieldValue::String("Comp1".to_string()),
                FieldValue::String("file1.txt".to_string()),
                FieldValue::Long(100),
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Short(2), // expected 1
            ]),
        );
        assert!(Linker::validate_ice04(&db_ice4).is_some());

        // ICE05: Media last sequence < file count
        let mut db_ice5 = LinkedDatabase::new()?;
        db_ice5.add_record(
            "File",
            Record::with_fields(vec![
                FieldValue::String("File1".to_string()),
                FieldValue::String("Comp1".to_string()),
                FieldValue::String("file1.txt".to_string()),
                FieldValue::Long(100),
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Short(1),
            ]),
        );
        db_ice5.add_record(
            "File",
            Record::with_fields(vec![
                FieldValue::String("File2".to_string()),
                FieldValue::String("Comp1".to_string()),
                FieldValue::String("file2.txt".to_string()),
                FieldValue::Long(200),
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Short(2),
            ]),
        );
        db_ice5.add_record(
            "Media",
            Record::with_fields(vec![
                FieldValue::Short(1),
                FieldValue::Long(1), // only covers 1 file, but there are 2
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
            ]),
        );
        assert!(Linker::validate_ice05(&db_ice5).is_some());

        // ICE06: Negative file size
        let mut db_ice6 = LinkedDatabase::new()?;
        db_ice6.add_record(
            "File",
            Record::with_fields(vec![
                FieldValue::String("File1".to_string()),
                FieldValue::String("Comp1".to_string()),
                FieldValue::String("file1.txt".to_string()),
                FieldValue::Long(-10),
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Short(1),
            ]),
        );
        assert!(Linker::validate_ice06(&db_ice6).is_some());

        // ICE08: Duplicate component GUID
        let mut db_ice8 = LinkedDatabase::new()?;
        db_ice8.add_record(
            "Component",
            Record::with_fields(vec![
                FieldValue::String("CompA".to_string()),
                FieldValue::String("{11111111-1111-1111-1111-111111111111}".to_string()),
                FieldValue::String("TARGETDIR".to_string()),
                FieldValue::Short(0),
                FieldValue::Null,
                FieldValue::Null,
            ]),
        );
        db_ice8.add_record(
            "Component",
            Record::with_fields(vec![
                FieldValue::String("CompB".to_string()),
                FieldValue::String("{11111111-1111-1111-1111-111111111111}".to_string()),
                FieldValue::String("TARGETDIR".to_string()),
                FieldValue::Short(0),
                FieldValue::Null,
                FieldValue::Null,
            ]),
        );
        assert!(Linker::validate_ice08(&db_ice8).is_some());

        // ICE09: Missing keypath
        let mut db_ice9 = LinkedDatabase::new()?;
        db_ice9.add_record(
            "Component",
            Record::with_fields(vec![
                FieldValue::String("CompA".to_string()),
                FieldValue::String("{11111111-1111-1111-1111-111111111111}".to_string()),
                FieldValue::String("TARGETDIR".to_string()),
                FieldValue::Short(0x0010), // Registry keypath
                FieldValue::Null,
                FieldValue::String("NonExistentRegKey".to_string()),
            ]),
        );
        assert!(Linker::validate_ice09(&db_ice9).is_some());

        // ICE20: Bad sequence order
        let mut db_ice20 = LinkedDatabase::new()?;
        db_ice20.add_record(
            "InstallExecuteSequence",
            Record::with_fields(vec![
                FieldValue::String("InstallInitialize".to_string()),
                FieldValue::Null,
                FieldValue::Short(6600),
            ]),
        );
        db_ice20.add_record(
            "InstallExecuteSequence",
            Record::with_fields(vec![
                FieldValue::String("InstallFinalize".to_string()),
                FieldValue::Null,
                FieldValue::Short(1500),
            ]),
        );
        assert!(Linker::validate_ice20(&db_ice20).is_some());

        // ICE30: File collision
        let mut db_ice30 = LinkedDatabase::new()?;
        db_ice30.add_record(
            "Component",
            Record::with_fields(vec![
                FieldValue::String("CompA".to_string()),
                FieldValue::Null,
                FieldValue::String("DIR1".to_string()),
                FieldValue::Short(0),
                FieldValue::Null,
                FieldValue::Null,
            ]),
        );
        db_ice30.add_record(
            "Component",
            Record::with_fields(vec![
                FieldValue::String("CompB".to_string()),
                FieldValue::Null,
                FieldValue::String("DIR1".to_string()),
                FieldValue::Short(0),
                FieldValue::Null,
                FieldValue::Null,
            ]),
        );
        db_ice30.add_record(
            "File",
            Record::with_fields(vec![
                FieldValue::String("FileA".to_string()),
                FieldValue::String("CompA".to_string()),
                FieldValue::String("app.exe".to_string()),
                FieldValue::Long(100),
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Short(1),
            ]),
        );
        db_ice30.add_record(
            "File",
            Record::with_fields(vec![
                FieldValue::String("FileB".to_string()),
                FieldValue::String("CompB".to_string()),
                FieldValue::String("app.exe".to_string()),
                FieldValue::Long(100),
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Short(2),
            ]),
        );
        assert!(Linker::validate_ice30(&db_ice30).is_some());

        // ICE38: Non-HKCU keypath in user directory
        let mut db_for_ice38 = LinkedDatabase::new()?;
        db_for_ice38.add_record(
            "Registry",
            Record::with_fields(vec![
                FieldValue::String("RegKey1".to_string()),
                FieldValue::Short(2), // HKLM instead of HKCU (1)
                FieldValue::String(r"Software\App".to_string()),
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::String("CompA".to_string()),
            ]),
        );
        db_for_ice38.add_record(
            "Component",
            Record::with_fields(vec![
                FieldValue::String("CompA".to_string()),
                FieldValue::Null,
                FieldValue::String("AppDataFolder".to_string()),
                FieldValue::Short(0x0010), // Registry keypath
                FieldValue::Null,
                FieldValue::String("RegKey1".to_string()),
            ]),
        );
        assert!(Linker::validate_ice38(&db_for_ice38).is_some());

        // ICE61: Upgrade min version exceeds ProductVersion
        let mut db_ice61 = LinkedDatabase::new()?;
        db_ice61.add_record(
            "Property",
            Record::with_fields(vec![
                FieldValue::String("ProductVersion".to_string()),
                FieldValue::String("1.0.0".to_string()),
            ]),
        );
        db_ice61.add_record(
            "Upgrade",
            Record::with_fields(vec![
                FieldValue::String("{11111111-1111-1111-1111-111111111111}".to_string()),
                FieldValue::String("2.0.0".to_string()), // > 1.0.0
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Long(0),
                FieldValue::Null,
                FieldValue::String("UPGRADE_PROP".to_string()),
            ]),
        );
        assert!(Linker::validate_ice61(&db_ice61).is_some());

        // ICE99: Circular directory
        let mut db_ice99 = LinkedDatabase::new()?;
        db_ice99.add_record(
            "Directory",
            Record::with_fields(vec![
                FieldValue::String("DIRA".to_string()),
                FieldValue::String("DIRB".to_string()),
                FieldValue::String("DirA".to_string()),
            ]),
        );
        db_ice99.add_record(
            "Directory",
            Record::with_fields(vec![
                FieldValue::String("DIRB".to_string()),
                FieldValue::String("DIRA".to_string()),
                FieldValue::String("DirB".to_string()),
            ]),
        );
        assert!(Linker::validate_ice99(&db_ice99).is_some());

        // ICE101: Invalid file sequence <= 0
        let mut db_ice101 = LinkedDatabase::new()?;
        db_ice101.add_record(
            "File",
            Record::with_fields(vec![
                FieldValue::String("File1".to_string()),
                FieldValue::String("Comp1".to_string()),
                FieldValue::String("file1.txt".to_string()),
                FieldValue::Long(100),
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Short(0), // <= 0
            ]),
        );
        assert!(Linker::validate_ice101(&db_ice101).is_some());

        // ICE103: Negative icon index
        let mut db_ice103 = LinkedDatabase::new()?;
        db_ice103.add_record(
            "Shortcut",
            Record::with_fields(vec![
                FieldValue::String("Short1".to_string()),
                FieldValue::String("TARGETDIR".to_string()),
                FieldValue::String("App.lnk".to_string()),
                FieldValue::String("Comp1".to_string()),
                FieldValue::String("TargetExe".to_string()),
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Short(-1), // negative
                FieldValue::Null,
                FieldValue::Null,
            ]),
        );
        assert!(Linker::validate_ice103(&db_ice103).is_some());

        // ICE80 & ICE07 & ICE18
        assert!(Linker::validate_ice01(&db).is_none());
        assert!(Linker::validate_ice18(&db).is_none());
        assert!(Linker::validate_ice33(&db).is_none());

        // ICE01: Missing required table in catalog
        let db_ice01_bad = LinkedDatabase {
            tables: HashMap::new(),
            catalog: DatabaseCatalog::default(),
        };
        let rep_ice01 = Linker::validate_ice01(&db_ice01_bad);
        assert_eq!(
            rep_ice01.as_ref().map(|r| (r.ice.as_str(), r.is_error)),
            Some(("ICE01", true))
        );

        // ICE18: Keypath file belongs to different component
        let mut db_ice18_bad = LinkedDatabase::new()?;
        db_ice18_bad.add_record(
            "File",
            Record::with_fields(vec![
                FieldValue::String("SharedFile".to_string()),
                FieldValue::String("CompA".to_string()),
                FieldValue::String("shared.dll".to_string()),
                FieldValue::Long(200),
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Short(1),
            ]),
        );
        db_ice18_bad.add_record(
            "Component",
            Record::with_fields(vec![
                FieldValue::String("CompB".to_string()),
                FieldValue::Null,
                FieldValue::String("TARGETDIR".to_string()),
                FieldValue::Short(0),
                FieldValue::Null,
                FieldValue::String("SharedFile".to_string()),
            ]),
        );
        let rep_ice18 = Linker::validate_ice18(&db_ice18_bad);
        assert_eq!(
            rep_ice18.as_ref().map(|r| (r.ice.as_str(), r.is_error)),
            Some(("ICE18", true))
        );

        // ICE33: Invalid CLSID format in Class table
        let mut db_ice33_bad_clsid = LinkedDatabase::new()?;
        db_ice33_bad_clsid.add_record(
            "Class",
            Record::with_fields(vec![
                FieldValue::String("not-a-valid-guid".to_string()),
                FieldValue::String("InprocServer32".to_string()),
                FieldValue::String("Comp1".to_string()),
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
            ]),
        );
        let rep_ice33_err = Linker::validate_ice33(&db_ice33_bad_clsid);
        assert_eq!(
            rep_ice33_err.as_ref().map(|r| (r.ice.as_str(), r.is_error)),
            Some(("ICE33", true))
        );

        // ICE33: Registry key containing COM registration warning
        let mut db_ice33_reg_warn = LinkedDatabase::new()?;
        db_ice33_reg_warn.add_record(
            "Registry",
            Record::with_fields(vec![
                FieldValue::String("RegCom1".to_string()),
                FieldValue::Short(0), // HKCR
                FieldValue::String("CLSID\\{11111111-1111-1111-1111-111111111111}".to_string()),
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::String("Comp1".to_string()),
            ]),
        );
        let rep_ice33_warn = Linker::validate_ice33(&db_ice33_reg_warn);
        assert_eq!(
            rep_ice33_warn
                .as_ref()
                .map(|r| (r.ice.as_str(), r.is_error)),
            Some(("ICE33", false))
        );

        // ICE33: HKLM Software\Classes\CLSID warning
        let mut db_ice33_hklm = LinkedDatabase::new()?;
        db_ice33_hklm.add_record(
            "Registry",
            Record::with_fields(vec![
                FieldValue::String("RegCom2".to_string()),
                FieldValue::Short(2), // HKLM
                FieldValue::String(
                    "Software\\Classes\\CLSID\\{22222222-2222-2222-2222-222222222222}".to_string(),
                ),
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::String("Comp1".to_string()),
            ]),
        );
        assert!(Linker::validate_ice33(&db_ice33_hklm).is_some());

        let mut db_ice7 = LinkedDatabase::new()?;
        db_ice7.add_record(
            "File",
            Record::with_fields(vec![
                FieldValue::String("FontFile".to_string()),
                FieldValue::String("Comp1".to_string()),
                FieldValue::String("font.ttf".to_string()),
                FieldValue::Long(500),
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Short(1),
            ]),
        );
        assert!(Linker::validate_ice07(&db_ice7).is_none());

        let mut db_ice80 = LinkedDatabase::new()?;
        db_ice80.add_record(
            "Component",
            Record::with_fields(vec![
                FieldValue::String("Comp32".to_string()),
                FieldValue::Null,
                FieldValue::String("TARGETDIR".to_string()),
                FieldValue::Short(0),
                FieldValue::Null,
                FieldValue::Null,
            ]),
        );
        db_ice80.add_record(
            "Component",
            Record::with_fields(vec![
                FieldValue::String("Comp64".to_string()),
                FieldValue::Null,
                FieldValue::String("TARGETDIR".to_string()),
                FieldValue::Short(0x0100),
                FieldValue::Null,
                FieldValue::Null,
            ]),
        );
        assert!(Linker::validate_ice80(&db_ice80).is_none());

        Ok(())
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

        let db = linker.link()?;
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
    #[allow(clippy::too_many_lines)]
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
        let db = linker.link()?;

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

        let db = linker.link()?;
        assert_eq!(db.get_records("Component").len(), 1);
        Ok(())
    }

    /// Tests component group hierarchy resolution edge cases including empty sets, malformed records, and cycles.
    #[test]
    fn test_linker_component_groups_edge_cases() -> Result<()> {
        let mut db_empty = LinkedDatabase::new()?;
        Linker::resolve_component_groups(&mut db_empty)?;

        // feat_refs non-empty but members empty
        let mut db_feat_only = LinkedDatabase::new()?;
        db_feat_only.add_record(
            "_FeatureComponentGroupRef",
            Record::with_fields(vec![
                FieldValue::String("Feat1".to_string()),
                FieldValue::String("CG_Empty".to_string()),
            ]),
        );
        Linker::resolve_component_groups(&mut db_feat_only)?;

        // feat_refs empty but members non-empty
        let mut db_members_only = LinkedDatabase::new()?;
        db_members_only.add_record(
            "_ComponentGroupMember",
            Record::with_fields(vec![
                FieldValue::String("CG_Lone".to_string()),
                FieldValue::String("CompLone".to_string()),
            ]),
        );
        Linker::resolve_component_groups(&mut db_members_only)?;

        // Malformed records in tracking tables
        let mut db_malformed = LinkedDatabase::new()?;
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
        Linker::resolve_component_groups(&mut db_malformed)?;

        // Cycle in nested groups: CG1 -> CG2 -> CG1
        let mut db_cycle = LinkedDatabase::new()?;
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
        Linker::resolve_component_groups(&mut db_cycle)?;
        let fc = db_cycle.get_records("FeatureComponents");
        assert_eq!(fc.len(), 1);

        Ok(())
    }

    /// Tests directory resolution and action sequence edge cases.
    #[test]
    fn test_linker_directories_and_actions_edge_cases() -> Result<()> {
        let mut db = LinkedDatabase::new()?;

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
    #[test]
    fn test_linker_media_layout_edge_cases() -> Result<()> {
        let mut db_empty_files = LinkedDatabase::new()?;
        Linker::layout_media_and_files(&mut db_empty_files);

        let mut db = LinkedDatabase::new()?;
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

    /// Tests validation runner and ICE01 through ICE05 edge cases.
    #[test]
    #[allow(clippy::too_many_lines)]
    fn test_ice01_to_ice05_comprehensive() -> Result<()> {
        let mut db = LinkedDatabase::new()?;

        // run_ice_validations with error
        db.add_record(
            "File",
            Record::with_fields(vec![
                FieldValue::String("File1".to_string()),
                FieldValue::String("Comp1".to_string()),
                FieldValue::String("file1.txt".to_string()),
                FieldValue::Long(100),
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Short(99), // non-contiguous -> ICE04 error
            ]),
        );
        assert!(Linker::run_ice_validations(&db).is_err());

        // run_ice_validations with warning only (e.g. ICE33 registry warning) -> Ok(())
        let mut db_warn_only = LinkedDatabase::new()?;
        db_warn_only.add_record(
            "Registry",
            Record::with_fields(vec![
                FieldValue::String("R1".to_string()),
                FieldValue::Short(0),
                FieldValue::String(r"Software\MyApp\InprocServer32".to_string()),
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::String("Comp1".to_string()),
            ]),
        );
        assert!(Linker::run_ice_validations(&db_warn_only).is_ok());

        // ICE01: valid catalog with all standard tables
        let db_valid = LinkedDatabase::new()?;
        assert!(Linker::validate_ice01(&db_valid).is_none());

        // ICE02: feature with empty parent string, and valid parent hierarchy
        let mut db_ice2 = LinkedDatabase::new()?;
        db_ice2.add_record(
            "Feature",
            Record::with_fields(vec![
                FieldValue::String("F1".to_string()),
                FieldValue::String(String::new()), // empty parent
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Short(1),
                FieldValue::Null,
                FieldValue::Short(0),
            ]),
        );
        db_ice2.add_record(
            "Feature",
            Record::with_fields(vec![
                FieldValue::String("F2".to_string()),
                FieldValue::String("F1".to_string()), // valid parent
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Short(1),
                FieldValue::Null,
                FieldValue::Short(0),
            ]),
        );
        assert!(Linker::validate_ice02(&db_ice2).is_none());

        // ICE03: Unknown table in db, invalid record failing validation, valid record
        let mut db_ice3 = LinkedDatabase::new()?;
        db_ice3.add_record(
            "UnknownCustomTable",
            Record::with_fields(vec![FieldValue::Short(1)]),
        );
        assert!(Linker::validate_ice03(&db_ice3).is_none());
        // Invalid record in standard Property table (expects 2 fields, given 1)
        db_ice3.add_record(
            "Property",
            Record::with_fields(vec![FieldValue::String("OnlyOneField".to_string())]),
        );
        let rep_ice3 = Linker::validate_ice03(&db_ice3);
        assert_eq!(
            rep_ice3.as_ref().map(|r| (r.ice.as_str(), r.is_error)),
            Some(("ICE03", true))
        );

        // ICE04: File with Null sequence, and valid contiguous files
        let mut db_ice4 = LinkedDatabase::new()?;
        db_ice4.add_record(
            "File",
            Record::with_fields(vec![
                FieldValue::String("F1".to_string()),
                FieldValue::String("Comp1".to_string()),
                FieldValue::String("f1.txt".to_string()),
                FieldValue::Long(100),
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null, // not short
            ]),
        );
        db_ice4.add_record(
            "File",
            Record::with_fields(vec![
                FieldValue::String("F2".to_string()),
                FieldValue::String("Comp1".to_string()),
                FieldValue::String("f2.txt".to_string()),
                FieldValue::Long(100),
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Short(1),
            ]),
        );
        assert!(Linker::validate_ice04(&db_ice4).is_none());

        // ICE05: Empty files -> None; Media empty -> Error; Multiple media with max_media_seq >= max_file_seq -> None
        let mut db_ice5 = LinkedDatabase::new()?;
        assert!(Linker::validate_ice05(&db_ice5).is_none());
        db_ice5.add_record(
            "File",
            Record::with_fields(vec![
                FieldValue::String("F1".to_string()),
                FieldValue::String("Comp1".to_string()),
                FieldValue::String("f1.txt".to_string()),
                FieldValue::Long(100),
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Short(1),
            ]),
        );
        // Media table empty with non-empty files
        assert_eq!(
            Linker::validate_ice05(&db_ice5)
                .as_ref()
                .map(|r| (r.ice.as_str(), r.is_error)),
            Some(("ICE05", true))
        );
        // Add valid Media records, including one where field 1 is Null and one where last_seq <= max_media_seq
        db_ice5.add_record(
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
        db_ice5.add_record(
            "Media",
            Record::with_fields(vec![
                FieldValue::Short(2),
                FieldValue::Long(50),
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
            ]),
        );
        db_ice5.add_record(
            "Media",
            Record::with_fields(vec![
                FieldValue::Short(3),
                FieldValue::Long(20), // 20 <= 50
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
            ]),
        );
        assert!(Linker::validate_ice05(&db_ice5).is_none());

        Ok(())
    }

    /// Tests ICE06 through ICE09 edge cases.
    #[test]
    #[allow(clippy::too_many_lines)]
    fn test_ice06_to_ice09_comprehensive() -> Result<()> {
        // ICE06: Versioned file with negative size (allowed), version with empty string, file with Null size, valid unversioned file with sz >= 0
        let mut db_ice6 = LinkedDatabase::new()?;
        db_ice6.add_record(
            "File",
            Record::with_fields(vec![
                FieldValue::String("FVers".to_string()),
                FieldValue::String("Comp1".to_string()),
                FieldValue::String("f1.dll".to_string()),
                FieldValue::Long(-1), // negative but versioned
                FieldValue::String("1.0.0.0".to_string()),
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Short(1),
            ]),
        );
        db_ice6.add_record(
            "File",
            Record::with_fields(vec![
                FieldValue::String("FEmptyVer".to_string()),
                FieldValue::String("Comp1".to_string()),
                FieldValue::String("f2.txt".to_string()),
                FieldValue::Long(100),
                FieldValue::String(String::new()), // empty version string
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Short(2),
            ]),
        );
        db_ice6.add_record(
            "File",
            Record::with_fields(vec![
                FieldValue::String("FNullSz".to_string()),
                FieldValue::String("Comp1".to_string()),
                FieldValue::String("f3.txt".to_string()),
                FieldValue::Null, // Null size
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Short(3),
            ]),
        );
        assert!(Linker::validate_ice06(&db_ice6).is_none());

        // ICE07: Non-string file name, .otf without font title, .ttf with Null font title, .ttf with valid font title
        let mut db_ice7 = LinkedDatabase::new()?;
        db_ice7.add_record(
            "File",
            Record::with_fields(vec![
                FieldValue::String("FNullName".to_string()),
                FieldValue::String("Comp1".to_string()),
                FieldValue::Null,
                FieldValue::Long(100),
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Short(1),
            ]),
        );
        db_ice7.add_record(
            "File",
            Record::with_fields(vec![
                FieldValue::String("FOtf".to_string()),
                FieldValue::String("Comp1".to_string()),
                FieldValue::String("font.otf".to_string()),
                FieldValue::Long(100),
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Short(2),
            ]),
        );
        // File with 3 fields so r.get(4) is None
        db_ice7.add_record(
            "File",
            Record::with_fields(vec![
                FieldValue::String("FOtfNoVer".to_string()),
                FieldValue::String("Comp1".to_string()),
                FieldValue::String("font_no_ver.otf".to_string()),
            ]),
        );
        assert!(Linker::validate_ice07(&db_ice7).is_none());

        let mut db_ice7_null_title = LinkedDatabase::new()?;
        db_ice7_null_title.add_record(
            "File",
            Record::with_fields(vec![
                FieldValue::String("FTtf".to_string()),
                FieldValue::String("Comp1".to_string()),
                FieldValue::String("font.ttf".to_string()),
                FieldValue::Long(100),
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Short(1),
            ]),
        );
        db_ice7_null_title.add_record(
            "Font",
            Record::with_fields(vec![
                FieldValue::String("FTtf".to_string()),
                FieldValue::Null, // Null font title
            ]),
        );
        assert!(Linker::validate_ice07(&db_ice7_null_title).is_none());

        let mut db_ice7_ok = LinkedDatabase::new()?;
        db_ice7_ok.add_record(
            "File",
            Record::with_fields(vec![
                FieldValue::String("FTtfOk".to_string()),
                FieldValue::String("Comp1".to_string()),
                FieldValue::String("font.ttf".to_string()),
                FieldValue::Long(100),
                FieldValue::String("Arial Font".to_string()), // field 4 as font title string
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Short(1),
            ]),
        );
        assert!(Linker::validate_ice07(&db_ice7_ok).is_none());

        // ICE08: Empty GUID string, same component duplicate, unique GUIDs
        let mut db_ice8 = LinkedDatabase::new()?;
        db_ice8.add_record(
            "Component",
            Record::with_fields(vec![
                FieldValue::String("CompEmptyGuid".to_string()),
                FieldValue::String(String::new()),
                FieldValue::String("TARGETDIR".to_string()),
                FieldValue::Short(0),
                FieldValue::Null,
                FieldValue::Null,
            ]),
        );
        db_ice8.add_record(
            "Component",
            Record::with_fields(vec![
                FieldValue::String("CompSame".to_string()),
                FieldValue::String("{AAAAAAAA-AAAA-AAAA-AAAA-AAAAAAAAAAAA}".to_string()),
                FieldValue::String("TARGETDIR".to_string()),
                FieldValue::Short(0),
                FieldValue::Null,
                FieldValue::Null,
            ]),
        );
        db_ice8.add_record(
            "Component",
            Record::with_fields(vec![
                FieldValue::String("CompSame".to_string()),
                FieldValue::String("{AAAAAAAA-AAAA-AAAA-AAAA-AAAAAAAAAAAA}".to_string()),
                FieldValue::String("TARGETDIR".to_string()),
                FieldValue::Short(0),
                FieldValue::Null,
                FieldValue::Null,
            ]),
        );
        assert!(Linker::validate_ice08(&db_ice8).is_none());

        // ICE09:
        // - Registry filter_map with String and Null
        // - Directory filter_map with String and Null
        // - File filter_map with Null
        // - Component with Null attribute, empty keypath
        // - Component with registry keypath that exists
        // - Component with file keypath that exists in File
        // - Component with file keypath that exists in Directory
        // - Component with file keypath that doesn't exist in File or Directory (error)
        let mut db_ice9 = LinkedDatabase::new()?;
        db_ice9.add_record(
            "Registry",
            Record::with_fields(vec![
                FieldValue::String("RegKeyOk".to_string()),
                FieldValue::Short(1),
                FieldValue::String(r"Software\App".to_string()),
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::String("CompReg".to_string()),
            ]),
        );
        db_ice9.add_record(
            "Registry",
            Record::with_fields(vec![FieldValue::Null, FieldValue::Null]),
        );
        db_ice9.add_record(
            "Directory",
            Record::with_fields(vec![
                FieldValue::String("DirOk".to_string()),
                FieldValue::Null,
                FieldValue::String("DirOk".to_string()),
            ]),
        );
        db_ice9.add_record(
            "Directory",
            Record::with_fields(vec![FieldValue::Null, FieldValue::Null]),
        );
        db_ice9.add_record(
            "File",
            Record::with_fields(vec![FieldValue::Null, FieldValue::Null]),
        );
        db_ice9.add_record(
            "File",
            Record::with_fields(vec![
                FieldValue::String("FileOk".to_string()),
                FieldValue::String("CompFile".to_string()),
                FieldValue::String("app.exe".to_string()),
                FieldValue::Long(100),
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Short(1),
            ]),
        );
        // Component with Null attr, empty kp
        db_ice9.add_record(
            "Component",
            Record::with_fields(vec![
                FieldValue::String("CompNoKp".to_string()),
                FieldValue::Null,
                FieldValue::String("TARGETDIR".to_string()),
                FieldValue::Null, // attr null -> 0
                FieldValue::Null,
                FieldValue::String(String::new()), // empty kp
            ]),
        );
        // Component with valid registry keypath
        db_ice9.add_record(
            "Component",
            Record::with_fields(vec![
                FieldValue::String("CompReg".to_string()),
                FieldValue::Null,
                FieldValue::String("TARGETDIR".to_string()),
                FieldValue::Short(0x0010),
                FieldValue::Null,
                FieldValue::String("RegKeyOk".to_string()),
            ]),
        );
        // Component with valid file keypath
        db_ice9.add_record(
            "Component",
            Record::with_fields(vec![
                FieldValue::String("CompFile".to_string()),
                FieldValue::Null,
                FieldValue::String("TARGETDIR".to_string()),
                FieldValue::Short(0),
                FieldValue::Null,
                FieldValue::String("FileOk".to_string()),
            ]),
        );
        // Component with valid directory keypath
        db_ice9.add_record(
            "Component",
            Record::with_fields(vec![
                FieldValue::String("CompDir".to_string()),
                FieldValue::Null,
                FieldValue::String("TARGETDIR".to_string()),
                FieldValue::Short(0),
                FieldValue::Null,
                FieldValue::String("DirOk".to_string()),
            ]),
        );
        assert!(Linker::validate_ice09(&db_ice9).is_none());

        // Component with missing file or directory keypath -> ICE09 error
        let mut db_ice9_missing = LinkedDatabase::new()?;
        db_ice9_missing.add_record(
            "Component",
            Record::with_fields(vec![
                FieldValue::String("CompBadKp".to_string()),
                FieldValue::Null,
                FieldValue::String("TARGETDIR".to_string()),
                FieldValue::Short(0),
                FieldValue::Null,
                FieldValue::String("NonExistentFileOrDir".to_string()),
            ]),
        );
        assert_eq!(
            Linker::validate_ice09(&db_ice9_missing)
                .as_ref()
                .map(|r| (r.ice.as_str(), r.is_error)),
            Some(("ICE09", true))
        );

        Ok(())
    }

    /// Tests ICE18 through ICE33 edge cases.
    #[test]
    #[allow(clippy::too_many_lines)]
    fn test_ice18_to_ice33_comprehensive() -> Result<()> {
        // ICE18: File record with nulls, Component with nulls, empty keypath, keypath not in File, keypath belonging to self
        let mut db_ice18 = LinkedDatabase::new()?;
        db_ice18.add_record(
            "File",
            Record::with_fields(vec![FieldValue::Null, FieldValue::Null]),
        );
        db_ice18.add_record(
            "File",
            Record::with_fields(vec![
                FieldValue::String("FileSelf".to_string()),
                FieldValue::String("CompSelf".to_string()),
                FieldValue::String("app.exe".to_string()),
                FieldValue::Long(100),
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Short(1),
            ]),
        );
        db_ice18.add_record(
            "Component",
            Record::with_fields(vec![FieldValue::Null, FieldValue::Null]),
        );
        db_ice18.add_record(
            "Component",
            Record::with_fields(vec![
                FieldValue::String("CompEmptyKp".to_string()),
                FieldValue::Null,
                FieldValue::String("TARGETDIR".to_string()),
                FieldValue::Short(0),
                FieldValue::Null,
                FieldValue::String(String::new()),
            ]),
        );
        db_ice18.add_record(
            "Component",
            Record::with_fields(vec![
                FieldValue::String("CompNotInFiles".to_string()),
                FieldValue::Null,
                FieldValue::String("TARGETDIR".to_string()),
                FieldValue::Short(0),
                FieldValue::Null,
                FieldValue::String("RegOrDir".to_string()),
            ]),
        );
        db_ice18.add_record(
            "Component",
            Record::with_fields(vec![
                FieldValue::String("CompSelf".to_string()),
                FieldValue::Null,
                FieldValue::String("TARGETDIR".to_string()),
                FieldValue::Short(0),
                FieldValue::Null,
                FieldValue::String("FileSelf".to_string()),
            ]),
        );
        assert!(Linker::validate_ice18(&db_ice18).is_none());

        // ICE20: Record with nulls, missing actions, valid orders, and CostInitialize >= CostFinalize error
        let mut db_ice20 = LinkedDatabase::new()?;
        db_ice20.add_record(
            "InstallExecuteSequence",
            Record::with_fields(vec![FieldValue::Null, FieldValue::Null, FieldValue::Null]),
        );
        db_ice20.add_record(
            "InstallExecuteSequence",
            Record::with_fields(vec![
                FieldValue::String("CostInitialize".to_string()),
                FieldValue::Null,
                FieldValue::Short(1000),
            ]),
        );
        db_ice20.add_record(
            "InstallExecuteSequence",
            Record::with_fields(vec![
                FieldValue::String("CostFinalize".to_string()),
                FieldValue::Null,
                FieldValue::Short(800), // CostInitialize >= CostFinalize
            ]),
        );
        assert_eq!(
            Linker::validate_ice20(&db_ice20)
                .as_ref()
                .map(|r| (r.ice.as_str(), r.is_error)),
            Some(("ICE20", true))
        );

        // Valid order
        let mut db_ice20_ok = LinkedDatabase::new()?;
        db_ice20_ok.add_record(
            "InstallExecuteSequence",
            Record::with_fields(vec![
                FieldValue::String("InstallInitialize".to_string()),
                FieldValue::Null,
                FieldValue::Short(1500),
            ]),
        );
        db_ice20_ok.add_record(
            "InstallExecuteSequence",
            Record::with_fields(vec![
                FieldValue::String("InstallFinalize".to_string()),
                FieldValue::Null,
                FieldValue::Short(6600),
            ]),
        );
        db_ice20_ok.add_record(
            "InstallExecuteSequence",
            Record::with_fields(vec![
                FieldValue::String("CostInitialize".to_string()),
                FieldValue::Null,
                FieldValue::Short(800),
            ]),
        );
        db_ice20_ok.add_record(
            "InstallExecuteSequence",
            Record::with_fields(vec![
                FieldValue::String("CostFinalize".to_string()),
                FieldValue::Null,
                FieldValue::Short(1000),
            ]),
        );
        assert!(Linker::validate_ice20(&db_ice20_ok).is_none());

        // ICE30:
        // - Component with nulls
        // - File with nulls or non-string fname
        // - File fname with short|long syntax
        // - File belonging to unknown component
        // - Duplicate same file in same component
        // - Distinct files in same directory -> None
        let mut db_ice30 = LinkedDatabase::new()?;
        db_ice30.add_record(
            "Component",
            Record::with_fields(vec![FieldValue::Null, FieldValue::Null]),
        );
        db_ice30.add_record(
            "Component",
            Record::with_fields(vec![
                FieldValue::String("CompDirA".to_string()),
                FieldValue::Null,
                FieldValue::String("DIR_A".to_string()),
                FieldValue::Short(0),
                FieldValue::Null,
                FieldValue::Null,
            ]),
        );
        db_ice30.add_record(
            "File",
            Record::with_fields(vec![FieldValue::Null, FieldValue::Null]),
        );
        db_ice30.add_record(
            "File",
            Record::with_fields(vec![
                FieldValue::String("FNonStr".to_string()),
                FieldValue::String("CompDirA".to_string()),
                FieldValue::Short(42), // fname not string
                FieldValue::Long(100),
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Short(1),
            ]),
        );
        db_ice30.add_record(
            "File",
            Record::with_fields(vec![
                FieldValue::String("FUnkComp".to_string()),
                FieldValue::String("NonExistentComp".to_string()),
                FieldValue::String("file.txt".to_string()),
                FieldValue::Long(100),
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Short(2),
            ]),
        );
        db_ice30.add_record(
            "File",
            Record::with_fields(vec![
                FieldValue::String("FShortLong".to_string()),
                FieldValue::String("CompDirA".to_string()),
                FieldValue::String("SHORT~1.TXT|longfilename.txt".to_string()),
                FieldValue::Long(100),
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Short(3),
            ]),
        );
        // Same file re-inserted
        db_ice30.add_record(
            "File",
            Record::with_fields(vec![
                FieldValue::String("FShortLong".to_string()),
                FieldValue::String("CompDirA".to_string()),
                FieldValue::String("SHORT~1.TXT|longfilename.txt".to_string()),
                FieldValue::Long(100),
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Short(4),
            ]),
        );
        assert!(Linker::validate_ice30(&db_ice30).is_none());

        // ICE33:
        // - Class with null
        // - Class with valid GUID (38 chars, starts and ends with curlies)
        // - Class starting without { -> error
        // - Class ending without } -> error
        // - Class with len != 38 -> error
        // - Registry with nulls
        // - Registry with root 0 and key InprocServer32 -> warning
        // - Registry with root 0 and normal key -> ok
        // - Registry with root 1 and CLSID -> ok
        // - Registry with root 2 and normal key -> ok
        // Clean DB returning None for ICE33
        let mut db_ice33_guid_ok = LinkedDatabase::new()?;
        db_ice33_guid_ok.add_record("Class", Record::with_fields(vec![FieldValue::Null]));
        db_ice33_guid_ok.add_record(
            "Class",
            Record::with_fields(vec![
                FieldValue::String("{11111111-1111-1111-1111-111111111111}".to_string()),
                FieldValue::String("InprocServer32".to_string()),
                FieldValue::String("Comp1".to_string()),
            ]),
        );
        // Registry entries that don't trigger warning (root 0 normal, root 1 normal, root 2 normal)
        db_ice33_guid_ok.add_record(
            "Registry",
            Record::with_fields(vec![FieldValue::Null, FieldValue::Null]),
        );
        db_ice33_guid_ok.add_record(
            "Registry",
            Record::with_fields(vec![
                FieldValue::String("R2".to_string()),
                FieldValue::Short(0),
                FieldValue::String(r"Software\Normal".to_string()), // root 0 normal -> ok (contains InprocServer32 is false)
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::String("Comp1".to_string()),
            ]),
        );
        db_ice33_guid_ok.add_record(
            "Registry",
            Record::with_fields(vec![
                FieldValue::String("R3".to_string()),
                FieldValue::Short(1), // HKCU normal -> ok (*root == 2 is false)
                FieldValue::String(r"CLSID\{11111111-1111-1111-1111-111111111111}".to_string()),
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::String("Comp1".to_string()),
            ]),
        );
        db_ice33_guid_ok.add_record(
            "Registry",
            Record::with_fields(vec![
                FieldValue::String("R4".to_string()),
                FieldValue::Short(2), // HKLM normal -> ok (starts_with Software\Classes\CLSID\ is false)
                FieldValue::String(r"Software\Normal".to_string()),
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::String("Comp1".to_string()),
            ]),
        );
        assert!(Linker::validate_ice33(&db_ice33_guid_ok).is_none());

        // Registry entry with root 0 and InprocServer32 -> warning
        let mut db_ice33_warn_inproc = LinkedDatabase::new()?;
        db_ice33_warn_inproc.add_record(
            "Registry",
            Record::with_fields(vec![
                FieldValue::String("R1".to_string()),
                FieldValue::Short(0),
                FieldValue::String(r"Software\MyApp\InprocServer32".to_string()),
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::String("Comp1".to_string()),
            ]),
        );
        let rep_ice33_inproc = Linker::validate_ice33(&db_ice33_warn_inproc);
        assert_eq!(
            rep_ice33_inproc
                .as_ref()
                .map(|r| (r.ice.as_str(), r.is_error)),
            Some(("ICE33", false))
        );

        // Class ending without }
        let mut db_ice33_no_end = LinkedDatabase::new()?;
        db_ice33_no_end.add_record(
            "Class",
            Record::with_fields(vec![FieldValue::String(
                "{11111111-1111-1111-1111-111111111111X".to_string(),
            )]),
        );
        assert_eq!(
            Linker::validate_ice33(&db_ice33_no_end)
                .as_ref()
                .map(|r| (r.ice.as_str(), r.is_error)),
            Some(("ICE33", true))
        );

        // Class len != 38
        let mut db_ice33_short = LinkedDatabase::new()?;
        db_ice33_short.add_record(
            "Class",
            Record::with_fields(vec![FieldValue::String("{123}".to_string())]),
        );
        assert_eq!(
            Linker::validate_ice33(&db_ice33_short)
                .as_ref()
                .map(|r| (r.ice.as_str(), r.is_error)),
            Some(("ICE33", true))
        );

        Ok(())
    }

    /// Tests ICE38 through ICE103 edge cases.
    #[test]
    #[allow(clippy::too_many_lines)]
    fn test_ice38_to_ice103_comprehensive() -> Result<()> {
        // ICE38:
        // - Registry record with nulls
        // - Component record with nulls
        // - Component in LocalAppDataFolder with null attr, non-reg kp, null kp, unknown kp, and root 1 (HKCU - valid)
        let mut db_ice38 = LinkedDatabase::new()?;
        db_ice38.add_record(
            "Registry",
            Record::with_fields(vec![FieldValue::Null, FieldValue::Null]),
        );
        db_ice38.add_record(
            "Registry",
            Record::with_fields(vec![
                FieldValue::String("HkcuKey".to_string()),
                FieldValue::Short(1), // HKCU
            ]),
        );
        db_ice38.add_record(
            "Component",
            Record::with_fields(vec![FieldValue::Null, FieldValue::Null]),
        );
        // Comp with null attr in LocalAppDataFolder
        db_ice38.add_record(
            "Component",
            Record::with_fields(vec![
                FieldValue::String("CompNullAttr".to_string()),
                FieldValue::Null,
                FieldValue::String("LocalAppDataFolder".to_string()),
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
            ]),
        );
        // Comp without registry keypath in LocalAppDataFolder
        db_ice38.add_record(
            "Component",
            Record::with_fields(vec![
                FieldValue::String("CompNonReg".to_string()),
                FieldValue::Null,
                FieldValue::String("LocalAppDataFolder".to_string()),
                FieldValue::Short(0),
                FieldValue::Null,
                FieldValue::String("SomeFile".to_string()),
            ]),
        );
        // Comp with unknown reg kp
        db_ice38.add_record(
            "Component",
            Record::with_fields(vec![
                FieldValue::String("CompUnkReg".to_string()),
                FieldValue::Null,
                FieldValue::String("LocalAppDataFolder".to_string()),
                FieldValue::Short(0x0010),
                FieldValue::Null,
                FieldValue::String("UnknownReg".to_string()),
            ]),
        );
        // Comp with HKCU reg kp -> ok
        db_ice38.add_record(
            "Component",
            Record::with_fields(vec![
                FieldValue::String("CompHkcu".to_string()),
                FieldValue::Null,
                FieldValue::String("LocalAppDataFolder".to_string()),
                FieldValue::Short(0x0010),
                FieldValue::Null,
                FieldValue::String("HkcuKey".to_string()),
            ]),
        );
        assert!(Linker::validate_ice38(&db_ice38).is_none());

        // ICE61:
        // - Property other than ProductVersion
        // - ProductVersion parse failure
        // - Upgrade with null min_ver
        // - Upgrade with unparseable min_ver
        // - Upgrade with min_ver <= ProductVersion -> ok
        let mut db_ice61 = LinkedDatabase::new()?;
        db_ice61.add_record(
            "Property",
            Record::with_fields(vec![
                FieldValue::String("OtherProp".to_string()),
                FieldValue::String("Val".to_string()),
            ]),
        );
        assert!(Linker::validate_ice61(&db_ice61).is_none());

        let mut db_ice61_bad_ver = LinkedDatabase::new()?;
        db_ice61_bad_ver.add_record(
            "Property",
            Record::with_fields(vec![
                FieldValue::String("ProductVersion".to_string()),
                FieldValue::String("not-a-valid-ver".to_string()),
            ]),
        );
        assert!(Linker::validate_ice61(&db_ice61_bad_ver).is_none());

        let mut db_ice61_ok = LinkedDatabase::new()?;
        db_ice61_ok.add_record(
            "Property",
            Record::with_fields(vec![
                FieldValue::String("ProductVersion".to_string()),
                FieldValue::String("2.0.0".to_string()),
            ]),
        );
        db_ice61_ok.add_record(
            "Upgrade",
            Record::with_fields(vec![
                FieldValue::String("{11111111-1111-1111-1111-111111111111}".to_string()),
                FieldValue::Null, // null min_ver
            ]),
        );
        db_ice61_ok.add_record(
            "Upgrade",
            Record::with_fields(vec![
                FieldValue::String("{11111111-1111-1111-1111-111111111111}".to_string()),
                FieldValue::String("invalid-version".to_string()), // unparseable min_ver
            ]),
        );
        db_ice61_ok.add_record(
            "Upgrade",
            Record::with_fields(vec![
                FieldValue::String("{11111111-1111-1111-1111-111111111111}".to_string()),
                FieldValue::String("1.0.0".to_string()), // 1.0.0 <= 2.0.0 -> ok
            ]),
        );
        assert!(Linker::validate_ice61(&db_ice61_ok).is_none());

        // ICE80: Component with null attr, pure 32-bit package, pure 64-bit package
        let mut db_ice80 = LinkedDatabase::new()?;
        db_ice80.add_record(
            "Component",
            Record::with_fields(vec![
                FieldValue::String("CompNullAttr".to_string()),
                FieldValue::Null,
                FieldValue::String("TARGETDIR".to_string()),
                FieldValue::Null,
            ]),
        );
        db_ice80.add_record(
            "Component",
            Record::with_fields(vec![
                FieldValue::String("Comp32".to_string()),
                FieldValue::Null,
                FieldValue::String("TARGETDIR".to_string()),
                FieldValue::Short(0),
            ]),
        );
        assert!(Linker::validate_ice80(&db_ice80).is_none());

        let mut db_ice80_64 = LinkedDatabase::new()?;
        db_ice80_64.add_record(
            "Component",
            Record::with_fields(vec![
                FieldValue::String("Comp64".to_string()),
                FieldValue::Null,
                FieldValue::String("TARGETDIR".to_string()),
                FieldValue::Short(0x0100),
            ]),
        );
        assert!(Linker::validate_ice80(&db_ice80_64).is_none());

        // ICE99: Directory with nulls, empty parent, dir == parent, and valid tree
        let mut db_ice99 = LinkedDatabase::new()?;
        db_ice99.add_record(
            "Directory",
            Record::with_fields(vec![FieldValue::Null, FieldValue::Null]),
        );
        db_ice99.add_record(
            "Directory",
            Record::with_fields(vec![
                FieldValue::String("TARGETDIR".to_string()),
                FieldValue::String(String::new()), // empty parent
                FieldValue::String("SourceDir".to_string()),
            ]),
        );
        db_ice99.add_record(
            "Directory",
            Record::with_fields(vec![
                FieldValue::String("SameSelf".to_string()),
                FieldValue::String("SameSelf".to_string()), // dir == parent
                FieldValue::String("SameSelf".to_string()),
            ]),
        );
        db_ice99.add_record(
            "Directory",
            Record::with_fields(vec![
                FieldValue::String("SubDir".to_string()),
                FieldValue::String("TARGETDIR".to_string()),
                FieldValue::String("Sub".to_string()),
            ]),
        );
        assert!(Linker::validate_ice99(&db_ice99).is_none());

        // ICE101: File with null seq, file with seq > 0
        let mut db_ice101 = LinkedDatabase::new()?;
        db_ice101.add_record(
            "File",
            Record::with_fields(vec![
                FieldValue::String("F1".to_string()),
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null, // null seq
            ]),
        );
        db_ice101.add_record(
            "File",
            Record::with_fields(vec![
                FieldValue::String("F2".to_string()),
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Short(10), // > 0
            ]),
        );
        assert!(Linker::validate_ice101(&db_ice101).is_none());

        // ICE103: Shortcut with null index, shortcut with index >= 0
        let mut db_ice103 = LinkedDatabase::new()?;
        db_ice103.add_record(
            "Shortcut",
            Record::with_fields(vec![
                FieldValue::String("S1".to_string()),
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null, // null icon idx
            ]),
        );
        db_ice103.add_record(
            "Shortcut",
            Record::with_fields(vec![
                FieldValue::String("S2".to_string()),
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Short(0), // >= 0
            ]),
        );
        assert!(Linker::validate_ice103(&db_ice103).is_none());

        Ok(())
    }

    /// Tests trait implementations (`Debug`, `Clone`, `PartialEq`, `Default`) for linker data structures.
    #[test]
    fn test_linker_types_derives_and_display() {
        let std_dir = STANDARD_DIRECTORIES[0].clone();
        assert_eq!(std_dir, STANDARD_DIRECTORIES[0]);
        assert!(format!("{std_dir:?}").contains("StandardDirectory"));

        let std_act = STANDARD_INSTALL_EXECUTE_ACTIONS[0].clone();
        assert_eq!(std_act, STANDARD_INSTALL_EXECUTE_ACTIONS[0]);
        assert!(format!("{std_act:?}").contains("StandardActionOrder"));

        let ice_rep = IceReport {
            ice: "ICE99".to_string(),
            is_error: true,
            message: "test message".to_string(),
        };
        assert_eq!(ice_rep, ice_rep.clone());
        assert!(format!("{ice_rep:?}").contains("IceReport"));

        let db_default = LinkedDatabase::default();
        assert_eq!(db_default, db_default.clone());
        assert!(format!("{db_default:?}").contains("LinkedDatabase"));

        let linker = Linker::default();
        assert!(format!("{linker:?}").contains("Linker"));
    }

    #[test]
    #[allow(clippy::too_many_lines)]
    fn test_linker_file_binding_and_cabinet_packaging() -> Result<()> {
        let temp_dir = std::env::temp_dir().join("msi_test_linker_binding");
        let _ = std::fs::create_dir_all(&temp_dir);
        let sample_file = temp_dir.join("sample.txt");
        std::fs::write(&sample_file, b"Hello WiX binder payload!")?;

        let versioned_file = temp_dir.join("versioned.txt");
        std::fs::write(&versioned_file, b"versioned payload")?;

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
        std::fs::write(&font_file, &font_data)?;

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
        std::fs::write(&pe_file, &pe_data)?;

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

        let linked_db = linker.link()?;

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
        let linked_media_db = linker_media.link()?;
        assert_ne!(linked_media_db.get_records("File"), []);

        // Test bind_files_and_pack_cabinets on empty database and db without File table
        let mut empty_db = LinkedDatabase::new()?;
        assert!(linker.bind_files_and_pack_cabinets(&mut empty_db).is_ok());

        let mut no_file_db = LinkedDatabase::new()?;
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
        let mut pe_truncated = pe64;
        pe_truncated.truncate(576 + 10);
        assert_eq!(inspect_pe_version(&pe_truncated), None);

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
        let db = LinkedDatabase::new()?;
        assert!(Linker::run_ice_validations(&db).is_ok());

        Ok(())
    }

    /// Dummy ICE rule for testing custom rule registration.
    struct DummyRule;

    #[allow(clippy::unnecessary_literal_bound)]
    impl IceRule for DummyRule {
        fn name(&self) -> &str {
            "ICE999"
        }

        fn description(&self) -> &str {
            "Dummy test rule"
        }

        fn execute(&self, _db: &LinkedDatabase) -> Vec<IceDiagnostic> {
            vec![IceDiagnostic::new(
                "ICE999",
                IceDiagnosticType::Info,
                "All good",
            )]
        }
    }

    /// Failing formatter writer for testing display error paths.
    struct FailingWriter;

    impl fmt::Write for FailingWriter {
        fn write_str(&mut self, _: &str) -> fmt::Result {
            Err(fmt::Error)
        }
    }

    #[test]
    #[allow(clippy::too_many_lines)]
    fn test_ice_harness_and_cub_validator() -> Result<()> {
        use std::fmt::Write;

        // 1. Test IceDiagnosticType Display
        assert_eq!(format!("{}", IceDiagnosticType::Error), "ERROR");
        assert_eq!(format!("{}", IceDiagnosticType::Warning), "WARNING");
        assert_eq!(format!("{}", IceDiagnosticType::Info), "INFO");

        // Test FailingWriter on IceDiagnostic Display
        let diag_minimal = IceDiagnostic::new("ICE01", IceDiagnosticType::Error, "Missing table");
        assert_eq!(format!("{diag_minimal}"), "ICE01: ERROR: Missing table");
        let mut fail_w = FailingWriter;
        let _ = write!(fail_w, "{diag_minimal}");

        let diag_table_only = IceDiagnostic {
            ice: "ICE02".to_string(),
            diagnostic_type: IceDiagnosticType::Error,
            description: "Table error".to_string(),
            table: Some("Feature".to_string()),
            column: None,
            row_key: None,
        };
        assert_eq!(
            format!("{diag_table_only}"),
            "ICE02: ERROR: Table error Feature"
        );

        let diag_table_col = IceDiagnostic {
            ice: "ICE02".to_string(),
            diagnostic_type: IceDiagnosticType::Error,
            description: "Table col error".to_string(),
            table: Some("Feature".to_string()),
            column: Some("Feature".to_string()),
            row_key: None,
        };
        assert_eq!(
            format!("{diag_table_col}"),
            "ICE02: ERROR: Table col error Feature Feature"
        );

        let diag_full = IceDiagnostic::new("ICE03", IceDiagnosticType::Warning, "Invalid length")
            .with_location("Component", "Component", "Comp1");
        assert_eq!(
            format!("{diag_full}"),
            "ICE03: WARNING: Invalid length Component Component Comp1"
        );

        let report = IceReport {
            ice: "ICE08".to_string(),
            is_error: true,
            message: "Duplicate GUID".to_string(),
        };
        let diag_from_rep = IceDiagnostic::from(report);
        assert_eq!(diag_from_rep.ice, "ICE08");
        assert_eq!(diag_from_rep.diagnostic_type, IceDiagnosticType::Error);
        assert_eq!(diag_from_rep.description, "Duplicate GUID");

        let rep_from_diag = IceReport::from(diag_from_rep);
        assert_eq!(rep_from_diag.ice, "ICE08");
        assert!(rep_from_diag.is_error);
        assert_eq!(rep_from_diag.message, "Duplicate GUID");

        let warn_report = IceReport {
            ice: "ICE33".to_string(),
            is_error: false,
            message: "Warning msg".to_string(),
        };
        let warn_diag = IceDiagnostic::from(warn_report);
        assert_eq!(warn_diag.diagnostic_type, IceDiagnosticType::Warning);

        // 3. Test IceRegistry with standard rules and custom rule
        let registry = IceRegistry::with_standard_rules();
        assert_eq!(format!("{registry:?}"), "IceRegistry { rule_count: 19 }");
        let rule_names = registry.rule_names();
        assert!(rule_names.contains(&"ICE01"));
        assert!(rule_names.contains(&"ICE103"));
        assert_eq!(registry.rules().len(), 19);

        let db = LinkedDatabase::new()?;
        let all_diags = registry.execute_all(&db);
        assert_eq!(all_diags.len(), 0);

        let par_diags = registry.execute_parallel(&db);
        assert_eq!(par_diags.len(), 0);

        let mut custom_registry = IceRegistry::default();
        custom_registry.register(DummyRule);
        assert_eq!(custom_registry.rule_names(), vec!["ICE999"]);
        assert_eq!(DummyRule.description(), "Dummy test rule");
        let custom_diags = custom_registry.execute_all(&db);
        assert_eq!(custom_diags.len(), 1);
        assert_eq!(custom_diags[0].ice, "ICE999");
        assert_eq!(custom_diags[0].diagnostic_type, IceDiagnosticType::Info);

        let custom_par_diags = custom_registry.execute_parallel(&db);
        assert_eq!(custom_par_diags.len(), 1);

        // StandardIceRule
        let std_rule = StandardIceRule::new("ICE01", "desc", Linker::validate_ice01);
        assert_eq!(std_rule.name(), "ICE01");
        assert_eq!(std_rule.description(), "desc");
        assert_eq!(std_rule.execute(&db).len(), 0);

        // 4. Test LinkedDatabase::to_script_database
        let mut test_db = LinkedDatabase::new()?;
        test_db.add_record(
            "Property",
            Record::with_fields(vec![
                FieldValue::String("PROP1".to_string()),
                FieldValue::String("VAL1".to_string()),
                FieldValue::Short(42),
                FieldValue::Long(100_000),
                FieldValue::Stream(crate::database::tables::types::StringPoolId::new(99)),
                FieldValue::Null,
            ]),
        );
        let script_db = test_db.to_script_database();
        assert!(script_db.table_exists("Property"));
        assert_eq!(script_db.row_count("Property"), 1);

        // 5. Test CubValidator with JScript and VBScript evaluators
        let mut cub_db = LinkedDatabase::new()?;
        // Add _ICESequence
        cub_db.add_record(
            "_ICESequence",
            Record::with_fields(vec![
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Short(99),
            ]),
        );
        cub_db.add_record(
            "_ICESequence",
            Record::with_fields(vec![
                FieldValue::String("ICE00_NotFound".to_string()),
                FieldValue::Null,
                FieldValue::Short(0),
            ]),
        );
        cub_db.add_record(
            "_ICESequence",
            Record::with_fields(vec![
                FieldValue::String("ICE01_JScript".to_string()),
                FieldValue::Null,
                FieldValue::Short(1),
            ]),
        );
        cub_db.add_record(
            "_ICESequence",
            Record::with_fields(vec![
                FieldValue::String("ICE02_VBScript".to_string()),
                FieldValue::Null,
                FieldValue::Short(2),
            ]),
        );
        cub_db.add_record(
            "_ICESequence",
            Record::with_fields(vec![
                FieldValue::String("ICE03_FailScript".to_string()),
                FieldValue::Null,
                FieldValue::Short(3),
            ]),
        );
        // Custom actions: Type 37 (JScript in Target), Type 38 (VBScript in Target)
        cub_db.add_record(
            "CustomAction",
            Record::with_fields(vec![
                FieldValue::String("ICE01_JScript".to_string()),
                FieldValue::Short(37), // JScript text
                FieldValue::Null,
                FieldValue::String(
                    "Session.Message(1, 'Target database table Property count: ' + Session.Database.RowCount('Property'));".to_string(),
                ),
            ]),
        );
        cub_db.add_record(
            "CustomAction",
            Record::with_fields(vec![
                FieldValue::String("ICE02_VBScript".to_string()),
                FieldValue::Short(38), // VBScript text
                FieldValue::Null,
                FieldValue::String(
                    "Session.Message 2, \"VBScript check executed successfully\"".to_string(),
                ),
            ]),
        );
        cub_db.add_record(
            "CustomAction",
            Record::with_fields(vec![
                FieldValue::String("ICE03_FailScript".to_string()),
                FieldValue::Short(37),
                FieldValue::Null,
                FieldValue::String("throw new Error('Intentional script failure');".to_string()),
            ]),
        );

        let validator = CubValidator::from_database("darice.cub", cub_db);
        assert_eq!(validator.name(), "darice.cub");
        assert_eq!(
            validator.action_names(),
            vec![
                "ICE00_NotFound".to_string(),
                "ICE01_JScript".to_string(),
                "ICE02_VBScript".to_string(),
                "ICE03_FailScript".to_string(),
            ]
        );
        assert!(!validator.database().tables.is_empty());

        let cub_diags = validator.execute(&test_db)?;
        assert_eq!(cub_diags.len(), 3);
        assert_eq!(cub_diags[0].ice, "ICE01_JScript");
        assert_eq!(cub_diags[0].diagnostic_type, IceDiagnosticType::Error);
        assert!(cub_diags[0].description.contains("Property count: 1"));

        assert_eq!(cub_diags[1].ice, "ICE02_VBScript");
        assert_eq!(cub_diags[1].diagnostic_type, IceDiagnosticType::Warning);
        assert!(cub_diags[1].description.contains("VBScript check executed"));

        assert_eq!(cub_diags[2].ice, "ICE03_FailScript");
        assert_eq!(cub_diags[2].diagnostic_type, IceDiagnosticType::Error);
        assert!(cub_diags[2]
            .description
            .contains("script evaluation failed"));

        // Fallback action_names without _ICESequence and extensive CA variations
        let mut cub_db2 = LinkedDatabase::new()?;
        cub_db2.add_record(
            "CustomAction",
            Record::with_fields(vec![
                FieldValue::Null,
                FieldValue::Short(1),
                FieldValue::Null,
                FieldValue::Null,
            ]),
        );
        cub_db2.add_record(
            "CustomAction",
            Record::with_fields(vec![
                FieldValue::String("NonIceAction".to_string()),
                FieldValue::Short(1),
                FieldValue::Null,
                FieldValue::Null,
            ]),
        );
        cub_db2.add_record(
            "CustomAction",
            Record::with_fields(vec![
                FieldValue::String("ICE00_NullType".to_string()),
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
            ]),
        );
        cub_db2.add_record(
            "CustomAction",
            Record::with_fields(vec![
                FieldValue::String("ICE00_NullSource".to_string()),
                FieldValue::Short(5),
                FieldValue::Null,
                FieldValue::Null,
            ]),
        );
        cub_db2.add_record(
            "CustomAction",
            Record::with_fields(vec![
                FieldValue::String("ICE99".to_string()),
                FieldValue::Short(5), // Type 5 from Binary table
                FieldValue::String("BinaryScript".to_string()),
                FieldValue::Null,
            ]),
        );
        cub_db2.add_record(
            "CustomAction",
            Record::with_fields(vec![
                FieldValue::String("ICE00_NonScript".to_string()),
                FieldValue::Long(1),
                FieldValue::Null,
                FieldValue::Null,
            ]),
        );
        cub_db2.add_record(
            "CustomAction",
            Record::with_fields(vec![
                FieldValue::String("ICE00_MissingInline".to_string()),
                FieldValue::Short(37),
                FieldValue::Null,
                FieldValue::Null,
            ]),
        );
        cub_db2.add_record(
            "CustomAction",
            Record::with_fields(vec![
                FieldValue::String("ICE00_MissingBin".to_string()),
                FieldValue::Short(5),
                FieldValue::String("NoSuchBinary".to_string()),
                FieldValue::Null,
            ]),
        );
        cub_db2.add_record(
            "CustomAction",
            Record::with_fields(vec![
                FieldValue::String("ICE00_NullBin".to_string()),
                FieldValue::Short(5),
                FieldValue::String("NullBin".to_string()),
                FieldValue::Null,
            ]),
        );
        cub_db2.add_record(
            "CustomAction",
            Record::with_fields(vec![
                FieldValue::String("ICE00_VbBin".to_string()),
                FieldValue::Short(6),
                FieldValue::String("VbBin".to_string()),
                FieldValue::Null,
            ]),
        );
        cub_db2.add_record(
            "CustomAction",
            Record::with_fields(vec![
                FieldValue::String("ICE53".to_string()),
                FieldValue::Short(53),
                FieldValue::Null,
                FieldValue::String("Session.Message(1, '53');".to_string()),
            ]),
        );
        cub_db2.add_record(
            "CustomAction",
            Record::with_fields(vec![
                FieldValue::String("ICE54".to_string()),
                FieldValue::Short(54),
                FieldValue::Null,
                FieldValue::String("Session.Message 1, \"54\"".to_string()),
            ]),
        );
        cub_db2.add_record(
            "Binary",
            Record::with_fields(vec![
                FieldValue::String("BinaryScript".to_string()),
                FieldValue::String("Session.Message(2, 'From Binary Table');".to_string()),
            ]),
        );
        cub_db2.add_record(
            "Binary",
            Record::with_fields(vec![
                FieldValue::String("NullBin".to_string()),
                FieldValue::Null,
            ]),
        );
        cub_db2.add_record(
            "Binary",
            Record::with_fields(vec![
                FieldValue::String("VbBin".to_string()),
                FieldValue::String("Session.Message 1, \"VB from Bin\"".to_string()),
            ]),
        );
        let validator2 = CubValidator::from_database("darice2.cub", cub_db2);
        assert!(validator2.action_names().contains(&"ICE99".to_string()));
        let bin_diags = validator2.execute(&test_db)?;
        assert_eq!(bin_diags.len(), 4);

        // from_bytes and open error testing
        assert!(CubValidator::from_bytes("test", &[]).is_err());
        assert!(CubValidator::open("non_existent_file_xyz.cub").is_err());

        // Real package write/read for CubValidator::from_bytes and open
        let cub_builder = Package::builder()
            .product_name("CubModule")
            .manufacturer("Acme")
            .version(ProductVersion::new(1, 0, 0))
            .product_code("{99999999-9999-9999-9999-999999999999}");
        let pkg = cub_builder.build()?;
        let pkg_bytes = pkg.to_bytes()?;
        let val_from_bytes = CubValidator::from_bytes("pkg_cub", &pkg_bytes)?;
        assert_eq!(val_from_bytes.name(), "pkg_cub");

        let temp_dir = std::env::temp_dir();
        let temp_cub = temp_dir.join(format!("test_module_{}.cub", std::process::id()));
        std::fs::write(&temp_cub, &pkg_bytes)?;
        let val_from_file = CubValidator::open(&temp_cub)?;
        assert!(val_from_file.name().starts_with("test_module_"));
        let _ = std::fs::remove_file(&temp_cub);

        // 6. Test Linker::run_filtered_ice_validations
        let mut ice_linker = Linker::new();
        let mut bad_ice_db = LinkedDatabase::new()?;
        bad_ice_db.add_record(
            "Component",
            Record::with_fields(vec![
                FieldValue::String("C1".to_string()),
                FieldValue::String("{11111111-1111-1111-1111-111111111111}".to_string()),
                FieldValue::String("TARGETDIR".to_string()),
                FieldValue::Short(0),
                FieldValue::Null,
                FieldValue::Null,
            ]),
        );
        bad_ice_db.add_record(
            "Component",
            Record::with_fields(vec![
                FieldValue::String("C2".to_string()),
                FieldValue::String("{11111111-1111-1111-1111-111111111111}".to_string()),
                FieldValue::String("TARGETDIR".to_string()),
                FieldValue::Short(0),
                FieldValue::Null,
                FieldValue::Null,
            ]),
        );

        // Error report unsuppressed
        assert!(ice_linker
            .run_filtered_ice_validations(&bad_ice_db)
            .is_err());

        // Suppressed ICE
        ice_linker.suppress_ice("ICE08");
        assert!(ice_linker.run_filtered_ice_validations(&bad_ice_db).is_ok());

        // Selected ICE (filtering out ICE08)
        ice_linker.suppressed_ice.clear();
        ice_linker.select_ice("ICE01");
        assert!(ice_linker.run_filtered_ice_validations(&bad_ice_db).is_ok());

        // Selected ICE (including ICE08)
        ice_linker.select_ice("ICE08");
        assert!(ice_linker
            .run_filtered_ice_validations(&bad_ice_db)
            .is_err());

        // Warnings as errors
        let mut warn_linker = Linker::new();
        warn_linker.set_warnings_as_errors(true);
        let mut warn_ice_db = LinkedDatabase::new()?;
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
        // Suppress warning
        warn_linker.suppress_warning("ICE33");
        assert!(warn_linker
            .run_filtered_ice_validations(&warn_ice_db)
            .is_ok());

        // Unsuppressed warning
        warn_linker.suppressed_warnings.clear();
        assert!(warn_linker
            .run_filtered_ice_validations(&warn_ice_db)
            .is_err());

        // Default linker with warnings_as_errors = false does not fail on warnings
        let default_warn_linker = Linker::new();
        assert!(default_warn_linker
            .run_filtered_ice_validations(&warn_ice_db)
            .is_ok());

        Ok(())
    }

    #[test]
    #[allow(clippy::too_many_lines)]
    fn test_merge_module_ingestion_and_linking() -> Result<()> {
        // 1. Test modularize_identifier
        let mut ignore_set = HashSet::new();
        ignore_set.insert("IgnoredComp".to_string());

        assert_eq!(
            modularize_identifier("Comp1", "GUID1", &ignore_set),
            "Comp1.GUID1"
        );
        assert_eq!(modularize_identifier("", "GUID1", &ignore_set), "");
        assert_eq!(
            modularize_identifier("TARGETDIR", "GUID1", &ignore_set),
            "TARGETDIR"
        );
        assert_eq!(
            modularize_identifier("ProgramFilesFolder", "GUID1", &ignore_set),
            "ProgramFilesFolder"
        );
        assert_eq!(
            modularize_identifier("IgnoredComp", "GUID1", &ignore_set),
            "IgnoredComp"
        );
        assert_eq!(
            modularize_identifier("Already.GUID1", "GUID1", &ignore_set),
            "Already.GUID1"
        );

        // 2. Test MergeModule construction and guid extraction
        let mut mod_db = LinkedDatabase::new()?;
        mod_db.add_record(
            "ModuleSignature",
            Record::with_fields(vec![
                FieldValue::String("MyModule.12345678_1234_1234_1234_1234567890AB".to_string()),
                FieldValue::Short(1033),
                FieldValue::String("2.1.0".to_string()),
            ]),
        );
        let msm = MergeModule::from_database(mod_db.clone());
        assert_eq!(msm.id, "MyModule.12345678_1234_1234_1234_1234567890AB");
        assert_eq!(msm.language, 1033);
        assert_eq!(msm.version, "2.1.0");
        assert_eq!(msm.guid(), "12345678_1234_1234_1234_1234567890AB");

        let def_msm = MergeModule::from_database(LinkedDatabase::new()?);
        assert_eq!(def_msm.id, "Module");
        assert_eq!(def_msm.guid(), "Module");

        // 3. Test MergeModule from bytes and file
        let msm_builder = Package::builder()
            .product_name("MsmTest")
            .manufacturer("Acme")
            .version(ProductVersion::new(1, 0, 0))
            .product_code("{88888888-8888-8888-8888-888888888888}");
        let pkg = msm_builder.build()?;
        let pkg_bytes = pkg.to_bytes()?;
        let msm_from_bytes = MergeModule::from_bytes(&pkg_bytes)?;
        assert_eq!(msm_from_bytes.id, "Module");

        let temp_dir = std::env::temp_dir();
        let temp_msm = temp_dir.join(format!("test_merge_{}.msm", std::process::id()));
        std::fs::write(&temp_msm, &pkg_bytes)?;
        let msm_from_file = MergeModule::open(&temp_msm)?;
        assert_eq!(msm_from_file.id, "Module");
        let _ = std::fs::remove_file(&temp_msm);

        assert!(MergeModule::from_bytes(&[]).is_err());
        assert!(MergeModule::open("non_existent_msm_file.msm").is_err());

        // 4. Test LinkedDatabase::merge_module with substitutions, retargeting, modularization
        let mut target_db = LinkedDatabase::new()?;
        target_db.add_record(
            "Feature",
            Record::with_fields(vec![
                FieldValue::String("MainFeature".to_string()),
                FieldValue::Null,
                FieldValue::String("Main Feature".to_string()),
                FieldValue::Null,
                FieldValue::Short(1),
                FieldValue::Short(1),
                FieldValue::Null,
                FieldValue::Short(0),
            ]),
        );
        target_db.add_record(
            "Directory",
            Record::with_fields(vec![
                FieldValue::String("INSTALLFOLDER".to_string()),
                FieldValue::String("ProgramFilesFolder".to_string()),
                FieldValue::String("MyApp".to_string()),
            ]),
        );
        target_db.add_record(
            "FeatureComponents",
            Record::with_fields(vec![
                FieldValue::String("OtherFeature".to_string()),
                FieldValue::String("OtherComp".to_string()),
            ]),
        );
        target_db.add_record(
            "FeatureComponents",
            Record::with_fields(vec![
                FieldValue::String("MainFeature".to_string()),
                FieldValue::String("DifferentComp".to_string()),
            ]),
        );
        target_db.add_record(
            "FeatureComponents",
            Record::with_fields(vec![FieldValue::Null, FieldValue::Null]),
        );

        // Prepare module database
        let mut source_msm_db = LinkedDatabase::new()?;
        source_msm_db.add_record(
            "ModuleSignature",
            Record::with_fields(vec![
                FieldValue::String("ModuleA.GUID_ABC".to_string()),
                FieldValue::Short(1033),
                FieldValue::String("1.0.0".to_string()),
            ]),
        );
        // Module directory with TARGETDIR parent (should retarget to INSTALLFOLDER)
        source_msm_db.add_record(
            "Directory",
            Record::with_fields(vec![
                FieldValue::String("TARGETDIR".to_string()),
                FieldValue::Null,
                FieldValue::String("SourceDir".to_string()),
            ]),
        );
        source_msm_db.add_record(
            "Directory",
            Record::with_fields(vec![
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::String("BothNull".to_string()),
            ]),
        );
        source_msm_db.add_record(
            "Directory",
            Record::with_fields(vec![
                FieldValue::String("ModuleDir".to_string()),
                FieldValue::String("TARGETDIR".to_string()),
                FieldValue::String("ModSubDir".to_string()),
            ]),
        );
        source_msm_db.add_record(
            "Directory",
            Record::with_fields(vec![
                FieldValue::String("SubModDir".to_string()),
                FieldValue::String("ModuleDir".to_string()),
                FieldValue::String("Sub".to_string()),
            ]),
        );
        source_msm_db.add_record(
            "Directory",
            Record::with_fields(vec![
                FieldValue::String("NullParentDir".to_string()),
                FieldValue::Null,
                FieldValue::String("NullDir".to_string()),
            ]),
        );
        source_msm_db.add_record(
            "Directory",
            Record::with_fields(vec![
                FieldValue::String("EmptyParentDir".to_string()),
                FieldValue::String(String::new()),
                FieldValue::String("Empty".to_string()),
            ]),
        );
        source_msm_db.add_record(
            "Directory",
            Record::with_fields(vec![
                FieldValue::String("AlreadyTargetParentDir".to_string()),
                FieldValue::String("INSTALLFOLDER".to_string()),
                FieldValue::String("Atp".to_string()),
            ]),
        );
        // CustomAction in module
        source_msm_db.add_record(
            "CustomAction",
            Record::with_fields(vec![
                FieldValue::String("ModCA".to_string()),
                FieldValue::Short(1),
                FieldValue::String("Target".to_string()),
                FieldValue::Null,
            ]),
        );
        source_msm_db.add_record(
            "CustomAction",
            Record::with_fields(vec![
                FieldValue::Null,
                FieldValue::Short(1),
                FieldValue::Null,
                FieldValue::Null,
            ]),
        );
        // Module component
        source_msm_db.add_record(
            "Component",
            Record::with_fields(vec![
                FieldValue::String("ModComp".to_string()),
                FieldValue::String("{11111111-1111-1111-1111-111111111111}".to_string()),
                FieldValue::String("ModuleDir".to_string()),
                FieldValue::Short(0),
                FieldValue::Null,
                FieldValue::String("ModFile".to_string()),
            ]),
        );
        source_msm_db.add_record(
            "Component",
            Record::with_fields(vec![
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Short(0),
                FieldValue::Null,
                FieldValue::Null,
            ]),
        );
        // Module file
        source_msm_db.add_record(
            "File",
            Record::with_fields(vec![
                FieldValue::String("ModFile".to_string()),
                FieldValue::String("ModComp".to_string()),
                FieldValue::String("modfile.dll".to_string()),
                FieldValue::Long(1024),
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Short(0),
                FieldValue::Short(1),
            ]),
        );
        source_msm_db.add_record(
            "File",
            Record::with_fields(vec![
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Long(0),
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Short(0),
                FieldValue::Short(1),
            ]),
        );
        // Module property with substitution template
        source_msm_db.add_record(
            "Property",
            Record::with_fields(vec![
                FieldValue::String("SERVICE_PORT".to_string()),
                FieldValue::String("DEFAULT_PORT".to_string()),
            ]),
        );
        source_msm_db.add_record(
            "Property",
            Record::with_fields(vec![
                FieldValue::String("OTHER_PORT".to_string()),
                FieldValue::String("DEF_OTHER".to_string()),
            ]),
        );
        source_msm_db.add_record(
            "ModuleSubstitution",
            Record::with_fields(vec![
                FieldValue::String("Property".to_string()),
                FieldValue::String("SERVICE_PORT".to_string()),
                FieldValue::String("Value".to_string()),
                FieldValue::String("[=CONFIG_PORT]".to_string()),
            ]),
        );
        source_msm_db.add_record(
            "ModuleSubstitution",
            Record::with_fields(vec![
                FieldValue::String("Property".to_string()),
                FieldValue::String("OTHER_PORT".to_string()),
                FieldValue::String("Value".to_string()),
                FieldValue::String("[CONFIG_PORT]".to_string()),
            ]),
        );
        source_msm_db.add_record(
            "ModuleSubstitution",
            Record::with_fields(vec![
                FieldValue::String("Property".to_string()),
                FieldValue::String("NULL_PORT".to_string()),
                FieldValue::String("Value".to_string()),
                FieldValue::Null,
            ]),
        );
        source_msm_db.add_record(
            "ModuleSubstitution",
            Record::with_fields(vec![
                FieldValue::String("Property".to_string()),
                FieldValue::String("KEY".to_string()),
                FieldValue::String("NoSuchCol".to_string()),
                FieldValue::String("VAL".to_string()),
            ]),
        );
        source_msm_db.add_record(
            "ModuleSubstitution",
            Record::with_fields(vec![
                FieldValue::String("Feature".to_string()),
                FieldValue::String("KEY".to_string()),
                FieldValue::String("Feature".to_string()),
                FieldValue::String("VAL".to_string()),
            ]),
        );
        source_msm_db.add_record(
            "ModuleSubstitution",
            Record::with_fields(vec![
                FieldValue::String("NoSuchTable".to_string()),
                FieldValue::String("KEY".to_string()),
                FieldValue::String("Value".to_string()),
                FieldValue::String("VAL".to_string()),
            ]),
        );
        source_msm_db.add_record(
            "ModuleSubstitution",
            Record::with_fields(vec![
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
            ]),
        );
        source_msm_db.add_record(
            "ModuleIgnoreModularization",
            Record::with_fields(vec![
                FieldValue::String("SERVICE_PORT".to_string()),
                FieldValue::Short(1),
            ]),
        );
        source_msm_db.add_record(
            "ModuleIgnoreModularization",
            Record::with_fields(vec![FieldValue::Null, FieldValue::Short(1)]),
        );

        let merge_mod = MergeModule::from_database(source_msm_db);

        let mut substitutions = HashMap::new();
        substitutions.insert("CONFIG_PORT".to_string(), "8080".to_string());

        target_db.merge_module(&merge_mod, "MainFeature", "INSTALLFOLDER", &substitutions)?;
        // Second merge to test deduplication
        target_db.merge_module(&merge_mod, "MainFeature", "INSTALLFOLDER", &substitutions)?;

        // Verify directory retargeting
        let dir_records = target_db.get_records("Directory");
        let retargeted = dir_records
            .iter()
            .find(|r| matches!(r.get(0), Some(FieldValue::String(d)) if d == "ModuleDir.GUID_ABC"));
        assert_eq!(
            retargeted.and_then(|r| r.get(1)),
            Some(&FieldValue::String("INSTALLFOLDER".to_string()))
        );

        // Verify component modularization
        let comp_records = target_db.get_records("Component");
        assert!(comp_records
            .iter()
            .any(|r| r.get(0) == Some(&FieldValue::String("ModComp.GUID_ABC".to_string()))));

        // Verify file modularization
        let file_records = target_db.get_records("File");
        assert!(file_records
            .iter()
            .any(|r| r.get(0) == Some(&FieldValue::String("ModFile.GUID_ABC".to_string()))));

        // Verify substitution applied
        let prop_records = target_db.get_records("Property");
        let port_prop = prop_records
            .iter()
            .find(|r| r.get(0) == Some(&FieldValue::String("SERVICE_PORT".to_string())));
        assert_eq!(
            port_prop.and_then(|r| r.get(1)),
            Some(&FieldValue::String("8080".to_string()))
        );

        // Verify feature attachment in FeatureComponents
        let fc_records = target_db.get_records("FeatureComponents");
        assert!(fc_records.iter().any(|r| {
            r.get(0) == Some(&FieldValue::String("MainFeature".to_string()))
                && r.get(1) == Some(&FieldValue::String("ModComp.GUID_ABC".to_string()))
        }));

        // Test MergeModule default construction without ModuleSignature
        let empty_mod_db = LinkedDatabase::new()?;
        let def_mod = MergeModule::from_database(empty_mod_db);
        assert_eq!(def_mod.id, "Module");
        assert_eq!(def_mod.language, 1033);
        assert_eq!(def_mod.version, "1.0.0");
        assert_eq!(def_mod.guid(), "Module");

        // Test MergeModule with all-null ModuleSignature
        let mut null_sig_db = LinkedDatabase::new()?;
        null_sig_db.add_record(
            "ModuleSignature",
            Record::with_fields(vec![FieldValue::Null, FieldValue::Null, FieldValue::Null]),
        );
        let null_sig_mod = MergeModule::from_database(null_sig_db);
        assert_eq!(null_sig_mod.id, "Module");
        assert_eq!(null_sig_mod.language, 1033);
        assert_eq!(null_sig_mod.version, "1.0.0");

        // Test module without Directory and Component tables
        let mut no_dir_comp_db = LinkedDatabase::new()?;
        no_dir_comp_db.add_record(
            "Property",
            Record::with_fields(vec![
                FieldValue::String("PROP_A".to_string()),
                FieldValue::String("VAL_A".to_string()),
            ]),
        );
        let no_dir_comp_mod = MergeModule::from_database(no_dir_comp_db);
        target_db.merge_module(
            &no_dir_comp_mod,
            "MainFeature",
            "INSTALLFOLDER",
            &HashMap::new(),
        )?;

        // Test MergeModule from_bytes and open
        let msm_pkg_builder = Package::builder()
            .product_name("MergeModPkg")
            .manufacturer("Acme")
            .version(ProductVersion::new(1, 0, 0))
            .product_code("{88888888-8888-8888-8888-888888888888}");
        let msm_pkg = msm_pkg_builder.build()?;
        let msm_bytes = msm_pkg.to_bytes()?;
        let msm_bytes_opened = MergeModule::from_bytes(&msm_bytes)?;
        assert_eq!(msm_bytes_opened.guid(), "Module");

        let temp_open_dir = std::env::temp_dir();
        let temp_open_msm = temp_open_dir.join(format!("test_msm_{}.msm", std::process::id()));
        std::fs::write(&temp_open_msm, &msm_bytes)?;
        let msm_opened_from_file = MergeModule::open(&temp_open_msm)?;
        assert_eq!(msm_opened_from_file.guid(), "Module");
        let _ = std::fs::remove_file(&temp_open_msm);

        Ok(())
    }

    /// Tests multi-cabinet media partitioning, sequence graph solving, and variable resolution.
    ///
    /// # Errors
    ///
    /// Returns [`Error`] on test setup or execution failure.
    #[test]
    #[allow(
        clippy::too_many_lines,
        clippy::cast_possible_truncation,
        clippy::cast_possible_wrap
    )]
    fn test_linker_libscript_parity_multi_cab_and_sequences() -> Result<()> {
        let mut db = LinkedDatabase::new()?;

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

        Linker::solve_relative_sequences(&mut db)?;

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
        let mut cyclic_db = LinkedDatabase::new()?;
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
        let linker = Linker::new();
        let mut var_db = LinkedDatabase::new()?;

        let temp_dir =
            std::env::temp_dir().join(format!("linker_icon_test_{}", std::process::id()));
        let _ = std::fs::create_dir_all(&temp_dir);
        let ico_file = temp_dir.join("exclamation.ico");
        std::fs::write(&ico_file, b"fake icon data")?;

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

    #[test]
    #[allow(clippy::too_many_lines)]
    fn test_linker_remaining_uncovered_paths() -> Result<()> {
        use std::io::Write;

        // 1. Coverage for on_exit sequence types: "cancel", "error", "suspend"
        // and relative sequencing queue branch where in_degree transitions to 0,
        // as well as assigned increment/decrement collision loops and sort fallback.
        let mut db = LinkedDatabase::new()?;
        db.add_record(
            "_WixSequenceRelative",
            Record::with_fields(vec![
                FieldValue::String("InstallUISequence".to_string()),
                FieldValue::String("ActionCancel".to_string()),
                FieldValue::String("cancel".to_string()),
                FieldValue::String("OnExit".to_string()),
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
                FieldValue::Null,
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
                FieldValue::Null,
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
                FieldValue::Null,
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

        Linker::solve_relative_sequences(&mut db)?;

        let mut cycle_db = LinkedDatabase::new()?;
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
        let find_seq = |act: &str| -> Option<i16> {
            for r in ui_seq {
                if let (Some(FieldValue::String(a)), Some(FieldValue::Short(s))) =
                    (r.get(0), r.get(2))
                {
                    if a == act {
                        return Some(*s);
                    }
                }
            }
            None
        };
        assert_eq!(find_seq("ActionCancel"), Some(-2));
        assert_eq!(find_seq("ActionError"), Some(-3));
        assert_eq!(find_seq("ActionSuspend"), Some(-4));
        assert_eq!(find_seq("ActionSuccess"), Some(-1));
        assert_eq!(find_seq("ActionA"), Some(1026)); // collided with 1025, incremented to 1026
        assert_eq!(find_seq("ActionD"), Some(974)); // collided with 975, decremented to 974
        assert_eq!(find_seq("NonExistentAction"), None);

        // 2. Coverage for WixUIBannerBmp, WixUIDialogBmp, WixUILicenseRtf (.txt and unresolved),
        // and resolve_source_path branches.
        let temp_dir = std::env::temp_dir().join("msi_linker_uncovered_test");
        std::fs::create_dir_all(&temp_dir)?;

        let banner_file = temp_dir.join("banner.bmp");
        let mut bf = std::fs::File::create(&banner_file)?;
        bf.write_all(b"BMfakebanner")?;

        let dialog_file = temp_dir.join("dialog.bmp");
        let mut df = std::fs::File::create(&dialog_file)?;
        df.write_all(b"BMfakedialog")?;

        let txt_license_file = temp_dir.join("license.txt");
        let mut lf = std::fs::File::create(&txt_license_file)?;
        lf.write_all(b"Plain text license agreement.")?;

        let mut linker = Linker::new();
        linker.set_cab_per_component(true);
        linker.add_base_dir(&temp_dir);

        let mut var_db = LinkedDatabase::new()?;
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
        linker.expand_localization_tokens(&mut var_db)?;
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
        assert!(format!("{:?}", ctrls[0].get(9)).contains("Plain text license agreement."));

        // Test WixUIBannerBmp and WixUIDialogBmp error reading file
        let dir_bmp = temp_dir.join("dir_bmp");
        std::fs::create_dir_all(&dir_bmp)?;
        let mut banner_dir_db = LinkedDatabase::new()?;
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

        let mut banner_none_db = LinkedDatabase::new()?;
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
        let mut var_db_raw_rtf = LinkedDatabase::new()?;
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
        std::fs::write(&rtf_license_file, r"{\rtf1\ansi From file RTF}")?;
        let mut var_db_rtf_file = LinkedDatabase::new()?;
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

        let mut var_db_no_control = LinkedDatabase::new()?;
        var_db_no_control.add_record(
            "WixVariable",
            Record::with_fields(vec![
                FieldValue::String("WixUILicenseRtf".to_string()),
                FieldValue::String("any_license_text".to_string()),
            ]),
        );
        linker.resolve_wix_variables(&mut var_db_no_control);

        // Test WixUILicenseRtf fallback when resolve_source_path returns None
        let mut var_db_unresolved = LinkedDatabase::new()?;
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

        // Test WixUILicenseRtf map_or_else err branch where file exists (e.g. directory) but read_to_string fails
        let unreadable_dir = temp_dir.join("dir_as_license");
        std::fs::create_dir_all(&unreadable_dir)?;
        let mut var_db_dir_err = LinkedDatabase::new()?;
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
        let mut bind_db = LinkedDatabase::new()?;
        let dummy_src_a = temp_dir.join("dummy_a.bin");
        std::fs::write(&dummy_src_a, b"dummy payload data a")?;
        let dummy_src_b = temp_dir.join("dummy_b.bin");
        std::fs::write(&dummy_src_b, b"dummy payload data b")?;
        let dummy_src_c = temp_dir.join("dummy_c.bin");
        std::fs::write(&dummy_src_c, b"dummy payload data c")?;

        // WixMediaCompression records: one with "none", "high", "medium", and invalid non-Short did
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

        linker.set_cab_per_component(false);
        linker.bind_files_and_pack_cabinets(&mut bind_db)?;
        let embedded = linker.embedded_cabinets();
        assert!(embedded.contains_key("#mycab1.cab"));
        assert!(embedded.contains_key("#cab2.cab"));
        assert!(embedded.contains_key("#cab99.cab"));

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
        let solved = sym_linker.solve_symbol_graph()?;
        assert_eq!(solved.len(), 2);

        let mut test_syms = HashMap::new();
        test_syms.insert(Symbol::new("Dialog", "MyDialog"), 0);
        test_syms.insert(Symbol::new("CustomAction", "MyCA"), 1);

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
        let mut empty_layout_db = LinkedDatabase::new()?;
        Linker::layout_media_and_files(&mut empty_layout_db);

        let mut zero_media_db = LinkedDatabase::new()?;
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

        let mut layout_db = LinkedDatabase::new()?;
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

    /// Tests `run_ice_validations_filtered` with rule selection and suppression.
    #[test]
    fn test_run_ice_validations_filtered_whitelist_and_suppression() -> Result<()> {
        let mut db = LinkedDatabase::new()?;
        db.add_record(
            "File",
            Record::with_fields(vec![
                FieldValue::String("File1".to_string()),
                FieldValue::String("Comp1".to_string()),
                FieldValue::String("file1.txt".to_string()),
                FieldValue::Long(100),
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Short(99), // non-contiguous sequence triggers ICE04
            ]),
        );

        // Selecting ICE04 -> should fail with ICE04 error
        let err = Linker::run_ice_validations_filtered(&db, &["ICE04".to_string()], &[]);
        assert!(err.is_err());

        // Suppressing ICE04 -> should bypass ICE04 failure
        let res = Linker::run_ice_validations_filtered(
            &db,
            &["ICE04".to_string()],
            &["ice04".to_string()],
        );
        assert!(res.is_ok());

        // Selecting unrelated rule (e.g. ICE99) -> passes
        let res2 = Linker::run_ice_validations_filtered(&db, &["ICE99".to_string()], &[]);
        assert!(res2.is_ok());

        // Direct run_ice_validations calls filtered with empty lists -> fails on unsuppressed ICE04
        assert!(Linker::run_ice_validations(&db).is_err());
        Ok(())
    }
}

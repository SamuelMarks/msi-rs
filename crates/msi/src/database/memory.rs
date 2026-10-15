//! In-memory relational database representation (`LinkedDatabase`) and view mutation modes.

use crate::database::catalogs::DatabaseCatalog;
use crate::database::tables::record::{FieldValue, Record};
use crate::error::{MsiError, Result};
use crate::execution::script_engine::ScriptDatabase;
use crate::wix::merge_module::{modularize_identifier, MergeModule};
use std::collections::{HashMap, HashSet};

/// Strongly-typed Windows Installer view modification mode (`MSIMODIFY_*`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModifyMode {
    /// Refreshes the record using primary keys (`MSIMODIFY_SEEK = -1`).
    Seek = -1,
    /// Refreshes the current record (`MSIMODIFY_REFRESH = 0`).
    Refresh = 0,
    /// Inserts a new record (`MSIMODIFY_INSERT = 1`).
    Insert = 1,
    /// Updates an existing record (`MSIMODIFY_UPDATE = 2`).
    Update = 2,
    /// Inserts or updates a record (`MSIMODIFY_ASSIGN = 3`).
    Assign = 3,
    /// Replaces an existing record or inserts if absent (`MSIMODIFY_REPLACE = 4`).
    Replace = 4,
    /// Merges a record if identical, or fails on conflict (`MSIMODIFY_MERGE = 5`).
    Merge = 5,
    /// Deletes an existing record (`MSIMODIFY_DELETE = 6`).
    Delete = 6,
    /// Inserts a temporary record (`MSIMODIFY_INSERT_TEMPORARY = 7`).
    InsertTemporary = 7,
    /// Validates an existing record against schema constraints (`MSIMODIFY_VALIDATE = 8`).
    Validate = 8,
    /// Validates a new record before insertion (`MSIMODIFY_VALIDATE_NEW = 9`).
    ValidateNew = 9,
    /// Validates fields of an existing record (`MSIMODIFY_VALIDATE_FIELD = 10`).
    ValidateField = 10,
    /// Validates whether a record can be deleted (`MSIMODIFY_VALIDATE_DELETE = 11`).
    ValidateDelete = 11,
}

impl TryFrom<i32> for ModifyMode {
    type Error = MsiError;

    fn try_from(value: i32) -> Result<Self> {
        match value {
            -1 => Ok(Self::Seek),
            0 => Ok(Self::Refresh),
            1 => Ok(Self::Insert),
            2 => Ok(Self::Update),
            3 => Ok(Self::Assign),
            4 => Ok(Self::Replace),
            5 => Ok(Self::Merge),
            6 => Ok(Self::Delete),
            7 => Ok(Self::InsertTemporary),
            8 => Ok(Self::Validate),
            9 => Ok(Self::ValidateNew),
            10 => Ok(Self::ValidateField),
            11 => Ok(Self::ValidateDelete),
            _ => Err(MsiError::WixLinker {
                message: format!("Unsupported modify mode: {value}"),
            }),
        }
    }
}

/// Linked database representation holding resolved tables and records.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LinkedDatabase {
    /// In-memory table contents mapping table name to list of records.
    pub tables: HashMap<String, Vec<Record>>,
    /// Database catalog describing table schemas.
    pub catalog: DatabaseCatalog,
    /// Embedded sub-storages extracted from the CFB container (e.g., embedded transforms).
    pub embedded_storages: HashMap<String, Vec<u8>>,
}

impl Default for LinkedDatabase {
    /// Creates a default empty [`LinkedDatabase`] with all standard schemas pre-populated.
    ///
    /// # Returns
    ///
    /// The resulting value or `Ok(())` on success.
    fn default() -> Self {
        let mut catalog = DatabaseCatalog::new();
        let _ = crate::database::tables::populate_standard_tables(&mut catalog);
        Self {
            tables: HashMap::new(),
            catalog,
            embedded_storages: HashMap::new(),
        }
    }
}

#[allow(clippy::multiple_inherent_impl)]
impl LinkedDatabase {
    /// Creates a new empty [`LinkedDatabase`] with all standard schemas pre-populated.
    ///
    /// # Returns
    ///
    /// A new [`LinkedDatabase`].
    ///
    /// # Errors
    ///
    /// Returns [`crate::MsiError`] if catalog population fails.
    pub fn new() -> Result<Self> {
        Ok(Self::default())
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

    /// Adds or merges a record into the specified table, resolving primary key collisions
    /// and deduplicating identical records across fragments.
    ///
    /// # Arguments
    ///
    /// * `table` - Target table name.
    /// * `record` - Record to insert or merge.
    ///
    /// # Errors
    ///
    /// Returns [`MsiError::WixLinker`] on conflicting primary key duplicate records.
    ///
    /// # Returns
    ///
    /// The resulting value or `Ok(())` on success.
    pub fn add_or_merge_record(&mut self, table: &str, record: Record) -> Result<()> {
        let records = self.tables.entry(table.to_string()).or_default();

        if let Some(schema) = self.catalog.get_table(table) {
            let pk_indices: Vec<usize> = schema
                .columns
                .iter()
                .enumerate()
                .filter_map(|(i, col)| col.primary_key.then_some(i))
                .collect();

            if pk_indices.is_empty() {
                if !records.contains(&record) {
                    records.push(record);
                }
                return Ok(());
            }

            let is_seq_table = table.ends_with("Sequence");

            // Check for existing record matching on primary key
            for existing in records.iter_mut() {
                let matches_pk = pk_indices
                    .iter()
                    .all(|&idx| existing.get(idx) == record.get(idx))
                    && (!is_seq_table || existing.get(1) == record.get(1));
                if matches_pk {
                    if table == "Property" {
                        // Product / later fragments override Property value
                        *existing = record;
                        return Ok(());
                    }
                    if existing == &record {
                        // Exactly identical record (e.g. TARGETDIR Directory definition across fragments)
                        return Ok(());
                    }
                    return Err(MsiError::WixLinker {
                        message: format!(
                            "primary key collision in table '{table}': conflicting record definitions for key {:?}",
                            pk_indices.iter().map(|&i| record.get(i)).collect::<Vec<_>>()
                        ),
                    });
                }
            }

            records.push(record);
        } else if !records.contains(&record) {
            // Temporary internal tables (e.g. _ComponentGroupMember)
            records.push(record);
        }

        Ok(())
    }

    /// Executes a dynamic database mutation (Insert, Update, Delete, Validate) from FFI.
    ///
    /// # Arguments
    ///
    /// * `table` - Target table name.
    /// * `record` - Record containing values.
    /// * `modify_mode` - Numeric `MSIMODIFY_*` mode code (`-1..=11`).
    ///
    /// # Errors
    ///
    /// Returns [`MsiError::WixLinker`] if a primary key conflict, schema validation error, or unsupported mode occurs.
    ///
    /// # Returns
    ///
    /// `Ok(())` when the mutation or validation succeeds.
    pub fn execute_mutation(
        &mut self,
        table: &str,
        record: &mut Record,
        modify_mode: i32,
    ) -> Result<()> {
        let mode = ModifyMode::try_from(modify_mode)?;
        let schema_opt = self.catalog.get_table(table).cloned();
        let records = self.tables.entry(table.to_string()).or_default();

        let pk_indices = schema_opt
            .as_ref()
            .map(|schema| {
                schema
                    .columns
                    .iter()
                    .enumerate()
                    .filter_map(|(i, col)| col.primary_key.then_some(i))
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();

        let find_existing = |rows: &mut [Record]| -> Option<usize> {
            if pk_indices.is_empty() {
                return None;
            }
            rows.iter().position(|existing| {
                pk_indices
                    .iter()
                    .all(|&idx| existing.get(idx) == record.get(idx))
            })
        };

        match mode {
            ModifyMode::Seek | ModifyMode::Refresh => {
                if let Some(idx) = find_existing(records) {
                    *record = records[idx].clone();
                } else {
                    return Err(MsiError::WixLinker {
                        message: format!("MSIMODIFY_REFRESH: record not found in table '{table}'"),
                    });
                }
            }
            ModifyMode::Insert | ModifyMode::InsertTemporary => {
                if find_existing(records).is_some() {
                    return Err(MsiError::WixLinker {
                        message: format!(
                            "MSIMODIFY_INSERT: primary key collision in table '{table}'"
                        ),
                    });
                }
                records.push(record.clone());
            }
            ModifyMode::Update => {
                if let Some(idx) = find_existing(records) {
                    records[idx] = record.clone();
                } else {
                    return Err(MsiError::WixLinker {
                        message: format!("MSIMODIFY_UPDATE: record not found in table '{table}'"),
                    });
                }
            }
            ModifyMode::Assign | ModifyMode::Replace => {
                if let Some(idx) = find_existing(records) {
                    records[idx] = record.clone();
                } else {
                    records.push(record.clone());
                }
            }
            ModifyMode::Merge => {
                if let Some(idx) = find_existing(records) {
                    if records[idx] != *record {
                        return Err(MsiError::WixLinker {
                            message: format!("MSIMODIFY_MERGE: record mismatch in table '{table}'"),
                        });
                    }
                } else {
                    records.push(record.clone());
                }
            }
            ModifyMode::Delete => {
                if let Some(idx) = find_existing(records) {
                    records.remove(idx);
                } else {
                    return Err(MsiError::WixLinker {
                        message: format!("MSIMODIFY_DELETE: record not found in table '{table}'"),
                    });
                }
            }
            ModifyMode::Validate
            | ModifyMode::ValidateNew
            | ModifyMode::ValidateField
            | ModifyMode::ValidateDelete => {
                if let Some(schema) = schema_opt {
                    for (idx, col) in schema.columns.iter().enumerate() {
                        if !col.nullable && matches!(record.get(idx), Some(FieldValue::Null)) {
                            return Err(MsiError::WixLinker {
                                message: format!(
                                    "MSIMODIFY_VALIDATE: non-nullable column '{}' is null in table '{table}'",
                                    col.name
                                ),
                            });
                        }
                    }
                }
            }
        }
        Ok(())
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

    /// Returns an optional slice of records for the given table, or `None` if the table does not exist.
    ///
    /// # Arguments
    ///
    /// * `name` - Table name.
    ///
    /// # Returns
    ///
    /// Optional slice of [`Record`].
    #[must_use]
    pub fn get_table(&self, name: &str) -> Option<&[Record]> {
        self.tables.get(name).map(Vec::as_slice)
    }

    /// Extracts a string field from a record at the specified column index.
    ///
    /// # Arguments
    ///
    /// * `record` - Target record.
    /// * `col` - 0-based column index.
    ///
    /// # Returns
    ///
    /// Optional borrowed string slice if column exists and contains a string.
    #[must_use]
    pub fn get_string_field<'a>(&self, record: &'a Record, col: usize) -> Option<&'a str> {
        match record.get(col) {
            Some(FieldValue::String(s)) => Some(s.as_str()),
            _ => None,
        }
    }

    /// Extracts an integer field from a record at the specified column index.
    ///
    /// # Arguments
    ///
    /// * `record` - Target record.
    /// * `col` - 0-based column index.
    ///
    /// # Returns
    ///
    /// Optional 32-bit signed integer if column exists and contains an integer.
    #[must_use]
    pub fn get_int_field(&self, record: &Record, col: usize) -> Option<i32> {
        match record.get(col) {
            Some(FieldValue::Short(s)) => Some(i32::from(*s)),
            Some(FieldValue::Long(l)) => Some(*l),
            _ => None,
        }
    }

    /// Returns the primary key column names for the specified table.
    ///
    /// # Arguments
    ///
    /// * `table` - Target table name.
    ///
    /// # Returns
    ///
    /// Vector of primary key column names.
    #[must_use]
    pub fn get_primary_keys(&self, table: &str) -> Vec<String> {
        self.catalog
            .get_table(table)
            .map_or_else(Vec::new, |schema| {
                schema
                    .columns
                    .iter()
                    .filter(|c| c.primary_key)
                    .map(|c| c.name.clone())
                    .collect()
            })
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
    /// Returns [`crate::MsiError`] on table schema or primary key merge collisions.
    ///
    /// # Returns
    ///
    /// The resulting value or `Ok(())` on success.
    #[allow(clippy::cognitive_complexity)]
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

        // Parse ModuleConfiguration for parameter defaults
        let mut final_substitutions = substitutions.clone();
        for r in mod_db.get_records("ModuleConfiguration") {
            if let (Some(FieldValue::String(name)), Some(FieldValue::String(default_val))) =
                (r.get(0), r.get(4))
            {
                final_substitutions
                    .entry(name.clone())
                    .or_insert_with(|| default_val.clone());
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
                for (param, val) in &final_substitutions {
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
            let is_sequence = table_name == "ModuleInstallExecuteSequence"
                || table_name == "ModuleInstallUISequence"
                || table_name == "ModuleAdminExecuteSequence"
                || table_name == "ModuleAdminUISequence"
                || table_name == "ModuleAdvtExecuteSequence"
                || table_name == "ModuleAdvtUISequence";

            let target_table_name = if is_sequence {
                table_name
                    .strip_prefix("Module")
                    .unwrap_or(&table_name)
                    .to_string()
            } else {
                table_name.clone()
            };

            // Skip other merge module system tables
            if !is_sequence && table_name.starts_with("Module") {
                continue;
            }

            let dest_rows = self.tables.entry(target_table_name).or_default();

            for mut record in rows {
                match table_name.as_str() {
                    "ModuleInstallExecuteSequence"
                    | "ModuleInstallUISequence"
                    | "ModuleAdminExecuteSequence"
                    | "ModuleAdminUISequence"
                    | "ModuleAdvtExecuteSequence"
                    | "ModuleAdvtUISequence" => {
                        if let Some(FieldValue::String(ref s)) = record.get(0) {
                            let m = modularize_identifier(s, module_guid, &ignore_set);
                            record.set(0, FieldValue::String(m));
                        }

                        let base_action = match record.get(2) {
                            Some(FieldValue::String(s)) => s.clone(),
                            _ => String::new(),
                        };
                        let after = match record.get(3) {
                            Some(&FieldValue::Short(a)) => a != 0,
                            _ => false,
                        };

                        if !base_action.is_empty() {
                            let mut base_seq = None;
                            for dr in dest_rows.iter() {
                                if dr.get(0) == Some(&FieldValue::String(base_action.clone())) {
                                    if let Some(&FieldValue::Short(s)) = dr.get(2) {
                                        // Sequence is index 2 in target
                                        base_seq = Some(s);
                                    }
                                    break;
                                }
                            }
                            if let Some(s) = base_seq {
                                let new_seq = if after { s + 1 } else { s - 1 };
                                record.set(1, FieldValue::Short(new_seq));
                            }
                        }

                        let action = record.get(0).cloned().unwrap_or(FieldValue::Null);
                        let condition = record.get(4).cloned().unwrap_or(FieldValue::Null);
                        let sequence = record.get(1).cloned().unwrap_or(FieldValue::Short(1));

                        record = Record::with_fields(vec![action, condition, sequence]);
                    }
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

#[cfg(test)]
mod tests {
    use super::*;

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

        // SEEK (-1), ASSIGN (3), INSERT_TEMPORARY (7)
        assert!(db
            .execute_mutation("Property", &mut rec1.clone(), -1)
            .is_ok());
        assert!(db
            .execute_mutation("Property", &mut rec3.clone(), -1)
            .is_err());
        assert!(db
            .execute_mutation("Property", &mut rec1.clone(), 3)
            .is_ok());
        assert!(db
            .execute_mutation("Property", &mut rec3.clone(), 7)
            .is_ok());
        assert!(db
            .execute_mutation("Property", &mut rec3.clone(), 7)
            .is_err());

        // VALIDATE (8..=11): valid record and null non-nullable column failure
        assert!(db
            .execute_mutation("Property", &mut rec1.clone(), 8)
            .is_ok());
        assert!(db
            .execute_mutation("Property", &mut rec1.clone(), 9)
            .is_ok());
        assert!(db
            .execute_mutation("Property", &mut rec1.clone(), 10)
            .is_ok());
        assert!(db
            .execute_mutation("Property", &mut rec1.clone(), 11)
            .is_ok());
        let mut rec_null_pk = Record::with_fields(vec![
            FieldValue::Null,
            FieldValue::String("Val".to_string()),
        ]);
        assert!(db
            .execute_mutation("Property", &mut rec_null_pk, 8)
            .is_err());

        // INVALID mode
        assert!(db.execute_mutation("Property", &mut rec1, 99).is_err());

        // No primary keys defined (e.g. some internal table)
        db.tables.insert("NoPKTable".to_string(), vec![]);
        let mut rec_nopk = Record::with_fields(vec![FieldValue::String("X".to_string())]);
        assert!(db
            .execute_mutation("NoPKTable", &mut rec_nopk.clone(), 1)
            .is_ok());
        assert!(db
            .execute_mutation("NoPKTable", &mut rec_nopk.clone(), 8)
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
}

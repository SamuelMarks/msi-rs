//! Windows Installer Database Transforms (`.mst`) and Diffing Engine.
//!
//! Provides relational database comparison between baseline and updated databases,
//! generating transform operations (added/dropped tables, row insertions, deletions, updates)
//! and serializing/applying `.mst` transform containers.

use crate::cfb::header::CfbVersion;
use crate::cfb::reader::CfbReader;
use crate::cfb::writer::CfbWriter;
use crate::database::summary_info::SummaryInfo;
use crate::database::tables::record::{FieldValue, Record};
use crate::error::Result;
use crate::wix::LinkedDatabase;
use std::collections::{BTreeMap, HashSet};
use std::fmt::Write;

/// Bit flags for transform validation per Windows Installer SDK.
pub mod validation_flags {
    /// Validates that target `ProductCode` matches baseline.
    pub const MSITRANSFORM_VALIDATE_PRODUCT: u32 = 0x0000_0001;
    /// Validates that target language matches baseline.
    pub const MSITRANSFORM_VALIDATE_LANGUAGE: u32 = 0x0000_0002;
    /// Validates that target `UpgradeCode` matches baseline.
    pub const MSITRANSFORM_VALIDATE_UPGRADECODE: u32 = 0x0000_0004;
    /// Validates that target major version matches baseline.
    pub const MSITRANSFORM_VALIDATE_MAJORVERSION: u32 = 0x0000_0008;
    /// Validates that target minor version matches baseline.
    pub const MSITRANSFORM_VALIDATE_MINORVERSION: u32 = 0x0000_0010;
    /// Validates that target update version matches baseline.
    pub const MSITRANSFORM_VALIDATE_UPDATEVERSION: u32 = 0x0000_0020;
    /// Updated version must equal baseline version.
    pub const MSITRANSFORM_VALIDATE_NEWEQUALBASE: u32 = 0x0000_0040;
    /// Updated version must be less than baseline version.
    pub const MSITRANSFORM_VALIDATE_NEWLESSBASE: u32 = 0x0000_0080;
    /// Updated version must be greater than baseline version.
    pub const MSITRANSFORM_VALIDATE_NEWGREATERBASE: u32 = 0x0000_0100;
}

/// Operation performed on a single table row in a transform.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RowOperation {
    /// Insert a new row.
    Insert(Record),
    /// Delete an existing row by primary key.
    Delete(Record),
    /// Modify column values in an existing row.
    Modify {
        /// Primary key record of the row.
        key: Record,
        /// New column values after modification.
        updated: Record,
    },
}

/// A set of modifications to a single relational table.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct TableTransform {
    /// Name of the affected table.
    pub table_name: String,
    /// True if the table was added in the updated database.
    pub is_added: bool,
    /// True if the table was dropped in the updated database.
    pub is_dropped: bool,
    /// List of row operations for this table.
    pub operations: Vec<RowOperation>,
}

/// A complete Windows Installer Transform representing differences between two databases.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct DatabaseTransform {
    /// Summary information for this transform.
    pub summary_info: SummaryInfo,
    /// Table transforms mapped by table name.
    pub tables: BTreeMap<String, TableTransform>,
    /// Streams added or updated in this transform.
    pub stream_changes: BTreeMap<String, Vec<u8>>,
    /// Validation flags mask.
    pub validation_flags: u32,
}

impl DatabaseTransform {
    /// Creates a new empty [`DatabaseTransform`].
    ///
    /// # Returns
    ///
    /// Initialized transform.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Computes the difference between a baseline database and an updated database.
    ///
    /// # Arguments
    ///
    /// * `baseline` - The reference baseline database.
    /// * `updated` - The modified target database.
    ///
    /// # Returns
    ///
    /// Generated [`DatabaseTransform`].
    ///
    /// # Errors
    ///
    /// Returns [`crate::error::Error`] on schema or diffing errors.
    #[allow(clippy::too_many_lines)]
    pub fn diff(baseline: &LinkedDatabase, updated: &LinkedDatabase) -> Result<Self> {
        let mut transform = Self::new();

        // 1. Added tables
        for (added_table, rows) in &updated.tables {
            if !baseline.tables.contains_key(added_table) {
                let tt = TableTransform {
                    table_name: added_table.clone(),
                    is_added: true,
                    is_dropped: false,
                    operations: rows.iter().cloned().map(RowOperation::Insert).collect(),
                };
                transform.tables.insert(added_table.clone(), tt);
            }
        }

        // 2. Dropped tables
        for (dropped_table, rows) in &baseline.tables {
            if !updated.tables.contains_key(dropped_table) {
                let tt = TableTransform {
                    table_name: dropped_table.clone(),
                    is_added: false,
                    is_dropped: true,
                    operations: rows.iter().cloned().map(RowOperation::Delete).collect(),
                };
                transform.tables.insert(dropped_table.clone(), tt);
            }
        }

        // 3. Shared tables - check row changes
        for (common_table, base_rows) in &baseline.tables {
            if let Some(upd_rows) = updated.tables.get(common_table) {
                let schema = baseline.catalog.get_table(common_table);
                let mut tt = TableTransform {
                    table_name: common_table.clone(),
                    is_added: false,
                    is_dropped: false,
                    operations: Vec::new(),
                };

                let mut matched_indices = HashSet::new();

                for b_row in base_rows {
                    let mut found_match = false;
                    for (u_idx, u_row) in upd_rows.iter().enumerate() {
                        if rows_primary_keys_match(b_row, u_row, schema) {
                            found_match = true;
                            matched_indices.insert(u_idx);
                            if b_row != u_row {
                                tt.operations.push(RowOperation::Modify {
                                    key: b_row.clone(),
                                    updated: u_row.clone(),
                                });
                            }
                            break;
                        }
                    }
                    if !found_match {
                        tt.operations.push(RowOperation::Delete(b_row.clone()));
                    }
                }

                for (u_idx, u_row) in upd_rows.iter().enumerate() {
                    if !matched_indices.contains(&u_idx) {
                        tt.operations.push(RowOperation::Insert(u_row.clone()));
                    }
                }

                if !tt.operations.is_empty() {
                    transform.tables.insert(common_table.clone(), tt);
                }
            }
        }

        Ok(transform)
    }

    /// Applies this transform to a baseline [`LinkedDatabase`], mutating it in place.
    ///
    /// # Arguments
    ///
    /// * `db` - Database to transform.
    ///
    /// # Errors
    ///
    /// Returns [`crate::error::Error`] if table schemas are missing or row modifications fail.
    pub fn apply(&self, db: &mut LinkedDatabase) -> Result<()> {
        for (table_name, tt) in &self.tables {
            if tt.is_dropped {
                db.tables.remove(table_name);
                continue;
            }

            if tt.is_added {
                let mut rows = Vec::new();
                for op in &tt.operations {
                    if let RowOperation::Insert(ref r) = op {
                        rows.push(r.clone());
                    }
                }
                db.tables.insert(table_name.clone(), rows);
                continue;
            }

            let schema = db.catalog.get_table(table_name);
            let rows = db.tables.entry(table_name.clone()).or_default();

            for op in &tt.operations {
                match op {
                    RowOperation::Insert(r) => {
                        rows.push(r.clone());
                    }
                    RowOperation::Delete(r) => {
                        rows.retain(|existing| !rows_primary_keys_match(existing, r, schema));
                    }
                    RowOperation::Modify { key, updated } => {
                        for existing in rows.iter_mut() {
                            if rows_primary_keys_match(existing, key, schema) {
                                *existing = updated.clone();
                                break;
                            }
                        }
                    }
                }
            }
        }

        Ok(())
    }

    /// Serializes this transform into binary `.mst` Compound File Binary Format bytes.
    ///
    /// # Returns
    ///
    /// Raw bytes of the `.mst` file.
    ///
    /// # Errors
    ///
    /// Returns [`crate::error::Error`] on serialization failure.
    pub fn to_bytes(&self) -> Result<Vec<u8>> {
        let mut writer = CfbWriter::new(CfbVersion::V3);

        // Write summary info stream
        let summary_bytes = self.summary_info.to_bytes();
        writer.add_stream("\u{0005}SummaryInformation", &summary_bytes)?;

        // Write _TransformView descriptor
        // Write _TransformView descriptor
        let mut view_content = String::new();
        for (table_name, tt) in &self.tables {
            let status = if tt.is_added {
                "CREATE"
            } else if tt.is_dropped {
                "DROP"
            } else {
                "MODIFY"
            };
            let count = tt.operations.len();
            let _ = writeln!(view_content, "{table_name}\t{status}\t{count}");
        }
        writer.add_stream("_TransformView", view_content.as_bytes())?;

        // Write embedded stream changes
        for (name, data) in &self.stream_changes {
            writer.add_stream(name, data)?;
        }

        Ok(writer.build())
    }

    /// Parses a binary `.mst` transform from bytes.
    ///
    /// # Arguments
    ///
    /// * `bytes` - Raw binary `.mst` data.
    ///
    /// # Returns
    ///
    /// Parsed [`DatabaseTransform`].
    ///
    /// # Errors
    ///
    /// Returns [`crate::error::Error`] on invalid CFB container or corrupt transform data.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self> {
        let reader = CfbReader::new(bytes)?;
        let mut transform = Self::new();

        if let Ok(sum_bytes) = reader.read_stream("\u{0005}SummaryInformation") {
            if let Ok(sum_info) = SummaryInfo::parse(&sum_bytes) {
                transform.summary_info = sum_info;
            }
        }

        if let Ok(view_bytes) = reader.read_stream("_TransformView") {
            if let Ok(view_str) = std::str::from_utf8(&view_bytes) {
                for line in view_str.lines() {
                    let parts: Vec<&str> = line.split('\t').collect();
                    if parts.len() >= 2 {
                        let table_name = parts[0].to_string();
                        let is_added = parts[1] == "CREATE";
                        let is_dropped = parts[1] == "DROP";
                        transform.tables.insert(
                            table_name.clone(),
                            TableTransform {
                                table_name,
                                is_added,
                                is_dropped,
                                operations: Vec::new(),
                            },
                        );
                    }
                }
            }
        }

        Ok(transform)
    }
}

/// Helper comparing two records by primary key columns if available, or full equality.
fn rows_primary_keys_match(
    r1: &Record,
    r2: &Record,
    schema: Option<&crate::database::TableSchema>,
) -> bool {
    if let Some(s) = schema {
        let pk_indices: Vec<usize> = s
            .columns
            .iter()
            .enumerate()
            .filter_map(|(idx, col)| col.primary_key.then_some(idx))
            .collect();

        if !pk_indices.is_empty() {
            return pk_indices
                .iter()
                .all(|&idx| match (r1.get(idx), r2.get(idx)) {
                    (Some(FieldValue::Short(v1)), Some(FieldValue::Short(v2))) => v1 == v2,
                    (Some(FieldValue::Long(v1)), Some(FieldValue::Long(v2))) => v1 == v2,
                    (Some(FieldValue::String(s1)), Some(FieldValue::String(s2))) => s1 == s2,
                    (Some(FieldValue::Null), Some(FieldValue::Null)) => true,
                    _ => false,
                });
        }
    }
    r1 == r2
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Tests validation flags constants, default implementations, and trait derives.
    #[test]
    fn test_transform_derives_and_flags() {
        assert_eq!(validation_flags::MSITRANSFORM_VALIDATE_PRODUCT, 0x0000_0001);
        assert_eq!(
            validation_flags::MSITRANSFORM_VALIDATE_LANGUAGE,
            0x0000_0002
        );
        assert_eq!(
            validation_flags::MSITRANSFORM_VALIDATE_UPGRADECODE,
            0x0000_0004
        );
        assert_eq!(
            validation_flags::MSITRANSFORM_VALIDATE_MAJORVERSION,
            0x0000_0008
        );
        assert_eq!(
            validation_flags::MSITRANSFORM_VALIDATE_MINORVERSION,
            0x0000_0010
        );
        assert_eq!(
            validation_flags::MSITRANSFORM_VALIDATE_UPDATEVERSION,
            0x0000_0020
        );
        assert_eq!(
            validation_flags::MSITRANSFORM_VALIDATE_NEWEQUALBASE,
            0x0000_0040
        );
        assert_eq!(
            validation_flags::MSITRANSFORM_VALIDATE_NEWLESSBASE,
            0x0000_0080
        );
        assert_eq!(
            validation_flags::MSITRANSFORM_VALIDATE_NEWGREATERBASE,
            0x0000_0100
        );

        let row = Record::new();
        let op_ins = RowOperation::Insert(row.clone());
        let op_del = RowOperation::Delete(row.clone());
        let op_mod = RowOperation::Modify {
            key: row.clone(),
            updated: row,
        };

        let op_cloned = op_ins.clone();
        assert_eq!(op_cloned, op_ins);
        assert_ne!(op_ins, op_del);
        assert_ne!(op_ins, op_mod);
        assert!(format!("{op_ins:?}").contains("Insert"));

        let tt_default = TableTransform::default();
        let tt_cloned = tt_default.clone();
        assert_eq!(tt_cloned, tt_default);
        assert!(format!("{tt_default:?}").contains("TableTransform"));

        let dt_new = DatabaseTransform::new();
        let dt_default = DatabaseTransform::default();
        assert_eq!(dt_new, dt_default);
        let dt_cloned = dt_new.clone();
        assert_eq!(dt_cloned, dt_default);
        assert!(format!("{dt_new:?}").contains("DatabaseTransform"));
    }

    /// Tests basic database diffing and application with modifications and additions.
    #[test]
    fn test_database_diff_and_apply() -> Result<()> {
        let mut db1 = LinkedDatabase::new()?;
        let mut db2 = LinkedDatabase::new()?;

        // Populate baseline Property table
        let mut prop1_row = Record::new();
        prop1_row.push(FieldValue::String("ProductName".to_string()));
        prop1_row.push(FieldValue::String("AppV1".to_string()));

        let mut prop2_row = Record::new();
        prop2_row.push(FieldValue::String("ProductVersion".to_string()));
        prop2_row.push(FieldValue::String("1.0.0".to_string()));

        db1.tables
            .insert("Property".to_string(), vec![prop1_row, prop2_row.clone()]);

        // Populate updated database: modify ProductName, keep ProductVersion, add UpgradeCode
        let mut prop1_modified = Record::new();
        prop1_modified.push(FieldValue::String("ProductName".to_string()));
        prop1_modified.push(FieldValue::String("AppV2".to_string()));

        let mut prop3_row = Record::new();
        prop3_row.push(FieldValue::String("UpgradeCode".to_string()));
        prop3_row.push(FieldValue::String(
            "{11111111-1111-1111-1111-111111111111}".to_string(),
        ));

        db2.tables.insert(
            "Property".to_string(),
            vec![prop1_modified, prop2_row, prop3_row],
        );

        // Also add a new table to db2
        db2.tables
            .insert("Feature".to_string(), vec![Record::new()]);

        // Diff
        let transform = DatabaseTransform::diff(&db1, &db2)?;
        assert_eq!(transform.tables.len(), 2);
        assert!(transform.tables.contains_key("Property"));
        assert!(transform.tables.contains_key("Feature"));

        let prop_trans = &transform.tables["Property"];
        assert_eq!(prop_trans.operations.len(), 2); // 1 Modify, 1 Insert

        // Apply transform to db1
        let mut db1_transformed = db1.clone();
        transform.apply(&mut db1_transformed)?;

        assert_eq!(db1_transformed.tables["Property"], db2.tables["Property"]);
        assert!(db1_transformed.tables.contains_key("Feature"));

        // Roundtrip serialization
        let bytes = transform.to_bytes()?;
        assert_ne!(bytes.len(), 0);
        let restored = DatabaseTransform::from_bytes(&bytes)?;
        assert_eq!(restored.tables.len(), 2);

        Ok(())
    }

    /// Tests diffing dropped tables between baseline and updated databases.
    #[test]
    fn test_dropped_table_diff() -> Result<()> {
        let mut db1 = LinkedDatabase::new()?;
        let db2 = LinkedDatabase::new()?;

        db1.tables
            .insert("Property".to_string(), vec![Record::new()]);

        let transform = DatabaseTransform::diff(&db1, &db2)?;
        assert_eq!(transform.tables.len(), 1);
        assert!(transform.tables["Property"].is_dropped);

        let mut db1_mut = db1;
        transform.apply(&mut db1_mut)?;
        assert!(!db1_mut.tables.contains_key("Property"));

        Ok(())
    }

    /// Tests database diffing with row deletion, identical tables, dropped tables, and added tables.
    #[test]
    fn test_database_diff_row_deletion_and_unchanged_table() -> Result<()> {
        let mut db1 = LinkedDatabase::new()?;
        let mut db2 = LinkedDatabase::new()?;

        let mut row_keep = Record::new();
        row_keep.push(FieldValue::String("PropKeep".to_string()));
        row_keep.push(FieldValue::String("1".to_string()));

        let mut row_del = Record::new();
        row_del.push(FieldValue::String("PropDelete".to_string()));
        row_del.push(FieldValue::String("2".to_string()));

        // In db1: Property has row_keep and row_del
        db1.tables
            .insert("Property".to_string(), vec![row_keep.clone(), row_del]);
        // In db2: Property has row_keep (so row_del was deleted in db2)
        db2.tables
            .insert("Property".to_string(), vec![row_keep.clone()]);

        // Component table is identical in both db1 and db2 (so operations is empty, not inserted in transform)
        let mut comp_row = Record::new();
        comp_row.push(FieldValue::String("Comp1".to_string()));
        db1.tables
            .insert("Component".to_string(), vec![comp_row.clone()]);
        db2.tables.insert("Component".to_string(), vec![comp_row]);

        // DroppedTable exists only in db1
        db1.tables
            .insert("DroppedTable".to_string(), vec![Record::new()]);

        // AddedTable exists only in db2
        db2.tables
            .insert("AddedTable".to_string(), vec![Record::new()]);

        let transform = DatabaseTransform::diff(&db1, &db2)?;
        assert!(transform.tables.contains_key("Property"));
        assert!(transform.tables.contains_key("DroppedTable"));
        assert!(transform.tables.contains_key("AddedTable"));
        assert!(!transform.tables.contains_key("Component"));

        let prop_trans = &transform.tables["Property"];
        assert_eq!(prop_trans.operations.len(), 1);
        assert!(matches!(prop_trans.operations[0], RowOperation::Delete(_)));

        Ok(())
    }

    /// Tests applying transforms with row deletions, row modifications, and added tables with extra ops.
    #[test]
    fn test_apply_extended_operations() -> Result<()> {
        let mut db = LinkedDatabase::new()?;

        let mut r1 = Record::new();
        r1.push(FieldValue::String("P1".to_string()));
        r1.push(FieldValue::String("V1".to_string()));

        let mut r2 = Record::new();
        r2.push(FieldValue::String("P2".to_string()));
        r2.push(FieldValue::String("V2".to_string()));

        db.tables
            .insert("Property".to_string(), vec![r1.clone(), r2.clone()]);

        let mut transform = DatabaseTransform::new();

        // 1. Added table with an Insert and a non-Insert op to cover the if let RowOperation::Insert branch
        let added_table = TableTransform {
            table_name: "Added".to_string(),
            is_added: true,
            is_dropped: false,
            operations: vec![
                RowOperation::Insert(r1.clone()),
                RowOperation::Delete(r2.clone()),
            ],
        };
        transform.tables.insert("Added".to_string(), added_table);

        // 2. Modify Property:
        // - Delete r1 (leaves r2 retained)
        // - Modify r2 to r2_updated (exercises matching and breaking out)
        // - Modify a non-existing record r_ghost (exercises loop completing without match)
        let mut r2_updated = Record::new();
        r2_updated.push(FieldValue::String("P2".to_string()));
        r2_updated.push(FieldValue::String("V2_NEW".to_string()));

        let mut r_ghost = Record::new();
        r_ghost.push(FieldValue::String("Ghost".to_string()));
        r_ghost.push(FieldValue::String("0".to_string()));

        let prop_table = TableTransform {
            table_name: "Property".to_string(),
            is_added: false,
            is_dropped: false,
            operations: vec![
                RowOperation::Delete(r1),
                RowOperation::Modify {
                    key: r2,
                    updated: r2_updated.clone(),
                },
                RowOperation::Modify {
                    key: r_ghost.clone(),
                    updated: r_ghost,
                },
            ],
        };
        transform.tables.insert("Property".to_string(), prop_table);

        transform.apply(&mut db)?;

        assert!(db.tables.contains_key("Added"));
        assert_eq!(db.tables["Added"].len(), 1);

        assert_eq!(db.tables["Property"].len(), 1);
        assert_eq!(db.tables["Property"][0], r2_updated);

        Ok(())
    }

    /// Tests serialization and deserialization of transforms across all statuses, stream changes, and error conditions.
    #[test]
    fn test_transform_to_bytes_and_from_bytes_extended() -> Result<()> {
        let mut transform = DatabaseTransform::new();
        transform.tables.insert(
            "CreatedTable".to_string(),
            TableTransform {
                table_name: "CreatedTable".to_string(),
                is_added: true,
                is_dropped: false,
                operations: Vec::new(),
            },
        );
        transform.tables.insert(
            "DroppedTable".to_string(),
            TableTransform {
                table_name: "DroppedTable".to_string(),
                is_added: false,
                is_dropped: true,
                operations: Vec::new(),
            },
        );
        transform.tables.insert(
            "ModifiedTable".to_string(),
            TableTransform {
                table_name: "ModifiedTable".to_string(),
                is_added: false,
                is_dropped: false,
                operations: Vec::new(),
            },
        );
        transform
            .stream_changes
            .insert("StreamData".to_string(), vec![1, 2, 3, 4]);

        let bytes = transform.to_bytes()?;
        let restored = DatabaseTransform::from_bytes(&bytes)?;

        assert_eq!(restored.tables.len(), 3);
        assert!(restored.tables["CreatedTable"].is_added);
        assert!(restored.tables["DroppedTable"].is_dropped);
        assert!(!restored.tables["ModifiedTable"].is_added);
        assert!(!restored.tables["ModifiedTable"].is_dropped);

        // CFB without _TransformView stream (exercises Err branch of reader.read_stream)
        let mut writer_no_view = CfbWriter::new(CfbVersion::V3);
        writer_no_view.add_stream("\u{0005}SummaryInformation", &[])?;
        let cfb_no_view = writer_no_view.build();
        let parsed_no_view = DatabaseTransform::from_bytes(&cfb_no_view)?;
        assert!(parsed_no_view.tables.is_empty());

        // Error path in to_bytes: duplicate stream name triggers DuplicateDirectoryEntry
        let mut bad_transform = DatabaseTransform::new();
        bad_transform
            .stream_changes
            .insert("_TransformView".to_string(), vec![1]);
        assert!(bad_transform.to_bytes().is_err());

        // Error path in from_bytes: invalid CFB bytes
        assert!(DatabaseTransform::from_bytes(b"not a valid CFB").is_err());

        // Valid CFB without SummaryInformation and with single-column / DROP _TransformView lines
        let mut writer = CfbWriter::new(CfbVersion::V3);
        let view_content = "SinglePartLine\nDropTable\tDROP\t0\n";
        writer.add_stream("_TransformView", view_content.as_bytes())?;
        let empty_cfb = writer.build();
        let parsed = DatabaseTransform::from_bytes(&empty_cfb)?;
        assert_eq!(parsed.tables.len(), 1);
        assert!(parsed.tables["DropTable"].is_dropped);

        // CFB with corrupted SummaryInformation and invalid UTF-8 _TransformView
        let mut writer2 = CfbWriter::new(CfbVersion::V3);
        writer2.add_stream("\u{0005}SummaryInformation", b"corrupted summary")?;
        writer2.add_stream("_TransformView", &[0xFF, 0xFE, 0xFD])?;
        let cfb2 = writer2.build();
        let parsed2 = DatabaseTransform::from_bytes(&cfb2)?;
        assert!(parsed2.tables.is_empty());

        Ok(())
    }

    /// Tests primary key matching across schema presence, column types (Short, Long, String, Null), and composites.
    #[test]
    #[allow(clippy::too_many_lines)]
    fn test_rows_primary_keys_match_all_branches() {
        use crate::database::column::{ColumnDef, DataType};
        use crate::database::TableSchema;

        let mut r_short1 = Record::new();
        r_short1.push(FieldValue::Short(10));
        let mut r_short1_dup = Record::new();
        r_short1_dup.push(FieldValue::Short(10));
        let mut r_short2 = Record::new();
        r_short2.push(FieldValue::Short(20));

        let mut r_long1 = Record::new();
        r_long1.push(FieldValue::Long(100));
        let mut r_long2 = Record::new();
        r_long2.push(FieldValue::Long(200));

        let mut r_str1 = Record::new();
        r_str1.push(FieldValue::String("Alpha".to_string()));
        let mut r_str2 = Record::new();
        r_str2.push(FieldValue::String("Beta".to_string()));

        let mut r_null1 = Record::new();
        r_null1.push(FieldValue::Null);
        let mut r_null2 = Record::new();
        r_null2.push(FieldValue::Null);

        // 1. Schema is None
        assert!(rows_primary_keys_match(&r_short1, &r_short1_dup, None));
        assert!(!rows_primary_keys_match(&r_short1, &r_short2, None));

        // 2. Schema has no primary key columns
        let schema_no_pk = TableSchema {
            name: "NoPk".to_string(),
            columns: vec![ColumnDef::new("Col", DataType::Short)],
        };
        assert!(rows_primary_keys_match(
            &r_short1,
            &r_short1_dup,
            Some(&schema_no_pk)
        ));
        assert!(!rows_primary_keys_match(
            &r_short1,
            &r_short2,
            Some(&schema_no_pk)
        ));

        // 3. Schema with Short primary key
        let mut col_short = ColumnDef::new("Id", DataType::Short);
        col_short.primary_key = true;
        let schema_short = TableSchema {
            name: "ShortPk".to_string(),
            columns: vec![col_short],
        };
        assert!(rows_primary_keys_match(
            &r_short1,
            &r_short1_dup,
            Some(&schema_short)
        ));
        assert!(!rows_primary_keys_match(
            &r_short1,
            &r_short2,
            Some(&schema_short)
        ));

        // 4. Schema with Long primary key
        let mut col_long = ColumnDef::new("Id", DataType::Long);
        col_long.primary_key = true;
        let schema_long = TableSchema {
            name: "LongPk".to_string(),
            columns: vec![col_long],
        };
        assert!(rows_primary_keys_match(
            &r_long1,
            &r_long1,
            Some(&schema_long)
        ));
        assert!(!rows_primary_keys_match(
            &r_long1,
            &r_long2,
            Some(&schema_long)
        ));

        // 5. Schema with String primary key
        let mut col_str = ColumnDef::new("Id", DataType::String { max_len: 32 });
        col_str.primary_key = true;
        let schema_str = TableSchema {
            name: "StrPk".to_string(),
            columns: vec![col_str],
        };
        assert!(rows_primary_keys_match(&r_str1, &r_str1, Some(&schema_str)));
        assert!(!rows_primary_keys_match(
            &r_str1,
            &r_str2,
            Some(&schema_str)
        ));

        // 6. Null primary key
        assert!(rows_primary_keys_match(
            &r_null1,
            &r_null2,
            Some(&schema_str)
        ));

        // 7. Mismatched / unhandled types (_ => false)
        assert!(!rows_primary_keys_match(
            &r_short1,
            &r_long1,
            Some(&schema_short)
        ));
        assert!(!rows_primary_keys_match(
            &r_short1,
            &r_null1,
            Some(&schema_short)
        ));

        let empty_row = Record::new();
        assert!(!rows_primary_keys_match(
            &empty_row,
            &r_short1,
            Some(&schema_short)
        ));

        // 8. Composite primary key
        let mut col1 = ColumnDef::new("Part1", DataType::String { max_len: 16 });
        col1.primary_key = true;
        let mut col2 = ColumnDef::new("Part2", DataType::Short);
        col2.primary_key = true;
        let schema_comp = TableSchema {
            name: "CompositePk".to_string(),
            columns: vec![col1, col2],
        };

        let mut comp1 = Record::new();
        comp1.push(FieldValue::String("KeyA".to_string()));
        comp1.push(FieldValue::Short(1));

        let mut comp1_dup = Record::new();
        comp1_dup.push(FieldValue::String("KeyA".to_string()));
        comp1_dup.push(FieldValue::Short(1));

        let mut comp2_diff_second = Record::new();
        comp2_diff_second.push(FieldValue::String("KeyA".to_string()));
        comp2_diff_second.push(FieldValue::Short(2));

        let mut comp3_diff_first = Record::new();
        comp3_diff_first.push(FieldValue::String("KeyB".to_string()));
        comp3_diff_first.push(FieldValue::Short(1));

        assert!(rows_primary_keys_match(
            &comp1,
            &comp1_dup,
            Some(&schema_comp)
        ));
        assert!(!rows_primary_keys_match(
            &comp1,
            &comp2_diff_second,
            Some(&schema_comp)
        ));
        assert!(!rows_primary_keys_match(
            &comp1,
            &comp3_diff_first,
            Some(&schema_comp)
        ));
    }
}

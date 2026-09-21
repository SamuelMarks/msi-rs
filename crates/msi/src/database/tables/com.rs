//! COM, OLE, and Shell registration tables for Windows Installer database.
//!
//! Implements schemas for:
//! - `Class`
//! - `ProgId`
//! - `TypeLib`
//! - `Extension`
//! - `Verb`
//! - `MIME`
//! - `AppId`
//! - `SelfReg`

use crate::database::catalogs::TableSchema;
use crate::database::column::{ColumnDef, DataType};
use crate::database::tables::record::{FieldValue, Record};
use crate::database::tables::types::FileKey;
use crate::error::{Error, Result};

/// Row in the `SelfReg` table.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelfRegRow {
    /// Foreign key into File table.
    pub file: FileKey,
    /// Cost of registration in units of disk space.
    pub cost: Option<i16>,
}

impl SelfRegRow {
    /// Converts this row into a generic [`Record`].
    ///
    /// # Returns
    ///
    /// A generic [`Record`].
    #[must_use]
    pub fn to_record(&self) -> Record {
        Record::with_fields(vec![
            FieldValue::String(self.file.as_str().to_string()),
            self.cost.map_or(FieldValue::Null, FieldValue::Short),
        ])
    }

    /// Parses a [`SelfRegRow`] from a generic [`Record`].
    ///
    /// # Arguments
    ///
    /// * `rec` - Generic record.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Validation`] or [`Error::RecordLengthMismatch`].
    pub fn from_record(rec: &Record) -> Result<Self> {
        if rec.len() < 2 {
            return Err(Error::RecordLengthMismatch {
                expected: 2,
                actual: rec.len(),
            });
        }
        let file = match rec.get(0) {
            Some(FieldValue::String(s)) => FileKey::new(s.as_str())?,
            _ => {
                return Err(Error::Validation {
                    element: "SelfReg.File_".to_string(),
                    reason: "missing File_".to_string(),
                })
            }
        };
        let cost = match rec.get(1) {
            Some(FieldValue::Short(c)) => Some(*c),
            _ => None,
        };
        Ok(Self { file, cost })
    }
}

/// Creates the official schema for `Class` table.
///
/// # Returns
///
/// [`TableSchema`] for `Class`.
#[must_use]
pub fn class_schema() -> TableSchema {
    TableSchema::new("Class")
        .with_column(ColumnDef::new("CLSID", DataType::String { max_len: 38 }).primary_key())
        .with_column(ColumnDef::new("Context", DataType::String { max_len: 32 }).primary_key())
        .with_column(ColumnDef::new("Component_", DataType::String { max_len: 72 }).primary_key())
        .with_column(ColumnDef::new("ProgId_Default", DataType::String { max_len: 255 }).nullable())
        .with_column(
            ColumnDef::new("Description", DataType::String { max_len: 255 })
                .nullable()
                .localizable(),
        )
        .with_column(ColumnDef::new("AppId_", DataType::String { max_len: 38 }).nullable())
        .with_column(ColumnDef::new("FileTypeMask", DataType::String { max_len: 255 }).nullable())
        .with_column(ColumnDef::new("Icon_", DataType::String { max_len: 72 }).nullable())
        .with_column(ColumnDef::new("IconIndex", DataType::Short).nullable())
        .with_column(
            ColumnDef::new("DefInprocHandler", DataType::String { max_len: 32 }).nullable(),
        )
        .with_column(ColumnDef::new("Argument", DataType::String { max_len: 255 }).nullable())
        .with_column(ColumnDef::new("Feature_", DataType::String { max_len: 38 }))
}

/// Creates the official schema for `ProgId` table.
///
/// # Returns
///
/// [`TableSchema`] for `ProgId`.
#[must_use]
pub fn prog_id_schema() -> TableSchema {
    TableSchema::new("ProgId")
        .with_column(ColumnDef::new("ProgId", DataType::String { max_len: 255 }).primary_key())
        .with_column(ColumnDef::new("ProgId_Parent", DataType::String { max_len: 255 }).nullable())
        .with_column(ColumnDef::new("Class_", DataType::String { max_len: 38 }).nullable())
        .with_column(
            ColumnDef::new("Description", DataType::String { max_len: 255 })
                .nullable()
                .localizable(),
        )
        .with_column(ColumnDef::new("Icon_", DataType::String { max_len: 72 }).nullable())
        .with_column(ColumnDef::new("IconIndex", DataType::Short).nullable())
}

/// Creates the official schema for `TypeLib` table.
///
/// # Returns
///
/// [`TableSchema`] for `TypeLib`.
#[must_use]
pub fn type_lib_schema() -> TableSchema {
    TableSchema::new("TypeLib")
        .with_column(ColumnDef::new("LibID", DataType::String { max_len: 38 }).primary_key())
        .with_column(ColumnDef::new("Language", DataType::Short).primary_key())
        .with_column(ColumnDef::new(
            "Component_",
            DataType::String { max_len: 72 },
        ))
        .with_column(ColumnDef::new("Version", DataType::Long).nullable())
        .with_column(
            ColumnDef::new("Description", DataType::String { max_len: 255 })
                .nullable()
                .localizable(),
        )
        .with_column(ColumnDef::new("Directory_", DataType::String { max_len: 72 }).nullable())
        .with_column(ColumnDef::new("Feature_", DataType::String { max_len: 38 }))
        .with_column(ColumnDef::new("Cost", DataType::Long).nullable())
}

/// Creates the official schema for `Extension` table.
///
/// # Returns
///
/// [`TableSchema`] for `Extension`.
#[must_use]
pub fn extension_schema() -> TableSchema {
    TableSchema::new("Extension")
        .with_column(ColumnDef::new("Extension", DataType::String { max_len: 255 }).primary_key())
        .with_column(ColumnDef::new("Component_", DataType::String { max_len: 72 }).primary_key())
        .with_column(ColumnDef::new("ProgId_", DataType::String { max_len: 255 }).nullable())
        .with_column(ColumnDef::new("MIME_", DataType::String { max_len: 64 }).nullable())
        .with_column(ColumnDef::new("Feature_", DataType::String { max_len: 38 }))
}

/// Creates the official schema for `Verb` table.
///
/// # Returns
///
/// [`TableSchema`] for `Verb`.
#[must_use]
pub fn verb_schema() -> TableSchema {
    TableSchema::new("Verb")
        .with_column(ColumnDef::new("Extension_", DataType::String { max_len: 255 }).primary_key())
        .with_column(ColumnDef::new("Verb", DataType::String { max_len: 32 }).primary_key())
        .with_column(ColumnDef::new("Sequence", DataType::Short).nullable())
        .with_column(
            ColumnDef::new("Command", DataType::String { max_len: 255 })
                .nullable()
                .localizable(),
        )
        .with_column(
            ColumnDef::new("Argument", DataType::String { max_len: 255 })
                .nullable()
                .localizable(),
        )
}

/// Creates the official schema for `MIME` table.
///
/// # Returns
///
/// [`TableSchema`] for `MIME`.
#[must_use]
pub fn mime_schema() -> TableSchema {
    TableSchema::new("MIME")
        .with_column(ColumnDef::new("ContentType", DataType::String { max_len: 64 }).primary_key())
        .with_column(ColumnDef::new(
            "Extension_",
            DataType::String { max_len: 255 },
        ))
        .with_column(ColumnDef::new("CLSID", DataType::String { max_len: 38 }).nullable())
}

/// Creates the official schema for `AppId` table.
///
/// # Returns
///
/// [`TableSchema`] for `AppId`.
#[must_use]
pub fn app_id_schema() -> TableSchema {
    TableSchema::new("AppId")
        .with_column(ColumnDef::new("AppId", DataType::String { max_len: 38 }).primary_key())
        .with_column(
            ColumnDef::new("RemoteServerName", DataType::String { max_len: 255 }).nullable(),
        )
        .with_column(ColumnDef::new("LocalService", DataType::String { max_len: 255 }).nullable())
        .with_column(
            ColumnDef::new("ServiceParameters", DataType::String { max_len: 255 }).nullable(),
        )
        .with_column(ColumnDef::new("DllSurrogate", DataType::String { max_len: 255 }).nullable())
        .with_column(ColumnDef::new("ActivateAtStorage", DataType::Short).nullable())
        .with_column(ColumnDef::new("RunAsInteractiveUser", DataType::Short).nullable())
}

/// Creates the official schema for `SelfReg` table.
///
/// # Returns
///
/// [`TableSchema`] for `SelfReg`.
#[must_use]
pub fn self_reg_schema() -> TableSchema {
    TableSchema::new("SelfReg")
        .with_column(ColumnDef::new("File_", DataType::String { max_len: 72 }).primary_key())
        .with_column(ColumnDef::new("Cost", DataType::Short).nullable())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Tests serialization, deserialization, and error handling for [`SelfRegRow`].
    #[test]
    fn test_self_reg_roundtrip() -> Result<()> {
        let file = FileKey::new("file1")?;
        let row = SelfRegRow {
            file,
            cost: Some(512),
        };
        let rec = row.to_record();
        assert_eq!(rec.len(), 2);
        let parsed = SelfRegRow::from_record(&rec);
        assert_eq!(parsed, Ok(row));

        // Test with None cost
        let row_no_cost = SelfRegRow {
            file: FileKey::new("file2")?,
            cost: None,
        };
        let rec_no_cost = row_no_cost.to_record();
        assert_eq!(SelfRegRow::from_record(&rec_no_cost), Ok(row_no_cost));

        // Error on record length mismatch (< 2 fields)
        assert!(SelfRegRow::from_record(&Record::new()).is_err());

        // Error on missing / non-string File_ field
        let mut bad_field_rec = Record::new();
        bad_field_rec.push(FieldValue::Null);
        bad_field_rec.push(FieldValue::Null);
        assert!(SelfRegRow::from_record(&bad_field_rec).is_err());

        // Error on invalid FileKey identifier
        let mut bad_key_rec = Record::new();
        bad_key_rec.push(FieldValue::String(String::new()));
        bad_key_rec.push(FieldValue::Null);
        assert!(SelfRegRow::from_record(&bad_key_rec).is_err());

        Ok(())
    }

    /// Tests COM table schemas.
    #[test]
    fn test_com_schemas() {
        assert_eq!(class_schema().name, "Class");
        assert_eq!(prog_id_schema().name, "ProgId");
        assert_eq!(type_lib_schema().name, "TypeLib");
        assert_eq!(extension_schema().name, "Extension");
        assert_eq!(verb_schema().name, "Verb");
        assert_eq!(mime_schema().name, "MIME");
        assert_eq!(app_id_schema().name, "AppId");
        assert_eq!(self_reg_schema().name, "SelfReg");
    }
}

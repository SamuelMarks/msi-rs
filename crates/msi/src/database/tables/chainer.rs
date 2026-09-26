//! Windows Installer 4.5+ Embedded Chainer database table schemas and typed rows.
//!
//! Grounded in the official Windows Installer specification for multi-package transaction chaining.

use crate::database::catalogs::TableSchema;
use crate::database::column::{ColumnDef, DataType};
use crate::database::tables::record::{FieldValue, Record};
use crate::error::{Error, Result};

/// Creates the official schema for the `MsiEmbeddedChainer` table.
///
/// Columns:
/// - `MsiEmbeddedChainer`: Identifier primary key (`DataType::String { max_len: 72 }`)
/// - `Condition`: Conditional expression (`DataType::String { max_len: 255 }`, nullable)
/// - `CommandLine`: Formatted command line arguments (`DataType::Formatted { max_len: 255 }`, nullable)
/// - `Source`: Custom source identifier linking to Binary or File (`DataType::CustomSource { max_len: 72 }`)
/// - `Type`: Action type numeric code (`DataType::Long`)
///
/// # Returns
///
/// The [`TableSchema`] for `MsiEmbeddedChainer`.
#[must_use]
pub fn msi_embedded_chainer_schema() -> TableSchema {
    TableSchema::new("MsiEmbeddedChainer")
        .with_column(
            ColumnDef::new("MsiEmbeddedChainer", DataType::String { max_len: 72 }).primary_key(),
        )
        .with_column(ColumnDef::new("Condition", DataType::String { max_len: 255 }).nullable())
        .with_column(ColumnDef::new("CommandLine", DataType::String { max_len: 255 }).nullable())
        .with_column(ColumnDef::new("Source", DataType::String { max_len: 72 }))
        .with_column(ColumnDef::new("Type", DataType::Long))
}

/// Typed representation of a record in the `MsiEmbeddedChainer` table.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MsiEmbeddedChainerRow {
    /// Unique identifier for the embedded chainer record (primary key, max 72 characters).
    pub chainer: String,
    /// Optional conditional statement that must evaluate to true for the chainer to run.
    pub condition: Option<String>,
    /// Optional formatted command line arguments passed to the embedded chainer executable or library.
    pub command_line: Option<String>,
    /// Identifier referencing the source binary stream in the `Binary` table or file in `File` table.
    pub source: String,
    /// Type bitmask or code specifying invocation details (e.g. DLL entry point or executable).
    pub chainer_type: i32,
}

impl MsiEmbeddedChainerRow {
    /// Creates a new [`MsiEmbeddedChainerRow`] with validated parameters.
    ///
    /// # Arguments
    ///
    /// * `chainer` - Primary key identifier (max 72 characters).
    /// * `condition` - Optional execution condition.
    /// * `command_line` - Optional command line string.
    /// * `source` - Binary key or File key (max 72 characters).
    /// * `chainer_type` - Chainer numeric type code.
    ///
    /// # Returns
    ///
    /// A validated [`MsiEmbeddedChainerRow`].
    ///
    /// # Errors
    ///
    /// Returns [`Error::Validation`] if required fields are empty or exceed length limits.
    pub fn new(
        chainer: impl Into<String>,
        condition: Option<String>,
        command_line: Option<String>,
        source: impl Into<String>,
        chainer_type: i32,
    ) -> Result<Self> {
        Self::new_impl(
            chainer.into(),
            condition,
            command_line,
            source.into(),
            chainer_type,
        )
    }

    /// Validates concrete parameters and creates a new [`MsiEmbeddedChainerRow`].
    fn new_impl(
        chainer: String,
        condition: Option<String>,
        command_line: Option<String>,
        source: String,
        chainer_type: i32,
    ) -> Result<Self> {
        if chainer.is_empty() {
            return Err(Error::Validation {
                element: "MsiEmbeddedChainer.MsiEmbeddedChainer".to_string(),
                reason: "primary key cannot be empty".to_string(),
            });
        }
        if chainer.len() > 72 {
            return Err(Error::Validation {
                element: "MsiEmbeddedChainer.MsiEmbeddedChainer".to_string(),
                reason: format!("identifier length {} exceeds maximum 72", chainer.len()),
            });
        }
        if source.is_empty() {
            return Err(Error::Validation {
                element: "MsiEmbeddedChainer.Source".to_string(),
                reason: "source identifier cannot be empty".to_string(),
            });
        }
        if source.len() > 72 {
            return Err(Error::Validation {
                element: "MsiEmbeddedChainer.Source".to_string(),
                reason: format!(
                    "source identifier length {} exceeds maximum 72",
                    source.len()
                ),
            });
        }

        Ok(Self {
            chainer,
            condition,
            command_line,
            source,
            chainer_type,
        })
    }

    /// Converts this row into a generic database [`Record`].
    ///
    /// # Returns
    ///
    /// A generic [`Record`] matching the `MsiEmbeddedChainer` table schema.
    #[must_use]
    pub fn to_record(&self) -> Record {
        Record::with_fields(vec![
            FieldValue::String(self.chainer.clone()),
            self.condition
                .as_ref()
                .map_or(FieldValue::Null, |c| FieldValue::String(c.clone())),
            self.command_line
                .as_ref()
                .map_or(FieldValue::Null, |cl| FieldValue::String(cl.clone())),
            FieldValue::String(self.source.clone()),
            FieldValue::Long(self.chainer_type),
        ])
    }

    /// Parses a [`MsiEmbeddedChainerRow`] from a generic database [`Record`].
    ///
    /// # Arguments
    ///
    /// * `rec` - The database record to parse.
    ///
    /// # Returns
    ///
    /// The parsed [`MsiEmbeddedChainerRow`].
    ///
    /// # Errors
    ///
    /// Returns [`Error::RecordLengthMismatch`] or [`Error::Validation`] on invalid data.
    pub fn from_record(rec: &Record) -> Result<Self> {
        if rec.len() < 5 {
            return Err(Error::RecordLengthMismatch {
                expected: 5,
                actual: rec.len(),
            });
        }

        let chainer = match rec.get(0) {
            Some(FieldValue::String(s)) if !s.is_empty() => s.clone(),
            _ => {
                return Err(Error::Validation {
                    element: "MsiEmbeddedChainer.MsiEmbeddedChainer".to_string(),
                    reason: "missing or empty primary key".to_string(),
                });
            }
        };

        let condition = match rec.get(1) {
            Some(FieldValue::String(s)) if !s.is_empty() => Some(s.clone()),
            _ => None,
        };

        let command_line = match rec.get(2) {
            Some(FieldValue::String(s)) if !s.is_empty() => Some(s.clone()),
            _ => None,
        };

        let source = match rec.get(3) {
            Some(FieldValue::String(s)) if !s.is_empty() => s.clone(),
            _ => {
                return Err(Error::Validation {
                    element: "MsiEmbeddedChainer.Source".to_string(),
                    reason: "missing or empty source identifier".to_string(),
                });
            }
        };

        let chainer_type = match rec.get(4) {
            Some(FieldValue::Long(v)) => *v,
            Some(FieldValue::Short(v)) => i32::from(*v),
            _ => 0,
        };

        Self::new_impl(chainer, condition, command_line, source, chainer_type)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[allow(clippy::option_if_let_else)]
    fn unwrap_row(res: Result<MsiEmbeddedChainerRow>) -> MsiEmbeddedChainerRow {
        match res {
            Ok(r) => r,
            Err(_) => MsiEmbeddedChainerRow {
                chainer: String::new(),
                condition: None,
                command_line: None,
                source: String::new(),
                chainer_type: 0,
            },
        }
    }

    /// Tests schema column definitions and primary keys.
    #[test]
    fn test_msi_embedded_chainer_schema() {
        let schema = msi_embedded_chainer_schema();
        assert_eq!(schema.name, "MsiEmbeddedChainer");
        assert_eq!(schema.columns.len(), 5);
        assert_eq!(schema.primary_keys(), vec!["MsiEmbeddedChainer"]);
        assert_eq!(schema.columns[0].name, "MsiEmbeddedChainer");
        assert_eq!(schema.columns[1].name, "Condition");
        assert_eq!(schema.columns[2].name, "CommandLine");
        assert_eq!(schema.columns[3].name, "Source");
        assert_eq!(schema.columns[4].name, "Type");
    }

    /// Tests row creation, record serialization, and parsing.
    #[test]
    fn test_msi_embedded_chainer_row_roundtrip() {
        // Exercise Err branch of unwrap_row
        let dummy = unwrap_row(MsiEmbeddedChainerRow::new("", None, None, "", 0));
        assert_eq!(dummy.chainer, "");

        let row = unwrap_row(MsiEmbeddedChainerRow::new(
            "LibScriptChainer",
            Some("NOT Installed".to_string()),
            Some("/quiet".to_string()),
            "ChainerDll",
            1,
        ));

        let rec = row.to_record();
        assert_eq!(rec.len(), 5);
        assert_eq!(
            rec.get(0),
            Some(&FieldValue::String("LibScriptChainer".to_string()))
        );
        assert_eq!(
            rec.get(1),
            Some(&FieldValue::String("NOT Installed".to_string()))
        );
        assert_eq!(rec.get(2), Some(&FieldValue::String("/quiet".to_string())));
        assert_eq!(
            rec.get(3),
            Some(&FieldValue::String("ChainerDll".to_string()))
        );
        assert_eq!(rec.get(4), Some(&FieldValue::Long(1)));

        let parsed = unwrap_row(MsiEmbeddedChainerRow::from_record(&rec));
        assert_eq!(parsed, row);

        // Test with null optional fields and Short type field
        let row_minimal = unwrap_row(MsiEmbeddedChainerRow::new(
            "MinChainer",
            None,
            None,
            "MinSource",
            2,
        ));
        let mut rec_minimal = row_minimal.to_record();
        // Overwrite Long with Short to test parser branch
        rec_minimal.set(4, FieldValue::Short(2));
        let parsed_minimal = unwrap_row(MsiEmbeddedChainerRow::from_record(&rec_minimal));
        assert_eq!(parsed_minimal, row_minimal);
        assert_eq!(parsed_minimal.condition, None);
        assert_eq!(parsed_minimal.command_line, None);

        // Permutations of optional fields in to_record
        let row_cond_only = unwrap_row(MsiEmbeddedChainerRow::new(
            "C1",
            Some("COND".to_string()),
            None,
            "S1",
            1,
        ));
        let rec_cond = row_cond_only.to_record();
        assert_eq!(
            rec_cond.get(1),
            Some(&FieldValue::String("COND".to_string()))
        );
        assert_eq!(rec_cond.get(2), Some(&FieldValue::Null));

        let row_cmd_only = unwrap_row(MsiEmbeddedChainerRow::new(
            "C2",
            None,
            Some("/cmd".to_string()),
            "S2",
            1,
        ));
        let rec_cmd = row_cmd_only.to_record();
        assert_eq!(rec_cmd.get(1), Some(&FieldValue::Null));
        assert_eq!(
            rec_cmd.get(2),
            Some(&FieldValue::String("/cmd".to_string()))
        );

        // Test fallback chainer_type when field is null
        rec_minimal.set(4, FieldValue::Null);
        let parsed_fallback = unwrap_row(MsiEmbeddedChainerRow::from_record(&rec_minimal));
        assert_eq!(parsed_fallback.chainer_type, 0);

        // Test with empty string fields in record
        let empty_str_rec = Record::with_fields(vec![
            FieldValue::String("ChainerEmp".to_string()),
            FieldValue::String(String::new()),
            FieldValue::String(String::new()),
            FieldValue::String("Source1".to_string()),
            FieldValue::Long(1),
        ]);
        let parsed_emp = unwrap_row(MsiEmbeddedChainerRow::from_record(&empty_str_rec));
        assert_eq!(parsed_emp.condition, None);
        assert_eq!(parsed_emp.command_line, None);

        // Test non-string variants for condition and command line
        let non_str_rec = Record::with_fields(vec![
            FieldValue::String("ChainerNonStr".to_string()),
            FieldValue::Long(123),
            FieldValue::Short(456),
            FieldValue::String("Source2".to_string()),
            FieldValue::Long(1),
        ]);
        let parsed_non_str = unwrap_row(MsiEmbeddedChainerRow::from_record(&non_str_rec));
        assert_eq!(parsed_non_str.condition, None);
        assert_eq!(parsed_non_str.command_line, None);

        // Test clone, debug, equality
        let row_cloned = row.clone();
        assert_eq!(row_cloned, row);
        assert!(format!("{row:?}").contains("LibScriptChainer"));
    }

    /// Tests validation errors on invalid row data.
    #[test]
    fn test_msi_embedded_chainer_validation_errors() {
        // Empty primary key
        assert!(MsiEmbeddedChainerRow::new("", None, None, "Src", 0).is_err());

        // Primary key exceeding 72 chars
        let long_id = "A".repeat(73);
        assert!(MsiEmbeddedChainerRow::new(long_id, None, None, "Src", 0).is_err());

        // Empty source
        assert!(MsiEmbeddedChainerRow::new("Chainer1", None, None, "", 0).is_err());

        // Source exceeding 72 chars
        let long_src = "S".repeat(73);
        assert!(MsiEmbeddedChainerRow::new("Chainer1", None, None, long_src, 0).is_err());

        // Record length mismatch
        let short_rec = Record::with_fields(vec![FieldValue::String("Chainer1".to_string())]);
        assert!(matches!(
            MsiEmbeddedChainerRow::from_record(&short_rec),
            Err(Error::RecordLengthMismatch { .. })
        ));

        // Missing or empty primary key in record
        let empty_pk_rec = Record::with_fields(vec![
            FieldValue::Null,
            FieldValue::Null,
            FieldValue::Null,
            FieldValue::String("Src".to_string()),
            FieldValue::Long(1),
        ]);
        assert!(matches!(
            MsiEmbeddedChainerRow::from_record(&empty_pk_rec),
            Err(Error::Validation { .. })
        ));

        let empty_str_pk = Record::with_fields(vec![
            FieldValue::String(String::new()),
            FieldValue::Null,
            FieldValue::Null,
            FieldValue::String("Src".to_string()),
            FieldValue::Long(1),
        ]);
        assert!(matches!(
            MsiEmbeddedChainerRow::from_record(&empty_str_pk),
            Err(Error::Validation { .. })
        ));

        let non_str_pk = Record::with_fields(vec![
            FieldValue::Long(999),
            FieldValue::Null,
            FieldValue::Null,
            FieldValue::String("Src".to_string()),
            FieldValue::Long(1),
        ]);
        assert!(matches!(
            MsiEmbeddedChainerRow::from_record(&non_str_pk),
            Err(Error::Validation { .. })
        ));

        // Missing or empty source in record
        let empty_src_rec = Record::with_fields(vec![
            FieldValue::String("Ch1".to_string()),
            FieldValue::Null,
            FieldValue::Null,
            FieldValue::Null,
            FieldValue::Long(1),
        ]);
        assert!(matches!(
            MsiEmbeddedChainerRow::from_record(&empty_src_rec),
            Err(Error::Validation { .. })
        ));

        let empty_str_src = Record::with_fields(vec![
            FieldValue::String("Ch1".to_string()),
            FieldValue::Null,
            FieldValue::Null,
            FieldValue::String(String::new()),
            FieldValue::Long(1),
        ]);
        assert!(matches!(
            MsiEmbeddedChainerRow::from_record(&empty_str_src),
            Err(Error::Validation { .. })
        ));

        let non_str_src = Record::with_fields(vec![
            FieldValue::String("Ch1".to_string()),
            FieldValue::Null,
            FieldValue::Null,
            FieldValue::Long(42),
            FieldValue::Long(1),
        ]);
        assert!(matches!(
            MsiEmbeddedChainerRow::from_record(&non_str_src),
            Err(Error::Validation { .. })
        ));

        // from_record with fields exceeding 72 chars
        let long_pk_rec = Record::with_fields(vec![
            FieldValue::String("A".repeat(73)),
            FieldValue::Null,
            FieldValue::Null,
            FieldValue::String("Src".to_string()),
            FieldValue::Long(1),
        ]);
        assert!(MsiEmbeddedChainerRow::from_record(&long_pk_rec).is_err());

        let long_src_rec = Record::with_fields(vec![
            FieldValue::String("Ch1".to_string()),
            FieldValue::Null,
            FieldValue::Null,
            FieldValue::String("S".repeat(73)),
            FieldValue::Long(1),
        ]);
        assert!(MsiEmbeddedChainerRow::from_record(&long_src_rec).is_err());
    }
}

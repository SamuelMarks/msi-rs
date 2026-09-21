//! Sequence tables for Windows Installer action scheduling.
//!
//! Implements schemas and strongly-typed rows for:
//! - `InstallExecuteSequence`
//! - `InstallUISequence`
//! - `AdminExecuteSequence`
//! - `AdminUISequence`
//! - `AdvtExecuteSequence`

use crate::database::catalogs::TableSchema;
use crate::database::column::{ColumnDef, DataType};
use crate::database::tables::record::{FieldValue, Record};
use crate::error::{Error, Result};

/// Generic row representing an entry in any standard action sequence table.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SequenceRow {
    /// Action name identifier (primary key, max 72 chars).
    pub action: String,
    /// Conditional execution expression (nullable, max 255 chars).
    pub condition: Option<String>,
    /// Execution order sequence number (nullable).
    pub sequence: Option<i16>,
}

impl SequenceRow {
    /// Creates a new [`SequenceRow`].
    ///
    /// # Arguments
    ///
    /// * `action` - Action name.
    /// * `condition` - Optional condition expression.
    /// * `sequence` - Optional execution sequence order.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Validation`] if action name is empty or exceeds 72 characters.
    pub fn new(
        action: impl Into<String>,
        condition: Option<String>,
        sequence: Option<i16>,
    ) -> Result<Self> {
        let a = action.into();
        if a.is_empty() || a.len() > 72 {
            return Err(Error::Validation {
                element: "SequenceRow.Action".to_string(),
                reason: format!(
                    "Action name must be between 1 and 72 characters, got {}",
                    a.len()
                ),
            });
        }
        if let Some(ref c) = condition {
            if c.len() > 255 {
                return Err(Error::Validation {
                    element: "SequenceRow.Condition".to_string(),
                    reason: format!(
                        "Condition length must not exceed 255 characters, got {}",
                        c.len()
                    ),
                });
            }
        }
        Ok(Self {
            action: a,
            condition,
            sequence,
        })
    }

    /// Converts this sequence entry into a generic [`Record`].
    ///
    /// # Returns
    ///
    /// A generic [`Record`].
    #[must_use]
    pub fn to_record(&self) -> Record {
        Record::with_fields(vec![
            FieldValue::String(self.action.clone()),
            self.condition
                .as_ref()
                .map_or(FieldValue::Null, |c| FieldValue::String(c.clone())),
            self.sequence.map_or(FieldValue::Null, FieldValue::Short),
        ])
    }

    /// Parses a [`SequenceRow`] from a generic [`Record`].
    ///
    /// # Arguments
    ///
    /// * `rec` - Generic record.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Validation`] or [`Error::RecordLengthMismatch`] on invalid record.
    pub fn from_record(rec: &Record) -> Result<Self> {
        if rec.len() < 3 {
            return Err(Error::RecordLengthMismatch {
                expected: 3,
                actual: rec.len(),
            });
        }

        let action = match rec.get(0) {
            Some(FieldValue::String(s)) => s.clone(),
            _ => {
                return Err(Error::Validation {
                    element: "Sequence.Action".to_string(),
                    reason: "missing Action primary key".to_string(),
                });
            }
        };

        let condition = match rec.get(1) {
            Some(FieldValue::String(s)) if !s.is_empty() => Some(s.clone()),
            _ => None,
        };

        let sequence = match rec.get(2) {
            Some(FieldValue::Short(s)) => Some(*s),
            _ => None,
        };

        Self::new(action, condition, sequence)
    }
}

/// Helper to generate a [`TableSchema`] for standard action sequence tables.
fn create_sequence_schema(name: &'static str) -> TableSchema {
    TableSchema::new(name)
        .with_column(ColumnDef::new("Action", DataType::String { max_len: 72 }).primary_key())
        .with_column(ColumnDef::new("Condition", DataType::String { max_len: 255 }).nullable())
        .with_column(ColumnDef::new("Sequence", DataType::Short).nullable())
}

/// Creates the official MSI SDK schema for `InstallExecuteSequence`.
///
/// # Returns
///
/// [`TableSchema`] for `InstallExecuteSequence`.
#[must_use]
pub fn install_execute_sequence_schema() -> TableSchema {
    create_sequence_schema("InstallExecuteSequence")
}

/// Creates the official MSI SDK schema for `InstallUISequence`.
///
/// # Returns
///
/// [`TableSchema`] for `InstallUISequence`.
#[must_use]
pub fn install_ui_sequence_schema() -> TableSchema {
    create_sequence_schema("InstallUISequence")
}

/// Creates the official MSI SDK schema for `AdminExecuteSequence`.
///
/// # Returns
///
/// [`TableSchema`] for `AdminExecuteSequence`.
#[must_use]
pub fn admin_execute_sequence_schema() -> TableSchema {
    create_sequence_schema("AdminExecuteSequence")
}

/// Creates the official MSI SDK schema for `AdminUISequence`.
///
/// # Returns
///
/// [`TableSchema`] for `AdminUISequence`.
#[must_use]
pub fn admin_ui_sequence_schema() -> TableSchema {
    create_sequence_schema("AdminUISequence")
}

/// Creates the official MSI SDK schema for `AdvtExecuteSequence`.
///
/// # Returns
///
/// [`TableSchema`] for `AdvtExecuteSequence`.
#[must_use]
pub fn advt_execute_sequence_schema() -> TableSchema {
    create_sequence_schema("AdvtExecuteSequence")
}

/// Creates the official MSI SDK schema for `CustomAction`.
///
/// # Returns
///
/// [`TableSchema`] for `CustomAction`.
#[must_use]
pub fn custom_action_schema() -> TableSchema {
    TableSchema::new("CustomAction")
        .with_column(ColumnDef::new("Action", DataType::String { max_len: 72 }).primary_key())
        .with_column(ColumnDef::new("Type", DataType::Short))
        .with_column(ColumnDef::new("Source", DataType::String { max_len: 72 }))
        .with_column(ColumnDef::new("Target", DataType::String { max_len: 255 }).nullable())
        .with_column(ColumnDef::new("ExtendedType", DataType::Long).nullable())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Tests serialization and deserialization roundtrip for sequence rows.
    #[test]
    fn test_sequence_row_roundtrip() -> Result<()> {
        let r = SequenceRow::new("CostInitialize", None, Some(800))?;
        let rec = r.to_record();
        assert_eq!(rec.len(), 3);
        let parsed = SequenceRow::from_record(&rec);
        assert_eq!(parsed, Ok(r));

        let cond_r = SequenceRow::new(
            "InstallFiles",
            Some("NOT Installed".to_string()),
            Some(4000),
        )?;
        let cond_rec = cond_r.to_record();
        let cond_parsed = SequenceRow::from_record(&cond_rec);
        assert_eq!(cond_parsed, Ok(cond_r));

        // Test with None sequence
        let no_seq_r = SequenceRow::new("CostFinalize", None, None)?;
        let no_seq_rec = no_seq_r.to_record();
        assert_eq!(SequenceRow::from_record(&no_seq_rec), Ok(no_seq_r));

        // Test with &String action and None condition
        let action_string = "CustomAction".to_string();
        assert!(SequenceRow::new(&action_string, None, None).is_ok());

        // Test with empty condition string in Record (falls through guard to None)
        let empty_cond_rec = Record::with_fields(vec![
            FieldValue::String("CostFinalize".to_string()),
            FieldValue::String(String::new()),
            FieldValue::Short(1000),
        ]);
        assert_eq!(
            SequenceRow::from_record(&empty_cond_rec),
            Ok(SequenceRow {
                action: "CostFinalize".to_string(),
                condition: None,
                sequence: Some(1000),
            })
        );

        Ok(())
    }

    /// Tests sequence row validation error handling.
    #[test]
    fn test_sequence_validation_errors() {
        // &str parameter
        assert!(SequenceRow::new("", None, None).is_err());
        let long_str = "a".repeat(73);
        assert!(SequenceRow::new(long_str.as_str(), None, None).is_err());
        assert!(SequenceRow::new("Action", Some("c".repeat(256)), None).is_err());
        assert!(SequenceRow::new("Action", Some("c".to_string()), None).is_ok());
        assert!(SequenceRow::new("Action", None, None).is_ok());

        // String parameter
        assert!(SequenceRow::new(String::new(), None, None).is_err());
        assert!(SequenceRow::new("a".repeat(73), None, None).is_err());
        assert!(SequenceRow::new("Action".to_string(), Some("c".repeat(256)), None).is_err());
        assert!(SequenceRow::new("Action".to_string(), Some("c".to_string()), None).is_ok());
        assert!(SequenceRow::new("Action".to_string(), None, None).is_ok());

        assert!(SequenceRow::from_record(&Record::new()).is_err());
        let bad_rec =
            Record::with_fields(vec![FieldValue::Null, FieldValue::Null, FieldValue::Null]);
        assert!(SequenceRow::from_record(&bad_rec).is_err());
    }

    /// Tests standard sequence table schemas.
    #[test]
    fn test_sequence_schemas() {
        let ies = install_execute_sequence_schema();
        assert_eq!(ies.name, "InstallExecuteSequence");
        assert_eq!(ies.columns.len(), 3);

        let ius = install_ui_sequence_schema();
        assert_eq!(ius.name, "InstallUISequence");

        let aes = admin_execute_sequence_schema();
        assert_eq!(aes.name, "AdminExecuteSequence");

        let aus = admin_ui_sequence_schema();
        assert_eq!(aus.name, "AdminUISequence");

        let adv = advt_execute_sequence_schema();
        assert_eq!(adv.name, "AdvtExecuteSequence");

        let ca = custom_action_schema();
        assert_eq!(ca.name, "CustomAction");
        assert_eq!(ca.columns.len(), 5);
    }
}

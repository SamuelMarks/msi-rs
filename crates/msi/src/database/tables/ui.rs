//! UI and Presentation tables for Windows Installer database.
//!
//! Implements schemas for:
//! - `Dialog`
//! - `Control`
//! - `ControlCondition`
//! - `ControlEvent`
//! - `EventMapping`
//! - `TextStyle`
//! - `RadioButton`
//! - `CheckBox`
//! - `ComboBox`
//! - `ListBox`
//! - `ListView`
//! - `Billboard`
//! - `BBControl`
//! - `ActionText`
//! - `Error`

use crate::database::catalogs::TableSchema;
use crate::database::column::{ColumnDef, DataType};
use crate::database::tables::record::{FieldValue, Record};
use crate::error::{Error, Result};

/// Row in the `Dialog` table.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DialogRow {
    /// Dialog identifier (primary key, max 72 chars).
    pub dialog: String,
    /// Horizontal centering (50 = centered).
    pub h_centering: i16,
    /// Vertical centering (50 = centered).
    pub v_centering: i16,
    /// Dialog width in installer dialog units.
    pub width: i16,
    /// Dialog height in installer dialog units.
    pub height: i16,
    /// Dialog window attributes bitmask.
    pub attributes: i32,
    /// Dialog window title (nullable, localizable, max 128 chars).
    pub title: Option<String>,
    /// Identifier of first control to receive focus.
    pub control_first: String,
    /// Default button control (nullable).
    pub control_default: Option<String>,
    /// Cancel button control (nullable).
    pub control_cancel: Option<String>,
}

impl DialogRow {
    /// Converts this row into a generic [`Record`].
    ///
    /// # Returns
    ///
    /// A generic [`Record`].
    #[must_use]
    pub fn to_record(&self) -> Record {
        Record::with_fields(vec![
            FieldValue::String(self.dialog.clone()),
            FieldValue::Short(self.h_centering),
            FieldValue::Short(self.v_centering),
            FieldValue::Short(self.width),
            FieldValue::Short(self.height),
            FieldValue::Long(self.attributes),
            self.title
                .as_ref()
                .map_or(FieldValue::Null, |t| FieldValue::String(t.clone())),
            FieldValue::String(self.control_first.clone()),
            self.control_default
                .as_ref()
                .map_or(FieldValue::Null, |d| FieldValue::String(d.clone())),
            self.control_cancel
                .as_ref()
                .map_or(FieldValue::Null, |c| FieldValue::String(c.clone())),
        ])
    }

    /// Parses a [`DialogRow`] from a generic [`Record`].
    ///
    /// # Arguments
    ///
    /// * `rec` - Generic record.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Validation`] or [`Error::RecordLengthMismatch`].
    pub fn from_record(rec: &Record) -> Result<Self> {
        if rec.len() < 10 {
            return Err(Error::RecordLengthMismatch {
                expected: 10,
                actual: rec.len(),
            });
        }
        let dialog = match rec.get(0) {
            Some(FieldValue::String(s)) => s.clone(),
            _ => {
                return Err(Error::Validation {
                    element: "Dialog.Dialog".to_string(),
                    reason: "missing Dialog PK".to_string(),
                })
            }
        };
        let h_centering = match rec.get(1) {
            Some(FieldValue::Short(s)) => *s,
            _ => 50,
        };
        let v_centering = match rec.get(2) {
            Some(FieldValue::Short(s)) => *s,
            _ => 50,
        };
        let width = match rec.get(3) {
            Some(FieldValue::Short(s)) => *s,
            _ => 370,
        };
        let height = match rec.get(4) {
            Some(FieldValue::Short(s)) => *s,
            _ => 270,
        };
        let attributes = match rec.get(5) {
            Some(FieldValue::Long(l)) => *l,
            _ => 3,
        };
        let title = match rec.get(6) {
            Some(FieldValue::String(s)) if !s.is_empty() => Some(s.clone()),
            _ => None,
        };
        let control_first = match rec.get(7) {
            Some(FieldValue::String(s)) => s.clone(),
            _ => {
                return Err(Error::Validation {
                    element: "Dialog.Control_First".to_string(),
                    reason: "missing Control_First".to_string(),
                })
            }
        };
        let control_default = match rec.get(8) {
            Some(FieldValue::String(s)) if !s.is_empty() => Some(s.clone()),
            _ => None,
        };
        let control_cancel = match rec.get(9) {
            Some(FieldValue::String(s)) if !s.is_empty() => Some(s.clone()),
            _ => None,
        };

        Ok(Self {
            dialog,
            h_centering,
            v_centering,
            width,
            height,
            attributes,
            title,
            control_first,
            control_default,
            control_cancel,
        })
    }
}

/// Creates official schema for `Dialog` table.
///
/// # Returns
///
/// [`TableSchema`] for `Dialog`.
#[must_use]
pub fn dialog_schema() -> TableSchema {
    TableSchema::new("Dialog")
        .with_column(ColumnDef::new("Dialog", DataType::String { max_len: 72 }).primary_key())
        .with_column(ColumnDef::new("HCentering", DataType::Short))
        .with_column(ColumnDef::new("VCentering", DataType::Short))
        .with_column(ColumnDef::new("Width", DataType::Short))
        .with_column(ColumnDef::new("Height", DataType::Short))
        .with_column(ColumnDef::new("Attributes", DataType::Long))
        .with_column(
            ColumnDef::new("Title", DataType::String { max_len: 128 })
                .nullable()
                .localizable(),
        )
        .with_column(ColumnDef::new(
            "Control_First",
            DataType::String { max_len: 50 },
        ))
        .with_column(ColumnDef::new("Control_Default", DataType::String { max_len: 50 }).nullable())
        .with_column(ColumnDef::new("Control_Cancel", DataType::String { max_len: 50 }).nullable())
}

/// Creates official schema for `Control` table.
///
/// # Returns
///
/// [`TableSchema`] for `Control`.
#[must_use]
pub fn control_schema() -> TableSchema {
    TableSchema::new("Control")
        .with_column(ColumnDef::new("Dialog_", DataType::String { max_len: 72 }).primary_key())
        .with_column(ColumnDef::new("Control", DataType::String { max_len: 50 }).primary_key())
        .with_column(ColumnDef::new("Type", DataType::String { max_len: 20 }))
        .with_column(ColumnDef::new("X", DataType::Short))
        .with_column(ColumnDef::new("Y", DataType::Short))
        .with_column(ColumnDef::new("Width", DataType::Short))
        .with_column(ColumnDef::new("Height", DataType::Short))
        .with_column(ColumnDef::new("Attributes", DataType::Long))
        .with_column(ColumnDef::new("Property", DataType::String { max_len: 50 }).nullable())
        .with_column(
            ColumnDef::new("Text", DataType::String { max_len: 0 })
                .nullable()
                .localizable(),
        )
        .with_column(ColumnDef::new("Control_Next", DataType::String { max_len: 50 }).nullable())
        .with_column(
            ColumnDef::new("Help", DataType::String { max_len: 50 })
                .nullable()
                .localizable(),
        )
}

/// Creates official schema for `ControlCondition` table.
///
/// # Returns
///
/// [`TableSchema`] for `ControlCondition`.
#[must_use]
pub fn control_condition_schema() -> TableSchema {
    TableSchema::new("ControlCondition")
        .with_column(ColumnDef::new("Dialog_", DataType::String { max_len: 72 }).primary_key())
        .with_column(ColumnDef::new("Control_", DataType::String { max_len: 50 }).primary_key())
        .with_column(ColumnDef::new("Action", DataType::String { max_len: 50 }).primary_key())
        .with_column(ColumnDef::new("Condition", DataType::String { max_len: 255 }).primary_key())
}

/// Creates official schema for `ControlEvent` table.
///
/// # Returns
///
/// [`TableSchema`] for `ControlEvent`.
#[must_use]
pub fn control_event_schema() -> TableSchema {
    TableSchema::new("ControlEvent")
        .with_column(ColumnDef::new("Dialog_", DataType::String { max_len: 72 }).primary_key())
        .with_column(ColumnDef::new("Control_", DataType::String { max_len: 50 }).primary_key())
        .with_column(ColumnDef::new("Event", DataType::String { max_len: 50 }).primary_key())
        .with_column(ColumnDef::new("Argument", DataType::String { max_len: 255 }).primary_key())
        .with_column(ColumnDef::new("Condition", DataType::String { max_len: 255 }).primary_key())
        .with_column(ColumnDef::new("Ordering", DataType::Short).nullable())
}

/// Creates official schema for `EventMapping` table.
///
/// # Returns
///
/// [`TableSchema`] for `EventMapping`.
#[must_use]
pub fn event_mapping_schema() -> TableSchema {
    TableSchema::new("EventMapping")
        .with_column(ColumnDef::new("Dialog_", DataType::String { max_len: 72 }).primary_key())
        .with_column(ColumnDef::new("Control_", DataType::String { max_len: 50 }).primary_key())
        .with_column(ColumnDef::new("Event", DataType::String { max_len: 50 }).primary_key())
        .with_column(ColumnDef::new(
            "Attribute",
            DataType::String { max_len: 50 },
        ))
}

/// Creates official schema for `TextStyle` table.
///
/// # Returns
///
/// [`TableSchema`] for `TextStyle`.
#[must_use]
pub fn text_style_schema() -> TableSchema {
    TableSchema::new("TextStyle")
        .with_column(ColumnDef::new("TextStyle", DataType::String { max_len: 72 }).primary_key())
        .with_column(ColumnDef::new("FaceName", DataType::String { max_len: 32 }))
        .with_column(ColumnDef::new("Size", DataType::Short))
        .with_column(ColumnDef::new("Color", DataType::Long).nullable())
        .with_column(ColumnDef::new("StyleBits", DataType::Short).nullable())
}

/// Creates official schema for `RadioButton` table.
///
/// # Returns
///
/// [`TableSchema`] for `RadioButton`.
#[must_use]
pub fn radio_button_schema() -> TableSchema {
    TableSchema::new("RadioButton")
        .with_column(ColumnDef::new("Property", DataType::String { max_len: 72 }).primary_key())
        .with_column(ColumnDef::new("Order", DataType::Short).primary_key())
        .with_column(ColumnDef::new("Value", DataType::String { max_len: 64 }))
        .with_column(ColumnDef::new("X", DataType::Short))
        .with_column(ColumnDef::new("Y", DataType::Short))
        .with_column(ColumnDef::new("Width", DataType::Short))
        .with_column(ColumnDef::new("Height", DataType::Short))
        .with_column(ColumnDef::new("Text", DataType::String { max_len: 64 }).localizable())
        .with_column(
            ColumnDef::new("Help", DataType::String { max_len: 50 })
                .nullable()
                .localizable(),
        )
}

/// Creates official schema for `CheckBox` table.
///
/// # Returns
///
/// [`TableSchema`] for `CheckBox`.
#[must_use]
pub fn check_box_schema() -> TableSchema {
    TableSchema::new("CheckBox")
        .with_column(ColumnDef::new("Property", DataType::String { max_len: 72 }).primary_key())
        .with_column(ColumnDef::new("Value", DataType::String { max_len: 64 }))
}

/// Creates official schema for `ComboBox` table.
///
/// # Returns
///
/// [`TableSchema`] for `ComboBox`.
#[must_use]
pub fn combo_box_schema() -> TableSchema {
    TableSchema::new("ComboBox")
        .with_column(ColumnDef::new("Property", DataType::String { max_len: 72 }).primary_key())
        .with_column(ColumnDef::new("Order", DataType::Short).primary_key())
        .with_column(ColumnDef::new("Value", DataType::String { max_len: 64 }))
        .with_column(ColumnDef::new("Text", DataType::String { max_len: 64 }).localizable())
}

/// Creates official schema for `ListBox` table.
///
/// # Returns
///
/// [`TableSchema`] for `ListBox`.
#[must_use]
pub fn list_box_schema() -> TableSchema {
    TableSchema::new("ListBox")
        .with_column(ColumnDef::new("Property", DataType::String { max_len: 72 }).primary_key())
        .with_column(ColumnDef::new("Order", DataType::Short).primary_key())
        .with_column(ColumnDef::new("Value", DataType::String { max_len: 64 }))
        .with_column(ColumnDef::new("Text", DataType::String { max_len: 64 }).localizable())
}

/// Creates official schema for `ListView` table.
///
/// # Returns
///
/// [`TableSchema`] for `ListView`.
#[must_use]
pub fn list_view_schema() -> TableSchema {
    TableSchema::new("ListView")
        .with_column(ColumnDef::new("Property", DataType::String { max_len: 72 }).primary_key())
        .with_column(ColumnDef::new("Order", DataType::Short).primary_key())
        .with_column(ColumnDef::new("Value", DataType::String { max_len: 64 }))
        .with_column(ColumnDef::new("Text", DataType::String { max_len: 64 }).localizable())
        .with_column(ColumnDef::new("Binary_", DataType::String { max_len: 72 }).nullable())
}

/// Creates official schema for `Billboard` table.
///
/// # Returns
///
/// [`TableSchema`] for `Billboard`.
#[must_use]
pub fn billboard_schema() -> TableSchema {
    TableSchema::new("Billboard")
        .with_column(ColumnDef::new("Billboard", DataType::String { max_len: 50 }).primary_key())
        .with_column(ColumnDef::new("Feature_", DataType::String { max_len: 38 }))
        .with_column(ColumnDef::new("Action_", DataType::String { max_len: 50 }))
        .with_column(ColumnDef::new("Ordering", DataType::Short).nullable())
}

/// Creates official schema for `BBControl` table.
///
/// # Returns
///
/// [`TableSchema`] for `BBControl`.
#[must_use]
pub fn bb_control_schema() -> TableSchema {
    TableSchema::new("BBControl")
        .with_column(ColumnDef::new("Billboard_", DataType::String { max_len: 50 }).primary_key())
        .with_column(ColumnDef::new("BBControl", DataType::String { max_len: 50 }).primary_key())
        .with_column(ColumnDef::new("Type", DataType::String { max_len: 50 }))
        .with_column(ColumnDef::new("X", DataType::Short))
        .with_column(ColumnDef::new("Y", DataType::Short))
        .with_column(ColumnDef::new("Width", DataType::Short))
        .with_column(ColumnDef::new("Height", DataType::Short))
        .with_column(ColumnDef::new("Attributes", DataType::Long))
        .with_column(ColumnDef::new("Text", DataType::String { max_len: 50 }).localizable())
}

/// Creates official schema for `ActionText` table.
///
/// # Returns
///
/// [`TableSchema`] for `ActionText`.
#[must_use]
pub fn action_text_schema() -> TableSchema {
    TableSchema::new("ActionText")
        .with_column(ColumnDef::new("Action", DataType::String { max_len: 72 }).primary_key())
        .with_column(
            ColumnDef::new("Description", DataType::String { max_len: 0 })
                .nullable()
                .localizable(),
        )
        .with_column(
            ColumnDef::new("Template", DataType::String { max_len: 0 })
                .nullable()
                .localizable(),
        )
}

/// Creates official schema for `Error` table.
///
/// # Returns
///
/// [`TableSchema`] for `Error`.
#[must_use]
pub fn error_schema() -> TableSchema {
    TableSchema::new("Error")
        .with_column(ColumnDef::new("Error", DataType::Short).primary_key())
        .with_column(
            ColumnDef::new("Message", DataType::String { max_len: 0 })
                .nullable()
                .localizable(),
        )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dialog_row_roundtrip() {
        let row = DialogRow {
            dialog: "InstallDlg".to_string(),
            h_centering: 50,
            v_centering: 50,
            width: 370,
            height: 270,
            attributes: 3,
            title: Some("Installer Wizard".to_string()),
            control_first: "NextButton".to_string(),
            control_default: Some("NextButton".to_string()),
            control_cancel: Some("CancelButton".to_string()),
        };

        let rec = row.to_record();
        assert_eq!(rec.len(), 10);
        let parsed = DialogRow::from_record(&rec);
        assert_eq!(parsed, Ok(row));

        // Minimal (all Optionals are None)
        let min_row = DialogRow {
            dialog: "MinDlg".to_string(),
            h_centering: 50,
            v_centering: 50,
            width: 370,
            height: 270,
            attributes: 3,
            title: None,
            control_first: "FirstCtrl".to_string(),
            control_default: None,
            control_cancel: None,
        };
        let min_rec = min_row.to_record();
        assert_eq!(DialogRow::from_record(&min_rec), Ok(min_row));

        // Defaults fallback and empty string options
        let fallback_rec = Record::with_fields(vec![
            FieldValue::String("DlgFallback".to_string()),
            FieldValue::Null,                  // h_centering fallback to 50
            FieldValue::Null,                  // v_centering fallback to 50
            FieldValue::Null,                  // width fallback to 370
            FieldValue::Null,                  // height fallback to 270
            FieldValue::Null,                  // attributes fallback to 3
            FieldValue::String(String::new()), // title empty string -> None
            FieldValue::String("Ctrl1".to_string()),
            FieldValue::String(String::new()), // control_default empty string -> None
            FieldValue::String(String::new()), // control_cancel empty string -> None
        ]);
        let fallback_row = DialogRow::from_record(&fallback_rec);
        assert_eq!(
            fallback_row,
            Ok(DialogRow {
                dialog: "DlgFallback".to_string(),
                h_centering: 50,
                v_centering: 50,
                width: 370,
                height: 270,
                attributes: 3,
                title: None,
                control_first: "Ctrl1".to_string(),
                control_default: None,
                control_cancel: None,
            })
        );

        assert!(DialogRow::from_record(&Record::new()).is_err());
        assert!(DialogRow::from_record(&Record::with_fields(vec![FieldValue::Null; 10])).is_err());
        let bad_first = Record::with_fields(vec![
            FieldValue::String("Dlg".to_string()),
            FieldValue::Short(50),
            FieldValue::Short(50),
            FieldValue::Short(300),
            FieldValue::Short(200),
            FieldValue::Long(3),
            FieldValue::Null,
            FieldValue::Null, // Control_First is null
            FieldValue::Null,
            FieldValue::Null,
        ]);
        assert!(DialogRow::from_record(&bad_first).is_err());
    }

    #[test]
    fn test_ui_schemas() {
        assert_eq!(dialog_schema().name, "Dialog");
        assert_eq!(control_schema().name, "Control");
        assert_eq!(control_condition_schema().name, "ControlCondition");
        assert_eq!(control_event_schema().name, "ControlEvent");
        assert_eq!(event_mapping_schema().name, "EventMapping");
        assert_eq!(text_style_schema().name, "TextStyle");
        assert_eq!(radio_button_schema().name, "RadioButton");
        assert_eq!(check_box_schema().name, "CheckBox");
        assert_eq!(combo_box_schema().name, "ComboBox");
        assert_eq!(list_box_schema().name, "ListBox");
        assert_eq!(list_view_schema().name, "ListView");
        assert_eq!(billboard_schema().name, "Billboard");
        assert_eq!(bb_control_schema().name, "BBControl");
        assert_eq!(action_text_schema().name, "ActionText");
        assert_eq!(error_schema().name, "Error");
    }
}

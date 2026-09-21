//! File management tables for Windows Installer database.
//!
//! Implements schemas for:
//! - `CreateFolder`
//! - `DuplicateFile`
//! - `MoveFile`
//! - `RemoveFile`
//! - `IniFile`
//! - `RemoveIniFile`

use crate::database::catalogs::TableSchema;
use crate::database::column::{ColumnDef, DataType};
use crate::database::tables::record::{FieldValue, Record};
use crate::database::tables::types::{ComponentName, DirectoryId};
use crate::error::{Error, Result};

/// Row in the `CreateFolder` table.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreateFolderRow {
    /// Directory to create (foreign key into Directory table).
    pub directory: DirectoryId,
    /// Controlling component (foreign key into Component table).
    pub component: ComponentName,
}

impl CreateFolderRow {
    /// Converts this row into a generic [`Record`].
    ///
    /// # Returns
    ///
    /// A generic [`Record`].
    #[must_use]
    pub fn to_record(&self) -> Record {
        Record::with_fields(vec![
            FieldValue::String(self.directory.as_str().to_string()),
            FieldValue::String(self.component.as_str().to_string()),
        ])
    }

    /// Parses a [`CreateFolderRow`] from a generic [`Record`].
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
        let directory = match rec.get(0) {
            Some(FieldValue::String(s)) => DirectoryId::new(s.as_str())?,
            _ => {
                return Err(Error::Validation {
                    element: "CreateFolder.Directory_".to_string(),
                    reason: "missing Directory_".to_string(),
                })
            }
        };
        let component = match rec.get(1) {
            Some(FieldValue::String(s)) => ComponentName::new(s.as_str())?,
            _ => {
                return Err(Error::Validation {
                    element: "CreateFolder.Component_".to_string(),
                    reason: "missing Component_".to_string(),
                })
            }
        };
        Ok(Self {
            directory,
            component,
        })
    }
}

/// Creates the official schema for `CreateFolder` table.
///
/// # Returns
///
/// [`TableSchema`] for `CreateFolder`.
#[must_use]
pub fn create_folder_schema() -> TableSchema {
    TableSchema::new("CreateFolder")
        .with_column(ColumnDef::new("Directory_", DataType::String { max_len: 72 }).primary_key())
        .with_column(ColumnDef::new("Component_", DataType::String { max_len: 72 }).primary_key())
}

/// Creates the official schema for `DuplicateFile` table.
///
/// # Returns
///
/// [`TableSchema`] for `DuplicateFile`.
#[must_use]
pub fn duplicate_file_schema() -> TableSchema {
    TableSchema::new("DuplicateFile")
        .with_column(ColumnDef::new("FileKey", DataType::String { max_len: 72 }).primary_key())
        .with_column(ColumnDef::new(
            "Component_",
            DataType::String { max_len: 72 },
        ))
        .with_column(ColumnDef::new("File_", DataType::String { max_len: 72 }))
        .with_column(
            ColumnDef::new("DestName", DataType::String { max_len: 255 })
                .nullable()
                .localizable(),
        )
        .with_column(ColumnDef::new("DestFolder", DataType::String { max_len: 72 }).nullable())
}

/// Creates the official schema for `MoveFile` table.
///
/// # Returns
///
/// [`TableSchema`] for `MoveFile`.
#[must_use]
pub fn move_file_schema() -> TableSchema {
    TableSchema::new("MoveFile")
        .with_column(ColumnDef::new("FileKey", DataType::String { max_len: 72 }).primary_key())
        .with_column(ColumnDef::new(
            "Component_",
            DataType::String { max_len: 72 },
        ))
        .with_column(
            ColumnDef::new("SourceName", DataType::String { max_len: 255 })
                .nullable()
                .localizable(),
        )
        .with_column(
            ColumnDef::new("DestName", DataType::String { max_len: 255 })
                .nullable()
                .localizable(),
        )
        .with_column(ColumnDef::new(
            "SourceFolder",
            DataType::String { max_len: 72 },
        ))
        .with_column(ColumnDef::new(
            "DestFolder",
            DataType::String { max_len: 72 },
        ))
        .with_column(ColumnDef::new("Options", DataType::Short))
}

/// Creates the official schema for `RemoveFile` table.
///
/// # Returns
///
/// [`TableSchema`] for `RemoveFile`.
#[must_use]
pub fn remove_file_schema() -> TableSchema {
    TableSchema::new("RemoveFile")
        .with_column(ColumnDef::new("FileKey", DataType::String { max_len: 72 }).primary_key())
        .with_column(ColumnDef::new(
            "Component_",
            DataType::String { max_len: 72 },
        ))
        .with_column(
            ColumnDef::new("FileName", DataType::String { max_len: 255 })
                .nullable()
                .localizable(),
        )
        .with_column(ColumnDef::new(
            "DirProperty",
            DataType::String { max_len: 72 },
        ))
        .with_column(ColumnDef::new("InstallMode", DataType::Short))
}

/// Row in the `RemoveFile` table.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemoveFileRow {
    /// File record primary key (max 72 chars).
    pub file_key: String,
    /// Controlling component (foreign key into Component table).
    pub component: ComponentName,
    /// Target file name, or None to delete the directory itself.
    pub file_name: Option<String>,
    /// Directory property or identifier.
    pub dir_property: String,
    /// Install mode: 1 = install, 2 = uninstall, 3 = both.
    pub install_mode: i16,
}

impl RemoveFileRow {
    /// Converts this row into a generic [`Record`].
    ///
    /// # Returns
    ///
    /// A generic [`Record`].
    #[must_use]
    pub fn to_record(&self) -> Record {
        Record::with_fields(vec![
            FieldValue::String(self.file_key.clone()),
            FieldValue::String(self.component.as_str().to_string()),
            self.file_name
                .as_ref()
                .map_or(FieldValue::Null, |n| FieldValue::String(n.clone())),
            FieldValue::String(self.dir_property.clone()),
            FieldValue::Short(self.install_mode),
        ])
    }

    /// Parses a [`RemoveFileRow`] from a generic [`Record`].
    ///
    /// # Arguments
    ///
    /// * `rec` - Generic record.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Validation`] or [`Error::RecordLengthMismatch`].
    pub fn from_record(rec: &Record) -> Result<Self> {
        if rec.len() < 5 {
            return Err(Error::RecordLengthMismatch {
                expected: 5,
                actual: rec.len(),
            });
        }
        let file_key = match rec.get(0) {
            Some(FieldValue::String(s)) => s.clone(),
            _ => {
                return Err(Error::Validation {
                    element: "RemoveFile.FileKey".to_string(),
                    reason: "missing FileKey".to_string(),
                })
            }
        };
        let component = match rec.get(1) {
            Some(FieldValue::String(s)) => ComponentName::new(s.as_str())?,
            _ => {
                return Err(Error::Validation {
                    element: "RemoveFile.Component_".to_string(),
                    reason: "missing Component_".to_string(),
                })
            }
        };
        let file_name = match rec.get(2) {
            Some(FieldValue::String(s)) if !s.is_empty() => Some(s.clone()),
            _ => None,
        };
        let dir_property = match rec.get(3) {
            Some(FieldValue::String(s)) => s.clone(),
            _ => {
                return Err(Error::Validation {
                    element: "RemoveFile.DirProperty".to_string(),
                    reason: "missing DirProperty".to_string(),
                })
            }
        };
        let install_mode = match rec.get(4) {
            Some(FieldValue::Short(m)) => *m,
            _ => 3,
        };
        Ok(Self {
            file_key,
            component,
            file_name,
            dir_property,
            install_mode,
        })
    }
}

/// Creates the official schema for `IniFile` table.
///
/// # Returns
///
/// [`TableSchema`] for `IniFile`.
#[must_use]
pub fn ini_file_schema() -> TableSchema {
    TableSchema::new("IniFile")
        .with_column(ColumnDef::new("IniFileKey", DataType::String { max_len: 72 }).primary_key())
        .with_column(ColumnDef::new("FileName", DataType::String { max_len: 255 }).localizable())
        .with_column(ColumnDef::new("DirProperty", DataType::String { max_len: 72 }).nullable())
        .with_column(ColumnDef::new("Section", DataType::String { max_len: 96 }).localizable())
        .with_column(ColumnDef::new("Key", DataType::String { max_len: 128 }).localizable())
        .with_column(ColumnDef::new("Value", DataType::String { max_len: 255 }).localizable())
        .with_column(ColumnDef::new("Action", DataType::Short))
        .with_column(ColumnDef::new(
            "Component_",
            DataType::String { max_len: 72 },
        ))
}

/// Creates the official schema for `RemoveIniFile` table.
///
/// # Returns
///
/// [`TableSchema`] for `RemoveIniFile`.
#[must_use]
pub fn remove_ini_file_schema() -> TableSchema {
    TableSchema::new("RemoveIniFile")
        .with_column(
            ColumnDef::new("RemoveIniFileKey", DataType::String { max_len: 72 }).primary_key(),
        )
        .with_column(ColumnDef::new("FileName", DataType::String { max_len: 255 }).localizable())
        .with_column(ColumnDef::new("DirProperty", DataType::String { max_len: 72 }).nullable())
        .with_column(ColumnDef::new("Section", DataType::String { max_len: 96 }).localizable())
        .with_column(ColumnDef::new("Key", DataType::String { max_len: 128 }).localizable())
        .with_column(
            ColumnDef::new("Value", DataType::String { max_len: 255 })
                .nullable()
                .localizable(),
        )
        .with_column(ColumnDef::new("Action", DataType::Short))
        .with_column(ColumnDef::new(
            "Component_",
            DataType::String { max_len: 72 },
        ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_folder_row_roundtrip() -> Result<()> {
        let dir = DirectoryId::new("TARGETDIR")?;
        let comp = ComponentName::new("Comp1")?;

        let row = CreateFolderRow {
            directory: dir,
            component: comp,
        };
        let rec = row.to_record();
        assert_eq!(rec.len(), 2);
        let parsed = CreateFolderRow::from_record(&rec);
        assert_eq!(parsed, Ok(row));

        // Short record
        assert!(CreateFolderRow::from_record(&Record::new()).is_err());

        // Missing Directory_ (field 0 not string)
        let bad_dir = Record::with_fields(vec![
            FieldValue::Null,
            FieldValue::String("Comp1".to_string()),
        ]);
        assert_eq!(
            CreateFolderRow::from_record(&bad_dir),
            Err(Error::Validation {
                element: "CreateFolder.Directory_".to_string(),
                reason: "missing Directory_".to_string(),
            })
        );

        // Invalid DirectoryId (empty string)
        let invalid_dir = Record::with_fields(vec![
            FieldValue::String(String::new()),
            FieldValue::String("Comp1".to_string()),
        ]);
        assert!(CreateFolderRow::from_record(&invalid_dir).is_err());

        // Missing Component_ (field 1 not string)
        let bad_comp = Record::with_fields(vec![
            FieldValue::String("TARGETDIR".to_string()),
            FieldValue::Null,
        ]);
        assert_eq!(
            CreateFolderRow::from_record(&bad_comp),
            Err(Error::Validation {
                element: "CreateFolder.Component_".to_string(),
                reason: "missing Component_".to_string(),
            })
        );

        // Invalid ComponentName (empty string)
        let invalid_comp = Record::with_fields(vec![
            FieldValue::String("TARGETDIR".to_string()),
            FieldValue::String(String::new()),
        ]);
        assert!(CreateFolderRow::from_record(&invalid_comp).is_err());

        Ok(())
    }

    #[test]
    fn test_remove_file_row_roundtrip() -> Result<()> {
        let comp = ComponentName::new("Comp1")?;
        let row = RemoveFileRow {
            file_key: "RemFile1".to_string(),
            component: comp.clone(),
            file_name: Some("test.txt".to_string()),
            dir_property: "INSTALLDIR".to_string(),
            install_mode: 3,
        };
        let rec = row.to_record();
        assert_eq!(rec.len(), 5);
        let parsed = RemoveFileRow::from_record(&rec)?;
        assert_eq!(parsed, row);

        // Test with None file_name (directory removal)
        let row_dir = RemoveFileRow {
            file_key: "RemDir1".to_string(),
            component: comp,
            file_name: None,
            dir_property: "TARGETDIR".to_string(),
            install_mode: 1,
        };
        let rec_dir = row_dir.to_record();
        let parsed_dir = RemoveFileRow::from_record(&rec_dir)?;
        assert_eq!(parsed_dir, row_dir);

        // Short record
        assert!(RemoveFileRow::from_record(&Record::new()).is_err());

        // Missing FileKey
        let bad_fk = Record::with_fields(vec![
            FieldValue::Null,
            FieldValue::String("Comp1".to_string()),
            FieldValue::Null,
            FieldValue::String("INSTALLDIR".to_string()),
            FieldValue::Short(3),
        ]);
        assert_eq!(
            RemoveFileRow::from_record(&bad_fk),
            Err(Error::Validation {
                element: "RemoveFile.FileKey".to_string(),
                reason: "missing FileKey".to_string(),
            })
        );

        // Missing Component_
        let bad_comp = Record::with_fields(vec![
            FieldValue::String("RemFile1".to_string()),
            FieldValue::Null,
            FieldValue::Null,
            FieldValue::String("INSTALLDIR".to_string()),
            FieldValue::Short(3),
        ]);
        assert_eq!(
            RemoveFileRow::from_record(&bad_comp),
            Err(Error::Validation {
                element: "RemoveFile.Component_".to_string(),
                reason: "missing Component_".to_string(),
            })
        );

        // Invalid ComponentName (empty string)
        let invalid_comp = Record::with_fields(vec![
            FieldValue::String("RemFile1".to_string()),
            FieldValue::String(String::new()),
            FieldValue::Null,
            FieldValue::String("INSTALLDIR".to_string()),
            FieldValue::Short(3),
        ]);
        assert!(RemoveFileRow::from_record(&invalid_comp).is_err());

        // Empty file_name string falling back to None, non-short install_mode falling back to 3
        let empty_fn_rec = Record::with_fields(vec![
            FieldValue::String("RemFile1".to_string()),
            FieldValue::String("Comp1".to_string()),
            FieldValue::String(String::new()), // empty string -> None
            FieldValue::String("INSTALLDIR".to_string()),
            FieldValue::Null, // non-short -> 3
        ]);
        let parsed_empty = RemoveFileRow::from_record(&empty_fn_rec)?;
        assert!(parsed_empty.file_name.is_none());
        assert_eq!(parsed_empty.install_mode, 3);

        // Missing DirProperty
        let bad_dir = Record::with_fields(vec![
            FieldValue::String("RemFile1".to_string()),
            FieldValue::String("Comp1".to_string()),
            FieldValue::Null,
            FieldValue::Null,
            FieldValue::Short(3),
        ]);
        assert_eq!(
            RemoveFileRow::from_record(&bad_dir),
            Err(Error::Validation {
                element: "RemoveFile.DirProperty".to_string(),
                reason: "missing DirProperty".to_string(),
            })
        );

        Ok(())
    }

    #[test]
    fn test_file_mgmt_schemas() {
        assert_eq!(create_folder_schema().name, "CreateFolder");
        assert_eq!(duplicate_file_schema().name, "DuplicateFile");
        assert_eq!(move_file_schema().name, "MoveFile");
        assert_eq!(remove_file_schema().name, "RemoveFile");
        assert_eq!(ini_file_schema().name, "IniFile");
        assert_eq!(remove_ini_file_schema().name, "RemoveIniFile");
    }
}

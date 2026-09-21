//! Cross-platform POSIX extension tables for MSI databases.
//!
//! Implements schemas for:
//! - `PosixFile`
//! - `PosixSymlink`
//! - `PosixDaemon`
//! - `PosixAcl`
//! - `PosixDesktop`

use crate::database::catalogs::TableSchema;
use crate::database::column::{ColumnDef, DataType};
use crate::database::tables::record::{FieldValue, Record};
use crate::database::tables::types::{ComponentName, DirectoryId, FileKey};
use crate::error::{Error, Result};

/// Row in the `PosixFile` table.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PosixFileRow {
    /// Foreign key to File table.
    pub file: FileKey,
    /// POSIX file mode (octal permissions e.g. `0o755`).
    pub mode_octal: i32,
    /// Owner user name (nullable, max 64 chars).
    pub owner_user: Option<String>,
    /// Owner group name (nullable, max 64 chars).
    pub owner_group: Option<String>,
    /// Extended flags (nullable).
    pub flags: Option<i32>,
}

impl PosixFileRow {
    /// Converts this row into a generic [`Record`].
    ///
    /// # Returns
    ///
    /// A generic [`Record`].
    #[must_use]
    pub fn to_record(&self) -> Record {
        Record::with_fields(vec![
            FieldValue::String(self.file.as_str().to_string()),
            FieldValue::Long(self.mode_octal),
            self.owner_user
                .as_ref()
                .map_or(FieldValue::Null, |u| FieldValue::String(u.clone())),
            self.owner_group
                .as_ref()
                .map_or(FieldValue::Null, |g| FieldValue::String(g.clone())),
            self.flags.map_or(FieldValue::Null, FieldValue::Long),
        ])
    }

    /// Parses a [`PosixFileRow`] from a generic [`Record`].
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
        let file = match rec.get(0) {
            Some(FieldValue::String(s)) => FileKey::new(s.as_str())?,
            _ => {
                return Err(Error::Validation {
                    element: "PosixFile.File_".to_string(),
                    reason: "missing File_".to_string(),
                })
            }
        };
        let mode_octal = match rec.get(1) {
            Some(FieldValue::Long(m)) => *m,
            _ => 0o644,
        };
        let owner_user = match rec.get(2) {
            Some(FieldValue::String(s)) if !s.is_empty() => Some(s.clone()),
            _ => None,
        };
        let owner_group = match rec.get(3) {
            Some(FieldValue::String(s)) if !s.is_empty() => Some(s.clone()),
            _ => None,
        };
        let flags = match rec.get(4) {
            Some(FieldValue::Long(f)) => Some(*f),
            _ => None,
        };
        Ok(Self {
            file,
            mode_octal,
            owner_user,
            owner_group,
            flags,
        })
    }
}

/// Creates official schema for `PosixFile` table.
///
/// # Returns
///
/// [`TableSchema`] for `PosixFile`.
#[must_use]
pub fn posix_file_schema() -> TableSchema {
    TableSchema::new("PosixFile")
        .with_column(ColumnDef::new("File_", DataType::String { max_len: 72 }).primary_key())
        .with_column(ColumnDef::new("ModeOctal", DataType::Long))
        .with_column(ColumnDef::new("OwnerUser", DataType::String { max_len: 64 }).nullable())
        .with_column(ColumnDef::new("OwnerGroup", DataType::String { max_len: 64 }).nullable())
        .with_column(ColumnDef::new("Flags", DataType::Long).nullable())
}

/// Row in the `PosixSymlink` table.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PosixSymlinkRow {
    /// Symlink primary key identifier (max 72 chars).
    pub symlink_key: String,
    /// Target path (relative or absolute, max 255 chars).
    pub target_path: String,
    /// Directory containing the symlink.
    pub link_directory: DirectoryId,
    /// Name of the symlink file.
    pub link_name: String,
    /// Controlling component.
    pub component: ComponentName,
}

impl PosixSymlinkRow {
    /// Converts this row into a generic [`Record`].
    ///
    /// # Returns
    ///
    /// A generic [`Record`].
    #[must_use]
    pub fn to_record(&self) -> Record {
        Record::with_fields(vec![
            FieldValue::String(self.symlink_key.clone()),
            FieldValue::String(self.target_path.clone()),
            FieldValue::String(self.link_directory.as_str().to_string()),
            FieldValue::String(self.link_name.clone()),
            FieldValue::String(self.component.as_str().to_string()),
        ])
    }

    /// Parses a [`PosixSymlinkRow`] from a generic [`Record`].
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
        let symlink_key = match rec.get(0) {
            Some(FieldValue::String(s)) => s.clone(),
            _ => {
                return Err(Error::Validation {
                    element: "PosixSymlink.SymlinkKey".to_string(),
                    reason: "missing SymlinkKey".to_string(),
                })
            }
        };
        let target_path = match rec.get(1) {
            Some(FieldValue::String(s)) => s.clone(),
            _ => String::new(),
        };
        let link_directory = match rec.get(2) {
            Some(FieldValue::String(s)) => DirectoryId::new(s.as_str())?,
            _ => {
                return Err(Error::Validation {
                    element: "PosixSymlink.LinkDirectory_".to_string(),
                    reason: "missing LinkDirectory_".to_string(),
                })
            }
        };
        let link_name = match rec.get(3) {
            Some(FieldValue::String(s)) => s.clone(),
            _ => String::new(),
        };
        let component = match rec.get(4) {
            Some(FieldValue::String(s)) => ComponentName::new(s.as_str())?,
            _ => {
                return Err(Error::Validation {
                    element: "PosixSymlink.Component_".to_string(),
                    reason: "missing Component_".to_string(),
                })
            }
        };
        Ok(Self {
            symlink_key,
            target_path,
            link_directory,
            link_name,
            component,
        })
    }
}

/// Creates official schema for `PosixSymlink` table.
///
/// # Returns
///
/// [`TableSchema`] for `PosixSymlink`.
#[must_use]
pub fn posix_symlink_schema() -> TableSchema {
    TableSchema::new("PosixSymlink")
        .with_column(ColumnDef::new("SymlinkKey", DataType::String { max_len: 72 }).primary_key())
        .with_column(ColumnDef::new(
            "TargetPath",
            DataType::String { max_len: 255 },
        ))
        .with_column(ColumnDef::new(
            "LinkDirectory_",
            DataType::String { max_len: 72 },
        ))
        .with_column(ColumnDef::new(
            "LinkName",
            DataType::String { max_len: 255 },
        ))
        .with_column(ColumnDef::new(
            "Component_",
            DataType::String { max_len: 72 },
        ))
}

/// Creates official schema for `PosixDaemon` table.
///
/// # Returns
///
/// [`TableSchema`] for `PosixDaemon`.
#[must_use]
pub fn posix_daemon_schema() -> TableSchema {
    TableSchema::new("PosixDaemon")
        .with_column(ColumnDef::new("Service_", DataType::String { max_len: 72 }).primary_key())
        .with_column(ColumnDef::new(
            "SupervisorType",
            DataType::String { max_len: 32 },
        ))
        .with_column(ColumnDef::new("UnitTemplate_", DataType::String { max_len: 72 }).nullable())
        .with_column(ColumnDef::new(
            "RestartPolicy",
            DataType::String { max_len: 32 },
        ))
        .with_column(ColumnDef::new("RunAsUser", DataType::String { max_len: 64 }).nullable())
}

/// Creates official schema for `PosixAcl` table.
///
/// # Returns
///
/// [`TableSchema`] for `PosixAcl`.
#[must_use]
pub fn posix_acl_schema() -> TableSchema {
    TableSchema::new("PosixAcl")
        .with_column(ColumnDef::new("AclKey", DataType::String { max_len: 72 }).primary_key())
        .with_column(ColumnDef::new("File_", DataType::String { max_len: 72 }))
        .with_column(ColumnDef::new(
            "PrincipalType",
            DataType::String { max_len: 16 },
        ))
        .with_column(ColumnDef::new(
            "PrincipalName",
            DataType::String { max_len: 64 },
        ))
        .with_column(ColumnDef::new("Permissions", DataType::Long))
        .with_column(ColumnDef::new("Flags", DataType::Long))
}

/// Creates official schema for `PosixDesktop` table.
///
/// # Returns
///
/// [`TableSchema`] for `PosixDesktop`.
#[must_use]
pub fn posix_desktop_schema() -> TableSchema {
    TableSchema::new("PosixDesktop")
        .with_column(ColumnDef::new("Shortcut_", DataType::String { max_len: 72 }).primary_key())
        .with_column(ColumnDef::new("Categories", DataType::String { max_len: 255 }).nullable())
        .with_column(ColumnDef::new("MimeTypes", DataType::String { max_len: 255 }).nullable())
        .with_column(
            ColumnDef::new("Keywords", DataType::String { max_len: 255 })
                .nullable()
                .localizable(),
        )
        .with_column(ColumnDef::new("Terminal", DataType::Short).nullable())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_posix_file_row_roundtrip() -> Result<()> {
        let file = FileKey::new("bin1")?;
        let row = PosixFileRow {
            file: file.clone(),
            mode_octal: 0o755,
            owner_user: Some("root".to_string()),
            owner_group: Some("wheel".to_string()),
            flags: Some(0),
        };
        let rec = row.to_record();
        assert_eq!(rec.len(), 5);
        let parsed = PosixFileRow::from_record(&rec);
        assert_eq!(parsed, Ok(row));

        // Minimal (all None)
        let min_row = PosixFileRow {
            file: file.clone(),
            mode_octal: 0o644,
            owner_user: None,
            owner_group: None,
            flags: None,
        };
        let min_rec = min_row.to_record();
        assert_eq!(PosixFileRow::from_record(&min_rec), Ok(min_row));

        // Fallback default mode and empty user/group strings
        let fallback_rec = Record::with_fields(vec![
            FieldValue::String("bin1".to_string()),
            FieldValue::Null,                  // default mode 0o644
            FieldValue::String(String::new()), // empty user -> None
            FieldValue::String(String::new()), // empty group -> None
            FieldValue::Null,                  // flags -> None
        ]);
        assert_eq!(
            PosixFileRow::from_record(&fallback_rec),
            Ok(PosixFileRow {
                file,
                mode_octal: 0o644,
                owner_user: None,
                owner_group: None,
                flags: None,
            })
        );

        // Errors
        assert!(PosixFileRow::from_record(&Record::new()).is_err());
        assert!(
            PosixFileRow::from_record(&Record::with_fields(vec![FieldValue::Null; 5])).is_err()
        );
        Ok(())
    }

    #[test]
    fn test_posix_symlink_row_roundtrip() -> Result<()> {
        let dir = DirectoryId::new("BINDIR")?;
        let comp = ComponentName::new("Comp1")?;
        let row = PosixSymlinkRow {
            symlink_key: "sym1".to_string(),
            target_path: "/usr/bin/app".to_string(),
            link_directory: dir.clone(),
            link_name: "app".to_string(),
            component: comp.clone(),
        };
        let rec = row.to_record();
        assert_eq!(rec.len(), 5);
        let parsed = PosixSymlinkRow::from_record(&rec);
        assert_eq!(parsed, Ok(row));

        // Fallbacks for optional target_path and link_name
        let fallback_rec = Record::with_fields(vec![
            FieldValue::String("symFallback".to_string()),
            FieldValue::Null, // default target_path ""
            FieldValue::String("BINDIR".to_string()),
            FieldValue::Null, // default link_name ""
            FieldValue::String("Comp1".to_string()),
        ]);
        assert_eq!(
            PosixSymlinkRow::from_record(&fallback_rec),
            Ok(PosixSymlinkRow {
                symlink_key: "symFallback".to_string(),
                target_path: String::new(),
                link_directory: dir,
                link_name: String::new(),
                component: comp,
            })
        );

        // Missing link_directory error
        let bad_dir_rec = Record::with_fields(vec![
            FieldValue::String("sym2".to_string()),
            FieldValue::String("/target".to_string()),
            FieldValue::Null, // missing LinkDirectory_
            FieldValue::String("link".to_string()),
            FieldValue::String("Comp1".to_string()),
        ]);
        assert!(PosixSymlinkRow::from_record(&bad_dir_rec).is_err());

        // Missing component error
        let bad_comp_rec = Record::with_fields(vec![
            FieldValue::String("sym3".to_string()),
            FieldValue::String("/target".to_string()),
            FieldValue::String("BINDIR".to_string()),
            FieldValue::String("link".to_string()),
            FieldValue::Null, // missing Component_
        ]);
        assert!(PosixSymlinkRow::from_record(&bad_comp_rec).is_err());

        assert!(PosixSymlinkRow::from_record(&Record::new()).is_err());
        assert!(
            PosixSymlinkRow::from_record(&Record::with_fields(vec![FieldValue::Null; 5])).is_err()
        );
        Ok(())
    }

    #[test]
    fn test_posix_schemas() {
        assert_eq!(posix_file_schema().name, "PosixFile");
        assert_eq!(posix_symlink_schema().name, "PosixSymlink");
        assert_eq!(posix_daemon_schema().name, "PosixDaemon");
        assert_eq!(posix_acl_schema().name, "PosixAcl");
        assert_eq!(posix_desktop_schema().name, "PosixDesktop");
    }
}

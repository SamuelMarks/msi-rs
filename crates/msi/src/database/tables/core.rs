//! Core packaging tables for Windows Installer database.
//!
//! Implements schemas and strongly-typed rows for:
//! - `Component`
//! - `Feature`
//! - `FeatureComponents`
//! - `Directory`
//! - `File`
//! - `FileHash`
//! - `Media`
//! - `Property`
//! - `Binary`
//! - `Font`
//! - `PatchPackage`
//! - `ModuleConfiguration`
//! - `ModuleSubstitution`
//! - `ModuleIgnoreModularization`
//! - `ModuleSignature`
//! - `ModuleComponents`
//! - `ModuleDependency`
//! - `ModuleExclusion`

use crate::database::catalogs::TableSchema;
use crate::database::column::{ColumnDef, DataType};
use crate::database::tables::record::{FieldValue, Record};
use crate::database::tables::types::{
    ComponentGuid, ComponentName, DirectoryId, FeatureName, FileKey, PropertyName,
};
use crate::error::{Error, Result};

/// Bit flags for `Component.Attributes`.
pub mod component_attributes {
    /// Component cannot be run from source (`0x0001`).
    pub const LOCAL_ONLY: i16 = 0x0001;
    /// Component can only be run from source (`0x0002`).
    pub const SOURCE_ONLY: i16 = 0x0002;
    /// Component can be run locally or from source (`0x0004`).
    pub const OPTIONAL: i16 = 0x0004;
    /// Key path is an entry in the Registry table (`0x0010`).
    pub const REGISTRY_KEY_PATH: i16 = 0x0010;
    /// Component shares DLL reference count (`0x0020`).
    pub const SHARED_DLL_REF_COUNT: i16 = 0x0020;
    /// Component is not removed upon uninstall (`0x0040`).
    pub const PERMANENT: i16 = 0x0040;
    /// Never overwrite an existing keypath file (`0x0080`).
    pub const NEVER_OVERWRITE: i16 = 0x0080;
    /// Component is a 64-bit component (`0x0100`).
    pub const BIT_64: i16 = 0x0100;
}

/// Bit flags for `Feature.Attributes`.
pub mod feature_attributes {
    /// Favor installing locally (`0x0001`).
    pub const FAVOR_LOCAL: i16 = 0x0001;
    /// Favor installing from source (`0x0002`).
    pub const FAVOR_SOURCE: i16 = 0x0002;
    /// Follow parent feature installation state (`0x0004`).
    pub const FOLLOW_PARENT: i16 = 0x0004;
    /// Feature favors advertise state (`0x0008`).
    pub const FAVOR_ADVERTISE: i16 = 0x0008;
    /// Disallow absent state (`0x0010`).
    pub const DISALLOW_ABSENT: i16 = 0x0010;
    /// Expand feature in `SelectionTree` UI (`0x0020`).
    pub const EXPAND: i16 = 0x0020;
}

/// Bit flags for `File.Attributes`.
pub mod file_attributes {
    /// Read-only file (`0x0001`).
    pub const READ_ONLY: i16 = 0x0001;
    /// Hidden file (`0x0002`).
    pub const HIDDEN: i16 = 0x0002;
    /// System file (`0x0004`).
    pub const SYSTEM: i16 = 0x0004;
    /// Vital file (installer fails if installation fails) (`0x0200`).
    pub const VITAL: i16 = 0x0200;
    /// File contains valid checksum (`0x0400`).
    pub const CHECKSUM: i16 = 0x0400;
    /// File added by patch (`0x1000`).
    pub const PATCH_ADDED: i16 = 0x1000;
    /// Non-compressed file (`0x2000`).
    pub const NON_COMPRESSED: i16 = 0x2000;
    /// Compressed file inside cabinet (`0x4000`).
    pub const COMPRESSED: i16 = 0x4000;
}

/// Row in the `Component` table.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComponentRow {
    /// Primary key identifying the component.
    pub component: ComponentName,
    /// Component GUID (nullable).
    pub component_id: Option<ComponentGuid>,
    /// Foreign key into Directory table.
    pub directory: DirectoryId,
    /// Component attribute flags.
    pub attributes: i16,
    /// Conditional execution expression.
    pub condition: Option<String>,
    /// Key path identifier (File, Registry, or `ODBCDataSource`).
    pub key_path: Option<String>,
}

impl ComponentRow {
    /// Converts this row into a generic [`Record`].
    ///
    /// # Returns
    ///
    /// A generic [`Record`].
    #[must_use]
    pub fn to_record(&self) -> Record {
        Record::with_fields(vec![
            FieldValue::String(self.component.as_str().to_string()),
            self.component_id.as_ref().map_or(FieldValue::Null, |g| {
                FieldValue::String(g.as_str().to_string())
            }),
            FieldValue::String(self.directory.as_str().to_string()),
            FieldValue::Short(self.attributes),
            self.condition
                .as_ref()
                .map_or(FieldValue::Null, |c| FieldValue::String(c.clone())),
            self.key_path
                .as_ref()
                .map_or(FieldValue::Null, |k| FieldValue::String(k.clone())),
        ])
    }

    /// Parses a [`ComponentRow`] from a generic [`Record`].
    ///
    /// # Arguments
    ///
    /// * `rec` - The generic record.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Validation`] or [`Error::RecordLengthMismatch`] on invalid record.
    pub fn from_record(rec: &Record) -> Result<Self> {
        if rec.len() < 6 {
            return Err(Error::RecordLengthMismatch {
                expected: 6,
                actual: rec.len(),
            });
        }

        let component = match rec.get(0) {
            Some(FieldValue::String(s)) => ComponentName::new(s.as_str())?,
            _ => {
                return Err(Error::Validation {
                    element: "Component.Component".to_string(),
                    reason: "missing or invalid Component primary key".to_string(),
                });
            }
        };

        let component_id = match rec.get(1) {
            Some(FieldValue::String(s)) if !s.is_empty() => Some(ComponentGuid::parse(s.as_str())?),
            _ => None,
        };

        let directory = match rec.get(2) {
            Some(FieldValue::String(s)) => DirectoryId::new(s.as_str())?,
            _ => {
                return Err(Error::Validation {
                    element: "Component.Directory_".to_string(),
                    reason: "missing or invalid Directory_".to_string(),
                });
            }
        };

        let attributes = match rec.get(3) {
            Some(FieldValue::Short(a)) => *a,
            _ => 0,
        };

        let condition = match rec.get(4) {
            Some(FieldValue::String(s)) if !s.is_empty() => Some(s.clone()),
            _ => None,
        };

        let key_path = match rec.get(5) {
            Some(FieldValue::String(s)) if !s.is_empty() => Some(s.clone()),
            _ => None,
        };

        Ok(Self {
            component,
            component_id,
            directory,
            attributes,
            condition,
            key_path,
        })
    }
}

/// Creates the official MSI SDK schema for the `Component` table.
///
/// # Returns
///
/// [`TableSchema`] for `Component`.
#[must_use]
pub fn component_schema() -> TableSchema {
    TableSchema::new("Component")
        .with_column(ColumnDef::new("Component", DataType::String { max_len: 72 }).primary_key())
        .with_column(ColumnDef::new("ComponentId", DataType::String { max_len: 38 }).nullable())
        .with_column(ColumnDef::new(
            "Directory_",
            DataType::String { max_len: 72 },
        ))
        .with_column(ColumnDef::new("Attributes", DataType::Short))
        .with_column(ColumnDef::new("Condition", DataType::String { max_len: 255 }).nullable())
        .with_column(ColumnDef::new("KeyPath", DataType::String { max_len: 72 }).nullable())
}

/// Row in the `Feature` table.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FeatureRow {
    /// Primary key identifying the feature.
    pub feature: FeatureName,
    /// Parent feature identifier (nullable).
    pub feature_parent: Option<FeatureName>,
    /// Display title for the feature (nullable, localizable).
    pub title: Option<String>,
    /// Long description of the feature (nullable, localizable).
    pub description: Option<String>,
    /// Display order / state numeric value.
    pub display: Option<i16>,
    /// Installation level threshold.
    pub level: i16,
    /// Directory associated with this feature (nullable).
    pub directory: Option<DirectoryId>,
    /// Attribute flags.
    pub attributes: i16,
}

impl FeatureRow {
    /// Converts this row into a generic [`Record`].
    ///
    /// # Returns
    ///
    /// A generic [`Record`].
    #[must_use]
    pub fn to_record(&self) -> Record {
        Record::with_fields(vec![
            FieldValue::String(self.feature.as_str().to_string()),
            self.feature_parent.as_ref().map_or(FieldValue::Null, |p| {
                FieldValue::String(p.as_str().to_string())
            }),
            self.title
                .as_ref()
                .map_or(FieldValue::Null, |t| FieldValue::String(t.clone())),
            self.description
                .as_ref()
                .map_or(FieldValue::Null, |d| FieldValue::String(d.clone())),
            self.display.map_or(FieldValue::Null, FieldValue::Short),
            FieldValue::Short(self.level),
            self.directory.as_ref().map_or(FieldValue::Null, |d| {
                FieldValue::String(d.as_str().to_string())
            }),
            FieldValue::Short(self.attributes),
        ])
    }

    /// Parses a [`FeatureRow`] from a generic [`Record`].
    ///
    /// # Arguments
    ///
    /// * `rec` - Generic record.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Validation`] or [`Error::RecordLengthMismatch`] on invalid record.
    pub fn from_record(rec: &Record) -> Result<Self> {
        if rec.len() < 8 {
            return Err(Error::RecordLengthMismatch {
                expected: 8,
                actual: rec.len(),
            });
        }

        let feature = match rec.get(0) {
            Some(FieldValue::String(s)) => FeatureName::new(s.as_str())?,
            _ => {
                return Err(Error::Validation {
                    element: "Feature.Feature".to_string(),
                    reason: "missing Feature primary key".to_string(),
                });
            }
        };

        let feature_parent = match rec.get(1) {
            Some(FieldValue::String(s)) if !s.is_empty() => Some(FeatureName::new(s.as_str())?),
            _ => None,
        };

        let title = match rec.get(2) {
            Some(FieldValue::String(s)) if !s.is_empty() => Some(s.clone()),
            _ => None,
        };

        let description = match rec.get(3) {
            Some(FieldValue::String(s)) if !s.is_empty() => Some(s.clone()),
            _ => None,
        };

        let display = match rec.get(4) {
            Some(FieldValue::Short(d)) => Some(*d),
            _ => None,
        };

        let level = match rec.get(5) {
            Some(FieldValue::Short(l)) => *l,
            _ => 1,
        };

        let directory = match rec.get(6) {
            Some(FieldValue::String(s)) if !s.is_empty() => Some(DirectoryId::new(s.as_str())?),
            _ => None,
        };

        let attributes = match rec.get(7) {
            Some(FieldValue::Short(a)) => *a,
            _ => 0,
        };

        Ok(Self {
            feature,
            feature_parent,
            title,
            description,
            display,
            level,
            directory,
            attributes,
        })
    }
}

/// Creates the official MSI SDK schema for the `Feature` table.
///
/// # Returns
///
/// [`TableSchema`] for `Feature`.
#[must_use]
pub fn feature_schema() -> TableSchema {
    TableSchema::new("Feature")
        .with_column(ColumnDef::new("Feature", DataType::String { max_len: 38 }).primary_key())
        .with_column(ColumnDef::new("Feature_Parent", DataType::String { max_len: 38 }).nullable())
        .with_column(
            ColumnDef::new("Title", DataType::String { max_len: 64 })
                .nullable()
                .localizable(),
        )
        .with_column(
            ColumnDef::new("Description", DataType::String { max_len: 255 })
                .nullable()
                .localizable(),
        )
        .with_column(ColumnDef::new("Display", DataType::Short).nullable())
        .with_column(ColumnDef::new("Level", DataType::Short))
        .with_column(ColumnDef::new("Directory_", DataType::String { max_len: 72 }).nullable())
        .with_column(ColumnDef::new("Attributes", DataType::Short))
}

/// Row in the `FeatureComponents` table.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FeatureComponentsRow {
    /// Foreign key to Feature table.
    pub feature: FeatureName,
    /// Foreign key to Component table.
    pub component: ComponentName,
}

impl FeatureComponentsRow {
    /// Converts this row into a generic [`Record`].
    ///
    /// # Returns
    ///
    /// A generic [`Record`].
    #[must_use]
    pub fn to_record(&self) -> Record {
        Record::with_fields(vec![
            FieldValue::String(self.feature.as_str().to_string()),
            FieldValue::String(self.component.as_str().to_string()),
        ])
    }

    /// Parses a [`FeatureComponentsRow`] from a generic [`Record`].
    ///
    /// # Arguments
    ///
    /// * `rec` - Generic record.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Validation`] or [`Error::RecordLengthMismatch`] on invalid record.
    pub fn from_record(rec: &Record) -> Result<Self> {
        if rec.len() < 2 {
            return Err(Error::RecordLengthMismatch {
                expected: 2,
                actual: rec.len(),
            });
        }

        let feature = match rec.get(0) {
            Some(FieldValue::String(s)) => FeatureName::new(s.as_str())?,
            _ => {
                return Err(Error::Validation {
                    element: "FeatureComponents.Feature_".to_string(),
                    reason: "missing Feature_".to_string(),
                });
            }
        };

        let component = match rec.get(1) {
            Some(FieldValue::String(s)) => ComponentName::new(s.as_str())?,
            _ => {
                return Err(Error::Validation {
                    element: "FeatureComponents.Component_".to_string(),
                    reason: "missing Component_".to_string(),
                });
            }
        };

        Ok(Self { feature, component })
    }
}

/// Creates the official MSI SDK schema for the `FeatureComponents` table.
///
/// # Returns
///
/// [`TableSchema`] for `FeatureComponents`.
#[must_use]
pub fn feature_components_schema() -> TableSchema {
    TableSchema::new("FeatureComponents")
        .with_column(ColumnDef::new("Feature_", DataType::String { max_len: 38 }).primary_key())
        .with_column(ColumnDef::new("Component_", DataType::String { max_len: 72 }).primary_key())
}

/// Row in the `Directory` table.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DirectoryRow {
    /// Primary key identifier of directory.
    pub directory: DirectoryId,
    /// Parent directory identifier (nullable).
    pub directory_parent: Option<DirectoryId>,
    /// Default directory specification (localizable, e.g. `.:PROGRAMF|Program Files`).
    pub default_dir: String,
}

impl DirectoryRow {
    /// Converts this row into a generic [`Record`].
    ///
    /// # Returns
    ///
    /// A generic [`Record`].
    #[must_use]
    pub fn to_record(&self) -> Record {
        Record::with_fields(vec![
            FieldValue::String(self.directory.as_str().to_string()),
            self.directory_parent
                .as_ref()
                .map_or(FieldValue::Null, |p| {
                    FieldValue::String(p.as_str().to_string())
                }),
            FieldValue::String(self.default_dir.clone()),
        ])
    }

    /// Parses a [`DirectoryRow`] from a generic [`Record`].
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

        let directory = match rec.get(0) {
            Some(FieldValue::String(s)) => DirectoryId::new(s.as_str())?,
            _ => {
                return Err(Error::Validation {
                    element: "Directory.Directory".to_string(),
                    reason: "missing Directory primary key".to_string(),
                });
            }
        };

        let directory_parent = match rec.get(1) {
            Some(FieldValue::String(s)) if !s.is_empty() => Some(DirectoryId::new(s.as_str())?),
            _ => None,
        };

        let default_dir = match rec.get(2) {
            Some(FieldValue::String(s)) => s.clone(),
            _ => ".".to_string(),
        };

        Ok(Self {
            directory,
            directory_parent,
            default_dir,
        })
    }
}

/// Creates the official MSI SDK schema for the `Directory` table.
///
/// # Returns
///
/// [`TableSchema`] for `Directory`.
#[must_use]
pub fn directory_schema() -> TableSchema {
    TableSchema::new("Directory")
        .with_column(ColumnDef::new("Directory", DataType::String { max_len: 72 }).primary_key())
        .with_column(
            ColumnDef::new("Directory_Parent", DataType::String { max_len: 72 }).nullable(),
        )
        .with_column(ColumnDef::new("DefaultDir", DataType::String { max_len: 255 }).localizable())
}

/// Row in the `File` table.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileRow {
    /// Primary key identifying this file.
    pub file: FileKey,
    /// Foreign key to Component table.
    pub component: ComponentName,
    /// Short/long filename (e.g. `FILE.TXT|File.txt`).
    pub file_name: String,
    /// Uncompressed file size in bytes.
    pub file_size: i32,
    /// File version string (nullable).
    pub version: Option<String>,
    /// Language ID or comma-separated list of IDs (nullable).
    pub language: Option<String>,
    /// File attribute flags (nullable).
    pub attributes: Option<i16>,
    /// Sequence number of the file in the installation media.
    pub sequence: i16,
}

impl FileRow {
    /// Converts this row into a generic [`Record`].
    ///
    /// # Returns
    ///
    /// A generic [`Record`].
    #[must_use]
    pub fn to_record(&self) -> Record {
        Record::with_fields(vec![
            FieldValue::String(self.file.as_str().to_string()),
            FieldValue::String(self.component.as_str().to_string()),
            FieldValue::String(self.file_name.clone()),
            FieldValue::Long(self.file_size),
            self.version
                .as_ref()
                .map_or(FieldValue::Null, |v| FieldValue::String(v.clone())),
            self.language
                .as_ref()
                .map_or(FieldValue::Null, |l| FieldValue::String(l.clone())),
            self.attributes.map_or(FieldValue::Null, FieldValue::Short),
            FieldValue::Short(self.sequence),
        ])
    }

    /// Parses a [`FileRow`] from a generic [`Record`].
    ///
    /// # Arguments
    ///
    /// * `rec` - Generic record.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Validation`] or [`Error::RecordLengthMismatch`] on invalid record.
    pub fn from_record(rec: &Record) -> Result<Self> {
        if rec.len() < 8 {
            return Err(Error::RecordLengthMismatch {
                expected: 8,
                actual: rec.len(),
            });
        }

        let file = match rec.get(0) {
            Some(FieldValue::String(s)) => FileKey::new(s.as_str())?,
            _ => {
                return Err(Error::Validation {
                    element: "File.File".to_string(),
                    reason: "missing File primary key".to_string(),
                });
            }
        };

        let component = match rec.get(1) {
            Some(FieldValue::String(s)) => ComponentName::new(s.as_str())?,
            _ => {
                return Err(Error::Validation {
                    element: "File.Component_".to_string(),
                    reason: "missing Component_".to_string(),
                });
            }
        };

        let file_name = match rec.get(2) {
            Some(FieldValue::String(s)) => s.clone(),
            _ => {
                return Err(Error::Validation {
                    element: "File.FileName".to_string(),
                    reason: "missing FileName".to_string(),
                });
            }
        };

        let file_size = match rec.get(3) {
            Some(FieldValue::Long(sz)) => *sz,
            _ => 0,
        };

        let version = match rec.get(4) {
            Some(FieldValue::String(s)) if !s.is_empty() => Some(s.clone()),
            _ => None,
        };

        let language = match rec.get(5) {
            Some(FieldValue::String(s)) if !s.is_empty() => Some(s.clone()),
            _ => None,
        };

        let attributes = match rec.get(6) {
            Some(FieldValue::Short(a)) => Some(*a),
            _ => None,
        };

        let sequence = match rec.get(7) {
            Some(FieldValue::Short(s)) => *s,
            _ => 1,
        };

        Ok(Self {
            file,
            component,
            file_name,
            file_size,
            version,
            language,
            attributes,
            sequence,
        })
    }
}

/// Creates the official MSI SDK schema for the `File` table.
///
/// # Returns
///
/// [`TableSchema`] for `File`.
#[must_use]
pub fn file_schema() -> TableSchema {
    TableSchema::new("File")
        .with_column(ColumnDef::new("File", DataType::String { max_len: 72 }).primary_key())
        .with_column(ColumnDef::new(
            "Component_",
            DataType::String { max_len: 72 },
        ))
        .with_column(ColumnDef::new("FileName", DataType::String { max_len: 255 }).localizable())
        .with_column(ColumnDef::new("FileSize", DataType::Long))
        .with_column(ColumnDef::new("Version", DataType::String { max_len: 72 }).nullable())
        .with_column(ColumnDef::new("Language", DataType::String { max_len: 20 }).nullable())
        .with_column(ColumnDef::new("Attributes", DataType::Short).nullable())
        .with_column(ColumnDef::new("Sequence", DataType::Short))
}

/// Row in the `FileHash` table.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileHashRow {
    /// Foreign key to File table.
    pub file: FileKey,
    /// Hash options bitmask (must be 0 in standard MSI).
    pub options: i16,
    /// MD5 hash part 1 (lower 32 bits).
    pub hash_part1: i32,
    /// MD5 hash part 2.
    pub hash_part2: i32,
    /// MD5 hash part 3.
    pub hash_part3: i32,
    /// MD5 hash part 4 (upper 32 bits).
    pub hash_part4: i32,
}

impl FileHashRow {
    /// Converts this row into a generic [`Record`].
    ///
    /// # Returns
    ///
    /// A generic [`Record`].
    #[must_use]
    pub fn to_record(&self) -> Record {
        Record::with_fields(vec![
            FieldValue::String(self.file.as_str().to_string()),
            FieldValue::Short(self.options),
            FieldValue::Long(self.hash_part1),
            FieldValue::Long(self.hash_part2),
            FieldValue::Long(self.hash_part3),
            FieldValue::Long(self.hash_part4),
        ])
    }

    /// Parses a [`FileHashRow`] from a generic [`Record`].
    ///
    /// # Arguments
    ///
    /// * `rec` - Generic record.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Validation`] or [`Error::RecordLengthMismatch`] on invalid record.
    pub fn from_record(rec: &Record) -> Result<Self> {
        if rec.len() < 6 {
            return Err(Error::RecordLengthMismatch {
                expected: 6,
                actual: rec.len(),
            });
        }

        let file = match rec.get(0) {
            Some(FieldValue::String(s)) => FileKey::new(s.as_str())?,
            _ => {
                return Err(Error::Validation {
                    element: "FileHash.File_".to_string(),
                    reason: "missing File_ foreign key".to_string(),
                });
            }
        };

        let options = match rec.get(1) {
            Some(FieldValue::Short(o)) => *o,
            _ => 0,
        };

        let hash_part1 = match rec.get(2) {
            Some(FieldValue::Long(h)) => *h,
            _ => 0,
        };

        let hash_part2 = match rec.get(3) {
            Some(FieldValue::Long(h)) => *h,
            _ => 0,
        };

        let hash_part3 = match rec.get(4) {
            Some(FieldValue::Long(h)) => *h,
            _ => 0,
        };

        let hash_part4 = match rec.get(5) {
            Some(FieldValue::Long(h)) => *h,
            _ => 0,
        };

        Ok(Self {
            file,
            options,
            hash_part1,
            hash_part2,
            hash_part3,
            hash_part4,
        })
    }
}

/// Creates the official MSI SDK schema for the `FileHash` table.
///
/// # Returns
///
/// [`TableSchema`] for `FileHash`.
#[must_use]
pub fn file_hash_schema() -> TableSchema {
    TableSchema::new("FileHash")
        .with_column(ColumnDef::new("File_", DataType::String { max_len: 72 }).primary_key())
        .with_column(ColumnDef::new("Options", DataType::Short))
        .with_column(ColumnDef::new("HashPart1", DataType::Long))
        .with_column(ColumnDef::new("HashPart2", DataType::Long))
        .with_column(ColumnDef::new("HashPart3", DataType::Long))
        .with_column(ColumnDef::new("HashPart4", DataType::Long))
}

/// Row in the `Media` table.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MediaRow {
    /// Disk identifier (primary key).
    pub disk_id: i16,
    /// Last sequence number of file on this disk.
    pub last_sequence: i32,
    /// Disk prompt displayed to user during multi-disk prompt (nullable).
    pub disk_prompt: Option<String>,
    /// Cabinet name containing files (e.g. `#cab1.cab` or `Data1.cab`, nullable).
    pub cabinet: Option<String>,
    /// Volume label of the disk (nullable).
    pub volume_label: Option<String>,
    /// Source property path (nullable).
    pub source: Option<String>,
}

impl MediaRow {
    /// Converts this row into a generic [`Record`].
    ///
    /// # Returns
    ///
    /// A generic [`Record`].
    #[must_use]
    pub fn to_record(&self) -> Record {
        Record::with_fields(vec![
            FieldValue::Short(self.disk_id),
            FieldValue::Long(self.last_sequence),
            self.disk_prompt
                .as_ref()
                .map_or(FieldValue::Null, |p| FieldValue::String(p.clone())),
            self.cabinet
                .as_ref()
                .map_or(FieldValue::Null, |c| FieldValue::String(c.clone())),
            self.volume_label
                .as_ref()
                .map_or(FieldValue::Null, |v| FieldValue::String(v.clone())),
            self.source
                .as_ref()
                .map_or(FieldValue::Null, |s| FieldValue::String(s.clone())),
        ])
    }

    /// Parses a [`MediaRow`] from a generic [`Record`].
    ///
    /// # Arguments
    ///
    /// * `rec` - Generic record.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Validation`] or [`Error::RecordLengthMismatch`] on invalid record.
    pub fn from_record(rec: &Record) -> Result<Self> {
        if rec.len() < 6 {
            return Err(Error::RecordLengthMismatch {
                expected: 6,
                actual: rec.len(),
            });
        }

        let disk_id = match rec.get(0) {
            Some(FieldValue::Short(id)) => *id,
            _ => {
                return Err(Error::Validation {
                    element: "Media.DiskId".to_string(),
                    reason: "missing DiskId primary key".to_string(),
                });
            }
        };

        let last_sequence = match rec.get(1) {
            Some(FieldValue::Long(seq)) => *seq,
            _ => 0,
        };

        let disk_prompt = match rec.get(2) {
            Some(FieldValue::String(s)) if !s.is_empty() => Some(s.clone()),
            _ => None,
        };

        let cabinet = match rec.get(3) {
            Some(FieldValue::String(s)) if !s.is_empty() => Some(s.clone()),
            _ => None,
        };

        let volume_label = match rec.get(4) {
            Some(FieldValue::String(s)) if !s.is_empty() => Some(s.clone()),
            _ => None,
        };

        let source = match rec.get(5) {
            Some(FieldValue::String(s)) if !s.is_empty() => Some(s.clone()),
            _ => None,
        };

        Ok(Self {
            disk_id,
            last_sequence,
            disk_prompt,
            cabinet,
            volume_label,
            source,
        })
    }
}

/// Creates the official MSI SDK schema for the `Media` table.
///
/// # Returns
///
/// [`TableSchema`] for `Media`.
#[must_use]
pub fn media_schema() -> TableSchema {
    TableSchema::new("Media")
        .with_column(ColumnDef::new("DiskId", DataType::Short).primary_key())
        .with_column(ColumnDef::new("LastSequence", DataType::Long))
        .with_column(
            ColumnDef::new("DiskPrompt", DataType::String { max_len: 64 })
                .nullable()
                .localizable(),
        )
        .with_column(ColumnDef::new("Cabinet", DataType::String { max_len: 255 }).nullable())
        .with_column(ColumnDef::new("VolumeLabel", DataType::String { max_len: 32 }).nullable())
        .with_column(ColumnDef::new("Source", DataType::String { max_len: 72 }).nullable())
}

/// Row in the `Property` table.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PropertyRow {
    /// Property identifier (primary key).
    pub property: PropertyName,
    /// Property string value.
    pub value: String,
}

impl PropertyRow {
    /// Converts this row into a generic [`Record`].
    ///
    /// # Returns
    ///
    /// A generic [`Record`].
    #[must_use]
    pub fn to_record(&self) -> Record {
        Record::with_fields(vec![
            FieldValue::String(self.property.as_str().to_string()),
            FieldValue::String(self.value.clone()),
        ])
    }

    /// Parses a [`PropertyRow`] from a generic [`Record`].
    ///
    /// # Arguments
    ///
    /// * `rec` - Generic record.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Validation`] or [`Error::RecordLengthMismatch`] on invalid record.
    pub fn from_record(rec: &Record) -> Result<Self> {
        if rec.len() < 2 {
            return Err(Error::RecordLengthMismatch {
                expected: 2,
                actual: rec.len(),
            });
        }

        let property = match rec.get(0) {
            Some(FieldValue::String(s)) => PropertyName::new(s.as_str())?,
            _ => {
                return Err(Error::Validation {
                    element: "Property.Property".to_string(),
                    reason: "missing Property primary key".to_string(),
                });
            }
        };

        let value = match rec.get(1) {
            Some(FieldValue::String(s)) => s.clone(),
            _ => String::new(),
        };

        Ok(Self { property, value })
    }
}

/// Creates the official MSI SDK schema for the `Property` table.
///
/// # Returns
///
/// [`TableSchema`] for `Property`.
#[must_use]
pub fn property_schema() -> TableSchema {
    TableSchema::new("Property")
        .with_column(ColumnDef::new("Property", DataType::String { max_len: 72 }).primary_key())
        .with_column(ColumnDef::new("Value", DataType::String { max_len: 0 }).localizable())
}

/// Row in the `Binary` table.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BinaryRow {
    /// Binary stream identifier (primary key).
    pub name: String,
    /// Stream identifier pointing to raw binary data in the string pool / stream catalog.
    pub data: crate::database::tables::types::StringPoolId,
}

impl BinaryRow {
    /// Converts this row into a generic [`Record`].
    ///
    /// # Returns
    ///
    /// A generic [`Record`].
    #[must_use]
    pub fn to_record(&self) -> Record {
        Record::with_fields(vec![
            FieldValue::String(self.name.clone()),
            FieldValue::Stream(self.data),
        ])
    }

    /// Parses a [`BinaryRow`] from a generic [`Record`].
    ///
    /// # Arguments
    ///
    /// * `rec` - Generic record.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Validation`] or [`Error::RecordLengthMismatch`] on invalid record.
    pub fn from_record(rec: &Record) -> Result<Self> {
        if rec.len() < 2 {
            return Err(Error::RecordLengthMismatch {
                expected: 2,
                actual: rec.len(),
            });
        }

        let name = match rec.get(0) {
            Some(FieldValue::String(s)) => s.clone(),
            _ => {
                return Err(Error::Validation {
                    element: "Binary.Name".to_string(),
                    reason: "missing Name primary key".to_string(),
                });
            }
        };

        let data = match rec.get(1) {
            Some(FieldValue::Stream(id)) => *id,
            _ => crate::database::tables::types::StringPoolId::new(0),
        };

        Ok(Self { name, data })
    }
}

/// Creates the official MSI SDK schema for the `Binary` table.
///
/// # Returns
///
/// [`TableSchema`] for `Binary`.
#[must_use]
pub fn binary_schema() -> TableSchema {
    TableSchema::new("Binary")
        .with_column(ColumnDef::new("Name", DataType::String { max_len: 72 }).primary_key())
        .with_column(ColumnDef::new("Data", DataType::Stream))
}

/// Row in the `Font` table.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FontRow {
    /// Foreign key to File table (primary key).
    pub file: FileKey,
    /// Registered font title description.
    pub font_title: Option<String>,
}

impl FontRow {
    /// Converts this row into a generic [`Record`].
    ///
    /// # Returns
    ///
    /// A generic [`Record`].
    #[must_use]
    pub fn to_record(&self) -> Record {
        Record::with_fields(vec![
            FieldValue::String(self.file.as_str().to_string()),
            self.font_title
                .as_ref()
                .map_or(FieldValue::Null, |t| FieldValue::String(t.clone())),
        ])
    }

    /// Parses a [`FontRow`] from a generic [`Record`].
    ///
    /// # Arguments
    ///
    /// * `rec` - Generic record.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Validation`] or [`Error::RecordLengthMismatch`] on invalid record.
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
                    element: "Font.File_".to_string(),
                    reason: "missing File_ primary key".to_string(),
                });
            }
        };

        let font_title = match rec.get(1) {
            Some(FieldValue::String(s)) if !s.is_empty() => Some(s.clone()),
            _ => None,
        };

        Ok(Self { file, font_title })
    }
}

/// Creates the official MSI SDK schema for the `Font` table.
///
/// # Returns
///
/// [`TableSchema`] for `Font`.
#[must_use]
pub fn font_schema() -> TableSchema {
    TableSchema::new("Font")
        .with_column(ColumnDef::new("File_", DataType::String { max_len: 72 }).primary_key())
        .with_column(ColumnDef::new("FontTitle", DataType::String { max_len: 128 }).nullable())
}

/// Row in the `PatchPackage` table.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PatchPackageRow {
    /// Unique identifier for the patch (primary key).
    pub patch_id: String,
    /// `DiskId` in Media table identifying the disk containing the patch (primary key).
    pub media: i16,
}

impl PatchPackageRow {
    /// Converts this row into a generic [`Record`].
    ///
    /// # Returns
    ///
    /// A generic [`Record`].
    #[must_use]
    pub fn to_record(&self) -> Record {
        Record::with_fields(vec![
            FieldValue::String(self.patch_id.clone()),
            FieldValue::Short(self.media),
        ])
    }

    /// Parses a [`PatchPackageRow`] from a generic [`Record`].
    ///
    /// # Arguments
    ///
    /// * `rec` - Generic record.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Validation`] or [`Error::RecordLengthMismatch`] on invalid record.
    pub fn from_record(rec: &Record) -> Result<Self> {
        if rec.len() < 2 {
            return Err(Error::RecordLengthMismatch {
                expected: 2,
                actual: rec.len(),
            });
        }

        let patch_id = match rec.get(0) {
            Some(FieldValue::String(s)) => s.clone(),
            _ => {
                return Err(Error::Validation {
                    element: "PatchPackage.PatchId".to_string(),
                    reason: "missing PatchId primary key".to_string(),
                });
            }
        };

        let media = match rec.get(1) {
            Some(FieldValue::Short(m)) => *m,
            _ => 1,
        };

        Ok(Self { patch_id, media })
    }
}

/// Creates the official MSI SDK schema for the `PatchPackage` table.
///
/// # Returns
///
/// [`TableSchema`] for `PatchPackage`.
#[must_use]
pub fn patch_package_schema() -> TableSchema {
    TableSchema::new("PatchPackage")
        .with_column(ColumnDef::new("PatchId", DataType::String { max_len: 72 }).primary_key())
        .with_column(ColumnDef::new("Media_", DataType::Short).primary_key())
}

/// Row in the `ModuleConfiguration` table.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModuleConfigurationRow {
    /// Name of the configurable parameter (primary key).
    pub name: String,
    /// Format type enumeration (integer code).
    pub format: i32,
    /// Type descriptor for the configurable item.
    pub type_: Option<String>,
    /// Context data for custom configuration.
    pub context_data: Option<String>,
    /// Default value for parameter.
    pub default_value: Option<String>,
    /// Attribute bitmask flags.
    pub attributes: Option<i32>,
    /// User-visible display name.
    pub display_name: Option<String>,
    /// Description of the configuration parameter.
    pub description: Option<String>,
    /// Help keyword identifier.
    pub help_keyword: Option<String>,
}

impl ModuleConfigurationRow {
    /// Converts this row into a generic [`Record`].
    ///
    /// # Returns
    ///
    /// A generic [`Record`].
    #[must_use]
    pub fn to_record(&self) -> Record {
        Record::with_fields(vec![
            FieldValue::String(self.name.clone()),
            FieldValue::Long(self.format),
            self.type_
                .as_ref()
                .map_or(FieldValue::Null, |s| FieldValue::String(s.clone())),
            self.context_data
                .as_ref()
                .map_or(FieldValue::Null, |s| FieldValue::String(s.clone())),
            self.default_value
                .as_ref()
                .map_or(FieldValue::Null, |s| FieldValue::String(s.clone())),
            self.attributes.map_or(FieldValue::Null, FieldValue::Long),
            self.display_name
                .as_ref()
                .map_or(FieldValue::Null, |s| FieldValue::String(s.clone())),
            self.description
                .as_ref()
                .map_or(FieldValue::Null, |s| FieldValue::String(s.clone())),
            self.help_keyword
                .as_ref()
                .map_or(FieldValue::Null, |s| FieldValue::String(s.clone())),
        ])
    }

    /// Parses a [`ModuleConfigurationRow`] from a generic [`Record`].
    ///
    /// # Arguments
    ///
    /// * `rec` - Generic record.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Validation`] or [`Error::RecordLengthMismatch`] on invalid record.
    pub fn from_record(rec: &Record) -> Result<Self> {
        if rec.len() < 2 {
            return Err(Error::RecordLengthMismatch {
                expected: 2,
                actual: rec.len(),
            });
        }

        let name = match rec.get(0) {
            Some(FieldValue::String(s)) => s.clone(),
            _ => {
                return Err(Error::Validation {
                    element: "ModuleConfiguration.Name".to_string(),
                    reason: "missing Name primary key".to_string(),
                });
            }
        };

        let format = match rec.get(1) {
            Some(FieldValue::Long(f)) => *f,
            _ => 0,
        };

        let type_ = match rec.get(2) {
            Some(FieldValue::String(s)) if !s.is_empty() => Some(s.clone()),
            _ => None,
        };

        let context_data = match rec.get(3) {
            Some(FieldValue::String(s)) if !s.is_empty() => Some(s.clone()),
            _ => None,
        };

        let default_value = match rec.get(4) {
            Some(FieldValue::String(s)) if !s.is_empty() => Some(s.clone()),
            _ => None,
        };

        let attributes = match rec.get(5) {
            Some(FieldValue::Long(a)) => Some(*a),
            _ => None,
        };

        let display_name = match rec.get(6) {
            Some(FieldValue::String(s)) if !s.is_empty() => Some(s.clone()),
            _ => None,
        };

        let description = match rec.get(7) {
            Some(FieldValue::String(s)) if !s.is_empty() => Some(s.clone()),
            _ => None,
        };

        let help_keyword = match rec.get(8) {
            Some(FieldValue::String(s)) if !s.is_empty() => Some(s.clone()),
            _ => None,
        };

        Ok(Self {
            name,
            format,
            type_,
            context_data,
            default_value,
            attributes,
            display_name,
            description,
            help_keyword,
        })
    }
}

/// Creates official schema for `ModuleConfiguration` table.
///
/// # Returns
///
/// [`TableSchema`] for `ModuleConfiguration`.
#[must_use]
pub fn module_configuration_schema() -> TableSchema {
    TableSchema::new("ModuleConfiguration")
        .with_column(ColumnDef::new("Name", DataType::String { max_len: 72 }).primary_key())
        .with_column(ColumnDef::new("Format", DataType::Long))
        .with_column(ColumnDef::new("Type", DataType::String { max_len: 64 }).nullable())
        .with_column(ColumnDef::new("ContextData", DataType::String { max_len: 255 }).nullable())
        .with_column(ColumnDef::new("DefaultValue", DataType::String { max_len: 255 }).nullable())
        .with_column(ColumnDef::new("Attributes", DataType::Long).nullable())
        .with_column(
            ColumnDef::new("DisplayName", DataType::String { max_len: 128 })
                .nullable()
                .localizable(),
        )
        .with_column(
            ColumnDef::new("Description", DataType::String { max_len: 255 })
                .nullable()
                .localizable(),
        )
        .with_column(ColumnDef::new("HelpKeyword", DataType::String { max_len: 32 }).nullable())
}

/// Row in the `ModuleSubstitution` table.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModuleSubstitutionRow {
    /// Target database table name (primary key).
    pub table: String,
    /// Primary key value of target row (primary key).
    pub row: String,
    /// Target column name (primary key).
    pub column: String,
    /// Substitution template string value.
    pub value: Option<String>,
}

impl ModuleSubstitutionRow {
    /// Converts this row into a generic [`Record`].
    ///
    /// # Returns
    ///
    /// A generic [`Record`].
    #[must_use]
    pub fn to_record(&self) -> Record {
        Record::with_fields(vec![
            FieldValue::String(self.table.clone()),
            FieldValue::String(self.row.clone()),
            FieldValue::String(self.column.clone()),
            self.value
                .as_ref()
                .map_or(FieldValue::Null, |s| FieldValue::String(s.clone())),
        ])
    }

    /// Parses a [`ModuleSubstitutionRow`] from a generic [`Record`].
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

        let table = match rec.get(0) {
            Some(FieldValue::String(s)) => s.clone(),
            _ => {
                return Err(Error::Validation {
                    element: "ModuleSubstitution.Table".to_string(),
                    reason: "missing Table primary key".to_string(),
                });
            }
        };

        let row = match rec.get(1) {
            Some(FieldValue::String(s)) => s.clone(),
            _ => {
                return Err(Error::Validation {
                    element: "ModuleSubstitution.Row".to_string(),
                    reason: "missing Row primary key".to_string(),
                });
            }
        };

        let column = match rec.get(2) {
            Some(FieldValue::String(s)) => s.clone(),
            _ => {
                return Err(Error::Validation {
                    element: "ModuleSubstitution.Column".to_string(),
                    reason: "missing Column primary key".to_string(),
                });
            }
        };

        let value = match rec.get(3) {
            Some(FieldValue::String(s)) if !s.is_empty() => Some(s.clone()),
            _ => None,
        };

        Ok(Self {
            table,
            row,
            column,
            value,
        })
    }
}

/// Creates official schema for `ModuleSubstitution` table.
///
/// # Returns
///
/// [`TableSchema`] for `ModuleSubstitution`.
#[must_use]
pub fn module_substitution_schema() -> TableSchema {
    TableSchema::new("ModuleSubstitution")
        .with_column(ColumnDef::new("Table", DataType::String { max_len: 32 }).primary_key())
        .with_column(ColumnDef::new("Row", DataType::String { max_len: 72 }).primary_key())
        .with_column(ColumnDef::new("Column", DataType::String { max_len: 32 }).primary_key())
        .with_column(ColumnDef::new("Value", DataType::String { max_len: 255 }).nullable())
}

/// Row in the `ModuleIgnoreModularization` table.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModuleIgnoreModularizationRow {
    /// Name of item to exclude from modularization GUID appending (primary key).
    pub name: String,
    /// Numeric type flag.
    pub type_: Option<i16>,
}

impl ModuleIgnoreModularizationRow {
    /// Converts this row into a generic [`Record`].
    ///
    /// # Returns
    ///
    /// A generic [`Record`].
    #[must_use]
    pub fn to_record(&self) -> Record {
        Record::with_fields(vec![
            FieldValue::String(self.name.clone()),
            self.type_.map_or(FieldValue::Null, FieldValue::Short),
        ])
    }

    /// Parses a [`ModuleIgnoreModularizationRow`] from a generic [`Record`].
    ///
    /// # Arguments
    ///
    /// * `rec` - Generic record.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Validation`] or [`Error::RecordLengthMismatch`] on invalid record.
    pub fn from_record(rec: &Record) -> Result<Self> {
        if rec.is_empty() {
            return Err(Error::RecordLengthMismatch {
                expected: 1,
                actual: 0,
            });
        }

        let name = match rec.get(0) {
            Some(FieldValue::String(s)) => s.clone(),
            _ => {
                return Err(Error::Validation {
                    element: "ModuleIgnoreModularization.Name".to_string(),
                    reason: "missing Name primary key".to_string(),
                });
            }
        };

        let type_ = match rec.get(1) {
            Some(FieldValue::Short(t)) => Some(*t),
            _ => None,
        };

        Ok(Self { name, type_ })
    }
}

/// Creates official schema for `ModuleIgnoreModularization` table.
///
/// # Returns
///
/// [`TableSchema`] for `ModuleIgnoreModularization`.
#[must_use]
pub fn module_ignore_modularization_schema() -> TableSchema {
    TableSchema::new("ModuleIgnoreModularization")
        .with_column(ColumnDef::new("Name", DataType::String { max_len: 72 }).primary_key())
        .with_column(ColumnDef::new("Type", DataType::Short).nullable())
}

/// Row in the `ModuleSignature` table.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModuleSignatureRow {
    /// Identifier for this merge module (primary key).
    pub module_id: String,
    /// Language identifier (primary key).
    pub language: i16,
    /// Module version string.
    pub version: String,
}

impl ModuleSignatureRow {
    /// Converts this row into a generic [`Record`].
    ///
    /// # Returns
    ///
    /// A generic [`Record`].
    #[must_use]
    pub fn to_record(&self) -> Record {
        Record::with_fields(vec![
            FieldValue::String(self.module_id.clone()),
            FieldValue::Short(self.language),
            FieldValue::String(self.version.clone()),
        ])
    }

    /// Parses a [`ModuleSignatureRow`] from a generic [`Record`].
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

        let module_id = match rec.get(0) {
            Some(FieldValue::String(s)) => s.clone(),
            _ => {
                return Err(Error::Validation {
                    element: "ModuleSignature.ModuleID".to_string(),
                    reason: "missing ModuleID primary key".to_string(),
                });
            }
        };

        let language = match rec.get(1) {
            Some(FieldValue::Short(l)) => *l,
            _ => {
                return Err(Error::Validation {
                    element: "ModuleSignature.Language".to_string(),
                    reason: "missing Language primary key".to_string(),
                });
            }
        };

        let version = match rec.get(2) {
            Some(FieldValue::String(s)) => s.clone(),
            _ => {
                return Err(Error::Validation {
                    element: "ModuleSignature.Version".to_string(),
                    reason: "missing Version field".to_string(),
                });
            }
        };

        Ok(Self {
            module_id,
            language,
            version,
        })
    }
}

/// Creates official schema for `ModuleSignature` table.
///
/// # Returns
///
/// [`TableSchema`] for `ModuleSignature`.
#[must_use]
pub fn module_signature_schema() -> TableSchema {
    TableSchema::new("ModuleSignature")
        .with_column(ColumnDef::new("ModuleID", DataType::String { max_len: 72 }).primary_key())
        .with_column(ColumnDef::new("Language", DataType::Short).primary_key())
        .with_column(ColumnDef::new("Version", DataType::String { max_len: 32 }))
}

/// Row in the `ModuleComponents` table.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModuleComponentsRow {
    /// Component identifier (primary key).
    pub component: String,
    /// Module identifier (primary key).
    pub module_id: String,
    /// Language identifier (primary key).
    pub language: i16,
}

impl ModuleComponentsRow {
    /// Converts this row into a generic [`Record`].
    ///
    /// # Returns
    ///
    /// A generic [`Record`].
    #[must_use]
    pub fn to_record(&self) -> Record {
        Record::with_fields(vec![
            FieldValue::String(self.component.clone()),
            FieldValue::String(self.module_id.clone()),
            FieldValue::Short(self.language),
        ])
    }

    /// Parses a [`ModuleComponentsRow`] from a generic [`Record`].
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

        let component = match rec.get(0) {
            Some(FieldValue::String(s)) => s.clone(),
            _ => {
                return Err(Error::Validation {
                    element: "ModuleComponents.Component".to_string(),
                    reason: "missing Component primary key".to_string(),
                });
            }
        };

        let module_id = match rec.get(1) {
            Some(FieldValue::String(s)) => s.clone(),
            _ => {
                return Err(Error::Validation {
                    element: "ModuleComponents.ModuleID".to_string(),
                    reason: "missing ModuleID primary key".to_string(),
                });
            }
        };

        let language = match rec.get(2) {
            Some(FieldValue::Short(l)) => *l,
            _ => {
                return Err(Error::Validation {
                    element: "ModuleComponents.Language".to_string(),
                    reason: "missing Language primary key".to_string(),
                });
            }
        };

        Ok(Self {
            component,
            module_id,
            language,
        })
    }
}

/// Creates official schema for `ModuleComponents` table.
///
/// # Returns
///
/// [`TableSchema`] for `ModuleComponents`.
#[must_use]
pub fn module_components_schema() -> TableSchema {
    TableSchema::new("ModuleComponents")
        .with_column(ColumnDef::new("Component", DataType::String { max_len: 72 }).primary_key())
        .with_column(ColumnDef::new("ModuleID", DataType::String { max_len: 72 }).primary_key())
        .with_column(ColumnDef::new("Language", DataType::Short).primary_key())
}

/// Row in the `ModuleDependency` table.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModuleDependencyRow {
    /// Module identifier (primary key).
    pub module_id: String,
    /// Module language (primary key).
    pub module_language: i16,
    /// Required module identifier (primary key).
    pub required_id: String,
    /// Required module language (primary key).
    pub required_language: i16,
    /// Optional required version.
    pub required_version: Option<String>,
}

impl ModuleDependencyRow {
    /// Converts this row into a generic [`Record`].
    ///
    /// # Returns
    ///
    /// A generic [`Record`].
    #[must_use]
    pub fn to_record(&self) -> Record {
        Record::with_fields(vec![
            FieldValue::String(self.module_id.clone()),
            FieldValue::Short(self.module_language),
            FieldValue::String(self.required_id.clone()),
            FieldValue::Short(self.required_language),
            self.required_version
                .as_ref()
                .map_or(FieldValue::Null, |v| FieldValue::String(v.clone())),
        ])
    }

    /// Parses a [`ModuleDependencyRow`] from a generic [`Record`].
    ///
    /// # Arguments
    ///
    /// * `rec` - Generic record.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Validation`] or [`Error::RecordLengthMismatch`] on invalid record.
    pub fn from_record(rec: &Record) -> Result<Self> {
        if rec.len() < 4 {
            return Err(Error::RecordLengthMismatch {
                expected: 4,
                actual: rec.len(),
            });
        }

        let module_id = match rec.get(0) {
            Some(FieldValue::String(s)) => s.clone(),
            _ => {
                return Err(Error::Validation {
                    element: "ModuleDependency.ModuleID".to_string(),
                    reason: "missing ModuleID primary key".to_string(),
                });
            }
        };

        let module_language = match rec.get(1) {
            Some(FieldValue::Short(l)) => *l,
            _ => {
                return Err(Error::Validation {
                    element: "ModuleDependency.ModuleLanguage".to_string(),
                    reason: "missing ModuleLanguage primary key".to_string(),
                });
            }
        };

        let required_id = match rec.get(2) {
            Some(FieldValue::String(s)) => s.clone(),
            _ => {
                return Err(Error::Validation {
                    element: "ModuleDependency.RequiredID".to_string(),
                    reason: "missing RequiredID primary key".to_string(),
                });
            }
        };

        let required_language = match rec.get(3) {
            Some(FieldValue::Short(l)) => *l,
            _ => {
                return Err(Error::Validation {
                    element: "ModuleDependency.RequiredLanguage".to_string(),
                    reason: "missing RequiredLanguage primary key".to_string(),
                });
            }
        };

        let required_version = match rec.get(4) {
            Some(FieldValue::String(s)) => Some(s.clone()),
            _ => None,
        };

        Ok(Self {
            module_id,
            module_language,
            required_id,
            required_language,
            required_version,
        })
    }
}

/// Creates official schema for `ModuleDependency` table.
///
/// # Returns
///
/// [`TableSchema`] for `ModuleDependency`.
#[must_use]
pub fn module_dependency_schema() -> TableSchema {
    TableSchema::new("ModuleDependency")
        .with_column(ColumnDef::new("ModuleID", DataType::String { max_len: 72 }).primary_key())
        .with_column(ColumnDef::new("ModuleLanguage", DataType::Short).primary_key())
        .with_column(ColumnDef::new("RequiredID", DataType::String { max_len: 72 }).primary_key())
        .with_column(ColumnDef::new("RequiredLanguage", DataType::Short).primary_key())
        .with_column(ColumnDef::new("RequiredVersion", DataType::String { max_len: 32 }).nullable())
}

/// Row in the `ModuleExclusion` table.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModuleExclusionRow {
    /// Module identifier (primary key).
    pub module_id: String,
    /// Module language (primary key).
    pub module_language: i16,
    /// Excluded module identifier (primary key).
    pub excluded_id: String,
    /// Excluded module language (primary key).
    pub excluded_language: i16,
    /// Optional minimum version excluded.
    pub excluded_version_min: Option<String>,
    /// Optional maximum version excluded.
    pub excluded_version_max: Option<String>,
}

impl ModuleExclusionRow {
    /// Converts this row into a generic [`Record`].
    ///
    /// # Returns
    ///
    /// A generic [`Record`].
    #[must_use]
    pub fn to_record(&self) -> Record {
        Record::with_fields(vec![
            FieldValue::String(self.module_id.clone()),
            FieldValue::Short(self.module_language),
            FieldValue::String(self.excluded_id.clone()),
            FieldValue::Short(self.excluded_language),
            self.excluded_version_min
                .as_ref()
                .map_or(FieldValue::Null, |v| FieldValue::String(v.clone())),
            self.excluded_version_max
                .as_ref()
                .map_or(FieldValue::Null, |v| FieldValue::String(v.clone())),
        ])
    }

    /// Parses a [`ModuleExclusionRow`] from a generic [`Record`].
    ///
    /// # Arguments
    ///
    /// * `rec` - Generic record.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Validation`] or [`Error::RecordLengthMismatch`] on invalid record.
    pub fn from_record(rec: &Record) -> Result<Self> {
        if rec.len() < 4 {
            return Err(Error::RecordLengthMismatch {
                expected: 4,
                actual: rec.len(),
            });
        }

        let module_id = match rec.get(0) {
            Some(FieldValue::String(s)) => s.clone(),
            _ => {
                return Err(Error::Validation {
                    element: "ModuleExclusion.ModuleID".to_string(),
                    reason: "missing ModuleID primary key".to_string(),
                });
            }
        };

        let module_language = match rec.get(1) {
            Some(FieldValue::Short(l)) => *l,
            _ => {
                return Err(Error::Validation {
                    element: "ModuleExclusion.ModuleLanguage".to_string(),
                    reason: "missing ModuleLanguage primary key".to_string(),
                });
            }
        };

        let excluded_id = match rec.get(2) {
            Some(FieldValue::String(s)) => s.clone(),
            _ => {
                return Err(Error::Validation {
                    element: "ModuleExclusion.ExcludedID".to_string(),
                    reason: "missing ExcludedID primary key".to_string(),
                });
            }
        };

        let excluded_language = match rec.get(3) {
            Some(FieldValue::Short(l)) => *l,
            _ => {
                return Err(Error::Validation {
                    element: "ModuleExclusion.ExcludedLanguage".to_string(),
                    reason: "missing ExcludedLanguage primary key".to_string(),
                });
            }
        };

        let excluded_version_min = match rec.get(4) {
            Some(FieldValue::String(s)) => Some(s.clone()),
            _ => None,
        };

        let excluded_version_max = match rec.get(5) {
            Some(FieldValue::String(s)) => Some(s.clone()),
            _ => None,
        };

        Ok(Self {
            module_id,
            module_language,
            excluded_id,
            excluded_language,
            excluded_version_min,
            excluded_version_max,
        })
    }
}

/// Creates official schema for `ModuleExclusion` table.
///
/// # Returns
///
/// [`TableSchema`] for `ModuleExclusion`.
#[must_use]
pub fn module_exclusion_schema() -> TableSchema {
    TableSchema::new("ModuleExclusion")
        .with_column(ColumnDef::new("ModuleID", DataType::String { max_len: 72 }).primary_key())
        .with_column(ColumnDef::new("ModuleLanguage", DataType::Short).primary_key())
        .with_column(ColumnDef::new("ExcludedID", DataType::String { max_len: 72 }).primary_key())
        .with_column(ColumnDef::new("ExcludedLanguage", DataType::Short).primary_key())
        .with_column(
            ColumnDef::new("ExcludedVersionMin", DataType::String { max_len: 32 }).nullable(),
        )
        .with_column(
            ColumnDef::new("ExcludedVersionMax", DataType::String { max_len: 32 }).nullable(),
        )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_component_row_roundtrip() -> Result<()> {
        let schema = component_schema();
        assert_eq!(schema.name, "Component");
        assert_eq!(schema.columns.len(), 6);

        let comp = ComponentName::new("Comp1")?;
        let comp_id = ComponentGuid::parse("{12345678-1234-1234-1234-1234567890AB}")?;
        let dir = DirectoryId::new("INSTALLDIR")?;

        let row = ComponentRow {
            component: comp,
            component_id: Some(comp_id),
            directory: dir,
            attributes: component_attributes::LOCAL_ONLY | component_attributes::BIT_64,
            condition: Some("VersionNT > 600".to_string()),
            key_path: Some("file1.exe".to_string()),
        };

        let rec = row.to_record();
        assert_eq!(rec.len(), 6);
        let parsed = ComponentRow::from_record(&rec);
        assert_eq!(parsed, Ok(row));
        Ok(())
    }

    #[test]
    fn test_feature_row_roundtrip() -> Result<()> {
        let schema = feature_schema();
        assert_eq!(schema.name, "Feature");
        assert_eq!(schema.columns.len(), 8);

        let feat = FeatureName::new("Feat1")?;
        let parent = FeatureName::new("FeatParent")?;
        let dir = DirectoryId::new("INSTALLDIR")?;

        let row = FeatureRow {
            feature: feat,
            feature_parent: Some(parent),
            title: Some("Main Feature".to_string()),
            description: Some("Description of feature".to_string()),
            display: Some(2),
            level: 1,
            directory: Some(dir),
            attributes: feature_attributes::FAVOR_LOCAL,
        };

        let rec = row.to_record();
        let parsed = FeatureRow::from_record(&rec);
        assert_eq!(parsed, Ok(row));
        Ok(())
    }

    #[test]
    fn test_feature_components_row_roundtrip() -> Result<()> {
        let schema = feature_components_schema();
        assert_eq!(schema.name, "FeatureComponents");

        let feat = FeatureName::new("Feat1")?;
        let comp = ComponentName::new("Comp1")?;

        let row = FeatureComponentsRow {
            feature: feat,
            component: comp,
        };

        let rec = row.to_record();
        let parsed = FeatureComponentsRow::from_record(&rec);
        assert_eq!(parsed, Ok(row));
        Ok(())
    }

    #[test]
    fn test_directory_row_roundtrip() -> Result<()> {
        let schema = directory_schema();
        assert_eq!(schema.name, "Directory");

        let dir = DirectoryId::new("TARGETDIR")?;

        let row = DirectoryRow {
            directory: dir,
            directory_parent: None,
            default_dir: "SourceDir".to_string(),
        };

        let rec = row.to_record();
        let parsed = DirectoryRow::from_record(&rec);
        assert_eq!(parsed, Ok(row));
        Ok(())
    }

    #[test]
    fn test_file_row_roundtrip() -> Result<()> {
        let schema = file_schema();
        assert_eq!(schema.name, "File");

        let file = FileKey::new("file_1")?;
        let comp = ComponentName::new("Comp1")?;

        let row = FileRow {
            file,
            component: comp,
            file_name: "test.txt".to_string(),
            file_size: 1024,
            version: Some("1.0.0.0".to_string()),
            language: Some("1033".to_string()),
            attributes: Some(file_attributes::VITAL | file_attributes::COMPRESSED),
            sequence: 1,
        };

        let rec = row.to_record();
        let parsed = FileRow::from_record(&rec);
        assert_eq!(parsed, Ok(row));
        Ok(())
    }

    #[test]
    fn test_file_hash_row_roundtrip() -> Result<()> {
        let schema = file_hash_schema();
        assert_eq!(schema.name, "FileHash");

        let file = FileKey::new("file_1")?;

        let row = FileHashRow {
            file,
            options: 0,
            hash_part1: 111,
            hash_part2: 222,
            hash_part3: 333,
            hash_part4: 444,
        };

        let rec = row.to_record();
        let parsed = FileHashRow::from_record(&rec);
        assert_eq!(parsed, Ok(row));
        Ok(())
    }

    #[test]
    fn test_media_row_roundtrip() {
        let schema = media_schema();
        assert_eq!(schema.name, "Media");

        let row = MediaRow {
            disk_id: 1,
            last_sequence: 50,
            disk_prompt: Some("Disk 1".to_string()),
            cabinet: Some("#cab1.cab".to_string()),
            volume_label: Some("DISK1".to_string()),
            source: Some("src_path".to_string()),
        };

        let rec = row.to_record();
        let parsed = MediaRow::from_record(&rec);
        assert_eq!(parsed, Ok(row));
    }

    #[test]
    fn test_property_row_roundtrip() -> Result<()> {
        let schema = property_schema();
        assert_eq!(schema.name, "Property");

        let prop = PropertyName::new("ProductName")?;

        let row = PropertyRow {
            property: prop,
            value: "My Product".to_string(),
        };

        let rec = row.to_record();
        let parsed = PropertyRow::from_record(&rec);
        assert_eq!(parsed, Ok(row));
        Ok(())
    }

    #[test]
    fn test_core_empty_string_fields() -> Result<()> {
        // ComponentRow with empty strings
        let rec_comp = Record::with_fields(vec![
            FieldValue::String("CompEmpty".to_string()),
            FieldValue::String(String::new()), // component_id empty -> None
            FieldValue::String("TARGETDIR".to_string()),
            FieldValue::Short(0),
            FieldValue::String(String::new()), // condition empty -> None
            FieldValue::String(String::new()), // key_path empty -> None
        ]);
        let parsed_comp = ComponentRow::from_record(&rec_comp)?;
        assert!(parsed_comp.component_id.is_none());
        assert!(parsed_comp.condition.is_none());
        assert!(parsed_comp.key_path.is_none());

        // FeatureRow with empty strings
        let rec_feat = Record::with_fields(vec![
            FieldValue::String("FeatEmpty".to_string()),
            FieldValue::String(String::new()), // parent empty -> None
            FieldValue::String(String::new()), // title empty -> None
            FieldValue::String(String::new()), // description empty -> None
            FieldValue::Null,
            FieldValue::Short(1),
            FieldValue::String(String::new()), // directory empty -> None
            FieldValue::Short(0),
        ]);
        let parsed_feat = FeatureRow::from_record(&rec_feat)?;
        assert!(parsed_feat.feature_parent.is_none());
        assert!(parsed_feat.title.is_none());
        assert!(parsed_feat.description.is_none());
        assert!(parsed_feat.directory.is_none());

        // DirectoryRow with empty parent string
        let rec_dir = Record::with_fields(vec![
            FieldValue::String("DirEmpty".to_string()),
            FieldValue::String(String::new()), // parent empty -> None
            FieldValue::String(".".to_string()),
        ]);
        let parsed_dir = DirectoryRow::from_record(&rec_dir)?;
        assert!(parsed_dir.directory_parent.is_none());

        // FileRow with empty version and language
        let rec_file = Record::with_fields(vec![
            FieldValue::String("file_empty".to_string()),
            FieldValue::String("CompEmpty".to_string()),
            FieldValue::String("empty.txt".to_string()),
            FieldValue::Long(0),
            FieldValue::String(String::new()), // version empty -> None
            FieldValue::String(String::new()), // language empty -> None
            FieldValue::Null,
            FieldValue::Short(1),
        ]);
        let parsed_file = FileRow::from_record(&rec_file)?;
        assert!(parsed_file.version.is_none());
        assert!(parsed_file.language.is_none());

        // MediaRow with empty disk_prompt, cabinet, volume_label
        let rec_media = Record::with_fields(vec![
            FieldValue::Short(1),
            FieldValue::Long(1),
            FieldValue::String(String::new()), // disk_prompt empty -> None
            FieldValue::String(String::new()), // cabinet empty -> None
            FieldValue::String(String::new()), // volume_label empty -> None
            FieldValue::Null,
        ]);
        let parsed_media = MediaRow::from_record(&rec_media)?;
        assert!(parsed_media.disk_prompt.is_none());
        assert!(parsed_media.cabinet.is_none());
        assert!(parsed_media.volume_label.is_none());

        // FontRow with empty font_title
        let rec_font = Record::with_fields(vec![
            FieldValue::String("FontEmpty".to_string()),
            FieldValue::String(String::new()), // font_title empty -> None
        ]);
        let parsed_font = FontRow::from_record(&rec_font)?;
        assert!(parsed_font.font_title.is_none());

        // ModuleConfigurationRow with empty strings
        let rec_mod_cfg = Record::with_fields(vec![
            FieldValue::String("MC_EMPTY".to_string()),
            FieldValue::Long(0),
            FieldValue::String(String::new()), // type_ empty -> None
            FieldValue::String(String::new()), // context_data empty -> None
            FieldValue::String(String::new()), // default_value empty -> None
            FieldValue::Null,
            FieldValue::String(String::new()), // display_name empty -> None
            FieldValue::String(String::new()), // description empty -> None
            FieldValue::String(String::new()), // help_keyword empty -> None
        ]);
        let parsed_mc = ModuleConfigurationRow::from_record(&rec_mod_cfg)?;
        assert!(parsed_mc.type_.is_none());
        assert!(parsed_mc.context_data.is_none());
        assert!(parsed_mc.default_value.is_none());
        assert!(parsed_mc.display_name.is_none());
        assert!(parsed_mc.description.is_none());
        assert!(parsed_mc.help_keyword.is_none());

        // ModuleSubstitutionRow with empty value
        let rec_subst = Record::with_fields(vec![
            FieldValue::String("Table".to_string()),
            FieldValue::String("Row".to_string()),
            FieldValue::String("Column".to_string()),
            FieldValue::String(String::new()), // value empty -> None
        ]);
        let parsed_ms = ModuleSubstitutionRow::from_record(&rec_subst)?;
        assert!(parsed_ms.value.is_none());

        Ok(())
    }

    #[test]
    fn test_core_parsing_errors() {
        assert!(ComponentRow::from_record(&Record::new()).is_err());
        assert!(FeatureRow::from_record(&Record::new()).is_err());
        assert!(FeatureComponentsRow::from_record(&Record::new()).is_err());
        assert!(DirectoryRow::from_record(&Record::new()).is_err());
        assert!(FileRow::from_record(&Record::new()).is_err());
        assert!(FileHashRow::from_record(&Record::new()).is_err());
        assert!(MediaRow::from_record(&Record::new()).is_err());
        assert!(PropertyRow::from_record(&Record::new()).is_err());

        // Field type errors
        let bad_null = Record::with_fields(vec![FieldValue::Null; 8]);
        assert!(ComponentRow::from_record(&bad_null).is_err());
        assert!(FeatureRow::from_record(&bad_null).is_err());
        assert!(FeatureComponentsRow::from_record(&bad_null).is_err());
        assert!(DirectoryRow::from_record(&bad_null).is_err());
        assert!(FileRow::from_record(&bad_null).is_err());
        assert!(FileHashRow::from_record(&bad_null).is_err());
        assert!(MediaRow::from_record(&bad_null).is_err());
        assert!(PropertyRow::from_record(&bad_null).is_err());

        // Secondary PK / FK errors
        let comp_bad_dir = Record::with_fields(vec![
            FieldValue::String("Comp".to_string()),
            FieldValue::Null,
            FieldValue::Null, // Directory_ is null
            FieldValue::Short(0),
            FieldValue::Null,
            FieldValue::Null,
        ]);
        assert!(ComponentRow::from_record(&comp_bad_dir).is_err());

        let feat_comp_bad = Record::with_fields(vec![
            FieldValue::String("Feat".to_string()),
            FieldValue::Null, // Component_ is null
        ]);
        assert!(FeatureComponentsRow::from_record(&feat_comp_bad).is_err());

        let file_bad_comp = Record::with_fields(vec![
            FieldValue::String("File".to_string()),
            FieldValue::Null, // Component_ is null
            FieldValue::String("file.txt".to_string()),
            FieldValue::Long(0),
            FieldValue::Null,
            FieldValue::Null,
            FieldValue::Null,
            FieldValue::Short(1),
        ]);
        assert!(FileRow::from_record(&file_bad_comp).is_err());

        let file_bad_name = Record::with_fields(vec![
            FieldValue::String("File".to_string()),
            FieldValue::String("Comp".to_string()),
            FieldValue::Null, // FileName is null
            FieldValue::Long(0),
            FieldValue::Null,
            FieldValue::Null,
            FieldValue::Null,
            FieldValue::Short(1),
        ]);
        assert!(FileRow::from_record(&file_bad_name).is_err());
    }

    #[test]
    fn test_core_minimal_rows_roundtrip() -> Result<()> {
        // ComponentRow minimal
        let comp = ComponentName::new("CompMin")?;
        let dir = DirectoryId::new("TARGETDIR")?;
        let comp_row = ComponentRow {
            component: comp,
            component_id: None,
            directory: dir,
            attributes: 0,
            condition: None,
            key_path: None,
        };
        let rec = comp_row.to_record();
        assert_eq!(ComponentRow::from_record(&rec), Ok(comp_row));

        // FeatureRow minimal
        let feat = FeatureName::new("FeatMin")?;
        let feat_row = FeatureRow {
            feature: feat,
            feature_parent: None,
            title: None,
            description: None,
            display: None,
            level: 1,
            directory: None,
            attributes: 0,
        };
        let rec_feat = feat_row.to_record();
        assert_eq!(FeatureRow::from_record(&rec_feat), Ok(feat_row));

        // FileRow minimal
        let file = FileKey::new("file_min")?;
        let comp_f = ComponentName::new("CompMin")?;
        let file_row = FileRow {
            file,
            component: comp_f,
            file_name: "min.txt".to_string(),
            file_size: 0,
            version: None,
            language: None,
            attributes: None,
            sequence: 1,
        };
        let rec_file = file_row.to_record();
        assert_eq!(FileRow::from_record(&rec_file), Ok(file_row));

        // DirectoryRow minimal
        let dir_min = DirectoryId::new("DIRMIN")?;
        let dir_row = DirectoryRow {
            directory: dir_min,
            directory_parent: None,
            default_dir: ".".to_string(),
        };
        let rec_dir = dir_row.to_record();
        assert_eq!(DirectoryRow::from_record(&rec_dir), Ok(dir_row));

        // MediaRow minimal
        let media_row = MediaRow {
            disk_id: 1,
            last_sequence: 1,
            disk_prompt: None,
            cabinet: None,
            volume_label: None,
            source: None,
        };
        let rec_media = media_row.to_record();
        assert_eq!(MediaRow::from_record(&rec_media), Ok(media_row));

        Ok(())
    }

    #[test]
    fn test_core_fallback_rows_roundtrip() -> Result<()> {
        // ComponentRow non-short attributes fallback to 0
        let rec_comp_fallback = Record::with_fields(vec![
            FieldValue::String("CompFallback".to_string()),
            FieldValue::Null,
            FieldValue::String("TARGETDIR".to_string()),
            FieldValue::Null, // non-short -> 0
            FieldValue::Null,
            FieldValue::Null,
        ]);
        let parsed_comp_fb = ComponentRow::from_record(&rec_comp_fallback)?;
        assert_eq!(parsed_comp_fb.attributes, 0);

        // FeatureRow non-short level and attributes fallback
        let rec_feat_fallback = Record::with_fields(vec![
            FieldValue::String("FeatFallback".to_string()),
            FieldValue::Null,
            FieldValue::Null,
            FieldValue::Null,
            FieldValue::Null,
            FieldValue::Null, // non-short level -> 1
            FieldValue::Null,
            FieldValue::Null, // non-short attributes -> 0
        ]);
        let parsed_feat_fb = FeatureRow::from_record(&rec_feat_fallback)?;
        assert_eq!(parsed_feat_fb.level, 1);
        assert_eq!(parsed_feat_fb.attributes, 0);

        // FileRow non-long file_size fallback to 0, non-short sequence fallback to 1
        let rec_file_fallback = Record::with_fields(vec![
            FieldValue::String("file_fb".to_string()),
            FieldValue::String("CompMin".to_string()),
            FieldValue::String("file_fb.txt".to_string()),
            FieldValue::Null, // non-long file_size -> 0
            FieldValue::Null,
            FieldValue::Null,
            FieldValue::Null,
            FieldValue::Null, // non-short sequence -> 1
        ]);
        let parsed_file_fb = FileRow::from_record(&rec_file_fallback)?;
        assert_eq!(parsed_file_fb.file_size, 0);
        assert_eq!(parsed_file_fb.sequence, 1);

        // DirectoryRow with parent and fallback default_dir
        let rec_dir_parent_fallback = Record::with_fields(vec![
            FieldValue::String("SUBDIR".to_string()),
            FieldValue::String("TARGETDIR".to_string()),
            FieldValue::Null, // non-string default_dir -> "."
        ]);
        let parsed_dir_fb = DirectoryRow::from_record(&rec_dir_parent_fallback)?;
        assert_eq!(
            parsed_dir_fb.directory_parent,
            Some(DirectoryId::new("TARGETDIR")?)
        );
        assert_eq!(parsed_dir_fb.default_dir, ".");

        // MediaRow fallback for last_sequence and empty source string
        let rec_media_fallback = Record::with_fields(vec![
            FieldValue::Short(1),
            FieldValue::Null, // non-long last_sequence -> 0
            FieldValue::Null,
            FieldValue::Null,
            FieldValue::Null,
            FieldValue::String(String::new()), // empty source string -> None
        ]);
        let parsed_media_fb = MediaRow::from_record(&rec_media_fallback)?;
        assert_eq!(parsed_media_fb.last_sequence, 0);
        assert!(parsed_media_fb.source.is_none());

        // FileHashRow fallback fields
        let file_h = FileKey::new("file_h")?;
        let rec_filehash_fallback = Record::with_fields(vec![
            FieldValue::String(file_h.as_str().to_string()),
            FieldValue::Null, // non-short options -> 0
            FieldValue::Null, // non-long hash_part1 -> 0
            FieldValue::Null, // non-long hash_part2 -> 0
            FieldValue::Null, // non-long hash_part3 -> 0
            FieldValue::Null, // non-long hash_part4 -> 0
        ]);
        let parsed_fh_fb = FileHashRow::from_record(&rec_filehash_fallback)?;
        assert_eq!(parsed_fh_fb.options, 0);
        assert_eq!(parsed_fh_fb.hash_part1, 0);
        assert_eq!(parsed_fh_fb.hash_part2, 0);
        assert_eq!(parsed_fh_fb.hash_part3, 0);
        assert_eq!(parsed_fh_fb.hash_part4, 0);

        Ok(())
    }

    #[test]
    fn test_core_fallback_rows_part2() -> Result<()> {
        // PropertyRow non-string value fallback
        let rec_prop_fallback = Record::with_fields(vec![
            FieldValue::String("PROP_FB".to_string()),
            FieldValue::Null, // non-string value -> empty string
        ]);
        let parsed_prop_fb = PropertyRow::from_record(&rec_prop_fallback)?;
        assert_eq!(parsed_prop_fb.value, "");

        // BinaryRow non-stream data fallback
        let rec_bin_fallback = Record::with_fields(vec![
            FieldValue::String("BIN_FB".to_string()),
            FieldValue::Null, // non-stream data -> pool id 0
        ]);
        let parsed_bin_fb = BinaryRow::from_record(&rec_bin_fallback)?;
        assert_eq!(
            parsed_bin_fb.data,
            crate::database::tables::types::StringPoolId::new(0)
        );

        // FontRow non-string font_title fallback
        let font_file = FileKey::new("FontKey")?;
        let rec_font_fallback = Record::with_fields(vec![
            FieldValue::String(font_file.as_str().to_string()),
            FieldValue::Null, // non-string title -> None
        ]);
        let parsed_font_fb = FontRow::from_record(&rec_font_fallback)?;
        assert!(parsed_font_fb.font_title.is_none());

        // PatchPackageRow non-short media fallback
        let rec_patch_fallback = Record::with_fields(vec![
            FieldValue::String("{22222222-3333-4444-5555-666666666666}".to_string()),
            FieldValue::Null, // non-short media -> 1
        ]);
        let parsed_patch_fb = PatchPackageRow::from_record(&rec_patch_fallback)?;
        assert_eq!(parsed_patch_fb.media, 1);

        // ModuleConfigurationRow non-long format and non-string/non-long optional fields fallback
        let rec_cfg_fallback = Record::with_fields(vec![
            FieldValue::String("MC_FB".to_string()),
            FieldValue::Null, // non-long format -> 0
            FieldValue::Null, // type -> None
            FieldValue::Null, // context_data -> None
            FieldValue::Null, // default_value -> None
            FieldValue::Null, // attributes -> None
            FieldValue::Null, // display_name -> None
            FieldValue::Null, // description -> None
            FieldValue::Null, // help_keyword -> None
        ]);
        let parsed_mc_fb = ModuleConfigurationRow::from_record(&rec_cfg_fallback)?;
        assert_eq!(parsed_mc_fb.format, 0);
        assert!(parsed_mc_fb.type_.is_none());
        assert!(parsed_mc_fb.context_data.is_none());
        assert!(parsed_mc_fb.default_value.is_none());
        assert!(parsed_mc_fb.attributes.is_none());
        assert!(parsed_mc_fb.display_name.is_none());
        assert!(parsed_mc_fb.description.is_none());
        assert!(parsed_mc_fb.help_keyword.is_none());

        // ModuleSubstitutionRow non-string value fallback
        let rec_sub_fallback = Record::with_fields(vec![
            FieldValue::String("Table".to_string()),
            FieldValue::String("Row".to_string()),
            FieldValue::String("Column".to_string()),
            FieldValue::Null, // value -> None
        ]);
        let parsed_ms_fb = ModuleSubstitutionRow::from_record(&rec_sub_fallback)?;
        assert!(parsed_ms_fb.value.is_none());

        // ModuleIgnoreModularizationRow non-short type fallback
        let rec_mim_fallback = Record::with_fields(vec![
            FieldValue::String("IgnoreName".to_string()),
            FieldValue::Null, // type -> None
        ]);
        let parsed_mim_fb = ModuleIgnoreModularizationRow::from_record(&rec_mim_fallback)?;
        assert!(parsed_mim_fb.type_.is_none());

        Ok(())
    }

    #[test]
    fn test_binary_row_roundtrip() -> Result<()> {
        let schema = binary_schema();
        assert_eq!(schema.name, "Binary");
        assert_eq!(schema.columns.len(), 2);

        let row = BinaryRow {
            name: "CustomActionDll".to_string(),
            data: crate::database::tables::types::StringPoolId::new(42),
        };
        let rec = row.to_record();
        let parsed = BinaryRow::from_record(&rec)?;
        assert_eq!(parsed, row);

        assert!(BinaryRow::from_record(&Record::new()).is_err());
        let rec_bad_pk = Record::with_fields(vec![FieldValue::Short(1), FieldValue::Null]);
        assert!(BinaryRow::from_record(&rec_bad_pk).is_err());
        Ok(())
    }

    #[test]
    fn test_font_row_roundtrip() -> Result<()> {
        let schema = font_schema();
        assert_eq!(schema.name, "Font");
        assert_eq!(schema.columns.len(), 2);

        let file = FileKey::new("ArialFont")?;
        let row = FontRow {
            file,
            font_title: Some("Arial Regular".to_string()),
        };
        let rec = row.to_record();
        let parsed = FontRow::from_record(&rec)?;
        assert_eq!(parsed, row);

        assert!(FontRow::from_record(&Record::new()).is_err());
        let rec_bad_pk = Record::with_fields(vec![FieldValue::Short(1), FieldValue::Null]);
        assert!(FontRow::from_record(&rec_bad_pk).is_err());
        Ok(())
    }

    #[test]
    fn test_patch_package_row_roundtrip() -> Result<()> {
        let schema = patch_package_schema();
        assert_eq!(schema.name, "PatchPackage");
        assert_eq!(schema.columns.len(), 2);

        let row = PatchPackageRow {
            patch_id: "{11111111-2222-3333-4444-555555555555}".to_string(),
            media: 1,
        };
        let rec = row.to_record();
        let parsed = PatchPackageRow::from_record(&rec)?;
        assert_eq!(parsed, row);

        assert!(PatchPackageRow::from_record(&Record::new()).is_err());
        let rec_bad_pk = Record::with_fields(vec![FieldValue::Short(1), FieldValue::Null]);
        assert!(PatchPackageRow::from_record(&rec_bad_pk).is_err());
        Ok(())
    }

    #[test]
    fn test_module_configuration_row_roundtrip() -> Result<()> {
        let schema = module_configuration_schema();
        assert_eq!(schema.name, "ModuleConfiguration");
        assert_eq!(schema.columns.len(), 9);

        let row = ModuleConfigurationRow {
            name: "PORT_PARAM".to_string(),
            format: 0,
            type_: Some("Integer".to_string()),
            context_data: Some("80;443;8080".to_string()),
            default_value: Some("8080".to_string()),
            attributes: Some(1),
            display_name: Some("Server Port".to_string()),
            description: Some("Target TCP port".to_string()),
            help_keyword: Some("port_help".to_string()),
        };
        let rec = row.to_record();
        let parsed = ModuleConfigurationRow::from_record(&rec)?;
        assert_eq!(parsed, row);

        assert!(ModuleConfigurationRow::from_record(&Record::new()).is_err());
        let rec_bad_pk = Record::with_fields(vec![FieldValue::Short(1), FieldValue::Null]);
        assert!(ModuleConfigurationRow::from_record(&rec_bad_pk).is_err());
        Ok(())
    }

    #[test]
    fn test_module_substitution_row_roundtrip() -> Result<()> {
        let schema = module_substitution_schema();
        assert_eq!(schema.name, "ModuleSubstitution");
        assert_eq!(schema.columns.len(), 4);

        let row = ModuleSubstitutionRow {
            table: "Property".to_string(),
            row: "PORT".to_string(),
            column: "Value".to_string(),
            value: Some("[=PORT_PARAM]".to_string()),
        };
        let rec = row.to_record();
        let parsed = ModuleSubstitutionRow::from_record(&rec)?;
        assert_eq!(parsed, row);

        assert!(ModuleSubstitutionRow::from_record(&Record::new()).is_err());
        let rec_bad_table = Record::with_fields(vec![
            FieldValue::Short(1),
            FieldValue::String("row".to_string()),
            FieldValue::String("col".to_string()),
        ]);
        assert!(ModuleSubstitutionRow::from_record(&rec_bad_table).is_err());
        let rec_bad_row = Record::with_fields(vec![
            FieldValue::String("tbl".to_string()),
            FieldValue::Short(1),
            FieldValue::String("col".to_string()),
        ]);
        assert!(ModuleSubstitutionRow::from_record(&rec_bad_row).is_err());
        let rec_bad_col = Record::with_fields(vec![
            FieldValue::String("tbl".to_string()),
            FieldValue::String("row".to_string()),
            FieldValue::Short(1),
        ]);
        assert!(ModuleSubstitutionRow::from_record(&rec_bad_col).is_err());
        Ok(())
    }

    #[test]
    fn test_module_ignore_modularization_row_roundtrip() -> Result<()> {
        let schema = module_ignore_modularization_schema();
        assert_eq!(schema.name, "ModuleIgnoreModularization");
        assert_eq!(schema.columns.len(), 2);

        let row = ModuleIgnoreModularizationRow {
            name: "AppSearch".to_string(),
            type_: Some(1),
        };
        let rec = row.to_record();
        let parsed = ModuleIgnoreModularizationRow::from_record(&rec)?;
        assert_eq!(parsed, row);

        assert!(ModuleIgnoreModularizationRow::from_record(&Record::new()).is_err());
        let rec_bad_pk = Record::with_fields(vec![FieldValue::Short(1)]);
        assert!(ModuleIgnoreModularizationRow::from_record(&rec_bad_pk).is_err());
        Ok(())
    }

    #[test]
    #[allow(clippy::too_many_lines)]
    fn test_merge_module_system_tables_roundtrip() -> Result<()> {
        // ModuleSignature
        let sig_schema = module_signature_schema();
        assert_eq!(sig_schema.name, "ModuleSignature");
        assert_eq!(sig_schema.columns.len(), 3);
        let sig_row = ModuleSignatureRow {
            module_id: "SampleModule.GUID".to_string(),
            language: 1033,
            version: "1.2.3".to_string(),
        };
        let sig_rec = sig_row.to_record();
        let parsed_sig = ModuleSignatureRow::from_record(&sig_rec)?;
        assert_eq!(parsed_sig, sig_row);
        assert!(ModuleSignatureRow::from_record(&Record::new()).is_err());
        assert!(ModuleSignatureRow::from_record(&Record::with_fields(vec![
            FieldValue::Short(1),
            FieldValue::Short(1033),
            FieldValue::String("1.0".to_string())
        ]))
        .is_err());
        assert!(ModuleSignatureRow::from_record(&Record::with_fields(vec![
            FieldValue::String("Mod".to_string()),
            FieldValue::String("BadLang".to_string()),
            FieldValue::String("1.0".to_string())
        ]))
        .is_err());
        assert!(ModuleSignatureRow::from_record(&Record::with_fields(vec![
            FieldValue::String("Mod".to_string()),
            FieldValue::Short(1033),
            FieldValue::Short(1)
        ]))
        .is_err());

        // ModuleComponents
        let comp_schema = module_components_schema();
        assert_eq!(comp_schema.name, "ModuleComponents");
        assert_eq!(comp_schema.columns.len(), 3);
        let comp_row = ModuleComponentsRow {
            component: "Comp1.GUID".to_string(),
            module_id: "SampleModule.GUID".to_string(),
            language: 1033,
        };
        let comp_rec = comp_row.to_record();
        let parsed_comp = ModuleComponentsRow::from_record(&comp_rec)?;
        assert_eq!(parsed_comp, comp_row);
        assert!(ModuleComponentsRow::from_record(&Record::new()).is_err());
        assert!(ModuleComponentsRow::from_record(&Record::with_fields(vec![
            FieldValue::Short(1),
            FieldValue::String("Mod".to_string()),
            FieldValue::Short(1033)
        ]))
        .is_err());
        assert!(ModuleComponentsRow::from_record(&Record::with_fields(vec![
            FieldValue::String("Comp".to_string()),
            FieldValue::Short(1),
            FieldValue::Short(1033)
        ]))
        .is_err());
        assert!(ModuleComponentsRow::from_record(&Record::with_fields(vec![
            FieldValue::String("Comp".to_string()),
            FieldValue::String("Mod".to_string()),
            FieldValue::String("BadLang".to_string())
        ]))
        .is_err());

        // ModuleDependency
        let dep_schema = module_dependency_schema();
        assert_eq!(dep_schema.name, "ModuleDependency");
        assert_eq!(dep_schema.columns.len(), 5);
        let dep_row = ModuleDependencyRow {
            module_id: "SampleModule.GUID".to_string(),
            module_language: 1033,
            required_id: "ReqModule.GUID".to_string(),
            required_language: 1033,
            required_version: Some("2.0.0".to_string()),
        };
        let dep_rec = dep_row.to_record();
        let parsed_dep = ModuleDependencyRow::from_record(&dep_rec)?;
        assert_eq!(parsed_dep, dep_row);
        let dep_row_none = ModuleDependencyRow {
            module_id: "SampleModule.GUID".to_string(),
            module_language: 1033,
            required_id: "ReqModule.GUID".to_string(),
            required_language: 1033,
            required_version: None,
        };
        let dep_rec_none = dep_row_none.to_record();
        let parsed_dep_none = ModuleDependencyRow::from_record(&dep_rec_none)?;
        assert_eq!(parsed_dep_none, dep_row_none);
        assert!(ModuleDependencyRow::from_record(&Record::new()).is_err());
        assert!(ModuleDependencyRow::from_record(&Record::with_fields(vec![
            FieldValue::Short(1),
            FieldValue::Short(1033),
            FieldValue::String("Req".to_string()),
            FieldValue::Short(1033)
        ]))
        .is_err());
        assert!(ModuleDependencyRow::from_record(&Record::with_fields(vec![
            FieldValue::String("Mod".to_string()),
            FieldValue::String("BadLang".to_string()),
            FieldValue::String("Req".to_string()),
            FieldValue::Short(1033)
        ]))
        .is_err());
        assert!(ModuleDependencyRow::from_record(&Record::with_fields(vec![
            FieldValue::String("Mod".to_string()),
            FieldValue::Short(1033),
            FieldValue::Short(1),
            FieldValue::Short(1033)
        ]))
        .is_err());
        assert!(ModuleDependencyRow::from_record(&Record::with_fields(vec![
            FieldValue::String("Mod".to_string()),
            FieldValue::Short(1033),
            FieldValue::String("Req".to_string()),
            FieldValue::String("BadLang".to_string())
        ]))
        .is_err());

        // ModuleExclusion
        let excl_schema = module_exclusion_schema();
        assert_eq!(excl_schema.name, "ModuleExclusion");
        assert_eq!(excl_schema.columns.len(), 6);
        let excl_row = ModuleExclusionRow {
            module_id: "SampleModule.GUID".to_string(),
            module_language: 1033,
            excluded_id: "ExclModule.GUID".to_string(),
            excluded_language: 1033,
            excluded_version_min: Some("1.0.0".to_string()),
            excluded_version_max: Some("2.0.0".to_string()),
        };
        let excl_rec = excl_row.to_record();
        let parsed_excl = ModuleExclusionRow::from_record(&excl_rec)?;
        assert_eq!(parsed_excl, excl_row);
        let excl_row_none = ModuleExclusionRow {
            module_id: "SampleModule.GUID".to_string(),
            module_language: 1033,
            excluded_id: "ExclModule.GUID".to_string(),
            excluded_language: 1033,
            excluded_version_min: None,
            excluded_version_max: None,
        };
        let excl_rec_none = excl_row_none.to_record();
        let parsed_excl_none = ModuleExclusionRow::from_record(&excl_rec_none)?;
        assert_eq!(parsed_excl_none, excl_row_none);
        assert!(ModuleExclusionRow::from_record(&Record::new()).is_err());
        assert!(ModuleExclusionRow::from_record(&Record::with_fields(vec![
            FieldValue::Short(1),
            FieldValue::Short(1033),
            FieldValue::String("Excl".to_string()),
            FieldValue::Short(1033)
        ]))
        .is_err());
        assert!(ModuleExclusionRow::from_record(&Record::with_fields(vec![
            FieldValue::String("Mod".to_string()),
            FieldValue::String("BadLang".to_string()),
            FieldValue::String("Excl".to_string()),
            FieldValue::Short(1033)
        ]))
        .is_err());
        assert!(ModuleExclusionRow::from_record(&Record::with_fields(vec![
            FieldValue::String("Mod".to_string()),
            FieldValue::Short(1033),
            FieldValue::Short(1),
            FieldValue::Short(1033)
        ]))
        .is_err());
        assert!(ModuleExclusionRow::from_record(&Record::with_fields(vec![
            FieldValue::String("Mod".to_string()),
            FieldValue::Short(1033),
            FieldValue::String("Excl".to_string()),
            FieldValue::String("BadLang".to_string())
        ]))
        .is_err());

        Ok(())
    }
}

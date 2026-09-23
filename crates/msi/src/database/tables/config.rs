//! Configuration and registration tables for Windows Installer database.
//!
//! Implements schemas and typed rows for:
//! - `Registry`
//! - `RemoveRegistry`
//! - `Environment`
//! - `Shortcut`
//! - `Icon`
//! - `ServiceInstall`
//! - `ServiceControl`
//! - `Upgrade`
//! - `Condition`
//! - `LaunchCondition`
//! - `AppSearch`
//! - `CompLocator`
//! - `DrLocator`
//! - `FileSearch`
//! - `IniLocator`
//! - `RegLocator`
//! - `Signature`

use crate::database::catalogs::TableSchema;
use crate::database::column::{ColumnDef, DataType};
use crate::database::tables::record::{FieldValue, Record};
use crate::database::tables::types::ComponentName;
use crate::error::{Error, Result};

/// Standard root keys for `Registry` table.
pub mod registry_root {
    /// `HKEY_CLASSES_ROOT` (`0`).
    pub const HKCR: i16 = 0;
    /// `HKEY_CURRENT_USER` (`1`).
    pub const HKCU: i16 = 1;
    /// `HKEY_LOCAL_MACHINE` (`2`).
    pub const HKLM: i16 = 2;
    /// `HKEY_USERS` (`3`).
    pub const HKU: i16 = 3;
}

/// Row in the `Registry` table.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegistryRow {
    /// Registry record primary key (max 72 chars).
    pub registry: String,
    /// Root key integer (`0` HKCR, `1` HKCU, `2` HKLM, `3` HKU).
    pub root: i16,
    /// Key path (localizable, max 255 chars).
    pub key: String,
    /// Value name (nullable, localizable, max 255 chars).
    pub name: Option<String>,
    /// Formatted value string (nullable, localizable, max 0 / unbounded).
    pub value: Option<String>,
    /// Foreign key to Component table.
    pub component: ComponentName,
}

impl RegistryRow {
    /// Converts this row into a generic [`Record`].
    ///
    /// # Returns
    ///
    /// A generic [`Record`].
    #[must_use]
    pub fn to_record(&self) -> Record {
        Record::with_fields(vec![
            FieldValue::String(self.registry.clone()),
            FieldValue::Short(self.root),
            FieldValue::String(self.key.clone()),
            self.name
                .as_ref()
                .map_or(FieldValue::Null, |n| FieldValue::String(n.clone())),
            self.value
                .as_ref()
                .map_or(FieldValue::Null, |v| FieldValue::String(v.clone())),
            FieldValue::String(self.component.as_str().to_string()),
        ])
    }

    /// Parses a [`RegistryRow`] from a generic [`Record`].
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
        let registry = match rec.get(0) {
            Some(FieldValue::String(s)) => s.clone(),
            _ => {
                return Err(Error::Validation {
                    element: "Registry.Registry".to_string(),
                    reason: "missing Registry PK".to_string(),
                })
            }
        };
        let root = match rec.get(1) {
            Some(FieldValue::Short(r)) => *r,
            _ => 0,
        };
        let key = match rec.get(2) {
            Some(FieldValue::String(k)) => k.clone(),
            _ => String::new(),
        };
        let name = match rec.get(3) {
            Some(FieldValue::String(n)) if !n.is_empty() => Some(n.clone()),
            _ => None,
        };
        let value = match rec.get(4) {
            Some(FieldValue::String(v)) if !v.is_empty() => Some(v.clone()),
            _ => None,
        };
        let component = match rec.get(5) {
            Some(FieldValue::String(c)) => ComponentName::new(c.as_str())?,
            _ => {
                return Err(Error::Validation {
                    element: "Registry.Component_".to_string(),
                    reason: "missing Component_".to_string(),
                })
            }
        };
        Ok(Self {
            registry,
            root,
            key,
            name,
            value,
            component,
        })
    }
}

/// Creates official schema for `Registry` table.
///
/// # Returns
///
/// [`TableSchema`] for `Registry`.
#[must_use]
pub fn registry_schema() -> TableSchema {
    TableSchema::new("Registry")
        .with_column(ColumnDef::new("Registry", DataType::String { max_len: 72 }).primary_key())
        .with_column(ColumnDef::new("Root", DataType::Short))
        .with_column(ColumnDef::new("Key", DataType::String { max_len: 255 }).localizable())
        .with_column(
            ColumnDef::new("Name", DataType::String { max_len: 255 })
                .nullable()
                .localizable(),
        )
        .with_column(
            ColumnDef::new("Value", DataType::String { max_len: 0 })
                .nullable()
                .localizable(),
        )
        .with_column(ColumnDef::new(
            "Component_",
            DataType::String { max_len: 72 },
        ))
}

/// Creates official schema for `RemoveRegistry` table.
///
/// # Returns
///
/// [`TableSchema`] for `RemoveRegistry`.
#[must_use]
pub fn remove_registry_schema() -> TableSchema {
    TableSchema::new("RemoveRegistry")
        .with_column(
            ColumnDef::new("RemoveRegistry", DataType::String { max_len: 72 }).primary_key(),
        )
        .with_column(ColumnDef::new("Root", DataType::Short))
        .with_column(ColumnDef::new("Key", DataType::String { max_len: 255 }).localizable())
        .with_column(
            ColumnDef::new("Name", DataType::String { max_len: 255 })
                .nullable()
                .localizable(),
        )
        .with_column(ColumnDef::new(
            "Component_",
            DataType::String { max_len: 72 },
        ))
}

/// Creates official schema for `Environment` table.
///
/// # Returns
///
/// [`TableSchema`] for `Environment`.
#[must_use]
pub fn environment_schema() -> TableSchema {
    TableSchema::new("Environment")
        .with_column(ColumnDef::new("Environment", DataType::String { max_len: 72 }).primary_key())
        .with_column(ColumnDef::new("Name", DataType::String { max_len: 255 }).localizable())
        .with_column(ColumnDef::new("Value", DataType::String { max_len: 0 }).localizable())
        .with_column(ColumnDef::new(
            "Component_",
            DataType::String { max_len: 72 },
        ))
}

/// Row in the `Environment` table.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EnvironmentRow {
    /// Primary key for the environment variable record (max 72 chars).
    pub environment: String,
    /// Environment variable name (with optional prefix like `=` or `+` or `-`).
    pub name: String,
    /// Environment variable formatted value.
    pub value: String,
    /// Controlling component (foreign key into Component table).
    pub component: ComponentName,
}

impl EnvironmentRow {
    /// Converts this row into a generic [`Record`].
    ///
    /// # Returns
    ///
    /// A generic [`Record`].
    #[must_use]
    pub fn to_record(&self) -> Record {
        Record::with_fields(vec![
            FieldValue::String(self.environment.clone()),
            FieldValue::String(self.name.clone()),
            FieldValue::String(self.value.clone()),
            FieldValue::String(self.component.as_str().to_string()),
        ])
    }

    /// Parses an [`EnvironmentRow`] from a generic [`Record`].
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
        let environment = match rec.get(0) {
            Some(FieldValue::String(s)) => s.clone(),
            _ => {
                return Err(Error::Validation {
                    element: "Environment.Environment".to_string(),
                    reason: "missing Environment PK".to_string(),
                })
            }
        };
        let name = match rec.get(1) {
            Some(FieldValue::String(s)) => s.clone(),
            _ => {
                return Err(Error::Validation {
                    element: "Environment.Name".to_string(),
                    reason: "missing Name".to_string(),
                })
            }
        };
        let value = match rec.get(2) {
            Some(FieldValue::String(s)) => s.clone(),
            _ => {
                return Err(Error::Validation {
                    element: "Environment.Value".to_string(),
                    reason: "missing Value".to_string(),
                })
            }
        };
        let component = match rec.get(3) {
            Some(FieldValue::String(s)) => ComponentName::new(s.as_str())?,
            _ => {
                return Err(Error::Validation {
                    element: "Environment.Component_".to_string(),
                    reason: "missing Component_".to_string(),
                })
            }
        };
        Ok(Self {
            environment,
            name,
            value,
            component,
        })
    }
}

/// Creates official schema for `Shortcut` table.
///
/// # Returns
///
/// [`TableSchema`] for `Shortcut`.
#[must_use]
pub fn shortcut_schema() -> TableSchema {
    TableSchema::new("Shortcut")
        .with_column(ColumnDef::new("Shortcut", DataType::String { max_len: 72 }).primary_key())
        .with_column(ColumnDef::new(
            "Directory_",
            DataType::String { max_len: 72 },
        ))
        .with_column(ColumnDef::new("Name", DataType::String { max_len: 128 }).localizable())
        .with_column(ColumnDef::new(
            "Component_",
            DataType::String { max_len: 72 },
        ))
        .with_column(ColumnDef::new("Target", DataType::String { max_len: 72 }))
        .with_column(ColumnDef::new("Arguments", DataType::String { max_len: 255 }).nullable())
        .with_column(
            ColumnDef::new("Description", DataType::String { max_len: 0 })
                .nullable()
                .localizable(),
        )
        .with_column(ColumnDef::new("Hotkey", DataType::Short).nullable())
        .with_column(ColumnDef::new("Icon_", DataType::String { max_len: 72 }).nullable())
        .with_column(ColumnDef::new("IconIndex", DataType::Short).nullable())
        .with_column(ColumnDef::new("ShowCmd", DataType::Short).nullable())
        .with_column(ColumnDef::new("WkDir", DataType::String { max_len: 72 }).nullable())
}

/// Creates official schema for `Icon` table.
///
/// # Returns
///
/// [`TableSchema`] for `Icon`.
#[must_use]
pub fn icon_schema() -> TableSchema {
    TableSchema::new("Icon")
        .with_column(ColumnDef::new("Name", DataType::String { max_len: 72 }).primary_key())
        .with_column(ColumnDef::new("Data", DataType::Stream))
}

/// Creates official schema for `ServiceInstall` table.
///
/// # Returns
///
/// [`TableSchema`] for `ServiceInstall`.
#[must_use]
pub fn service_install_schema() -> TableSchema {
    TableSchema::new("ServiceInstall")
        .with_column(
            ColumnDef::new("ServiceInstall", DataType::String { max_len: 72 }).primary_key(),
        )
        .with_column(ColumnDef::new("Name", DataType::String { max_len: 255 }))
        .with_column(
            ColumnDef::new("DisplayName", DataType::String { max_len: 255 })
                .nullable()
                .localizable(),
        )
        .with_column(ColumnDef::new("ServiceType", DataType::Long))
        .with_column(ColumnDef::new("StartType", DataType::Long))
        .with_column(ColumnDef::new("ErrorControl", DataType::Long))
        .with_column(ColumnDef::new("LoadOrderGroup", DataType::String { max_len: 255 }).nullable())
        .with_column(ColumnDef::new("Dependencies", DataType::String { max_len: 255 }).nullable())
        .with_column(ColumnDef::new("StartName", DataType::String { max_len: 255 }).nullable())
        .with_column(ColumnDef::new("Password", DataType::String { max_len: 255 }).nullable())
        .with_column(ColumnDef::new(
            "Component_",
            DataType::String { max_len: 72 },
        ))
}

/// Creates official schema for `ServiceControl` table.
///
/// # Returns
///
/// [`TableSchema`] for `ServiceControl`.
#[must_use]
pub fn service_control_schema() -> TableSchema {
    TableSchema::new("ServiceControl")
        .with_column(
            ColumnDef::new("ServiceControl", DataType::String { max_len: 72 }).primary_key(),
        )
        .with_column(ColumnDef::new("Name", DataType::String { max_len: 255 }).localizable())
        .with_column(ColumnDef::new("Event", DataType::Short))
        .with_column(
            ColumnDef::new("Arguments", DataType::String { max_len: 255 })
                .nullable()
                .localizable(),
        )
        .with_column(ColumnDef::new("Wait", DataType::Short).nullable())
        .with_column(ColumnDef::new(
            "Component_",
            DataType::String { max_len: 72 },
        ))
}

/// Creates official schema for `Upgrade` table.
///
/// # Returns
///
/// [`TableSchema`] for `Upgrade`.
#[must_use]
pub fn upgrade_schema() -> TableSchema {
    TableSchema::new("Upgrade")
        .with_column(ColumnDef::new("UpgradeCode", DataType::String { max_len: 38 }).primary_key())
        .with_column(ColumnDef::new("VersionMin", DataType::String { max_len: 20 }).nullable())
        .with_column(ColumnDef::new("VersionMax", DataType::String { max_len: 20 }).nullable())
        .with_column(ColumnDef::new("Language", DataType::String { max_len: 255 }).nullable())
        .with_column(ColumnDef::new("Attributes", DataType::Long))
        .with_column(ColumnDef::new("Remove", DataType::String { max_len: 255 }).nullable())
        .with_column(ColumnDef::new(
            "ActionProperty",
            DataType::String { max_len: 72 },
        ))
}

/// Creates official schema for `Condition` table.
///
/// # Returns
///
/// [`TableSchema`] for `Condition`.
#[must_use]
pub fn condition_schema() -> TableSchema {
    TableSchema::new("Condition")
        .with_column(ColumnDef::new("Feature_", DataType::String { max_len: 38 }).primary_key())
        .with_column(ColumnDef::new("Level", DataType::Short).primary_key())
        .with_column(ColumnDef::new(
            "Condition",
            DataType::String { max_len: 255 },
        ))
}

/// Creates official schema for `LaunchCondition` table.
///
/// # Returns
///
/// [`TableSchema`] for `LaunchCondition`.
#[must_use]
pub fn launch_condition_schema() -> TableSchema {
    TableSchema::new("LaunchCondition")
        .with_column(ColumnDef::new("Condition", DataType::String { max_len: 0 }).primary_key())
        .with_column(ColumnDef::new("Description", DataType::String { max_len: 0 }).localizable())
}

/// Creates official schema for `AppSearch` table.
///
/// # Returns
///
/// [`TableSchema`] for `AppSearch`.
#[must_use]
pub fn app_search_schema() -> TableSchema {
    TableSchema::new("AppSearch")
        .with_column(ColumnDef::new("Property", DataType::String { max_len: 72 }).primary_key())
        .with_column(ColumnDef::new("Signature_", DataType::String { max_len: 72 }).primary_key())
}

/// Creates official schema for `CompLocator` table.
///
/// # Returns
///
/// [`TableSchema`] for `CompLocator`.
#[must_use]
pub fn comp_locator_schema() -> TableSchema {
    TableSchema::new("CompLocator")
        .with_column(ColumnDef::new("Signature_", DataType::String { max_len: 72 }).primary_key())
        .with_column(ColumnDef::new(
            "ComponentId",
            DataType::String { max_len: 38 },
        ))
        .with_column(ColumnDef::new("Type", DataType::Short).nullable())
}

/// Creates official schema for `DrLocator` table.
///
/// # Returns
///
/// [`TableSchema`] for `DrLocator`.
#[must_use]
pub fn dr_locator_schema() -> TableSchema {
    TableSchema::new("DrLocator")
        .with_column(ColumnDef::new("Signature_", DataType::String { max_len: 72 }).primary_key())
        .with_column(ColumnDef::new("Parent", DataType::String { max_len: 72 }).nullable())
        .with_column(ColumnDef::new("Path", DataType::String { max_len: 255 }).nullable())
        .with_column(ColumnDef::new("Depth", DataType::Short).nullable())
}

/// Creates official schema for `FileSearch` table.
///
/// # Returns
///
/// [`TableSchema`] for `FileSearch`.
#[must_use]
pub fn file_search_schema() -> TableSchema {
    TableSchema::new("FileSearch")
        .with_column(ColumnDef::new("Property", DataType::String { max_len: 72 }).primary_key())
        .with_column(ColumnDef::new(
            "Signature_",
            DataType::String { max_len: 72 },
        ))
        .with_column(ColumnDef::new("MinVersion", DataType::String { max_len: 72 }).nullable())
        .with_column(ColumnDef::new("MaxVersion", DataType::String { max_len: 72 }).nullable())
        .with_column(ColumnDef::new("MinSize", DataType::Long).nullable())
        .with_column(ColumnDef::new("MaxSize", DataType::Long).nullable())
        .with_column(ColumnDef::new("MinDate", DataType::Long).nullable())
        .with_column(ColumnDef::new("MaxDate", DataType::Long).nullable())
        .with_column(ColumnDef::new("Languages", DataType::String { max_len: 255 }).nullable())
}

/// Creates official schema for `IniLocator` table.
///
/// # Returns
///
/// [`TableSchema`] for `IniLocator`.
#[must_use]
pub fn ini_locator_schema() -> TableSchema {
    TableSchema::new("IniLocator")
        .with_column(ColumnDef::new("Signature_", DataType::String { max_len: 72 }).primary_key())
        .with_column(ColumnDef::new(
            "FileName",
            DataType::String { max_len: 255 },
        ))
        .with_column(ColumnDef::new("Section", DataType::String { max_len: 96 }))
        .with_column(ColumnDef::new("Key", DataType::String { max_len: 128 }))
        .with_column(ColumnDef::new("Field", DataType::Short).nullable())
        .with_column(ColumnDef::new("Type", DataType::Short).nullable())
}

/// Creates official schema for `RegLocator` table.
///
/// # Returns
///
/// [`TableSchema`] for `RegLocator`.
#[must_use]
pub fn reg_locator_schema() -> TableSchema {
    TableSchema::new("RegLocator")
        .with_column(ColumnDef::new("Signature_", DataType::String { max_len: 72 }).primary_key())
        .with_column(ColumnDef::new("Root", DataType::Short))
        .with_column(ColumnDef::new("Key", DataType::String { max_len: 255 }))
        .with_column(ColumnDef::new("Name", DataType::String { max_len: 255 }).nullable())
        .with_column(ColumnDef::new("Type", DataType::Short).nullable())
}

/// Creates official schema for `Signature` table.
///
/// # Returns
///
/// [`TableSchema`] for `Signature`.
#[must_use]
pub fn signature_schema() -> TableSchema {
    TableSchema::new("Signature")
        .with_column(ColumnDef::new("Signature", DataType::String { max_len: 72 }).primary_key())
        .with_column(ColumnDef::new(
            "FileName",
            DataType::String { max_len: 128 },
        ))
        .with_column(ColumnDef::new("MinVersion", DataType::String { max_len: 72 }).nullable())
        .with_column(ColumnDef::new("MaxVersion", DataType::String { max_len: 72 }).nullable())
        .with_column(ColumnDef::new("MinSize", DataType::Long).nullable())
        .with_column(ColumnDef::new("MaxSize", DataType::Long).nullable())
        .with_column(ColumnDef::new("MinDate", DataType::Long).nullable())
        .with_column(ColumnDef::new("MaxDate", DataType::Long).nullable())
        .with_column(ColumnDef::new("Languages", DataType::String { max_len: 255 }).nullable())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper to construct [`RegistryRow`] for testing.
    ///
    /// # Arguments
    ///
    /// * `registry` - Registry key identifier.
    /// * `root` - Root registry hive integer.
    /// * `key` - Registry subkey path.
    /// * `name` - Optional value name.
    /// * `value` - Optional value data.
    /// * `comp` - Component name.
    ///
    /// # Returns
    ///
    /// Vector containing [`RegistryRow`] on success, or empty vector on failure.
    fn make_registry_rows(
        registry: &str,
        root: i16,
        key: &str,
        name: Option<&str>,
        value: Option<&str>,
        comp: &str,
    ) -> Vec<RegistryRow> {
        let Ok(component) = ComponentName::new(comp) else {
            return Vec::new();
        };
        vec![RegistryRow {
            registry: registry.to_string(),
            root,
            key: key.to_string(),
            name: name.map(ToString::to_string),
            value: value.map(ToString::to_string),
            component,
        }]
    }

    /// Helper to construct [`EnvironmentRow`] for testing.
    ///
    /// # Arguments
    ///
    /// * `env` - Environment identifier.
    /// * `name` - Environment variable name.
    /// * `value` - Environment variable value.
    /// * `comp` - Component name.
    ///
    /// # Returns
    ///
    /// Vector containing [`EnvironmentRow`] on success, or empty vector on failure.
    fn make_environment_rows(
        env: &str,
        name: &str,
        value: &str,
        comp: &str,
    ) -> Vec<EnvironmentRow> {
        let Ok(component) = ComponentName::new(comp) else {
            return Vec::new();
        };
        vec![EnvironmentRow {
            environment: env.to_string(),
            name: name.to_string(),
            value: value.to_string(),
            component,
        }]
    }

    /// Tests serialization, deserialization, and error handling for [`RegistryRow`].
    #[test]
    fn test_registry_row_roundtrip() {
        assert!(make_registry_rows("Reg1", 0, "k", None, None, "").is_empty());

        for row in make_registry_rows(
            "Reg1",
            registry_root::HKLM,
            r"Software\Acme",
            Some("InstallPath"),
            Some("[INSTALLDIR]"),
            "Comp1",
        ) {
            let rec = row.to_record();
            assert_eq!(rec.len(), 6);
            let parsed = RegistryRow::from_record(&rec);
            assert_eq!(parsed, Ok(row));
        }

        // Test with empty/default fields
        for minimal_row in make_registry_rows("RegMin", 0, "", None, None, "CompMin") {
            let min_rec = Record::with_fields(vec![
                FieldValue::String("RegMin".to_string()),
                FieldValue::Null, // non-Short root fallback
                FieldValue::Null, // non-String key fallback
                FieldValue::Null, // non-String name fallback
                FieldValue::Null, // non-String value fallback
                FieldValue::String("CompMin".to_string()),
            ]);
            assert_eq!(RegistryRow::from_record(&min_rec), Ok(minimal_row));
        }

        // Test with empty string name and value
        let empty_str_rec = Record::with_fields(vec![
            FieldValue::String("RegMin".to_string()),
            FieldValue::Short(1),
            FieldValue::String("Key".to_string()),
            FieldValue::String(String::new()),
            FieldValue::String(String::new()),
            FieldValue::String("CompMin".to_string()),
        ]);
        for expected_empty in make_registry_rows("RegMin", 1, "Key", None, None, "CompMin") {
            assert_eq!(RegistryRow::from_record(&empty_str_rec), Ok(expected_empty));
        }

        // Error cases
        assert!(RegistryRow::from_record(&Record::new()).is_err());

        // Missing Registry PK (field 0 is Null)
        let bad_rec = Record::with_fields(vec![
            FieldValue::Null,
            FieldValue::Short(1),
            FieldValue::String("Key".to_string()),
            FieldValue::Null,
            FieldValue::Null,
            FieldValue::String("CompMin".to_string()),
        ]);
        assert_eq!(
            RegistryRow::from_record(&bad_rec),
            Err(Error::Validation {
                element: "Registry.Registry".to_string(),
                reason: "missing Registry PK".to_string(),
            })
        );

        // Missing Component_ (field 5 is Null)
        let bad_rec2 = Record::with_fields(vec![
            FieldValue::String("RegMin".to_string()),
            FieldValue::Short(1),
            FieldValue::String("Key".to_string()),
            FieldValue::Null,
            FieldValue::Null,
            FieldValue::Null,
        ]);
        assert_eq!(
            RegistryRow::from_record(&bad_rec2),
            Err(Error::Validation {
                element: "Registry.Component_".to_string(),
                reason: "missing Component_".to_string(),
            })
        );

        // Invalid ComponentName (empty string component)
        let bad_comp_rec = Record::with_fields(vec![
            FieldValue::String("RegMin".to_string()),
            FieldValue::Short(1),
            FieldValue::String("Key".to_string()),
            FieldValue::Null,
            FieldValue::Null,
            FieldValue::String(String::new()),
        ]);
        assert!(RegistryRow::from_record(&bad_comp_rec).is_err());
    }

    /// Tests serialization, deserialization, and error handling for [`EnvironmentRow`].
    #[test]
    fn test_environment_row_roundtrip() {
        assert!(make_environment_rows("Env1", "n", "v", "").is_empty());

        for row in make_environment_rows("Env1", "=PATH", "[INSTALLDIR]", "Comp1") {
            let rec = row.to_record();
            assert_eq!(rec.len(), 4);
            let parsed = EnvironmentRow::from_record(&rec);
            assert_eq!(parsed, Ok(row));
        }

        assert!(EnvironmentRow::from_record(&Record::new()).is_err());

        // Field 0 not string
        let bad_rec = Record::with_fields(vec![
            FieldValue::Null,
            FieldValue::String("=PATH".to_string()),
            FieldValue::String("[INSTALLDIR]".to_string()),
            FieldValue::String("Comp1".to_string()),
        ]);
        assert_eq!(
            EnvironmentRow::from_record(&bad_rec),
            Err(Error::Validation {
                element: "Environment.Environment".to_string(),
                reason: "missing Environment PK".to_string(),
            })
        );

        // Field 1 not string
        let bad_rec1 = Record::with_fields(vec![
            FieldValue::String("Env1".to_string()),
            FieldValue::Null,
            FieldValue::String("[INSTALLDIR]".to_string()),
            FieldValue::String("Comp1".to_string()),
        ]);
        assert_eq!(
            EnvironmentRow::from_record(&bad_rec1),
            Err(Error::Validation {
                element: "Environment.Name".to_string(),
                reason: "missing Name".to_string(),
            })
        );

        // Field 2 not string
        let bad_rec2 = Record::with_fields(vec![
            FieldValue::String("Env1".to_string()),
            FieldValue::String("=PATH".to_string()),
            FieldValue::Null,
            FieldValue::String("Comp1".to_string()),
        ]);
        assert_eq!(
            EnvironmentRow::from_record(&bad_rec2),
            Err(Error::Validation {
                element: "Environment.Value".to_string(),
                reason: "missing Value".to_string(),
            })
        );

        // Field 3 not string
        let bad_rec3 = Record::with_fields(vec![
            FieldValue::String("Env1".to_string()),
            FieldValue::String("=PATH".to_string()),
            FieldValue::String("[INSTALLDIR]".to_string()),
            FieldValue::Null,
        ]);
        assert_eq!(
            EnvironmentRow::from_record(&bad_rec3),
            Err(Error::Validation {
                element: "Environment.Component_".to_string(),
                reason: "missing Component_".to_string(),
            })
        );

        // Invalid ComponentName (empty string component)
        let bad_comp_rec = Record::with_fields(vec![
            FieldValue::String("Env1".to_string()),
            FieldValue::String("=PATH".to_string()),
            FieldValue::String("[INSTALLDIR]".to_string()),
            FieldValue::String(String::new()),
        ]);
        assert!(EnvironmentRow::from_record(&bad_comp_rec).is_err());
    }

    /// Tests schemas for configuration tables.
    #[test]
    fn test_config_schemas() {
        assert_eq!(registry_schema().name, "Registry");
        assert_eq!(remove_registry_schema().name, "RemoveRegistry");
        assert_eq!(environment_schema().name, "Environment");
        assert_eq!(shortcut_schema().name, "Shortcut");
        assert_eq!(icon_schema().name, "Icon");
        assert_eq!(service_install_schema().name, "ServiceInstall");
        assert_eq!(service_control_schema().name, "ServiceControl");
        assert_eq!(upgrade_schema().name, "Upgrade");
        assert_eq!(condition_schema().name, "Condition");
        assert_eq!(launch_condition_schema().name, "LaunchCondition");
        assert_eq!(app_search_schema().name, "AppSearch");
        assert_eq!(comp_locator_schema().name, "CompLocator");
        assert_eq!(dr_locator_schema().name, "DrLocator");
        assert_eq!(file_search_schema().name, "FileSearch");
        assert_eq!(ini_locator_schema().name, "IniLocator");
        assert_eq!(reg_locator_schema().name, "RegLocator");
        assert_eq!(signature_schema().name, "Signature");
    }
}

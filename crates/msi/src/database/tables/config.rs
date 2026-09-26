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

/// Creates official schema for `MsiServiceConfig` table.
///
/// # Returns
///
/// [`TableSchema`] for `MsiServiceConfig`.
#[must_use]
pub fn msi_service_config_schema() -> TableSchema {
    TableSchema::new("MsiServiceConfig")
        .with_column(
            ColumnDef::new("MsiServiceConfig", DataType::String { max_len: 72 }).primary_key(),
        )
        .with_column(ColumnDef::new("Name", DataType::String { max_len: 255 }))
        .with_column(ColumnDef::new("Event", DataType::Long))
        .with_column(ColumnDef::new("ConfigType", DataType::Long))
        .with_column(ColumnDef::new("Argument", DataType::String { max_len: 255 }).nullable())
        .with_column(ColumnDef::new(
            "Component_",
            DataType::String { max_len: 72 },
        ))
}

/// Row in the `MsiServiceConfig` table.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MsiServiceConfigRow {
    /// Unique primary key for the service configuration record (max 72 characters).
    pub msi_service_config: String,
    /// Name of the service to configure.
    pub name: String,
    /// Configuration event flags (e.g. install, uninstall).
    pub event: i32,
    /// Service configuration type code (e.g. failure actions or delayed start).
    pub config_type: i32,
    /// Optional formatted argument string.
    pub argument: Option<String>,
    /// Foreign key into Component table.
    pub component: ComponentName,
}

impl MsiServiceConfigRow {
    /// Creates a new [`MsiServiceConfigRow`] with validation.
    ///
    /// # Arguments
    ///
    /// * `msi_service_config` - Primary key identifier (max 72 chars).
    /// * `name` - Service name.
    /// * `event` - Event flags bitmask.
    /// * `config_type` - Configuration type integer.
    /// * `argument` - Optional configuration argument.
    /// * `component` - Controlling component foreign key.
    ///
    /// # Returns
    ///
    /// A validated [`MsiServiceConfigRow`].
    ///
    /// # Errors
    ///
    /// Returns [`Error::Validation`] on invalid input.
    pub fn new(
        msi_service_config: impl Into<String>,
        name: impl Into<String>,
        event: i32,
        config_type: i32,
        argument: Option<String>,
        component: ComponentName,
    ) -> Result<Self> {
        Self::new_impl(
            msi_service_config.into(),
            name.into(),
            event,
            config_type,
            argument,
            component,
        )
    }

    /// Validates concrete parameters and creates a new [`MsiServiceConfigRow`].
    fn new_impl(
        msi_service_config: String,
        name: String,
        event: i32,
        config_type: i32,
        argument: Option<String>,
        component: ComponentName,
    ) -> Result<Self> {
        if msi_service_config.is_empty() {
            return Err(Error::Validation {
                element: "MsiServiceConfig.MsiServiceConfig".to_string(),
                reason: "primary key cannot be empty".to_string(),
            });
        }
        if msi_service_config.len() > 72 {
            return Err(Error::Validation {
                element: "MsiServiceConfig.MsiServiceConfig".to_string(),
                reason: format!(
                    "identifier length {} exceeds maximum 72",
                    msi_service_config.len()
                ),
            });
        }
        if name.is_empty() {
            return Err(Error::Validation {
                element: "MsiServiceConfig.Name".to_string(),
                reason: "service name cannot be empty".to_string(),
            });
        }

        Ok(Self {
            msi_service_config,
            name,
            event,
            config_type,
            argument,
            component,
        })
    }

    /// Converts this row into a generic database [`Record`].
    ///
    /// # Returns
    ///
    /// A generic [`Record`].
    #[must_use]
    pub fn to_record(&self) -> Record {
        Record::with_fields(vec![
            FieldValue::String(self.msi_service_config.clone()),
            FieldValue::String(self.name.clone()),
            FieldValue::Long(self.event),
            FieldValue::Long(self.config_type),
            self.argument
                .as_ref()
                .map_or(FieldValue::Null, |a| FieldValue::String(a.clone())),
            FieldValue::String(self.component.as_str().to_string()),
        ])
    }

    /// Parses a [`MsiServiceConfigRow`] from a generic database [`Record`].
    ///
    /// # Arguments
    ///
    /// * `rec` - The database record to parse.
    ///
    /// # Returns
    ///
    /// Parsed [`MsiServiceConfigRow`].
    ///
    /// # Errors
    ///
    /// Returns [`Error::RecordLengthMismatch`] or [`Error::Validation`] on invalid data.
    pub fn from_record(rec: &Record) -> Result<Self> {
        if rec.len() < 6 {
            return Err(Error::RecordLengthMismatch {
                expected: 6,
                actual: rec.len(),
            });
        }

        let msi_service_config = match rec.get(0) {
            Some(FieldValue::String(s)) if !s.is_empty() => s.clone(),
            _ => {
                return Err(Error::Validation {
                    element: "MsiServiceConfig.MsiServiceConfig".to_string(),
                    reason: "missing or empty primary key".to_string(),
                });
            }
        };

        let name = match rec.get(1) {
            Some(FieldValue::String(s)) if !s.is_empty() => s.clone(),
            _ => {
                return Err(Error::Validation {
                    element: "MsiServiceConfig.Name".to_string(),
                    reason: "missing or empty service name".to_string(),
                });
            }
        };

        let event = match rec.get(2) {
            Some(FieldValue::Long(v)) => *v,
            Some(FieldValue::Short(v)) => i32::from(*v),
            _ => 0,
        };

        let config_type = match rec.get(3) {
            Some(FieldValue::Long(v)) => *v,
            Some(FieldValue::Short(v)) => i32::from(*v),
            _ => 0,
        };

        let argument = match rec.get(4) {
            Some(FieldValue::String(s)) if !s.is_empty() => Some(s.clone()),
            _ => None,
        };

        let component = match rec.get(5) {
            Some(FieldValue::String(s)) => ComponentName::new(s.as_str())?,
            _ => {
                return Err(Error::Validation {
                    element: "MsiServiceConfig.Component_".to_string(),
                    reason: "missing or empty component foreign key".to_string(),
                });
            }
        };

        Self::new_impl(
            msi_service_config,
            name,
            event,
            config_type,
            argument,
            component,
        )
    }
}

/// Creates official schema for `ServiceConfig` table.
///
/// # Returns
///
/// [`TableSchema`] for `ServiceConfig`.
#[must_use]
pub fn service_config_schema() -> TableSchema {
    TableSchema::new("ServiceConfig")
        .with_column(ColumnDef::new("ServiceName", DataType::String { max_len: 255 }).primary_key())
        .with_column(ColumnDef::new("Component_", DataType::String { max_len: 72 }).primary_key())
        .with_column(ColumnDef::new("OnInstall", DataType::Short))
        .with_column(ColumnDef::new("OnReinstall", DataType::Short))
        .with_column(ColumnDef::new("OnUninstall", DataType::Short))
        .with_column(ColumnDef::new("FirstFailureActionType", DataType::Long).nullable())
        .with_column(ColumnDef::new("SecondFailureActionType", DataType::Long).nullable())
        .with_column(ColumnDef::new("ThirdFailureActionType", DataType::Long).nullable())
        .with_column(ColumnDef::new("ResetPeriodInDays", DataType::Long).nullable())
        .with_column(ColumnDef::new("RestartServiceDelayInSeconds", DataType::Long).nullable())
        .with_column(
            ColumnDef::new("ProgramCommandLine", DataType::String { max_len: 255 }).nullable(),
        )
        .with_column(ColumnDef::new("RebootMessage", DataType::String { max_len: 255 }).nullable())
}

/// Row in the `ServiceConfig` table.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServiceConfigRow {
    /// Service name to configure (composite primary key).
    pub service_name: String,
    /// Controlling component (composite primary key).
    pub component: ComponentName,
    /// Configure on installation flag.
    pub on_install: i16,
    /// Configure on reinstallation flag.
    pub on_reinstall: i16,
    /// Configure on uninstallation flag.
    pub on_uninstall: i16,
    /// Primary failure action type.
    pub first_failure_action_type: Option<i32>,
    /// Secondary failure action type.
    pub second_failure_action_type: Option<i32>,
    /// Subsequent failure action type.
    pub third_failure_action_type: Option<i32>,
    /// Failure count reset period in days.
    pub reset_period_in_days: Option<i32>,
    /// Delay before restarting service in seconds.
    pub restart_service_delay_in_seconds: Option<i32>,
    /// Custom command line to execute on service failure.
    pub program_command_line: Option<String>,
    /// Broadcast reboot message string.
    pub reboot_message: Option<String>,
}

impl ServiceConfigRow {
    /// Creates a new [`ServiceConfigRow`] with validation.
    ///
    /// # Arguments
    ///
    /// * `service_name` - Target service name.
    /// * `component` - Controlling component foreign key.
    /// * `on_install` - Configure on install flag.
    /// * `on_reinstall` - Configure on reinstall flag.
    /// * `on_uninstall` - Configure on uninstall flag.
    /// * `first_failure_action_type` - Optional first failure action code.
    /// * `second_failure_action_type` - Optional second failure action code.
    /// * `third_failure_action_type` - Optional third failure action code.
    /// * `reset_period_in_days` - Optional failure reset period in days.
    /// * `restart_service_delay_in_seconds` - Optional restart delay in seconds.
    /// * `program_command_line` - Optional failure command line.
    /// * `reboot_message` - Optional reboot broadcast message.
    ///
    /// # Returns
    ///
    /// A validated [`ServiceConfigRow`].
    ///
    /// # Errors
    ///
    /// Returns [`Error::Validation`] on invalid input.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        service_name: impl Into<String>,
        component: ComponentName,
        on_install: i16,
        on_reinstall: i16,
        on_uninstall: i16,
        first_failure_action_type: Option<i32>,
        second_failure_action_type: Option<i32>,
        third_failure_action_type: Option<i32>,
        reset_period_in_days: Option<i32>,
        restart_service_delay_in_seconds: Option<i32>,
        program_command_line: Option<String>,
        reboot_message: Option<String>,
    ) -> Result<Self> {
        Self::new_impl(
            service_name.into(),
            component,
            on_install,
            on_reinstall,
            on_uninstall,
            first_failure_action_type,
            second_failure_action_type,
            third_failure_action_type,
            reset_period_in_days,
            restart_service_delay_in_seconds,
            program_command_line,
            reboot_message,
        )
    }

    /// Validates concrete parameters and creates a new [`ServiceConfigRow`].
    #[allow(clippy::too_many_arguments)]
    fn new_impl(
        service_name: String,
        component: ComponentName,
        on_install: i16,
        on_reinstall: i16,
        on_uninstall: i16,
        first_failure_action_type: Option<i32>,
        second_failure_action_type: Option<i32>,
        third_failure_action_type: Option<i32>,
        reset_period_in_days: Option<i32>,
        restart_service_delay_in_seconds: Option<i32>,
        program_command_line: Option<String>,
        reboot_message: Option<String>,
    ) -> Result<Self> {
        if service_name.is_empty() {
            return Err(Error::Validation {
                element: "ServiceConfig.ServiceName".to_string(),
                reason: "service name cannot be empty".to_string(),
            });
        }

        Ok(Self {
            service_name,
            component,
            on_install,
            on_reinstall,
            on_uninstall,
            first_failure_action_type,
            second_failure_action_type,
            third_failure_action_type,
            reset_period_in_days,
            restart_service_delay_in_seconds,
            program_command_line,
            reboot_message,
        })
    }

    /// Converts this row into a generic database [`Record`].
    ///
    /// # Returns
    ///
    /// A generic [`Record`].
    #[must_use]
    pub fn to_record(&self) -> Record {
        Record::with_fields(vec![
            FieldValue::String(self.service_name.clone()),
            FieldValue::String(self.component.as_str().to_string()),
            FieldValue::Short(self.on_install),
            FieldValue::Short(self.on_reinstall),
            FieldValue::Short(self.on_uninstall),
            self.first_failure_action_type
                .map_or(FieldValue::Null, FieldValue::Long),
            self.second_failure_action_type
                .map_or(FieldValue::Null, FieldValue::Long),
            self.third_failure_action_type
                .map_or(FieldValue::Null, FieldValue::Long),
            self.reset_period_in_days
                .map_or(FieldValue::Null, FieldValue::Long),
            self.restart_service_delay_in_seconds
                .map_or(FieldValue::Null, FieldValue::Long),
            self.program_command_line
                .as_ref()
                .map_or(FieldValue::Null, |p| FieldValue::String(p.clone())),
            self.reboot_message
                .as_ref()
                .map_or(FieldValue::Null, |r| FieldValue::String(r.clone())),
        ])
    }

    /// Parses a [`ServiceConfigRow`] from a generic database [`Record`].
    ///
    /// # Arguments
    ///
    /// * `rec` - The database record to parse.
    ///
    /// # Returns
    ///
    /// Parsed [`ServiceConfigRow`].
    ///
    /// # Errors
    ///
    /// Returns [`Error::RecordLengthMismatch`] or [`Error::Validation`] on invalid data.
    pub fn from_record(rec: &Record) -> Result<Self> {
        if rec.len() < 12 {
            return Err(Error::RecordLengthMismatch {
                expected: 12,
                actual: rec.len(),
            });
        }

        let service_name = match rec.get(0) {
            Some(FieldValue::String(s)) if !s.is_empty() => s.clone(),
            _ => {
                return Err(Error::Validation {
                    element: "ServiceConfig.ServiceName".to_string(),
                    reason: "missing or empty service name".to_string(),
                });
            }
        };

        let component = match rec.get(1) {
            Some(FieldValue::String(s)) => ComponentName::new(s.as_str())?,
            _ => {
                return Err(Error::Validation {
                    element: "ServiceConfig.Component_".to_string(),
                    reason: "missing or empty component foreign key".to_string(),
                });
            }
        };

        let on_install = match rec.get(2) {
            Some(FieldValue::Short(v)) => *v,
            Some(FieldValue::Long(v)) => i16::try_from(*v).unwrap_or(0),
            _ => 0,
        };

        let on_reinstall = match rec.get(3) {
            Some(FieldValue::Short(v)) => *v,
            Some(FieldValue::Long(v)) => i16::try_from(*v).unwrap_or(0),
            _ => 0,
        };

        let on_uninstall = match rec.get(4) {
            Some(FieldValue::Short(v)) => *v,
            Some(FieldValue::Long(v)) => i16::try_from(*v).unwrap_or(0),
            _ => 0,
        };

        let first_failure_action_type = match rec.get(5) {
            Some(FieldValue::Long(v)) => Some(*v),
            Some(FieldValue::Short(v)) => Some(i32::from(*v)),
            _ => None,
        };

        let second_failure_action_type = match rec.get(6) {
            Some(FieldValue::Long(v)) => Some(*v),
            Some(FieldValue::Short(v)) => Some(i32::from(*v)),
            _ => None,
        };

        let third_failure_action_type = match rec.get(7) {
            Some(FieldValue::Long(v)) => Some(*v),
            Some(FieldValue::Short(v)) => Some(i32::from(*v)),
            _ => None,
        };

        let reset_period_in_days = match rec.get(8) {
            Some(FieldValue::Long(v)) => Some(*v),
            Some(FieldValue::Short(v)) => Some(i32::from(*v)),
            _ => None,
        };

        let restart_service_delay_in_seconds = match rec.get(9) {
            Some(FieldValue::Long(v)) => Some(*v),
            Some(FieldValue::Short(v)) => Some(i32::from(*v)),
            _ => None,
        };

        let program_command_line = match rec.get(10) {
            Some(FieldValue::String(s)) if !s.is_empty() => Some(s.clone()),
            _ => None,
        };

        let reboot_message = match rec.get(11) {
            Some(FieldValue::String(s)) if !s.is_empty() => Some(s.clone()),
            _ => None,
        };

        Self::new_impl(
            service_name,
            component,
            on_install,
            on_reinstall,
            on_uninstall,
            first_failure_action_type,
            second_failure_action_type,
            third_failure_action_type,
            reset_period_in_days,
            restart_service_delay_in_seconds,
            program_command_line,
            reboot_message,
        )
    }
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
        .with_column(
            ColumnDef::new("VersionMin", DataType::String { max_len: 20 })
                .nullable()
                .primary_key(),
        )
        .with_column(
            ColumnDef::new("VersionMax", DataType::String { max_len: 20 })
                .nullable()
                .primary_key(),
        )
        .with_column(
            ColumnDef::new("Language", DataType::String { max_len: 255 })
                .nullable()
                .primary_key(),
        )
        .with_column(ColumnDef::new("Attributes", DataType::Long).primary_key())
        .with_column(ColumnDef::new("Remove", DataType::String { max_len: 255 }).nullable())
        .with_column(
            ColumnDef::new("ActionProperty", DataType::String { max_len: 72 }).primary_key(),
        )
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
        assert_eq!(msi_service_config_schema().name, "MsiServiceConfig");
        assert_eq!(service_config_schema().name, "ServiceConfig");
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

    /// Helper testing `MsiServiceConfig` pipeline stages with success and failure paths.
    ///
    /// # Arguments
    ///
    /// * `stage_to_fail` - 0 for success, 1..=4 to induce a specific failure stage.
    ///
    /// # Returns
    ///
    /// `Result<()>` indicating pipeline success.
    ///
    /// # Errors
    ///
    /// Returns error when induced failure occurs.
    fn check_msi_service_config_pipeline(stage_to_fail: u8) -> Result<()> {
        let comp = if stage_to_fail == 1 {
            ComponentName::new("")?
        } else {
            ComponentName::new("C_MySQL")?
        };

        let row = if stage_to_fail == 2 {
            MsiServiceConfigRow::new("", "LibScript_MySQL", 1, 2, None, comp.clone())?
        } else {
            MsiServiceConfigRow::new(
                "CfgMySQL",
                "LibScript_MySQL",
                1,
                2,
                Some("args".to_string()),
                comp.clone(),
            )?
        };

        let rec = if stage_to_fail == 3 {
            Record::with_fields(vec![FieldValue::String("short".to_string())])
        } else {
            row.to_record()
        };
        let parsed = MsiServiceConfigRow::from_record(&rec)?;
        assert_eq!(parsed, row);

        let row_no_arg = if stage_to_fail == 4 {
            MsiServiceConfigRow::new("", "LibScript_MySQL", 0, 0, None, comp)?
        } else {
            MsiServiceConfigRow::new("CfgNoArg", "LibScript_MySQL", 0, 0, None, comp)?
        };
        let rec_no_arg = row_no_arg.to_record();
        assert_eq!(rec_no_arg.get(4), Some(&FieldValue::Null));

        Ok(())
    }

    /// Tests `MsiServiceConfig` schema and typed row conversions.
    #[test]
    fn test_msi_service_config_row_and_schema() -> Result<()> {
        let schema = msi_service_config_schema();
        assert_eq!(schema.name, "MsiServiceConfig");
        assert_eq!(schema.columns.len(), 6);
        assert_eq!(schema.primary_keys(), vec!["MsiServiceConfig"]);

        check_msi_service_config_pipeline(0)?;
        assert!(check_msi_service_config_pipeline(1).is_err());
        assert!(check_msi_service_config_pipeline(2).is_err());
        assert!(check_msi_service_config_pipeline(3).is_err());
        assert!(check_msi_service_config_pipeline(4).is_err());

        // from_record with Short event, Short config_type, and empty string argument
        let rec_short_and_emp = Record::with_fields(vec![
            FieldValue::String("CfgShort".to_string()),
            FieldValue::String("LibScript_MySQL".to_string()),
            FieldValue::Short(1),
            FieldValue::Short(2),
            FieldValue::String(String::new()),
            FieldValue::String("C_MySQL".to_string()),
        ]);
        let parsed_short_and_emp = MsiServiceConfigRow::from_record(&rec_short_and_emp);
        assert_eq!(parsed_short_and_emp.as_ref().map(|p| p.event), Ok(1));
        assert_eq!(parsed_short_and_emp.as_ref().map(|p| p.config_type), Ok(2));
        assert_eq!(
            parsed_short_and_emp.as_ref().map(|p| p.argument.as_deref()),
            Ok(None)
        );

        // from_record with Null event, Null config_type, and Null argument
        let mut rec_nulls = rec_short_and_emp;
        rec_nulls.set(2, FieldValue::Null);
        rec_nulls.set(3, FieldValue::Null);
        rec_nulls.set(4, FieldValue::Null);
        let parsed_nulls = MsiServiceConfigRow::from_record(&rec_nulls);
        assert_eq!(parsed_nulls.as_ref().map(|p| p.event), Ok(0));
        assert_eq!(parsed_nulls.as_ref().map(|p| p.config_type), Ok(0));
        assert_eq!(
            parsed_nulls.as_ref().map(|p| p.argument.as_deref()),
            Ok(None)
        );

        let comp = ComponentName::new("C_MySQL")?;

        // Validation errors
        assert!(MsiServiceConfigRow::new("", "svc", 0, 0, None, comp.clone()).is_err());
        assert!(MsiServiceConfigRow::new("A".repeat(73), "svc", 0, 0, None, comp.clone()).is_err());
        assert!(MsiServiceConfigRow::new("id", "", 0, 0, None, comp).is_err());

        let short_rec = Record::with_fields(vec![FieldValue::String("A".to_string())]);
        assert!(MsiServiceConfigRow::from_record(&short_rec).is_err());

        let empty_pk = Record::with_fields(vec![
            FieldValue::Null,
            FieldValue::String("svc".to_string()),
            FieldValue::Long(0),
            FieldValue::Long(0),
            FieldValue::Null,
            FieldValue::String("c".to_string()),
        ]);
        assert!(MsiServiceConfigRow::from_record(&empty_pk).is_err());

        let empty_name = Record::with_fields(vec![
            FieldValue::String("id".to_string()),
            FieldValue::Null,
            FieldValue::Long(0),
            FieldValue::Long(0),
            FieldValue::Null,
            FieldValue::String("c".to_string()),
        ]);
        assert!(MsiServiceConfigRow::from_record(&empty_name).is_err());

        let empty_comp = Record::with_fields(vec![
            FieldValue::String("id".to_string()),
            FieldValue::String("svc".to_string()),
            FieldValue::Long(0),
            FieldValue::Long(0),
            FieldValue::Null,
            FieldValue::Null,
        ]);
        assert!(MsiServiceConfigRow::from_record(&empty_comp).is_err());

        let mut empty_str_pk = empty_pk;
        empty_str_pk.set(0, FieldValue::String(String::new()));
        assert!(MsiServiceConfigRow::from_record(&empty_str_pk).is_err());

        let mut empty_str_name = empty_name;
        empty_str_name.set(1, FieldValue::String(String::new()));
        assert!(MsiServiceConfigRow::from_record(&empty_str_name).is_err());

        let mut empty_str_comp = empty_comp;
        empty_str_comp.set(5, FieldValue::String(String::new()));
        assert!(MsiServiceConfigRow::from_record(&empty_str_comp).is_err());

        Ok(())
    }

    /// Helper testing `ServiceConfig` pipeline stages with success and failure paths.
    ///
    /// # Arguments
    ///
    /// * `stage_to_fail` - 0 for success, 1..=4 to induce a specific failure stage.
    ///
    /// # Returns
    ///
    /// `Result<()>` indicating pipeline success.
    ///
    /// # Errors
    ///
    /// Returns error when induced failure occurs.
    fn check_service_config_pipeline(stage_to_fail: u8) -> Result<()> {
        let comp = if stage_to_fail == 1 {
            ComponentName::new("")?
        } else {
            ComponentName::new("C_MySQL")?
        };

        let row = if stage_to_fail == 2 {
            ServiceConfigRow::new(
                "",
                comp.clone(),
                1,
                0,
                0,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
            )?
        } else {
            ServiceConfigRow::new(
                "LibScript_MySQL",
                comp.clone(),
                1,
                0,
                0,
                Some(1),
                Some(1),
                Some(0),
                Some(1),
                Some(60),
                Some("reboot.cmd".to_string()),
                Some("Service failed".to_string()),
            )?
        };

        let rec = if stage_to_fail == 3 {
            Record::with_fields(vec![FieldValue::String("short".to_string())])
        } else {
            row.to_record()
        };
        let parsed = ServiceConfigRow::from_record(&rec)?;
        assert_eq!(parsed, row);

        let row_min = if stage_to_fail == 4 {
            ServiceConfigRow::new("", comp, 0, 0, 0, None, None, None, None, None, None, None)?
        } else {
            ServiceConfigRow::new(
                "MinService",
                comp,
                0,
                0,
                0,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
            )?
        };
        let rec_min = row_min.to_record();
        for col in 5..=11 {
            assert_eq!(rec_min.get(col), Some(&FieldValue::Null));
        }
        let parsed_min = ServiceConfigRow::from_record(&rec_min)?;
        assert_eq!(parsed_min, row_min);

        Ok(())
    }

    /// Tests `ServiceConfig` schema and typed row conversions.
    #[test]
    #[allow(clippy::too_many_lines)]
    fn test_service_config_row_and_schema() -> Result<()> {
        let schema = service_config_schema();
        assert_eq!(schema.name, "ServiceConfig");
        assert_eq!(schema.columns.len(), 12);
        assert_eq!(schema.primary_keys(), vec!["ServiceName", "Component_"]);

        check_service_config_pipeline(0)?;
        assert!(check_service_config_pipeline(1).is_err());
        assert!(check_service_config_pipeline(2).is_err());
        assert!(check_service_config_pipeline(3).is_err());
        assert!(check_service_config_pipeline(4).is_err());

        // from_record with Long for install flags, Short for action types/delays, and empty strings for text
        let rec_long_flags = Record::with_fields(vec![
            FieldValue::String("SvcLong".to_string()),
            FieldValue::String("C_MySQL".to_string()),
            FieldValue::Long(1),
            FieldValue::Long(2),
            FieldValue::Long(3),
            FieldValue::Short(10),
            FieldValue::Short(20),
            FieldValue::Short(30),
            FieldValue::Short(40),
            FieldValue::Short(50),
            FieldValue::String(String::new()),
            FieldValue::String(String::new()),
        ]);
        let parsed_long = ServiceConfigRow::from_record(&rec_long_flags);
        assert_eq!(parsed_long.as_ref().map(|p| p.on_install), Ok(1));
        assert_eq!(parsed_long.as_ref().map(|p| p.on_reinstall), Ok(2));
        assert_eq!(parsed_long.as_ref().map(|p| p.on_uninstall), Ok(3));
        assert_eq!(
            parsed_long.as_ref().map(|p| p.first_failure_action_type),
            Ok(Some(10))
        );
        assert_eq!(
            parsed_long.as_ref().map(|p| p.second_failure_action_type),
            Ok(Some(20))
        );
        assert_eq!(
            parsed_long.as_ref().map(|p| p.third_failure_action_type),
            Ok(Some(30))
        );
        assert_eq!(
            parsed_long.as_ref().map(|p| p.reset_period_in_days),
            Ok(Some(40))
        );
        assert_eq!(
            parsed_long
                .as_ref()
                .map(|p| p.restart_service_delay_in_seconds),
            Ok(Some(50))
        );
        assert_eq!(
            parsed_long
                .as_ref()
                .map(|p| p.program_command_line.as_deref()),
            Ok(None)
        );
        assert_eq!(
            parsed_long.as_ref().map(|p| p.reboot_message.as_deref()),
            Ok(None)
        );

        // from_record with Null install flags
        let mut rec_null_flags = rec_long_flags;
        rec_null_flags.set(2, FieldValue::Null);
        rec_null_flags.set(3, FieldValue::Null);
        rec_null_flags.set(4, FieldValue::Null);
        let parsed_null_flags = ServiceConfigRow::from_record(&rec_null_flags);
        assert_eq!(parsed_null_flags.as_ref().map(|p| p.on_install), Ok(0));
        assert_eq!(parsed_null_flags.as_ref().map(|p| p.on_reinstall), Ok(0));
        assert_eq!(parsed_null_flags.as_ref().map(|p| p.on_uninstall), Ok(0));

        let comp = ComponentName::new("C_MySQL")?;

        // Validation errors
        assert!(
            ServiceConfigRow::new("", comp, 0, 0, 0, None, None, None, None, None, None, None)
                .is_err()
        );

        let short_rec = Record::with_fields(vec![FieldValue::String("A".to_string())]);
        assert!(ServiceConfigRow::from_record(&short_rec).is_err());

        let empty_name = Record::with_fields(vec![
            FieldValue::Null,
            FieldValue::String("c".to_string()),
            FieldValue::Short(0),
            FieldValue::Short(0),
            FieldValue::Short(0),
            FieldValue::Null,
            FieldValue::Null,
            FieldValue::Null,
            FieldValue::Null,
            FieldValue::Null,
            FieldValue::Null,
            FieldValue::Null,
        ]);
        assert!(ServiceConfigRow::from_record(&empty_name).is_err());

        let empty_comp = Record::with_fields(vec![
            FieldValue::String("svc".to_string()),
            FieldValue::Null,
            FieldValue::Short(0),
            FieldValue::Short(0),
            FieldValue::Short(0),
            FieldValue::Null,
            FieldValue::Null,
            FieldValue::Null,
            FieldValue::Null,
            FieldValue::Null,
            FieldValue::Null,
            FieldValue::Null,
        ]);
        assert!(ServiceConfigRow::from_record(&empty_comp).is_err());

        let mut empty_str_name = empty_name;
        empty_str_name.set(0, FieldValue::String(String::new()));
        assert!(ServiceConfigRow::from_record(&empty_str_name).is_err());

        let mut empty_str_comp = empty_comp;
        empty_str_comp.set(1, FieldValue::String(String::new()));
        assert!(ServiceConfigRow::from_record(&empty_str_comp).is_err());

        Ok(())
    }
}

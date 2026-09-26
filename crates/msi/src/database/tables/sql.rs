//! `WiX` SQL Extension database table schemas and typed rows (`WixSqlExtension`).
//!
//! Provides relational schemas and typed representations for declarative database schema provisioning:
//! - `SqlDatabase`
//! - `SqlString`
//! - `SqlScript`

use crate::database::catalogs::TableSchema;
use crate::database::column::{ColumnDef, DataType};
use crate::database::tables::record::{FieldValue, Record};
use crate::database::tables::types::ComponentName;
use crate::error::{Error, Result};

/// Creates official schema for `SqlDatabase` table.
///
/// Columns:
/// - `SqlDatabase`: Primary key identifier (`DataType::String { max_len: 72 }`)
/// - `Server`: Database host or server name (`DataType::String { max_len: 255 }`)
/// - `Instance`: Database instance name (`DataType::String { max_len: 255 }`, nullable)
/// - `Database`: Database catalog/schema name (`DataType::String { max_len: 255 }`)
/// - `Component_`: Foreign key into `Component` table (`DataType::String { max_len: 72 }`)
/// - `User_`: Optional foreign key or property for credentials (`DataType::String { max_len: 72 }`, nullable)
/// - `Attributes`: Bitmask integer attributes (`DataType::Long`)
///
/// # Returns
///
/// The [`TableSchema`] for `SqlDatabase`.
#[must_use]
pub fn sql_database_schema() -> TableSchema {
    TableSchema::new("SqlDatabase")
        .with_column(ColumnDef::new("SqlDatabase", DataType::String { max_len: 72 }).primary_key())
        .with_column(ColumnDef::new("Server", DataType::String { max_len: 255 }))
        .with_column(ColumnDef::new("Instance", DataType::String { max_len: 255 }).nullable())
        .with_column(ColumnDef::new(
            "Database",
            DataType::String { max_len: 255 },
        ))
        .with_column(ColumnDef::new(
            "Component_",
            DataType::String { max_len: 72 },
        ))
        .with_column(ColumnDef::new("User_", DataType::String { max_len: 72 }).nullable())
        .with_column(ColumnDef::new("Attributes", DataType::Long))
}

/// Typed representation of a row in the `SqlDatabase` table.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SqlDatabaseRow {
    /// Unique identifier for the database connection (primary key, max 72 characters).
    pub sql_database: String,
    /// Host or IP address of the database server (e.g. `127.0.0.1` or `[PROP_MYSQL_HOST]`).
    pub server: String,
    /// Optional instance name.
    pub instance: Option<String>,
    /// Target database name (e.g. `openedx` or `wordpress`).
    pub database: String,
    /// Controlling component for installation lifecycle.
    pub component: ComponentName,
    /// Optional user/credential identifier or property reference.
    pub user: Option<String>,
    /// Bitmask configuration attributes (e.g. create on install, drop on uninstall).
    pub attributes: i32,
}

impl SqlDatabaseRow {
    /// Creates a new [`SqlDatabaseRow`] with validation.
    ///
    /// # Arguments
    ///
    /// * `sql_database` - Primary key identifier (max 72 chars).
    /// * `server` - Server name or host string.
    /// * `instance` - Optional instance string.
    /// * `database` - Database name string.
    /// * `component` - Component name foreign key.
    /// * `user` - Optional user identifier.
    /// * `attributes` - Attributes integer bitmask.
    ///
    /// # Returns
    ///
    /// A validated [`SqlDatabaseRow`].
    ///
    /// # Errors
    ///
    /// Returns [`Error::Validation`] on invalid or empty required fields.
    pub fn new(
        sql_database: impl Into<String>,
        server: impl Into<String>,
        instance: Option<String>,
        database: impl Into<String>,
        component: ComponentName,
        user: Option<String>,
        attributes: i32,
    ) -> Result<Self> {
        Self::new_impl(
            sql_database.into(),
            server.into(),
            instance,
            database.into(),
            component,
            user,
            attributes,
        )
    }

    /// Validates concrete parameters and creates a new [`SqlDatabaseRow`].
    fn new_impl(
        sql_database: String,
        server: String,
        instance: Option<String>,
        database: String,
        component: ComponentName,
        user: Option<String>,
        attributes: i32,
    ) -> Result<Self> {
        if sql_database.is_empty() {
            return Err(Error::Validation {
                element: "SqlDatabase.SqlDatabase".to_string(),
                reason: "primary key cannot be empty".to_string(),
            });
        }
        if sql_database.len() > 72 {
            return Err(Error::Validation {
                element: "SqlDatabase.SqlDatabase".to_string(),
                reason: format!(
                    "identifier length {} exceeds maximum 72",
                    sql_database.len()
                ),
            });
        }
        if server.is_empty() {
            return Err(Error::Validation {
                element: "SqlDatabase.Server".to_string(),
                reason: "server cannot be empty".to_string(),
            });
        }
        if database.is_empty() {
            return Err(Error::Validation {
                element: "SqlDatabase.Database".to_string(),
                reason: "database name cannot be empty".to_string(),
            });
        }

        Ok(Self {
            sql_database,
            server,
            instance,
            database,
            component,
            user,
            attributes,
        })
    }

    /// Converts this row into a generic database [`Record`].
    ///
    /// # Returns
    ///
    /// Generic [`Record`].
    #[must_use]
    pub fn to_record(&self) -> Record {
        Record::with_fields(vec![
            FieldValue::String(self.sql_database.clone()),
            FieldValue::String(self.server.clone()),
            self.instance
                .as_ref()
                .map_or(FieldValue::Null, |i| FieldValue::String(i.clone())),
            FieldValue::String(self.database.clone()),
            FieldValue::String(self.component.as_str().to_string()),
            self.user
                .as_ref()
                .map_or(FieldValue::Null, |u| FieldValue::String(u.clone())),
            FieldValue::Long(self.attributes),
        ])
    }

    /// Parses a [`SqlDatabaseRow`] from a generic database [`Record`].
    ///
    /// # Arguments
    ///
    /// * `rec` - The database record to parse.
    ///
    /// # Returns
    ///
    /// Parsed [`SqlDatabaseRow`].
    ///
    /// # Errors
    ///
    /// Returns [`Error::RecordLengthMismatch`] or [`Error::Validation`] on invalid data.
    pub fn from_record(rec: &Record) -> Result<Self> {
        if rec.len() < 7 {
            return Err(Error::RecordLengthMismatch {
                expected: 7,
                actual: rec.len(),
            });
        }

        let sql_database = match rec.get(0) {
            Some(FieldValue::String(s)) if !s.is_empty() => s.clone(),
            _ => {
                return Err(Error::Validation {
                    element: "SqlDatabase.SqlDatabase".to_string(),
                    reason: "missing or empty primary key".to_string(),
                });
            }
        };

        let server = match rec.get(1) {
            Some(FieldValue::String(s)) if !s.is_empty() => s.clone(),
            _ => {
                return Err(Error::Validation {
                    element: "SqlDatabase.Server".to_string(),
                    reason: "missing or empty server name".to_string(),
                });
            }
        };

        let instance = match rec.get(2) {
            Some(FieldValue::String(s)) if !s.is_empty() => Some(s.clone()),
            _ => None,
        };

        let database = match rec.get(3) {
            Some(FieldValue::String(s)) if !s.is_empty() => s.clone(),
            _ => {
                return Err(Error::Validation {
                    element: "SqlDatabase.Database".to_string(),
                    reason: "missing or empty database name".to_string(),
                });
            }
        };

        let component = match rec.get(4) {
            Some(FieldValue::String(s)) => ComponentName::new(s.as_str())?,
            _ => {
                return Err(Error::Validation {
                    element: "SqlDatabase.Component_".to_string(),
                    reason: "missing or empty component foreign key".to_string(),
                });
            }
        };

        let user = match rec.get(5) {
            Some(FieldValue::String(s)) if !s.is_empty() => Some(s.clone()),
            _ => None,
        };

        let attributes = match rec.get(6) {
            Some(FieldValue::Long(v)) => *v,
            Some(FieldValue::Short(v)) => i32::from(*v),
            _ => 0,
        };

        Self::new_impl(
            sql_database,
            server,
            instance,
            database,
            component,
            user,
            attributes,
        )
    }
}

/// Creates official schema for `SqlString` table.
///
/// Columns:
/// - `SqlString`: Primary key identifier (`DataType::String { max_len: 72 }`)
/// - `SqlDatabase_`: Foreign key into `SqlDatabase` (`DataType::String { max_len: 72 }`)
/// - `SQL`: SQL statement string (`DataType::String { max_len: 0 }`, formatted)
/// - `User_`: Optional user reference (`DataType::String { max_len: 72 }`, nullable)
/// - `Attributes`: Bitmask integer attributes (`DataType::Long`)
/// - `Sequence`: Optional ordering sequence (`DataType::Long`, nullable)
///
/// # Returns
///
/// The [`TableSchema`] for `SqlString`.
#[must_use]
pub fn sql_string_schema() -> TableSchema {
    TableSchema::new("SqlString")
        .with_column(ColumnDef::new("SqlString", DataType::String { max_len: 72 }).primary_key())
        .with_column(ColumnDef::new(
            "SqlDatabase_",
            DataType::String { max_len: 72 },
        ))
        .with_column(ColumnDef::new("SQL", DataType::String { max_len: 0 }))
        .with_column(ColumnDef::new("User_", DataType::String { max_len: 72 }).nullable())
        .with_column(ColumnDef::new("Attributes", DataType::Long))
        .with_column(ColumnDef::new("Sequence", DataType::Long).nullable())
}

/// Typed representation of a row in the `SqlString` table.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SqlStringRow {
    /// Unique identifier for the SQL statement (primary key, max 72 characters).
    pub sql_string: String,
    /// Foreign key reference to target database in `SqlDatabase` table.
    pub sql_database: String,
    /// SQL query or DDL text to execute.
    pub sql: String,
    /// Optional user/credential identifier.
    pub user: Option<String>,
    /// Execution attributes (e.g. continue on error, rollback transaction).
    pub attributes: i32,
    /// Relative sequence number within the database execution batch.
    pub sequence: Option<i32>,
}

impl SqlStringRow {
    /// Creates a new [`SqlStringRow`] with validation.
    ///
    /// # Arguments
    ///
    /// * `sql_string` - Primary key identifier (max 72 chars).
    /// * `sql_database` - Target database foreign key.
    /// * `sql` - SQL statement string.
    /// * `user` - Optional user identifier.
    /// * `attributes` - Attributes bitmask.
    /// * `sequence` - Optional execution sequence order.
    ///
    /// # Returns
    ///
    /// A validated [`SqlStringRow`].
    ///
    /// # Errors
    ///
    /// Returns [`Error::Validation`] on invalid input.
    pub fn new(
        sql_string: impl Into<String>,
        sql_database: impl Into<String>,
        sql: impl Into<String>,
        user: Option<String>,
        attributes: i32,
        sequence: Option<i32>,
    ) -> Result<Self> {
        Self::new_impl(
            sql_string.into(),
            sql_database.into(),
            sql.into(),
            user,
            attributes,
            sequence,
        )
    }

    /// Validates concrete parameters and creates a new [`SqlStringRow`].
    fn new_impl(
        sql_string: String,
        sql_database: String,
        sql: String,
        user: Option<String>,
        attributes: i32,
        sequence: Option<i32>,
    ) -> Result<Self> {
        if sql_string.is_empty() {
            return Err(Error::Validation {
                element: "SqlString.SqlString".to_string(),
                reason: "primary key cannot be empty".to_string(),
            });
        }
        if sql_string.len() > 72 {
            return Err(Error::Validation {
                element: "SqlString.SqlString".to_string(),
                reason: format!("identifier length {} exceeds maximum 72", sql_string.len()),
            });
        }
        if sql_database.is_empty() {
            return Err(Error::Validation {
                element: "SqlString.SqlDatabase_".to_string(),
                reason: "database reference cannot be empty".to_string(),
            });
        }
        if sql.is_empty() {
            return Err(Error::Validation {
                element: "SqlString.SQL".to_string(),
                reason: "SQL statement text cannot be empty".to_string(),
            });
        }

        Ok(Self {
            sql_string,
            sql_database,
            sql,
            user,
            attributes,
            sequence,
        })
    }

    /// Converts this row into a generic database [`Record`].
    ///
    /// # Returns
    ///
    /// Generic [`Record`].
    #[must_use]
    pub fn to_record(&self) -> Record {
        Record::with_fields(vec![
            FieldValue::String(self.sql_string.clone()),
            FieldValue::String(self.sql_database.clone()),
            FieldValue::String(self.sql.clone()),
            self.user
                .as_ref()
                .map_or(FieldValue::Null, |u| FieldValue::String(u.clone())),
            FieldValue::Long(self.attributes),
            self.sequence.map_or(FieldValue::Null, FieldValue::Long),
        ])
    }

    /// Parses a [`SqlStringRow`] from a generic database [`Record`].
    ///
    /// # Arguments
    ///
    /// * `rec` - The database record to parse.
    ///
    /// # Returns
    ///
    /// Parsed [`SqlStringRow`].
    ///
    /// # Errors
    ///
    /// Returns [`Error::RecordLengthMismatch`] or [`Error::Validation`].
    pub fn from_record(rec: &Record) -> Result<Self> {
        if rec.len() < 6 {
            return Err(Error::RecordLengthMismatch {
                expected: 6,
                actual: rec.len(),
            });
        }

        let sql_string = match rec.get(0) {
            Some(FieldValue::String(s)) if !s.is_empty() => s.clone(),
            _ => {
                return Err(Error::Validation {
                    element: "SqlString.SqlString".to_string(),
                    reason: "missing or empty primary key".to_string(),
                });
            }
        };

        let sql_database = match rec.get(1) {
            Some(FieldValue::String(s)) if !s.is_empty() => s.clone(),
            _ => {
                return Err(Error::Validation {
                    element: "SqlString.SqlDatabase_".to_string(),
                    reason: "missing or empty database foreign key".to_string(),
                });
            }
        };

        let sql = match rec.get(2) {
            Some(FieldValue::String(s)) if !s.is_empty() => s.clone(),
            _ => {
                return Err(Error::Validation {
                    element: "SqlString.SQL".to_string(),
                    reason: "missing or empty SQL text".to_string(),
                });
            }
        };

        let user = match rec.get(3) {
            Some(FieldValue::String(s)) if !s.is_empty() => Some(s.clone()),
            _ => None,
        };

        let attributes = match rec.get(4) {
            Some(FieldValue::Long(v)) => *v,
            Some(FieldValue::Short(v)) => i32::from(*v),
            _ => 0,
        };

        let sequence = match rec.get(5) {
            Some(FieldValue::Long(v)) => Some(*v),
            Some(FieldValue::Short(v)) => Some(i32::from(*v)),
            _ => None,
        };

        Self::new_impl(sql_string, sql_database, sql, user, attributes, sequence)
    }
}

/// Creates official schema for `SqlScript` table.
///
/// Columns:
/// - `SqlScript`: Primary key identifier (`DataType::String { max_len: 72 }`)
/// - `SqlDatabase_`: Foreign key into `SqlDatabase` (`DataType::String { max_len: 72 }`)
/// - `Component_`: Foreign key into `Component` (`DataType::String { max_len: 72 }`)
/// - `ScriptFile`: Path to script file or Binary key (`DataType::String { max_len: 255 }`)
/// - `User_`: Optional user identifier (`DataType::String { max_len: 72 }`, nullable)
/// - `Attributes`: Bitmask integer attributes (`DataType::Long`)
/// - `Sequence`: Optional ordering sequence (`DataType::Long`, nullable)
///
/// # Returns
///
/// The [`TableSchema`] for `SqlScript`.
#[must_use]
pub fn sql_script_schema() -> TableSchema {
    TableSchema::new("SqlScript")
        .with_column(ColumnDef::new("SqlScript", DataType::String { max_len: 72 }).primary_key())
        .with_column(ColumnDef::new(
            "SqlDatabase_",
            DataType::String { max_len: 72 },
        ))
        .with_column(ColumnDef::new(
            "Component_",
            DataType::String { max_len: 72 },
        ))
        .with_column(ColumnDef::new(
            "ScriptFile",
            DataType::String { max_len: 255 },
        ))
        .with_column(ColumnDef::new("User_", DataType::String { max_len: 72 }).nullable())
        .with_column(ColumnDef::new("Attributes", DataType::Long))
        .with_column(ColumnDef::new("Sequence", DataType::Long).nullable())
}

/// Typed representation of a row in the `SqlScript` table.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SqlScriptRow {
    /// Unique identifier for the SQL script action (primary key, max 72 characters).
    pub sql_script: String,
    /// Target database reference.
    pub sql_database: String,
    /// Controlling component for installation lifecycle.
    pub component: ComponentName,
    /// Path to the SQL script file or Binary stream key.
    pub script_file: String,
    /// Optional user reference.
    pub user: Option<String>,
    /// Bitmask configuration attributes.
    pub attributes: i32,
    /// Relative sequence number within execution batch.
    pub sequence: Option<i32>,
}

impl SqlScriptRow {
    /// Creates a new [`SqlScriptRow`] with validation.
    ///
    /// # Arguments
    ///
    /// * `sql_script` - Primary key identifier (max 72 chars).
    /// * `sql_database` - Database foreign key.
    /// * `component` - Component foreign key.
    /// * `script_file` - Script file identifier or path.
    /// * `user` - Optional user identifier.
    /// * `attributes` - Attributes bitmask.
    /// * `sequence` - Optional execution sequence order.
    ///
    /// # Returns
    ///
    /// A validated [`SqlScriptRow`].
    ///
    /// # Errors
    ///
    /// Returns [`Error::Validation`] on invalid input.
    pub fn new(
        sql_script: impl Into<String>,
        sql_database: impl Into<String>,
        component: ComponentName,
        script_file: impl Into<String>,
        user: Option<String>,
        attributes: i32,
        sequence: Option<i32>,
    ) -> Result<Self> {
        Self::new_impl(
            sql_script.into(),
            sql_database.into(),
            component,
            script_file.into(),
            user,
            attributes,
            sequence,
        )
    }

    /// Validates concrete parameters and creates a new [`SqlScriptRow`].
    fn new_impl(
        sql_script: String,
        sql_database: String,
        component: ComponentName,
        script_file: String,
        user: Option<String>,
        attributes: i32,
        sequence: Option<i32>,
    ) -> Result<Self> {
        if sql_script.is_empty() {
            return Err(Error::Validation {
                element: "SqlScript.SqlScript".to_string(),
                reason: "primary key cannot be empty".to_string(),
            });
        }
        if sql_script.len() > 72 {
            return Err(Error::Validation {
                element: "SqlScript.SqlScript".to_string(),
                reason: format!("identifier length {} exceeds maximum 72", sql_script.len()),
            });
        }
        if sql_database.is_empty() {
            return Err(Error::Validation {
                element: "SqlScript.SqlDatabase_".to_string(),
                reason: "database reference cannot be empty".to_string(),
            });
        }
        if script_file.is_empty() {
            return Err(Error::Validation {
                element: "SqlScript.ScriptFile".to_string(),
                reason: "script file path cannot be empty".to_string(),
            });
        }

        Ok(Self {
            sql_script,
            sql_database,
            component,
            script_file,
            user,
            attributes,
            sequence,
        })
    }

    /// Converts this row into a generic database [`Record`].
    ///
    /// # Returns
    ///
    /// Generic [`Record`].
    #[must_use]
    pub fn to_record(&self) -> Record {
        Record::with_fields(vec![
            FieldValue::String(self.sql_script.clone()),
            FieldValue::String(self.sql_database.clone()),
            FieldValue::String(self.component.as_str().to_string()),
            FieldValue::String(self.script_file.clone()),
            self.user
                .as_ref()
                .map_or(FieldValue::Null, |u| FieldValue::String(u.clone())),
            FieldValue::Long(self.attributes),
            self.sequence.map_or(FieldValue::Null, FieldValue::Long),
        ])
    }

    /// Parses a [`SqlScriptRow`] from a generic database [`Record`].
    ///
    /// # Arguments
    ///
    /// * `rec` - The database record to parse.
    ///
    /// # Returns
    ///
    /// Parsed [`SqlScriptRow`].
    ///
    /// # Errors
    ///
    /// Returns [`Error::RecordLengthMismatch`] or [`Error::Validation`].
    pub fn from_record(rec: &Record) -> Result<Self> {
        if rec.len() < 7 {
            return Err(Error::RecordLengthMismatch {
                expected: 7,
                actual: rec.len(),
            });
        }

        let sql_script = match rec.get(0) {
            Some(FieldValue::String(s)) if !s.is_empty() => s.clone(),
            _ => {
                return Err(Error::Validation {
                    element: "SqlScript.SqlScript".to_string(),
                    reason: "missing or empty primary key".to_string(),
                });
            }
        };

        let sql_database = match rec.get(1) {
            Some(FieldValue::String(s)) if !s.is_empty() => s.clone(),
            _ => {
                return Err(Error::Validation {
                    element: "SqlScript.SqlDatabase_".to_string(),
                    reason: "missing or empty database foreign key".to_string(),
                });
            }
        };

        let component = match rec.get(2) {
            Some(FieldValue::String(s)) => ComponentName::new(s.as_str())?,
            _ => {
                return Err(Error::Validation {
                    element: "SqlScript.Component_".to_string(),
                    reason: "missing or empty component foreign key".to_string(),
                });
            }
        };

        let script_file = match rec.get(3) {
            Some(FieldValue::String(s)) if !s.is_empty() => s.clone(),
            _ => {
                return Err(Error::Validation {
                    element: "SqlScript.ScriptFile".to_string(),
                    reason: "missing or empty script file path".to_string(),
                });
            }
        };

        let user = match rec.get(4) {
            Some(FieldValue::String(s)) if !s.is_empty() => Some(s.clone()),
            _ => None,
        };

        let attributes = match rec.get(5) {
            Some(FieldValue::Long(v)) => *v,
            Some(FieldValue::Short(v)) => i32::from(*v),
            _ => 0,
        };

        let sequence = match rec.get(6) {
            Some(FieldValue::Long(v)) => Some(*v),
            Some(FieldValue::Short(v)) => Some(i32::from(*v)),
            _ => None,
        };

        Self::new_impl(
            sql_script,
            sql_database,
            component,
            script_file,
            user,
            attributes,
            sequence,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Tests `SqlDatabase` schema and typed row conversions.
    #[test]
    #[allow(clippy::too_many_lines)]
    fn test_sql_database_schema_and_row() -> Result<()> {
        let schema = sql_database_schema();
        assert_eq!(schema.name, "SqlDatabase");
        assert_eq!(schema.columns.len(), 7);
        assert_eq!(schema.primary_keys(), vec!["SqlDatabase"]);

        let comp = ComponentName::new("C_Db")?;

        let row = SqlDatabaseRow::new(
            "OpenEdXDb",
            "127.0.0.1",
            Some("DEFAULT".to_string()),
            "openedx",
            comp.clone(),
            Some("root_user".to_string()),
            1,
        )?;

        let rec = row.to_record();
        assert_eq!(rec.len(), 7);
        let parsed = SqlDatabaseRow::from_record(&rec)?;
        assert_eq!(parsed, row);

        // Minimal row (None branches for instance and user)
        let row_min =
            SqlDatabaseRow::new("MinDb", "localhost", None, "mindb", comp.clone(), None, 0)?;
        let rec_min = row_min.to_record();
        assert_eq!(rec_min.get(2), Some(&FieldValue::Null));
        assert_eq!(rec_min.get(5), Some(&FieldValue::Null));
        let parsed_min = SqlDatabaseRow::from_record(&rec_min)?;
        assert_eq!(parsed_min, row_min);

        // from_record with empty string for optional fields (instance, user)
        let rec_empty_opts = Record::with_fields(vec![
            FieldValue::String("DbEmptyOpts".to_string()),
            FieldValue::String("127.0.0.1".to_string()),
            FieldValue::String(String::new()),
            FieldValue::String("testdb".to_string()),
            FieldValue::String("C_Db".to_string()),
            FieldValue::String(String::new()),
            FieldValue::Short(2),
        ]);
        let parsed_empty_opts = SqlDatabaseRow::from_record(&rec_empty_opts)?;
        assert_eq!(parsed_empty_opts.instance, None);
        assert_eq!(parsed_empty_opts.user, None);
        assert_eq!(parsed_empty_opts.attributes, 2);

        // from_record with Null user (covering line 227 _ => None branch)
        let mut rec_null_user = rec_empty_opts.clone();
        rec_null_user.set(5, FieldValue::Null);
        let parsed_null_user = SqlDatabaseRow::from_record(&rec_null_user)?;
        assert_eq!(parsed_null_user.user, None);

        // from_record with Null attributes (fallback to 0)
        let mut rec_null_attrs = rec_empty_opts;
        rec_null_attrs.set(6, FieldValue::Null);
        let parsed_null_attrs = SqlDatabaseRow::from_record(&rec_null_attrs)?;
        assert_eq!(parsed_null_attrs.attributes, 0);

        // Validation errors
        assert!(SqlDatabaseRow::new("", "srv", None, "db", comp.clone(), None, 0).is_err());
        assert!(
            SqlDatabaseRow::new("A".repeat(73), "srv", None, "db", comp.clone(), None, 0).is_err()
        );
        assert!(SqlDatabaseRow::new("Db", "", None, "db", comp.clone(), None, 0).is_err());
        assert!(SqlDatabaseRow::new("Db", "srv", None, "", comp, None, 0).is_err());

        // Short record
        let short_rec = Record::with_fields(vec![FieldValue::String("A".to_string())]);
        assert!(SqlDatabaseRow::from_record(&short_rec).is_err());

        // Missing fields in record
        let empty_pk = Record::with_fields(vec![
            FieldValue::Null,
            FieldValue::String("s".to_string()),
            FieldValue::Null,
            FieldValue::String("d".to_string()),
            FieldValue::String("c".to_string()),
            FieldValue::Null,
            FieldValue::Long(0),
        ]);
        assert!(SqlDatabaseRow::from_record(&empty_pk).is_err());

        let empty_str_pk = Record::with_fields(vec![
            FieldValue::String(String::new()),
            FieldValue::String("s".to_string()),
            FieldValue::Null,
            FieldValue::String("d".to_string()),
            FieldValue::String("c".to_string()),
            FieldValue::Null,
            FieldValue::Long(0),
        ]);
        assert!(SqlDatabaseRow::from_record(&empty_str_pk).is_err());

        let empty_srv = Record::with_fields(vec![
            FieldValue::String("id".to_string()),
            FieldValue::Null,
            FieldValue::Null,
            FieldValue::String("d".to_string()),
            FieldValue::String("c".to_string()),
            FieldValue::Null,
            FieldValue::Long(0),
        ]);
        assert!(SqlDatabaseRow::from_record(&empty_srv).is_err());

        let empty_str_srv = Record::with_fields(vec![
            FieldValue::String("id".to_string()),
            FieldValue::String(String::new()),
            FieldValue::Null,
            FieldValue::String("d".to_string()),
            FieldValue::String("c".to_string()),
            FieldValue::Null,
            FieldValue::Long(0),
        ]);
        assert!(SqlDatabaseRow::from_record(&empty_str_srv).is_err());

        let empty_db = Record::with_fields(vec![
            FieldValue::String("id".to_string()),
            FieldValue::String("s".to_string()),
            FieldValue::Null,
            FieldValue::Null,
            FieldValue::String("c".to_string()),
            FieldValue::Null,
            FieldValue::Long(0),
        ]);
        assert!(SqlDatabaseRow::from_record(&empty_db).is_err());

        let empty_str_db = Record::with_fields(vec![
            FieldValue::String("id".to_string()),
            FieldValue::String("s".to_string()),
            FieldValue::Null,
            FieldValue::String(String::new()),
            FieldValue::String("c".to_string()),
            FieldValue::Null,
            FieldValue::Long(0),
        ]);
        assert!(SqlDatabaseRow::from_record(&empty_str_db).is_err());

        let empty_comp = Record::with_fields(vec![
            FieldValue::String("id".to_string()),
            FieldValue::String("s".to_string()),
            FieldValue::Null,
            FieldValue::String("d".to_string()),
            FieldValue::Null,
            FieldValue::Null,
            FieldValue::Long(0),
        ]);
        assert!(SqlDatabaseRow::from_record(&empty_comp).is_err());

        let empty_str_comp = Record::with_fields(vec![
            FieldValue::String("id".to_string()),
            FieldValue::String("s".to_string()),
            FieldValue::Null,
            FieldValue::String("d".to_string()),
            FieldValue::String(String::new()),
            FieldValue::Null,
            FieldValue::Long(0),
        ]);
        assert!(SqlDatabaseRow::from_record(&empty_str_comp).is_err());

        // from_record with fields exceeding 72 chars
        let long_id_rec = Record::with_fields(vec![
            FieldValue::String("A".repeat(73)),
            FieldValue::String("s".to_string()),
            FieldValue::Null,
            FieldValue::String("d".to_string()),
            FieldValue::String("C_Db".to_string()),
            FieldValue::Null,
            FieldValue::Long(0),
        ]);
        assert!(SqlDatabaseRow::from_record(&long_id_rec).is_err());

        // clone, debug, equality
        let row_cloned = row.clone();
        assert_eq!(row_cloned, row);
        assert!(format!("{row:?}").contains("OpenEdXDb"));

        Ok(())
    }

    /// Tests `SqlString` schema and typed row conversions.
    #[test]
    #[allow(clippy::too_many_lines)]
    fn test_sql_string_schema_and_row() -> Result<()> {
        let schema = sql_string_schema();
        assert_eq!(schema.name, "SqlString");
        assert_eq!(schema.columns.len(), 6);
        assert_eq!(schema.primary_keys(), vec!["SqlString"]);

        let row = SqlStringRow::new(
            "CreateSchema",
            "OpenEdXDb",
            "CREATE DATABASE IF NOT EXISTS `openedx`;",
            Some("root_user".to_string()),
            1,
            Some(10),
        )?;

        let rec = row.to_record();
        assert_eq!(rec.len(), 6);
        let parsed = SqlStringRow::from_record(&rec)?;
        assert_eq!(parsed, row);

        // Minimal row (None branches for user and sequence)
        let row_min = SqlStringRow::new("MinSql", "Db", "SELECT 1;", None, 0, None)?;
        let rec_min = row_min.to_record();
        assert_eq!(rec_min.get(3), Some(&FieldValue::Null));
        assert_eq!(rec_min.get(5), Some(&FieldValue::Null));
        let parsed_min = SqlStringRow::from_record(&rec_min)?;
        assert_eq!(parsed_min, row_min);

        // from_record with empty string for user
        let rec_empty_user = Record::with_fields(vec![
            FieldValue::String("StrEmptyUser".to_string()),
            FieldValue::String("OpenEdXDb".to_string()),
            FieldValue::String("SELECT 1;".to_string()),
            FieldValue::String(String::new()),
            FieldValue::Short(2),
            FieldValue::Short(5),
        ]);
        let parsed_empty_user = SqlStringRow::from_record(&rec_empty_user)?;
        assert_eq!(parsed_empty_user.user, None);
        assert_eq!(parsed_empty_user.attributes, 2);
        assert_eq!(parsed_empty_user.sequence, Some(5));

        // from_record with Null user (covering line 430 _ => None branch)
        let mut rec_null_user = rec_empty_user.clone();
        rec_null_user.set(3, FieldValue::Null);
        let parsed_null_user = SqlStringRow::from_record(&rec_null_user)?;
        assert_eq!(parsed_null_user.user, None);

        // from_record with Null attributes and Null sequence
        let mut rec_null_attrs = rec_empty_user;
        rec_null_attrs.set(4, FieldValue::Null);
        rec_null_attrs.set(5, FieldValue::Null);
        let parsed_null_attrs = SqlStringRow::from_record(&rec_null_attrs)?;
        assert_eq!(parsed_null_attrs.attributes, 0);
        assert_eq!(parsed_null_attrs.sequence, None);

        // Validation errors
        assert!(SqlStringRow::new("", "Db", "SQL", None, 0, None).is_err());
        assert!(SqlStringRow::new("A".repeat(73), "Db", "SQL", None, 0, None).is_err());
        assert!(SqlStringRow::new("Id", "", "SQL", None, 0, None).is_err());
        assert!(SqlStringRow::new("Id", "Db", "", None, 0, None).is_err());

        let short_rec = Record::with_fields(vec![FieldValue::String("A".to_string())]);
        assert!(SqlStringRow::from_record(&short_rec).is_err());

        let empty_pk = Record::with_fields(vec![
            FieldValue::Null,
            FieldValue::String("db".to_string()),
            FieldValue::String("sql".to_string()),
            FieldValue::Null,
            FieldValue::Long(0),
            FieldValue::Null,
        ]);
        assert!(SqlStringRow::from_record(&empty_pk).is_err());

        let empty_str_pk = Record::with_fields(vec![
            FieldValue::String(String::new()),
            FieldValue::String("db".to_string()),
            FieldValue::String("sql".to_string()),
            FieldValue::Null,
            FieldValue::Long(0),
            FieldValue::Null,
        ]);
        assert!(SqlStringRow::from_record(&empty_str_pk).is_err());

        let empty_db = Record::with_fields(vec![
            FieldValue::String("id".to_string()),
            FieldValue::Null,
            FieldValue::String("sql".to_string()),
            FieldValue::Null,
            FieldValue::Long(0),
            FieldValue::Null,
        ]);
        assert!(SqlStringRow::from_record(&empty_db).is_err());

        let empty_str_db = Record::with_fields(vec![
            FieldValue::String("id".to_string()),
            FieldValue::String(String::new()),
            FieldValue::String("sql".to_string()),
            FieldValue::Null,
            FieldValue::Long(0),
            FieldValue::Null,
        ]);
        assert!(SqlStringRow::from_record(&empty_str_db).is_err());

        let empty_sql = Record::with_fields(vec![
            FieldValue::String("id".to_string()),
            FieldValue::String("db".to_string()),
            FieldValue::Null,
            FieldValue::Null,
            FieldValue::Long(0),
            FieldValue::Null,
        ]);
        assert!(SqlStringRow::from_record(&empty_sql).is_err());

        let empty_str_sql = Record::with_fields(vec![
            FieldValue::String("id".to_string()),
            FieldValue::String("db".to_string()),
            FieldValue::String(String::new()),
            FieldValue::Null,
            FieldValue::Long(0),
            FieldValue::Null,
        ]);
        assert!(SqlStringRow::from_record(&empty_str_sql).is_err());

        // from_record with id > 72 chars
        let long_id_rec = Record::with_fields(vec![
            FieldValue::String("A".repeat(73)),
            FieldValue::String("db".to_string()),
            FieldValue::String("sql".to_string()),
            FieldValue::Null,
            FieldValue::Long(0),
            FieldValue::Null,
        ]);
        assert!(SqlStringRow::from_record(&long_id_rec).is_err());

        // clone, debug, equality
        let row_cloned = row.clone();
        assert_eq!(row_cloned, row);
        assert!(format!("{row:?}").contains("CreateSchema"));

        Ok(())
    }

    /// Tests `SqlScript` schema and typed row conversions.
    #[test]
    #[allow(clippy::too_many_lines)]
    fn test_sql_script_schema_and_row() -> Result<()> {
        let schema = sql_script_schema();
        assert_eq!(schema.name, "SqlScript");
        assert_eq!(schema.columns.len(), 7);
        assert_eq!(schema.primary_keys(), vec!["SqlScript"]);

        let comp = ComponentName::new("C_Db")?;

        let row = SqlScriptRow::new(
            "InitScript",
            "OpenEdXDb",
            comp.clone(),
            "[#schema.sql]",
            None,
            0,
            Some(1),
        )?;

        let rec = row.to_record();
        assert_eq!(rec.len(), 7);
        let parsed = SqlScriptRow::from_record(&rec)?;
        assert_eq!(parsed, row);

        // Row with user present (Some branch for user in to_record)
        let row_with_user = SqlScriptRow::new(
            "ScriptUser",
            "OpenEdXDb",
            comp.clone(),
            "file.sql",
            Some("admin".to_string()),
            1,
            None,
        )?;
        let rec_user = row_with_user.to_record();
        assert_eq!(
            rec_user.get(4),
            Some(&FieldValue::String("admin".to_string()))
        );
        assert_eq!(rec_user.get(6), Some(&FieldValue::Null));
        let parsed_user = SqlScriptRow::from_record(&rec_user)?;
        assert_eq!(parsed_user, row_with_user);

        // from_record with empty user, Short attributes, Short sequence
        let rec_empty_user = Record::with_fields(vec![
            FieldValue::String("ScrEmptyUser".to_string()),
            FieldValue::String("OpenEdXDb".to_string()),
            FieldValue::String("C_Db".to_string()),
            FieldValue::String("file.sql".to_string()),
            FieldValue::String(String::new()),
            FieldValue::Short(4),
            FieldValue::Short(8),
        ]);
        let parsed_empty_user = SqlScriptRow::from_record(&rec_empty_user)?;
        assert_eq!(parsed_empty_user.user, None);
        assert_eq!(parsed_empty_user.attributes, 4);
        assert_eq!(parsed_empty_user.sequence, Some(8));

        // from_record with Null user (covering line 654 _ => None branch)
        let mut rec_null_user = rec_empty_user.clone();
        rec_null_user.set(4, FieldValue::Null);
        let parsed_null_user = SqlScriptRow::from_record(&rec_null_user)?;
        assert_eq!(parsed_null_user.user, None);

        // from_record with Null attributes and Null sequence
        let mut rec_null = rec_empty_user;
        rec_null.set(5, FieldValue::Null);
        rec_null.set(6, FieldValue::Null);
        let parsed_null = SqlScriptRow::from_record(&rec_null)?;
        assert_eq!(parsed_null.attributes, 0);
        assert_eq!(parsed_null.sequence, None);

        // Validation errors
        assert!(SqlScriptRow::new("", "Db", comp.clone(), "f.sql", None, 0, None).is_err());
        assert!(
            SqlScriptRow::new("A".repeat(73), "Db", comp.clone(), "f.sql", None, 0, None).is_err()
        );
        assert!(SqlScriptRow::new("Id", "", comp.clone(), "f.sql", None, 0, None).is_err());
        assert!(SqlScriptRow::new("Id", "Db", comp, "", None, 0, None).is_err());

        let short_rec = Record::with_fields(vec![FieldValue::String("A".to_string())]);
        assert!(SqlScriptRow::from_record(&short_rec).is_err());

        let empty_pk = Record::with_fields(vec![
            FieldValue::Null,
            FieldValue::String("db".to_string()),
            FieldValue::String("c".to_string()),
            FieldValue::String("f".to_string()),
            FieldValue::Null,
            FieldValue::Long(0),
            FieldValue::Null,
        ]);
        assert!(SqlScriptRow::from_record(&empty_pk).is_err());

        let empty_str_pk = Record::with_fields(vec![
            FieldValue::String(String::new()),
            FieldValue::String("db".to_string()),
            FieldValue::String("c".to_string()),
            FieldValue::String("f".to_string()),
            FieldValue::Null,
            FieldValue::Long(0),
            FieldValue::Null,
        ]);
        assert!(SqlScriptRow::from_record(&empty_str_pk).is_err());

        let empty_db = Record::with_fields(vec![
            FieldValue::String("id".to_string()),
            FieldValue::Null,
            FieldValue::String("c".to_string()),
            FieldValue::String("f".to_string()),
            FieldValue::Null,
            FieldValue::Long(0),
            FieldValue::Null,
        ]);
        assert!(SqlScriptRow::from_record(&empty_db).is_err());

        let empty_str_db = Record::with_fields(vec![
            FieldValue::String("id".to_string()),
            FieldValue::String(String::new()),
            FieldValue::String("c".to_string()),
            FieldValue::String("f".to_string()),
            FieldValue::Null,
            FieldValue::Long(0),
            FieldValue::Null,
        ]);
        assert!(SqlScriptRow::from_record(&empty_str_db).is_err());

        let empty_comp = Record::with_fields(vec![
            FieldValue::String("id".to_string()),
            FieldValue::String("db".to_string()),
            FieldValue::Null,
            FieldValue::String("f".to_string()),
            FieldValue::Null,
            FieldValue::Long(0),
            FieldValue::Null,
        ]);
        assert!(SqlScriptRow::from_record(&empty_comp).is_err());

        let empty_str_comp = Record::with_fields(vec![
            FieldValue::String("id".to_string()),
            FieldValue::String("db".to_string()),
            FieldValue::String(String::new()),
            FieldValue::String("f".to_string()),
            FieldValue::Null,
            FieldValue::Long(0),
            FieldValue::Null,
        ]);
        assert!(SqlScriptRow::from_record(&empty_str_comp).is_err());

        let empty_f = Record::with_fields(vec![
            FieldValue::String("id".to_string()),
            FieldValue::String("db".to_string()),
            FieldValue::String("c".to_string()),
            FieldValue::Null,
            FieldValue::Null,
            FieldValue::Long(0),
            FieldValue::Null,
        ]);
        assert!(SqlScriptRow::from_record(&empty_f).is_err());

        let empty_str_f = Record::with_fields(vec![
            FieldValue::String("id".to_string()),
            FieldValue::String("db".to_string()),
            FieldValue::String("c".to_string()),
            FieldValue::String(String::new()),
            FieldValue::Null,
            FieldValue::Long(0),
            FieldValue::Null,
        ]);
        assert!(SqlScriptRow::from_record(&empty_str_f).is_err());

        // from_record with id > 72 chars
        let long_id_rec = Record::with_fields(vec![
            FieldValue::String("A".repeat(73)),
            FieldValue::String("db".to_string()),
            FieldValue::String("C_Db".to_string()),
            FieldValue::String("f.sql".to_string()),
            FieldValue::Null,
            FieldValue::Long(0),
            FieldValue::Null,
        ]);
        assert!(SqlScriptRow::from_record(&long_id_rec).is_err());

        // clone, debug, equality
        let row_cloned = row.clone();
        assert_eq!(row_cloned, row);
        assert!(format!("{row:?}").contains("InitScript"));

        Ok(())
    }
}

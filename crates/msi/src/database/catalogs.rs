//! MSI Database System Catalogs (`_Tables`, `_Columns`, `_Streams`, `_Storages`).

use crate::database::column::{ColumnDef, DataType};
use crate::error::{Error, Result};
use std::collections::HashMap;

/// Name of the system catalog table defining all user and system tables.
pub const TABLE_CATALOG_NAME: &str = "_Tables";

/// Name of the system catalog table defining all table columns.
pub const COLUMN_CATALOG_NAME: &str = "_Columns";

/// Name of the system catalog table defining binary streams.
pub const STREAM_CATALOG_NAME: &str = "_Streams";

/// Name of the system catalog table defining nested sub-storages.
pub const STORAGE_CATALOG_NAME: &str = "_Storages";

/// Schema definition for a single database table.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TableSchema {
    /// Name of the table.
    pub name: String,
    /// Ordered list of column definitions.
    pub columns: Vec<ColumnDef>,
}

impl TableSchema {
    /// Creates a new [`TableSchema`] with no columns.
    ///
    /// # Arguments
    ///
    /// * `name` - Table name.
    ///
    /// # Returns
    ///
    /// An empty initialized [`TableSchema`].
    #[must_use]
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            columns: Vec::new(),
        }
    }

    /// Adds a column to the table schema.
    ///
    /// # Arguments
    ///
    /// * `col` - Column definition.
    ///
    /// # Returns
    ///
    /// Self with the column appended.
    #[must_use]
    pub fn with_column(mut self, col: ColumnDef) -> Self {
        self.columns.push(col);
        self
    }

    /// Returns a slice of all column definitions in this table.
    ///
    /// # Returns
    ///
    /// Column definitions slice.
    #[must_use]
    pub fn columns(&self) -> &[ColumnDef] {
        &self.columns
    }

    /// Returns a slice of column names forming the composite primary key.
    ///
    /// # Returns
    ///
    /// Vector of primary key column names.
    #[must_use]
    pub fn primary_keys(&self) -> Vec<&str> {
        self.columns
            .iter()
            .filter(|c| c.primary_key)
            .map(|c| c.name.as_str())
            .collect()
    }

    /// Calculates the row record width in bytes for this table.
    ///
    /// # Arguments
    ///
    /// * `string_index_size` - Size of string pool index (2 or 3 bytes).
    ///
    /// # Returns
    ///
    /// Size of a row record in bytes.
    #[must_use]
    pub fn row_record_size(&self, string_index_size: usize) -> usize {
        self.columns
            .iter()
            .map(|c| c.data_type.record_field_size(string_index_size))
            .sum()
    }
}

/// In-memory catalog manager for all tables in an MSI database.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DatabaseCatalog {
    /// Map of table names to their schemas.
    tables: HashMap<String, TableSchema>,
}

impl DatabaseCatalog {
    /// Creates a new [`DatabaseCatalog`] pre-populated with standard system catalogs.
    ///
    /// Pre-populates:
    /// - `_Tables`: `Name` (s64 \[PK\])
    /// - `_Columns`: `Table` (s64 \[PK\]), `Number` (i2 \[PK\]), `Name` (s64), `Type` (i2)
    /// - `_Streams`: `Name` (s64 \[PK\]), `Data` (v0)
    /// - `_Storages`: `Name` (s64 \[PK\]), `Data` (v0)
    ///
    /// # Returns
    ///
    /// A new [`DatabaseCatalog`].
    #[must_use]
    pub fn new() -> Self {
        let mut catalog = Self {
            tables: HashMap::new(),
        };

        // 1. _Tables catalog
        let tables_schema = TableSchema::new(TABLE_CATALOG_NAME)
            .with_column(ColumnDef::new("Name", DataType::String { max_len: 64 }).primary_key());
        catalog
            .tables
            .insert(TABLE_CATALOG_NAME.to_string(), tables_schema);

        // 2. _Columns catalog
        let columns_schema = TableSchema::new(COLUMN_CATALOG_NAME)
            .with_column(ColumnDef::new("Table", DataType::String { max_len: 64 }).primary_key())
            .with_column(ColumnDef::new("Number", DataType::Short).primary_key())
            .with_column(ColumnDef::new("Name", DataType::String { max_len: 64 }))
            .with_column(ColumnDef::new("Type", DataType::Short));
        catalog
            .tables
            .insert(COLUMN_CATALOG_NAME.to_string(), columns_schema);

        // 3. _Streams catalog
        let streams_schema = TableSchema::new(STREAM_CATALOG_NAME)
            .with_column(ColumnDef::new("Name", DataType::String { max_len: 64 }).primary_key())
            .with_column(ColumnDef::new("Data", DataType::Stream));
        catalog
            .tables
            .insert(STREAM_CATALOG_NAME.to_string(), streams_schema);

        // 4. _Storages catalog
        let storages_schema = TableSchema::new(STORAGE_CATALOG_NAME)
            .with_column(ColumnDef::new("Name", DataType::String { max_len: 64 }).primary_key())
            .with_column(ColumnDef::new("Data", DataType::Stream));
        catalog
            .tables
            .insert(STORAGE_CATALOG_NAME.to_string(), storages_schema);

        catalog
    }

    /// Adds a table schema to the catalog.
    ///
    /// # Arguments
    ///
    /// * `schema` - The table schema.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Validation`] if a table with this name already exists.
    pub fn add_table(&mut self, schema: TableSchema) -> Result<()> {
        if self.tables.contains_key(&schema.name) {
            return Err(Error::Validation {
                element: schema.name,
                reason: "table already exists in catalog".to_string(),
            });
        }
        self.tables.insert(schema.name.clone(), schema);
        Ok(())
    }

    /// Retrieves a table schema by name.
    ///
    /// # Arguments
    ///
    /// * `name` - Table name.
    ///
    /// # Returns
    ///
    /// Reference to [`TableSchema`], or `None` if not found.
    #[must_use]
    pub fn get_table(&self, name: &str) -> Option<&TableSchema> {
        self.tables.get(name)
    }

    /// Returns a list of all table names registered in the catalog.
    ///
    /// # Returns
    ///
    /// Vector of table name strings.
    #[must_use]
    pub fn table_names(&self) -> Vec<&str> {
        self.tables.keys().map(String::as_str).collect()
    }
}

impl Default for DatabaseCatalog {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Tests pre-populated system catalogs and primary keys.
    #[test]
    fn test_system_catalogs() {
        let catalog = DatabaseCatalog::new();
        assert!(catalog.get_table(TABLE_CATALOG_NAME).is_some());
        assert!(catalog.get_table(COLUMN_CATALOG_NAME).is_some());
        assert!(catalog.get_table(STREAM_CATALOG_NAME).is_some());
        assert!(catalog.get_table(STORAGE_CATALOG_NAME).is_some());
        assert_eq!(catalog.get_table("NonExistent"), None);

        assert_eq!(
            catalog
                .get_table(COLUMN_CATALOG_NAME)
                .map(|t| (t.primary_keys(), t.row_record_size(2))),
            Some((vec!["Table", "Number"], 8))
        );

        assert!(catalog.table_names().contains(&TABLE_CATALOG_NAME));
    }

    /// Tests adding user tables and duplicate validation.
    #[test]
    fn test_add_user_table() {
        let mut catalog = DatabaseCatalog::new();

        let property_schema = TableSchema::new("Property")
            .with_column(ColumnDef::new("Property", DataType::String { max_len: 72 }).primary_key())
            .with_column(ColumnDef::new("Value", DataType::String { max_len: 0 }));

        assert!(catalog.add_table(property_schema.clone()).is_ok());

        // Duplicate table error
        assert!(catalog.add_table(property_schema).is_err());

        assert_eq!(
            catalog.get_table("Property").map(|t| (
                t.name.as_str(),
                t.columns().len(),
                t.primary_keys(),
                t.row_record_size(2)
            )),
            Some(("Property", 2, vec!["Property"], 4))
        );
    }

    /// Tests [`DatabaseCatalog::default`] constructor.
    #[test]
    fn test_database_catalog_default() {
        let def = DatabaseCatalog::default();
        assert!(def.get_table(TABLE_CATALOG_NAME).is_some());
    }
}

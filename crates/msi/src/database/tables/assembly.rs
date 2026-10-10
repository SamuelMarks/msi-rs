//! Assembly management and Side-by-Side (WinSxS) schemas.
//!
//! Provides table definitions and strongly-typed rows for `MsiAssembly` and `MsiAssemblyName`.

use crate::database::catalogs::TableSchema;
use crate::database::column::{ColumnDef, DataType};
use crate::database::tables::record::{FieldValue, Record};
use crate::error::{MsiError, Result};

/// Returns the schema for the `MsiAssembly` table.
///
/// Specifies Windows Installer settings for side-by-side (WinSxS) and .NET Global Assembly Cache (GAC) assemblies.
///
/// # Returns
///
/// An instance of this struct, or an appropriate return type.
#[must_use]
pub fn msi_assembly_schema() -> TableSchema {
    TableSchema::new("MsiAssembly")
        .with_column(ColumnDef::new("Component_", DataType::String { max_len: 72 }).primary_key())
        .with_column(ColumnDef::new("Feature_", DataType::String { max_len: 38 }).primary_key())
        .with_column(ColumnDef::new("File_Manifest", DataType::String { max_len: 72 }).nullable())
        .with_column(ColumnDef::new("File_Application", DataType::String { max_len: 72 }).nullable())
        .with_column(ColumnDef::new("Attributes", DataType::Short).nullable())
}

/// A strongly-typed row from the `MsiAssembly` table.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MsiAssemblyRow {
    /// External key to `Component` table.
    pub component: String,
    /// External key to `Feature` table.
    pub feature: String,
    /// External key to `File` table for the manifest.
    pub file_manifest: Option<String>,
    /// External key to `File` table for the application.
    pub file_application: Option<String>,
    /// Assembly attributes (e.g. 0 for .NET, 1 for Win32).
    pub attributes: Option<u16>,
}

impl MsiAssemblyRow {
    /// Parses an [`MsiAssemblyRow`] from a raw [`Record`].
    ///
    /// # Arguments
    ///
    /// * `record` - The raw database record.
    ///
    /// # Errors
    ///
    /// Returns [`MsiError::Validation`] if fields are missing or wrongly typed.
    ///
    /// # Returns
    ///
    /// An instance of this struct, or an appropriate return type.
    pub fn from_record(record: &Record) -> Result<Self> {
        if record.len() < 5 {
            return Err(MsiError::Validation {
                element: "MsiAssembly".to_string(),
                reason: "Record has insufficient fields".to_string(),
            });
        }

        let component = match record.get(0) {
            Some(FieldValue::String(s)) => s.clone(),
            _ => return Err(MsiError::Validation {
                element: "MsiAssembly.Component_".to_string(),
                reason: "Must be a string".to_string(),
            }),
        };

        let feature = match record.get(1) {
            Some(FieldValue::String(s)) => s.clone(),
            _ => return Err(MsiError::Validation {
                element: "MsiAssembly.Feature_".to_string(),
                reason: "Must be a string".to_string(),
            }),
        };

        let file_manifest = match record.get(2) {
            Some(FieldValue::String(s)) => Some(s.clone()),
            Some(FieldValue::Null) | None => None,
            _ => return Err(MsiError::Validation {
                element: "MsiAssembly.File_Manifest".to_string(),
                reason: "Must be a string or null".to_string(),
            }),
        };

        let file_application = match record.get(3) {
            Some(FieldValue::String(s)) => Some(s.clone()),
            Some(FieldValue::Null) | None => None,
            _ => return Err(MsiError::Validation {
                element: "MsiAssembly.File_Application".to_string(),
                reason: "Must be a string or null".to_string(),
            }),
        };

        let attributes = match record.get(4) {
            Some(FieldValue::Short(i)) => Some(u16::try_from(*i).unwrap_or(0)),
            Some(FieldValue::Null) | None => None,
            _ => return Err(MsiError::Validation {
                element: "MsiAssembly.Attributes".to_string(),
                reason: "Must be an integer or null".to_string(),
            }),
        };

        Ok(Self {
            component,
            feature,
            file_manifest,
            file_application,
            attributes,
        })
    }
}

/// Returns the schema for the `MsiAssemblyName` table.
///
/// Specifies the elements of a strong assembly name.
///
/// # Returns
///
/// An instance of this struct, or an appropriate return type.
#[must_use]
pub fn msi_assembly_name_schema() -> TableSchema {
    TableSchema::new("MsiAssemblyName")
        .with_column(ColumnDef::new("Component_", DataType::String { max_len: 72 }).primary_key())
        .with_column(ColumnDef::new("Name", DataType::String { max_len: 255 }).primary_key())
        .with_column(ColumnDef::new("Value", DataType::String { max_len: 255 }))
}

/// A strongly-typed row from the `MsiAssemblyName` table.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MsiAssemblyNameRow {
    /// External key to `Component` table.
    pub component: String,
    /// Name of the attribute.
    pub name: String,
    /// Value of the attribute.
    pub value: String,
}

impl MsiAssemblyNameRow {
    /// Parses an [`MsiAssemblyNameRow`] from a raw [`Record`].
    ///
    /// # Arguments
    ///
    /// * `record` - The raw database record.
    ///
    /// # Errors
    ///
    /// Returns [`MsiError::Validation`] if fields are missing or wrongly typed.
    ///
    /// # Returns
    ///
    /// An instance of this struct, or an appropriate return type.
    pub fn from_record(record: &Record) -> Result<Self> {
        if record.len() < 3 {
            return Err(MsiError::Validation {
                element: "MsiAssemblyName".to_string(),
                reason: "Record has insufficient fields".to_string(),
            });
        }

        let component = match record.get(0) {
            Some(FieldValue::String(s)) => s.clone(),
            _ => return Err(MsiError::Validation {
                element: "MsiAssemblyName.Component_".to_string(),
                reason: "Must be a string".to_string(),
            }),
        };

        let name = match record.get(1) {
            Some(FieldValue::String(s)) => s.clone(),
            _ => return Err(MsiError::Validation {
                element: "MsiAssemblyName.Name".to_string(),
                reason: "Must be a string".to_string(),
            }),
        };

        let value = match record.get(2) {
            Some(FieldValue::String(s)) => s.clone(),
            _ => return Err(MsiError::Validation {
                element: "MsiAssemblyName.Value".to_string(),
                reason: "Must be a string".to_string(),
            }),
        };

        Ok(Self {
            component,
            name,
            value,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_msi_assembly_schema() {
        let schema = msi_assembly_schema();
        assert_eq!(schema.name, "MsiAssembly");
        assert_eq!(schema.columns.len(), 5);
        assert!(schema.columns[0].primary_key);
        assert!(schema.columns[1].primary_key);
        assert!(schema.columns[2].nullable);
        assert!(schema.columns[3].nullable);
        assert!(schema.columns[4].nullable);
    }

    #[test]
    fn test_msi_assembly_row_parsing() {
        let mut record = Record::new();
        record.push(FieldValue::String("Comp1".to_string()));
        record.push(FieldValue::String("Feat1".to_string()));
        record.push(FieldValue::String("Manifest1".to_string()));
        record.push(FieldValue::String("App1".to_string()));
        record.push(FieldValue::Short(1));

        let row = MsiAssemblyRow::from_record(&record).expect("test");
        assert_eq!(row.component, "Comp1");
        assert_eq!(row.feature, "Feat1");
        assert_eq!(row.file_manifest, Some("Manifest1".to_string()));
        assert_eq!(row.file_application, Some("App1".to_string()));
        assert_eq!(row.attributes, Some(1));

        let mut record_null = Record::new();
        record_null.push(FieldValue::String("Comp2".to_string()));
        record_null.push(FieldValue::String("Feat2".to_string()));
        record_null.push(FieldValue::Null);
        record_null.push(FieldValue::Null);
        record_null.push(FieldValue::Null);

        let row_null = MsiAssemblyRow::from_record(&record_null).expect("test");
        assert_eq!(row_null.file_manifest, None);
        assert_eq!(row_null.file_application, None);
        assert_eq!(row_null.attributes, None);
    }

    #[test]
    fn test_msi_assembly_row_errors() {
        assert!(MsiAssemblyRow::from_record(&Record::new()).is_err());
        
        let mut r1 = Record::new();
        r1.push(FieldValue::Short(1));
        r1.push(FieldValue::String("Feat1".to_string()));
        r1.push(FieldValue::Null);
        r1.push(FieldValue::Null);
        r1.push(FieldValue::Null);
        assert!(MsiAssemblyRow::from_record(&r1).is_err());
        
        let mut r2 = Record::new();
        r2.push(FieldValue::String("Comp1".to_string()));
        r2.push(FieldValue::Short(1));
        r2.push(FieldValue::Null);
        r2.push(FieldValue::Null);
        r2.push(FieldValue::Null);
        assert!(MsiAssemblyRow::from_record(&r2).is_err());
        
        let mut r3 = Record::new();
        r3.push(FieldValue::String("Comp1".to_string()));
        r3.push(FieldValue::String("Feat1".to_string()));
        r3.push(FieldValue::Short(1));
        r3.push(FieldValue::Null);
        r3.push(FieldValue::Null);
        assert!(MsiAssemblyRow::from_record(&r3).is_err());
        
        let mut r4 = Record::new();
        r4.push(FieldValue::String("Comp1".to_string()));
        r4.push(FieldValue::String("Feat1".to_string()));
        r4.push(FieldValue::Null);
        r4.push(FieldValue::Short(1));
        r4.push(FieldValue::Null);
        assert!(MsiAssemblyRow::from_record(&r4).is_err());
        
        let mut r5 = Record::new();
        r5.push(FieldValue::String("Comp1".to_string()));
        r5.push(FieldValue::String("Feat1".to_string()));
        r5.push(FieldValue::Null);
        r5.push(FieldValue::Null);
        r5.push(FieldValue::String("1".to_string()));
        assert!(MsiAssemblyRow::from_record(&r5).is_err());
    }

    #[test]
    fn test_msi_assembly_name_schema() {
        let schema = msi_assembly_name_schema();
        assert_eq!(schema.name, "MsiAssemblyName");
        assert_eq!(schema.columns.len(), 3);
        assert!(schema.columns[0].primary_key);
        assert!(schema.columns[1].primary_key);
        assert!(!schema.columns[2].primary_key);
    }

    #[test]
    fn test_msi_assembly_name_row_parsing() {
        let mut record = Record::new();
        record.push(FieldValue::String("Comp1".to_string()));
        record.push(FieldValue::String("version".to_string()));
        record.push(FieldValue::String("1.0.0.0".to_string()));

        let row = MsiAssemblyNameRow::from_record(&record).expect("test");
        assert_eq!(row.component, "Comp1");
        assert_eq!(row.name, "version");
        assert_eq!(row.value, "1.0.0.0");
    }

    #[test]
    fn test_msi_assembly_name_row_errors() {
        assert!(MsiAssemblyNameRow::from_record(&Record::new()).is_err());
        
        let mut r1 = Record::new();
        r1.push(FieldValue::Short(1));
        r1.push(FieldValue::String("name".to_string()));
        r1.push(FieldValue::String("val".to_string()));
        assert!(MsiAssemblyNameRow::from_record(&r1).is_err());
        
        let mut r2 = Record::new();
        r2.push(FieldValue::String("Comp".to_string()));
        r2.push(FieldValue::Short(1));
        r2.push(FieldValue::String("val".to_string()));
        assert!(MsiAssemblyNameRow::from_record(&r2).is_err());
        
        let mut r3 = Record::new();
        r3.push(FieldValue::String("Comp".to_string()));
        r3.push(FieldValue::String("name".to_string()));
        r3.push(FieldValue::Short(1));
        assert!(MsiAssemblyNameRow::from_record(&r3).is_err());
    }
}
use crate::database::catalogs::TableSchema;
use crate::database::column::ColumnDef;

/// Returns the schema for the `_Validation` table.
///
/// # Returns
///
/// An instance of this struct, or an appropriate return type.
#[must_use]
pub fn validation_schema() -> TableSchema {
    TableSchema::new("_Validation")
        .with_column(
            ColumnDef::new(
                "Table",
                crate::database::column::DataType::String { max_len: 32 },
            )
            .primary_key(),
        )
        .with_column(
            ColumnDef::new(
                "Column",
                crate::database::column::DataType::String { max_len: 32 },
            )
            .primary_key(),
        )
        .with_column(ColumnDef::new(
            "Nullable",
            crate::database::column::DataType::String { max_len: 255 },
        ))
        .with_column(ColumnDef::new("MinValue", crate::database::column::DataType::Long).nullable())
        .with_column(ColumnDef::new("MaxValue", crate::database::column::DataType::Long).nullable())
        .with_column(
            ColumnDef::new(
                "KeyTable",
                crate::database::column::DataType::String { max_len: 255 },
            )
            .nullable(),
        )
        .with_column(
            ColumnDef::new("KeyColumn", crate::database::column::DataType::Long).nullable(),
        )
        .with_column(
            ColumnDef::new(
                "Category",
                crate::database::column::DataType::String { max_len: 32 },
            )
            .nullable(),
        )
        .with_column(
            ColumnDef::new(
                "Set",
                crate::database::column::DataType::String { max_len: 255 },
            )
            .nullable(),
        )
        .with_column(
            ColumnDef::new(
                "Description",
                crate::database::column::DataType::String { max_len: 255 },
            )
            .nullable(),
        )
}

#[cfg(test)]
#[allow(clippy::redundant_clone)]
mod tests {
    use super::*;

    #[test]
    fn test_validation_schema() {
        let schema = validation_schema();
        assert_eq!(schema.name.as_str(), "_Validation");
        assert_eq!(schema.columns.len(), 10);
        assert_eq!(schema.columns[0].name.as_str(), "Table");
        assert!(schema.columns[0].primary_key);
        assert_eq!(schema.columns[1].name.as_str(), "Column");
        assert!(schema.columns[1].primary_key);
        assert_eq!(schema.columns[2].name.as_str(), "Nullable");
    }
}

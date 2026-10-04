use crate::database::catalogs::TableSchema;
use crate::database::column::ColumnDef;

/// Returns the schema for the `_Validation` table.
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

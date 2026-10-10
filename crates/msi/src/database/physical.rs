//! Physical table layout abstractions and indices mapping.

use crate::database::column::{ColumnDef, DataType};
use crate::error::{MsiError, Result};

/// A strongly-typed wrapper for a logical column index.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct LogicalIndex(pub usize);

/// A strongly-typed wrapper for a physical column index.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PhysicalIndex(pub usize);

/// Maintains the mapping between logical and physical column indices for a table.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PhysicalTableLayout<'a> {
    /// Reference to the table's logical column definitions.
    columns: &'a [ColumnDef],
    /// Mapping from logical index to physical index.
    logical_to_physical: Vec<PhysicalIndex>,
    /// Mapping from physical index to logical index.
    physical_to_logical: Vec<LogicalIndex>,
}

impl<'a> PhysicalTableLayout<'a> {
    /// Creates a new physical layout mapping from a slice of logical column definitions.
    ///
    /// # Arguments
    ///
    /// * `columns` - The logical column definitions.
    ///
    /// # Returns
    ///
    /// A computed `PhysicalTableLayout`.
    ///
    /// # Errors
    ///
    /// Returns `PhysicalLayoutError` if a mapping cannot be created (e.g., Stream column as Primary Key).
    pub fn new(columns: &'a [ColumnDef], _is_system_catalog: bool) -> Result<Self> {
        let sorted_indices: Vec<usize> = (0..columns.len()).collect();

        // Check for invalid column states that prevent layout
        for col in columns {
            if col.primary_key && matches!(col.data_type, DataType::Stream) {
                return Err(MsiError::PhysicalLayoutError {
                    reason: format!(
                        "column '{}' is a Stream but is marked as a primary key",
                        col.name
                    ),
                });
            }
        }

        // We enforce a strict 1:1 mapping between logical and physical columns
        // to match Windows Installer's `_Columns` table order.

        let mut logical_to_physical = vec![PhysicalIndex(0); columns.len()];
        let mut physical_to_logical = vec![LogicalIndex(0); columns.len()];

        for (physical, &logical) in sorted_indices.iter().enumerate() {
            logical_to_physical[logical] = PhysicalIndex(physical);
            physical_to_logical[physical] = LogicalIndex(logical);
        }

        Ok(Self {
            columns,
            logical_to_physical,
            physical_to_logical,
        })
    }

    /// Retrieves the physical index for a given logical index.
    ///
    /// # Arguments
    ///
    /// * `logical` - The 0-based logical index.
    ///
    /// # Returns
    ///
    /// The corresponding `PhysicalIndex`, or `None` if out of bounds.
    #[must_use]
    pub fn physical_index(&self, logical: LogicalIndex) -> Option<PhysicalIndex> {
        self.logical_to_physical.get(logical.0).copied()
    }

    /// Retrieves the logical index for a given physical index.
    ///
    /// # Arguments
    ///
    /// * `physical` - The 0-based physical index.
    ///
    /// # Returns
    ///
    /// The corresponding `LogicalIndex`, or `None` if out of bounds.
    #[must_use]
    pub fn logical_index(&self, physical: PhysicalIndex) -> Option<LogicalIndex> {
        self.physical_to_logical.get(physical.0).copied()
    }

    /// Returns a slice of the logical column definitions.
    ///
    /// # Returns
    ///
    /// Slice of `ColumnDef`.
    #[must_use]
    pub const fn columns(&self) -> &'a [ColumnDef] {
        self.columns
    }

    /// Returns an iterator over column definitions in their sorted physical order.
    ///
    /// # Returns
    ///
    /// Iterator yielding `&ColumnDef`.
    pub fn physical_columns(&self) -> impl Iterator<Item = &'a ColumnDef> + '_ {
        self.physical_to_logical
            .iter()
            .map(move |logical| &self.columns[logical.0])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Tests that tables containing composite primary keys maintain strict 1:1 logical order.
    #[test]
    fn test_layout_strict_one_to_one() {
        let cols = vec![
            ColumnDef::new("Attr", DataType::Short),
            ColumnDef::new("Key1", DataType::String { max_len: 72 }).primary_key(),
            ColumnDef::new("Data", DataType::String { max_len: 0 }),
            ColumnDef::new("Key2", DataType::Long).primary_key(),
        ];

        let layout = PhysicalTableLayout::new(&cols, false).expect("valid layout");

        let physical_cols: Vec<_> = layout.physical_columns().map(|c| c.name.as_str()).collect();
        // Strict 1:1 mapping mapping
        assert_eq!(physical_cols, vec!["Attr", "Key1", "Data", "Key2"]);

        // Bidirectional checks
        // Attr: logical 0, physical 0
        assert_eq!(
            layout.physical_index(LogicalIndex(0)),
            Some(PhysicalIndex(0))
        );
        assert_eq!(
            layout.logical_index(PhysicalIndex(0)),
            Some(LogicalIndex(0))
        );

        // Key1: logical 1, physical 1
        assert_eq!(
            layout.physical_index(LogicalIndex(1)),
            Some(PhysicalIndex(1))
        );
        assert_eq!(
            layout.logical_index(PhysicalIndex(1)),
            Some(LogicalIndex(1))
        );

        // Key2: logical 3, physical 3
        assert_eq!(
            layout.physical_index(LogicalIndex(3)),
            Some(PhysicalIndex(3))
        );
        assert_eq!(
            layout.logical_index(PhysicalIndex(3)),
            Some(LogicalIndex(3))
        );
    }

    /// Tests with tables containing mixed length, out-of-order logical definitions (e.g., `InstallExecuteSequence`).
    #[test]
    fn test_layout_mixed_length_strict() {
        // InstallExecuteSequence:
        // Action (String) [PK], Condition (String), Sequence (Short)
        let cols = vec![
            ColumnDef::new("Action", DataType::String { max_len: 72 }).primary_key(),
            ColumnDef::new("Condition", DataType::String { max_len: 255 }).nullable(),
            ColumnDef::new("Sequence", DataType::Short).nullable(),
        ];

        let layout = PhysicalTableLayout::new(&cols, false).expect("valid layout");

        let physical_cols: Vec<_> = layout.physical_columns().map(|c| c.name.as_str()).collect();
        // Strict 1:1 mapping
        assert_eq!(physical_cols, vec!["Action", "Condition", "Sequence"]);

        // Action: logical 0, physical 0
        assert_eq!(
            layout.physical_index(LogicalIndex(0)),
            Some(PhysicalIndex(0))
        );
        // Condition: logical 1, physical 1
        assert_eq!(
            layout.physical_index(LogicalIndex(1)),
            Some(PhysicalIndex(1))
        );
        // Sequence: logical 2, physical 2
        assert_eq!(
            layout.physical_index(LogicalIndex(2)),
            Some(PhysicalIndex(2))
        );
    }

    /// Tests `PhysicalLayoutError` propagation when stream is marked as a primary key.
    #[test]
    fn test_layout_stream_primary_key_error() {
        let cols = vec![ColumnDef::new("Data", DataType::Stream).primary_key()];

        let err = PhysicalTableLayout::new(&cols, false).unwrap_err();
        assert!(matches!(err, MsiError::PhysicalLayoutError { .. }));
    }

    /// Tests out of bounds lookups.
    #[test]
    fn test_layout_out_of_bounds() {
        let cols = vec![ColumnDef::new("Attr", DataType::Short)];
        let layout = PhysicalTableLayout::new(&cols, false).expect("valid layout");

        assert_eq!(layout.physical_index(LogicalIndex(5)), None);
        assert_eq!(layout.logical_index(PhysicalIndex(5)), None);
    }

    /// Tests `columns()` accessor.
    #[test]
    fn test_layout_columns_accessor() {
        let cols = vec![ColumnDef::new("Attr", DataType::Short)];
        let layout = PhysicalTableLayout::new(&cols, false).expect("valid layout");

        assert_eq!(layout.columns().len(), 1);
        assert_eq!(layout.columns()[0].name, "Attr");
    }
}

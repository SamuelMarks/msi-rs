//! MSI table column types, bitmasks, and validation rules (MSI SDK).

use crate::error::{MsiError, Result};
use std::fmt;

/// Non-nullable column flag (`0x0000`).
pub const MSIDB_NULL: u16 = 0x0000;

/// Nullable column flag (`0x1000`).
pub const MSIDB_NULLABLE: u16 = 0x1000;

/// Primary key column flag (`0x2000`).
pub const MSIDB_PRIMARY_KEY: u16 = 0x2000;

/// Localizable string column flag (`0x0200`).
pub const MSIDB_LOCALIZABLE: u16 = 0x0200;

/// Stream object column type flag (`0x0900`).
pub const MSIDB_STREAM: u16 = 0x0900;

/// 4-byte integer column type (`0x0104`).
pub const MSIDB_LONG: u16 = 0x0104;

/// 2-byte integer column type (`0x0502`).
pub const MSIDB_SHORT: u16 = 0x0502;

/// String column base type (`0x0D00`).
pub const MSIDB_STRING: u16 = 0x0D00;

/// Mask for valid data types (`0x00FF` length + type flags).
pub const MSIDB_VALID_FLAGS: u16 = 0x0FFF;

/// Mask for all column attribute and data type flags.
pub const MSIDB_ALL_FLAGS: u16 = 0x3FFF;

/// Primary data type of an MSI table column.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DataType {
    /// 2-byte signed integer (i2 / I2).
    Short,
    /// 4-byte signed integer (i4 / I4).
    Long,
    /// Character string (s / S / l / L) with maximum length (0 = variable/unbounded).
    String {
        /// Maximum length in characters (0 = variable length up to 65,535).
        max_len: u8,
    },
    /// Binary data stream pointer (v / V).
    Stream,
}

impl DataType {
    /// Returns the storage width of this data type in raw table records in bytes.
    ///
    /// For strings and streams, returns the string pool reference size in bytes
    /// (2 bytes if pool entries < 65535, or 3 bytes if large pool).
    ///
    /// # Arguments
    ///
    /// * `string_index_size` - Byte width of string pool indices (2 or 3).
    ///
    /// # Returns
    ///
    /// The size in bytes occupied in each row record.
    #[must_use]
    pub const fn record_field_size(self, string_index_size: usize) -> usize {
        match self {
            Self::Short => 2,
            Self::Long => 4,
            Self::String { .. } | Self::Stream => string_index_size,
        }
    }
}

/// Strongly-typed MSI column schema definition.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ColumnDef {
    /// Column name in table schema.
    pub name: String,
    /// Primary data type.
    pub data_type: DataType,
    /// Whether `NULL` values are permitted in this column.
    pub nullable: bool,
    /// Whether this column is part of the table's composite primary key.
    pub primary_key: bool,
    /// Whether this column holds localizable string content.
    pub localizable: bool,
}

impl ColumnDef {
    /// Creates a new [`ColumnDef`].
    ///
    /// # Arguments
    ///
    /// * `name` - The column name.
    /// * `data_type` - Column data type.
    ///
    /// # Returns
    ///
    /// A new non-nullable, non-primary-key [`ColumnDef`].
    #[must_use]
    pub fn new(name: impl Into<String>, data_type: DataType) -> Self {
        Self {
            name: name.into(),
            data_type,
            nullable: false,
            primary_key: false,
            localizable: false,
        }
    }

    /// Marks this column as nullable.
    ///
    /// # Returns
    ///
    /// TODO: Document return value.
    #[must_use]
    pub const fn nullable(mut self) -> Self {
        self.nullable = true;
        self
    }

    /// Marks this column as part of the primary key.
    ///
    /// # Returns
    ///
    /// TODO: Document return value.
    #[must_use]
    pub const fn primary_key(mut self) -> Self {
        self.primary_key = true;
        self
    }

    /// Marks this column as localizable.
    ///
    /// # Returns
    ///
    /// TODO: Document return value.
    #[must_use]
    pub const fn localizable(mut self) -> Self {
        self.localizable = true;
        self
    }

    /// Encodes this column definition into its 16-bit MSI SDK type bitmask.
    ///
    /// # Returns
    ///
    /// 16-bit type bitmask integer.
    #[must_use]
    pub fn to_bitmask(&self) -> u16 {
        let mut mask = 0u16;

        if self.nullable {
            mask |= MSIDB_NULLABLE;
        }
        if self.primary_key {
            mask |= MSIDB_PRIMARY_KEY;
        }
        if self.localizable {
            mask |= MSIDB_LOCALIZABLE;
        }

        match self.data_type {
            DataType::Short => mask |= MSIDB_SHORT,
            DataType::Long => mask |= MSIDB_LONG,
            DataType::Stream => mask |= MSIDB_STREAM,
            DataType::String { max_len } => {
                mask |= MSIDB_STRING;
                mask |= u16::from(max_len);
            }
        }

        mask
    }

    /// Parses an MSI column type bitmask into a [`ColumnDef`].
    ///
    /// # Arguments
    ///
    /// * `name` - The column name.
    /// * `bitmask` - Raw 16-bit integer from `_Columns` table.
    ///
    /// # Returns
    ///
    /// A parsed [`ColumnDef`].
    ///
    /// # Errors
    ///
    /// Returns [`MsiError::InvalidColumnType`] if conflicting type flags are specified.
    pub fn from_bitmask(name: impl Into<String>, bitmask: u16) -> Result<Self> {
        let nullable = (bitmask & MSIDB_NULLABLE) != 0;
        let primary_key = (bitmask & MSIDB_PRIMARY_KEY) != 0;
        let localizable = (bitmask & MSIDB_LOCALIZABLE) != 0;

        let category = bitmask & 0x0D00;
        let data_type = if category == (MSIDB_SHORT & 0x0D00) && (bitmask & 0xFF) == 2 {
            DataType::Short
        } else if category == (MSIDB_LONG & 0x0D00) && (bitmask & 0xFF) == 4 {
            DataType::Long
        } else if category == (MSIDB_STREAM & 0x0D00) && bitmask.trailing_zeros() >= 8 {
            DataType::Stream
        } else if category == (MSIDB_STRING & 0x0D00)
            || category == 0x0800
            || category == 0x0400
            || category == 0x0000
        {
            DataType::String {
                max_len: (bitmask & 0x00FF) as u8,
            }
        } else {
            return Err(MsiError::InvalidColumnType { raw: bitmask });
        };
        Ok(Self {
            name: name.into(),
            data_type,
            nullable,
            primary_key,
            localizable,
        })
    }
}

impl fmt::Display for ColumnDef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let type_char = match self.data_type {
            DataType::Short => "i2",
            DataType::Long => "i4",
            DataType::Stream => "v0",
            DataType::String { max_len } => {
                if self.localizable {
                    if self.nullable {
                        return write!(f, "{}: L{max_len}", self.name);
                    }
                    return write!(f, "{}: l{max_len}", self.name);
                }
                if self.nullable {
                    return write!(f, "{}: S{max_len}", self.name);
                }
                return write!(f, "{}: s{max_len}", self.name);
            }
        };
        let null_flag = if self.nullable { "?" } else { "" };
        let pk_flag = if self.primary_key { " [PK]" } else { "" };
        write!(f, "{}: {type_char}{null_flag}{pk_flag}", self.name)
    }
}

#[cfg(test)]
mod tests {

    #[test]
    fn test_column_def_bitmask_stream_and_string_cases() {
        // Stream without trailing zeroes >= 8 shouldn't normally happen for a well-formed stream,
        // but if it does, it might fall through. Let's test the string fallbacks.
        // Stream is 0x0100.
        // We want to hit the string categories 0x0800, 0x0400, 0x0000.
        // String is 0x0C00.
        // 0x0800
        let col1 = ColumnDef::from_bitmask("Col1", 0x0800 | 0x0010).expect("test");
        assert_eq!(col1.data_type, DataType::String { max_len: 16 });

        // 0x0400
        let col2 = ColumnDef::from_bitmask("Col2", 0x0400 | 0x0020).expect("test");
        assert_eq!(col2.data_type, DataType::String { max_len: 32 });

        // 0x0000
        let col3 = ColumnDef::from_bitmask("Col3", 0x0030).expect("test");
        assert_eq!(col3.data_type, DataType::String { max_len: 48 });

        // A stream type 0x0100 where trailing zeros < 8 (e.g., bitmask & 0xFF != 0)
        // should fall through to the Error since it doesn't match string categories either.
        let err = ColumnDef::from_bitmask("BadStream", 0x0100 | 0x0001).unwrap_err();
        assert!(matches!(err, MsiError::InvalidColumnType { .. }));
    }
    use super::*;

    /// Tests column bitmask encoding and decoding roundtrips.
    #[test]
    fn test_column_def_bitmask_roundtrip() {
        assert_eq!(MSIDB_VALID_FLAGS, 0x0FFF);
        assert_eq!(MSIDB_ALL_FLAGS, 0x3FFF);
        assert_eq!(MSIDB_NULL, 0x0000);

        // Non-nullable, primary key string
        let col_pk = ColumnDef::new("Component", DataType::String { max_len: 72 }).primary_key();
        let mask_pk = col_pk.to_bitmask();
        assert_eq!(ColumnDef::from_bitmask("Component", mask_pk), Ok(col_pk));

        // Nullable, localizable string
        let col_loc = ColumnDef::new("Title", DataType::String { max_len: 64 })
            .nullable()
            .localizable();
        let mask_loc = col_loc.to_bitmask();
        assert_eq!(ColumnDef::from_bitmask("Title", mask_loc), Ok(col_loc));

        // Short integer
        let col_short = ColumnDef::new("Attributes", DataType::Short);
        let mask_short = col_short.to_bitmask();
        assert_eq!(
            ColumnDef::from_bitmask("Attributes", mask_short),
            Ok(col_short)
        );

        // Long integer
        let col_long = ColumnDef::new("FileSize", DataType::Long).nullable();
        let mask_long = col_long.to_bitmask();
        assert_eq!(ColumnDef::from_bitmask("FileSize", mask_long), Ok(col_long));

        // Stream
        let col_stream = ColumnDef::new("Data", DataType::Stream);
        let mask_stream = col_stream.to_bitmask();
        assert_eq!(ColumnDef::from_bitmask("Data", mask_stream), Ok(col_stream));

        // Conflicting type flags (both Short and Long)
        assert!(ColumnDef::from_bitmask("Bad", MSIDB_SHORT | MSIDB_LONG).is_err());
    }

    /// Tests record field size calculation.
    #[test]
    fn test_record_field_size() {
        assert_eq!(DataType::Short.record_field_size(2), 2);
        assert_eq!(DataType::Long.record_field_size(2), 4);
        assert_eq!(DataType::String { max_len: 72 }.record_field_size(2), 2);
        assert_eq!(DataType::String { max_len: 72 }.record_field_size(3), 3);
        assert_eq!(DataType::Stream.record_field_size(2), 2);
        assert_eq!(DataType::Stream.record_field_size(3), 3);
    }

    /// Tests Display formatting for various column definitions.
    #[test]
    fn test_column_def_display() {
        let c1 = ColumnDef::new("Col1", DataType::Short).primary_key();
        assert_eq!(format!("{c1}"), "Col1: i2 [PK]");

        let c2 = ColumnDef::new("Col2", DataType::Long).nullable();
        assert_eq!(format!("{c2}"), "Col2: i4?");

        let c3 = ColumnDef::new("Col3", DataType::Stream);
        assert_eq!(format!("{c3}"), "Col3: v0");

        let c4 = ColumnDef::new("Col4", DataType::String { max_len: 72 });
        assert_eq!(format!("{c4}"), "Col4: s72");

        let c5 = ColumnDef::new("Col5", DataType::String { max_len: 72 }).nullable();
        assert_eq!(format!("{c5}"), "Col5: S72");

        let c6 = ColumnDef::new("Col6", DataType::String { max_len: 255 }).localizable();
        assert_eq!(format!("{c6}"), "Col6: l255");

        let c7 = ColumnDef::new("Col7", DataType::String { max_len: 255 })
            .localizable()
            .nullable();
        assert_eq!(format!("{c7}"), "Col7: L255");
    }
}

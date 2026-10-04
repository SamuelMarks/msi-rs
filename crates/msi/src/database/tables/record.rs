//! Generic table record representation and binary serialization / deserialization.
//!
//! Conforms directly to the Windows Installer SDK physical table stream layout.

use crate::database::column::{ColumnDef, DataType};
use crate::database::physical::{PhysicalIndex, PhysicalTableLayout};
use crate::database::string_pool::StringPool;
use crate::database::tables::types::StringPoolId;
use crate::error::{MsiError, Result};
use std::fmt;

/// Sentinels representing `NULL` integers in Windows Installer SDK (`MsiRecordGetInteger`).
pub const MSI_NULL_INTEGER_16: i16 = i16::MIN;

/// 32-bit Sentinel representing `NULL` integer (`0x80000000` / `-2147483648`).
pub const MSI_NULL_INTEGER_32: i32 = i32::MIN;

/// Mask used to serialize and deserialize MSI short integers.
///
/// According to the Windows Installer specification, valid short integers (16-bit)
/// are transformed by applying an XOR mask of `0x8000`. The physical value `0x0000`
/// is reserved explicitly to represent a `NULL` short integer, meaning the valid
/// physical range for values shifts. This avoids collisions with the `NULL` sentinel.
pub const MSI_SHORT_INT_MASK: u16 = 0x8000;

/// Strongly-typed field value in an MSI database table record.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum FieldValue {
    /// 16-bit integer (i2 / I2).
    Short(i16),
    /// 32-bit integer (i4 / I4).
    Long(i32),
    /// String value (s / S / l / L).
    String(String),
    /// Stream identifier (v / V).
    Stream(StringPoolId),
    /// Null field value.
    Null,
}

impl FieldValue {
    /// Returns whether this field is [`FieldValue::Null`].
    ///
    /// # Returns
    ///
    /// `true` if null, `false` otherwise.
    #[must_use]
    pub const fn is_null(&self) -> bool {
        matches!(self, Self::Null)
    }
}

impl fmt::Display for FieldValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Short(v) => write!(f, "{v}"),
            Self::Long(v) => write!(f, "{v}"),
            Self::String(s) => write!(f, "'{s}'"),
            Self::Stream(id) => write!(f, "{id}"),
            Self::Null => write!(f, "NULL"),
        }
    }
}

/// Generic record containing an ordered list of fields corresponding to a table's columns.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Record {
    /// Ordered vector of field values in this record.
    fields: Vec<FieldValue>,
}

impl Record {
    /// Creates a new empty [`Record`].
    ///
    /// # Returns
    ///
    /// An empty [`Record`].
    #[must_use]
    pub const fn new() -> Self {
        Self { fields: Vec::new() }
    }

    /// Creates a [`Record`] with the specified fields.
    ///
    /// # Arguments
    ///
    /// * `fields` - Vector of [`FieldValue`].
    ///
    /// # Returns
    ///
    /// A new [`Record`].
    #[must_use]
    pub const fn with_fields(fields: Vec<FieldValue>) -> Self {
        Self { fields }
    }

    /// Adds a field to the record.
    ///
    /// # Arguments
    ///
    /// * `field` - Field value to append.
    pub fn push(&mut self, field: FieldValue) {
        self.fields.push(field);
    }

    /// Returns a slice of the fields in this record.
    ///
    /// # Returns
    ///
    /// Slice of [`FieldValue`].
    #[must_use]
    pub fn fields(&self) -> &[FieldValue] {
        &self.fields
    }

    /// Returns a mutable slice of the fields in this record.
    ///
    /// # Returns
    ///
    /// Mutable slice of [`FieldValue`].
    #[must_use]
    pub fn fields_mut(&mut self) -> &mut [FieldValue] {
        &mut self.fields
    }
    #[allow(clippy::missing_errors_doc)]
    /// Deserializes a single field from binary format.
    pub fn deserialize_field(
        bytes: &[u8],
        col: &ColumnDef,
        pool: &StringPool,
        string_index_size: usize,
    ) -> Result<FieldValue> {
        match col.data_type {
            DataType::Short => {
                let raw = u16::from_le_bytes([bytes[0], bytes[1]]);
                if raw == 0x0000 {
                    Ok(FieldValue::Null)
                } else {
                    #[allow(clippy::cast_possible_wrap)]
                    Ok(FieldValue::Short((raw ^ MSI_SHORT_INT_MASK) as i16))
                }
            }
            DataType::Long => {
                let val = i32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
                if val == MSI_NULL_INTEGER_32 {
                    Ok(FieldValue::Null)
                } else {
                    Ok(FieldValue::Long(val))
                }
            }
            DataType::Stream => {
                let raw = u16::from_le_bytes([bytes[0], bytes[1]]);
                if raw == 0 {
                    Ok(FieldValue::Null)
                } else {
                    Ok(FieldValue::Stream(StringPoolId::new(u32::from(raw))))
                }
            }
            DataType::String { .. } => {
                let raw = if string_index_size == 2 {
                    u32::from(u16::from_le_bytes([bytes[0], bytes[1]]))
                } else {
                    u32::from_le_bytes([bytes[0], bytes[1], bytes[2], 0])
                };
                if raw == 0 {
                    Ok(FieldValue::Null)
                } else {
                    let s = pool
                        .get_string(raw)
                        .map_err(|_| MsiError::DataIntegrityError {
                            reason: format!("invalid string pool reference: {raw}"),
                        })?;
                    Ok(FieldValue::String(s.to_string()))
                }
            }
        }
    }

    /// Retrieves a field value by 0-based index.
    ///
    /// # Arguments
    ///
    /// * `idx` - 0-based index.
    ///
    /// # Returns
    ///
    /// Optional reference to [`FieldValue`].
    #[must_use]
    pub fn get(&self, idx: usize) -> Option<&FieldValue> {
        self.fields.get(idx)
    }

    /// Sets the value of a field at the given 0-based index.
    ///
    /// # Arguments
    ///
    /// * `idx` - 0-based field index.
    /// * `val` - New [`FieldValue`].
    pub fn set(&mut self, idx: usize, val: FieldValue) {
        if idx < self.fields.len() {
            self.fields[idx] = val;
        }
    }

    /// Returns the number of fields in this record.
    ///
    /// # Returns
    ///
    /// Count of fields.
    #[must_use]
    pub fn len(&self) -> usize {
        self.fields.len()
    }

    /// Returns whether this record has no fields.
    ///
    /// # Returns
    ///
    /// `true` if empty, `false` otherwise.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.fields.is_empty()
    }

    /// Validates this record against a slice of column definitions.
    ///
    /// Checks:
    /// - Field count matches column count.
    /// - Non-nullable columns do not contain [`FieldValue::Null`].
    /// - String values do not exceed maximum defined length.
    /// - Field types match column data types.
    ///
    /// # Arguments
    ///
    /// * `table_name` - Name of table for error reporting.
    /// * `columns` - Column definitions slice.
    ///
    /// # Errors
    ///
    /// Returns [`MsiError::RecordLengthMismatch`] or [`MsiError::Validation`] on violation.
    pub fn validate(&self, table_name: &str, columns: &[ColumnDef]) -> Result<()> {
        if self.fields.len() != columns.len() {
            return Err(MsiError::RecordLengthMismatch {
                expected: columns.len(),
                actual: self.fields.len(),
            });
        }

        for (i, (field, col)) in self.fields.iter().zip(columns.iter()).enumerate() {
            if !col.nullable && field.is_null() {
                return Err(MsiError::Validation {
                    element: format!("{table_name}.{}", col.name),
                    reason: format!("column {} is not nullable, but field {i} is NULL", col.name),
                });
            }

            match (field, col.data_type) {
                (FieldValue::Short(_), DataType::Short)
                | (FieldValue::Long(_), DataType::Long)
                | (FieldValue::Stream(_) | FieldValue::String(_), DataType::Stream)
                | (FieldValue::Null, _) => {}
                (FieldValue::String(ref s), DataType::String { max_len }) => {
                    if max_len > 0 && s.len() > usize::from(max_len) {
                        return Err(MsiError::Validation {
                            element: format!("{table_name}.{}", col.name),
                            reason: format!(
                                "string length {} exceeds maximum allowed length of {} for column {}",
                                s.len(),
                                max_len,
                                col.name
                            ),
                        });
                    }
                }
                _ => {
                    return Err(MsiError::Validation {
                        element: format!("{table_name}.{}", col.name),
                        reason: format!(
                            "field type mismatch for column {}: expected {:?}, got {:?}",
                            col.name, col.data_type, field
                        ),
                    });
                }
            }
        }

        Ok(())
    }

    /// Serializes this record into raw binary bytes using a string pool.
    ///
    /// # Arguments
    ///
    /// * `layout` - Physical table layout for field ordering.
    /// * `pool` - String pool used to intern strings and obtain 1-based indices.
    /// * `string_index_size` - Size of string pool index (2 or 3 bytes).
    ///
    /// # Returns
    ///
    /// Serialized byte vector for this record row.
    ///
    /// # Errors
    ///
    /// Returns [`MsiError::RecordLengthMismatch`] if field count does not match column count.
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    pub fn serialize(
        &self,
        layout: &PhysicalTableLayout<'_>,
        pool: &mut StringPool,
        string_index_size: usize,
    ) -> Result<Vec<u8>> {
        let columns = layout.columns();
        if self.fields.len() != columns.len() {
            return Err(MsiError::RecordLengthMismatch {
                expected: columns.len(),
                actual: self.fields.len(),
            });
        }

        let mut out = Vec::new();

        for physical_idx in 0..columns.len() {
            let logical_idx = layout
                .logical_index(PhysicalIndex(physical_idx))
                .unwrap_or(crate::database::physical::LogicalIndex(0));
            let col = &columns[logical_idx.0];
            let field = &self.fields[logical_idx.0];

            match col.data_type {
                DataType::Short => {
                    let val = match field {
                        FieldValue::Short(s) => (*s as u16) ^ MSI_SHORT_INT_MASK,
                        FieldValue::Null => 0x0000,
                        FieldValue::Long(l) => {
                            if *l > i32::from(i16::MAX) || *l < i32::from(i16::MIN) {
                                return Err(MsiError::DataIntegrityError {
                                    reason: "integer exceeds bounds for short".into(),
                                });
                            }
                            (*l as u16) ^ MSI_SHORT_INT_MASK
                        }
                        _ => {
                            return Err(MsiError::DataIntegrityError {
                                reason: format!("expected short integer, got {field:?}"),
                            });
                        }
                    };
                    out.extend_from_slice(&val.to_le_bytes());
                }
                DataType::Long => {
                    let val = match field {
                        FieldValue::Long(l) => *l,
                        FieldValue::Null => MSI_NULL_INTEGER_32,
                        FieldValue::Short(s) => i32::from(*s),
                        _ => {
                            return Err(MsiError::DataIntegrityError {
                                reason: format!("expected long integer, got {field:?}"),
                            });
                        }
                    };
                    out.extend_from_slice(&(val as u32).to_le_bytes());
                }
                DataType::String { .. } | DataType::Stream => {
                    let str_id = match field {
                        FieldValue::String(ref s) => pool.add_string(s),
                        FieldValue::Stream(id) => id.get(),
                        _ => 0,
                    };

                    if string_index_size == 3 {
                        let b = str_id.to_le_bytes();
                        out.extend_from_slice(&b[0..3]);
                    } else {
                        out.extend_from_slice(&(str_id as u16).to_le_bytes());
                    }
                }
            }
        }

        Ok(out)
    }

    /// Deserializes a record from raw binary bytes.
    ///
    /// # Arguments
    ///
    /// * `bytes` - Slice containing record bytes.
    /// * `layout` - Physical table layout.
    /// * `pool` - String pool used to resolve string indices.
    /// * `string_index_size` - Size of string pool index (2 or 3 bytes).
    ///
    /// # Returns
    ///
    /// Reconstructed [`Record`].
    ///
    /// # Errors
    ///
    /// Returns [`MsiError::RecordLengthMismatch`] or string pool retrieval errors.
    #[allow(clippy::cast_possible_wrap)]
    pub fn deserialize(
        bytes: &[u8],
        layout: &PhysicalTableLayout<'_>,
        pool: &StringPool,
        string_index_size: usize,
    ) -> Result<Self> {
        let columns = layout.columns();
        let expected_size: usize = columns
            .iter()
            .map(|c| c.data_type.record_field_size(string_index_size))
            .sum();

        if bytes.len() < expected_size {
            return Err(MsiError::RecordLengthMismatch {
                expected: expected_size,
                actual: bytes.len(),
            });
        }

        let mut logical_fields = vec![FieldValue::Null; columns.len()];
        let mut cursor = 0;

        for physical_idx in 0..columns.len() {
            let logical_idx = layout
                .logical_index(PhysicalIndex(physical_idx))
                .unwrap_or(crate::database::physical::LogicalIndex(0));
            let col = &columns[logical_idx.0];

            let field_val = match col.data_type {
                DataType::Short => {
                    let raw = u16::from_le_bytes([bytes[cursor], bytes[cursor + 1]]);
                    cursor += 2;
                    if raw == 0x0000 {
                        FieldValue::Null
                    } else {
                        FieldValue::Short((raw ^ MSI_SHORT_INT_MASK) as i16)
                    }
                }
                DataType::Long => {
                    let val = i32::from_le_bytes([
                        bytes[cursor],
                        bytes[cursor + 1],
                        bytes[cursor + 2],
                        bytes[cursor + 3],
                    ]);
                    cursor += 4;
                    if col.nullable && val == MSI_NULL_INTEGER_32 {
                        FieldValue::Null
                    } else {
                        FieldValue::Long(val)
                    }
                }
                DataType::String { .. } => {
                    let str_id = if string_index_size == 3 {
                        let id = u32::from_le_bytes([
                            bytes[cursor],
                            bytes[cursor + 1],
                            bytes[cursor + 2],
                            0,
                        ]);
                        cursor += 3;
                        id
                    } else {
                        let id = u16::from_le_bytes([bytes[cursor], bytes[cursor + 1]]);
                        cursor += 2;
                        u32::from(id)
                    };

                    if str_id == 0 {
                        if col.nullable {
                            FieldValue::Null
                        } else {
                            FieldValue::String(String::new())
                        }
                    } else {
                        let s = pool.get_string(str_id)?;
                        FieldValue::String(s.to_string())
                    }
                }
                DataType::Stream => {
                    let str_id = if string_index_size == 3 {
                        let id = u32::from_le_bytes([
                            bytes[cursor],
                            bytes[cursor + 1],
                            bytes[cursor + 2],
                            0,
                        ]);
                        cursor += 3;
                        id
                    } else {
                        let id = u16::from_le_bytes([bytes[cursor], bytes[cursor + 1]]);
                        cursor += 2;
                        u32::from(id)
                    };

                    if str_id == 0 {
                        FieldValue::Null
                    } else {
                        FieldValue::Stream(StringPoolId::new(str_id))
                    }
                }
            };

            logical_fields[logical_idx.0] = field_val;
        }

        Ok(Self {
            fields: logical_fields,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::database::string_pool::CODEPAGE_UTF8;

    /// Helper to serialize a record and return its bytes, or empty vector on error.
    ///
    /// # Arguments
    ///
    /// * `rec` - Record to serialize.
    /// * `cols` - Column definitions.
    /// * `pool` - String pool.
    /// * `str_bytes` - String id byte width (2 or 3).
    ///
    /// # Returns
    ///
    /// Vector of bytes on success, or empty vector on serialization failure.
    #[allow(clippy::option_if_let_else, clippy::manual_unwrap_or_default)]
    fn try_serialize(
        rec: &Record,
        cols: &[ColumnDef],
        pool: &mut StringPool,
        str_bytes: usize,
    ) -> Vec<u8> {
        let Ok(layout) = PhysicalTableLayout::new(cols, false) else {
            return Vec::new();
        };
        match rec.serialize(&layout, pool, str_bytes) {
            Ok(b) => b,
            Err(_) => Vec::new(),
        }
    }

    /// Helper to deserialize a record and return a vector containing it, or empty vector on error.
    ///
    /// # Arguments
    ///
    /// * `bytes` - Serialized bytes.
    /// * `cols` - Column definitions.
    /// * `pool` - String pool.
    /// * `str_bytes` - String id byte width.
    ///
    /// # Returns
    ///
    /// Vector containing [`Record`] on success, or empty vector on failure.
    #[allow(clippy::option_if_let_else)]
    fn try_deserialize(
        bytes: &[u8],
        cols: &[ColumnDef],
        pool: &StringPool,
        str_bytes: usize,
    ) -> Vec<Record> {
        let Ok(layout) = PhysicalTableLayout::new(cols, false) else {
            return Vec::new();
        };
        match Record::deserialize(bytes, &layout, pool, str_bytes) {
            Ok(r) => vec![r],
            Err(_) => Vec::new(),
        }
    }

    /// Tests serialization and deserialization using divergent physical and logical layout.
    #[test]
    fn test_record_layout_reordering() {
        let mut pool = StringPool::new(CODEPAGE_UTF8);

        // Example schema where physical != logical:
        // Logically:
        // 0: Value (String)
        // 1: Name (String) [PK]
        // 2: Id (Short)
        let cols = vec![
            ColumnDef::new("Value", DataType::String { max_len: 255 }).nullable(),
            ColumnDef::new("Name", DataType::String { max_len: 72 }).primary_key(),
            ColumnDef::new("Id", DataType::Short),
        ];

        // Physically:
        // 0: Name (String) [PK] -> Logical 1
        // 1: Id (Short) -> Logical 2
        // 2: Value (String) -> Logical 0

        let rec = Record::with_fields(vec![
            FieldValue::String("MyValue".to_string()), // Logical 0
            FieldValue::String("MyName".to_string()),  // Logical 1
            FieldValue::Short(123),                    // Logical 2
        ]);

        let bytes = try_serialize(&rec, &cols, &mut pool, 2);

        // Calculate expected bytes:
        // Name string id = 1 (inserted first because physical iteration hits it first)
        // Id short = 123 ^ 0x8000 = 0x807B -> 7B 80
        // Value string id = 2

        let expected_bytes = vec![
            0x01, 0x00, // Name (str id = 1)
            0x7B, 0x80, // Id (short = 123)
            0x02, 0x00, // Value (str id = 2)
        ];

        assert_eq!(bytes, expected_bytes);

        let des_rec = try_deserialize(&bytes, &cols, &pool, 2);
        assert_eq!(des_rec.len(), 1);

        // Fields should be back in LOGICAL order!
        assert_eq!(
            des_rec[0].fields()[0],
            FieldValue::String("MyValue".to_string())
        );
        assert_eq!(
            des_rec[0].fields()[1],
            FieldValue::String("MyName".to_string())
        );
        assert_eq!(des_rec[0].fields()[2], FieldValue::Short(123));
    }

    /// Tests basic record mutations and schema validation.
    #[test]
    fn test_record_basic_and_validation() {
        let mut r = Record::new();
        assert!(r.is_empty());
        r.push(FieldValue::String("Comp1".to_string()));
        r.push(FieldValue::Short(1));
        r.push(FieldValue::Null);
        assert_eq!(r.len(), 3);
        assert!(!r.is_empty());
        assert_eq!(r.get(0), Some(&FieldValue::String("Comp1".to_string())));
        assert_eq!(r.get(3), None);

        let cols = vec![
            ColumnDef::new("Name", DataType::String { max_len: 10 }),
            ColumnDef::new("Attr", DataType::Short),
            ColumnDef::new("Desc", DataType::String { max_len: 50 }).nullable(),
        ];

        assert_eq!(r.validate("TestTable", &cols), Ok(()));

        // Mismatched length
        let bad_cols = vec![ColumnDef::new("Name", DataType::Short)];
        assert!(r.validate("TestTable", &bad_cols).is_err());

        // Null in non-nullable column
        let non_null_cols = vec![
            ColumnDef::new("Name", DataType::String { max_len: 10 }),
            ColumnDef::new("Attr", DataType::Short),
            ColumnDef::new("Desc", DataType::String { max_len: 50 }), // Non-nullable!
        ];
        assert!(r.validate("TestTable", &non_null_cols).is_err());

        // String length exceeded
        let too_short_cols = vec![
            ColumnDef::new("Name", DataType::String { max_len: 2 }),
            ColumnDef::new("Attr", DataType::Short),
            ColumnDef::new("Desc", DataType::String { max_len: 50 }).nullable(),
        ];
        assert!(r.validate("TestTable", &too_short_cols).is_err());

        // Type mismatch
        let type_mismatch_cols = vec![
            ColumnDef::new("Name", DataType::Long),
            ColumnDef::new("Attr", DataType::Short),
            ColumnDef::new("Desc", DataType::String { max_len: 50 }).nullable(),
        ];
        assert!(r.validate("TestTable", &type_mismatch_cols).is_err());
    }

    /// Tests 2-byte string pool ID serialization and deserialization roundtrip.
    #[test]
    fn test_record_roundtrip_2byte() {
        let mut pool = StringPool::new(CODEPAGE_UTF8);
        let cols = vec![
            ColumnDef::new("ColShort", DataType::Short),
            ColumnDef::new("ColLong", DataType::Long),
            ColumnDef::new("ColStr", DataType::String { max_len: 64 }),
            ColumnDef::new("ColNullShort", DataType::Short).nullable(),
            ColumnDef::new("ColNullLong", DataType::Long).nullable(),
            ColumnDef::new("ColNullStr", DataType::String { max_len: 64 }).nullable(),
            ColumnDef::new("ColNullShortVal", DataType::Short).nullable(),
            ColumnDef::new("ColNullLongVal", DataType::Long).nullable(),
            ColumnDef::new("ColStream", DataType::Stream),
        ];

        assert!(try_serialize(&Record::new(), &cols, &mut pool, 2).is_empty());
        assert!(try_deserialize(&[], &cols, &pool, 2).is_empty());

        let stream_id = pool.add_string("DataStreamName");

        let r = Record::with_fields(vec![
            FieldValue::Short(42),
            FieldValue::Long(100_000),
            FieldValue::String("Hello".to_string()),
            FieldValue::Null,
            FieldValue::Null,
            FieldValue::Null,
            FieldValue::Short(77),
            FieldValue::Long(888),
            FieldValue::Stream(StringPoolId::new(stream_id)),
        ]);

        let bytes = try_serialize(&r, &cols, &mut pool, 2);
        assert!(!bytes.is_empty());
        let des_res = try_deserialize(&bytes, &cols, &pool, 2);
        assert_eq!(des_res, vec![r]);
    }

    /// Tests 3-byte string pool ID serialization and deserialization roundtrip.
    #[test]
    fn test_record_roundtrip_3byte() {
        let mut pool = StringPool::new(CODEPAGE_UTF8);
        let cols = vec![
            ColumnDef::new("ColStr", DataType::String { max_len: 0 }),
            ColumnDef::new("ColStream", DataType::Stream),
        ];

        let stream_id = pool.add_string("BigStream");
        let r = Record::with_fields(vec![
            FieldValue::String("World".to_string()),
            FieldValue::Stream(StringPoolId::new(stream_id)),
        ]);

        let bytes = try_serialize(&r, &cols, &mut pool, 3);
        assert!(!bytes.is_empty());
        let des_res = try_deserialize(&bytes, &cols, &pool, 3);
        assert_eq!(des_res, vec![r]);
    }

    /// Tests [`FieldValue`] display formatting and helper methods.
    #[test]
    fn test_field_value_display_and_helpers() {
        assert_eq!(format!("{}", FieldValue::Short(10)), "10");
        assert_eq!(format!("{}", FieldValue::Long(20)), "20");
        assert_eq!(
            format!("{}", FieldValue::String("abc".to_string())),
            "'abc'"
        );
        assert_eq!(
            format!("{}", FieldValue::Stream(StringPoolId::new(1))),
            "StringPool#1"
        );
        assert_eq!(format!("{}", FieldValue::Null), "NULL");
        assert!(FieldValue::Null.is_null());
        assert!(!FieldValue::Short(5).is_null());
    }

    /// Tests record error cases and boundary conditions.
    #[test]
    fn test_record_errors() {
        let mut pool = StringPool::new(CODEPAGE_UTF8);
        let cols = vec![ColumnDef::new("Col1", DataType::Short)];
        let layout = PhysicalTableLayout::new(&cols, false).unwrap();
        let r = Record::new(); // 0 fields, but 1 column expected
        assert_eq!(r.fields().len(), 0);
        assert!(r.serialize(&layout, &mut pool, 2).is_err());
        assert!(Record::deserialize(&[], &layout, &pool, 2).is_err());

        // Test non-nullable string with str_id 0 and stream with str_id 0
        let zero_cols = vec![
            ColumnDef::new("NonNullableStr", DataType::String { max_len: 10 }), // non-nullable!
            ColumnDef::new("NullableStream", DataType::Stream),
            ColumnDef::new("ShortFallback", DataType::Short),
            ColumnDef::new("LongFallback", DataType::Long),
        ];

        // bytes with: 0 (str_id 0), 0 (stream_id 0), short 0, long 0
        let mut raw_bytes = Vec::new();
        raw_bytes.extend_from_slice(&0u16.to_le_bytes()); // string id 0
        raw_bytes.extend_from_slice(&0u16.to_le_bytes()); // stream id 0
        raw_bytes.extend_from_slice(&0u16.to_le_bytes()); // short
        raw_bytes.extend_from_slice(&0u32.to_le_bytes()); // long

        let des_zero = try_deserialize(&raw_bytes, &zero_cols, &pool, 2);
        for rec in des_zero {
            assert_eq!(rec.get(0), Some(&FieldValue::String(String::new())));
            assert_eq!(rec.get(1), Some(&FieldValue::Null));
        }

        // Test fallback serialization when mismatched field types provided
        let fallback_rec = Record::with_fields(vec![
            FieldValue::Short(1),                  // Mismatched field for String
            FieldValue::Long(2),                   // Mismatched field for Stream
            FieldValue::String("NaN".to_string()), // Mismatched field for Short
            FieldValue::String("NaN".to_string()), // Mismatched field for Long
        ]);
        let ser_fallback = try_serialize(&fallback_rec, &zero_cols, &mut pool, 2);
        assert_eq!(ser_fallback.len(), 0); // Serialization now fails correctly

        // Test Record::set and Record::fields_mut
        let mut test_rec = Record::with_fields(vec![FieldValue::Short(10), FieldValue::Short(20)]);
        test_rec.set(0, FieldValue::Long(100));
        test_rec.set(5, FieldValue::Null); // out of bounds check
        assert_eq!(test_rec.get(0), Some(&FieldValue::Long(100)));
        test_rec.fields_mut()[1] = FieldValue::String("mutated".to_string());
        assert_eq!(
            test_rec.get(1),
            Some(&FieldValue::String("mutated".to_string()))
        );
    }

    /// Tests short integer serialization boundaries and hex output.
    #[test]
    fn test_short_integer_serialization_exhaustive() {
        let mut pool = StringPool::new(CODEPAGE_UTF8);
        let cols = vec![ColumnDef::new("Col1", DataType::Short)];

        let cases = vec![
            (FieldValue::Short(0), vec![0x00, 0x80]),
            (FieldValue::Short(1), vec![0x01, 0x80]),
            (FieldValue::Short(32767), vec![0xFF, 0xFF]),
            (FieldValue::Short(-1), vec![0xFF, 0x7F]),
            (FieldValue::Short(i16::MIN), vec![0x00, 0x00]), // -32768 is physically identical to NULL
            (FieldValue::Null, vec![0x00, 0x00]),
        ];

        for (field_val, expected_bytes) in cases {
            let r = Record::with_fields(vec![field_val.clone()]);
            let bytes = try_serialize(&r, &cols, &mut pool, 2);
            assert_eq!(
                bytes, expected_bytes,
                "Serialization failed for {field_val:?}",
            );

            let des_res = try_deserialize(&bytes, &cols, &pool, 2);
            assert_eq!(des_res.len(), 1);
            let des_field = des_res[0].get(0).unwrap();

            // i16::MIN and Null both deserialize back to Null
            let expected_des = if field_val == FieldValue::Short(i16::MIN) {
                FieldValue::Null
            } else {
                field_val.clone()
            };
            assert_eq!(
                des_field, &expected_des,
                "Deserialization failed for {field_val:?}",
            );
        }
    }

    /// Tests integer data integrity errors during serialization.
    #[test]
    fn test_integer_data_integrity() {
        let mut pool = StringPool::new(CODEPAGE_UTF8);

        let cols_short = vec![ColumnDef::new("Col1", DataType::Short)];
        let layout_short = PhysicalTableLayout::new(&cols_short, false).unwrap();

        let r_out_of_bounds = Record::with_fields(vec![FieldValue::Long(32768)]);
        let res = r_out_of_bounds.serialize(&layout_short, &mut pool, 2);
        assert!(matches!(res, Err(MsiError::DataIntegrityError { .. })));

        let r_out_of_bounds2 = Record::with_fields(vec![FieldValue::Long(-32769)]);
        let res2 = r_out_of_bounds2.serialize(&layout_short, &mut pool, 2);
        assert!(matches!(res2, Err(MsiError::DataIntegrityError { .. })));

        let r_wrong_type = Record::with_fields(vec![FieldValue::String("NaN".to_string())]);
        let res3 = r_wrong_type.serialize(&layout_short, &mut pool, 2);
        assert!(matches!(res3, Err(MsiError::DataIntegrityError { .. })));

        let cols_long = vec![ColumnDef::new("Col1", DataType::Long)];
        let layout_long = PhysicalTableLayout::new(&cols_long, false).unwrap();

        let r_wrong_type_long = Record::with_fields(vec![FieldValue::String("NaN".to_string())]);
        let res4 = r_wrong_type_long.serialize(&layout_long, &mut pool, 2);
        assert!(matches!(res4, Err(MsiError::DataIntegrityError { .. })));
    }

    #[test]
    fn test_record_extra_coverage() {
        let mut pool = StringPool::new(CODEPAGE_UTF8);
        let cols = vec![ColumnDef::new("Col1", DataType::Short)];
        let layout = PhysicalTableLayout::new(&cols, false).unwrap();

        // Line 374-375
        assert!(Record::deserialize(&[0; 0], &layout, &pool, 2).is_err());

        // Line 487: Stream str_id == 0 -> Null
        let cols_stream = vec![ColumnDef::new("Col1", DataType::Stream)];
        let layout_stream = PhysicalTableLayout::new(&cols_stream, false).unwrap();
        let des = Record::deserialize(&[0, 0], &layout_stream, &pool, 2).unwrap();
        assert_eq!(des.fields()[0], FieldValue::Null);

        // try_serialize fail
        assert_eq!(
            try_serialize(
                &Record::with_fields(vec![FieldValue::Long(32768)]),
                &cols,
                &mut pool,
                2
            )
            .len(),
            0
        );

        // Lines 285-286: Long to Short valid
        let r_long_to_short = Record::with_fields(vec![FieldValue::Long(123)]);
        let serialized_short = r_long_to_short.serialize(&layout, &mut pool, 2).unwrap();
        assert_eq!(serialized_short, vec![0x7B, 0x80]);

        // Line 300: Short to Long valid
        let cols_long = vec![ColumnDef::new("Col1", DataType::Long)];
        let layout_long = PhysicalTableLayout::new(&cols_long, false).unwrap();
        let r_short_to_long = Record::with_fields(vec![FieldValue::Short(123)]);
        let serialized_long = r_short_to_long
            .serialize(&layout_long, &mut pool, 2)
            .unwrap();
        assert_eq!(serialized_long, vec![123, 0, 0, 0]);

        // Lines 483 and 511: try_serialize and try_deserialize with invalid cols
        let bad_cols = vec![ColumnDef::new("BadCol", DataType::Stream).primary_key()];
        assert_eq!(
            try_serialize(&Record::new(), &bad_cols, &mut pool, 2),
            Vec::<u8>::new()
        );
        assert_eq!(
            try_deserialize(&[], &bad_cols, &pool, 2),
            Vec::<Record>::new()
        );
    }
}

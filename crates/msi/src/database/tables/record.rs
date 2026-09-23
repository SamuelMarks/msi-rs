//! Generic table record representation and binary serialization / deserialization.
//!
//! Conforms directly to the Windows Installer SDK physical table stream layout.

use crate::database::column::{ColumnDef, DataType};
use crate::database::string_pool::StringPool;
use crate::database::tables::types::StringPoolId;
use crate::error::{Error, Result};
use std::fmt;

/// Sentinels representing `NULL` integers in Windows Installer SDK (`MsiRecordGetInteger`).
pub const MSI_NULL_INTEGER_16: i16 = i16::MIN;

/// 32-bit Sentinel representing `NULL` integer (`0x80000000` / `-2147483648`).
pub const MSI_NULL_INTEGER_32: i32 = i32::MIN;

/// Strongly-typed field value in an MSI database table record.
#[derive(Debug, Clone, PartialEq, Eq)]
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

    /// Retrieves a mutable reference to all field values.
    ///
    /// # Returns
    ///
    /// Mutable slice of [`FieldValue`].
    pub fn fields_mut(&mut self) -> &mut [FieldValue] {
        &mut self.fields
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
    /// Returns [`Error::RecordLengthMismatch`] or [`Error::Validation`] on violation.
    pub fn validate(&self, table_name: &str, columns: &[ColumnDef]) -> Result<()> {
        if self.fields.len() != columns.len() {
            return Err(Error::RecordLengthMismatch {
                expected: columns.len(),
                actual: self.fields.len(),
            });
        }

        for (i, (field, col)) in self.fields.iter().zip(columns.iter()).enumerate() {
            if !col.nullable && field.is_null() {
                return Err(Error::Validation {
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
                        return Err(Error::Validation {
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
                    return Err(Error::Validation {
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
    /// * `columns` - Table column definitions.
    /// * `pool` - String pool used to intern strings and obtain 1-based indices.
    /// * `string_index_size` - Size of string pool index (2 or 3 bytes).
    ///
    /// # Returns
    ///
    /// Serialized byte vector for this record row.
    ///
    /// # Errors
    ///
    /// Returns [`Error::RecordLengthMismatch`] if field count does not match column count.
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    pub fn serialize(
        &self,
        columns: &[ColumnDef],
        pool: &mut StringPool,
        string_index_size: usize,
    ) -> Result<Vec<u8>> {
        if self.fields.len() != columns.len() {
            return Err(Error::RecordLengthMismatch {
                expected: columns.len(),
                actual: self.fields.len(),
            });
        }

        let mut out = Vec::new();

        for (field, col) in self.fields.iter().zip(columns.iter()) {
            match col.data_type {
                DataType::Short => {
                    let val = match field {
                        FieldValue::Short(s) => *s,
                        FieldValue::Null => MSI_NULL_INTEGER_16,
                        _ => 0,
                    };
                    out.extend_from_slice(&(val as u16).to_le_bytes());
                }
                DataType::Long => {
                    let val = match field {
                        FieldValue::Long(l) => *l,
                        FieldValue::Null => MSI_NULL_INTEGER_32,
                        _ => 0,
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
    /// * `columns` - Table column definitions.
    /// * `pool` - String pool used to resolve string indices.
    /// * `string_index_size` - Size of string pool index (2 or 3 bytes).
    ///
    /// # Returns
    ///
    /// Reconstructed [`Record`].
    ///
    /// # Errors
    ///
    /// Returns [`Error::RecordLengthMismatch`] or string pool retrieval errors.
    pub fn deserialize(
        bytes: &[u8],
        columns: &[ColumnDef],
        pool: &StringPool,
        string_index_size: usize,
    ) -> Result<Self> {
        let expected_size: usize = columns
            .iter()
            .map(|c| c.data_type.record_field_size(string_index_size))
            .sum();

        if bytes.len() < expected_size {
            return Err(Error::RecordLengthMismatch {
                expected: expected_size,
                actual: bytes.len(),
            });
        }

        let mut fields = Vec::with_capacity(columns.len());
        let mut cursor = 0;

        for col in columns {
            match col.data_type {
                DataType::Short => {
                    let val = i16::from_le_bytes([bytes[cursor], bytes[cursor + 1]]);
                    cursor += 2;
                    if col.nullable && val == MSI_NULL_INTEGER_16 {
                        fields.push(FieldValue::Null);
                    } else {
                        fields.push(FieldValue::Short(val));
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
                        fields.push(FieldValue::Null);
                    } else {
                        fields.push(FieldValue::Long(val));
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
                            fields.push(FieldValue::Null);
                        } else {
                            fields.push(FieldValue::String(String::new()));
                        }
                    } else {
                        let s = pool.get_string(str_id)?;
                        fields.push(FieldValue::String(s.to_string()));
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
                        fields.push(FieldValue::Null);
                    } else {
                        fields.push(FieldValue::Stream(StringPoolId::new(str_id)));
                    }
                }
            }
        }

        Ok(Self { fields })
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
        match rec.serialize(cols, pool, str_bytes) {
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
        match Record::deserialize(bytes, cols, pool, str_bytes) {
            Ok(r) => vec![r],
            Err(_) => Vec::new(),
        }
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
        let r = Record::new(); // 0 fields, but 1 column expected
        assert_eq!(r.fields().len(), 0);
        assert!(r.serialize(&cols, &mut pool, 2).is_err());
        assert!(Record::deserialize(&[], &cols, &pool, 2).is_err());

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
        assert_eq!(ser_fallback.len(), 10);

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
}

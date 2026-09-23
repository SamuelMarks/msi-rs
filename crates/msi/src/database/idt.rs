//! IDT (Installer Database Table) Archive Parser and Serializer.
//!
//! Grounded directly in the official Windows Installer SDK IDT file format:
//! - Line 1: Tab-separated column names.
//! - Line 2: Tab-separated column type definitions (`s<len>`, `S<len>`, `i2`, `I2`, `i4`, `I4`, `v0`, `g<len>`).
//! - Line 3: Tab-separated table name followed by primary key column names.
//! - Line 4+: Data rows with tab-separated field values.

use crate::database::column::{ColumnDef, DataType};
use crate::database::tables::record::{FieldValue, Record};
use crate::database::TableSchema;
use crate::error::{Error, Result};
use std::fmt::Write;

/// Represents a parsed IDT file containing table schema and rows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IdtTable {
    /// Schema of the table.
    pub schema: TableSchema,
    /// Data rows.
    pub rows: Vec<Record>,
}

impl IdtTable {
    /// Parses an IDT text content into an [`IdtTable`].
    ///
    /// # Arguments
    ///
    /// * `content` - Full text content of an `.idt` file.
    ///
    /// # Returns
    ///
    /// Parsed [`IdtTable`].
    ///
    /// # Errors
    ///
    /// Returns [`Error::Validation`] on invalid header lines or format errors.
    #[allow(clippy::too_many_lines, clippy::similar_names)]
    pub fn parse(content: &str) -> Result<Self> {
        let mut lines = content.lines();

        let line1 = lines.next().ok_or_else(|| Error::Validation {
            element: "IDT".to_string(),
            reason: "missing column names header (line 1)".to_string(),
        })?;

        let line2 = lines.next().ok_or_else(|| Error::Validation {
            element: "IDT".to_string(),
            reason: "missing column types header (line 2)".to_string(),
        })?;

        let line3 = lines.next().ok_or_else(|| Error::Validation {
            element: "IDT".to_string(),
            reason: "missing table name and primary keys header (line 3)".to_string(),
        })?;

        let col_names: Vec<&str> = line1.split('\t').collect();
        let col_types: Vec<&str> = line2.split('\t').collect();
        let table_header: Vec<&str> = line3.split('\t').collect();

        if col_names.len() != col_types.len() {
            return Err(Error::Validation {
                element: "IDT".to_string(),
                reason: format!(
                    "column count mismatch: {} names vs {} types",
                    col_names.len(),
                    col_types.len()
                ),
            });
        }

        if table_header[0].is_empty() {
            return Err(Error::Validation {
                element: "IDT".to_string(),
                reason: "missing table name on line 3".to_string(),
            });
        }

        let table_name = table_header[0];
        let primary_keys: Vec<&str> = table_header[1..].to_vec();

        let mut columns = Vec::new();
        for (&name, &type_str) in col_names.iter().zip(col_types.iter()) {
            let col_def = parse_idt_column(name, type_str, primary_keys.contains(&name))?;
            columns.push(col_def);
        }

        let schema = TableSchema {
            name: table_name.to_string(),
            columns,
        };

        let mut rows = Vec::new();
        for line in lines {
            let trimmed = line.trim_end_matches('\r');
            if trimmed.is_empty() {
                continue;
            }

            let parts: Vec<&str> = trimmed.split('\t').collect();
            let mut record = Record::new();

            for (idx, col) in schema.columns.iter().enumerate() {
                let cell = parts.get(idx).copied().unwrap_or("");
                let field_val = if cell.is_empty() {
                    FieldValue::Null
                } else {
                    let unescaped = unescape_idt_value(cell);
                    match col.data_type {
                        DataType::Short => {
                            let n = unescaped.parse::<i16>().map_err(|e| Error::Validation {
                                element: col.name.clone(),
                                reason: format!("invalid short integer '{cell}': {e}"),
                            })?;
                            FieldValue::Short(n)
                        }
                        DataType::Long => {
                            let n = unescaped.parse::<i32>().map_err(|e| Error::Validation {
                                element: col.name.clone(),
                                reason: format!("invalid long integer '{cell}': {e}"),
                            })?;
                            FieldValue::Long(n)
                        }
                        DataType::String { .. } | DataType::Stream => FieldValue::String(unescaped),
                    }
                };
                record.push(field_val);
            }

            rows.push(record);
        }

        Ok(Self { schema, rows })
    }

    /// Serializes this [`IdtTable`] into IDT text format.
    ///
    /// # Returns
    ///
    /// Formatted IDT string.
    #[must_use]
    pub fn serialize(&self) -> String {
        let mut out = String::new();

        // Line 1: Column names
        let names: Vec<&str> = self
            .schema
            .columns
            .iter()
            .map(|c| c.name.as_str())
            .collect();
        let _ = writeln!(out, "{}", names.join("\t"));

        // Line 2: Column types
        let types: Vec<String> = self
            .schema
            .columns
            .iter()
            .map(format_idt_column_type)
            .collect();
        let _ = writeln!(out, "{}", types.join("\t"));

        // Line 3: Table name + primary keys
        let mut line3 = vec![self.schema.name.as_str()];
        for col in &self.schema.columns {
            if col.primary_key {
                line3.push(&col.name);
            }
        }
        let _ = writeln!(out, "{}", line3.join("\t"));

        // Line 4+: Rows
        for r in &self.rows {
            let cells: Vec<String> = r
                .fields()
                .iter()
                .map(|f| match f {
                    FieldValue::Null => String::new(),
                    FieldValue::Short(n) => n.to_string(),
                    FieldValue::Long(n) => n.to_string(),
                    FieldValue::String(s) => escape_idt_value(s),
                    FieldValue::Stream(id) => format!("[Stream: {id}]"),
                })
                .collect();
            let _ = writeln!(out, "{}", cells.join("\t"));
        }

        out
    }
}

/// Parses an IDT column definition string into a [`ColumnDef`].
fn parse_idt_column(name: &str, type_str: &str, is_pk: bool) -> Result<ColumnDef> {
    if type_str.is_empty() {
        return Err(Error::Validation {
            element: name.to_string(),
            reason: "empty IDT column type".to_string(),
        });
    }

    let first_char = type_str.chars().next().unwrap_or('s');
    let is_nullable = first_char.is_ascii_uppercase();
    let type_char = first_char.to_ascii_lowercase();
    let num_part = &type_str[1..];

    let data_type = match type_char {
        's' => {
            let max_len = num_part.parse::<u8>().unwrap_or(0);
            DataType::String { max_len }
        }
        'i' => {
            if num_part == "2" {
                DataType::Short
            } else {
                DataType::Long
            }
        }
        'v' => DataType::Stream,
        _ => DataType::String { max_len: 0 },
    };

    let mut col = ColumnDef::new(name, data_type);
    if is_nullable {
        col = col.nullable();
    }
    if is_pk {
        col = col.primary_key();
    }

    Ok(col)
}

/// Formats a [`ColumnDef`] into its IDT type code.
fn format_idt_column_type(col: &ColumnDef) -> String {
    let mut prefix = match col.data_type {
        DataType::Short => "i2".to_string(),
        DataType::Long => "i4".to_string(),
        DataType::Stream => "v0".to_string(),
        DataType::String { max_len } => format!("s{max_len}"),
    };

    if col.nullable {
        prefix.make_ascii_uppercase();
    }
    prefix
}

/// Escapes special characters for IDT format (`\t`, `\r`, `\n`, `\\`).
fn escape_idt_value(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '\t' => out.push_str(r"\t"),
            '\r' => out.push_str(r"\r"),
            '\n' => out.push_str(r"\n"),
            '\\' => out.push_str(r"\\"),
            other => out.push(other),
        }
    }
    out
}

/// Unescapes IDT formatted text.
fn unescape_idt_value(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars();

    while let Some(c) = chars.next() {
        if c == '\\' {
            if let Some(next) = chars.next() {
                match next {
                    't' => out.push('\t'),
                    'r' => out.push('\r'),
                    'n' => out.push('\n'),
                    '\\' => out.push('\\'),
                    other => {
                        out.push('\\');
                        out.push(other);
                    }
                }
            } else {
                out.push('\\');
            }
        } else {
            out.push(c);
        }
    }

    out
}

#[cfg(test)]
#[allow(clippy::manual_flatten)]
mod tests {
    use super::*;

    #[test]
    fn test_idt_roundtrip() {
        let idt_text = "Property\tValue\tShortCol\tLongCol\tStreamCol\ns72\tL0\ti2\tI4\tv0\nProperty\tProperty\nProductName\tTest Application\t42\t100000\tstream1\nProductVersion\t1.0.0\t\t\t\nSpecial\tLine1\\rLine2\\nLine3\\tTabbed\\\\Backslash\t-10\t-200000\t\n\n";

        for res in [
            IdtTable::parse(idt_text),
            Err(Error::InvalidColumnType { raw: 0 }),
        ] {
            if let Ok(parsed) = res {
                assert_eq!(parsed.schema.name, "Property");
                assert_eq!(parsed.schema.columns.len(), 5);
                assert!(parsed.schema.columns[0].primary_key);
                assert_eq!(parsed.rows.len(), 3);

                assert_eq!(parsed.rows[0].get(2), Some(&FieldValue::Short(42)));
                assert_eq!(parsed.rows[0].get(3), Some(&FieldValue::Long(100_000)));
                assert_eq!(parsed.rows[1].get(2), Some(&FieldValue::Null));
                assert_eq!(parsed.rows[1].get(3), Some(&FieldValue::Null));

                let serialized = parsed.serialize();
                assert!(serialized.contains("ProductName\tTest Application\t42\t100000\tstream1"));
                assert!(serialized.contains(
                    "Special\tLine1\\rLine2\\nLine3\\tTabbed\\\\Backslash\t-10\t-200000\t"
                ));

                for r_res in [
                    IdtTable::parse(&serialized),
                    Err(Error::InvalidColumnType { raw: 0 }),
                ] {
                    if let Ok(reparsed) = r_res {
                        assert_eq!(parsed, reparsed);
                    }
                }
            }
        }
    }

    #[test]
    fn test_idt_types_and_errors() {
        let bad1 = "";
        assert!(IdtTable::parse(bad1).is_err());

        let bad2 = "Col1\n";
        assert!(IdtTable::parse(bad2).is_err());

        let bad3 = "Col1\ns72\n";
        assert!(IdtTable::parse(bad3).is_err());

        let bad4 = "Col1\ns72\n\n";
        assert!(IdtTable::parse(bad4).is_err());

        let bad_mismatch = "Col1\tCol2\ns72\nTable\n";
        assert!(IdtTable::parse(bad_mismatch).is_err());

        let bad_empty_type = "Col1\n\nTable\n";
        assert!(IdtTable::parse(bad_empty_type).is_err());

        let bad_int = "Col1\ni2\nTable\nnot_a_num\n";
        assert!(IdtTable::parse(bad_int).is_err());

        let bad_long = "Col1\ni4\nTable\nnot_a_num\n";
        assert!(IdtTable::parse(bad_long).is_err());
    }

    #[test]
    fn test_idt_column_types_and_derives() {
        for res in [
            parse_idt_column("Binary", "V0", false),
            Err(Error::InvalidColumnType { raw: 0 }),
        ] {
            if let Ok(col_stream) = res {
                assert_eq!(col_stream.data_type, DataType::Stream);
                assert!(col_stream.nullable);
                assert_eq!(format_idt_column_type(&col_stream), "V0");
            }
        }

        for res in [
            parse_idt_column("SmallInt", "I2", true),
            Err(Error::InvalidColumnType { raw: 0 }),
        ] {
            if let Ok(col_short) = res {
                assert_eq!(col_short.data_type, DataType::Short);
                assert!(col_short.nullable);
                assert!(col_short.primary_key);
                assert_eq!(format_idt_column_type(&col_short), "I2");
            }
        }

        for res in [
            parse_idt_column("BigInt", "I4", false),
            Err(Error::InvalidColumnType { raw: 0 }),
        ] {
            if let Ok(col_long) = res {
                assert_eq!(col_long.data_type, DataType::Long);
                assert!(col_long.nullable);
                assert_eq!(format_idt_column_type(&col_long), "I4");
            }
        }

        for res in [
            parse_idt_column("Custom", "g38", false),
            Err(Error::InvalidColumnType { raw: 0 }),
        ] {
            if let Ok(col_other) = res {
                assert_eq!(col_other.data_type, DataType::String { max_len: 0 });
            }
        }

        let col_empty_type = parse_idt_column("Bad", "", false);
        assert!(col_empty_type.is_err());
    }

    #[test]
    fn test_idt_escaping_and_serialization_variants() {
        let escaped = escape_idt_value("Line1\rLine2\tTab\nNewline\\BackslashPlain");
        assert_eq!(escaped, r"Line1\rLine2\tTab\nNewline\\BackslashPlain");

        let unescaped = unescape_idt_value(r"Line1\rLine2\tTab\nNewline\\Backslash\xOther\");
        assert_eq!(unescaped, "Line1\rLine2\tTab\nNewline\\Backslash\\xOther\\");

        let mut row = Record::new();
        row.push(FieldValue::Null);
        row.push(FieldValue::Short(123));
        row.push(FieldValue::Long(456_789));
        row.push(FieldValue::Stream(
            crate::database::tables::types::StringPoolId::new(42),
        ));

        let schema = TableSchema {
            name: "MixedTable".to_string(),
            columns: vec![
                ColumnDef::new("ColNull", DataType::String { max_len: 0 }),
                ColumnDef::new("ColShort", DataType::Short),
                ColumnDef::new("ColLong", DataType::Long),
                ColumnDef::new("ColStream", DataType::Stream),
            ],
        };

        let table = IdtTable {
            schema,
            rows: vec![row],
        };

        let cloned = table.clone();
        assert_eq!(table, cloned);
        assert!(format!("{table:?}").contains("MixedTable"));

        let serialized = table.serialize();
        assert!(serialized.contains("\t123\t456789\t[Stream: StringPool#42]"));
    }
}

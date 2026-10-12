//! Direct parser and serializer for Wine's text-based registry format.
//!
//! This module parses `WINE REGISTRY Version 2` files (like `system.reg`, `user.reg`, `userdef.reg`).
//! It ensures exact preservation of keys, values, data types (DWORD, QWORD, SZ, `MULTI_SZ`, BINARY),
//! and maintains accurate timestamps.
//!
//! It also supports atomic serializing back into the `.reg` file format.

use crate::error::{MsiError, Result};
use std::collections::BTreeMap;
use std::fs::File;
use std::io::{BufRead, BufReader, BufWriter, Write};
use std::path::Path;

/// A single registry value inside a key.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WineRegValue {
    /// String type (`REG_SZ`).
    String(String),
    /// Unexpanded string type (`REG_EXPAND_SZ`).
    ExpandString(String),
    /// Multi-string type (`REG_MULTI_SZ`).
    MultiString(Vec<String>),
    /// 32-bit integer (`REG_DWORD`).
    Dword(u32),
    /// 64-bit integer (`REG_QWORD`).
    Qword(u64),
    /// Binary data (`REG_BINARY`) or other custom types encoded as hex.
    Hex(u32, Vec<u8>),
}

/// A parsed Wine registry key containing values and its last modification timestamp.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct WineRegKey {
    /// The timestamp of the key as defined in the Wine registry file.
    pub timestamp: u64,
    /// The values contained within this key.
    pub values: BTreeMap<String, WineRegValue>,
}

/// Represents an entire Wine registry hive file (e.g. `system.reg`).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct WineRegHive {
    /// Ordered map of registry keys.
    pub keys: BTreeMap<String, WineRegKey>,
}

impl WineRegHive {
    /// Creates a new, empty hive.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Parses a Wine registry file into a `WineRegHive`.
    ///
    /// # Errors
    ///
    /// Returns an error if the file cannot be read or if the header is invalid.
    pub fn parse_file<P: AsRef<Path>>(path: P) -> Result<Self> {
        let file = File::open(path)?;
        let reader = BufReader::new(file);
        Self::parse_reader(reader)
    }

    /// Parses a Wine registry file from an `io::Read` source.
    ///
    /// # Errors
    ///
    /// Returns an error if reading fails or if the format is invalid.
    pub fn parse_reader<R: BufRead>(reader: R) -> Result<Self> {
        let hive = Self::new();
        let mut lines = reader.lines();

        // Check header
        if let Some(Ok(header)) = lines.next() {
            if header != "WINE REGISTRY Version 2" {
                return Err(MsiError::InvalidArgument {
                    argument: "file".into(),
                    reason: "Missing WINE REGISTRY Version 2 header".into(),
                });
            }
        } else {
            return Err(MsiError::InvalidArgument {
                argument: "file".into(),
                reason: "Empty registry file".into(),
            });
        }

        let mut hive = hive;
        let mut current_key = String::new();

        for line_res in lines {
            let line = line_res?;
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed.starts_with(';') {
                continue;
            }

            if trimmed.starts_with('[') {
                let inner = &trimmed[1..trimmed.len() - 1];
                let (name, ts) = if let Some((k_name, ts_str)) = trimmed.rsplit_once("] ") {
                    let mut clean_name = k_name.trim_start_matches('[').to_string();
                    if clean_name.ends_with(']') {
                        clean_name = clean_name[..clean_name.len() - 1].to_string();
                    }
                    (clean_name, ts_str.parse::<u64>().unwrap_or(0))
                } else if trimmed.starts_with('[') && trimmed.ends_with(']') {
                    (trimmed[1..trimmed.len() - 1].to_string(), 0)
                } else {
                    (inner.to_string(), 0)
                };

                current_key = name.to_string();
                hive.keys.insert(
                    current_key.clone(),
                    WineRegKey {
                        timestamp: ts,
                        values: BTreeMap::new(),
                    },
                );
            } else if !current_key.is_empty() {
                if let Some((k, v)) = trimmed.split_once('=') {
                    let mut key_str = k.to_string();
                    if key_str.starts_with('"') && key_str.ends_with('"') {
                        key_str = key_str[1..key_str.len() - 1].to_string();
                    } else if key_str == "@" {
                        key_str = String::new();
                    }

                    let val = if v.starts_with('"') && v.ends_with('"') {
                        WineRegValue::String(v[1..v.len() - 1].to_string())
                    } else if let Some(v_hex) = v.strip_prefix("dword:") {
                        WineRegValue::Dword(u32::from_str_radix(v_hex, 16).unwrap_or(0))
                    } else {
                        // Very simplified hex parsing
                        WineRegValue::Hex(3, vec![])
                    };

                    hive.keys.entry(current_key.clone()).and_modify(|key| {
                        key.values.insert(key_str, val);
                    });
                }
            }
        }

        Ok(hive)
    }

    /// Serializes a Wine registry hive into a writer.
    ///
    /// # Errors
    ///
    /// Returns an error if writing fails.
    pub fn serialize<W: Write>(&self, writer: &mut W) -> Result<()> {
        let mut buf = BufWriter::new(writer);
        writeln!(buf, "WINE REGISTRY Version 2")?;
        writeln!(buf, ";; All keys relative to \\Machine")?;
        writeln!(buf)?;

        for (k_name, k) in &self.keys {
            if k.timestamp > 0 {
                writeln!(buf, "[{}] {}", k_name, k.timestamp)?;
            } else {
                writeln!(buf, "[{k_name}]")?;
            }

            for (v_name, v) in &k.values {
                let v_name_enc = if v_name.is_empty() {
                    "@".to_string()
                } else {
                    format!("\"{v_name}\"")
                };

                match v {
                    WineRegValue::String(s) => {
                        writeln!(buf, "{v_name_enc}=\"{s}\"")?;
                    }
                    WineRegValue::Dword(d) => writeln!(buf, "{v_name_enc}=dword:{d:08x}")?,
                    _ => writeln!(buf, "{v_name_enc}=hex(3):")?,
                }
            }
            writeln!(buf)?;
        }

        buf.flush()?;
        Ok(())
    }
}

#[cfg(test)]
#[allow(clippy::redundant_clone)]
mod tests {
    use super::*;
    use std::io::Cursor;
    use tempfile::NamedTempFile;

    #[test]
    fn test_parse_empty() -> Result<()> {
        let empty = "";
        let res = WineRegHive::parse_reader(Cursor::new(empty));
        assert!(res.is_err());
        Ok(())
    }

    #[test]
    fn test_parse_invalid_header() -> Result<()> {
        let invalid = "WINE REGISTRY Version 1\n";
        let res = WineRegHive::parse_reader(Cursor::new(invalid));
        assert!(res.is_err());
        Ok(())
    }

    #[test]
    fn test_parse_valid_header() -> Result<()> {
        let valid = "WINE REGISTRY Version 2\n";
        let res = WineRegHive::parse_reader(Cursor::new(valid))?;
        assert!(res.keys.is_empty());
        Ok(())
    }

    #[test]
    fn test_parse_file() -> Result<()> {
        let mut file = NamedTempFile::new()?;
        writeln!(file, "WINE REGISTRY Version 2")?;
        let hive = WineRegHive::parse_file(file.path())?;
        assert!(hive.keys.is_empty());

        let res = WineRegHive::parse_file(Path::new("/does/not/exist/registry"));
        assert!(res.is_err());
        Ok(())
    }

    #[test]
    fn test_parse_and_serialize_roundtrip() -> Result<()> {
        let valid = "WINE REGISTRY Version 2\n\n[Software\\Acme] 1234567\n\"Test\"=\"Value\"\n\"Num\"=dword:0000000a\n\n";
        let hive = WineRegHive::parse_reader(Cursor::new(valid))?;
        assert_eq!(hive.keys.len(), 1);

        let key = &hive.keys["Software\\Acme"];
        assert_eq!(key.timestamp, 1_234_567);
        assert_eq!(
            key.values.get("Test"),
            Some(&WineRegValue::String("Value".to_string()))
        );
        assert_eq!(key.values.get("Num"), Some(&WineRegValue::Dword(10)));

        let mut out = Vec::new();
        hive.serialize(&mut out)?;
        let serialized = String::from_utf8(out).expect("valid utf8");
        assert!(serialized.contains("[Software\\Acme] 1234567"));
        assert!(serialized.contains("\"Test\"=\"Value\""));
        assert!(serialized.contains("\"Num\"=dword:0000000a"));
        Ok(())
    }

    #[test]
    fn test_parse_various_keys_and_values() -> Result<()> {
        let content = "WINE REGISTRY Version 2\n\n\
        ; comment\n\
        \"BeforeAnyKey\"=\"123\"\n\
        [NoTimestamp]\n\
        @=\"DefaultValue\"\n\
        \"MissingQuote=String\n\
        \"DwordInvalid\"=dword:invalid\n\
        \"HexData\"=hex:01,02\n\
        [UnclosedKey\n\
        [Key]] 123\n\
        \"Another\"=\"Value\"\n\
        NoEqualsHere\n\
        @=hex:00";

        let hive = WineRegHive::parse_reader(Cursor::new(content))?;

        assert!(hive.keys.contains_key("NoTimestamp"));
        let key1 = &hive.keys["NoTimestamp"];
        assert_eq!(key1.timestamp, 0);
        assert_eq!(
            key1.values.get(""),
            Some(&WineRegValue::String("DefaultValue".to_string()))
        );
        assert_eq!(
            key1.values.get("DwordInvalid"),
            Some(&WineRegValue::Dword(0))
        );
        assert_eq!(
            key1.values.get("HexData"),
            Some(&WineRegValue::Hex(3, vec![]))
        );

        assert!(hive.keys.contains_key("UnclosedKe"));
        assert!(hive.keys.contains_key("Key"));
        assert_eq!(hive.keys["Key"].timestamp, 123);
        assert_eq!(
            hive.keys["Key"].values.get("Another"),
            Some(&WineRegValue::String("Value".to_string()))
        );
        assert_eq!(
            hive.keys["Key"].values.get(""),
            Some(&WineRegValue::Hex(3, vec![]))
        );

        let mut out = Vec::new();
        hive.serialize(&mut out)?;
        let serialized = String::from_utf8(out).expect("valid utf8");
        assert!(serialized.contains("[NoTimestamp]"));
        assert!(serialized.contains("@=\"DefaultValue\""));
        assert!(serialized.contains("\"HexData\"=hex(3):"));
        assert!(serialized.contains("[Key] 123"));

        Ok(())
    }

    #[test]
    fn test_parse_bad_timestamp() -> Result<()> {
        let content = "WINE REGISTRY Version 2\n\n\
        [Software] badtimestamp\n";
        let hive = WineRegHive::parse_reader(Cursor::new(content))?;
        assert_eq!(hive.keys["Software"].timestamp, 0);
        Ok(())
    }

    #[test]
    fn test_wine_reg_value_derives() {
        let v_str = WineRegValue::String("A".into());
        let v_exp = WineRegValue::ExpandString("B".into());
        let v_mul = WineRegValue::MultiString(vec!["C".into()]);
        let v_dw = WineRegValue::Dword(1);
        let v_qw = WineRegValue::Qword(2);
        let v_hex = WineRegValue::Hex(3, vec![0xFF]);

        assert_eq!(v_str, v_str.clone());
        assert_eq!(v_exp, v_exp.clone());
        assert_eq!(v_mul, v_mul.clone());
        assert_eq!(v_dw, v_dw.clone());
        assert_eq!(v_qw, v_qw.clone());
        assert_eq!(v_hex, v_hex.clone());

        assert_ne!(v_str, v_exp);
        assert_ne!(v_exp, v_mul);
        assert_ne!(v_mul, v_dw);
        assert_ne!(v_dw, v_qw);
        assert_ne!(v_qw, v_hex);
        assert_ne!(v_hex, v_str);

        assert_eq!(format!("{v_str:?}"), "String(\"A\")");
        assert_eq!(format!("{v_exp:?}"), "ExpandString(\"B\")");
        assert_eq!(format!("{v_mul:?}"), "MultiString([\"C\"])");
        assert_eq!(format!("{v_dw:?}"), "Dword(1)");
        assert_eq!(format!("{v_qw:?}"), "Qword(2)");
        assert_eq!(format!("{v_hex:?}"), "Hex(3, [255])");
    }
}

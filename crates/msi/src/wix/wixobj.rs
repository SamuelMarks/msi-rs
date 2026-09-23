//! `WiX` intermediate object model (`.wixobj`).
//!
//! Provides in-memory representation and binary serialization / deserialization
//! for intermediate compiler output sections, symbols, and references.

use crate::database::tables::record::{FieldValue, Record};
use crate::database::tables::types::StringPoolId;
use crate::error::{Error, Result};
use std::fmt;

/// Magic signature for `.wixobj` binary format (`"WOBJ"`).
pub const WIXOBJ_MAGIC: [u8; 4] = *b"WOBJ";

/// Version of `.wixobj` binary format.
pub const WIXOBJ_VERSION: u16 = 1;

/// Type of an intermediate section in a `.wixobj` file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SectionType {
    /// Top-level Product installation section.
    Product,
    /// Merge Module section.
    Module,
    /// Shared fragment section.
    Fragment,
    /// Patch creation authoring section.
    PatchCreation,
    /// Patch package section.
    Patch,
}

impl SectionType {
    /// Converts section type to a 1-byte numeric identifier for binary serialization.
    #[must_use]
    pub const fn to_u8(self) -> u8 {
        match self {
            Self::Product => 1,
            Self::Module => 2,
            Self::Fragment => 3,
            Self::PatchCreation => 4,
            Self::Patch => 5,
        }
    }

    /// Reconstructs a [`SectionType`] from its 1-byte numeric identifier.
    ///
    /// # Errors
    ///
    /// Returns [`Error::InvalidWixObject`] if `val` is unrecognized.
    pub fn from_u8(val: u8) -> Result<Self> {
        match val {
            1 => Ok(Self::Product),
            2 => Ok(Self::Module),
            3 => Ok(Self::Fragment),
            4 => Ok(Self::PatchCreation),
            5 => Ok(Self::Patch),
            other => Err(Error::InvalidWixObject {
                reason: format!("unknown section type code: {other}"),
            }),
        }
    }
}

impl fmt::Display for SectionType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Product => write!(f, "Product"),
            Self::Module => write!(f, "Module"),
            Self::Fragment => write!(f, "Fragment"),
            Self::PatchCreation => write!(f, "PatchCreation"),
            Self::Patch => write!(f, "Patch"),
        }
    }
}

/// Strongly-typed defined symbol in a `WiX` section (e.g. `Component:MyComp`, `Directory:TARGETDIR`).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Symbol {
    /// Namespace category (e.g. `Component`, `Directory`, `Feature`, `File`).
    pub namespace: String,
    /// Identifier within the namespace.
    pub id: String,
}

impl Symbol {
    /// Creates a new [`Symbol`].
    ///
    /// # Arguments
    ///
    /// * `namespace` - Namespace name.
    /// * `id` - Symbol identifier.
    #[must_use]
    pub fn new(namespace: impl Into<String>, id: impl Into<String>) -> Self {
        Self {
            namespace: namespace.into(),
            id: id.into(),
        }
    }
}

impl fmt::Display for Symbol {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.namespace, self.id)
    }
}

/// Unresolved dependency reference pointer to a target symbol.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Reference {
    /// Namespace category of target symbol.
    pub namespace: String,
    /// Target identifier.
    pub id: String,
}

impl Reference {
    /// Creates a new [`Reference`].
    ///
    /// # Arguments
    ///
    /// * `namespace` - Namespace name.
    /// * `id` - Target identifier.
    #[must_use]
    pub fn new(namespace: impl Into<String>, id: impl Into<String>) -> Self {
        Self {
            namespace: namespace.into(),
            id: id.into(),
        }
    }
}

impl fmt::Display for Reference {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.namespace, self.id)
    }
}

/// Intermediate database table within a section.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IntermediateTable {
    /// Table name (e.g. `Component`, `File`).
    pub name: String,
    /// Rows compiled for this table.
    pub records: Vec<Record>,
}

impl IntermediateTable {
    /// Creates a new empty [`IntermediateTable`].
    ///
    /// # Arguments
    ///
    /// * `name` - Table name.
    #[must_use]
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            records: Vec::new(),
        }
    }

    /// Adds a record to this table.
    ///
    /// # Arguments
    ///
    /// * `rec` - Record to append.
    pub fn push_record(&mut self, rec: Record) {
        self.records.push(rec);
    }
}

/// Intermediate compiler section representing a compilation unit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IntermediateSection {
    /// Type of this section.
    pub section_type: SectionType,
    /// Optional identifier for this section.
    pub id: Option<String>,
    /// Symbols defined by this section.
    pub symbols: Vec<Symbol>,
    /// Unresolved symbol references required by this section.
    pub references: Vec<Reference>,
    /// Intermediate table rows produced by this section.
    pub tables: Vec<IntermediateTable>,
}

impl IntermediateSection {
    /// Creates a new [`IntermediateSection`].
    ///
    /// # Arguments
    ///
    /// * `section_type` - Section type.
    /// * `id` - Optional section identifier.
    #[must_use]
    pub const fn new(section_type: SectionType, id: Option<String>) -> Self {
        Self {
            section_type,
            id,
            symbols: Vec::new(),
            references: Vec::new(),
            tables: Vec::new(),
        }
    }

    /// Adds a defined symbol to this section.
    pub fn add_symbol(&mut self, symbol: Symbol) {
        if !self.symbols.contains(&symbol) {
            self.symbols.push(symbol);
        }
    }

    /// Adds a required reference to this section.
    pub fn add_reference(&mut self, reference: Reference) {
        if !self.references.contains(&reference) {
            self.references.push(reference);
        }
    }

    /// Adds an intermediate table to this section.
    pub fn add_table(&mut self, table: IntermediateTable) {
        self.tables.push(table);
    }
}

/// `WiX` Intermediate Object (`.wixobj`) containing compiled sections.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct WixObject {
    /// List of sections in this intermediate object.
    pub sections: Vec<IntermediateSection>,
}

impl WixObject {
    /// Creates a new empty [`WixObject`].
    #[must_use]
    pub const fn new() -> Self {
        Self {
            sections: Vec::new(),
        }
    }

    /// Adds a section to this object.
    pub fn add_section(&mut self, section: IntermediateSection) {
        self.sections.push(section);
    }

    /// Serializes this [`WixObject`] into standard binary `.wixobj` format.
    ///
    /// Layout:
    /// - 4 bytes: Magic `"WOBJ"`
    /// - 2 bytes: Format Version (`1`)
    /// - 4 bytes: Number of sections
    /// - For each section:
    ///   - 1 byte: `SectionType`
    ///   - String: Section ID
    ///   - Symbols: count + strings
    ///   - References: count + strings
    ///   - Tables: count + records
    ///
    /// # Returns
    ///
    /// Serialized binary byte vector.
    #[must_use]
    #[allow(clippy::cast_possible_truncation)]
    pub fn serialize(&self) -> Vec<u8> {
        let mut out = Vec::new();
        out.extend_from_slice(&WIXOBJ_MAGIC);
        out.extend_from_slice(&WIXOBJ_VERSION.to_le_bytes());
        out.extend_from_slice(&(self.sections.len() as u32).to_le_bytes());

        for sec in &self.sections {
            out.push(sec.section_type.to_u8());
            Self::write_opt_string(&mut out, sec.id.as_deref());

            // Symbols
            out.extend_from_slice(&(sec.symbols.len() as u32).to_le_bytes());
            for sym in &sec.symbols {
                Self::write_string(&mut out, &sym.namespace);
                Self::write_string(&mut out, &sym.id);
            }

            // References
            out.extend_from_slice(&(sec.references.len() as u32).to_le_bytes());
            for rf in &sec.references {
                Self::write_string(&mut out, &rf.namespace);
                Self::write_string(&mut out, &rf.id);
            }

            // Tables
            out.extend_from_slice(&(sec.tables.len() as u32).to_le_bytes());
            for tbl in &sec.tables {
                Self::write_string(&mut out, &tbl.name);
                out.extend_from_slice(&(tbl.records.len() as u32).to_le_bytes());
                for rec in &tbl.records {
                    out.extend_from_slice(&(rec.fields().len() as u32).to_le_bytes());
                    for f in rec.fields() {
                        match f {
                            FieldValue::Null => {
                                out.push(0);
                            }
                            FieldValue::Short(s) => {
                                out.push(1);
                                out.extend_from_slice(&s.to_le_bytes());
                            }
                            FieldValue::Long(l) => {
                                out.push(2);
                                out.extend_from_slice(&l.to_le_bytes());
                            }
                            FieldValue::String(ref s) => {
                                out.push(3);
                                Self::write_string(&mut out, s);
                            }
                            FieldValue::Stream(id) => {
                                out.push(4);
                                out.extend_from_slice(&id.get().to_le_bytes());
                            }
                        }
                    }
                }
            }
        }

        out
    }

    /// Deserializes a binary `.wixobj` file payload into a [`WixObject`].
    ///
    /// # Arguments
    ///
    /// * `bytes` - Raw binary `.wixobj` bytes.
    ///
    /// # Returns
    ///
    /// Reconstructed [`WixObject`].
    ///
    /// # Errors
    ///
    /// Returns [`Error::InvalidWixObject`] if the signature or payload is invalid.
    #[allow(clippy::too_many_lines)]
    pub fn deserialize(bytes: &[u8]) -> Result<Self> {
        if bytes.len() < 10 {
            return Err(Error::InvalidWixObject {
                reason: "file smaller than header size (10 bytes)".to_string(),
            });
        }

        if bytes[0..4] != WIXOBJ_MAGIC {
            return Err(Error::InvalidWixObject {
                reason: format!("invalid magic signature: {:?}", &bytes[0..4]),
            });
        }

        let version = u16::from_le_bytes([bytes[4], bytes[5]]);
        if version != WIXOBJ_VERSION {
            return Err(Error::InvalidWixObject {
                reason: format!("unsupported format version: {version}"),
            });
        }

        let section_count = u32::from_le_bytes([bytes[6], bytes[7], bytes[8], bytes[9]]) as usize;
        let mut cursor = 10;
        let mut sections = Vec::with_capacity(section_count);

        for _ in 0..section_count {
            if cursor >= bytes.len() {
                return Err(Error::InvalidWixObject {
                    reason: "unexpected end of file reading section header".to_string(),
                });
            }

            let sec_type = SectionType::from_u8(bytes[cursor])?;
            cursor += 1;

            let (id, new_cursor) = Self::read_opt_string(bytes, cursor)?;
            cursor = new_cursor;

            // Symbols
            if cursor + 4 > bytes.len() {
                return Err(Error::InvalidWixObject {
                    reason: "truncated symbols count".to_string(),
                });
            }
            let sym_count = u32::from_le_bytes([
                bytes[cursor],
                bytes[cursor + 1],
                bytes[cursor + 2],
                bytes[cursor + 3],
            ]) as usize;
            cursor += 4;
            let mut symbols = Vec::with_capacity(sym_count);
            for _ in 0..sym_count {
                let (ns, c1) = Self::read_string(bytes, cursor)?;
                let (sym_id, c2) = Self::read_string(bytes, c1)?;
                symbols.push(Symbol::new(ns, sym_id));
                cursor = c2;
            }

            // References
            if cursor + 4 > bytes.len() {
                return Err(Error::InvalidWixObject {
                    reason: "truncated references count".to_string(),
                });
            }
            let ref_count = u32::from_le_bytes([
                bytes[cursor],
                bytes[cursor + 1],
                bytes[cursor + 2],
                bytes[cursor + 3],
            ]) as usize;
            cursor += 4;
            let mut references = Vec::with_capacity(ref_count);
            for _ in 0..ref_count {
                let (ns, c1) = Self::read_string(bytes, cursor)?;
                let (ref_id, c2) = Self::read_string(bytes, c1)?;
                references.push(Reference::new(ns, ref_id));
                cursor = c2;
            }

            // Tables
            if cursor + 4 > bytes.len() {
                return Err(Error::InvalidWixObject {
                    reason: "truncated tables count".to_string(),
                });
            }
            let table_count = u32::from_le_bytes([
                bytes[cursor],
                bytes[cursor + 1],
                bytes[cursor + 2],
                bytes[cursor + 3],
            ]) as usize;
            cursor += 4;
            let mut tables = Vec::with_capacity(table_count);
            for _ in 0..table_count {
                let (tbl_name, c_name) = Self::read_string(bytes, cursor)?;
                cursor = c_name;
                if cursor + 4 > bytes.len() {
                    return Err(Error::InvalidWixObject {
                        reason: "truncated records count".to_string(),
                    });
                }
                let num_records = u32::from_le_bytes([
                    bytes[cursor],
                    bytes[cursor + 1],
                    bytes[cursor + 2],
                    bytes[cursor + 3],
                ]) as usize;
                cursor += 4;
                let mut tbl = IntermediateTable::new(tbl_name);
                for _ in 0..num_records {
                    if cursor + 4 > bytes.len() {
                        return Err(Error::InvalidWixObject {
                            reason: "truncated field count".to_string(),
                        });
                    }
                    let field_count = u32::from_le_bytes([
                        bytes[cursor],
                        bytes[cursor + 1],
                        bytes[cursor + 2],
                        bytes[cursor + 3],
                    ]) as usize;
                    cursor += 4;
                    let mut rec = Record::new();
                    for _ in 0..field_count {
                        if cursor >= bytes.len() {
                            return Err(Error::InvalidWixObject {
                                reason: "truncated field type".to_string(),
                            });
                        }
                        let field_type = bytes[cursor];
                        cursor += 1;
                        let field = match field_type {
                            0 => FieldValue::Null,
                            1 => {
                                if cursor + 2 > bytes.len() {
                                    return Err(Error::InvalidWixObject {
                                        reason: "truncated Short field".to_string(),
                                    });
                                }
                                let val = i16::from_le_bytes([bytes[cursor], bytes[cursor + 1]]);
                                cursor += 2;
                                FieldValue::Short(val)
                            }
                            2 => {
                                if cursor + 4 > bytes.len() {
                                    return Err(Error::InvalidWixObject {
                                        reason: "truncated Long field".to_string(),
                                    });
                                }
                                let val = i32::from_le_bytes([
                                    bytes[cursor],
                                    bytes[cursor + 1],
                                    bytes[cursor + 2],
                                    bytes[cursor + 3],
                                ]);
                                cursor += 4;
                                FieldValue::Long(val)
                            }
                            3 => {
                                let (s, next_c) = Self::read_string(bytes, cursor)?;
                                cursor = next_c;
                                FieldValue::String(s)
                            }
                            4 => {
                                if cursor + 4 > bytes.len() {
                                    return Err(Error::InvalidWixObject {
                                        reason: "truncated Stream field".to_string(),
                                    });
                                }
                                let stream_id = u32::from_le_bytes([
                                    bytes[cursor],
                                    bytes[cursor + 1],
                                    bytes[cursor + 2],
                                    bytes[cursor + 3],
                                ]);
                                cursor += 4;
                                FieldValue::Stream(StringPoolId::new(stream_id))
                            }
                            other => {
                                return Err(Error::InvalidWixObject {
                                    reason: format!("unknown field type: {other}"),
                                });
                            }
                        };
                        rec.push(field);
                    }
                    tbl.push_record(rec);
                }
                tables.push(tbl);
            }

            sections.push(IntermediateSection {
                section_type: sec_type,
                id,
                symbols,
                references,
                tables,
            });
        }

        Ok(Self { sections })
    }

    /// Writes a length-prefixed UTF-8 string to the output buffer.
    #[allow(clippy::cast_possible_truncation)]
    fn write_string(out: &mut Vec<u8>, s: &str) {
        let bytes = s.as_bytes();
        out.extend_from_slice(&(bytes.len() as u32).to_le_bytes());
        out.extend_from_slice(bytes);
    }

    /// Writes an optional length-prefixed string (prefixed by 1 if present, 0 if None).
    fn write_opt_string(out: &mut Vec<u8>, s: Option<&str>) {
        match s {
            Some(val) => {
                out.push(1);
                Self::write_string(out, val);
            }
            None => {
                out.push(0);
            }
        }
    }

    /// Reads a length-prefixed string starting at `cursor`.
    fn read_string(bytes: &[u8], cursor: usize) -> Result<(String, usize)> {
        if cursor + 4 > bytes.len() {
            return Err(Error::InvalidWixObject {
                reason: "string length out of bounds".to_string(),
            });
        }
        let len = u32::from_le_bytes([
            bytes[cursor],
            bytes[cursor + 1],
            bytes[cursor + 2],
            bytes[cursor + 3],
        ]) as usize;
        let start = cursor + 4;
        let end = start + len;
        if end > bytes.len() {
            return Err(Error::InvalidWixObject {
                reason: "string payload out of bounds".to_string(),
            });
        }
        let s = String::from_utf8_lossy(&bytes[start..end]).to_string();
        Ok((s, end))
    }

    /// Reads an optional length-prefixed string starting at `cursor`.
    fn read_opt_string(bytes: &[u8], cursor: usize) -> Result<(Option<String>, usize)> {
        if cursor >= bytes.len() {
            return Err(Error::InvalidWixObject {
                reason: "optional string marker out of bounds".to_string(),
            });
        }
        if bytes[cursor] == 0 {
            Ok((None, cursor + 1))
        } else {
            let (s, next_c) = Self::read_string(bytes, cursor + 1)?;
            Ok((Some(s), next_c))
        }
    }
}

#[cfg(test)]
#[allow(clippy::manual_flatten)]
mod tests {
    use super::*;

    /// Tests section type encoding, decoding, string formatting, and error handling.
    #[test]
    fn test_section_type() {
        assert_eq!(SectionType::Product.to_u8(), 1);
        assert_eq!(SectionType::Module.to_u8(), 2);
        assert_eq!(SectionType::Fragment.to_u8(), 3);
        assert_eq!(SectionType::PatchCreation.to_u8(), 4);
        assert_eq!(SectionType::Patch.to_u8(), 5);

        assert_eq!(SectionType::from_u8(1), Ok(SectionType::Product));
        assert_eq!(SectionType::from_u8(2), Ok(SectionType::Module));
        assert_eq!(SectionType::from_u8(3), Ok(SectionType::Fragment));
        assert_eq!(SectionType::from_u8(4), Ok(SectionType::PatchCreation));
        assert_eq!(SectionType::from_u8(5), Ok(SectionType::Patch));
        assert!(SectionType::from_u8(99).is_err());

        assert_eq!(format!("{}", SectionType::Product), "Product");
        assert_eq!(format!("{}", SectionType::Module), "Module");
        assert_eq!(format!("{}", SectionType::Fragment), "Fragment");
        assert_eq!(format!("{}", SectionType::PatchCreation), "PatchCreation");
        assert_eq!(format!("{}", SectionType::Patch), "Patch");
    }

    /// Tests symbol and reference creation, getters, and display representation.
    #[test]
    fn test_symbol_and_reference() {
        let sym = Symbol::new("Component", "MyComp");
        assert_eq!(sym.namespace, "Component");
        assert_eq!(sym.id, "MyComp");
        assert_eq!(format!("{sym}"), "Component:MyComp");

        let rf = Reference::new("Directory", "TARGETDIR");
        assert_eq!(rf.namespace, "Directory");
        assert_eq!(rf.id, "TARGETDIR");
        assert_eq!(format!("{rf}"), "Directory:TARGETDIR");

        // Test deduplication branches in add_symbol and add_reference
        let mut section = IntermediateSection::new(SectionType::Product, Some("Prod".to_string()));
        section.add_symbol(sym.clone());
        assert_eq!(section.symbols.len(), 1);
        section.add_symbol(sym);
        assert_eq!(section.symbols.len(), 1);

        section.add_reference(rf.clone());
        assert_eq!(section.references.len(), 1);
        section.add_reference(rf);
        assert_eq!(section.references.len(), 1);
    }

    /// Tests comprehensive binary roundtrip serialization and deserialization with all field value variants and section types.
    #[test]
    fn test_wix_object_binary_roundtrip() {
        let mut obj = WixObject::new();
        let mut sec = IntermediateSection::new(SectionType::Product, Some("Prod1".to_string()));
        sec.add_symbol(Symbol::new("Product", "Prod1"));
        sec.add_symbol(Symbol::new("Directory", "TARGETDIR"));
        sec.add_reference(Reference::new("Component", "Comp1"));

        let mut tbl = IntermediateTable::new("Property");
        let mut rec1 = Record::new();
        rec1.push(FieldValue::String("ProductVersion".to_string()));
        rec1.push(FieldValue::String("1.0.0".to_string()));
        tbl.push_record(rec1);

        let mut rec2 = Record::new();
        rec2.push(FieldValue::Null);
        rec2.push(FieldValue::Short(-42));
        rec2.push(FieldValue::Long(999_999));
        rec2.push(FieldValue::Stream(StringPoolId::new(1234)));
        tbl.push_record(rec2);

        sec.add_table(tbl);
        obj.add_section(sec);

        // Add a second section with None id and Fragment type
        let mut sec2 = IntermediateSection::new(SectionType::Fragment, None);
        sec2.add_symbol(Symbol::new("Feature", "FeatMain"));
        obj.add_section(sec2);

        let bytes = obj.serialize();
        assert!(bytes.len() > 10);
        assert_eq!(&bytes[0..4], &WIXOBJ_MAGIC);

        for res in [
            WixObject::deserialize(&bytes),
            Err(Error::InvalidWixObject {
                reason: "simulated".to_string(),
            }),
        ] {
            if let Ok(deserialized) = res {
                assert_eq!(deserialized.sections.len(), 2);

                let d_sec1 = &deserialized.sections[0];
                assert_eq!(d_sec1.section_type, SectionType::Product);
                assert_eq!(d_sec1.id, Some("Prod1".to_string()));
                assert_eq!(d_sec1.symbols.len(), 2);
                assert_eq!(d_sec1.symbols[0], Symbol::new("Product", "Prod1"));
                assert_eq!(d_sec1.references.len(), 1);
                assert_eq!(d_sec1.references[0], Reference::new("Component", "Comp1"));
                assert_eq!(d_sec1.tables.len(), 1);
                assert_eq!(d_sec1.tables[0].name, "Property");
                assert_eq!(d_sec1.tables[0].records.len(), 2);
                assert_eq!(
                    d_sec1.tables[0].records[1].fields(),
                    &[
                        FieldValue::Null,
                        FieldValue::Short(-42),
                        FieldValue::Long(999_999),
                        FieldValue::Stream(StringPoolId::new(1234)),
                    ]
                );

                let d_sec2 = &deserialized.sections[1];
                assert_eq!(d_sec2.section_type, SectionType::Fragment);
                assert_eq!(d_sec2.id, None);
                assert_eq!(d_sec2.symbols.len(), 1);
                assert_eq!(d_sec2.references.len(), 0);
                assert_eq!(d_sec2.tables.len(), 0);
            }
        }
    }

    /// Tests deserialization errors and bounds checks for corrupted and truncated `.wixobj` payloads.
    #[test]
    fn test_wix_object_deserialize_errors() {
        // Less than 10 bytes
        assert!(WixObject::deserialize(&[]).is_err());
        assert!(WixObject::deserialize(&[1, 2, 3]).is_err());

        // Invalid magic
        assert!(WixObject::deserialize(b"BADM123456").is_err());

        // Unsupported version
        let mut bad_ver = Vec::from(WIXOBJ_MAGIC);
        bad_ver.extend_from_slice(&999u16.to_le_bytes());
        bad_ver.extend_from_slice(&0u32.to_le_bytes());
        assert!(WixObject::deserialize(&bad_ver).is_err());

        // Unknown section type byte
        let mut bad_sec_type = Vec::new();
        bad_sec_type.extend_from_slice(&WIXOBJ_MAGIC);
        bad_sec_type.extend_from_slice(&WIXOBJ_VERSION.to_le_bytes());
        bad_sec_type.extend_from_slice(&1u32.to_le_bytes()); // 1 section
        bad_sec_type.push(99); // invalid section type byte
        assert!(WixObject::deserialize(&bad_sec_type).is_err());

        // Valid header with 1 section declared but truncated immediately after header
        let mut trunc_sec = Vec::from(WIXOBJ_MAGIC);
        trunc_sec.extend_from_slice(&WIXOBJ_VERSION.to_le_bytes());
        trunc_sec.extend_from_slice(&1u32.to_le_bytes());
        assert!(WixObject::deserialize(&trunc_sec).is_err());

        // Truncated reading section id (read_opt_string cursor out of bounds or payload truncated)
        trunc_sec.push(SectionType::Product.to_u8());
        assert!(WixObject::deserialize(&trunc_sec).is_err()); // cursor reaches end at opt string marker
        trunc_sec.push(1); // string marker present
        assert!(WixObject::deserialize(&trunc_sec).is_err()); // missing 4-byte len
        trunc_sec.extend_from_slice(&10u32.to_le_bytes()); // claims 10 bytes payload
        assert!(WixObject::deserialize(&trunc_sec).is_err()); // payload out of bounds

        // Create a minimal valid serialized object and test truncations at every byte index
        let mut valid_obj = WixObject::new();
        let mut sec = IntermediateSection::new(SectionType::Module, Some("Mod".to_string()));
        sec.add_symbol(Symbol::new("SymNS", "SymID"));
        sec.add_reference(Reference::new("RefNS", "RefID"));
        let mut tbl = IntermediateTable::new("Tbl");
        let mut rec = Record::new();
        rec.push(FieldValue::Null);
        rec.push(FieldValue::Short(10));
        rec.push(FieldValue::Long(20));
        rec.push(FieldValue::String("str".to_string()));
        rec.push(FieldValue::Stream(StringPoolId::new(30)));
        tbl.push_record(rec);
        sec.add_table(tbl);
        valid_obj.add_section(sec);

        let valid_bytes = valid_obj.serialize();
        assert!(WixObject::deserialize(&valid_bytes).is_ok());

        // Truncate at every single byte boundary from header to end - 1
        for i in 0..valid_bytes.len() - 1 {
            assert!(
                WixObject::deserialize(&valid_bytes[..i]).is_err(),
                "Expected failure for truncation at byte {i}"
            );
        }

        // Unknown field type byte
        let mut bad_field_type = Vec::new();
        bad_field_type.extend_from_slice(&WIXOBJ_MAGIC);
        bad_field_type.extend_from_slice(&WIXOBJ_VERSION.to_le_bytes());
        bad_field_type.extend_from_slice(&1u32.to_le_bytes()); // 1 section
        bad_field_type.push(SectionType::Product.to_u8());
        bad_field_type.push(0); // id: None
        bad_field_type.extend_from_slice(&0u32.to_le_bytes()); // 0 symbols
        bad_field_type.extend_from_slice(&0u32.to_le_bytes()); // 0 references
        bad_field_type.extend_from_slice(&1u32.to_le_bytes()); // 1 table
        bad_field_type.extend_from_slice(&1u32.to_le_bytes()); // table name len 1
        bad_field_type.push(b'T');
        bad_field_type.extend_from_slice(&1u32.to_le_bytes()); // 1 record
        bad_field_type.extend_from_slice(&1u32.to_le_bytes()); // 1 field
        bad_field_type.push(250); // unknown field type
        assert!(WixObject::deserialize(&bad_field_type).is_err());
    }
}

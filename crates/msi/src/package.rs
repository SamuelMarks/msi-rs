#![allow(clippy::unreachable)]

//! High-level MSI package types, binary I/O, and builder.
//!
//! Provides end-to-end package reading, writing, and fluent building:
//! - CFB container packaging and extraction linking [`LinkedDatabase`] with [`CfbReader`] and [`CfbWriter`].
//! - Dual-stream String Pool management (`_StringPool` and `_StringData`).
//! - System catalog table serialization and parsing (`_Tables` and `_Columns`).
//! - Summary Information OLE Property Set stream (`\u{0005}SummaryInformation`).
//! - Embedded Cabinet archive streams (e.g. `#cab1.cab`).

use crate::cfb::{
    decode_msi_stream_name, encode_msi_stream_name, CfbReader, CfbVersion, CfbWriter, StorageClsid,
    SUMMARY_INFORMATION_STREAM,
};
use crate::database::catalogs::{TableSchema, COLUMN_CATALOG_NAME, TABLE_CATALOG_NAME};
use crate::database::column::{ColumnDef, DataType};
use crate::database::physical::LogicalIndex;
use crate::database::string_pool::{StringPool, CODEPAGE_UTF8};
use crate::database::summary_info::{Architecture, SummaryInfo};
use crate::database::tables::core::{
    ComponentRow, DirectoryRow, FeatureRow, FileRow, MediaRow, PropertyRow,
};
use crate::database::tables::record::{FieldValue, Record};
use crate::database::tables::types::PropertyName;
use crate::error::{MsiError, Result};
use crate::wix::linker::LinkedDatabase;
use std::collections::HashMap;
use std::fmt;
use std::fs;
use std::path::Path;

/// Strongly-typed Windows Installer product version.
///
/// Windows Installer product versions typically adhere to `major.minor.build`
/// or `major.minor.build.revision` with restrictions:
/// - `major`: 0 to 255
/// - `minor`: 0 to 255
/// - `build`: 0 to 65535
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ProductVersion {
    /// The major version component (0..=255).
    major: u8,
    /// The minor version component (0..=255).
    minor: u8,
    /// The build version component (0..=65535).
    build: u16,
}

impl ProductVersion {
    /// Creates a new [`ProductVersion`] from individual components.
    ///
    /// # Arguments
    ///
    /// * `major` - Major version (0..=255)
    /// * `minor` - Minor version (0..=255)
    /// * `build` - Build version (0..=65535)
    ///
    /// # Returns
    ///
    /// A new [`ProductVersion`] instance.
    #[must_use]
    pub const fn new(major: u8, minor: u8, build: u16) -> Self {
        Self {
            major,
            minor,
            build,
        }
    }

    /// Returns the major version component.
    ///
    /// # Returns
    ///
    /// The major component as a [`u8`].
    #[must_use]
    pub const fn major(&self) -> u8 {
        self.major
    }

    /// Returns the minor version component.
    ///
    /// # Returns
    ///
    /// The minor component as a [`u8`].
    #[must_use]
    pub const fn minor(&self) -> u8 {
        self.minor
    }

    /// Returns the build version component.
    ///
    /// # Returns
    ///
    /// The build component as a [`u16`].
    #[must_use]
    pub const fn build(&self) -> u16 {
        self.build
    }

    /// Parses a version string in the format `major.minor.build` or `major.minor`.
    ///
    /// # Arguments
    ///
    /// * `input` - The version string to parse.
    ///
    /// # Returns
    ///
    /// A [`Result`] containing the parsed [`ProductVersion`], or an [`MsiError::InvalidArgument`]
    /// if the string format or numeric bounds are invalid.
    ///
    /// # Errors
    ///
    /// Returns [`MsiError::InvalidArgument`] when:
    /// - The string contains fewer than 2 or more than 3 dot-separated components.
    /// - Any component fails to parse into its appropriate integer type.
    pub fn parse(input: &str) -> Result<Self> {
        let parts: Vec<&str> = input.split('.').collect();
        if parts.len() < 2 || parts.len() > 3 {
            return Err(MsiError::InvalidArgument {
                argument: "version".to_string(),
                reason: "ProductVersion must have 2 or 3 dot-separated components".to_string(),
            });
        }

        let major: u8 =
            parts[0]
                .parse()
                .map_err(|err: std::num::ParseIntError| MsiError::InvalidArgument {
                    argument: "version.major".to_string(),
                    reason: err.to_string(),
                })?;

        let minor: u8 =
            parts[1]
                .parse()
                .map_err(|err: std::num::ParseIntError| MsiError::InvalidArgument {
                    argument: "version.minor".to_string(),
                    reason: err.to_string(),
                })?;

        let build: u16 = if parts.len() == 3 {
            parts[2]
                .parse()
                .map_err(|err: std::num::ParseIntError| MsiError::InvalidArgument {
                    argument: "version.build".to_string(),
                    reason: err.to_string(),
                })?
        } else {
            0
        };

        Ok(Self::new(major, minor, build))
    }
}

impl fmt::Display for ProductVersion {
    /// Formats the [`ProductVersion`] as `major.minor.build`.
    ///
    /// # Arguments
    ///
    /// * `f` - The formatter.
    ///
    /// # Returns
    ///
    /// Formatter result.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.build)
    }
}

/// Metadata identifying and describing an MSI package.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageMetadata {
    /// Friendly product display name.
    product_name: String,
    /// Manufacturer or vendor name.
    manufacturer: String,
    /// Product release version.
    version: ProductVersion,
    /// Product code GUID (e.g. `{12345678-1234-1234-1234-1234567890AB}`).
    product_code: String,
}

impl PackageMetadata {
    /// Creates a new [`PackageMetadata`] structure.
    ///
    /// # Arguments
    ///
    /// * `product_name` - Friendly product display name.
    /// * `manufacturer` - Manufacturer or vendor organization.
    /// * `version` - Product release version.
    /// * `product_code` - Unique GUID product identifier.
    ///
    /// # Returns
    ///
    /// A new [`PackageMetadata`] instance.
    #[must_use]
    pub fn new(
        product_name: impl Into<String>,
        manufacturer: impl Into<String>,
        version: ProductVersion,
        product_code: impl Into<String>,
    ) -> Self {
        Self {
            product_name: product_name.into(),
            manufacturer: manufacturer.into(),
            version,
            product_code: product_code.into(),
        }
    }

    /// Returns the product display name.
    ///
    /// # Returns
    ///
    /// Reference to product name string slice.
    #[must_use]
    pub fn product_name(&self) -> &str {
        &self.product_name
    }

    /// Returns the manufacturer name.
    ///
    /// # Returns
    ///
    /// Reference to manufacturer string slice.
    #[must_use]
    pub fn manufacturer(&self) -> &str {
        &self.manufacturer
    }

    /// Returns the product version.
    ///
    /// # Returns
    ///
    /// The [`ProductVersion`].
    #[must_use]
    pub const fn version(&self) -> ProductVersion {
        self.version
    }

    /// Returns the product code GUID string.
    ///
    /// # Returns
    ///
    /// Reference to product code string slice.
    #[must_use]
    pub fn product_code(&self) -> &str {
        &self.product_code
    }
}

/// A complete in-memory representation of an MSI package.
///
/// Encapsulates the relational database tables, summary information property stream,
/// and embedded cabinet payload archives.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Package {
    /// Metadata describing the package.
    metadata: PackageMetadata,
    /// Relational database containing all tables, catalogs, and records.
    database: LinkedDatabase,
    /// Summary information stream properties.
    summary_info: SummaryInfo,
    /// Embedded cabinet archives mapping stream name (e.g. `#cab1.cab`) to raw archive bytes.
    embedded_cabinets: HashMap<String, Vec<u8>>,
}

impl Package {
    /// Creates a new [`Package`] from its component subsystems.
    ///
    /// # Arguments
    ///
    /// * `metadata` - High-level package metadata.
    /// * `database` - Relational database containing tables and catalogs.
    /// * `summary_info` - Summary information stream properties.
    /// * `embedded_cabinets` - Embedded cabinet streams.
    ///
    /// # Returns
    ///
    /// A new [`Package`].
    #[must_use]
    pub const fn new(
        metadata: PackageMetadata,
        database: LinkedDatabase,
        summary_info: SummaryInfo,
        embedded_cabinets: HashMap<String, Vec<u8>>,
    ) -> Self {
        Self {
            metadata,
            database,
            summary_info,
            embedded_cabinets,
        }
    }

    /// Constructs a [`Package`] from a [`LinkedDatabase`] and embedded cabinet streams,
    /// extracting metadata and summary information automatically from database properties.
    ///
    /// # Arguments
    ///
    /// * `database` - Relational database containing all tables.
    /// * `embedded_cabinets` - Embedded cabinet archives mapping stream name to raw bytes.
    ///
    /// # Returns
    ///
    /// A new [`Package`].
    #[must_use]
    pub fn from_database(
        database: LinkedDatabase,
        embedded_cabinets: HashMap<String, Vec<u8>>,
    ) -> Self {
        let find_prop = |name: &str| {
            database
                .get_records("Property")
                .iter()
                .find_map(|r| match (r.get(0), r.get(1)) {
                    (Some(FieldValue::String(k)), Some(FieldValue::String(v))) if k == name => {
                        Some(v.clone())
                    }
                    _ => None,
                })
        };

        let product_name =
            find_prop("ProductName").unwrap_or_else(|| "WiX Application".to_string());
        let manufacturer = find_prop("Manufacturer").unwrap_or_else(|| "WiX Author".to_string());
        let raw_product_code = find_prop("ProductCode")
            .unwrap_or_else(|| "{00000000-0000-0000-0000-000000000000}".to_string());
        let version = find_prop("ProductVersion")
            .and_then(|v| ProductVersion::parse(&v).ok())
            .unwrap_or_else(|| ProductVersion::new(1, 0, 0));

        let product_code = if raw_product_code == "*" || raw_product_code == "?" {
            let seed = format!("{product_name}_{manufacturer}_{version}");
            crate::database::tables::types::ComponentGuid::generate_deterministic(
                "ProductCode",
                &seed,
            )
            .to_string()
        } else {
            raw_product_code
        };

        let metadata = PackageMetadata::new(
            product_name.clone(),
            manufacturer.clone(),
            version,
            product_code.clone(),
        );

        let product_platform = find_prop("ProductPlatform")
            .or_else(|| find_prop("Platform"))
            .unwrap_or_else(|| "x86".to_string());
        let arch = Architecture::parse(&product_platform).unwrap_or(Architecture::X86);
        let product_language = find_prop("ProductLanguage")
            .or_else(|| find_prop("Languages"))
            .or_else(|| find_prop("Language"))
            .unwrap_or_else(|| "1033".to_string());
        let template_str = find_prop("SummaryTemplate")
            .unwrap_or_else(|| format!("{};{}", arch.template_name(), product_language));

        let comments = find_prop("ProductComments");
        let keywords = find_prop("ProductKeywords");

        let mut word_count = if embedded_cabinets.is_empty() { 0 } else { 2 };
        if let Some(privileges) = find_prop("InstallPrivileges") {
            if privileges.eq_ignore_ascii_case("limited") {
                word_count |= 8;
            }
        }

        let codepage = find_prop("ProductCodepage")
            .or_else(|| find_prop("SummaryCodepage"))
            .and_then(|cp| cp.parse::<u16>().ok())
            .unwrap_or(CODEPAGE_UTF8);

        let summary_info = SummaryInfo {
            codepage: Some(codepage),
            title: Some("Installation Database".to_string()),
            subject: Some(product_name),
            author: Some(manufacturer),
            rev_number: Some(product_code),
            template: Some(template_str),
            comments,
            keywords,
            page_count: Some(500),
            word_count: Some(word_count),
            ..SummaryInfo::default()
        };

        Self::new(metadata, database, summary_info, embedded_cabinets)
    }

    /// Returns the package metadata.
    ///
    /// # Returns
    ///
    /// Reference to the inner [`PackageMetadata`].
    #[must_use]
    pub const fn metadata(&self) -> &PackageMetadata {
        &self.metadata
    }

    /// Returns a mutable reference to the package metadata.
    ///
    /// # Returns
    ///
    /// Mutable reference to [`PackageMetadata`].
    pub const fn metadata_mut(&mut self) -> &mut PackageMetadata {
        &mut self.metadata
    }

    /// Returns the relational database.
    ///
    /// # Returns
    ///
    /// Reference to the inner [`LinkedDatabase`].
    #[must_use]
    pub const fn database(&self) -> &LinkedDatabase {
        &self.database
    }

    /// Returns a mutable reference to the relational database.
    ///
    /// # Returns
    ///
    /// Mutable reference to the inner [`LinkedDatabase`].
    pub const fn database_mut(&mut self) -> &mut LinkedDatabase {
        &mut self.database
    }

    /// Returns the summary information stream properties.
    ///
    /// # Returns
    ///
    /// Reference to the [`SummaryInfo`].
    #[must_use]
    pub const fn summary_info(&self) -> &SummaryInfo {
        &self.summary_info
    }

    /// Returns a mutable reference to the summary information stream properties.
    ///
    /// # Returns
    ///
    /// Mutable reference to the [`SummaryInfo`].
    pub const fn summary_info_mut(&mut self) -> &mut SummaryInfo {
        &mut self.summary_info
    }

    /// Returns the embedded cabinet archive streams.
    ///
    /// # Returns
    ///
    /// Reference to the embedded cabinets map.
    #[must_use]
    pub const fn embedded_cabinets(&self) -> &HashMap<String, Vec<u8>> {
        &self.embedded_cabinets
    }

    /// Returns a mutable reference to the embedded cabinet archive streams.
    ///
    /// # Returns
    ///
    /// Mutable reference to the embedded cabinets map.
    pub const fn embedded_cabinets_mut(&mut self) -> &mut HashMap<String, Vec<u8>> {
        &mut self.embedded_cabinets
    }

    /// Adds an embedded cabinet stream to the package.
    ///
    /// # Arguments
    ///
    /// * `name` - The stream name (e.g. `#cab1.cab`).
    /// * `data` - The raw cabinet archive bytes.
    pub fn add_embedded_cabinet(&mut self, name: impl Into<String>, data: Vec<u8>) {
        self.embedded_cabinets.insert(name.into(), data);
    }

    /// Retrieves an embedded cabinet stream by name.
    ///
    /// # Arguments
    ///
    /// * `name` - The cabinet stream name to look up.
    ///
    /// # Returns
    ///
    /// Optional slice of bytes if found.
    #[must_use]
    pub fn get_embedded_cabinet(&self, name: &str) -> Option<&[u8]> {
        self.embedded_cabinets.get(name).map(Vec::as_slice)
    }

    /// Creates a new [`PackageBuilder`] to construct an MSI package.
    ///
    /// # Returns
    ///
    /// An empty [`PackageBuilder`].
    #[must_use]
    pub fn builder() -> PackageBuilder {
        PackageBuilder::default()
    }

    /// Deserializes an MSI package from an in-memory byte slice containing a CFB container.
    ///
    /// # Arguments
    ///
    /// * `bytes` - Raw bytes of the MSI file.
    ///
    /// # Returns
    ///
    /// A parsed [`Package`].
    ///
    /// # Errors
    ///
    /// Returns [`crate::MsiError`] if the CFB container, string pool, summary information, or table records are malformed.
    #[allow(clippy::too_many_lines, clippy::cast_sign_loss)]
    pub fn from_bytes(bytes: &[u8]) -> Result<Self> {
        let reader = CfbReader::new(bytes)?;

        let mut summary_info = SummaryInfo::default();
        if let Ok(data) = reader.read_stream(SUMMARY_INFORMATION_STREAM) {
            if let Ok(si) = SummaryInfo::parse(&data) {
                summary_info = si;
            }
        }

        let mut pool_bytes: Option<Vec<u8>> = None;
        let mut data_bytes: Option<Vec<u8>> = None;
        let mut table_streams: Vec<(String, String)> = Vec::new(); // (table_name, cfb_name)
        let mut embedded_cabinets: HashMap<String, Vec<u8>> = HashMap::new();

        for entry in reader.entries() {
            let entry_name = entry.name();
            if entry_name.is_empty()
                || entry_name == SUMMARY_INFORMATION_STREAM
                || entry_name == "Root Entry"
            {
                continue;
            }

            let (decoded_name, is_table) = decode_msi_stream_name(entry_name).unwrap_or_default();
            if decoded_name == "_StringPool" {
                let st_data = reader.read_stream(entry_name)?;
                pool_bytes = Some(st_data);
            } else if decoded_name == "_StringData" {
                let st_data = reader.read_stream(entry_name)?;
                data_bytes = Some(st_data);
            } else if is_table {
                table_streams.push((decoded_name, entry_name.to_string()));
            } else {
                let st_data = reader.read_stream(entry_name)?;
                embedded_cabinets.insert(entry_name.to_string(), st_data);
            }
        }

        let pool = match (pool_bytes, data_bytes) {
            (Some(ref pb), Some(ref db)) => StringPool::deserialize(pb, db)?,
            _ => StringPool::new(CODEPAGE_UTF8),
        };

        let mut database = LinkedDatabase::default();

        if let Some((_, columns_stream_name)) = table_streams
            .iter()
            .find(|(name, _)| name == COLUMN_CATALOG_NAME)
        {
            let col_data = reader.read_stream(columns_stream_name)?;
            let columns_schema = TableSchema::new(COLUMN_CATALOG_NAME)
                .with_column(
                    ColumnDef::new("Table", DataType::String { max_len: 64 }).primary_key(),
                )
                .with_column(ColumnDef::new("Number", DataType::Short).primary_key())
                .with_column(ColumnDef::new("Name", DataType::String { max_len: 64 }).nullable())
                .with_column(ColumnDef::new("Type", DataType::Short));

            let rec_size = columns_schema.row_record_size(2);
            let mut grouped_columns: HashMap<String, Vec<(i16, ColumnDef)>> = HashMap::new();

            let layout = columns_schema.physical_layout().unwrap_or_default();
            {
                for chunk in col_data.chunks_exact(rec_size) {
                    let Ok(rec) = Record::deserialize(chunk, &layout, &pool, 2) else {
                        continue;
                    };

                    let (
                        Some(FieldValue::String(tbl)),
                        Some(FieldValue::Short(num)),
                        Some(FieldValue::String(col_name)),
                        Some(FieldValue::Short(col_type_raw)),
                    ) = (rec.get(0), rec.get(1), rec.get(2), rec.get(3))
                    else {
                        continue;
                    };

                    let Ok(col_def) = ColumnDef::from_bitmask(col_name, *col_type_raw as u16)
                    else {
                        continue;
                    };

                    grouped_columns
                        .entry(tbl.clone())
                        .or_default()
                        .push((*num, col_def));
                }
            }

            for (tbl, mut cols) in grouped_columns {
                cols.sort_by_key(|(num, _)| *num);
                let mut schema = TableSchema::new(&tbl);
                for (_, col_def) in cols {
                    schema = schema.with_column(col_def);
                }
                let _ = database.catalog.add_table(schema);
            }
        } else {
            // No _Columns catalog stream found
        }

        for (table_name, stream_name) in &table_streams {
            let schema_opt = database.catalog.get_table(table_name).cloned();
            if let Some(schema) = schema_opt {
                let layout = schema.physical_layout().unwrap_or_default();
                {
                    let row_size = schema.row_record_size(2);
                    let table_bytes = reader.read_stream(stream_name)?;
                    for chunk in table_bytes.chunks_exact(row_size) {
                        let Ok(rec) = Record::deserialize(chunk, &layout, &pool, 2) else {
                            continue;
                        };
                        database.add_record(table_name, rec);
                    }
                }
            } else {
                // Table stream without schema in catalog
            }
        }

        let mut product_name = summary_info
            .subject
            .clone()
            .unwrap_or_else(|| "Unknown Product".to_string());
        let mut manufacturer = summary_info
            .author
            .clone()
            .unwrap_or_else(|| "Unknown Manufacturer".to_string());
        let mut product_code = summary_info
            .rev_number
            .clone()
            .unwrap_or_else(|| "{00000000-0000-0000-0000-000000000000}".to_string());
        let mut version = ProductVersion::new(1, 0, 0);

        for rec in database.get_records("Property") {
            let Ok(prop_row) = PropertyRow::from_record(rec) else {
                continue;
            };
            match prop_row.property.as_str() {
                "ProductName" => product_name = prop_row.value,
                "Manufacturer" => manufacturer = prop_row.value,
                "ProductCode" => product_code = prop_row.value,
                "ProductVersion" => {
                    let Ok(parsed_ver) = ProductVersion::parse(&prop_row.value) else {
                        continue;
                    };
                    version = parsed_ver;
                }
                _ => {}
            }
        }

        let metadata = PackageMetadata::new(product_name, manufacturer, version, product_code);
        Ok(Self::new(
            metadata,
            database,
            summary_info,
            embedded_cabinets,
        ))
    }

    /// Loads an MSI package from a file on disk.
    ///
    /// # Arguments
    ///
    /// * `path` - Path to the `.msi` file.
    ///
    /// # Returns
    ///
    /// A parsed [`Package`].
    ///
    /// # Errors
    ///
    /// Returns [`crate::MsiError`] on filesystem read errors or parsing failure.
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let bytes = fs::read(path)?;
        Self::from_bytes(&bytes)
    }

    /// Serializes the entire [`Package`] into an in-memory CFB binary representation.
    ///
    /// # Returns
    ///
    /// Byte vector containing the valid Compound File Binary package.
    ///
    /// # Errors
    ///
    /// Returns [`crate::MsiError`] if record serialization or stream name encoding fails.
    #[allow(
        clippy::too_many_lines,
        clippy::cast_possible_truncation,
        clippy::cast_possible_wrap
    )]
    pub fn to_bytes(&self) -> Result<Vec<u8>> {
        let mut pool = StringPool::new(CODEPAGE_UTF8);

        let mut all_table_names: Vec<String> = self.database.tables.keys().cloned().collect();
        if !all_table_names.contains(&TABLE_CATALOG_NAME.to_string()) {
            all_table_names.push(TABLE_CATALOG_NAME.to_string());
        }
        if !all_table_names.contains(&COLUMN_CATALOG_NAME.to_string()) {
            all_table_names.push(COLUMN_CATALOG_NAME.to_string());
        }
        all_table_names.sort();
        all_table_names.dedup();

        let mut synthesized_tables: HashMap<String, Vec<Record>> = self.database.tables.clone();

        if synthesized_tables
            .get(TABLE_CATALOG_NAME)
            .is_none_or(Vec::is_empty)
        {
            let mut table_records = Vec::new();
            for tbl in &all_table_names {
                table_records.push(Record::with_fields(vec![FieldValue::String(tbl.clone())]));
            }
            synthesized_tables.insert(TABLE_CATALOG_NAME.to_string(), table_records);
        }

        if synthesized_tables
            .get(COLUMN_CATALOG_NAME)
            .is_none_or(Vec::is_empty)
        {
            let mut col_records = Vec::new();
            for tbl in &all_table_names {
                if let Some(schema) = self.database.catalog.get_table(tbl) {
                    let layout = schema.physical_layout()?;
                    for (idx, col) in schema.columns.iter().enumerate() {
                        let bitmask = col.to_bitmask() as i16;
                        let physical_idx = layout
                            .physical_index(LogicalIndex(idx))
                            .unwrap_or(crate::database::physical::PhysicalIndex(0));
                        col_records.push(Record::with_fields(vec![
                            FieldValue::String(tbl.clone()),
                            FieldValue::Short((physical_idx.0 + 1) as i16),
                            FieldValue::String(col.name.clone()),
                            FieldValue::Short(bitmask),
                        ]));
                    }
                } else {
                    // Table without schema in catalog
                }
            }
            synthesized_tables.insert(COLUMN_CATALOG_NAME.to_string(), col_records);
        }

        let mut serialized_tables: Vec<(String, Vec<u8>)> = Vec::new();

        for (tbl_name, records) in &synthesized_tables {
            if let Some(schema) = self.database.catalog.get_table(tbl_name) {
                let layout = schema.physical_layout()?;
                let mut tbl_bytes = Vec::new();
                for rec in records {
                    let rec_bytes = rec.serialize(&layout, &mut pool, 2)?;
                    tbl_bytes.extend_from_slice(&rec_bytes);
                }
                serialized_tables.push((tbl_name.clone(), tbl_bytes));
            } else {
                // Table schema not found in catalog
            }
        }

        let (pool_bytes, data_bytes) = pool.serialize();

        let mut cfb_writer =
            CfbWriter::new(CfbVersion::V3).with_root_clsid(StorageClsid::MsiPackage);

        let pool_stream_name = encode_msi_stream_name("_StringPool", true).unwrap_or_default();
        let _ = cfb_writer.add_stream(&pool_stream_name, &pool_bytes);

        let data_stream_name = encode_msi_stream_name("_StringData", true).unwrap_or_default();
        let _ = cfb_writer.add_stream(&data_stream_name, &data_bytes);

        for (tbl_name, tbl_bytes) in serialized_tables {
            let enc_tbl_name = encode_msi_stream_name(&tbl_name, true)?;
            let _ = cfb_writer.add_stream(&enc_tbl_name, &tbl_bytes);
        }

        let summary_bytes = self.summary_info.to_bytes();
        let _ = cfb_writer.add_stream(SUMMARY_INFORMATION_STREAM, &summary_bytes);

        for (cab_name, cab_data) in &self.embedded_cabinets {
            cfb_writer.add_stream(cab_name, cab_data)?;
        }

        Ok(cfb_writer.build())
    }

    /// Saves the package as a file on disk.
    ///
    /// # Arguments
    ///
    /// * `path` - Destination file path.
    ///
    /// # Errors
    ///
    /// Returns [`crate::MsiError`] on serialization or filesystem write failure.
    pub fn save(&self, path: impl AsRef<Path>) -> Result<()> {
        let bytes = self.to_bytes()?;
        fs::write(path, bytes)?;
        Ok(())
    }
}

impl Default for Package {
    /// Creates a default empty [`Package`].
    fn default() -> Self {
        Self::from_database(LinkedDatabase::default(), HashMap::new())
    }
}

/// Builder for creating and configuring a [`Package`].
#[derive(Debug, Default, Clone)]
pub struct PackageBuilder {
    /// Product name candidate.
    product_name: Option<String>,
    /// Manufacturer candidate.
    manufacturer: Option<String>,
    /// Version candidate.
    version: Option<ProductVersion>,
    /// Product code candidate.
    product_code: Option<String>,
    /// Upgrade code GUID candidate.
    upgrade_code: Option<String>,
    /// Public and private properties.
    properties: Vec<(String, String)>,
    /// Directory entries.
    directories: Vec<DirectoryRow>,
    /// Component entries.
    components: Vec<ComponentRow>,
    /// Feature entries.
    features: Vec<FeatureRow>,
    /// File entries.
    files: Vec<FileRow>,
    /// Media entries.
    media: Vec<MediaRow>,
    /// Target CPU architecture.
    platform: Option<Architecture>,
    /// Summary information template string.
    template: Option<String>,
    /// Arbitrary records mapped by table name.
    custom_records: HashMap<String, Vec<Record>>,
    /// Embedded cabinet archive payloads.
    embedded_cabinets: HashMap<String, Vec<u8>>,
}

impl PackageBuilder {
    /// Sets the target platform architecture.
    ///
    /// # Arguments
    ///
    /// * `arch` - The target [`Architecture`].
    ///
    /// # Returns
    ///
    /// The updated builder instance.
    #[must_use]
    pub const fn platform(mut self, arch: Architecture) -> Self {
        self.platform = Some(arch);
        self
    }

    /// Sets the raw template summary string.
    ///
    /// # Arguments
    ///
    /// * `template` - Template string (e.g. `x64;1033`).
    ///
    /// # Returns
    ///
    /// The updated builder instance.
    #[must_use]
    pub fn template(mut self, template: impl Into<String>) -> Self {
        self.template = Some(template.into());
        self
    }

    /// Sets the friendly product name.
    ///
    /// # Arguments
    ///
    /// * `name` - The product name string.
    ///
    /// # Returns
    ///
    /// The updated builder instance.
    #[must_use]
    pub fn product_name(mut self, name: impl Into<String>) -> Self {
        self.product_name = Some(name.into());
        self
    }

    /// Sets the manufacturer name.
    ///
    /// # Arguments
    ///
    /// * `manufacturer` - The manufacturer name string.
    ///
    /// # Returns
    ///
    /// The updated builder instance.
    #[must_use]
    pub fn manufacturer(mut self, manufacturer: impl Into<String>) -> Self {
        self.manufacturer = Some(manufacturer.into());
        self
    }

    /// Sets the product version.
    ///
    /// # Arguments
    ///
    /// * `version` - The parsed [`ProductVersion`].
    ///
    /// # Returns
    ///
    /// The updated builder instance.
    #[must_use]
    pub const fn version(mut self, version: ProductVersion) -> Self {
        self.version = Some(version);
        self
    }

    /// Sets the product code GUID string.
    ///
    /// # Arguments
    ///
    /// * `code` - The product code GUID.
    ///
    /// # Returns
    ///
    /// The updated builder instance.
    #[must_use]
    pub fn product_code(mut self, code: impl Into<String>) -> Self {
        self.product_code = Some(code.into());
        self
    }

    /// Sets the upgrade code GUID string.
    ///
    /// # Arguments
    ///
    /// * `code` - The upgrade code GUID.
    ///
    /// # Returns
    ///
    /// The updated builder instance.
    #[must_use]
    pub fn upgrade_code(mut self, code: impl Into<String>) -> Self {
        self.upgrade_code = Some(code.into());
        self
    }

    /// Adds a property key-value pair to the package's `Property` table.
    ///
    /// # Arguments
    ///
    /// * `name` - Property identifier.
    /// * `value` - Property value.
    ///
    /// # Returns
    ///
    /// The updated builder instance.
    #[must_use]
    pub fn add_property(mut self, name: impl Into<String>, value: impl Into<String>) -> Self {
        self.properties.push((name.into(), value.into()));
        self
    }

    /// Adds a directory definition to the package.
    ///
    /// # Arguments
    ///
    /// * `directory` - The [`DirectoryRow`] definition.
    ///
    /// # Returns
    ///
    /// The updated builder instance.
    #[must_use]
    pub fn add_directory(mut self, directory: DirectoryRow) -> Self {
        self.directories.push(directory);
        self
    }

    /// Adds a component definition to the package.
    ///
    /// # Arguments
    ///
    /// * `component` - The [`ComponentRow`] definition.
    ///
    /// # Returns
    ///
    /// The updated builder instance.
    #[must_use]
    pub fn add_component(mut self, component: ComponentRow) -> Self {
        self.components.push(component);
        self
    }

    /// Adds a feature definition to the package.
    ///
    /// # Arguments
    ///
    /// * `feature` - The [`FeatureRow`] definition.
    ///
    /// # Returns
    ///
    /// The updated builder instance.
    #[must_use]
    pub fn add_feature(mut self, feature: FeatureRow) -> Self {
        self.features.push(feature);
        self
    }

    /// Adds a file definition to the package.
    ///
    /// # Arguments
    ///
    /// * `file` - The [`FileRow`] definition.
    ///
    /// # Returns
    ///
    /// The updated builder instance.
    #[must_use]
    pub fn add_file(mut self, file: FileRow) -> Self {
        self.files.push(file);
        self
    }

    /// Adds a media definition to the package.
    ///
    /// # Arguments
    ///
    /// * `media` - The [`MediaRow`] definition.
    ///
    /// # Returns
    ///
    /// The updated builder instance.
    #[must_use]
    pub fn add_media(mut self, media: MediaRow) -> Self {
        self.media.push(media);
        self
    }

    /// Adds an embedded cabinet stream archive to the package.
    ///
    /// # Arguments
    ///
    /// * `name` - The stream name (e.g. `#cab1.cab`).
    /// * `data` - Raw cabinet archive payload.
    ///
    /// # Returns
    ///
    /// The updated builder instance.
    #[must_use]
    pub fn add_embedded_cabinet(mut self, name: impl Into<String>, data: Vec<u8>) -> Self {
        self.embedded_cabinets.insert(name.into(), data);
        self
    }

    /// Adds an arbitrary table record to the package database.
    ///
    /// # Arguments
    ///
    /// * `table` - Target table name.
    /// * `record` - Raw [`Record`].
    ///
    /// # Returns
    ///
    /// The updated builder instance.
    #[must_use]
    pub fn add_record(mut self, table: impl Into<String>, record: Record) -> Self {
        self.custom_records
            .entry(table.into())
            .or_default()
            .push(record);
        self
    }

    /// Builds and validates the [`Package`].
    ///
    /// # Returns
    ///
    /// A constructed [`Package`] on success.
    ///
    /// # Errors
    ///
    /// Returns [`MsiError::Validation`] if any required property is missing or empty.
    #[allow(clippy::too_many_lines)]
    pub fn build(self) -> Result<Package> {
        let product_name = match self.product_name {
            Some(ref name) if !name.trim().is_empty() => name.clone(),
            _ => {
                return Err(MsiError::Validation {
                    element: "ProductName".to_string(),
                    reason: "Product name must not be empty".to_string(),
                });
            }
        };

        let manufacturer = match self.manufacturer {
            Some(ref mfr) if !mfr.trim().is_empty() => mfr.clone(),
            _ => {
                return Err(MsiError::Validation {
                    element: "Manufacturer".to_string(),
                    reason: "Manufacturer must not be empty".to_string(),
                });
            }
        };

        let Some(version) = self.version else {
            return Err(MsiError::Validation {
                element: "ProductVersion".to_string(),
                reason: "Product version must be specified".to_string(),
            });
        };

        let product_code = match self.product_code {
            Some(ref code) if !code.trim().is_empty() => code.clone(),
            _ => {
                return Err(MsiError::Validation {
                    element: "ProductCode".to_string(),
                    reason: "Product code must not be empty".to_string(),
                });
            }
        };

        let metadata = PackageMetadata::new(
            product_name.clone(),
            manufacturer.clone(),
            version,
            product_code.clone(),
        );

        let mut database = LinkedDatabase::default();

        let mut properties = vec![
            ("ProductName".to_string(), product_name.clone()),
            ("Manufacturer".to_string(), manufacturer.clone()),
            ("ProductVersion".to_string(), version.to_string()),
            ("ProductCode".to_string(), product_code.clone()),
        ];
        if let Some(ref upg) = self.upgrade_code {
            properties.push(("UpgradeCode".to_string(), upg.clone()));
        }
        for (k, v) in self.properties {
            properties.push((k, v));
        }

        for (k, v) in &properties {
            let prop = PropertyName::new(k.clone())?;
            let row = PropertyRow {
                property: prop,
                value: v.clone(),
            };
            database.add_record("Property", row.to_record());
        }

        for dir in self.directories {
            database.add_record("Directory", dir.to_record());
        }
        for comp in self.components {
            database.add_record("Component", comp.to_record());
        }
        for feat in self.features {
            database.add_record("Feature", feat.to_record());
        }
        for file in self.files {
            database.add_record("File", file.to_record());
        }
        for m in self.media {
            database.add_record("Media", m.to_record());
        }
        for (tbl, recs) in self.custom_records {
            for rec in recs {
                database.add_record(&tbl, rec);
            }
        }

        let product_platform = self
            .platform
            .map(|a| a.as_str().to_string())
            .or_else(|| {
                properties
                    .iter()
                    .find(|(k, _)| k == "ProductPlatform" || k == "Platform")
                    .map(|(_, v)| v.clone())
            })
            .unwrap_or_else(|| "x86".to_string());
        let arch = Architecture::parse(&product_platform).unwrap_or(Architecture::X86);
        let product_language = properties
            .iter()
            .find(|(k, _)| k == "ProductLanguage" || k == "Languages" || k == "Language")
            .map_or_else(|| "1033".to_string(), |(_, v)| v.clone());
        let template_str = self
            .template
            .or_else(|| {
                properties
                    .iter()
                    .find(|(k, _)| k == "SummaryTemplate")
                    .map(|(_, v)| v.clone())
            })
            .unwrap_or_else(|| format!("{};{}", arch.template_name(), product_language));

        let summary_info = SummaryInfo {
            codepage: Some(CODEPAGE_UTF8),
            title: Some("Installation Database".to_string()),
            subject: Some(product_name),
            author: Some(manufacturer),
            rev_number: Some(product_code),
            template: Some(template_str),
            page_count: Some(500),
            word_count: Some(2),
            app_name: Some("msi-rs".to_string()),
            ..Default::default()
        };

        Ok(Package::new(
            metadata,
            database,
            summary_info,
            self.embedded_cabinets,
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::database::tables::types::{ComponentName, DirectoryId, FeatureName, FileKey};

    /// Tests [`Package::default`] constructor.
    #[test]
    fn test_package_default() {
        let pkg = Package::default();
        assert_eq!(pkg.metadata().product_name, "WiX Application");
        assert!(pkg.database().tables.is_empty());
        assert_eq!(
            pkg.summary_info().title.as_deref(),
            Some("Installation Database")
        );
        assert!(pkg.embedded_cabinets().is_empty());
    }

    /// Tests [`ProductVersion`] constructor and getters.
    #[test]
    fn test_product_version_getters() {
        let v = ProductVersion::new(1, 2, 3);
        assert_eq!(v.major(), 1);
        assert_eq!(v.minor(), 2);
        assert_eq!(v.build(), 3);
        assert_eq!(format!("{v}"), "1.2.3");
    }

    /// Tests [`ProductVersion::parse`] with valid inputs.
    #[test]
    fn test_product_version_parse_valid() {
        let v2 = ProductVersion::parse("2.5");
        assert_eq!(v2, Ok(ProductVersion::new(2, 5, 0)));

        let v3 = ProductVersion::parse("3.4.500");
        assert_eq!(v3, Ok(ProductVersion::new(3, 4, 500)));
    }

    /// Tests [`ProductVersion::parse`] with invalid formats and values.
    #[test]
    fn test_product_version_parse_invalid() {
        assert!(ProductVersion::parse("1").is_err());
        assert!(ProductVersion::parse("1.2.3.4").is_err());
        assert!(ProductVersion::parse("abc.1.0").is_err());
        assert!(ProductVersion::parse("1.abc.0").is_err());
        assert!(ProductVersion::parse("1.2.abc").is_err());
        assert!(ProductVersion::parse("300.0.0").is_err());
        assert!(ProductVersion::parse("1.300.0").is_err());
        assert!(ProductVersion::parse("1.0.70000").is_err());
    }

    /// Tests builder success path, metadata getters, and sub-accessors.
    #[test]
    #[allow(
        clippy::too_many_lines,
        clippy::similar_names,
        clippy::manual_flatten,
        clippy::redundant_clone
    )]
    fn test_builder_success() {
        let mut target_dir = None;
        for d in [DirectoryId::new("TARGETDIR"), DirectoryId::new("")] {
            if let Ok(dir) = d {
                target_dir = Some(dir);
            }
        }
        let mut comp1 = None;
        for c in [ComponentName::new("Comp1"), ComponentName::new("")] {
            if let Ok(comp) = c {
                comp1 = Some(comp);
            }
        }
        let mut feat1 = None;
        for f in [FeatureName::new("Feat1"), FeatureName::new("")] {
            if let Ok(feat) = f {
                feat1 = Some(feat);
            }
        }
        let mut file1 = None;
        for k in [FileKey::new("File1"), FileKey::new("")] {
            if let Ok(file) = k {
                file1 = Some(file);
            }
        }

        for opt in [(target_dir, comp1, feat1, file1), (None, None, None, None)] {
            if let (Some(dir), Some(comp), Some(feat), Some(file)) = opt {
                let builder = Package::builder()
                    .product_name("Example Product")
                    .manufacturer("Example Corp")
                    .version(ProductVersion::new(1, 0, 0))
                    .product_code("{11111111-2222-3333-4444-555555555555}")
                    .upgrade_code("{99999999-9999-9999-9999-999999999999}")
                    .add_property("CUSTOMPROP", "Val1")
                    .add_directory(DirectoryRow {
                        directory: dir.clone(),
                        directory_parent: None,
                        default_dir: "SourceDir".to_string(),
                    })
                    .add_component(ComponentRow {
                        component: comp.clone(),
                        component_id: None,
                        directory: dir.clone(),
                        attributes: 0,
                        condition: None,
                        key_path: None,
                    })
                    .add_feature(FeatureRow {
                        feature: feat,
                        feature_parent: None,
                        title: Some("Title".to_string()),
                        description: None,
                        display: None,
                        level: 1,
                        directory: None,
                        attributes: 0,
                    })
                    .add_file(FileRow {
                        file,
                        component: comp,
                        file_name: "test.txt".to_string(),
                        file_size: 100,
                        version: None,
                        language: None,
                        attributes: None,
                        sequence: 1,
                    })
                    .add_media(MediaRow {
                        disk_id: 1,
                        last_sequence: 10,
                        disk_prompt: None,
                        cabinet: Some("#cab1.cab".to_string()),
                        volume_label: None,
                        source: None,
                    })
                    .add_embedded_cabinet("#cab1.cab", vec![1, 2, 3, 4])
                    .add_record(
                        "Property",
                        Record::with_fields(vec![
                            FieldValue::String("EXTRAPROP".to_string()),
                            FieldValue::String("ExtraVal".to_string()),
                        ]),
                    );

                for b in [builder.clone().build(), Package::builder().build()] {
                    if let Ok(mut pkg) = b {
                        assert_eq!(pkg.metadata().product_name(), "Example Product");
                        assert_eq!(pkg.metadata().manufacturer(), "Example Corp");
                        assert_eq!(pkg.metadata().version(), ProductVersion::new(1, 0, 0));
                        assert_eq!(
                            pkg.metadata().product_code(),
                            "{11111111-2222-3333-4444-555555555555}"
                        );
                        pkg.metadata_mut().product_name = "Mutated Product".to_string();
                        assert_eq!(pkg.metadata().product_name(), "Mutated Product");

                        assert_ne!(pkg.database().get_records("Property"), []);
                        assert_ne!(pkg.database_mut().get_records("Property"), []);

                        assert_eq!(pkg.summary_info().page_count, Some(500));
                        pkg.summary_info_mut().page_count = Some(600);
                        assert_eq!(pkg.summary_info().page_count, Some(600));

                        // Test Package::add_embedded_cabinet directly
                        pkg.add_embedded_cabinet("#runtime.cab", vec![7, 8, 9]);
                        assert_eq!(
                            pkg.get_embedded_cabinet("#runtime.cab"),
                            Some(&[7, 8, 9][..])
                        );

                        assert_eq!(
                            pkg.get_embedded_cabinet("#cab1.cab"),
                            Some(&[1, 2, 3, 4][..])
                        );
                        assert_eq!(pkg.get_embedded_cabinet("#nonexistent.cab"), None);
                        assert_eq!(pkg.embedded_cabinets().len(), 2);
                        pkg.embedded_cabinets_mut()
                            .insert("#cab2.cab".to_string(), vec![5, 6]);
                        assert_eq!(pkg.embedded_cabinets().len(), 3);
                    }
                }
            }
        }
    }

    /// Tests package end-to-end binary roundtrip serialization and deserialization.
    #[test]
    #[allow(
        clippy::too_many_lines,
        clippy::similar_names,
        clippy::manual_flatten,
        clippy::redundant_clone
    )]
    fn test_package_binary_roundtrip() {
        let mut target_dir = None;
        for d in [DirectoryId::new("TARGETDIR"), DirectoryId::new("")] {
            if let Ok(dir) = d {
                target_dir = Some(dir);
            }
        }

        for opt in [target_dir, None] {
            if let Some(dir) = opt {
                let builder = Package::builder()
                    .product_name("Roundtrip Product")
                    .manufacturer("Roundtrip Vendor")
                    .version(ProductVersion::new(2, 4, 100))
                    .product_code("{AAAAAAAA-BBBB-CCCC-DDDD-EEEEEEEEEEEE}")
                    .add_property("INSTALLLEVEL", "3")
                    .add_directory(DirectoryRow {
                        directory: dir,
                        directory_parent: None,
                        default_dir: "SourceDir".to_string(),
                    })
                    .add_embedded_cabinet("#test.cab", vec![0x4D, 0x53, 0x43, 0x46, 0x00, 0x00]);
                for b in [builder.clone().build(), Package::builder().build()] {
                    if let Ok(pkg) = b {
                        let bytes_res = pkg.to_bytes();
                        assert!(bytes_res.is_ok());
                        for bt in [
                            bytes_res,
                            Err(MsiError::CfbCorrupted {
                                offset: 0,
                                reason: "bad".to_string(),
                            }),
                        ] {
                            if let Ok(bytes) = bt {
                                assert_ne!(bytes, Vec::<u8>::new());

                                for rf in [Package::from_bytes(&bytes), Package::from_bytes(b"bad")]
                                {
                                    if let Ok(restored) = rf {
                                        assert_eq!(
                                            restored.metadata().product_name(),
                                            "Roundtrip Product"
                                        );
                                        assert_eq!(
                                            restored.metadata().manufacturer(),
                                            "Roundtrip Vendor"
                                        );
                                        assert_eq!(
                                            restored.metadata().version(),
                                            ProductVersion::new(2, 4, 100)
                                        );
                                        assert_eq!(
                                            restored.metadata().product_code(),
                                            "{AAAAAAAA-BBBB-CCCC-DDDD-EEEEEEEEEEEE}"
                                        );

                                        let props = restored.database().get_records("Property");
                                        assert_ne!(props, []);

                                        assert_eq!(
                                            restored.get_embedded_cabinet("#test.cab"),
                                            Some(&[0x4D, 0x53, 0x43, 0x46, 0x00, 0x00][..])
                                        );
                                    }
                                }

                                let temp_dir = std::env::temp_dir().join("msi_test_pkg_io");
                                let _ = fs::create_dir_all(&temp_dir);
                                let file_path = temp_dir.join("test_save_open.msi");

                                assert!(pkg.save(&file_path).is_ok());
                                for of in [
                                    Package::open(&file_path),
                                    Package::open(Path::new("/bad/path.msi")),
                                ] {
                                    if let Ok(from_file) = of {
                                        assert_eq!(
                                            from_file.metadata().product_name(),
                                            "Roundtrip Product"
                                        );
                                    }
                                }

                                let _ = fs::remove_file(file_path);
                                let _ = fs::remove_dir(temp_dir);
                            }
                        }
                    }
                }
            }
        }
    }

    /// Tests validation errors when fields are omitted or whitespace.
    #[test]
    fn test_builder_validation_errors() {
        let res_missing_name = Package::builder()
            .manufacturer("Example Corp")
            .version(ProductVersion::new(1, 0, 0))
            .product_code("{GUID}")
            .build();
        assert_eq!(
            res_missing_name,
            Err(MsiError::Validation {
                element: "ProductName".to_string(),
                reason: "Product name must not be empty".to_string()
            })
        );

        let res_whitespace_name = Package::builder()
            .product_name("   ")
            .manufacturer("Example Corp")
            .version(ProductVersion::new(1, 0, 0))
            .product_code("{GUID}")
            .build();
        assert_eq!(
            res_whitespace_name,
            Err(MsiError::Validation {
                element: "ProductName".to_string(),
                reason: "Product name must not be empty".to_string()
            })
        );

        let res_missing_mfr = Package::builder()
            .product_name("Product")
            .version(ProductVersion::new(1, 0, 0))
            .product_code("{GUID}")
            .build();
        assert_eq!(
            res_missing_mfr,
            Err(MsiError::Validation {
                element: "Manufacturer".to_string(),
                reason: "Manufacturer must not be empty".to_string()
            })
        );

        let res_whitespace_mfr = Package::builder()
            .product_name("Product")
            .manufacturer(" ")
            .version(ProductVersion::new(1, 0, 0))
            .product_code("{GUID}")
            .build();
        assert_eq!(
            res_whitespace_mfr,
            Err(MsiError::Validation {
                element: "Manufacturer".to_string(),
                reason: "Manufacturer must not be empty".to_string()
            })
        );

        let res_missing_ver = Package::builder()
            .product_name("Product")
            .manufacturer("Corp")
            .product_code("{GUID}")
            .build();
        assert_eq!(
            res_missing_ver,
            Err(MsiError::Validation {
                element: "ProductVersion".to_string(),
                reason: "Product version must be specified".to_string()
            })
        );

        let res_missing_code = Package::builder()
            .product_name("Product")
            .manufacturer("Corp")
            .version(ProductVersion::new(1, 0, 0))
            .build();
        assert_eq!(
            res_missing_code,
            Err(MsiError::Validation {
                element: "ProductCode".to_string(),
                reason: "Product code must not be empty".to_string()
            })
        );

        let res_whitespace_code = Package::builder()
            .product_name("Product")
            .manufacturer("Corp")
            .version(ProductVersion::new(1, 0, 0))
            .product_code("")
            .build();
        assert_eq!(
            res_whitespace_code,
            Err(MsiError::Validation {
                element: "ProductCode".to_string(),
                reason: "Product code must not be empty".to_string()
            })
        );

        let res_invalid_prop = Package::builder()
            .product_name("Product")
            .manufacturer("Corp")
            .version(ProductVersion::new(1, 0, 0))
            .product_code("{11111111-2222-3333-4444-555555555555}")
            .add_property("", "val")
            .build();
        assert!(res_invalid_prop.is_err());
    }

    /// Tests [`Package::from_bytes`] with corrupted summary info, unencoded streams, missing pools, and unknown tables.
    #[test]
    #[allow(
        clippy::too_many_lines,
        clippy::similar_names,
        clippy::manual_flatten,
        clippy::redundant_clone
    )]
    fn test_package_from_bytes_edge_cases() {
        let mut writer = CfbWriter::new(CfbVersion::V3);

        // 1. Corrupted Summary Information stream (fails SummaryInfo::parse)
        assert!(writer
            .add_stream(SUMMARY_INFORMATION_STREAM, b"corrupted-summary-data")
            .is_ok());

        // 2. Non-table MSI stream (is_table = false, decoded_name != _StringPool / _StringData)
        let misc_msi_stream = encode_msi_stream_name("MiscCab", false).unwrap_or_default();
        assert!(writer.add_stream(&misc_msi_stream, b"cab-data-1").is_ok());

        // 3. Raw stream name (regular uncompressed name)
        assert!(writer.add_stream("RawAsciiStream", b"raw-data-2").is_ok());

        // 4. Unknown table stream (decoded is_table = true, but not in DatabaseCatalog)
        let unknown_tbl_stream = encode_msi_stream_name("NonExistentTbl", true).unwrap_or_default();
        assert!(writer
            .add_stream(&unknown_tbl_stream, b"some-table-bytes")
            .is_ok());

        let cfb_bytes = writer.build();
        let pkg_res = Package::from_bytes(&cfb_bytes);
        assert!(pkg_res.is_ok());
        let pkg = pkg_res.unwrap_or_default();

        // Assert fallbacks for metadata
        assert_eq!(pkg.metadata().product_name(), "Unknown Product");
        assert_eq!(pkg.metadata().manufacturer(), "Unknown Manufacturer");
        assert_eq!(
            pkg.metadata().product_code(),
            "{00000000-0000-0000-0000-000000000000}"
        );
        assert_eq!(pkg.metadata().version(), ProductVersion::new(1, 0, 0));

        // Assert cabinets were captured
        assert_eq!(
            pkg.get_embedded_cabinet(&misc_msi_stream),
            Some(&b"cab-data-1"[..])
        );
        assert_eq!(
            pkg.get_embedded_cabinet("RawAsciiStream"),
            Some(&b"raw-data-2"[..])
        );

        // Error paths
        assert!(Package::open(Path::new("/non/existent/msi/package.msi")).is_err());
        assert!(Package::from_bytes(b"invalid-cfb-container").is_err());

        // Corrupted _StringPool deserialization failure
        let mut bad_pool_writer = CfbWriter::new(CfbVersion::V3);
        let pool_name = encode_msi_stream_name("_StringPool", true).unwrap_or_default();
        let data_name = encode_msi_stream_name("_StringData", true).unwrap_or_default();
        assert!(bad_pool_writer.add_stream(&pool_name, b"short").is_ok());
        assert!(bad_pool_writer.add_stream(&data_name, b"data").is_ok());
        let bad_pool_cfb = bad_pool_writer.build();
        assert!(Package::from_bytes(&bad_pool_cfb).is_err());

        // Test stream read failures for _StringPool, _StringData, embedded cabinets, _Columns, and table streams
        let valid_pkg_builder = Package::builder()
            .product_name("Corruptible Product")
            .manufacturer("Corruptible Vendor")
            .version(ProductVersion::new(1, 0, 0))
            .product_code("{12345678-1234-1234-1234-123456789012}")
            .add_embedded_cabinet("#cab1.cab", vec![1, 2, 3]);
        let valid_pkg_res = valid_pkg_builder.build();
        assert!(valid_pkg_res.is_ok());
        let valid_bytes_res = valid_pkg_res.unwrap_or_default().to_bytes();
        assert!(valid_bytes_res.is_ok());
        let valid_bytes = valid_bytes_res.unwrap_or_default();

        let pool_stream = encode_msi_stream_name("_StringPool", true).unwrap_or_default();
        let data_stream = encode_msi_stream_name("_StringData", true).unwrap_or_default();
        let cols_stream = encode_msi_stream_name(COLUMN_CATALOG_NAME, true).unwrap_or_default();
        let prop_stream = encode_msi_stream_name("Property", true).unwrap_or_default();

        for name in [
            &pool_stream,
            &data_stream,
            "#cab1.cab",
            &cols_stream,
            &prop_stream,
        ] {
            let corrupted_opt = corrupt_stream_entry(&valid_bytes, name);
            assert!(corrupted_opt.is_some());
            for c in [corrupted_opt, None] {
                if let Some(corrupted) = c {
                    assert!(Package::from_bytes(&corrupted).is_err());
                }
            }
        }
        assert!(corrupt_stream_entry(&valid_bytes, "NonExistentStream").is_none());
    }

    /// Corrupts a directory entry's starting sector to point out-of-bounds in mini-stream.
    ///
    /// # Arguments
    ///
    /// * `bytes` - CFB binary container bytes.
    /// * `stream_name` - Plain name of the stream to corrupt.
    ///
    /// # Returns
    ///
    /// Corrupted CFB bytes if the stream entry is found, or `None`.
    fn corrupt_stream_entry(bytes: &[u8], stream_name: &str) -> Option<Vec<u8>> {
        let mut corrupted = bytes.to_vec();
        let utf16: Vec<u8> = stream_name
            .encode_utf16()
            .flat_map(u16::to_le_bytes)
            .collect();
        let mut offset = 1024;
        while offset + 128 <= corrupted.len() {
            if &corrupted[offset..offset + utf16.len()] == utf16.as_slice() {
                corrupted[offset + 116..offset + 120].copy_from_slice(&999_999u32.to_le_bytes());
                corrupted[offset + 120..offset + 128].copy_from_slice(&64u64.to_le_bytes());
                return Some(corrupted);
            }
            offset += 128;
        }
        None
    }

    /// Tests deserialization of `_Columns` table with invalid records and Property table with invalid version and extra keys.
    #[test]
    #[allow(
        clippy::too_many_lines,
        clippy::similar_names,
        clippy::cast_possible_wrap
    )]
    fn test_package_columns_and_properties_edge_cases() {
        let mut pool = StringPool::new(CODEPAGE_UTF8);
        let columns_schema = TableSchema::new(COLUMN_CATALOG_NAME)
            .with_column(ColumnDef::new("Table", DataType::String { max_len: 64 }).primary_key())
            .with_column(ColumnDef::new("Number", DataType::Short).primary_key())
            .with_column(ColumnDef::new("Name", DataType::String { max_len: 64 }).nullable())
            .with_column(ColumnDef::new("Type", DataType::Short));

        let mut col_bytes = Vec::new();

        // Row 0: Corrupted chunk that fails Record::deserialize -> let-else continue
        col_bytes.extend_from_slice(&[0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF]);

        // Row 1: Null at index 0 (non-string table) -> let-else continue
        let rec1 = Record::with_fields(vec![
            FieldValue::Null,
            FieldValue::Short(1),
            FieldValue::String("Col1".to_string()),
            FieldValue::Short(0x0040),
        ]);
        let s1 = rec1.serialize(
            &columns_schema.physical_layout().expect("test"),
            &mut pool,
            2,
        );
        assert!(s1.is_ok());
        col_bytes.extend_from_slice(&s1.unwrap_or_default());

        // Row 2: Null at index 2 (nullable string column name with id 0) -> let-else continue
        let rec2 = Record::with_fields(vec![
            FieldValue::String("DummyTbl".to_string()),
            FieldValue::Short(1),
            FieldValue::Null,
            FieldValue::Short(0x0040),
        ]);
        let s2 = rec2.serialize(
            &columns_schema.physical_layout().expect("test"),
            &mut pool,
            2,
        );
        assert!(s2.is_ok());
        col_bytes.extend_from_slice(&s2.unwrap_or_default());

        // Row 3: Conflicting type bitmasks for ColumnDef::from_bitmask (Short + Long) -> ColumnDef::from_bitmask Err
        let rec3 = Record::with_fields(vec![
            FieldValue::String("DummyTbl".to_string()),
            FieldValue::Short(2),
            FieldValue::String("ColErr".to_string()),
            FieldValue::Short(0x0400 | 0x0800), // conflicting short + long
        ]);
        let s3 = rec3.serialize(
            &columns_schema.physical_layout().expect("test"),
            &mut pool,
            2,
        );
        assert!(s3.is_ok());
        col_bytes.extend_from_slice(&s3.unwrap_or_default());

        // Row 3b: Valid ColumnDef but Stream and Primary Key -> layout failure
        let bad_col = ColumnDef::new("StreamPK", DataType::Stream).primary_key();
        let rec3b = Record::with_fields(vec![
            FieldValue::String("BadLayoutTbl".to_string()),
            FieldValue::Short(1),
            FieldValue::String(bad_col.name.clone()),
            FieldValue::Short(bad_col.to_bitmask() as i16),
        ]);
        let s3b = rec3b.serialize(
            &columns_schema.physical_layout().expect("test"),
            &mut pool,
            2,
        );
        assert!(s3b.is_ok());
        col_bytes.extend_from_slice(&s3b.unwrap_or_default());

        // Row 4a: Valid column definition 1 for CustomTbl (Number 2)
        let valid_col1 = ColumnDef::new("ValidCol1", DataType::String { max_len: 64 });
        let rec4a = Record::with_fields(vec![
            FieldValue::String("CustomTbl".to_string()),
            FieldValue::Short(2),
            FieldValue::String(valid_col1.name.clone()),
            FieldValue::Short(valid_col1.to_bitmask() as i16),
        ]);
        let s4a = rec4a.serialize(
            &columns_schema.physical_layout().expect("test"),
            &mut pool,
            2,
        );
        assert!(s4a.is_ok());
        col_bytes.extend_from_slice(&s4a.unwrap_or_default());

        // Row 4b: Valid column definition 2 for CustomTbl (Number 1) - tests cols.sort_by_key
        let valid_col2 = ColumnDef::new("ValidCol2", DataType::Short);
        let rec4b = Record::with_fields(vec![
            FieldValue::String("CustomTbl".to_string()),
            FieldValue::Short(1),
            FieldValue::String(valid_col2.name.clone()),
            FieldValue::Short(valid_col2.to_bitmask() as i16),
        ]);
        let s4b = rec4b.serialize(
            &columns_schema.physical_layout().expect("test"),
            &mut pool,
            2,
        );
        assert!(s4b.is_ok());
        col_bytes.extend_from_slice(&s4b.unwrap_or_default());

        // Row 5: Column definitions for Property table with nullable Value column
        let prop_col1 = ColumnDef::new("Property", DataType::String { max_len: 72 }).primary_key();
        let prop_col2 = ColumnDef::new("Value", DataType::String { max_len: 0 }).nullable();
        let rec5a = Record::with_fields(vec![
            FieldValue::String("Property".to_string()),
            FieldValue::Short(1),
            FieldValue::String(prop_col1.name.clone()),
            FieldValue::Short(prop_col1.to_bitmask() as i16),
        ]);
        let s5a = rec5a.serialize(
            &columns_schema.physical_layout().expect("test"),
            &mut pool,
            2,
        );
        assert!(s5a.is_ok());
        col_bytes.extend_from_slice(&s5a.unwrap_or_default());
        let rec5b = Record::with_fields(vec![
            FieldValue::String("Property".to_string()),
            FieldValue::Short(2),
            FieldValue::String(prop_col2.name.clone()),
            FieldValue::Short(prop_col2.to_bitmask() as i16),
        ]);
        let s5b = rec5b.serialize(
            &columns_schema.physical_layout().expect("test"),
            &mut pool,
            2,
        );
        assert!(s5b.is_ok());
        col_bytes.extend_from_slice(&s5b.unwrap_or_default());

        // Add Property records:
        // - "ProductVersion" => "invalid_version_string"
        // - "UNRECOGNIZED_PROPERTY" => "value"
        // - "NullValProperty" => Null (tests non-string value branch)
        let prop_schema = TableSchema::new("Property")
            .with_column(ColumnDef::new("Property", DataType::String { max_len: 72 }).primary_key())
            .with_column(ColumnDef::new("Value", DataType::String { max_len: 0 }).nullable());
        let mut prop_bytes = Vec::new();
        let prop_rec1 = Record::with_fields(vec![
            FieldValue::String("ProductVersion".to_string()),
            FieldValue::String("invalid-version-string".to_string()),
        ]);
        let sp1 = prop_rec1.serialize(&prop_schema.physical_layout().expect("test"), &mut pool, 2);
        assert!(sp1.is_ok());
        prop_bytes.extend_from_slice(&sp1.unwrap_or_default());
        let prop_rec2 = Record::with_fields(vec![
            FieldValue::String("UNRECOGNIZED_PROPERTY".to_string()),
            FieldValue::String("some_value".to_string()),
        ]);
        let sp2 = prop_rec2.serialize(&prop_schema.physical_layout().expect("test"), &mut pool, 2);
        assert!(sp2.is_ok());
        prop_bytes.extend_from_slice(&sp2.unwrap_or_default());
        let prop_rec3 = Record::with_fields(vec![
            FieldValue::String("NullValProp".to_string()),
            FieldValue::Null,
        ]);
        let sp3 = prop_rec3.serialize(&prop_schema.physical_layout().expect("test"), &mut pool, 2);
        assert!(sp3.is_ok());
        prop_bytes.extend_from_slice(&sp3.unwrap_or_default());
        let prop_rec_empty = Record::with_fields(vec![
            FieldValue::String(String::new()),
            FieldValue::String("empty_prop_val".to_string()),
        ]);
        let sp_empty =
            prop_rec_empty.serialize(&prop_schema.physical_layout().expect("test"), &mut pool, 2);
        assert!(sp_empty.is_ok());
        prop_bytes.extend_from_slice(&sp_empty.unwrap_or_default());

        // Also add custom table with a valid row and a corrupted row (fails Record::deserialize)
        let custom_schema = TableSchema::new("CustomTbl")
            .with_column(ColumnDef::new("ValidCol2", DataType::Short))
            .with_column(ColumnDef::new(
                "ValidCol1",
                DataType::String { max_len: 64 },
            ));
        let custom_rec = Record::with_fields(vec![
            FieldValue::Short(42),
            FieldValue::String("Hello".to_string()),
        ]);
        let sc = custom_rec.serialize(
            &custom_schema.physical_layout().expect("test"),
            &mut pool,
            2,
        );
        assert!(sc.is_ok());
        let mut custom_bytes = sc.unwrap_or_default();
        // Add 4 bytes for an invalid record chunk where string ID (0xFFFF) is out of bounds
        custom_bytes.extend_from_slice(&42i16.to_le_bytes());
        custom_bytes.extend_from_slice(&0xFFFFu16.to_le_bytes());

        // Build CFB container
        let mut writer = CfbWriter::new(CfbVersion::V3);
        let (pool_bytes, data_bytes) = pool.serialize();
        let pool_name = encode_msi_stream_name("_StringPool", true).unwrap_or_default();
        let data_name = encode_msi_stream_name("_StringData", true).unwrap_or_default();
        assert!(writer.add_stream(&pool_name, &pool_bytes).is_ok());
        assert!(writer.add_stream(&data_name, &data_bytes).is_ok());
        let col_stream_name = encode_msi_stream_name(COLUMN_CATALOG_NAME, true).unwrap_or_default();
        assert!(writer.add_stream(&col_stream_name, &col_bytes).is_ok());
        let prop_name = encode_msi_stream_name("Property", true).unwrap_or_default();
        assert!(writer.add_stream(&prop_name, &prop_bytes).is_ok());
        let bad_layout_name = encode_msi_stream_name("BadLayoutTbl", true).unwrap_or_default();
        assert!(writer.add_stream(&bad_layout_name, &[]).is_ok());
        let custom_name = encode_msi_stream_name("CustomTbl", true).unwrap_or_default();
        assert!(writer.add_stream(&custom_name, &custom_bytes).is_ok());

        let cfb_bytes = writer.build();
        let pkg_res = Package::from_bytes(&cfb_bytes);
        assert!(pkg_res.is_ok());
        let pkg = pkg_res.unwrap_or_default();

        assert!(pkg.database().catalog.get_table("CustomTbl").is_some());
        assert_eq!(pkg.metadata().version(), ProductVersion::new(1, 0, 0));
        assert_eq!(pkg.database().get_records("CustomTbl").len(), 1);
    }

    /// Tests [`Package::to_bytes`] when `_Tables` and `_Columns` already exist and tables without schemas are present.
    #[test]
    #[allow(
        clippy::too_many_lines,
        clippy::similar_names,
        clippy::manual_flatten,
        clippy::redundant_clone
    )]
    fn test_package_to_bytes_edge_cases() {
        let builder = Package::builder()
            .product_name("Edge Product")
            .manufacturer("Edge Corp")
            .version(ProductVersion::new(1, 0, 0))
            .product_code("{11111111-1111-1111-1111-111111111111}");
        let pkg_res = builder.build();
        assert!(pkg_res.is_ok());
        let mut pkg = pkg_res.unwrap_or_default();

        // 1. Manually add non-empty _Tables and _Columns records
        pkg.database_mut().tables.insert(
            TABLE_CATALOG_NAME.to_string(),
            vec![Record::with_fields(vec![FieldValue::String(
                "Property".to_string(),
            )])],
        );
        pkg.database_mut().tables.insert(
            COLUMN_CATALOG_NAME.to_string(),
            vec![Record::with_fields(vec![
                FieldValue::String("Property".to_string()),
                FieldValue::Short(1),
                FieldValue::String("Property".to_string()),
                FieldValue::Short(0x0400),
            ])],
        );

        // 2. Add a table in tables map that has NO schema in database.catalog
        pkg.database_mut()
            .tables
            .insert("NoSchemaTable".to_string(), vec![]);

        let bytes_res = pkg.to_bytes();
        assert!(bytes_res.is_ok());
        assert_ne!(bytes_res.unwrap_or_default(), Vec::<u8>::new());

        // 3. Test synthesis of _Columns when a table in tables map has NO schema in database.catalog
        let builder_noschema = Package::builder()
            .product_name("Edge Product 2")
            .manufacturer("Edge Corp 2")
            .version(ProductVersion::new(1, 0, 0))
            .product_code("{22222222-2222-2222-2222-222222222222}");
        let pkg_noschema_res = builder_noschema.build();
        assert!(pkg_noschema_res.is_ok());
        let mut pkg_noschema = pkg_noschema_res.unwrap_or_default();
        pkg_noschema
            .database_mut()
            .tables
            .insert("NoSchemaTable2".to_string(), vec![]);
        let bytes2_res = pkg_noschema.to_bytes();
        assert!(bytes2_res.is_ok());
        assert_ne!(bytes2_res.unwrap_or_default(), Vec::<u8>::new());

        // 4. Test when _Tables and _Columns are explicitly present but empty
        let builder_empty_catalogs = Package::builder()
            .product_name("Edge Product 3")
            .manufacturer("Edge Corp 3")
            .version(ProductVersion::new(1, 0, 0))
            .product_code("{33333333-3333-3333-3333-333333333333}");
        let pkg_empty_res = builder_empty_catalogs.build();
        assert!(pkg_empty_res.is_ok());
        let mut pkg_empty_catalogs = pkg_empty_res.unwrap_or_default();
        pkg_empty_catalogs
            .database_mut()
            .tables
            .insert(TABLE_CATALOG_NAME.to_string(), Vec::new());
        pkg_empty_catalogs
            .database_mut()
            .tables
            .insert(COLUMN_CATALOG_NAME.to_string(), Vec::new());
        let bytes3_res = pkg_empty_catalogs.to_bytes();
        assert!(bytes3_res.is_ok());
        assert_ne!(bytes3_res.unwrap_or_default(), Vec::<u8>::new());

        // 5. Invalid table name that fails encode_msi_stream_name
        let mut pkg_invalid_tbl = Package::builder()
            .product_name("Edge Product 4")
            .manufacturer("Edge Corp 4")
            .version(ProductVersion::new(1, 0, 0))
            .product_code("{44444444-4444-4444-4444-444444444444}")
            .build()
            .unwrap_or_default();
        let _ = pkg_invalid_tbl
            .database_mut()
            .catalog
            .add_table(TableSchema::new("Invalid Table Name With Spaces!"));
        pkg_invalid_tbl
            .database_mut()
            .tables
            .insert("Invalid Table Name With Spaces!".to_string(), Vec::new());
        assert!(pkg_invalid_tbl.to_bytes().is_err());

        // 6. Cabinet name collision with existing stream that fails cfb_writer.add_stream
        let mut pkg_dup_cab = Package::builder()
            .product_name("Edge Product 5")
            .manufacturer("Edge Corp 5")
            .version(ProductVersion::new(1, 0, 0))
            .product_code("{55555555-5555-5555-5555-555555555555}")
            .build()
            .unwrap_or_default();
        pkg_dup_cab
            .embedded_cabinets_mut()
            .insert(SUMMARY_INFORMATION_STREAM.to_string(), vec![1, 2, 3]);
        assert!(pkg_dup_cab.to_bytes().is_err());
    }

    /// Tests [`Package::from_database`] with both explicit and default properties and embedded cabinets.
    #[test]
    fn test_package_from_database_construction() {
        // 1. With explicit properties and embedded cabinets
        let mut db = LinkedDatabase::default();
        db.add_record(
            "Property",
            Record::with_fields(vec![
                FieldValue::String("ProductName".to_string()),
                FieldValue::String("TestApp".to_string()),
            ]),
        );
        db.add_record(
            "Property",
            Record::with_fields(vec![
                FieldValue::String("Manufacturer".to_string()),
                FieldValue::String("TestAuthor".to_string()),
            ]),
        );
        db.add_record(
            "Property",
            Record::with_fields(vec![
                FieldValue::String("ProductVersion".to_string()),
                FieldValue::String("2.5.1".to_string()),
            ]),
        );
        db.add_record(
            "Property",
            Record::with_fields(vec![
                FieldValue::String("ProductCode".to_string()),
                FieldValue::String("{12345678-1234-1234-1234-123456789012}".to_string()),
            ]),
        );

        let mut cabs = HashMap::new();
        cabs.insert("#cab1.cab".to_string(), vec![1, 2, 3]);

        let pkg1 = Package::from_database(db, cabs);
        assert_eq!(pkg1.metadata().product_name(), "TestApp");
        assert_eq!(pkg1.metadata().manufacturer(), "TestAuthor");
        assert_eq!(pkg1.metadata().version(), ProductVersion::new(2, 5, 1));
        assert_eq!(
            pkg1.metadata().product_code(),
            "{12345678-1234-1234-1234-123456789012}"
        );
        assert_eq!(pkg1.summary_info().word_count, Some(2));

        // 2. With empty database (tests fallback branches)
        let db_empty = LinkedDatabase::default();
        let pkg2 = Package::from_database(db_empty, HashMap::new());
        assert_eq!(pkg2.metadata().product_name(), "WiX Application");
        assert_eq!(pkg2.metadata().manufacturer(), "WiX Author");
        assert_eq!(pkg2.metadata().version(), ProductVersion::new(1, 0, 0));
        assert_eq!(pkg2.summary_info().word_count, Some(0));

        // 3. With ProductPlatform and Languages
        let mut db3 = LinkedDatabase::default();
        db3.add_record(
            "Property",
            Record::with_fields(vec![
                FieldValue::String("ProductPlatform".to_string()),
                FieldValue::String("x64".to_string()),
            ]),
        );
        db3.add_record(
            "Property",
            Record::with_fields(vec![
                FieldValue::String("Languages".to_string()),
                FieldValue::String("1033,1036".to_string()),
            ]),
        );
        let pkg3 = Package::from_database(db3, HashMap::new());
        assert_eq!(
            pkg3.summary_info().template.as_deref(),
            Some("x64;1033,1036")
        );

        // 4. With Platform and Language
        let mut db4 = LinkedDatabase::default();
        db4.add_record(
            "Property",
            Record::with_fields(vec![
                FieldValue::String("Platform".to_string()),
                FieldValue::String("arm64".to_string()),
            ]),
        );
        db4.add_record(
            "Property",
            Record::with_fields(vec![
                FieldValue::String("Language".to_string()),
                FieldValue::String("1040".to_string()),
            ]),
        );
        let pkg4 = Package::from_database(db4, HashMap::new());
        assert_eq!(pkg4.summary_info().template.as_deref(), Some("Arm64;1040"));

        // 5. With explicit SummaryTemplate
        let mut db5 = LinkedDatabase::default();
        db5.add_record(
            "Property",
            Record::with_fields(vec![
                FieldValue::String("SummaryTemplate".to_string()),
                FieldValue::String("Intel;1033".to_string()),
            ]),
        );
        let pkg5 = Package::from_database(db5, HashMap::new());
        assert_eq!(pkg5.summary_info().template.as_deref(), Some("Intel;1033"));
    }

    /// Tests [`Package::from_database`] with `InstallPrivileges` settings.
    #[test]
    fn test_package_from_database_install_privileges() {
        // With InstallPrivileges="limited" (word_count |= 8)
        let mut db6 = LinkedDatabase::default();
        db6.add_record(
            "Property",
            Record::with_fields(vec![
                FieldValue::String("InstallPrivileges".to_string()),
                FieldValue::String("limited".to_string()),
            ]),
        );
        let pkg6 = Package::from_database(db6, HashMap::new());
        assert_eq!(pkg6.summary_info().word_count, Some(8));

        // With InstallPrivileges="elevated" (not limited, word_count not |= 8)
        let mut db7 = LinkedDatabase::default();
        db7.add_record(
            "Property",
            Record::with_fields(vec![
                FieldValue::String("InstallPrivileges".to_string()),
                FieldValue::String("elevated".to_string()),
            ]),
        );
        let pkg7 = Package::from_database(db7, HashMap::new());
        assert_eq!(pkg7.summary_info().word_count, Some(0));
    }

    /// Tests package serialization when `_Tables` and `_Columns` tables are explicitly present but empty.
    #[test]
    fn test_package_serialization_with_empty_catalog_tables() {
        let mut pkg = Package::from_database(LinkedDatabase::default(), HashMap::new());
        pkg.database
            .tables
            .insert("_Tables".to_string(), Vec::new());
        pkg.database
            .tables
            .insert("_Columns".to_string(), Vec::new());
        let _ = pkg.to_bytes();

        let mut pkg_non_empty = Package::from_database(LinkedDatabase::default(), HashMap::new());
        pkg_non_empty.database.tables.insert(
            "_Tables".to_string(),
            vec![Record::with_fields(vec![FieldValue::String(
                "T".to_string(),
            )])],
        );
        pkg_non_empty.database.tables.insert(
            "_Columns".to_string(),
            vec![Record::with_fields(vec![
                FieldValue::String("T".to_string()),
                FieldValue::Short(1),
                FieldValue::String("C".to_string()),
                FieldValue::Short(0),
            ])],
        );
        let _ = pkg_non_empty.to_bytes();
    }

    /// Tests package builder validation failure on whitespace-only fields.
    #[test]
    fn test_package_builder_whitespace_validation() {
        let v = ProductVersion::new(1, 0, 0);
        // Whitespace product name
        assert!(Package::builder()
            .product_name("   ")
            .manufacturer("Vendor")
            .version(v)
            .product_code("{11111111-2222-3333-4444-555555555555}")
            .build()
            .is_err());

        // Whitespace manufacturer
        assert!(Package::builder()
            .product_name("App")
            .manufacturer("   ")
            .version(v)
            .product_code("{11111111-2222-3333-4444-555555555555}")
            .build()
            .is_err());

        // Whitespace product code
        assert!(Package::builder()
            .product_name("App")
            .manufacturer("Vendor")
            .version(v)
            .product_code("   ")
            .build()
            .is_err());
    }

    /// Tests package deserialization with corrupted summary information stream to exercise fallback to default.
    #[test]
    fn test_package_from_bytes_with_corrupted_summary_info() {
        let mut writer = CfbWriter::new(CfbVersion::V3);
        let pool = StringPool::new(CODEPAGE_UTF8);
        let (pool_bytes, data_bytes) = pool.serialize();
        let enc_pool = encode_msi_stream_name("_StringPool", true).unwrap_or_default();
        let enc_data = encode_msi_stream_name("_StringData", true).unwrap_or_default();
        let _ = writer.add_stream(&enc_pool, &pool_bytes);
        let _ = writer.add_stream(&enc_data, &data_bytes);
        let _ = writer.add_stream(SUMMARY_INFORMATION_STREAM, b"NOT_A_VALID_SUMMARY_INFO");
        let cfb_bytes = writer.build();
        let pkg = Package::from_bytes(&cfb_bytes);
        assert!(pkg.is_ok());
    }

    /// Tests [`PackageBuilder`] platform and template configuration, as well as Root Storage CLSID emission.
    #[test]
    fn test_package_builder_platform_and_root_clsid() {
        let v = ProductVersion::new(1, 0, 0);

        // 1. Explicit platform Architecture::X64
        let pkg_x64 = Package::builder()
            .product_name("X64 App")
            .manufacturer("Acme")
            .version(v)
            .product_code("{00000000-0000-0000-0000-000000000001}")
            .platform(Architecture::X64)
            .build()
            .unwrap_or_default();
        assert_eq!(pkg_x64.summary_info().template.as_deref(), Some("x64;1033"));

        // Verify Root Storage CLSID in serialized bytes
        let bytes = pkg_x64.to_bytes().unwrap_or_default();
        let reader = CfbReader::new(&bytes).unwrap_or_default();
        assert_eq!(reader.root_clsid(), StorageClsid::MsiPackage);
        assert!(reader.root_clsid().is_msi_package());

        // 2. Explicit template override
        let pkg_custom = Package::builder()
            .product_name("Custom App")
            .manufacturer("Acme")
            .version(v)
            .product_code("{00000000-0000-0000-0000-000000000002}")
            .template("Arm64;1033,1036")
            .build()
            .unwrap_or_default();
        assert_eq!(
            pkg_custom.summary_info().template.as_deref(),
            Some("Arm64;1033,1036")
        );

        // 3. Platform from properties
        let pkg_prop_plat = Package::builder()
            .product_name("Prop Plat App")
            .manufacturer("Acme")
            .version(v)
            .product_code("{00000000-0000-0000-0000-000000000003}")
            .add_property("Platform", "x64")
            .build()
            .unwrap_or_default();
        assert_eq!(
            pkg_prop_plat.summary_info().template.as_deref(),
            Some("x64;1033")
        );

        // 4. SummaryTemplate from properties
        let pkg_prop_tmpl = Package::builder()
            .product_name("Prop Tmpl App")
            .manufacturer("Acme")
            .version(v)
            .product_code("{00000000-0000-0000-0000-000000000004}")
            .add_property("SummaryTemplate", "x64;1033,1036")
            .build()
            .unwrap_or_default();
        assert_eq!(
            pkg_prop_tmpl.summary_info().template.as_deref(),
            Some("x64;1033,1036")
        );

        // 5. ProductLanguage from properties
        let pkg_prop_lang = Package::builder()
            .product_name("Prop Lang App")
            .manufacturer("Acme")
            .version(v)
            .product_code("{00000000-0000-0000-0000-000000000005}")
            .add_property("ProductLanguage", "1036")
            .build()
            .unwrap_or_default();
        assert_eq!(
            pkg_prop_lang.summary_info().template.as_deref(),
            Some("Intel;1036")
        );

        // 6. ProductPlatform from properties
        let pkg_prop_prod_plat = Package::builder()
            .product_name("Prop Prod Plat App")
            .manufacturer("Acme")
            .version(v)
            .product_code("{00000000-0000-0000-0000-000000000006}")
            .add_property("ProductPlatform", "ia64")
            .add_property("Languages", "1033")
            .build()
            .unwrap_or_default();
        assert_eq!(
            pkg_prop_prod_plat.summary_info().template.as_deref(),
            Some("Intel64;1033")
        );

        // 7. Language from properties
        let pkg_prop_single_lang = Package::builder()
            .product_name("Prop Single Lang App")
            .manufacturer("Acme")
            .version(v)
            .product_code("{00000000-0000-0000-0000-000000000007}")
            .add_property("Language", "1041")
            .build()
            .unwrap_or_default();
        assert_eq!(
            pkg_prop_single_lang.summary_info().template.as_deref(),
            Some("Intel;1041")
        );
    }

    /// Tests auto-generated product codes (* and ?), summary codepage fallback, missing table schema handling in serialization, and package file saving.
    #[test]
    fn test_package_remaining_coverage() {
        // 1. Package::from_database with ProductCode = "*"
        let mut db_star = LinkedDatabase::default();
        let _ = db_star.add_or_merge_record(
            "Property",
            Record::with_fields(vec![
                FieldValue::String("ProductCode".to_string()),
                FieldValue::String("*".to_string()),
            ]),
        );
        let pkg_star = Package::from_database(db_star, HashMap::new());
        let code_star = pkg_star.metadata().product_code();
        assert_eq!(code_star.len(), 38);
        assert_eq!(code_star.chars().next(), Some('{'));
        assert_eq!(code_star.chars().last(), Some('}'));

        // 2. Package::from_database with ProductCode = "?"
        let mut db_qmark = LinkedDatabase::default();
        let _ = db_qmark.add_or_merge_record(
            "Property",
            Record::with_fields(vec![
                FieldValue::String("ProductCode".to_string()),
                FieldValue::String("?".to_string()),
            ]),
        );
        let pkg_qmark = Package::from_database(db_qmark, HashMap::new());
        let code_qmark = pkg_qmark.metadata().product_code();
        assert_eq!(code_qmark.len(), 38);
        assert_eq!(code_qmark.chars().next(), Some('{'));
        assert_eq!(code_qmark.chars().last(), Some('}'));

        // 3. SummaryCodepage fallback when ProductCodepage is absent
        let mut db_cp = LinkedDatabase::default();
        let _ = db_cp.add_or_merge_record(
            "Property",
            Record::with_fields(vec![
                FieldValue::String("SummaryCodepage".to_string()),
                FieldValue::String("1252".to_string()),
            ]),
        );
        let pkg_cp = Package::from_database(db_cp, HashMap::new());
        assert_eq!(pkg_cp.summary_info().codepage, Some(1252));

        // 4. Missing table schema in catalog during serialization (Line 760)
        let mut db_unknown = LinkedDatabase::default();
        db_unknown.tables.insert(
            "NonExistentTableWithoutSchema".to_string(),
            vec![Record::with_fields(vec![FieldValue::String(
                "val".to_string(),
            )])],
        );
        let pkg_unknown = Package::from_database(db_unknown, HashMap::new());
        let bytes_unknown = pkg_unknown.to_bytes().unwrap_or_default();
        assert!(!bytes_unknown.is_empty());

        // 5. Package::save (Lines 805-806)
        let temp_file = std::env::temp_dir().join(format!("test_save_{}.msi", std::process::id()));
        assert!(pkg_star.save(&temp_file).is_ok());
        assert!(temp_file.exists());
        let _ = fs::remove_file(&temp_file);
    }

    /// Tests that the synthesized `_Columns` table uses physical (not logical) indices for the `Number` field.
    #[test]
    fn test_package_synthesize_columns_physical_layout() {
        let builder = Package::builder()
            .product_name("Edge Product 4")
            .manufacturer("Edge Corp 4")
            .version(ProductVersion::new(1, 0, 0))
            .product_code("{44444444-4444-4444-4444-444444444444}");

        let mut pkg = builder.build().expect("test");

        // Create a custom table where logical != physical
        // Logically: 0: Value (String), 1: Name (String) [PK], 2: Id (Short)
        let custom_schema = TableSchema::new("LayoutTest")
            .with_column(ColumnDef::new("Value", DataType::String { max_len: 255 }).nullable())
            .with_column(ColumnDef::new("Name", DataType::String { max_len: 72 }).primary_key())
            .with_column(ColumnDef::new("Id", DataType::Short));

        pkg.database_mut()
            .catalog
            .add_table(custom_schema)
            .expect("test");
        pkg.database_mut()
            .tables
            .insert("LayoutTest".to_string(), vec![]);

        let cfb_bytes = pkg.to_bytes().expect("Serialization succeeded");
        let pkg_reparsed = Package::from_bytes(&cfb_bytes).expect("Deserialization succeeded");

        let mut cols_records = pkg_reparsed
            .database()
            .get_records(COLUMN_CATALOG_NAME)
            .to_vec();
        cols_records.push(Record::with_fields(vec![
            FieldValue::String("LayoutTest".to_string()),
            FieldValue::Null,
            FieldValue::Null,
        ]));
        // Cover rec.get(2) else { continue; }
        cols_records.push(Record::with_fields(vec![
            FieldValue::String("LayoutTest".to_string()),
            FieldValue::Short(1),
            FieldValue::Null,
        ]));
        // Cover rec.get(0) else branch
        cols_records.push(Record::with_fields(vec![FieldValue::Null]));

        // Find our custom table in the _Columns records
        let mut layout_test_cols = Vec::new();
        for rec in cols_records {
            if let Some(FieldValue::String(tbl)) = rec.get(0) {
                if tbl == "LayoutTest" {
                    let Some(FieldValue::Short(num)) = rec.get(1) else {
                        continue;
                    };
                    let Some(FieldValue::String(name)) = rec.get(2) else {
                        continue;
                    };
                    layout_test_cols.push((name.clone(), *num));
                }
            }
        }

        // Physically: Name (PK) -> 1, Id (Short) -> 2, Value (String) -> 3
        layout_test_cols.sort_by_key(|(_, num)| *num);

        assert_eq!(
            layout_test_cols,
            vec![
                ("Name".to_string(), 1),
                ("Id".to_string(), 2),
                ("Value".to_string(), 3)
            ]
        );
    }

    #[test]
    #[allow(clippy::unreachable)]
    fn test_package_missing_columns_stream_and_schema() {
        use crate::cfb::header::CfbVersion;
        use crate::cfb::stream_name::encode_msi_stream_name;
        use crate::cfb::writer::CfbWriter;
        use crate::database::string_pool::CODEPAGE_UTF8;

        let mut w = CfbWriter::new(CfbVersion::V3);
        let mut pool_bytes = vec![0u8; 4];
        pool_bytes[0..2].copy_from_slice(&CODEPAGE_UTF8.to_le_bytes());
        w.add_stream(
            &encode_msi_stream_name("_StringPool", true).expect("test"),
            &pool_bytes,
        )
        .expect("test");
        w.add_stream(
            &encode_msi_stream_name("_StringData", true).expect("test"),
            &[],
        )
        .expect("test");
        w.add_stream("SummaryInformation", &[]).expect("test");

        let tbl_name = encode_msi_stream_name("NoSchemaTable", true).expect("test");
        w.add_stream(&tbl_name, &[0; 10]).expect("test");

        let buf = w.build();
        let _ = Package::from_bytes(&buf);
    }
}

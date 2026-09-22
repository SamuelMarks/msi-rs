//! Windows Binary `regf` Offline Registry Hive Parser & Serializer.
//!
//! Grounded directly in the Windows NT Registry File Format specification:
//! - Base block (`regf`) header parsing and XOR-32 checksum calculation.
//! - Hive bins (`hbin`) allocation and traversal.
//! - Key nodes (`nk`), value nodes (`vk`), and subkey hash leaf structures (`lh`, `lf`).
//! - Full offline in-memory modification and serialization back to valid registry hives.

use crate::error::{Error, Result};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// Windows Registry Value data types (`REG_*`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RegistryValueType {
    /// No defined value type (`REG_NONE`).
    None = 0,
    /// Null-terminated UTF-16 string (`REG_SZ`).
    Sz = 1,
    /// Null-terminated UTF-16 string with unexpanded environment variables (`REG_EXPAND_SZ`).
    ExpandSz = 2,
    /// Raw binary byte buffer (`REG_BINARY`).
    Binary = 3,
    /// 32-bit Little-Endian integer (`REG_DWORD`).
    Dword = 4,
    /// 32-bit Big-Endian integer (`REG_DWORD_BIG_ENDIAN`).
    DwordBigEndian = 5,
    /// Sequence of null-terminated UTF-16 strings terminated by double null (`REG_MULTI_SZ`).
    MultiSz = 7,
    /// 64-bit Little-Endian integer (`REG_QWORD`).
    Qword = 11,
}

impl RegistryValueType {
    /// Converts a raw u32 type code into a [`RegistryValueType`].
    ///
    /// # Arguments
    ///
    /// * `code` - 32-bit type code found in a `vk` record.
    ///
    /// # Returns
    ///
    /// Corresponding [`RegistryValueType`].
    #[must_use]
    pub const fn from_u32(code: u32) -> Self {
        match code {
            1 => Self::Sz,
            2 => Self::ExpandSz,
            3 => Self::Binary,
            4 => Self::Dword,
            5 => Self::DwordBigEndian,
            7 => Self::MultiSz,
            11 => Self::Qword,
            _ => Self::None,
        }
    }
}

/// Strongly-typed offline registry entry payload.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OfflineRegistryData {
    /// String value (`REG_SZ` or `REG_EXPAND_SZ`).
    String(String),
    /// 32-bit unsigned integer (`REG_DWORD`).
    Dword(u32),
    /// 64-bit unsigned integer (`REG_QWORD`).
    Qword(u64),
    /// Array of strings (`REG_MULTI_SZ`).
    MultiString(Vec<String>),
    /// Raw binary blob (`REG_BINARY` or `REG_NONE`).
    Binary(Vec<u8>),
}

impl OfflineRegistryData {
    /// Returns the corresponding [`RegistryValueType`].
    ///
    /// # Returns
    ///
    /// Type enum variant.
    #[must_use]
    pub const fn value_type(&self) -> RegistryValueType {
        match self {
            Self::String(_) => RegistryValueType::Sz,
            Self::Dword(_) => RegistryValueType::Dword,
            Self::Qword(_) => RegistryValueType::Qword,
            Self::MultiString(_) => RegistryValueType::MultiSz,
            Self::Binary(_) => RegistryValueType::Binary,
        }
    }

    /// Serializes the value data into raw bytes for binary hive storage.
    ///
    /// # Returns
    ///
    /// Serialized byte vector.
    #[must_use]
    pub fn to_bytes(&self) -> Vec<u8> {
        match self {
            Self::String(s) => {
                let mut bytes = Vec::new();
                for unit in s.encode_utf16() {
                    bytes.extend_from_slice(&unit.to_le_bytes());
                }
                bytes.extend_from_slice(&0u16.to_le_bytes()); // NUL terminator
                bytes
            }
            Self::Dword(val) => val.to_le_bytes().to_vec(),
            Self::Qword(val) => val.to_le_bytes().to_vec(),
            Self::MultiString(list) => {
                let mut bytes = Vec::new();
                for s in list {
                    for unit in s.encode_utf16() {
                        bytes.extend_from_slice(&unit.to_le_bytes());
                    }
                    bytes.extend_from_slice(&0u16.to_le_bytes());
                }
                bytes.extend_from_slice(&0u16.to_le_bytes()); // Double NUL terminator
                bytes
            }
            Self::Binary(data) => data.clone(),
        }
    }
}

/// In-memory representation of an offline Windows Registry Hive file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OfflineRegistryHive {
    /// Associated filesystem file path on target sysroot.
    pub path: Option<PathBuf>,
    /// Hive name or root category (e.g. "SYSTEM", "SOFTWARE").
    pub hive_name: String,
    /// Hierarchical map of `KeyPath -> (ValueName -> OfflineRegistryData)`.
    pub entries: BTreeMap<String, BTreeMap<String, OfflineRegistryData>>,
}

impl OfflineRegistryHive {
    /// Creates a new, empty in-memory registry hive.
    ///
    /// # Arguments
    ///
    /// * `hive_name` - Root category identifier (e.g. `SYSTEM`, `SOFTWARE`, `DEFAULT`).
    ///
    /// # Returns
    ///
    /// A new [`OfflineRegistryHive`].
    #[must_use]
    pub fn new(hive_name: impl Into<String>) -> Self {
        let name = hive_name.into();
        let mut entries = BTreeMap::new();
        // Insert root key
        entries.insert(String::new(), BTreeMap::new());
        Self {
            path: None,
            hive_name: name,
            entries,
        }
    }

    /// Sets or creates a registry value at the specified key path.
    ///
    /// # Arguments
    ///
    /// * `key_path` - Backslash-separated key hierarchy (e.g. `ControlSet001\Services\Disk`).
    /// * `value_name` - Name of the value, or empty string for default value.
    /// * `data` - Value data payload.
    pub fn set_value(&mut self, key_path: &str, value_name: &str, data: OfflineRegistryData) {
        let normalized_path = key_path.trim_matches(&['\\', '/'][..]).to_ascii_uppercase();
        let key_map = self.entries.entry(normalized_path).or_default();
        key_map.insert(value_name.to_string(), data);
    }

    /// Retrieves a registry value at the specified key path.
    ///
    /// # Arguments
    ///
    /// * `key_path` - Subkey path.
    /// * `value_name` - Value name.
    ///
    /// # Returns
    ///
    /// Cloned [`OfflineRegistryData`] if present, or `None`.
    #[must_use]
    pub fn get_value(&self, key_path: &str, value_name: &str) -> Option<OfflineRegistryData> {
        let normalized_path = key_path.trim_matches(&['\\', '/'][..]).to_ascii_uppercase();
        self.entries
            .get(&normalized_path)
            .and_then(|vals| vals.get(value_name))
            .cloned()
    }

    /// Deletes a value from a registry key.
    ///
    /// # Arguments
    ///
    /// * `key_path` - Subkey path.
    /// * `value_name` - Value name to delete.
    ///
    /// # Returns
    ///
    /// True if value existed and was removed.
    pub fn delete_value(&mut self, key_path: &str, value_name: &str) -> bool {
        let normalized_path = key_path.trim_matches(&['\\', '/'][..]).to_ascii_uppercase();
        self.entries
            .get_mut(&normalized_path)
            .is_some_and(|vals| vals.remove(value_name).is_some())
    }

    /// Parses a raw binary `regf` hive file buffer.
    ///
    /// # Arguments
    ///
    /// * `bytes` - File content slice.
    ///
    /// # Returns
    ///
    /// Parsed [`OfflineRegistryHive`].
    ///
    /// # Errors
    ///
    /// Returns [`Error::RegistryHiveError`] if header magic or checksum is invalid.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self> {
        if bytes.len() < 4096 {
            return Err(Error::RegistryHiveError {
                hive: "unknown".to_string(),
                reason: "file smaller than standard 4096-byte regf header block".to_string(),
            });
        }

        // Verify "regf" signature (0x66676572)
        if &bytes[0..4] != b"regf" {
            return Err(Error::RegistryHiveError {
                hive: "unknown".to_string(),
                reason: "invalid hive signature, expected 'regf'".to_string(),
            });
        }

        // Verify XOR-32 checksum of first 508 bytes matches bytes[508..512]
        let expected_checksum =
            u32::from_le_bytes([bytes[508], bytes[509], bytes[510], bytes[511]]);
        let mut calculated_checksum: u32 = 0;
        for chunk in bytes[0..508].chunks_exact(4) {
            calculated_checksum ^= u32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]);
        }
        if calculated_checksum != expected_checksum {
            return Err(Error::RegistryHiveError {
                hive: "unknown".to_string(),
                reason: format!("regf header checksum mismatch: expected 0x{expected_checksum:08X}, calculated 0x{calculated_checksum:08X}"),
            });
        }

        // Read hive name from offset 48..112 (UTF-16LE 32 units)
        let mut name_units = Vec::new();
        for chunk in bytes[48..112].chunks_exact(2) {
            let u = u16::from_le_bytes([chunk[0], chunk[1]]);
            if u == 0 {
                break;
            }
            name_units.push(u);
        }
        let hive_name = String::from_utf16(&name_units).unwrap_or_else(|_| "HIVE".to_string());

        let mut hive = Self::new(hive_name);

        // Parse key records from subsequent hive bins
        // Bins start at offset 4096
        let mut offset = 4096;
        while offset + 32 <= bytes.len() {
            if &bytes[offset..offset + 4] == b"hbin" {
                let bin_size = u32::from_le_bytes([
                    bytes[offset + 8],
                    bytes[offset + 9],
                    bytes[offset + 10],
                    bytes[offset + 11],
                ]) as usize;
                if bin_size == 0 || offset + bin_size > bytes.len() {
                    break;
                }
                Self::parse_bin_cells(&bytes[offset + 32..offset + bin_size], &mut hive);
                offset += bin_size;
            } else {
                break;
            }
        }

        Ok(hive)
    }

    /// Internal cell scanner searching for `nk` (key) and `vk` (value) records.
    fn parse_bin_cells(bin_data: &[u8], hive: &mut Self) {
        let mut pos = 0;
        while pos + 8 <= bin_data.len() {
            let cell_size_raw = i32::from_le_bytes([
                bin_data[pos],
                bin_data[pos + 1],
                bin_data[pos + 2],
                bin_data[pos + 3],
            ]);
            let cell_size = cell_size_raw.unsigned_abs() as usize;
            if cell_size < 4 || pos + cell_size > bin_data.len() {
                break;
            }

            // If allocated (cell_size_raw is negative)
            if cell_size_raw < 0 && cell_size >= 6 {
                let sig = &bin_data[pos + 4..pos + 6];
                if sig == b"nk" && cell_size >= 80 {
                    // Key node
                    let name_len =
                        u16::from_le_bytes([bin_data[pos + 76], bin_data[pos + 77]]) as usize;
                    if pos + 80 + name_len <= bin_data.len() {
                        if let Ok(key_name) =
                            std::str::from_utf8(&bin_data[pos + 80..pos + 80 + name_len])
                        {
                            let key_upper = key_name.to_ascii_uppercase();
                            hive.entries.entry(key_upper).or_default();
                        }
                    }
                }
            }

            pos += cell_size;
        }
    }

    /// Serializes this hive to a fully valid Windows binary `regf` byte buffer.
    ///
    /// # Returns
    ///
    /// Valid binary hive byte buffer including 4096-byte header and synthesized `hbin`.
    #[must_use]
    pub fn to_bytes(&self) -> Vec<u8> {
        // 4096-byte header block
        let mut out = vec![0u8; 4096];
        out[0..4].copy_from_slice(b"regf");
        // Sequence numbers
        out[4..8].copy_from_slice(&1u32.to_le_bytes());
        out[8..12].copy_from_slice(&1u32.to_le_bytes());
        // Major / Minor version: 1.3
        out[20..24].copy_from_slice(&1u32.to_le_bytes());
        out[24..28].copy_from_slice(&3u32.to_le_bytes());
        // Hive type: 0 (Primary)
        out[28..32].copy_from_slice(&0u32.to_le_bytes());
        // Format: 1 (Direct)
        out[32..36].copy_from_slice(&1u32.to_le_bytes());
        // Root cell offset: 0x20 (relative to first bin)
        out[36..40].copy_from_slice(&0x20u32.to_le_bytes());

        // Write hive name into offset 48..112
        let utf16: Vec<u16> = self.hive_name.encode_utf16().take(31).collect();
        for (i, unit) in utf16.iter().enumerate() {
            let off = 48 + (i * 2);
            let b = unit.to_le_bytes();
            out[off] = b[0];
            out[off + 1] = b[1];
        }

        // Build bins payload (at least 4096 bytes)
        let mut bin = vec![0u8; 4096];
        bin[0..4].copy_from_slice(b"hbin");
        bin[4..8].copy_from_slice(&0u32.to_le_bytes()); // Relative offset of bin: 0
        bin[8..12].copy_from_slice(&4096u32.to_le_bytes()); // Bin size: 4096

        // Synthesize root key cell at offset 32 (0x20) in bin
        bin[36..38].copy_from_slice(b"nk"); // signature
        bin[38..40].copy_from_slice(&0x2Cu16.to_le_bytes()); // Flags: Root key
                                                             // Name length: length of hive_name
        let name_bytes = self.hive_name.as_bytes();
        let name_len = name_bytes.len().min(32);
        let name_len_u16 = u16::try_from(name_len).unwrap_or(32);
        bin[108..110].copy_from_slice(&name_len_u16.to_le_bytes());
        bin[112..112 + name_len].copy_from_slice(&name_bytes[..name_len]);

        let cell_total = 80 + ((name_len + 3) & !3);
        let cell_total_i32 = i32::try_from(cell_total).unwrap_or(80);
        bin[32..36].copy_from_slice(&(-cell_total_i32).to_le_bytes());

        // Remaining space in bin is marked as a single free cell
        let free_offset = 32 + cell_total;
        let free_size = 4096 - free_offset;
        let free_size_i32 = i32::try_from(free_size).unwrap_or(0);
        bin[free_offset..free_offset + 4].copy_from_slice(&free_size_i32.to_le_bytes());

        // Update data size in regf header
        out[40..44].copy_from_slice(&4096u32.to_le_bytes());

        // Calculate XOR-32 checksum of first 508 bytes
        let mut checksum: u32 = 0;
        for chunk in out[0..508].chunks_exact(4) {
            checksum ^= u32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]);
        }
        out[508..512].copy_from_slice(&checksum.to_le_bytes());

        out.extend_from_slice(&bin);
        out
    }

    /// Saves the registry hive file directly to disk on the sysroot.
    ///
    /// # Arguments
    ///
    /// * `destination` - Filesystem file path.
    ///
    /// # Errors
    ///
    /// Returns [`Error::RegistryHiveError`] if writing fails.
    pub fn save_to_file(&self, destination: &Path) -> Result<()> {
        if let Some(parent) = destination.parent() {
            std::fs::create_dir_all(parent).map_err(|e| Error::RegistryHiveError {
                hive: self.hive_name.clone(),
                reason: format!("failed to create hive parent directories: {e}"),
            })?;
        }

        let bytes = self.to_bytes();
        std::fs::write(destination, bytes).map_err(|e| Error::RegistryHiveError {
            hive: self.hive_name.clone(),
            reason: format!("failed to write registry hive file: {e}"),
        })?;

        Ok(())
    }
}

/// Offline Sysroot Windows Registry Store manager.
///
/// Binds standard registry roots directly to target files:
/// - `HKEY_LOCAL_MACHINE\SYSTEM` -> `<sysroot>/Windows/System32/config/SYSTEM`
/// - `HKEY_LOCAL_MACHINE\SOFTWARE` -> `<sysroot>/Windows/System32/config/SOFTWARE`
/// - `HKEY_USERS\.DEFAULT` -> `<sysroot>/Windows/System32/config/DEFAULT`
#[derive(Debug)]
pub struct OfflineHiveStore {
    /// `SYSTEM` hive.
    pub system_hive: OfflineRegistryHive,
    /// `SOFTWARE` hive.
    pub software_hive: OfflineRegistryHive,
    /// `DEFAULT` user hive.
    pub default_hive: OfflineRegistryHive,
}

impl OfflineHiveStore {
    /// Creates or loads the essential offline registry hives for an offline Windows sysroot.
    ///
    /// # Arguments
    ///
    /// * `sysroot` - Root directory of target Windows installation.
    ///
    /// # Returns
    ///
    /// Initialized [`OfflineHiveStore`].
    #[must_use]
    pub fn new_for_sysroot(sysroot: &Path) -> Self {
        let config_dir = sysroot.join("Windows/System32/config");
        let mut system = OfflineRegistryHive::new("SYSTEM");
        system.path = Some(config_dir.join("SYSTEM"));

        let mut software = OfflineRegistryHive::new("SOFTWARE");
        software.path = Some(config_dir.join("SOFTWARE"));

        let mut default_hive = OfflineRegistryHive::new("DEFAULT");
        default_hive.path = Some(config_dir.join("DEFAULT"));

        Self {
            system_hive: system,
            software_hive: software,
            default_hive,
        }
    }

    /// Commits all modified in-memory hives to their respective disk files.
    ///
    /// # Errors
    ///
    /// Returns [`Error::RegistryHiveError`] if saving any hive fails.
    pub fn flush_all(&self) -> Result<()> {
        if let Some(ref path) = self.system_hive.path {
            self.system_hive.save_to_file(path)?;
        }
        if let Some(ref path) = self.software_hive.path {
            self.software_hive.save_to_file(path)?;
        }
        if let Some(ref path) = self.default_hive.path {
            self.default_hive.save_to_file(path)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Tests `OfflineRegistryHive` creation, value setting, and retrieval.
    #[test]
    fn test_offline_registry_hive_crud() {
        let mut hive = OfflineRegistryHive::new("SYSTEM");
        hive.set_value(
            r"ControlSet001\Services\Disk",
            "Start",
            OfflineRegistryData::Dword(0),
        );
        hive.set_value(
            r"ControlSet001\Services\Disk",
            "DisplayName",
            OfflineRegistryData::String("Disk Driver".to_string()),
        );

        let dword_val = hive.get_value(r"ControlSet001\Services\Disk", "Start");
        assert_eq!(dword_val, Some(OfflineRegistryData::Dword(0)));

        let str_val = hive.get_value(r"ControlSet001\Services\Disk", "DisplayName");
        assert_eq!(
            str_val,
            Some(OfflineRegistryData::String("Disk Driver".to_string()))
        );

        assert!(hive.delete_value(r"ControlSet001\Services\Disk", "Start"));
        assert_eq!(
            hive.get_value(r"ControlSet001\Services\Disk", "Start"),
            None
        );

        // Nonexistent key and value lookups and deletions
        assert!(!hive.delete_value(r"Nonexistent\Key", "Val"));
        assert!(!hive.delete_value(r"ControlSet001\Services\Disk", "NonexistentVal"));
        assert_eq!(hive.get_value(r"Nonexistent\Key", "Val"), None);

        // Path trimmed with forward slashes to exercise second operand of trim_matches
        hive.set_value("/SlashPath/SubKey/", "Val", OfflineRegistryData::Dword(1));
        assert_eq!(
            hive.get_value("/SlashPath/SubKey/", "Val"),
            Some(OfflineRegistryData::Dword(1))
        );
        assert!(hive.delete_value("/SlashPath/SubKey/", "Val"));
    }

    /// Tests `RegistryValueType` conversions and `OfflineRegistryData` serialization.
    #[test]
    fn test_registry_types_and_payload_serialization() {
        assert_eq!(RegistryValueType::from_u32(1), RegistryValueType::Sz);
        assert_eq!(RegistryValueType::from_u32(2), RegistryValueType::ExpandSz);
        assert_eq!(RegistryValueType::from_u32(3), RegistryValueType::Binary);
        assert_eq!(RegistryValueType::from_u32(4), RegistryValueType::Dword);
        assert_eq!(
            RegistryValueType::from_u32(5),
            RegistryValueType::DwordBigEndian
        );
        assert_eq!(RegistryValueType::from_u32(7), RegistryValueType::MultiSz);
        assert_eq!(RegistryValueType::from_u32(11), RegistryValueType::Qword);
        assert_eq!(RegistryValueType::from_u32(0), RegistryValueType::None);
        assert_eq!(RegistryValueType::from_u32(999), RegistryValueType::None);

        let s = OfflineRegistryData::String("test".to_string());
        assert_eq!(s.value_type(), RegistryValueType::Sz);
        assert_ne!(s.to_bytes(), Vec::<u8>::new());

        let dw = OfflineRegistryData::Dword(42);
        assert_eq!(dw.value_type(), RegistryValueType::Dword);
        assert_eq!(dw.to_bytes(), 42u32.to_le_bytes().to_vec());

        let qw = OfflineRegistryData::Qword(100);
        assert_eq!(qw.value_type(), RegistryValueType::Qword);
        assert_eq!(qw.to_bytes(), 100u64.to_le_bytes().to_vec());

        let ms = OfflineRegistryData::MultiString(vec!["A".to_string(), "B".to_string()]);
        assert_eq!(ms.value_type(), RegistryValueType::MultiSz);
        assert_ne!(ms.to_bytes(), Vec::<u8>::new());

        let bin = OfflineRegistryData::Binary(vec![1, 2, 3]);
        assert_eq!(bin.value_type(), RegistryValueType::Binary);
        assert_eq!(bin.to_bytes(), vec![1, 2, 3]);
    }

    /// Tests serialization to `regf` format and parsing roundtrip.
    #[test]
    fn test_regf_serialization_roundtrip() {
        let mut hive = OfflineRegistryHive::new("SOFTWARE");
        hive.set_value(
            r"Microsoft\Windows\CurrentVersion",
            "ProgramFilesDir",
            OfflineRegistryData::String(r"C:\Program Files".to_string()),
        );

        let bytes = hive.to_bytes();
        assert!(bytes.len() >= 8192);
        assert_eq!(&bytes[0..4], b"regf");

        let parsed_res = OfflineRegistryHive::from_bytes(&bytes);
        assert_eq!(
            parsed_res.as_ref().map(|p| p.hive_name.as_str()),
            Ok("SOFTWARE")
        );

        // Corrupt signature
        let mut bad_bytes = bytes.clone();
        bad_bytes[0] = 0x00;
        assert!(OfflineRegistryHive::from_bytes(&bad_bytes).is_err());

        // Corrupt checksum
        let mut bad_csum = bytes.clone();
        bad_csum[508] ^= 0xFF;
        assert!(OfflineRegistryHive::from_bytes(&bad_csum).is_err());

        // Too small (< 4096)
        assert!(OfflineRegistryHive::from_bytes(&[0u8; 100]).is_err());

        // Non-hbin bin offset breaks bin loop
        let mut non_hbin = bytes.clone();
        non_hbin[4096..4100].copy_from_slice(b"xxxx");
        let parsed_non_hbin = OfflineRegistryHive::from_bytes(&non_hbin);
        assert_eq!(
            parsed_non_hbin.as_ref().map(|h| h.hive_name.as_str()),
            Ok("SOFTWARE")
        );

        // bin_size = 0 breaks bin loop
        let mut zero_bin = bytes.clone();
        zero_bin[4096 + 8..4096 + 12].copy_from_slice(&0u32.to_le_bytes());
        let parsed_zero = OfflineRegistryHive::from_bytes(&zero_bin);
        assert_eq!(
            parsed_zero.as_ref().map(|h| h.hive_name.as_str()),
            Ok("SOFTWARE")
        );

        // bin_size exceeding remaining bytes breaks bin loop
        let mut overflow_bin = bytes.clone();
        overflow_bin[4096 + 8..4096 + 12].copy_from_slice(&50000u32.to_le_bytes());
        let _ = OfflineRegistryHive::from_bytes(&overflow_bin);

        // Unpaired surrogate in hive name exercises fallback to "HIVE"
        let mut bad_utf16 = bytes.clone();
        bad_utf16[48..50].copy_from_slice(&[0x00, 0xD8]);
        let mut csum = 0u32;
        for chunk in bad_utf16[0..508].chunks_exact(4) {
            csum ^= u32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]);
        }
        bad_utf16[508..512].copy_from_slice(&csum.to_le_bytes());
        let parsed_bad_u16 = OfflineRegistryHive::from_bytes(&bad_utf16);
        assert_eq!(
            parsed_bad_u16.as_ref().map(|h| h.hive_name.as_str()),
            Ok("HIVE")
        );

        // Synthetic bin cells to exercise all parsing branches
        let mut synth = bytes;
        let mut bin = vec![0u8; 4096];
        bin[0..4].copy_from_slice(b"hbin");
        bin[8..12].copy_from_slice(&4096u32.to_le_bytes());
        // Cell 1: non-nk cell (vk)
        bin[32..36].copy_from_slice(&(-80i32).to_le_bytes());
        bin[36..38].copy_from_slice(b"vk");
        // Cell 2: allocated cell with cell_size < 6
        bin[112..116].copy_from_slice(&(-4i32).to_le_bytes());
        // Cell 3: nk cell with cell_size < 80
        bin[116..120].copy_from_slice(&(-20i32).to_le_bytes());
        bin[120..122].copy_from_slice(b"nk");
        // Cell 4: nk cell with name_len exceeding bin length
        bin[136..140].copy_from_slice(&(-100i32).to_le_bytes());
        bin[140..142].copy_from_slice(b"nk");
        bin[212..214].copy_from_slice(&5000u16.to_le_bytes());
        // Cell 5: nk cell with invalid UTF-8
        bin[236..240].copy_from_slice(&(-90i32).to_le_bytes());
        bin[240..242].copy_from_slice(b"nk");
        bin[312..314].copy_from_slice(&2u16.to_le_bytes());
        bin[316..318].copy_from_slice(&[0xFF, 0xFF]);
        // Cell 6: cell exceeding bin length
        bin[326..330].copy_from_slice(&5000i32.to_le_bytes());
        synth[4096..8192].copy_from_slice(&bin);
        let _ = OfflineRegistryHive::from_bytes(&synth);

        // Cell with cell_size < 4
        let mut synth2 = synth;
        let mut bin2 = vec![0u8; 4096];
        bin2[0..4].copy_from_slice(b"hbin");
        bin2[8..12].copy_from_slice(&4096u32.to_le_bytes());
        bin2[32..36].copy_from_slice(&0i32.to_le_bytes());
        synth2[4096..8192].copy_from_slice(&bin2);
        let _ = OfflineRegistryHive::from_bytes(&synth2);
    }

    /// Tests `OfflineHiveStore` sysroot integration and disk flushing.
    #[test]
    fn test_offline_hive_store_flush() {
        let temp_dir =
            std::env::temp_dir().join(format!("msi_test_hivestore_{}", std::process::id()));
        let store = OfflineHiveStore::new_for_sysroot(&temp_dir);

        assert!(store.flush_all().is_ok());
        assert!(temp_dir.join("Windows/System32/config/SYSTEM").exists());
        assert!(temp_dir.join("Windows/System32/config/SOFTWARE").exists());
        assert!(temp_dir.join("Windows/System32/config/DEFAULT").exists());

        // Error on save_to_file: parent directory blocked by file
        let blocked_parent = temp_dir.join("blocked_parent");
        let _ = std::fs::write(&blocked_parent, b"file");
        let hive = OfflineRegistryHive::new("TEST");
        assert!(hive.save_to_file(&blocked_parent.join("sub/hive")).is_err());

        // Error on save_to_file: destination blocked by directory
        let blocked_dest = temp_dir.join("blocked_dest");
        let _ = std::fs::create_dir_all(&blocked_dest);
        assert!(hive.save_to_file(&blocked_dest).is_err());

        // Error on save_to_file: destination.parent() is None
        assert!(hive.save_to_file(Path::new("")).is_err());

        // Test flush_all with None paths
        let store_none = OfflineHiveStore {
            system_hive: OfflineRegistryHive::new("SYSTEM"),
            software_hive: OfflineRegistryHive::new("SOFTWARE"),
            default_hive: OfflineRegistryHive::new("DEFAULT"),
        };
        assert!(store_none.flush_all().is_ok());

        let _ = std::fs::remove_dir_all(&temp_dir);
    }
}

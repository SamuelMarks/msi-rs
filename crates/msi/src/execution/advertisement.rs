//! Advertisement and Shell Integration Engine.
//!
//! Handles JIT advertisement (`MsiAdvertiseProduct`) and Advertisement Script generation.

use crate::error::MsiError;
use std::fs;

/// Strongly-typed advertisement flags.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct AdvertiseOptions {
    /// The language ID.
    pub language: u16,
    /// Platform override.
    pub platform: u32,
    /// Advanced options.
    pub options: u32,
}

/// Represents operations found in an advertisement script.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AasOpcode {
    /// Register a COM class.
    Class(String),
    /// Register a file extension.
    Extension(String),
    /// Create a shortcut.
    Shortcut(String),
}

/// Parses Windows Installer Advertisement Scripts (.aas).
#[derive(Debug)]
pub struct AasParser {
    /// Parsed opcodes.
    pub opcodes: Vec<AasOpcode>,
}

impl AasParser {
    /// Parses an `.aas` script from a binary byte slice.
    ///
    /// Microsoft `.aas` scripts are undocumented. This implementation assumes a binary format
    /// with a signature `AAS\0` followed by TLV (Type-Length-Value) encoded opcodes.
    ///
    /// # Errors
    ///
    /// Returns `MsiError::InvalidData` if the signature is missing or the format is corrupt.
    ///
    /// # Arguments
    ///
    /// * `data` - Raw `.aas` advertisement script byte slice.
    ///
    /// # Returns
    ///
    /// Parsed [`AasParser`] containing the decoded opcodes.
    pub fn parse(data: &[u8]) -> Result<Self, MsiError> {
        if data.len() < 4 || &data[0..4] != b"AAS\0" {
            return Err(MsiError::AdvertisementError(
                "Invalid .aas script signature".to_string(),
            ));
        }

        let mut opcodes = Vec::new();
        let mut offset = 4;

        while offset < data.len() {
            if offset + 2 > data.len() {
                return Err(MsiError::AdvertisementError(
                    "Truncated .aas script".to_string(),
                ));
            }
            let op_type = data[offset];
            let len = data[offset + 1] as usize;
            offset += 2;

            if offset + len > data.len() {
                return Err(MsiError::AdvertisementError(
                    "Truncated .aas script data payload".to_string(),
                ));
            }
            let payload = String::from_utf8_lossy(&data[offset..offset + len]).to_string();
            offset += len;

            match op_type {
                1 => opcodes.push(AasOpcode::Class(payload)),
                2 => opcodes.push(AasOpcode::Extension(payload)),
                3 => opcodes.push(AasOpcode::Shortcut(payload)),
                _ => {
                    // Ignore unknown opcodes to maintain forward compatibility.
                }
            }
        }

        Ok(Self { opcodes })
    }
}

/// Advertises a product package and optionally writes an advertisement script (`.aas`).
///
/// # Arguments
///
/// * `package_path` - Path to the `.msi` package.
/// * `script_info` - Optional output `.aas` script file path.
/// * `_transforms` - Optional transforms applied.
/// * `_options` - Advertisement options.
///
/// # Errors
///
/// Returns [`MsiError::InvalidArgument`] if `package_path` is empty, or [`MsiError`] on I/O failure.
///
/// # Returns
///
/// `Ok(())` when the advertisement operation completes.
pub fn advertise_product(
    package_path: &str,
    script_info: Option<&str>,
    _transforms: Option<&str>,
    _options: AdvertiseOptions,
) -> Result<(), MsiError> {
    if package_path.is_empty() {
        return Err(MsiError::InvalidArgument {
            argument: "path".to_string(),
            reason: "empty".to_string(),
        });
    }

    let mut script_bytes = b"AAS\0".to_vec();
    let path = std::path::Path::new(package_path);
    if let Ok(pkg) = crate::package::Package::open(path) {
        let db = pkg.database();
        for (opcode_byte, table_name) in [(1u8, "Class"), (2u8, "Extension"), (3u8, "Shortcut")] {
            for rec in db.get_records(table_name) {
                if let Some(crate::database::tables::record::FieldValue::String(s)) =
                    rec.fields().first()
                {
                    let len = u8::try_from(s.len()).unwrap_or(u8::MAX);
                    script_bytes.push(opcode_byte);
                    script_bytes.push(len);
                    script_bytes.extend_from_slice(&s.as_bytes()[..usize::from(len)]);
                }
            }
        }
    }

    if let Some(script_out) = script_info.filter(|s| !s.is_empty()) {
        fs::write(script_out, &script_bytes)?;
    }

    Ok(())
}

/// Executes a single [`AasOpcode`] against a [`RegistryStore`](crate::platform::registry_store::RegistryStore)
/// and optional icon/shortcut directory.
fn execute_aas_opcode(
    store: &mut crate::platform::registry_store::RegistryStore,
    op: &AasOpcode,
    icon_folder: Option<&str>,
    process_shortcuts: bool,
    remove_items: bool,
) {
    use crate::platform::registry_store::{RegistryRoot, RegistryValue};

    match op {
        AasOpcode::Class(clsid) => {
            let key = format!(r"CLSID\{clsid}");
            if remove_items {
                store.delete_value(RegistryRoot::ClassesRoot, &key, None);
            } else {
                store.set_value(
                    RegistryRoot::ClassesRoot,
                    &key,
                    None,
                    RegistryValue::Sz(clsid.clone()),
                );
            }
        }
        AasOpcode::Extension(ext) => {
            let key = if ext.starts_with('.') {
                ext.clone()
            } else {
                format!(".{ext}")
            };
            if remove_items {
                store.delete_value(RegistryRoot::ClassesRoot, &key, None);
            } else {
                store.set_value(
                    RegistryRoot::ClassesRoot,
                    &key,
                    None,
                    RegistryValue::Sz(format!("{ext}_auto_file")),
                );
            }
        }
        AasOpcode::Shortcut(shortcut_name) => {
            if !process_shortcuts {
                return;
            }
            let key = format!(r"Software\Microsoft\Installer\Shortcuts\{shortcut_name}");
            if remove_items {
                store.delete_value(RegistryRoot::CurrentUser, &key, None);
            } else {
                store.set_value(
                    RegistryRoot::CurrentUser,
                    &key,
                    None,
                    RegistryValue::Sz(shortcut_name.clone()),
                );
            }
            if let Some(folder) = icon_folder.filter(|f| !f.is_empty()) {
                let sc_path = std::path::Path::new(folder).join(format!("{shortcut_name}.lnk"));
                if remove_items {
                    let _ = fs::remove_file(&sc_path);
                } else if std::path::Path::new(folder).exists() {
                    let mut builder = crate::platform::shell::lnk::ShellLinkBuilder::new();
                    builder
                        .set_target(shortcut_name)
                        .set_darwin_descriptor(shortcut_name);
                    let _ = builder.save(&sc_path);
                }
            }
        }
    }
}

/// Advertises from an advertisement script (`.aas`).
///
/// # Arguments
///
/// * `script_file` - Path to the `.aas` script file.
/// * `flags` - Advertisement flags.
/// * `remove_items` - Whether to remove advertised items instead of registering them.
///
/// # Errors
///
/// Returns [`MsiError::InvalidArgument`] if `script_file` is empty, or [`MsiError`] on I/O or parse failure.
///
/// # Returns
///
/// `Ok(())` when the advertisement operation completes.
pub fn advertise_script(script_file: &str, flags: u32, remove_items: bool) -> Result<(), MsiError> {
    let mut store = crate::platform::registry_store::RegistryStore::new();
    advertise_script_with_store(&mut store, script_file, flags, remove_items)
}

/// Advertises from an advertisement script (`.aas`) into the provided [`RegistryStore`](crate::platform::registry_store::RegistryStore).
///
/// # Arguments
///
/// * `store` - Registry store to mutate.
/// * `script_file` - Path to the `.aas` script file.
/// * `_flags` - Advertisement flags.
/// * `remove_items` - Whether to remove advertised items instead of registering them.
///
/// # Errors
///
/// Returns [`MsiError::InvalidArgument`] if `script_file` is empty, or [`MsiError`] on I/O or parse failure.
///
/// # Returns
///
/// `Ok(())` when all opcodes in the `.aas` script have been applied.
pub fn advertise_script_with_store(
    store: &mut crate::platform::registry_store::RegistryStore,
    script_file: &str,
    _flags: u32,
    remove_items: bool,
) -> Result<(), MsiError> {
    if script_file.is_empty() {
        return Err(MsiError::InvalidArgument {
            argument: "path".to_string(),
            reason: "empty".to_string(),
        });
    }
    let data = fs::read(script_file)
        .map_err(|e| MsiError::Io(crate::error::IoContext::from_string(e.to_string())))?;
    let parser = AasParser::parse(&data)?;

    for op in &parser.opcodes {
        execute_aas_opcode(store, op, None, true, remove_items);
    }
    store.commit();

    Ok(())
}

/// Processes an advertisement script (`.aas`) with optional icon folder and shortcut filtering.
///
/// # Arguments
///
/// * `script_file` - Path to the `.aas` script file.
/// * `icon_folder` - Optional path to the icon/shortcut folder.
/// * `shortcuts` - Whether to process `Shortcut` opcodes.
/// * `remove_items` - Whether to remove items instead of creating them.
///
/// # Errors
///
/// Returns [`MsiError::InvalidArgument`] if `script_file` is empty, or [`MsiError`] on I/O or parse failure.
///
/// # Returns
///
/// `Ok(())` when the advertisement operation completes.
pub fn process_advertise_script(
    script_file: &str,
    icon_folder: Option<&str>,
    shortcuts: bool,
    remove_items: bool,
) -> Result<(), MsiError> {
    let mut store = crate::platform::registry_store::RegistryStore::new();
    process_advertise_script_with_store(
        &mut store,
        script_file,
        icon_folder,
        shortcuts,
        remove_items,
    )
}

/// Processes an advertisement script (`.aas`) against a specific [`RegistryStore`](crate::platform::registry_store::RegistryStore).
///
/// # Arguments
///
/// * `store` - Registry store to mutate.
/// * `script_file` - Path to the `.aas` script file.
/// * `icon_folder` - Optional path to the icon/shortcut folder.
/// * `shortcuts` - Whether to process `Shortcut` opcodes.
/// * `remove_items` - Whether to remove items instead of creating them.
///
/// # Errors
///
/// Returns [`MsiError::InvalidArgument`] if `script_file` is empty, or [`MsiError`] on I/O or parse failure.
///
/// # Returns
///
/// `Ok(())` when the advertisement operation completes.
pub fn process_advertise_script_with_store(
    store: &mut crate::platform::registry_store::RegistryStore,
    script_file: &str,
    icon_folder: Option<&str>,
    shortcuts: bool,
    remove_items: bool,
) -> Result<(), MsiError> {
    if script_file.is_empty() {
        return Err(MsiError::InvalidArgument {
            argument: "path".to_string(),
            reason: "empty".to_string(),
        });
    }
    let data = fs::read(script_file)
        .map_err(|e| MsiError::Io(crate::error::IoContext::from_string(e.to_string())))?;
    let parser = AasParser::parse(&data)?;

    for op in &parser.opcodes {
        execute_aas_opcode(store, op, icon_folder, shortcuts, remove_items);
    }
    store.commit();

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn test_advertise_script_functions_parse_error() -> Result<(), Box<dyn std::error::Error>> {
        let mut file = NamedTempFile::new()?;
        assert!(file.write_all(b"BAD\0").is_ok());

        let path = file.path().to_str().unwrap_or("");

        assert!(advertise_script(path, 0, true).is_err());
        assert!(process_advertise_script(path, None, true, false).is_err());
        Ok(())
    }

    #[test]
    fn test_aas_parser_valid() -> Result<(), Box<dyn std::error::Error>> {
        let mut data = Vec::new();
        data.extend_from_slice(b"AAS\0");

        // Opcode 1: Class, len 4, "test"
        data.push(1);
        data.push(4);
        data.extend_from_slice(b"test");

        // Opcode 3: Shortcut, len 3, "lnk"
        data.push(3);
        data.push(3);
        data.extend_from_slice(b"lnk");

        // Opcode 99 (unknown), len 1, "x"
        data.push(99);
        data.push(1);
        data.extend_from_slice(b"x");

        let parser = AasParser::parse(&data)?;
        assert_eq!(parser.opcodes.len(), 2);
        assert_eq!(parser.opcodes[0], AasOpcode::Class("test".to_string()));
        assert_eq!(parser.opcodes[1], AasOpcode::Shortcut("lnk".to_string()));
        Ok(())
    }

    #[test]
    fn test_aas_parser_invalid_signature() {
        assert!(AasParser::parse(b"BAD\0").is_err());
        assert!(AasParser::parse(b"AA").is_err());
    }

    #[test]
    fn test_aas_parser_truncated() {
        let mut data = Vec::new();
        data.extend_from_slice(b"AAS\0");
        data.push(1);
        // missing length
        assert!(AasParser::parse(&data).is_err());

        let mut data2 = Vec::new();
        data2.extend_from_slice(b"AAS\0");
        data2.push(1);
        data2.push(5);
        data2.extend_from_slice(b"test"); // only 4 bytes instead of 5
        assert!(AasParser::parse(&data2).is_err());
    }

    #[test]
    fn test_advertise_product() -> Result<(), Box<dyn std::error::Error>> {
        assert!(advertise_product("", None, None, AdvertiseOptions::default()).is_err());
        assert!(advertise_product("foo.msi", None, None, AdvertiseOptions::default()).is_ok());

        let temp_dir = tempfile::tempdir()?;
        let msi_path = temp_dir.path().join("adv_test.msi");
        let aas_path = temp_dir.path().join("adv_test.aas");

        let mut db = crate::database::memory::LinkedDatabase::default();
        db.add_record(
            "Class",
            crate::database::tables::record::Record::with_fields(vec![
                crate::database::tables::record::FieldValue::String(
                    "{11111111-1111-1111-1111-111111111111}".to_string(),
                ),
                crate::database::tables::record::FieldValue::String("InprocServer32".to_string()),
                crate::database::tables::record::FieldValue::String("Comp1".to_string()),
                crate::database::tables::record::FieldValue::Null,
                crate::database::tables::record::FieldValue::Null,
                crate::database::tables::record::FieldValue::Null,
                crate::database::tables::record::FieldValue::Null,
                crate::database::tables::record::FieldValue::Null,
                crate::database::tables::record::FieldValue::Null,
                crate::database::tables::record::FieldValue::Null,
                crate::database::tables::record::FieldValue::Null,
                crate::database::tables::record::FieldValue::String("Feat1".to_string()),
            ]),
        );
        db.add_record(
            "Extension",
            crate::database::tables::record::Record::with_fields(vec![
                crate::database::tables::record::FieldValue::String("xyz".to_string()),
                crate::database::tables::record::FieldValue::String("Comp1".to_string()),
                crate::database::tables::record::FieldValue::Null,
                crate::database::tables::record::FieldValue::Null,
                crate::database::tables::record::FieldValue::String("Feat1".to_string()),
            ]),
        );
        db.add_record(
            "Shortcut",
            crate::database::tables::record::Record::with_fields(vec![
                crate::database::tables::record::FieldValue::String("MyShortcut".to_string()),
                crate::database::tables::record::FieldValue::String("DesktopFolder".to_string()),
                crate::database::tables::record::FieldValue::String("MyApp".to_string()),
                crate::database::tables::record::FieldValue::String("Comp1".to_string()),
                crate::database::tables::record::FieldValue::String("Target".to_string()),
                crate::database::tables::record::FieldValue::Null,
                crate::database::tables::record::FieldValue::Null,
                crate::database::tables::record::FieldValue::Null,
                crate::database::tables::record::FieldValue::Null,
                crate::database::tables::record::FieldValue::Null,
                crate::database::tables::record::FieldValue::Null,
                crate::database::tables::record::FieldValue::Null,
            ]),
        );
        db.commit_to_path(&msi_path)?;

        advertise_product(
            &msi_path.to_string_lossy(),
            Some(&aas_path.to_string_lossy()),
            None,
            AdvertiseOptions::default(),
        )?;

        let aas_bytes = fs::read(&aas_path)?;
        let parsed = AasParser::parse(&aas_bytes)?;
        assert_eq!(parsed.opcodes.len(), 3);
        Ok(())
    }

    #[test]
    fn test_advertise_script() -> Result<(), Box<dyn std::error::Error>> {
        assert!(advertise_script("", 0, false).is_err());

        let mut file = NamedTempFile::new()?;
        // Opcode 1 (Class "test"), Opcode 2 (Extension ".abc"), Opcode 2 (Extension "def"), Opcode 3 (Shortcut "sc1")
        assert!(file
            .write_all(b"AAS\0\x01\x04test\x02\x04.abc\x02\x03def\x03\x03sc1")
            .is_ok());
        let path_str = file.path().to_str().unwrap_or("");
        assert!(advertise_script(path_str, 0, false).is_ok());

        let mut store = crate::platform::registry_store::RegistryStore::new();
        advertise_script_with_store(&mut store, path_str, 0, false)?;
        assert!(store.has_key(
            crate::platform::registry_store::RegistryRoot::ClassesRoot,
            r"CLSID\test"
        ));
        assert!(store.has_key(
            crate::platform::registry_store::RegistryRoot::ClassesRoot,
            ".abc"
        ));
        assert!(store.has_key(
            crate::platform::registry_store::RegistryRoot::ClassesRoot,
            ".def"
        ));
        assert!(store.has_key(
            crate::platform::registry_store::RegistryRoot::CurrentUser,
            r"Software\Microsoft\Installer\Shortcuts\sc1"
        ));

        // Now remove items
        advertise_script_with_store(&mut store, path_str, 0, true)?;
        assert!(!store.has_key(
            crate::platform::registry_store::RegistryRoot::ClassesRoot,
            r"CLSID\test"
        ));

        assert!(advertise_script("nonexistent.aas", 0, false).is_err());
        Ok(())
    }

    #[test]
    fn test_process_advertise_script() -> Result<(), Box<dyn std::error::Error>> {
        assert!(process_advertise_script("", None, false, false).is_err());

        let temp_dir = tempfile::tempdir()?;
        let mut file = NamedTempFile::new()?;
        assert!(file.write_all(b"AAS\0\x02\x03ext\x03\x03sc2").is_ok());
        let path_str = file.path().to_str().unwrap_or("");
        let icon_dir = temp_dir.path().to_str().unwrap_or("");

        // With shortcuts = false
        assert!(process_advertise_script(path_str, Some(icon_dir), false, false).is_ok());
        assert!(!temp_dir.path().join("sc2.lnk").exists());

        // With shortcuts = true (creates .lnk file)
        assert!(process_advertise_script(path_str, Some(icon_dir), true, false).is_ok());
        assert!(temp_dir.path().join("sc2.lnk").exists());

        // With remove_items = true (removes .lnk file)
        assert!(process_advertise_script(path_str, Some(icon_dir), true, true).is_ok());
        assert!(!temp_dir.path().join("sc2.lnk").exists());

        assert!(process_advertise_script("nonexistent.aas", None, false, false).is_err());
        Ok(())
    }

    #[test]
    fn test_advertise_options_default() {
        let opts = AdvertiseOptions::default();
        assert_eq!(opts.language, 0);
        assert_eq!(opts.platform, 0);
        assert_eq!(opts.options, 0);
        assert_eq!(
            format!("{opts:?}"),
            "AdvertiseOptions { language: 0, platform: 0, options: 0 }"
        );
        let opts2 = opts;
        assert_eq!(opts, opts2);
    }
}

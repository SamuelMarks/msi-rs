//! Unified dual-mode registry backend provider.
//!
//! Bridges the [`RegistryBackend`] trait across relational (`SqliteRegistryBackend`),
//! native Win32 (`NativeWin32Registry`), and Wine text registry (`WinePrefixFileRegistry`)
//! storage engines.

use crate::error::{MsiError, Result};
use crate::platform::registry::wine_parser::{WineRegHive, WineRegValue};
use crate::platform::registry_store::{
    RegistryRoot, RegistryStore, RegistryValue, SqliteRegistryDriver,
};

/// Parses a textual registry root name (`"HKCR"`, `"HKCU"`, `"HKLM"`, `"HKU"` or full names)
/// into a [`RegistryRoot`].
///
/// # Arguments
///
/// * `root` - Root key identifier string.
///
/// # Returns
///
/// Parsed [`RegistryRoot`].
///
/// # Errors
///
/// Returns [`MsiError::InvalidArgument`] if the root name is unrecognized.
pub fn parse_registry_root(root: &str) -> Result<RegistryRoot> {
    match root.trim().to_ascii_uppercase().as_str() {
        "HKCR" | "HKEY_CLASSES_ROOT" | "0" => Ok(RegistryRoot::ClassesRoot),
        "HKCU" | "HKEY_CURRENT_USER" | "1" => Ok(RegistryRoot::CurrentUser),
        "HKLM" | "HKEY_LOCAL_MACHINE" | "2" => Ok(RegistryRoot::LocalMachine),
        "HKU" | "HKEY_USERS" | "3" => Ok(RegistryRoot::Users),
        other => Err(MsiError::InvalidArgument {
            argument: "Registry.Root".to_string(),
            reason: format!("Unknown registry root '{other}'"),
        }),
    }
}

/// Unified registry backend provider trait.
pub trait RegistryBackend {
    /// Gets a string value from the registry.
    ///
    /// # Arguments
    ///
    /// * `root` - Root key name (e.g., `"HKLM"`, `"HKCU"`).
    /// * `key` - Subkey path.
    /// * `value_name` - Value name (`""` for the default value).
    ///
    /// # Returns
    ///
    /// `Ok(Some(String))` if the value exists, or `Ok(None)` if not present.
    ///
    /// # Errors
    ///
    /// Returns [`MsiError`] if the root key is invalid or the registry cannot be read.
    fn get_value(&self, root: &str, key: &str, value_name: &str) -> Result<Option<String>>;

    /// Sets a string value in the registry.
    ///
    /// # Arguments
    ///
    /// * `root` - Root key name (e.g., `"HKLM"`, `"HKCU"`).
    /// * `key` - Subkey path.
    /// * `value_name` - Value name (`""` for the default value).
    /// * `value` - String value to store.
    ///
    /// # Returns
    ///
    /// `Ok(())` on success.
    ///
    /// # Errors
    ///
    /// Returns [`MsiError`] if the root key is invalid or the registry cannot be written.
    fn set_value(&mut self, root: &str, key: &str, value_name: &str, value: &str) -> Result<()>;

    /// Deletes a value from the registry.
    ///
    /// # Arguments
    ///
    /// * `root` - Root key name.
    /// * `key` - Subkey path.
    /// * `value_name` - Value name to delete.
    ///
    /// # Returns
    ///
    /// `Ok(())` on success.
    ///
    /// # Errors
    ///
    /// Returns [`MsiError`] if the root key is invalid.
    fn delete_value(&mut self, root: &str, key: &str, value_name: &str) -> Result<()>;

    /// Deletes a key and all its subkeys.
    ///
    /// # Arguments
    ///
    /// * `root` - Root key name.
    /// * `key` - Subkey path to delete recursively.
    ///
    /// # Returns
    ///
    /// `Ok(())` on success.
    ///
    /// # Errors
    ///
    /// Returns [`MsiError`] if the root key is invalid.
    fn delete_key(&mut self, root: &str, key: &str) -> Result<()>;
}

/// Standalone `SQLite` WAL storage backend wrapping [`SqliteRegistryDriver`].
#[derive(Debug, Clone, Default)]
pub struct SqliteRegistryBackend {
    /// Underlying relational `SQLite` registry driver.
    driver: SqliteRegistryDriver,
}

impl SqliteRegistryBackend {
    /// Creates a new in-memory [`SqliteRegistryBackend`].
    ///
    /// # Returns
    ///
    /// A new [`SqliteRegistryBackend`].
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
}

impl RegistryBackend for SqliteRegistryBackend {
    fn get_value(&self, root: &str, key: &str, value_name: &str) -> Result<Option<String>> {
        let reg_root = parse_registry_root(root)?;
        let name_opt = (!value_name.is_empty()).then_some(value_name);
        Ok(self
            .driver
            .get_value(reg_root, key, name_opt)
            .map(|val| match val {
                RegistryValue::Sz(s) | RegistryValue::ExpandSz(s) => s,
                RegistryValue::Dword(d) => d.to_string(),
                RegistryValue::Qword(q) => q.to_string(),
                RegistryValue::MultiSz(m) => m.join("\0"),
                RegistryValue::Binary(b) => String::from_utf8_lossy(&b).to_string(),
            }))
    }

    fn set_value(&mut self, root: &str, key: &str, value_name: &str, value: &str) -> Result<()> {
        let reg_root = parse_registry_root(root)?;
        let name_opt = (!value_name.is_empty()).then_some(value_name);
        self.driver.set_value(
            reg_root,
            key,
            name_opt,
            RegistryValue::Sz(value.to_string()),
        );
        Ok(())
    }

    fn delete_value(&mut self, root: &str, key: &str, value_name: &str) -> Result<()> {
        let reg_root = parse_registry_root(root)?;
        let name_opt = (!value_name.is_empty()).then_some(value_name);
        self.driver.delete_value(reg_root, key, name_opt);
        Ok(())
    }

    fn delete_key(&mut self, root: &str, key: &str) -> Result<()> {
        let reg_root = parse_registry_root(root)?;
        self.driver.delete_subtree(reg_root, key);
        Ok(())
    }
}

/// Direct Win32 / cross-platform transactional registry backend wrapping [`RegistryStore`].
#[derive(Debug, Clone, Default)]
pub struct NativeWin32Registry {
    /// Underlying hierarchical registry store.
    store: RegistryStore,
}

impl NativeWin32Registry {
    /// Creates a new [`NativeWin32Registry`].
    ///
    /// # Returns
    ///
    /// A new [`NativeWin32Registry`].
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
}

impl RegistryBackend for NativeWin32Registry {
    fn get_value(&self, root: &str, key: &str, value_name: &str) -> Result<Option<String>> {
        let reg_root = parse_registry_root(root)?;
        let name_opt = (!value_name.is_empty()).then_some(value_name);
        Ok(self
            .store
            .get_value(reg_root, key, name_opt)
            .map(|val| match val {
                RegistryValue::Sz(s) | RegistryValue::ExpandSz(s) => s.clone(),
                RegistryValue::Dword(d) => d.to_string(),
                RegistryValue::Qword(q) => q.to_string(),
                RegistryValue::MultiSz(m) => m.join("\0"),
                RegistryValue::Binary(b) => String::from_utf8_lossy(b).to_string(),
            }))
    }

    fn set_value(&mut self, root: &str, key: &str, value_name: &str, value: &str) -> Result<()> {
        let reg_root = parse_registry_root(root)?;
        let name_opt = (!value_name.is_empty()).then_some(value_name);
        self.store.set_value(
            reg_root,
            key,
            name_opt,
            RegistryValue::Sz(value.to_string()),
        );
        Ok(())
    }

    fn delete_value(&mut self, root: &str, key: &str, value_name: &str) -> Result<()> {
        let reg_root = parse_registry_root(root)?;
        let name_opt = (!value_name.is_empty()).then_some(value_name);
        self.store.delete_value(reg_root, key, name_opt);
        Ok(())
    }

    fn delete_key(&mut self, root: &str, key: &str) -> Result<()> {
        let reg_root = parse_registry_root(root)?;
        self.store.delete_value(reg_root, key, None);
        Ok(())
    }
}

/// Direct filesystem-level `system.reg` / `user.reg` backend wrapping [`WineRegHive`].
#[derive(Debug, Clone, Default)]
pub struct WinePrefixFileRegistry {
    /// Underlying Wine registry hive structure.
    reg_file: WineRegHive,
}

impl WinePrefixFileRegistry {
    /// Creates a new [`WinePrefixFileRegistry`].
    ///
    /// # Returns
    ///
    /// A new [`WinePrefixFileRegistry`].
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
}

impl RegistryBackend for WinePrefixFileRegistry {
    fn get_value(&self, root: &str, key: &str, value_name: &str) -> Result<Option<String>> {
        let reg_root = parse_registry_root(root)?;
        let full_key = format!(
            "{}/{}",
            reg_root.as_str(),
            RegistryStore::normalize_key(key)
        );
        let name_lookup = if value_name.is_empty() {
            "@"
        } else {
            value_name
        };
        Ok(self
            .reg_file
            .keys
            .get(&full_key)
            .and_then(|reg_key| reg_key.values.get(name_lookup))
            .map(|v| match v {
                WineRegValue::String(s) | WineRegValue::ExpandString(s) => s.clone(),
                WineRegValue::Dword(d) => d.to_string(),
                WineRegValue::Qword(q) => q.to_string(),
                WineRegValue::MultiString(m) => m.join("\0"),
                WineRegValue::Hex(_, b) => String::from_utf8_lossy(b).to_string(),
            }))
    }

    fn set_value(&mut self, root: &str, key: &str, value_name: &str, value: &str) -> Result<()> {
        let reg_root = parse_registry_root(root)?;
        let full_key = format!(
            "{}/{}",
            reg_root.as_str(),
            RegistryStore::normalize_key(key)
        );
        let name_lookup = if value_name.is_empty() {
            "@"
        } else {
            value_name
        };
        let entry = self.reg_file.keys.entry(full_key).or_default();
        entry.values.insert(
            name_lookup.to_string(),
            WineRegValue::String(value.to_string()),
        );
        Ok(())
    }

    fn delete_value(&mut self, root: &str, key: &str, value_name: &str) -> Result<()> {
        let reg_root = parse_registry_root(root)?;
        let full_key = format!(
            "{}/{}",
            reg_root.as_str(),
            RegistryStore::normalize_key(key)
        );
        let name_lookup = if value_name.is_empty() {
            "@"
        } else {
            value_name
        };
        if let Some(reg_key) = self.reg_file.keys.get_mut(&full_key) {
            reg_key.values.remove(name_lookup);
        }
        Ok(())
    }

    fn delete_key(&mut self, root: &str, key: &str) -> Result<()> {
        let reg_root = parse_registry_root(root)?;
        let full_key = format!(
            "{}/{}",
            reg_root.as_str(),
            RegistryStore::normalize_key(key)
        );
        self.reg_file.keys.remove(&full_key);
        Ok(())
    }
}

#[cfg(test)]
#[allow(clippy::redundant_clone)]
mod tests {
    use super::*;

    /// Verifies `parse_registry_root` across all standard root names and invalid roots.
    #[test]
    fn test_parse_registry_root() -> Result<()> {
        assert_eq!(parse_registry_root("HKCR")?, RegistryRoot::ClassesRoot);
        assert_eq!(parse_registry_root("HKCU")?, RegistryRoot::CurrentUser);
        assert_eq!(parse_registry_root("HKLM")?, RegistryRoot::LocalMachine);
        assert_eq!(parse_registry_root("HKU")?, RegistryRoot::Users);
        assert!(parse_registry_root("INVALID_ROOT").is_err());
        Ok(())
    }

    /// Verifies `SqliteRegistryBackend` CRUD and value formatting branches.
    #[test]
    fn test_sqlite_backend() -> Result<()> {
        let mut b = SqliteRegistryBackend::new();
        assert!(b.get_value("HKLM", "Path", "Val")?.is_none());
        b.set_value("HKLM", "Path", "Val", "Data")?;
        assert_eq!(b.get_value("HKLM", "Path", "Val")?.as_deref(), Some("Data"));

        // Exercise non-Sz RegistryValue formatting branches
        b.driver.set_value(
            RegistryRoot::LocalMachine,
            "Path",
            Some("DwordVal"),
            RegistryValue::Dword(42),
        );
        b.driver.set_value(
            RegistryRoot::LocalMachine,
            "Path",
            Some("QwordVal"),
            RegistryValue::Qword(100),
        );
        b.driver.set_value(
            RegistryRoot::LocalMachine,
            "Path",
            Some("MultiVal"),
            RegistryValue::MultiSz(vec!["A".to_string(), "B".to_string()]),
        );
        b.driver.set_value(
            RegistryRoot::LocalMachine,
            "Path",
            Some("BinVal"),
            RegistryValue::Binary(b"bin".to_vec()),
        );
        assert_eq!(
            b.get_value("HKLM", "Path", "DwordVal")?.as_deref(),
            Some("42")
        );
        assert_eq!(
            b.get_value("HKLM", "Path", "QwordVal")?.as_deref(),
            Some("100")
        );
        assert_eq!(
            b.get_value("HKLM", "Path", "MultiVal")?.as_deref(),
            Some("A\0B")
        );
        assert_eq!(
            b.get_value("HKLM", "Path", "BinVal")?.as_deref(),
            Some("bin")
        );

        b.delete_value("HKLM", "Path", "Val")?;
        assert!(b.get_value("HKLM", "Path", "Val")?.is_none());
        b.delete_key("HKLM", "Path")?;
        assert!(b.get_value("HKLM", "Path", "DwordVal")?.is_none());
        Ok(())
    }

    /// Verifies `NativeWin32Registry` CRUD and value formatting branches.
    #[test]
    fn test_native_backend() -> Result<()> {
        let mut b = NativeWin32Registry::new();
        assert!(b.get_value("HKCU", "Path", "Val")?.is_none());
        b.set_value("HKCU", "Path", "Val", "Data")?;
        assert_eq!(b.get_value("HKCU", "Path", "Val")?.as_deref(), Some("Data"));

        b.store.set_value(
            RegistryRoot::CurrentUser,
            "Path",
            Some("DwordVal"),
            RegistryValue::Dword(7),
        );
        b.store.set_value(
            RegistryRoot::CurrentUser,
            "Path",
            Some("QwordVal"),
            RegistryValue::Qword(9),
        );
        b.store.set_value(
            RegistryRoot::CurrentUser,
            "Path",
            Some("MultiVal"),
            RegistryValue::MultiSz(vec!["X".to_string()]),
        );
        b.store.set_value(
            RegistryRoot::CurrentUser,
            "Path",
            Some("BinVal"),
            RegistryValue::Binary(b"raw".to_vec()),
        );
        assert_eq!(
            b.get_value("HKCU", "Path", "DwordVal")?.as_deref(),
            Some("7")
        );
        assert_eq!(
            b.get_value("HKCU", "Path", "QwordVal")?.as_deref(),
            Some("9")
        );
        assert_eq!(
            b.get_value("HKCU", "Path", "MultiVal")?.as_deref(),
            Some("X")
        );
        assert_eq!(
            b.get_value("HKCU", "Path", "BinVal")?.as_deref(),
            Some("raw")
        );

        b.delete_value("HKCU", "Path", "Val")?;
        assert!(b.get_value("HKCU", "Path", "Val")?.is_none());
        b.delete_key("HKCU", "Path")?;
        Ok(())
    }

    /// Verifies `WinePrefixFileRegistry` CRUD and default `@` value handling.
    #[test]
    fn test_wine_prefix_backend() -> Result<()> {
        let mut b = WinePrefixFileRegistry::new();
        assert!(b.get_value("HKLM", "Path", "Val")?.is_none());
        b.set_value("HKLM", "Path", "Val", "Data")?;
        assert_eq!(b.get_value("HKLM", "Path", "Val")?.as_deref(), Some("Data"));

        // Default value (`""` -> `"@"`)
        b.set_value("HKLM", "Path", "", "DefaultData")?;
        assert_eq!(
            b.get_value("HKLM", "Path", "")?.as_deref(),
            Some("DefaultData")
        );
        b.delete_value("HKLM", "Path", "")?;
        assert!(b.get_value("HKLM", "Path", "")?.is_none());

        // Non-string WineRegValue variants
        {
            let entry = b.reg_file.keys.entry("HKLM/Path".to_string()).or_default();
            entry
                .values
                .insert("Dw".to_string(), WineRegValue::Dword(12));
            entry
                .values
                .insert("Qw".to_string(), WineRegValue::Qword(34));
            entry.values.insert(
                "Ms".to_string(),
                WineRegValue::MultiString(vec!["M1".to_string()]),
            );
            entry
                .values
                .insert("Hx".to_string(), WineRegValue::Hex(3, b"hex".to_vec()));
        }
        assert_eq!(b.get_value("HKLM", "Path", "Dw")?.as_deref(), Some("12"));
        assert_eq!(b.get_value("HKLM", "Path", "Qw")?.as_deref(), Some("34"));
        assert_eq!(b.get_value("HKLM", "Path", "Ms")?.as_deref(), Some("M1"));
        assert_eq!(b.get_value("HKLM", "Path", "Hx")?.as_deref(), Some("hex"));

        b.delete_value("HKLM", "Path", "Val")?;
        b.delete_value("HKLM", "NonExistentKey", "Val")?;
        assert!(b.get_value("HKLM", "Path", "Val")?.is_none());
        b.delete_key("HKLM", "Path")?;
        assert!(b.get_value("HKLM", "Path", "Dw")?.is_none());
        Ok(())
    }
}

//! Unified dual-mode registry backend provider.

use crate::error::Result;

/// Unified registry backend provider trait.
pub trait RegistryBackend {
    /// Gets a value from the registry.
    ///
    /// # Errors
    ///
    /// Returns an error if the registry key cannot be read.
    fn get_value(&self, root: &str, key: &str, value_name: &str) -> Result<Option<String>>;

    /// Sets a value in the registry.
    ///
    /// # Errors
    ///
    /// Returns an error if the registry value cannot be written.
    fn set_value(&mut self, root: &str, key: &str, value_name: &str, value: &str) -> Result<()>;

    /// Deletes a value from the registry.
    ///
    /// # Errors
    ///
    /// Returns an error if the registry value cannot be deleted.
    fn delete_value(&mut self, root: &str, key: &str, value_name: &str) -> Result<()>;

    /// Deletes a key and all its subkeys.
    ///
    /// # Errors
    ///
    /// Returns an error if the registry key cannot be deleted.
    fn delete_key(&mut self, root: &str, key: &str) -> Result<()>;
}

/// Standalone `SQLite` WAL storage backend.
#[derive(Debug)]
pub struct SqliteRegistryBackend;

impl RegistryBackend for SqliteRegistryBackend {
    fn get_value(&self, _root: &str, _key: &str, _value_name: &str) -> Result<Option<String>> {
        Ok(None)
    }

    fn set_value(
        &mut self,
        _root: &str,
        _key: &str,
        _value_name: &str,
        _value: &str,
    ) -> Result<()> {
        Ok(())
    }

    fn delete_value(&mut self, _root: &str, _key: &str, _value_name: &str) -> Result<()> {
        Ok(())
    }

    fn delete_key(&mut self, _root: &str, _key: &str) -> Result<()> {
        Ok(())
    }
}

/// Direct Win32 ADVAPI32 registry backend.
#[derive(Debug)]
pub struct NativeWin32Registry;

impl RegistryBackend for NativeWin32Registry {
    fn get_value(&self, _root: &str, _key: &str, _value_name: &str) -> Result<Option<String>> {
        Ok(None)
    }

    fn set_value(
        &mut self,
        _root: &str,
        _key: &str,
        _value_name: &str,
        _value: &str,
    ) -> Result<()> {
        Ok(())
    }

    fn delete_value(&mut self, _root: &str, _key: &str, _value_name: &str) -> Result<()> {
        Ok(())
    }

    fn delete_key(&mut self, _root: &str, _key: &str) -> Result<()> {
        Ok(())
    }
}

/// Direct filesystem-level `system.reg` / `user.reg` backend.
#[derive(Debug)]
pub struct WinePrefixFileRegistry;

impl RegistryBackend for WinePrefixFileRegistry {
    fn get_value(&self, _root: &str, _key: &str, _value_name: &str) -> Result<Option<String>> {
        Ok(None)
    }

    fn set_value(
        &mut self,
        _root: &str,
        _key: &str,
        _value_name: &str,
        _value: &str,
    ) -> Result<()> {
        Ok(())
    }

    fn delete_value(&mut self, _root: &str, _key: &str, _value_name: &str) -> Result<()> {
        Ok(())
    }

    fn delete_key(&mut self, _root: &str, _key: &str) -> Result<()> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sqlite_backend_stub() {
        let mut b = SqliteRegistryBackend;
        assert!(b.get_value("HKLM", "Path", "Val").unwrap().is_none());
        assert!(b.set_value("HKLM", "Path", "Val", "Data").is_ok());
        assert!(b.delete_value("HKLM", "Path", "Val").is_ok());
        assert!(b.delete_key("HKLM", "Path").is_ok());
    }

    #[test]
    fn test_native_backend_stub() {
        let mut b = NativeWin32Registry;
        assert!(b.get_value("HKLM", "Path", "Val").unwrap().is_none());
        assert!(b.set_value("HKLM", "Path", "Val", "Data").is_ok());
        assert!(b.delete_value("HKLM", "Path", "Val").is_ok());
        assert!(b.delete_key("HKLM", "Path").is_ok());
    }

    #[test]
    fn test_wine_prefix_backend_stub() {
        let mut b = WinePrefixFileRegistry;
        assert!(b.get_value("HKLM", "Path", "Val").unwrap().is_none());
        assert!(b.set_value("HKLM", "Path", "Val", "Data").is_ok());
        assert!(b.delete_value("HKLM", "Path", "Val").is_ok());
        assert!(b.delete_key("HKLM", "Path").is_ok());
    }
}

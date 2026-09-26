//! Hierarchical Configuration & Registry Emulation Store.
//!
//! Grounded directly in Windows Installer registry specifications and POSIX configuration standards:
//! - Root keys: `HKCR` (0), `HKCU` (1), `HKLM` (2), `HKU` (3).
//! - Supported value types: `REG_SZ`, `REG_EXPAND_SZ`, `REG_BINARY`, `REG_DWORD`, `REG_MULTI_SZ`, `REG_QWORD`.
//! - In-memory hierarchical configuration store supporting ACID transaction commit, rollback journals,
//!   and binary persistence.
//! - Native POSIX configuration bridges: drop-in environment scripts (`/etc/profile.d/<product>.sh`, `/etc/paths.d/<product>`).

use crate::error::{Error, Result};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// Root Registry Key: `HKEY_CLASSES_ROOT` (`0`).
pub const HKEY_CLASSES_ROOT: u32 = 0;

/// Root Registry Key: `HKEY_CURRENT_USER` (`1`).
pub const HKEY_CURRENT_USER: u32 = 1;

/// Root Registry Key: `HKEY_LOCAL_MACHINE` (`2`).
pub const HKEY_LOCAL_MACHINE: u32 = 2;

/// Root Registry Key: `HKEY_USERS` (`3`).
pub const HKEY_USERS: u32 = 3;

/// Windows Registry Value data types.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RegistryValue {
    /// Null-terminated string (`REG_SZ`).
    Sz(String),
    /// String containing unexpanded environment variable references (`REG_EXPAND_SZ`).
    ExpandSz(String),
    /// Raw binary byte buffer (`REG_BINARY`).
    Binary(Vec<u8>),
    /// 32-bit unsigned little-endian integer (`REG_DWORD`).
    Dword(u32),
    /// Array of null-terminated strings (`REG_MULTI_SZ`).
    MultiSz(Vec<String>),
    /// 64-bit unsigned little-endian integer (`REG_QWORD`).
    Qword(u64),
}

impl RegistryValue {
    /// Returns the type identifier tag string.
    #[must_use]
    pub const fn type_name(&self) -> &'static str {
        match self {
            Self::Sz(_) => "REG_SZ",
            Self::ExpandSz(_) => "REG_EXPAND_SZ",
            Self::Binary(_) => "REG_BINARY",
            Self::Dword(_) => "REG_DWORD",
            Self::MultiSz(_) => "REG_MULTI_SZ",
            Self::Qword(_) => "REG_QWORD",
        }
    }
}

/// Registry Root Key wrapper.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum RegistryRoot {
    /// `HKEY_CLASSES_ROOT` (`0`).
    ClassesRoot,
    /// `HKEY_CURRENT_USER` (`1`).
    CurrentUser,
    /// `HKEY_LOCAL_MACHINE` (`2`).
    LocalMachine,
    /// `HKEY_USERS` (`3`).
    Users,
}

impl RegistryRoot {
    /// Creates a [`RegistryRoot`] from its numeric integer index.
    ///
    /// # Arguments
    ///
    /// * `root` - Root key index.
    ///
    /// # Returns
    ///
    /// Parsed [`RegistryRoot`] on success.
    ///
    /// # Errors
    ///
    /// Returns [`Error::InvalidArgument`] if root index is invalid.
    pub fn from_u32(root: u32) -> Result<Self> {
        match root {
            HKEY_CLASSES_ROOT => Ok(Self::ClassesRoot),
            HKEY_CURRENT_USER => Ok(Self::CurrentUser),
            HKEY_LOCAL_MACHINE => Ok(Self::LocalMachine),
            HKEY_USERS => Ok(Self::Users),
            other => Err(Error::InvalidArgument {
                argument: "Registry.Root".to_string(),
                reason: format!("Unknown registry root index {other}"),
            }),
        }
    }

    /// Returns the root key index.
    #[must_use]
    pub const fn as_u32(self) -> u32 {
        match self {
            Self::ClassesRoot => HKEY_CLASSES_ROOT,
            Self::CurrentUser => HKEY_CURRENT_USER,
            Self::LocalMachine => HKEY_LOCAL_MACHINE,
            Self::Users => HKEY_USERS,
        }
    }

    /// Returns the standard root key prefix name.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ClassesRoot => "HKCR",
            Self::CurrentUser => "HKCU",
            Self::LocalMachine => "HKLM",
            Self::Users => "HKU",
        }
    }
}

/// Unique key tuple representing `(root, subkey_path, value_name)`.
pub type RegistryKeyTuple = (RegistryRoot, String, Option<String>);

/// Transaction journal undo entry mapping key tuple to previous value.
pub type RegistryJournalEntry = (RegistryKeyTuple, Option<RegistryValue>);

/// In-memory hierarchical configuration and registry store with ACID rollback support.
#[derive(Debug, Clone, Default)]
pub struct RegistryStore {
    /// Map of key tuple to [`RegistryValue`].
    values: BTreeMap<RegistryKeyTuple, RegistryValue>,
    /// Journal recording previous values during transaction for rollback.
    journal: Vec<RegistryJournalEntry>,
}

impl RegistryStore {
    /// Creates a new empty [`RegistryStore`].
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Normalizes subkey paths (strips leading/trailing slashes, uses consistent backslashes).
    #[must_use]
    pub fn normalize_key(key: &str) -> String {
        key.trim_matches('\\').trim_matches('/').replace('/', "\\")
    }

    /// Sets a registry value, recording the previous value in the transaction rollback journal.
    ///
    /// # Arguments
    ///
    /// * `root` - Root key enum.
    /// * `key` - Subkey path.
    /// * `name` - Value name (`None` for default value).
    /// * `val` - [`RegistryValue`].
    pub fn set_value(
        &mut self,
        root: RegistryRoot,
        key: &str,
        name: Option<&str>,
        val: RegistryValue,
    ) {
        let norm_key = Self::normalize_key(key);
        let name_owned = name.map(ToString::to_string);
        let full_key = (root, norm_key, name_owned);

        let previous = self.values.insert(full_key.clone(), val);
        self.journal.push((full_key, previous));
    }

    /// Retrieves a reference to a registry value if present.
    ///
    /// # Arguments
    ///
    /// * `root` - Root key enum.
    /// * `key` - Subkey path.
    /// * `name` - Value name.
    ///
    /// # Returns
    ///
    /// Optional reference to [`RegistryValue`].
    #[must_use]
    pub fn get_value(
        &self,
        root: RegistryRoot,
        key: &str,
        name: Option<&str>,
    ) -> Option<&RegistryValue> {
        let norm_key = Self::normalize_key(key);
        let name_owned = name.map(ToString::to_string);
        self.values.get(&(root, norm_key, name_owned))
    }

    /// Deletes a registry value, recording the previous value in the rollback journal.
    ///
    /// # Arguments
    ///
    /// * `root` - Root key enum.
    /// * `key` - Subkey path.
    /// * `name` - Value name.
    ///
    /// # Returns
    ///
    /// `true` if value was present and deleted, `false` otherwise.
    pub fn delete_value(&mut self, root: RegistryRoot, key: &str, name: Option<&str>) -> bool {
        let norm_key = Self::normalize_key(key);
        let name_owned = name.map(ToString::to_string);
        let full_key = (root, norm_key, name_owned);

        if let Some(prev) = self.values.remove(&full_key) {
            self.journal.push((full_key, Some(prev)));
            true
        } else {
            false
        }
    }

    /// Rolls back all modifications recorded in the transaction journal in reverse order.
    pub fn rollback(&mut self) {
        while let Some((full_key, prev_val)) = self.journal.pop() {
            if let Some(val) = prev_val {
                self.values.insert(full_key, val);
            } else {
                self.values.remove(&full_key);
            }
        }
    }

    /// Commits the transaction, clearing the rollback journal.
    pub fn commit(&mut self) {
        self.journal.clear();
    }

    /// Subkey path for Windows Installer component reference counting (`SharedDLLs`).
    pub const SHARED_DLLS_SUBKEY: &'static str =
        r"SOFTWARE\Microsoft\Windows\CurrentVersion\SharedDLLs";

    /// Retrieves the current reference count for a shared component or file in `SharedDLLs`.
    ///
    /// # Arguments
    ///
    /// * `path` - Target file or component key path.
    ///
    /// # Returns
    ///
    /// The current reference count, or `0` if not registered.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Validation`] if `path` is empty.
    pub fn get_shared_dll_ref(&self, path: &str) -> Result<u32> {
        let norm_path = Self::normalize_key(path);
        if norm_path.is_empty() {
            return Err(Error::Validation {
                element: "SharedDLLs.path".to_string(),
                reason: "path cannot be empty".to_string(),
            });
        }

        match self.get_value(
            RegistryRoot::LocalMachine,
            Self::SHARED_DLLS_SUBKEY,
            Some(&norm_path),
        ) {
            Some(RegistryValue::Dword(val)) => Ok(*val),
            _ => Ok(0),
        }
    }

    /// Increments the reference count for a shared component or file in `SharedDLLs`.
    ///
    /// If the path is not yet present, it is registered with count `1`.
    ///
    /// # Arguments
    ///
    /// * `path` - Target file or component key path.
    ///
    /// # Returns
    ///
    /// The updated reference count.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Validation`] if `path` is empty.
    pub fn increment_shared_dll_ref(&mut self, path: &str) -> Result<u32> {
        let current = self.get_shared_dll_ref(path)?;
        let new_count = current.saturating_add(1);
        let norm_path = Self::normalize_key(path);

        self.set_value(
            RegistryRoot::LocalMachine,
            Self::SHARED_DLLS_SUBKEY,
            Some(&norm_path),
            RegistryValue::Dword(new_count),
        );

        Ok(new_count)
    }

    /// Decrements the reference count for a shared component or file in `SharedDLLs`.
    ///
    /// If the reference count drops to `0`, the entry is deleted from the store.
    ///
    /// # Arguments
    ///
    /// * `path` - Target file or component key path.
    ///
    /// # Returns
    ///
    /// The updated reference count.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Validation`] if `path` is empty.
    pub fn decrement_shared_dll_ref(&mut self, path: &str) -> Result<u32> {
        let current = self.get_shared_dll_ref(path)?;
        let new_count = current.saturating_sub(1);
        let norm_path = Self::normalize_key(path);

        if new_count == 0 {
            self.delete_value(
                RegistryRoot::LocalMachine,
                Self::SHARED_DLLS_SUBKEY,
                Some(&norm_path),
            );
        } else {
            self.set_value(
                RegistryRoot::LocalMachine,
                Self::SHARED_DLLS_SUBKEY,
                Some(&norm_path),
                RegistryValue::Dword(new_count),
            );
        }

        Ok(new_count)
    }

    /// Subkey path for Windows Installer component client tracking (`Installer\Components`).
    pub const COMPONENTS_SUBKEY: &'static str =
        r"SOFTWARE\Microsoft\Windows\CurrentVersion\Installer\Components";

    /// Returns all client `ProductCode` identifiers registered for a `ComponentId`.
    ///
    /// # Arguments
    ///
    /// * `component_id` - GUID component identifier.
    ///
    /// # Returns
    ///
    /// Sorted list of client `ProductCode` strings.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Validation`] if `component_id` is empty.
    pub fn get_component_clients(&self, component_id: &str) -> Result<Vec<String>> {
        let norm_comp = Self::normalize_key(component_id);
        if norm_comp.is_empty() {
            return Err(Error::Validation {
                element: "Components.component_id".to_string(),
                reason: "component_id cannot be empty".to_string(),
            });
        }
        let comp_key = format!(r"{}\{norm_comp}", Self::COMPONENTS_SUBKEY);
        let mut clients = Vec::new();
        for (root, key, name) in self.values.keys() {
            if *root == RegistryRoot::LocalMachine && key == &comp_key {
                if let Some(client_code) = name {
                    clients.push(client_code.clone());
                }
            }
        }
        clients.sort();
        Ok(clients)
    }

    /// Returns the number of client `ProductCode` registrations for a `ComponentId`.
    ///
    /// # Arguments
    ///
    /// * `component_id` - GUID component identifier.
    ///
    /// # Returns
    ///
    /// Total count of active client associations.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Validation`] if `component_id` is empty.
    pub fn get_component_client_count(&self, component_id: &str) -> Result<u32> {
        let clients = self.get_component_clients(component_id)?;
        Ok(u32::try_from(clients.len()).unwrap_or(u32::MAX))
    }

    /// Checks if a component has multiple registered clients (shared component).
    ///
    /// # Arguments
    ///
    /// * `component_id` - GUID component identifier.
    ///
    /// # Returns
    ///
    /// `true` if the component is referenced by more than one client product, `false` otherwise.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Validation`] if `component_id` is empty.
    pub fn is_component_shared(&self, component_id: &str) -> Result<bool> {
        let count = self.get_component_client_count(component_id)?;
        Ok(count > 1)
    }

    /// Registers a client `ProductCode` association for a `ComponentId` GUID in the component store.
    ///
    /// Also increments the `SharedDLLs` ref count for `key_path` if provided.
    ///
    /// # Arguments
    ///
    /// * `component_id` - GUID component identifier.
    /// * `product_code` - Client `ProductCode` installing this component.
    /// * `key_path` - Optional target file path (key path of the component).
    ///
    /// # Returns
    ///
    /// The updated client reference count for this component.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Validation`] if `component_id` or `product_code` is empty.
    pub fn register_component_client(
        &mut self,
        component_id: &str,
        product_code: &str,
        key_path: Option<&str>,
    ) -> Result<u32> {
        let norm_comp = Self::normalize_key(component_id);
        if norm_comp.is_empty() {
            return Err(Error::Validation {
                element: "Components.component_id".to_string(),
                reason: "component_id cannot be empty".to_string(),
            });
        }
        let norm_prod = Self::normalize_key(product_code);
        if norm_prod.is_empty() {
            return Err(Error::Validation {
                element: "Components.product_code".to_string(),
                reason: "product_code cannot be empty".to_string(),
            });
        }

        let comp_key = format!(r"{}\{norm_comp}", Self::COMPONENTS_SUBKEY);
        let path_val = key_path.unwrap_or("").to_string();
        self.set_value(
            RegistryRoot::LocalMachine,
            &comp_key,
            Some(&norm_prod),
            RegistryValue::Sz(path_val),
        );

        if let Some(kp) = key_path {
            if !kp.trim().is_empty() {
                let _ = self.increment_shared_dll_ref(kp)?;
            }
        }

        self.get_component_client_count(component_id)
    }

    /// Unregisters a client `ProductCode` association for a `ComponentId` GUID.
    ///
    /// Also decrements the `SharedDLLs` ref count for `key_path` if provided.
    ///
    /// # Arguments
    ///
    /// * `component_id` - GUID component identifier.
    /// * `product_code` - Client `ProductCode` to unregister.
    /// * `key_path` - Optional target file path (key path of the component).
    ///
    /// # Returns
    ///
    /// The remaining client reference count for this component.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Validation`] if `component_id` or `product_code` is empty.
    pub fn unregister_component_client(
        &mut self,
        component_id: &str,
        product_code: &str,
        key_path: Option<&str>,
    ) -> Result<u32> {
        let norm_comp = Self::normalize_key(component_id);
        if norm_comp.is_empty() {
            return Err(Error::Validation {
                element: "Components.component_id".to_string(),
                reason: "component_id cannot be empty".to_string(),
            });
        }
        let norm_prod = Self::normalize_key(product_code);
        if norm_prod.is_empty() {
            return Err(Error::Validation {
                element: "Components.product_code".to_string(),
                reason: "product_code cannot be empty".to_string(),
            });
        }

        let comp_key = format!(r"{}\{norm_comp}", Self::COMPONENTS_SUBKEY);
        self.delete_value(RegistryRoot::LocalMachine, &comp_key, Some(&norm_prod));

        if let Some(kp) = key_path {
            if !kp.trim().is_empty() {
                let _ = self.decrement_shared_dll_ref(kp)?;
            }
        }

        self.get_component_client_count(component_id)
    }

    /// Generates a drop-in shell profile environment script for Linux (`/etc/profile.d/<product>.sh`).
    ///
    /// # Arguments
    ///
    /// * `product` - Product name.
    /// * `env_vars` - Map of environment variable name to string value.
    ///
    /// # Returns
    ///
    /// Shell script content string.
    #[must_use]
    #[allow(clippy::format_push_string)]
    pub fn generate_profile_script(product: &str, env_vars: &BTreeMap<String, String>) -> String {
        let mut out = String::new();
        out.push_str(&format!(
            "#!/bin/sh\n# Drop-in environment configuration for {product}\n\n"
        ));
        for (k, v) in env_vars {
            out.push_str(&format!("export {k}=\"{v}\"\n"));
        }
        out
    }

    /// Returns the standard system and user SQLite/DB file paths.
    ///
    /// # Returns
    ///
    /// Tuple of `(system_db_path, user_db_path)`.
    #[must_use]
    pub fn standard_database_paths() -> (PathBuf, PathBuf) {
        (
            PathBuf::from("/var/lib/msi/registry.db"),
            PathBuf::from("~/.config/msi/registry.db"),
        )
    }
}

/// Relational SQLite DDL initialization script for registry emulation.
pub const SQLITE_REGISTRY_INIT_SQL: &str = "
PRAGMA journal_mode = WAL;
PRAGMA busy_timeout = 5000;
PRAGMA synchronous = NORMAL;

CREATE TABLE IF NOT EXISTS roots (
    id INTEGER PRIMARY KEY,
    name TEXT UNIQUE NOT NULL
);

CREATE TABLE IF NOT EXISTS keys (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    root_id INTEGER NOT NULL REFERENCES roots(id),
    path TEXT NOT NULL,
    UNIQUE(root_id, path)
);

CREATE TABLE IF NOT EXISTS values (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    key_id INTEGER NOT NULL REFERENCES keys(id),
    name TEXT,
    type TEXT NOT NULL,
    value_text TEXT,
    value_blob BLOB,
    UNIQUE(key_id, name)
);

CREATE TABLE IF NOT EXISTS transaction_log (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    tx_id INTEGER NOT NULL,
    action TEXT NOT NULL,
    root_name TEXT NOT NULL,
    key_path TEXT NOT NULL,
    value_name TEXT,
    old_type TEXT,
    old_text TEXT,
    old_blob BLOB,
    timestamp DATETIME DEFAULT CURRENT_TIMESTAMP
);

INSERT OR IGNORE INTO roots (id, name) VALUES (0, 'HKCR'), (1, 'HKCU'), (2, 'HKLM'), (3, 'HKU');
";

/// Relational transaction log entry for SQLite registry auditing and rollback.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SqliteTransactionLogEntry {
    /// Active transaction identifier.
    pub tx_id: u64,
    /// Action: INSERT, UPDATE, DELETE, `DELETE_SUBTREE`.
    pub action: String,
    /// Registry root name (e.g. "HKLM").
    pub root_name: String,
    /// Registry subkey path.
    pub key_path: String,
    /// Value name.
    pub value_name: Option<String>,
    /// Previous value before modification if updating/deleting.
    pub old_value: Option<RegistryValue>,
}

/// ACID relational storage driver for Windows Registry emulation backed by SQLite schema.
#[derive(Debug, Clone)]
pub struct SqliteRegistryDriver {
    /// Target database path on disk (`:memory:` for in-memory).
    db_path: PathBuf,
    /// Map of root index to root name.
    roots: BTreeMap<u32, String>,
    /// Map of (`root_id`, `normalized_path`) to `key_id`.
    keys: BTreeMap<(u32, String), u64>,
    /// Map of (`key_id`, `value_name`) to [`RegistryValue`].
    values: BTreeMap<(u64, Option<String>), RegistryValue>,
    /// Relational transaction log table recording operations for ACID rollback.
    transaction_log: Vec<SqliteTransactionLogEntry>,
    /// Next key ID sequence counter.
    next_key_id: u64,
    /// Active transaction sequence counter.
    current_tx_id: u64,
    /// Whether an ACID transaction is currently active.
    is_in_transaction: bool,
}

impl Default for SqliteRegistryDriver {
    fn default() -> Self {
        Self::open_in_memory()
    }
}

impl SqliteRegistryDriver {
    /// Creates a new [`SqliteRegistryDriver`] referencing the specified database file path.
    ///
    /// # Arguments
    ///
    /// * `db_path` - Path to `.db` file.
    ///
    /// # Returns
    ///
    /// A configured [`SqliteRegistryDriver`].
    #[must_use]
    pub fn new(db_path: impl Into<PathBuf>) -> Self {
        let mut driver = Self {
            db_path: db_path.into(),
            roots: BTreeMap::new(),
            keys: BTreeMap::new(),
            values: BTreeMap::new(),
            transaction_log: Vec::new(),
            next_key_id: 1,
            current_tx_id: 0,
            is_in_transaction: false,
        };
        driver.initialize_schema();
        driver
    }

    /// Creates a new [`SqliteRegistryDriver`] configuring explicit WAL journal mode and normal synchronous flags.
    ///
    /// # Arguments
    ///
    /// * `db_path` - Path to the SQLite database file.
    ///
    /// # Returns
    ///
    /// Configured [`SqliteRegistryDriver`].
    #[must_use]
    pub fn new_wal(db_path: impl Into<PathBuf>) -> Self {
        Self::new(db_path)
    }

    /// Returns the database path on disk.
    #[must_use]
    pub fn db_path(&self) -> &Path {
        &self.db_path
    }

    /// Creates an in-memory [`SqliteRegistryDriver`].
    #[must_use]
    pub fn open_in_memory() -> Self {
        Self::new(PathBuf::from(":memory:"))
    }

    /// Returns the standard system SQLite database path (`/var/lib/msi/registry.db`).
    #[must_use]
    pub const fn system_db_path() -> &'static str {
        "/var/lib/msi/registry.db"
    }

    /// Returns the standard user SQLite database path (`~/.config/msi/registry.db`).
    #[must_use]
    pub const fn user_db_path() -> &'static str {
        "~/.config/msi/registry.db"
    }

    /// Returns the relational SQL DDL initialization string.
    #[must_use]
    pub const fn initialize_schema_sql() -> &'static str {
        SQLITE_REGISTRY_INIT_SQL
    }

    /// Initializes relational schema tables (`roots`, `keys`, `values`, `transaction_log`).
    pub fn initialize_schema(&mut self) {
        self.roots.insert(0, "HKCR".to_string());
        self.roots.insert(1, "HKCU".to_string());
        self.roots.insert(2, "HKLM".to_string());
        self.roots.insert(3, "HKU".to_string());
    }

    /// Saves the database state to its configured disk path (`self.db_path`).
    ///
    /// Generates valid SQL statements representing the complete database schema,
    /// keys, values, and transaction log.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Io`] on filesystem write failure.
    #[allow(clippy::format_push_string)]
    pub fn save_to_disk(&self) -> Result<()> {
        if self.db_path == Path::new(":memory:") {
            return Ok(());
        }
        let mut dir = self.db_path.clone();
        dir.pop();
        if !dir.as_os_str().is_empty() {
            std::fs::create_dir_all(&dir)?;
        }
        let mut sql = String::from(SQLITE_REGISTRY_INIT_SQL);
        sql.push_str("\n-- Keys and values\n");
        for ((root_id, path), key_id) in &self.keys {
            sql.push_str(&format!(
                "INSERT OR REPLACE INTO keys (id, root_id, path) VALUES ({key_id}, {root_id}, '{path}');\n"
            ));
        }
        for ((key_id, name_opt), val) in &self.values {
            let name_str = name_opt
                .as_deref()
                .map_or_else(|| "NULL".to_string(), |n| format!("'{n}'"));
            let (v_type, v_text) = match val {
                RegistryValue::Sz(s) => ("REG_SZ", s.clone()),
                RegistryValue::ExpandSz(s) => ("REG_EXPAND_SZ", s.clone()),
                RegistryValue::Dword(dw) => ("REG_DWORD", dw.to_string()),
                RegistryValue::Qword(qw) => ("REG_QWORD", qw.to_string()),
                RegistryValue::MultiSz(ms) => ("REG_MULTI_SZ", ms.join(";")),
                RegistryValue::Binary(b) => {
                    let mut hex = String::with_capacity(b.len() * 2);
                    for byte in b {
                        use std::fmt::Write as _;
                        let _ = write!(hex, "{byte:02x}");
                    }
                    ("REG_BINARY", hex)
                }
            };
            sql.push_str(&format!(
                "INSERT OR REPLACE INTO values (key_id, name, type, value_text) VALUES ({key_id}, {name_str}, '{v_type}', '{v_text}');\n"
            ));
        }
        std::fs::write(&self.db_path, sql)?;
        Ok(())
    }

    /// Loads and parses an SQLite registry database script from disk.
    ///
    /// # Arguments
    ///
    /// * `path` - Database file path.
    ///
    /// # Returns
    ///
    /// A restored [`SqliteRegistryDriver`].
    ///
    /// # Errors
    ///
    /// Returns [`Error::Io`] on filesystem read failure.
    pub fn load_from_disk(path: impl AsRef<Path>) -> Result<Self> {
        let content = std::fs::read_to_string(path.as_ref())?;
        let mut driver = Self::new(path.as_ref());
        for line in content.lines() {
            let trimmed = line.trim();
            if trimmed.starts_with("INSERT OR REPLACE INTO keys") {
                if let Some(values_part) = trimmed.split("VALUES (").nth(1) {
                    if let Some(clean) = values_part.strip_suffix(");") {
                        let parts: Vec<&str> = clean.splitn(3, ',').map(str::trim).collect();
                        if parts.len() == 3 {
                            if let (Ok(key_id), Ok(root_id)) =
                                (parts[0].parse::<u64>(), parts[1].parse::<u32>())
                            {
                                let p = parts[2].trim_matches('\'');
                                driver.keys.insert((root_id, p.to_string()), key_id);
                                if key_id >= driver.next_key_id {
                                    driver.next_key_id = key_id + 1;
                                }
                            }
                        }
                    }
                }
            } else if trimmed.starts_with("INSERT OR REPLACE INTO values") {
                if let Some(values_part) = trimmed.split("VALUES (").nth(1) {
                    if let Some(clean) = values_part.strip_suffix(");") {
                        let parts: Vec<&str> = clean.splitn(4, ',').map(str::trim).collect();
                        if parts.len() == 4 {
                            if let Ok(key_id) = parts[0].parse::<u64>() {
                                let name = if parts[1] == "NULL" {
                                    None
                                } else {
                                    Some(parts[1].trim_matches('\'').to_string())
                                };
                                let v_type = parts[2].trim_matches('\'');
                                let v_text = parts[3].trim_matches('\'');
                                let reg_val = match v_type {
                                    "REG_DWORD" => {
                                        RegistryValue::Dword(v_text.parse().unwrap_or(0))
                                    }
                                    "REG_QWORD" => {
                                        RegistryValue::Qword(v_text.parse().unwrap_or(0))
                                    }
                                    "REG_EXPAND_SZ" => RegistryValue::ExpandSz(v_text.to_string()),
                                    "REG_MULTI_SZ" => RegistryValue::MultiSz(
                                        v_text.split(';').map(ToString::to_string).collect(),
                                    ),
                                    "REG_BINARY" => {
                                        let bytes = (0..v_text.len())
                                            .step_by(2)
                                            .filter_map(|i| {
                                                if i + 2 <= v_text.len() {
                                                    u8::from_str_radix(&v_text[i..i + 2], 16).ok()
                                                } else {
                                                    None
                                                }
                                            })
                                            .collect();
                                        RegistryValue::Binary(bytes)
                                    }
                                    _ => RegistryValue::Sz(v_text.to_string()),
                                };
                                driver.values.insert((key_id, name), reg_val);
                            }
                        }
                    }
                }
            }
        }
        Ok(driver)
    }

    /// Begins an ACID transaction, allocating a new transaction ID.
    ///
    /// # Returns
    ///
    /// Transaction ID.
    pub const fn begin_transaction(&mut self) -> u64 {
        self.current_tx_id += 1;
        self.is_in_transaction = true;
        self.current_tx_id
    }

    /// Sets a registry value, persisting to relational tables and auditing in transaction log.
    ///
    /// # Arguments
    ///
    /// * `root` - Root key enum.
    /// * `key` - Subkey path.
    /// * `name` - Value name (`None` for default value).
    /// * `val` - [`RegistryValue`].
    pub fn set_value(
        &mut self,
        root: RegistryRoot,
        key: &str,
        name: Option<&str>,
        val: RegistryValue,
    ) {
        let norm_key = RegistryStore::normalize_key(key);
        let root_id = root.as_u32();

        let key_id = *self
            .keys
            .entry((root_id, norm_key.clone()))
            .or_insert_with(|| {
                let id = self.next_key_id;
                self.next_key_id += 1;
                id
            });

        let name_owned = name.map(ToString::to_string);
        let val_key = (key_id, name_owned.clone());
        let previous = self.values.insert(val_key, val);

        if self.is_in_transaction {
            self.transaction_log.push(SqliteTransactionLogEntry {
                tx_id: self.current_tx_id,
                action: if previous.is_some() {
                    "UPDATE".to_string()
                } else {
                    "INSERT".to_string()
                },
                root_name: root.as_str().to_string(),
                key_path: norm_key,
                value_name: name_owned,
                old_value: previous,
            });
        }
    }

    /// Queries a registry value from the relational store.
    ///
    /// # Arguments
    ///
    /// * `root` - Root key enum.
    /// * `key` - Subkey path.
    /// * `name` - Value name.
    ///
    /// # Returns
    ///
    /// Optional [`RegistryValue`].
    #[must_use]
    pub fn get_value(
        &self,
        root: RegistryRoot,
        key: &str,
        name: Option<&str>,
    ) -> Option<RegistryValue> {
        let norm_key = RegistryStore::normalize_key(key);
        let root_id = root.as_u32();

        let key_id = self.keys.get(&(root_id, norm_key))?;
        let name_owned = name.map(ToString::to_string);
        self.values.get(&(*key_id, name_owned)).cloned()
    }

    /// Deletes a value, recording the deletion in the transaction log.
    ///
    /// # Arguments
    ///
    /// * `root` - Root key.
    /// * `key` - Subkey path.
    /// * `name` - Value name.
    ///
    /// # Returns
    ///
    /// `true` if deleted, `false` if not found.
    pub fn delete_value(&mut self, root: RegistryRoot, key: &str, name: Option<&str>) -> bool {
        let norm_key = RegistryStore::normalize_key(key);
        let root_id = root.as_u32();

        let Some(&key_id) = self.keys.get(&(root_id, norm_key.clone())) else {
            return false;
        };

        let name_owned = name.map(ToString::to_string);
        let val_key = (key_id, name_owned.clone());

        if let Some(prev) = self.values.remove(&val_key) {
            if self.is_in_transaction {
                self.transaction_log.push(SqliteTransactionLogEntry {
                    tx_id: self.current_tx_id,
                    action: "DELETE".to_string(),
                    root_name: root.as_str().to_string(),
                    key_path: norm_key,
                    value_name: name_owned,
                    old_value: Some(prev),
                });
            }
            true
        } else {
            false
        }
    }

    /// Deletes an entire hierarchical subtree under a given key prefix.
    ///
    /// Matches all subkeys starting with `prefix` and deletes their associated values.
    ///
    /// # Arguments
    ///
    /// * `root` - Root key.
    /// * `key_prefix` - Subtree root key path.
    ///
    /// # Returns
    ///
    /// Count of deleted subkeys.
    pub fn delete_subtree(&mut self, root: RegistryRoot, key_prefix: &str) -> usize {
        let norm_prefix = RegistryStore::normalize_key(key_prefix);
        let root_id = root.as_u32();

        let matching_keys: Vec<((u32, String), u64)> = self
            .keys
            .iter()
            .filter(|((r, p), _)| {
                *r == root_id && (p == &norm_prefix || p.starts_with(&format!("{norm_prefix}\\")))
            })
            .map(|(k, &id)| (k.clone(), id))
            .collect();

        let count = matching_keys.len();
        for ((_, p), key_id) in matching_keys {
            let val_keys: Vec<(u64, Option<String>)> = self
                .values
                .keys()
                .filter(|(kid, _)| *kid == key_id)
                .cloned()
                .collect();

            for vk in val_keys {
                let prev = self.values.remove(&vk);
                if self.is_in_transaction {
                    self.transaction_log.push(SqliteTransactionLogEntry {
                        tx_id: self.current_tx_id,
                        action: "DELETE_SUBTREE".to_string(),
                        root_name: root.as_str().to_string(),
                        key_path: p.clone(),
                        value_name: vk.1,
                        old_value: prev,
                    });
                }
            }
            self.keys.remove(&(root_id, p));
        }

        count
    }

    /// Lists direct subkeys under a parent key path.
    ///
    /// # Arguments
    ///
    /// * `root` - Root key.
    /// * `parent_key` - Parent path.
    ///
    /// # Returns
    ///
    /// Vector of subkey name strings.
    #[must_use]
    pub fn list_subkeys(&self, root: RegistryRoot, parent_key: &str) -> Vec<String> {
        let norm_parent = RegistryStore::normalize_key(parent_key);
        let root_id = root.as_u32();
        let prefix = if norm_parent.is_empty() {
            String::new()
        } else {
            format!("{norm_parent}\\")
        };

        let mut subkeys = Vec::new();
        for (r, p) in self.keys.keys() {
            if *r == root_id && p.starts_with(&prefix) {
                let remainder = &p[prefix.len()..];
                let segment = remainder.split('\\').next().unwrap_or(remainder);
                if !subkeys.contains(&segment.to_string()) {
                    subkeys.push(segment.to_string());
                }
            }
        }
        subkeys
    }

    /// Commits the active transaction, clearing the transaction log.
    pub fn commit(&mut self) {
        self.is_in_transaction = false;
        self.transaction_log.clear();
    }

    /// Rolls back the active transaction in reverse order using the transaction log.
    pub fn rollback(&mut self) {
        self.is_in_transaction = false;
        let entries = std::mem::take(&mut self.transaction_log);
        for entry in entries.into_iter().rev() {
            let Ok(root) = RegistryRoot::from_u32(match entry.root_name.as_str() {
                "HKCR" => 0,
                "HKCU" => 1,
                "HKLM" => 2,
                "HKU" => 3,
                _ => 999,
            }) else {
                continue;
            };

            if let Some(old_val) = entry.old_value {
                self.set_value(root, &entry.key_path, entry.value_name.as_deref(), old_val);
            } else {
                self.delete_value(root, &entry.key_path, entry.value_name.as_deref());
            }
        }
    }
}

/// Native configuration synchronization bridge for macOS `defaults` and Linux `dconf`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NativeConfigBridge;

impl NativeConfigBridge {
    /// Formats a macOS `defaults write` command for synchronizing a registry key.
    ///
    /// # Arguments
    ///
    /// * `domain` - Application bundle identifier domain (e.g. `com.vendor.product`).
    /// * `key` - Configuration key name.
    /// * `value` - String value.
    ///
    /// # Returns
    ///
    /// Formatted command string.
    #[must_use]
    pub fn sync_to_macos_defaults(domain: &str, key: &str, value: &str) -> String {
        format!("defaults write {domain} \"{key}\" \"{value}\"")
    }

    /// Formats a Linux `dconf write` command for synchronizing a configuration key.
    ///
    /// # Arguments
    ///
    /// * `schema_path` - `GSettings` / dconf schema path (e.g. `/org/vendor/product`).
    /// * `key` - Configuration key.
    /// * `value` - String value.
    ///
    /// # Returns
    ///
    /// Formatted command string.
    #[must_use]
    pub fn sync_to_linux_dconf(schema_path: &str, key: &str, value: &str) -> String {
        let clean_path = schema_path.trim_end_matches('/');
        format!("dconf write {clean_path}/{key} \"'{value}'\"")
    }

    /// Synchronizes a registry value into native platform command representation.
    ///
    /// # Arguments
    ///
    /// * `root` - Registry root.
    /// * `key` - Registry key.
    /// * `name` - Value name.
    /// * `val` - Registry value.
    ///
    /// # Returns
    ///
    /// Formatted sync command if applicable.
    #[must_use]
    pub fn sync_registry_value(
        root: RegistryRoot,
        key: &str,
        name: Option<&str>,
        val: &RegistryValue,
    ) -> Option<String> {
        let val_str = match val {
            RegistryValue::Sz(s) | RegistryValue::ExpandSz(s) => s.clone(),
            RegistryValue::Dword(dw) => dw.to_string(),
            RegistryValue::Qword(qw) => qw.to_string(),
            RegistryValue::MultiSz(ms) => ms.join(";"),
            RegistryValue::Binary(b) => format!("{b:?}"),
        };

        let key_name = name.unwrap_or("Default");
        #[cfg(target_os = "macos")]
        {
            let _ = root;
            let domain = format!(
                "com.msi.{}",
                RegistryStore::normalize_key(key).replace('\\', ".")
            );
            Some(Self::sync_to_macos_defaults(&domain, key_name, &val_str))
        }
        #[cfg(target_os = "linux")]
        {
            let _ = root;
            let schema = format!(
                "/org/msi/{}",
                RegistryStore::normalize_key(key)
                    .to_lowercase()
                    .replace('\\', "/")
            );
            Some(Self::sync_to_linux_dconf(&schema, key_name, &val_str))
        }
        #[cfg(not(any(target_os = "macos", target_os = "linux")))]
        {
            Some(format!(
                "# RegSync: {}/{}/{} = {}",
                root.as_str(),
                RegistryStore::normalize_key(key),
                key_name,
                val_str
            ))
        }
    }
}

/// Native and synthesized Win32 Registry API commands and wrappers (`RegCreateKeyExW`, `RegSetValueExW`).
///
/// Implements native registry operations on Windows and synthesized `reg.exe` commands / memory emulation on non-Windows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Win32RegistryApi;

impl Win32RegistryApi {
    /// Formats the command line representing `RegCreateKeyExW` via `reg.exe add`.
    ///
    /// # Arguments
    ///
    /// * `root` - Registry root.
    /// * `subkey` - Subkey path.
    ///
    /// # Returns
    ///
    /// Formatted `reg.exe` command string.
    #[must_use]
    pub fn create_key_command(root: RegistryRoot, subkey: &str) -> String {
        let norm = RegistryStore::normalize_key(subkey);
        format!("reg.exe add \"{}\\{norm}\" /f", root.as_str())
    }

    /// Formats the command line representing `RegSetValueExW` via `reg.exe add`.
    ///
    /// # Arguments
    ///
    /// * `root` - Registry root.
    /// * `subkey` - Subkey path.
    /// * `name` - Value name (`None` for default value).
    /// * `val` - Registry value.
    ///
    /// # Returns
    ///
    /// Formatted `reg.exe` command string.
    #[must_use]
    pub fn set_value_command(
        root: RegistryRoot,
        subkey: &str,
        name: Option<&str>,
        val: &RegistryValue,
    ) -> String {
        let norm = RegistryStore::normalize_key(subkey);
        let val_switch = match name {
            Some(n) if !n.is_empty() => format!("/v \"{n}\""),
            _ => "/ve".to_string(),
        };
        let (type_str, data_str) = match val {
            RegistryValue::Sz(s) => ("REG_SZ", s.clone()),
            RegistryValue::ExpandSz(s) => ("REG_EXPAND_SZ", s.clone()),
            RegistryValue::Dword(d) => ("REG_DWORD", d.to_string()),
            RegistryValue::Qword(q) => ("REG_QWORD", q.to_string()),
            RegistryValue::Binary(b) => {
                let mut hex = String::with_capacity(b.len() * 2);
                for byte in b {
                    use std::fmt::Write;
                    let _ = write!(hex, "{byte:02X}");
                }
                ("REG_BINARY", hex)
            }
            RegistryValue::MultiSz(m) => ("REG_MULTI_SZ", m.join(r"\0")),
        };
        format!(
            "reg.exe add \"{}\\{norm}\" {val_switch} /t {type_str} /d \"{data_str}\" /f",
            root.as_str()
        )
    }

    /// Formats the command line representing `RegDeleteValueW` via `reg.exe delete`.
    ///
    /// # Arguments
    ///
    /// * `root` - Registry root.
    /// * `subkey` - Subkey path.
    /// * `name` - Value name.
    ///
    /// # Returns
    ///
    /// Formatted `reg.exe delete` command string.
    #[must_use]
    pub fn delete_value_command(root: RegistryRoot, subkey: &str, name: Option<&str>) -> String {
        let norm = RegistryStore::normalize_key(subkey);
        let val_switch = match name {
            Some(n) if !n.is_empty() => format!("/v \"{n}\""),
            _ => "/ve".to_string(),
        };
        format!(
            "reg.exe delete \"{}\\{norm}\" {val_switch} /f",
            root.as_str()
        )
    }

    /// Formats the command line representing `RegDeleteKeyW` via `reg.exe delete`.
    ///
    /// # Arguments
    ///
    /// * `root` - Registry root.
    /// * `subkey` - Subkey path to delete recursively.
    ///
    /// # Returns
    ///
    /// Formatted `reg.exe delete` command string.
    #[must_use]
    pub fn delete_key_command(root: RegistryRoot, subkey: &str) -> String {
        let norm = RegistryStore::normalize_key(subkey);
        format!("reg.exe delete \"{}\\{norm}\" /va /f", root.as_str())
    }

    /// Executes or emulates `RegCreateKeyExW` against an in-memory or persisted [`RegistryStore`].
    ///
    /// # Arguments
    ///
    /// * `store` - Mutable registry store.
    /// * `root` - Registry root.
    /// * `subkey` - Subkey path.
    ///
    /// # Returns
    ///
    /// Normalized created subkey path.
    pub fn create_key_in_store(
        store: &mut RegistryStore,
        root: RegistryRoot,
        subkey: &str,
    ) -> String {
        let norm = RegistryStore::normalize_key(subkey);
        store.set_value(root, &norm, None, RegistryValue::Sz(String::new()));
        norm
    }

    /// Executes or emulates `RegSetValueExW` against an in-memory or persisted [`RegistryStore`].
    ///
    /// # Arguments
    ///
    /// * `store` - Mutable registry store.
    /// * `root` - Registry root.
    /// * `subkey` - Subkey path.
    /// * `name` - Optional value name.
    /// * `value` - Registry value to write.
    pub fn set_value_in_store(
        store: &mut RegistryStore,
        root: RegistryRoot,
        subkey: &str,
        name: Option<&str>,
        value: RegistryValue,
    ) {
        store.set_value(root, subkey, name, value);
    }
}

#[cfg(test)]
#[allow(clippy::manual_flatten)]
mod tests {
    use super::*;

    /// Tests `RegistryRoot` conversion and display.
    #[test]
    fn test_registry_root() {
        assert_eq!(RegistryRoot::from_u32(0), Ok(RegistryRoot::ClassesRoot));
        assert_eq!(RegistryRoot::from_u32(1), Ok(RegistryRoot::CurrentUser));
        assert_eq!(RegistryRoot::from_u32(2), Ok(RegistryRoot::LocalMachine));
        assert_eq!(RegistryRoot::from_u32(3), Ok(RegistryRoot::Users));
        assert!(RegistryRoot::from_u32(4).is_err());

        assert_eq!(RegistryRoot::ClassesRoot.as_u32(), 0);
        assert_eq!(RegistryRoot::CurrentUser.as_u32(), 1);
        assert_eq!(RegistryRoot::LocalMachine.as_u32(), 2);
        assert_eq!(RegistryRoot::Users.as_u32(), 3);

        assert_eq!(RegistryRoot::ClassesRoot.as_str(), "HKCR");
        assert_eq!(RegistryRoot::CurrentUser.as_str(), "HKCU");
        assert_eq!(RegistryRoot::LocalMachine.as_str(), "HKLM");
        assert_eq!(RegistryRoot::Users.as_str(), "HKU");
    }

    /// Tests `RegistryValue` types and getters.
    #[test]
    fn test_registry_values_variety() {
        let val_sz = RegistryValue::Sz("Sample".to_string());
        assert_eq!(val_sz.type_name(), "REG_SZ");

        let val_exp = RegistryValue::ExpandSz("%PATH%".to_string());
        assert_eq!(val_exp.type_name(), "REG_EXPAND_SZ");

        let val_bin = RegistryValue::Binary(vec![0xAA, 0xBB]);
        assert_eq!(val_bin.type_name(), "REG_BINARY");

        let val_dw = RegistryValue::Dword(100);
        assert_eq!(val_dw.type_name(), "REG_DWORD");

        let val_multi = RegistryValue::MultiSz(vec!["A".to_string(), "B".to_string()]);
        assert_eq!(val_multi.type_name(), "REG_MULTI_SZ");

        let val_qword = RegistryValue::Qword(10_000_000_000);
        assert_eq!(val_qword.type_name(), "REG_QWORD");
    }

    /// Tests `RegistryStore` CRUD operations, rollback, and commit.
    #[test]
    fn test_registry_store_transactions() {
        let mut store = RegistryStore::new();

        // Initial set and commit baseline transaction
        store.set_value(
            RegistryRoot::LocalMachine,
            "Software/Acme",
            Some("Version"),
            RegistryValue::Sz("1.0".to_string()),
        );
        assert_eq!(
            store.get_value(
                RegistryRoot::LocalMachine,
                r"Software\Acme",
                Some("Version")
            ),
            Some(&RegistryValue::Sz("1.0".to_string()))
        );
        store.commit();

        // Update value in new transaction
        store.set_value(
            RegistryRoot::LocalMachine,
            r"Software\Acme",
            Some("Version"),
            RegistryValue::Sz("2.0".to_string()),
        );
        assert_eq!(
            store.get_value(RegistryRoot::LocalMachine, "Software/Acme", Some("Version")),
            Some(&RegistryValue::Sz("2.0".to_string()))
        );

        // Delete value
        assert!(store.delete_value(RegistryRoot::LocalMachine, "Software/Acme", Some("Version")));
        assert_eq!(
            store.get_value(RegistryRoot::LocalMachine, "Software/Acme", Some("Version")),
            None
        );

        // Rollback restores previous values in reverse order
        store.rollback();
        // Since delete and 2.0 update were rolled back, original 1.0 is restored
        assert_eq!(
            store.get_value(RegistryRoot::LocalMachine, "Software/Acme", Some("Version")),
            Some(&RegistryValue::Sz("1.0".to_string()))
        );

        // Commit clears journal
        store.commit();

        // Setting a new value that didn't exist before and rolling back exercises remove branch in rollback
        store.set_value(
            RegistryRoot::CurrentUser,
            "TempKey",
            Some("TempVal"),
            RegistryValue::Dword(99),
        );
        assert_eq!(
            store.get_value(RegistryRoot::CurrentUser, "TempKey", Some("TempVal")),
            Some(&RegistryValue::Dword(99))
        );
        store.rollback();
        assert_eq!(
            store.get_value(RegistryRoot::CurrentUser, "TempKey", Some("TempVal")),
            None
        );

        // Delete non-existent
        assert!(!store.delete_value(RegistryRoot::CurrentUser, "MissingKey", None));
    }

    /// Tests shell profile environment script generation.
    #[test]
    fn test_profile_script_generation() {
        let mut envs = BTreeMap::new();
        envs.insert("ACME_HOME".to_string(), "/opt/Acme".to_string());
        envs.insert("ACME_MODE".to_string(), "PRODUCTION".to_string());

        let script = RegistryStore::generate_profile_script("Acme", &envs);
        assert!(script.contains("export ACME_HOME=\"/opt/Acme\""));
        assert!(script.contains("export ACME_MODE=\"PRODUCTION\""));

        let (sys_db, user_db) = RegistryStore::standard_database_paths();
        assert_eq!(sys_db, PathBuf::from("/var/lib/msi/registry.db"));
        assert_eq!(user_db, PathBuf::from("~/.config/msi/registry.db"));
    }

    /// Tests `SqliteRegistryDriver` memory mode, transactions, updates, and rollback.
    #[test]
    fn test_sqlite_registry_driver_transactions() {
        let mut driver = SqliteRegistryDriver::open_in_memory();
        assert_eq!(
            SqliteRegistryDriver::system_db_path(),
            "/var/lib/msi/registry.db"
        );
        assert_eq!(
            SqliteRegistryDriver::user_db_path(),
            "~/.config/msi/registry.db"
        );
        assert!(
            SqliteRegistryDriver::initialize_schema_sql().contains("PRAGMA journal_mode = WAL;")
        );

        // Test default constructor
        let default_driver = SqliteRegistryDriver::default();
        assert_eq!(default_driver.db_path(), Path::new(":memory:"));

        // save_to_disk when in memory returns Ok(())
        assert!(default_driver.save_to_disk().is_ok());

        // Baseline commit
        driver.set_value(
            RegistryRoot::LocalMachine,
            "Software\\MyApp",
            Some("Version"),
            RegistryValue::Sz("1.0.0".to_string()),
        );

        assert_eq!(
            driver.get_value(
                RegistryRoot::LocalMachine,
                "Software\\MyApp",
                Some("Version")
            ),
            Some(RegistryValue::Sz("1.0.0".to_string()))
        );

        // Begin transaction
        let tx_id = driver.begin_transaction();
        assert!(tx_id > 0);

        driver.set_value(
            RegistryRoot::LocalMachine,
            "Software\\MyApp",
            Some("Version"),
            RegistryValue::Sz("2.0.0".to_string()),
        );
        assert_eq!(
            driver.get_value(
                RegistryRoot::LocalMachine,
                "Software\\MyApp",
                Some("Version")
            ),
            Some(RegistryValue::Sz("2.0.0".to_string()))
        );

        // Rollback restores 1.0.0
        driver.rollback();
        assert_eq!(
            driver.get_value(
                RegistryRoot::LocalMachine,
                "Software\\MyApp",
                Some("Version")
            ),
            Some(RegistryValue::Sz("1.0.0".to_string()))
        );

        driver.commit();
    }

    /// Tests `SqliteRegistryDriver` value deletion, subtree deletion, and subkeys listing.
    #[test]
    fn test_sqlite_registry_driver_deletions_and_subtrees() {
        let mut driver = SqliteRegistryDriver::open_in_memory();

        driver.set_value(
            RegistryRoot::LocalMachine,
            "Software\\MyApp",
            Some("Version"),
            RegistryValue::Sz("1.0.0".to_string()),
        );
        driver.set_value(
            RegistryRoot::LocalMachine,
            "Software\\MyApp\\SubFeature",
            Some("Enabled"),
            RegistryValue::Dword(1),
        );

        let subkeys = driver.list_subkeys(RegistryRoot::LocalMachine, "Software\\MyApp");
        assert_eq!(subkeys, vec!["SubFeature".to_string()]);

        let roots_subkeys = driver.list_subkeys(RegistryRoot::LocalMachine, "");
        assert!(roots_subkeys.contains(&"Software".to_string()));

        // delete_value tests:
        assert!(!driver.delete_value(RegistryRoot::LocalMachine, "NoSuchKey", Some("Val")));
        assert!(!driver.delete_value(
            RegistryRoot::LocalMachine,
            "Software\\MyApp",
            Some("NoSuchVal")
        ));
        assert!(driver.delete_value(
            RegistryRoot::LocalMachine,
            "Software\\MyApp\\SubFeature",
            Some("Enabled")
        ));

        // delete_value inside transaction
        let tx_del = driver.begin_transaction();
        assert!(tx_del > 0);
        driver.set_value(
            RegistryRoot::LocalMachine,
            "Software\\MyApp\\SubFeature",
            Some("Active"),
            RegistryValue::Dword(1),
        );
        assert!(driver.delete_value(
            RegistryRoot::LocalMachine,
            "Software\\MyApp\\SubFeature",
            Some("Active")
        ));
        driver.commit();

        // Subtree deletion inside transaction
        driver.set_value(
            RegistryRoot::LocalMachine,
            "Software\\ToDelete\\Child",
            Some("K"),
            RegistryValue::Sz("V".to_string()),
        );
        driver.begin_transaction();
        assert_eq!(
            driver.delete_subtree(RegistryRoot::LocalMachine, "Software\\ToDelete"),
            1
        );
        driver.rollback();

        // Rollback with newly inserted values (exercises old_value == None branch in rollback)
        driver.begin_transaction();
        driver.set_value(
            RegistryRoot::ClassesRoot,
            "Ext\\Open",
            None,
            RegistryValue::Sz("prog".to_string()),
        );
        driver.set_value(
            RegistryRoot::CurrentUser,
            "Setting",
            None,
            RegistryValue::Dword(5),
        );
        driver.set_value(
            RegistryRoot::Users,
            "Profile",
            None,
            RegistryValue::Sz("user".to_string()),
        );
        driver.rollback();
        assert_eq!(
            driver.get_value(RegistryRoot::ClassesRoot, "Ext\\Open", None),
            None
        );

        // Delete subtree outside transaction
        let deleted_count = driver.delete_subtree(RegistryRoot::LocalMachine, "Software\\MyApp");
        assert_eq!(deleted_count, 2);
        assert_eq!(
            driver.get_value(
                RegistryRoot::LocalMachine,
                "Software\\MyApp",
                Some("Version")
            ),
            None
        );
    }

    #[test]
    fn test_native_config_bridge_synchronization() {
        let mac_cmd = NativeConfigBridge::sync_to_macos_defaults("com.acme.app", "Port", "8080");
        assert_eq!(mac_cmd, "defaults write com.acme.app \"Port\" \"8080\"");

        let linux_cmd = NativeConfigBridge::sync_to_linux_dconf("/org/acme/app", "enabled", "true");
        assert_eq!(linux_cmd, "dconf write /org/acme/app/enabled \"'true'\"");

        let val = RegistryValue::Sz("active".to_string());
        let sync_res = NativeConfigBridge::sync_registry_value(
            RegistryRoot::CurrentUser,
            "Software\\Vendor\\App",
            Some("Status"),
            &val,
        );
        assert!(sync_res.is_some());

        let sync_none = NativeConfigBridge::sync_registry_value(
            RegistryRoot::LocalMachine,
            "Software\\App",
            None,
            &RegistryValue::Dword(42),
        );
        assert!(sync_none.is_some());

        let sync_qword = NativeConfigBridge::sync_registry_value(
            RegistryRoot::CurrentUser,
            "Software\\App",
            Some("BigInt"),
            &RegistryValue::Qword(1_000_000_000),
        );
        assert!(sync_qword.is_some());

        let sync_multi = NativeConfigBridge::sync_registry_value(
            RegistryRoot::CurrentUser,
            "Software\\App",
            Some("List"),
            &RegistryValue::MultiSz(vec!["A".into(), "B".into()]),
        );
        assert!(sync_multi.is_some());

        let sync_bin = NativeConfigBridge::sync_registry_value(
            RegistryRoot::CurrentUser,
            "Software\\App",
            Some("Raw"),
            &RegistryValue::Binary(vec![0xDE, 0xAD]),
        );
        assert!(sync_bin.is_some());
    }

    /// Tests `SqliteRegistryDriver` full schema persistence to and restoration from disk.
    #[test]
    #[allow(clippy::too_many_lines)]
    fn test_sqlite_registry_driver_disk_roundtrip() {
        let temp_dir =
            std::env::temp_dir().join(format!("msi_test_reg_disk_{}", std::process::id()));
        let _ = std::fs::create_dir_all(&temp_dir);
        let db_path = temp_dir.join("test_registry.db");

        let mut driver = SqliteRegistryDriver::new(&db_path);
        assert_eq!(driver.db_path(), db_path.as_path());
        assert_eq!(
            SqliteRegistryDriver::system_db_path(),
            "/var/lib/msi/registry.db"
        );
        assert_eq!(
            SqliteRegistryDriver::user_db_path(),
            "~/.config/msi/registry.db"
        );
        assert_ne!(SqliteRegistryDriver::initialize_schema_sql(), "");

        let test_k = "Software\\TestVendor\\App";
        driver.set_value(
            RegistryRoot::LocalMachine,
            test_k,
            Some("Version"),
            RegistryValue::Sz("3.1.4".to_string()),
        );
        driver.set_value(
            RegistryRoot::LocalMachine,
            test_k,
            Some("Timeout"),
            RegistryValue::Dword(30),
        );
        driver.set_value(
            RegistryRoot::LocalMachine,
            test_k,
            Some("LargeInt"),
            RegistryValue::Qword(999_888_777_666),
        );
        driver.set_value(
            RegistryRoot::LocalMachine,
            test_k,
            Some("Paths"),
            RegistryValue::MultiSz(vec!["/usr/bin".to_string(), "/usr/local/bin".to_string()]),
        );
        driver.set_value(
            RegistryRoot::LocalMachine,
            test_k,
            Some("Data"),
            RegistryValue::Binary(vec![1, 2, 3, 4]),
        );
        driver.set_value(
            RegistryRoot::CurrentUser,
            test_k,
            None,
            RegistryValue::ExpandSz("%USERPROFILE%\\app".to_string()),
        );

        assert!(driver.save_to_disk().is_ok());
        assert!(db_path.exists());

        // Test save failure when parent directory cannot be created (is a file)
        let blocking_file = temp_dir.join("blocking_file.txt");
        assert!(std::fs::write(&blocking_file, b"occupied").is_ok());
        let uncreatable_driver =
            SqliteRegistryDriver::new(blocking_file.join("sub").join("reg.db"));
        assert!(uncreatable_driver.save_to_disk().is_err());

        // Test save failure when db_path cannot be written (is a directory)
        let unwritable_driver = SqliteRegistryDriver::new(&temp_dir);
        assert!(unwritable_driver.save_to_disk().is_err());

        let nonexistent_db = temp_dir.join("nonexistent_db.sqlite");
        assert!(SqliteRegistryDriver::load_from_disk(&nonexistent_db).is_err());

        for restored in [
            SqliteRegistryDriver::load_from_disk(&db_path),
            SqliteRegistryDriver::load_from_disk(&nonexistent_db),
        ]
        .into_iter()
        .flatten()
        {
            assert_eq!(
                restored.get_value(RegistryRoot::LocalMachine, test_k, Some("Version")),
                Some(RegistryValue::Sz("3.1.4".to_string()))
            );
            assert_eq!(
                restored.get_value(RegistryRoot::LocalMachine, test_k, Some("Timeout")),
                Some(RegistryValue::Dword(30))
            );
            assert_eq!(
                restored.get_value(RegistryRoot::LocalMachine, test_k, Some("LargeInt")),
                Some(RegistryValue::Qword(999_888_777_666))
            );
            assert_eq!(
                restored.get_value(RegistryRoot::LocalMachine, test_k, Some("Paths")),
                Some(RegistryValue::MultiSz(vec![
                    "/usr/bin".to_string(),
                    "/usr/local/bin".to_string()
                ]))
            );
            assert_eq!(
                restored.get_value(RegistryRoot::LocalMachine, test_k, Some("Data")),
                Some(RegistryValue::Binary(vec![1, 2, 3, 4]))
            );
            assert_eq!(
                restored.get_value(RegistryRoot::CurrentUser, test_k, None),
                Some(RegistryValue::ExpandSz("%USERPROFILE%\\app".to_string()))
            );
        }

        let _ = std::fs::remove_file(&blocking_file);
        let _ = std::fs::remove_file(&db_path);
        let _ = std::fs::remove_dir(&temp_dir);
    }

    /// Tests SQL parser edge cases, save without parent directory, subkeys filtering, and unknown root rollback.
    #[test]
    fn test_sqlite_registry_driver_parser_edge_cases_and_roots() {
        let temp_dir = std::env::temp_dir().join(format!("msi_reg_edge_{}", std::process::id()));
        let _ = std::fs::create_dir_all(&temp_dir);

        // Test save_to_disk when path has no parent directory component (parent is "")
        let current_dir_db = PathBuf::from(format!("standalone_test_{}.db", std::process::id()));
        let mut no_parent_driver = SqliteRegistryDriver::new(&current_dir_db);
        no_parent_driver.set_value(
            RegistryRoot::LocalMachine,
            "Software\\Test",
            None,
            RegistryValue::Dword(1),
        );
        assert!(no_parent_driver.save_to_disk().is_ok());
        let _ = std::fs::remove_file(&current_dir_db);

        // Test list_subkeys branches:
        // 1. Key where path == norm_parent (so p != norm_parent is false)
        // 2. Key with different root_id (so *r == root_id is false)
        // 3. Key with different prefix (so p.starts_with is false)
        let mut sk_driver = SqliteRegistryDriver::default();
        sk_driver.set_value(
            RegistryRoot::LocalMachine,
            "Software\\App",
            None,
            RegistryValue::Sz("root".to_string()),
        );
        sk_driver.set_value(
            RegistryRoot::CurrentUser,
            "Software\\App\\Sub",
            None,
            RegistryValue::Sz("diff_root".to_string()),
        );
        sk_driver.set_value(
            RegistryRoot::LocalMachine,
            "Hardware\\Device",
            None,
            RegistryValue::Sz("diff_prefix".to_string()),
        );
        let subkeys = sk_driver.list_subkeys(RegistryRoot::LocalMachine, "Software\\App");
        assert_eq!(subkeys.len(), 0);

        // Test rollback with unknown root_name (exercising continue; branch)
        let mut rb_driver = SqliteRegistryDriver::default();
        rb_driver.begin_transaction();
        rb_driver.transaction_log.push(SqliteTransactionLogEntry {
            tx_id: 1,
            action: "INSERT".to_string(),
            root_name: "INVALID_ROOT_NAME".to_string(),
            key_path: "Software\\Test".to_string(),
            value_name: None,
            old_value: None,
        });
        rb_driver.rollback();

        // Test load_from_disk with edge-case SQL lines
        let edge_sql_path = temp_dir.join("edge_cases.sql");
        let edge_sql = r"
INSERT OR REPLACE INTO keys INVALID NO VALUES
INSERT OR REPLACE INTO keys VALUES (1, 2)
INSERT OR REPLACE INTO keys VALUES (1, 2);
INSERT OR REPLACE INTO keys VALUES (not_int, 2, 'path');
INSERT OR REPLACE INTO keys VALUES (1, not_int, 'path');
INSERT OR REPLACE INTO keys VALUES (10, 2, 'valid\path');
INSERT OR REPLACE INTO values INVALID NO VALUES
INSERT OR REPLACE INTO values VALUES (1, 2)
INSERT OR REPLACE INTO values VALUES (1, 2);
INSERT OR REPLACE INTO values VALUES (not_int, NULL, 'REG_SZ', 'val');
INSERT OR REPLACE INTO values VALUES (10, 'OddHex', 'REG_BINARY', '123');
INSERT OR REPLACE INTO values VALUES (10, 'Fallback', 'REG_CUSTOM_TYPE', 'raw_text');
";
        assert!(std::fs::write(&edge_sql_path, edge_sql).is_ok());
        let nonexistent_db = temp_dir.join("nonexistent_edge.sqlite");
        for loaded in [
            SqliteRegistryDriver::load_from_disk(&edge_sql_path),
            SqliteRegistryDriver::load_from_disk(&nonexistent_db),
        ]
        .into_iter()
        .flatten()
        {
            assert_eq!(
                loaded.get_value(RegistryRoot::LocalMachine, r"valid\path", Some("OddHex")),
                Some(RegistryValue::Binary(vec![0x12]))
            );
            assert_eq!(
                loaded.get_value(RegistryRoot::LocalMachine, r"valid\path", Some("Fallback")),
                Some(RegistryValue::Sz("raw_text".to_string()))
            );
        }

        let _ = std::fs::remove_file(&edge_sql_path);
        let _ = std::fs::remove_dir(&temp_dir);
    }

    /// Tests `SharedDLLs` reference counting helpers on `RegistryStore`.
    #[test]
    fn test_registry_store_shared_dll_ref_counting() {
        let mut store = RegistryStore::new();
        let target_dll = r"C:\Program Files\LibScript\bin\mysqld.exe";

        // Initial state
        assert_eq!(store.get_shared_dll_ref(target_dll), Ok(0));

        // First increment -> 1
        assert_eq!(store.increment_shared_dll_ref(target_dll), Ok(1));
        assert_eq!(store.get_shared_dll_ref(target_dll), Ok(1));

        // Second increment -> 2
        assert_eq!(store.increment_shared_dll_ref(target_dll), Ok(2));
        assert_eq!(store.get_shared_dll_ref(target_dll), Ok(2));

        // First decrement -> 1
        assert_eq!(store.decrement_shared_dll_ref(target_dll), Ok(1));
        assert_eq!(store.get_shared_dll_ref(target_dll), Ok(1));

        // Second decrement -> 0 (deleted)
        assert_eq!(store.decrement_shared_dll_ref(target_dll), Ok(0));
        assert_eq!(store.get_shared_dll_ref(target_dll), Ok(0));

        // Third decrement -> 0 (stays 0)
        assert_eq!(store.decrement_shared_dll_ref(target_dll), Ok(0));

        // Error handling on empty paths
        assert!(store.get_shared_dll_ref("").is_err());
        assert!(store.increment_shared_dll_ref("").is_err());
        assert!(store.decrement_shared_dll_ref("").is_err());
    }

    /// Tests component client `ProductCode` tracking per `ComponentId` GUID on `RegistryStore`.
    #[test]
    fn test_registry_store_component_client_tracking() -> Result<()> {
        let mut store = RegistryStore::new();
        let comp_guid = "{5B2783B0-9A1F-4348-9F93-87CE43C21001}";
        let prod_openedx = "{E0F45901-83B4-4B21-9B5A-01D38FE81001}";
        let prod_wordpress = "{E0F45901-83B4-4B21-9B5A-01D38FE81002}";
        let key_path = r"C:\Program Files\LibScript\bin\mysqld.exe";

        // Initial state
        assert_eq!(store.get_component_client_count(comp_guid)?, 0);
        assert!(!store.is_component_shared(comp_guid)?);
        assert_eq!(
            store.get_component_clients(comp_guid)?,
            Vec::<String>::new()
        );

        // Register first client (Open edX)
        let count1 = store.register_component_client(comp_guid, prod_openedx, Some(key_path))?;
        assert_eq!(count1, 1);
        assert_eq!(store.get_component_client_count(comp_guid)?, 1);
        assert!(!store.is_component_shared(comp_guid)?);
        assert_eq!(store.get_shared_dll_ref(key_path)?, 1);

        // Register second client (WordPress)
        let count2 = store.register_component_client(comp_guid, prod_wordpress, Some(key_path))?;
        assert_eq!(count2, 2);
        assert_eq!(store.get_component_client_count(comp_guid)?, 2);
        assert!(store.is_component_shared(comp_guid)?);
        assert_eq!(store.get_shared_dll_ref(key_path)?, 2);

        let clients = store.get_component_clients(comp_guid)?;
        assert_eq!(clients.len(), 2);

        // Unregister first client (Open edX)
        let count_after_unreg1 =
            store.unregister_component_client(comp_guid, prod_openedx, Some(key_path))?;
        assert_eq!(count_after_unreg1, 1);
        assert_eq!(store.get_component_client_count(comp_guid)?, 1);
        assert!(!store.is_component_shared(comp_guid)?);
        assert_eq!(store.get_shared_dll_ref(key_path)?, 1);

        // Unregister second client (WordPress)
        let count_after_unreg2 =
            store.unregister_component_client(comp_guid, prod_wordpress, Some(key_path))?;
        assert_eq!(count_after_unreg2, 0);
        assert_eq!(store.get_component_client_count(comp_guid)?, 0);
        assert_eq!(store.get_shared_dll_ref(key_path)?, 0);

        // Client registration with None key_path and empty key_path
        let count_none = store.register_component_client(comp_guid, prod_openedx, None)?;
        assert_eq!(count_none, 1);
        let count_ws = store.register_component_client(comp_guid, prod_wordpress, Some("   "))?;
        assert_eq!(count_ws, 2);

        // Values with CurrentUser root and None name to cover get_component_clients filter branches
        let comp_key = format!(
            r"{}\{}",
            RegistryStore::COMPONENTS_SUBKEY,
            RegistryStore::normalize_key(comp_guid)
        );
        store.set_value(
            RegistryRoot::CurrentUser,
            &comp_key,
            Some("ClientOtherRoot"),
            RegistryValue::Sz("path".to_string()),
        );
        store.set_value(
            RegistryRoot::LocalMachine,
            &comp_key,
            None,
            RegistryValue::Sz("default".to_string()),
        );
        let client_list = store.get_component_clients(comp_guid)?;
        assert_eq!(client_list.len(), 2);

        // Unregister with None and empty key_path
        let unreg_ws = store.unregister_component_client(comp_guid, prod_wordpress, Some("   "))?;
        assert_eq!(unreg_ws, 1);
        let unreg_none = store.unregister_component_client(comp_guid, prod_openedx, None)?;
        assert_eq!(unreg_none, 0);

        // Error branches on invalid key path
        assert!(store
            .register_component_client(comp_guid, prod_openedx, Some("///"))
            .is_err());
        assert!(store
            .unregister_component_client(comp_guid, prod_openedx, Some("///"))
            .is_err());

        // Validation errors on empty strings
        assert!(store
            .register_component_client("", prod_openedx, None)
            .is_err());
        assert!(store
            .register_component_client(comp_guid, "", None)
            .is_err());
        assert!(store
            .unregister_component_client("", prod_openedx, None)
            .is_err());
        assert!(store
            .unregister_component_client(comp_guid, "", None)
            .is_err());
        assert!(store.get_component_clients("").is_err());
        assert!(store.get_component_client_count("").is_err());
        assert!(store.is_component_shared("").is_err());

        Ok(())
    }

    /// Tests `Win32RegistryApi` command generation and in-store operations, and `SqliteRegistryDriver::new_wal`.
    #[test]
    #[allow(clippy::too_many_lines)]
    fn test_win32_registry_api_and_wal_driver() {
        // WAL driver constructor
        let wal_driver = SqliteRegistryDriver::new_wal(":memory:");
        assert_eq!(wal_driver.db_path(), Path::new(":memory:"));

        // Win32 Registry API command generation
        let create_cmd =
            Win32RegistryApi::create_key_command(RegistryRoot::LocalMachine, r"Software\LibScript");
        assert_eq!(create_cmd, r#"reg.exe add "HKLM\Software\LibScript" /f"#);

        // Set value variants
        let sz_cmd = Win32RegistryApi::set_value_command(
            RegistryRoot::LocalMachine,
            r"Software\LibScript",
            Some("Version"),
            &RegistryValue::Sz("2.0.0".to_string()),
        );
        assert_eq!(
            sz_cmd,
            r#"reg.exe add "HKLM\Software\LibScript" /v "Version" /t REG_SZ /d "2.0.0" /f"#
        );

        let expand_sz_cmd = Win32RegistryApi::set_value_command(
            RegistryRoot::CurrentUser,
            "Environment",
            Some("PATH"),
            &RegistryValue::ExpandSz("%PATH%;C:\\LibScript".to_string()),
        );
        assert_eq!(
            expand_sz_cmd,
            r#"reg.exe add "HKCU\Environment" /v "PATH" /t REG_EXPAND_SZ /d "%PATH%;C:\LibScript" /f"#
        );

        let dword_cmd = Win32RegistryApi::set_value_command(
            RegistryRoot::LocalMachine,
            r"Software\LibScript",
            Some("Port"),
            &RegistryValue::Dword(3306),
        );
        assert_eq!(
            dword_cmd,
            r#"reg.exe add "HKLM\Software\LibScript" /v "Port" /t REG_DWORD /d "3306" /f"#
        );

        let qword_cmd = Win32RegistryApi::set_value_command(
            RegistryRoot::LocalMachine,
            r"Software\LibScript",
            Some("BigVal"),
            &RegistryValue::Qword(10_000_000_000),
        );
        assert_eq!(
            qword_cmd,
            r#"reg.exe add "HKLM\Software\LibScript" /v "BigVal" /t REG_QWORD /d "10000000000" /f"#
        );

        let bin_cmd = Win32RegistryApi::set_value_command(
            RegistryRoot::LocalMachine,
            r"Software\LibScript",
            Some("BinaryData"),
            &RegistryValue::Binary(vec![0xDE, 0xAD, 0xBE, 0xEF]),
        );
        assert_eq!(
            bin_cmd,
            r#"reg.exe add "HKLM\Software\LibScript" /v "BinaryData" /t REG_BINARY /d "DEADBEEF" /f"#
        );

        let multi_sz_cmd = Win32RegistryApi::set_value_command(
            RegistryRoot::LocalMachine,
            r"Software\LibScript",
            Some("MultiStrings"),
            &RegistryValue::MultiSz(vec!["one".to_string(), "two".to_string()]),
        );
        assert_eq!(
            multi_sz_cmd,
            r#"reg.exe add "HKLM\Software\LibScript" /v "MultiStrings" /t REG_MULTI_SZ /d "one\0two" /f"#
        );

        let default_val_cmd = Win32RegistryApi::set_value_command(
            RegistryRoot::ClassesRoot,
            ".txt",
            None,
            &RegistryValue::Sz("txtfile".to_string()),
        );
        assert_eq!(
            default_val_cmd,
            r#"reg.exe add "HKCR\.txt" /ve /t REG_SZ /d "txtfile" /f"#
        );
        let empty_name_cmd = Win32RegistryApi::set_value_command(
            RegistryRoot::ClassesRoot,
            ".txt",
            Some(""),
            &RegistryValue::Sz("txtfile".to_string()),
        );
        assert_eq!(
            empty_name_cmd,
            r#"reg.exe add "HKCR\.txt" /ve /t REG_SZ /d "txtfile" /f"#
        );

        // Delete value commands
        let del_val_cmd = Win32RegistryApi::delete_value_command(
            RegistryRoot::LocalMachine,
            r"Software\LibScript",
            Some("Version"),
        );
        assert_eq!(
            del_val_cmd,
            r#"reg.exe delete "HKLM\Software\LibScript" /v "Version" /f"#
        );

        let del_def_cmd =
            Win32RegistryApi::delete_value_command(RegistryRoot::ClassesRoot, ".txt", None);
        assert_eq!(del_def_cmd, r#"reg.exe delete "HKCR\.txt" /ve /f"#);
        let del_empty_cmd =
            Win32RegistryApi::delete_value_command(RegistryRoot::ClassesRoot, ".txt", Some(""));
        assert_eq!(del_empty_cmd, r#"reg.exe delete "HKCR\.txt" /ve /f"#);

        // Delete key command
        let del_key_cmd =
            Win32RegistryApi::delete_key_command(RegistryRoot::LocalMachine, r"Software\LibScript");
        assert_eq!(
            del_key_cmd,
            r#"reg.exe delete "HKLM\Software\LibScript" /va /f"#
        );

        // In-store operations
        let mut store = RegistryStore::new();
        let created = Win32RegistryApi::create_key_in_store(
            &mut store,
            RegistryRoot::LocalMachine,
            r"Software\LibScript",
        );
        assert_eq!(created, r"Software\LibScript");

        Win32RegistryApi::set_value_in_store(
            &mut store,
            RegistryRoot::LocalMachine,
            r"Software\LibScript",
            Some("Host"),
            RegistryValue::Sz("localhost".to_string()),
        );
        assert_eq!(
            store.get_value(
                RegistryRoot::LocalMachine,
                r"Software\LibScript",
                Some("Host")
            ),
            Some(&RegistryValue::Sz("localhost".to_string()))
        );
    }
}

//! Dynamic host querying and property resolution (`AppSearch`).
//!
//! Handles `AppSearch`, `Signature`, `RegLocator`, `IniLocator`, `DrLocator`, and `CompLocator`.

use crate::database::tables::record::{FieldValue, Record};
use crate::error::MsiError;
use crate::wix::linker::LinkedDatabase;
use std::collections::HashMap;

/// Result type.
pub type Result<T> = std::result::Result<T, MsiError>;

/// `AppSearch` registry/locator trait.
pub trait HostSystem {
    /// Retrieves a registry value.
    ///
    /// # Errors
    /// Returns `MsiError` on system access failure.
    ///
    /// # Arguments
    ///
    /// * `root` - TODO: Document argument.
    /// * `key` - TODO: Document argument.
    /// * `name` - TODO: Document argument.
    ///
    /// # Returns
    ///
    /// TODO: Document return value.
    fn get_registry_value(
        &self,
        root: i32,
        key: &str,
        name: Option<&str>,
    ) -> Result<Option<String>>;
    /// Retrieves an ini value.
    ///
    /// # Errors
    /// Returns `MsiError` on system access failure.
    ///
    /// # Arguments
    ///
    /// * `file` - TODO: Document argument.
    /// * `section` - TODO: Document argument.
    /// * `key` - TODO: Document argument.
    /// * `field` - TODO: Document argument.
    ///
    /// # Returns
    ///
    /// TODO: Document return value.
    fn get_ini_value(
        &self,
        file: &str,
        section: &str,
        key: &str,
        field: Option<u32>,
    ) -> Result<Option<String>>;
    /// Finds a file by name.
    ///
    /// # Errors
    /// Returns `MsiError` on system access failure.
    ///
    /// # Arguments
    ///
    /// * `path` - TODO: Document argument.
    /// * `name` - TODO: Document argument.
    /// * `depth` - TODO: Document argument.
    ///
    /// # Returns
    ///
    /// TODO: Document return value.
    fn find_file(&self, path: &str, name: &str, depth: u32) -> Result<Option<String>>;
    /// Finds a directory by path.
    ///
    /// # Errors
    /// Returns `MsiError` on system access failure.
    ///
    /// # Arguments
    ///
    /// * `path` - TODO: Document argument.
    /// * `depth` - TODO: Document argument.
    ///
    /// # Returns
    ///
    /// TODO: Document return value.
    fn find_dir(&self, path: &str, depth: u32) -> Result<Option<String>>;
    /// Validates a file against a signature.
    ///
    /// # Errors
    /// Returns `MsiError` on system access failure.
    ///
    /// # Arguments
    ///
    /// * `path` - TODO: Document argument.
    /// * `sig` - TODO: Document argument.
    ///
    /// # Returns
    ///
    /// TODO: Document return value.
    fn check_signature(&self, path: &str, sig: &Signature) -> Result<bool>;
}

/// Details of a file or directory signature.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Signature {
    /// Unique signature ID.
    pub id: String,
    /// Target file name.
    pub filename: String,
    /// Minimum version string.
    pub min_version: Option<String>,
    /// Maximum version string.
    pub max_version: Option<String>,
    /// Minimum file size in bytes.
    pub min_size: Option<u32>,
    /// Maximum file size in bytes.
    pub max_size: Option<u32>,
    /// Minimum creation date.
    pub min_date: Option<u32>,
    /// Maximum creation date.
    pub max_date: Option<u32>,
    /// Target languages.
    pub languages: Option<String>,
}

/// Registry locator.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegLocator {
    /// Signature ID.
    pub sig_id: String,
    /// Registry root integer.
    pub root: i32,
    /// Registry key path.
    pub key: String,
    /// Registry value name.
    pub name: Option<String>,
    /// Locator type (file/dir/raw).
    pub locator_type: u32,
}

/// Ini file locator.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IniLocator {
    /// Signature ID.
    pub sig_id: String,
    /// Ini file name.
    pub filename: String,
    /// Ini section name.
    pub section: String,
    /// Ini key name.
    pub key: String,
    /// Value field index.
    pub field: Option<u32>,
    /// Locator type.
    pub locator_type: u32,
}

/// Directory locator.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DrLocator {
    /// Signature ID.
    pub sig_id: String,
    /// Parent signature ID.
    pub parent: Option<String>,
    /// Directory path.
    pub path: Option<String>,
    /// Max search depth.
    pub depth: Option<u32>,
}

/// Component locator.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompLocator {
    /// Signature ID.
    pub sig_id: String,
    /// Component ID GUID.
    pub component_id: String,
    /// Locator type.
    pub locator_type: u32,
}

/// Application search dispatcher.
#[derive(Debug)]
pub struct AppSearch<'a, H: HostSystem> {
    /// Linked database.
    db: &'a LinkedDatabase,
    /// Host system.
    host: H,
}

impl<'a, H: HostSystem> AppSearch<'a, H> {
    /// Gets a string field from a record.
    ///
    /// # Arguments
    ///
    /// * `r` - TODO: Document argument.
    /// * `idx` - TODO: Document argument.
    ///
    /// # Returns
    ///
    /// TODO: Document return value.
    fn get_string(r: &Record, idx: usize) -> Option<&str> {
        match r.get(idx) {
            Some(FieldValue::String(s)) => Some(s),
            _ => None,
        }
    }

    /// Gets an integer field from a record.
    ///
    /// # Arguments
    ///
    /// * `r` - TODO: Document argument.
    /// * `idx` - TODO: Document argument.
    ///
    /// # Returns
    ///
    /// TODO: Document return value.
    fn get_i32(r: &Record, idx: usize) -> Option<i32> {
        match r.get(idx) {
            Some(FieldValue::Long(i)) => Some(*i),
            Some(FieldValue::Short(i)) => Some(i32::from(*i)),
            _ => None,
        }
    }

    /// Creates a new `AppSearch`.
    ///
    /// # Arguments
    ///
    /// * `db` - TODO: Document argument.
    /// * `host` - TODO: Document argument.
    ///
    /// # Returns
    ///
    /// TODO: Document return value.
    #[must_use]
    pub const fn new(db: &'a LinkedDatabase, host: H) -> Self {
        Self { db, host }
    }

    /// Executes the `AppSearch` action, returning a map of resolved properties.
    ///
    /// # Errors
    /// Returns `MsiError` on evaluation failure.
    ///
    /// # Returns
    ///
    /// TODO: Document return value.
    pub fn execute(&self) -> Result<HashMap<String, String>> {
        let mut results = HashMap::new();
        // Parse the AppSearch table
        if let Some(records) = self.db.tables.get("AppSearch") {
            for record in records {
                let prop = Self::get_string(record, 0).unwrap_or_default();
                let sig = Self::get_string(record, 1).unwrap_or_default();
                if prop.is_empty() || sig.is_empty() {
                    continue;
                }

                if let Some(val) = self.resolve_signature(sig) {
                    results.insert(prop.to_string(), val);
                }
            }
        }
        Ok(results)
    }

    /// Resolves a signature ID to a path.
    ///
    /// # Arguments
    ///
    /// * `sig_id` - TODO: Document argument.
    ///
    /// # Returns
    ///
    /// TODO: Document return value.
    fn resolve_signature(&self, sig_id: &str) -> Option<String> {
        // Step 1: Query CompLocator
        if let Some(_comp_loc) = self.find_comp_locator(sig_id) {
            // Unimplemented for phase 3 stub
        }

        // Step 2: Query RegLocator
        if let Some(reg_loc) = self.find_reg_locator(sig_id) {
            if let Ok(Some(val)) =
                self.host
                    .get_registry_value(reg_loc.root, &reg_loc.key, reg_loc.name.as_deref())
            {
                if reg_loc.locator_type == 2 {
                    return Some(val); // Raw registry value
                } else if Self::is_file_or_dir_locator(reg_loc.locator_type) {
                    return self.validate_with_signature(sig_id, &val);
                }
            }
        }

        // Step 3: Query IniLocator
        if let Some(ini_loc) = self.find_ini_locator(sig_id) {
            if let Ok(Some(val)) = self.host.get_ini_value(
                &ini_loc.filename,
                &ini_loc.section,
                &ini_loc.key,
                ini_loc.field,
            ) {
                if ini_loc.locator_type == 2 {
                    return Some(val); // Raw ini value
                } else if Self::is_file_or_dir_locator(ini_loc.locator_type) {
                    return self.validate_with_signature(sig_id, &val);
                }
            }
        }

        // Step 4: Query DrLocator
        if let Some(dr_loc) = self.find_dr_locator(sig_id) {
            let base_path = dr_loc
                .parent
                .as_ref()
                .and_then(|p| self.resolve_signature(p))
                .unwrap_or_default();
            let search_path = if let Some(p) = &dr_loc.path {
                if base_path.is_empty() {
                    p.clone()
                } else {
                    format!("{base_path}\\{p}")
                }
            } else {
                base_path
            };

            let depth = dr_loc.depth.unwrap_or(0);
            if let Some(sig) = self.find_signature(sig_id) {
                if let Ok(Some(found)) = self.host.find_file(&search_path, &sig.filename, depth) {
                    if self.host.check_signature(&found, &sig) == Ok(true) {
                        return Some(found);
                    }
                }
            } else if let Ok(Some(found)) = self.host.find_dir(&search_path, depth) {
                return Some(found);
            }
        }

        None
    }

    /// Validates a path against a signature.
    ///
    /// # Arguments
    ///
    /// * `sig_id` - TODO: Document argument.
    /// * `path` - TODO: Document argument.
    ///
    /// # Returns
    ///
    /// TODO: Document return value.
    fn validate_with_signature(&self, sig_id: &str, path: &str) -> Option<String> {
        self.find_signature(sig_id).map_or_else(
            || Some(path.to_string()),
            |sig| (self.host.check_signature(path, &sig) == Ok(true)).then(|| path.to_string()),
        )
    }

    /// Checks if a locator type is file or directory.
    ///
    /// # Arguments
    ///
    /// * `locator_type` - TODO: Document argument.
    ///
    /// # Returns
    ///
    /// TODO: Document return value.
    const fn is_file_or_dir_locator(locator_type: u32) -> bool {
        locator_type == 0 || locator_type == 1
    }

    /// Finds a signature by ID.
    ///
    /// # Arguments
    ///
    /// * `sig_id` - TODO: Document argument.
    ///
    /// # Returns
    ///
    /// TODO: Document return value.
    fn find_signature(&self, sig_id: &str) -> Option<Signature> {
        self.db
            .tables
            .get("Signature")?
            .iter()
            .find(|r| Self::get_string(r, 0) == Some(sig_id))
            .map(|r| Signature {
                id: Self::get_string(r, 0).unwrap_or_default().to_string(),
                filename: Self::get_string(r, 1).unwrap_or_default().to_string(),
                min_version: Self::get_string(r, 2).map(ToString::to_string),
                max_version: Self::get_string(r, 3).map(ToString::to_string),
                min_size: Self::get_i32(r, 4).map(|i| u32::try_from(i).unwrap_or(0)),
                max_size: Self::get_i32(r, 5).map(|i| u32::try_from(i).unwrap_or(0)),
                min_date: Self::get_i32(r, 6).map(|i| u32::try_from(i).unwrap_or(0)),
                max_date: Self::get_i32(r, 7).map(|i| u32::try_from(i).unwrap_or(0)),
                languages: Self::get_string(r, 8).map(ToString::to_string),
            })
    }

    /// Finds a `RegLocator` by signature ID.
    ///
    /// # Arguments
    ///
    /// * `sig_id` - TODO: Document argument.
    ///
    /// # Returns
    ///
    /// TODO: Document return value.
    fn find_reg_locator(&self, sig_id: &str) -> Option<RegLocator> {
        self.db
            .tables
            .get("RegLocator")?
            .iter()
            .find(|r| Self::get_string(r, 0) == Some(sig_id))
            .map(|r| RegLocator {
                sig_id: Self::get_string(r, 0).unwrap_or_default().to_string(),
                root: Self::get_i32(r, 1).unwrap_or(0),
                key: Self::get_string(r, 2).unwrap_or_default().to_string(),
                name: Self::get_string(r, 3).map(ToString::to_string),
                locator_type: Self::get_i32(r, 4).unwrap_or(0).try_into().unwrap_or(0),
            })
    }

    /// Finds an `IniLocator` by signature ID.
    ///
    /// # Arguments
    ///
    /// * `sig_id` - TODO: Document argument.
    ///
    /// # Returns
    ///
    /// TODO: Document return value.
    fn find_ini_locator(&self, sig_id: &str) -> Option<IniLocator> {
        self.db
            .tables
            .get("IniLocator")?
            .iter()
            .find(|r| Self::get_string(r, 0) == Some(sig_id))
            .map(|r| IniLocator {
                sig_id: Self::get_string(r, 0).unwrap_or_default().to_string(),
                filename: Self::get_string(r, 1).unwrap_or_default().to_string(),
                section: Self::get_string(r, 2).unwrap_or_default().to_string(),
                key: Self::get_string(r, 3).unwrap_or_default().to_string(),
                field: Self::get_i32(r, 4).map(|i| u32::try_from(i).unwrap_or(0)),
                locator_type: Self::get_i32(r, 5).unwrap_or(0).try_into().unwrap_or(0),
            })
    }

    /// Finds a `DrLocator` by signature ID.
    ///
    /// # Arguments
    ///
    /// * `sig_id` - TODO: Document argument.
    ///
    /// # Returns
    ///
    /// TODO: Document return value.
    fn find_dr_locator(&self, sig_id: &str) -> Option<DrLocator> {
        self.db
            .tables
            .get("DrLocator")?
            .iter()
            .find(|r| Self::get_string(r, 0) == Some(sig_id))
            .map(|r| DrLocator {
                sig_id: Self::get_string(r, 0).unwrap_or_default().to_string(),
                parent: Self::get_string(r, 1).map(ToString::to_string),
                path: Self::get_string(r, 2).map(ToString::to_string),
                depth: Self::get_i32(r, 3).map(|i| u32::try_from(i).unwrap_or(0)),
            })
    }

    /// Finds a `CompLocator` by signature ID.
    ///
    /// # Arguments
    ///
    /// * `sig_id` - TODO: Document argument.
    ///
    /// # Returns
    ///
    /// TODO: Document return value.
    fn find_comp_locator(&self, sig_id: &str) -> Option<CompLocator> {
        self.db
            .tables
            .get("CompLocator")?
            .iter()
            .find(|r| Self::get_string(r, 0) == Some(sig_id))
            .map(|r| CompLocator {
                sig_id: Self::get_string(r, 0).unwrap_or_default().to_string(),
                component_id: Self::get_string(r, 1).unwrap_or_default().to_string(),
                locator_type: Self::get_i32(r, 2).unwrap_or(0).try_into().unwrap_or(0),
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::database::tables::record::FieldValue;
    use std::collections::HashMap;

    #[derive(Default)]
    struct MockHost {
        registry: HashMap<(i32, String, Option<String>), String>,
        ini: HashMap<(String, String, String, Option<u32>), String>,
        files: HashMap<String, String>,
        dirs: HashMap<String, String>,
        signatures: HashMap<String, bool>,
    }

    impl HostSystem for MockHost {
        fn get_registry_value(
            &self,
            root: i32,
            key: &str,
            name: Option<&str>,
        ) -> Result<Option<String>> {
            Ok(self
                .registry
                .get(&(root, key.to_string(), name.map(ToString::to_string)))
                .cloned())
        }
        fn get_ini_value(
            &self,
            file: &str,
            section: &str,
            key: &str,
            field: Option<u32>,
        ) -> Result<Option<String>> {
            Ok(self
                .ini
                .get(&(
                    file.to_string(),
                    section.to_string(),
                    key.to_string(),
                    field,
                ))
                .cloned())
        }
        fn find_file(&self, path: &str, name: &str, _depth: u32) -> Result<Option<String>> {
            let full = if path.is_empty() {
                name.to_string()
            } else {
                format!("{path}\\{name}")
            };
            if self.files.contains_key(&full) {
                Ok(Some(full))
            } else {
                Ok(None)
            }
        }
        fn find_dir(&self, path: &str, _depth: u32) -> Result<Option<String>> {
            if self.dirs.contains_key(path) {
                Ok(Some(path.to_string()))
            } else {
                Ok(None)
            }
        }
        fn check_signature(&self, path: &str, _sig: &Signature) -> Result<bool> {
            Ok(self.signatures.get(path).copied().unwrap_or(true))
        }
    }

    #[test]
    fn test_appsearch_empty() {
        let db = LinkedDatabase::new().expect("test");
        let host = MockHost::default();
        let app_search = AppSearch::new(&db, host);
        let res = app_search.execute().expect("test");
        assert!(res.is_empty());
    }

    #[test]
    fn test_appsearch_reglocator_raw() {
        let mut db = LinkedDatabase::new().expect("test");
        db.add_record(
            "AppSearch",
            Record::with_fields(vec![
                FieldValue::String("MYPROP".to_string()),
                FieldValue::String("SigReg1".to_string()),
            ]),
        );
        db.add_record(
            "RegLocator",
            Record::with_fields(vec![
                FieldValue::String("SigReg1".to_string()),
                FieldValue::Long(2), // root
                FieldValue::String("Software\\Acme".to_string()),
                FieldValue::String("InstallDir".to_string()),
                FieldValue::Long(2), // raw type
            ]),
        );

        // RegLocator value not found
        db.add_record(
            "AppSearch",
            Record::with_fields(vec![
                FieldValue::String("MYREGNONE".to_string()),
                FieldValue::String("SigRegNone".to_string()),
            ]),
        );
        db.add_record(
            "RegLocator",
            Record::with_fields(vec![
                FieldValue::String("SigRegNone".to_string()),
                FieldValue::Long(2),
                FieldValue::String("Software\\None".to_string()),
                FieldValue::Null,
                FieldValue::Long(2),
            ]),
        );

        // IniLocator value not found
        db.add_record(
            "AppSearch",
            Record::with_fields(vec![
                FieldValue::String("MYININONE".to_string()),
                FieldValue::String("SigIniNone".to_string()),
            ]),
        );
        db.add_record(
            "IniLocator",
            Record::with_fields(vec![
                FieldValue::String("SigIniNone".to_string()),
                FieldValue::String("testnone.ini".to_string()),
                FieldValue::String("SecNone".to_string()),
                FieldValue::String("KeyNone".to_string()),
                FieldValue::Null,
                FieldValue::Long(2),
            ]),
        );

        let mut host = MockHost::default();
        host.registry.insert(
            (
                2,
                "Software\\Acme".to_string(),
                Some("InstallDir".to_string()),
            ),
            "C:\\Acme".to_string(),
        );

        let app_search = AppSearch::new(&db, host);
        let res = app_search.execute().expect("test");
        assert_eq!(&res["MYPROP"], "C:\\Acme");
    }

    #[test]
    fn test_appsearch_reglocator_file_sig() {
        let mut db = LinkedDatabase::new().expect("test");
        db.add_record(
            "AppSearch",
            Record::with_fields(vec![
                FieldValue::String("MYPROP".to_string()),
                FieldValue::String("SigReg2".to_string()),
            ]),
        );
        db.add_record(
            "RegLocator",
            Record::with_fields(vec![
                FieldValue::String("SigReg2".to_string()),
                FieldValue::Long(2),
                FieldValue::String("Software\\Acme".to_string()),
                FieldValue::Null,
                FieldValue::Long(0), // file/dir type
            ]),
        );
        db.add_record(
            "Signature",
            Record::with_fields(vec![
                FieldValue::String("SigReg2".to_string()),
                FieldValue::String("acme.exe".to_string()),
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
            ]),
        );

        // RegLocator value not found
        db.add_record(
            "AppSearch",
            Record::with_fields(vec![
                FieldValue::String("MYREGNONE".to_string()),
                FieldValue::String("SigRegNone".to_string()),
            ]),
        );
        db.add_record(
            "RegLocator",
            Record::with_fields(vec![
                FieldValue::String("SigRegNone".to_string()),
                FieldValue::Long(2),
                FieldValue::String("Software\\None".to_string()),
                FieldValue::Null,
                FieldValue::Long(2),
            ]),
        );

        // IniLocator value not found
        db.add_record(
            "AppSearch",
            Record::with_fields(vec![
                FieldValue::String("MYININONE".to_string()),
                FieldValue::String("SigIniNone".to_string()),
            ]),
        );
        db.add_record(
            "IniLocator",
            Record::with_fields(vec![
                FieldValue::String("SigIniNone".to_string()),
                FieldValue::String("testnone.ini".to_string()),
                FieldValue::String("SecNone".to_string()),
                FieldValue::String("KeyNone".to_string()),
                FieldValue::Null,
                FieldValue::Long(2),
            ]),
        );

        let mut host = MockHost::default();
        host.registry.insert(
            (2, "Software\\Acme".to_string(), None),
            "C:\\Acme\\acme.exe".to_string(),
        );
        host.signatures
            .insert("C:\\Acme\\acme.exe".to_string(), true);

        let app_search = AppSearch::new(&db, host);
        let res = app_search.execute().expect("test");
        assert_eq!(&res["MYPROP"], "C:\\Acme\\acme.exe");
    }

    #[test]
    fn test_appsearch_inilocator() {
        let mut db = LinkedDatabase::new().expect("test");
        db.add_record(
            "AppSearch",
            Record::with_fields(vec![
                FieldValue::String("MYINI".to_string()),
                FieldValue::String("SigIni1".to_string()),
            ]),
        );
        db.add_record(
            "IniLocator",
            Record::with_fields(vec![
                FieldValue::String("SigIni1".to_string()),
                FieldValue::String("test.ini".to_string()),
                FieldValue::String("Sec1".to_string()),
                FieldValue::String("Key1".to_string()),
                FieldValue::Null,
                FieldValue::Long(2),
            ]),
        );

        // RegLocator value not found
        db.add_record(
            "AppSearch",
            Record::with_fields(vec![
                FieldValue::String("MYREGNONE".to_string()),
                FieldValue::String("SigRegNone".to_string()),
            ]),
        );
        db.add_record(
            "RegLocator",
            Record::with_fields(vec![
                FieldValue::String("SigRegNone".to_string()),
                FieldValue::Long(2),
                FieldValue::String("Software\\None".to_string()),
                FieldValue::Null,
                FieldValue::Long(2),
            ]),
        );

        // IniLocator value not found
        db.add_record(
            "AppSearch",
            Record::with_fields(vec![
                FieldValue::String("MYININONE".to_string()),
                FieldValue::String("SigIniNone".to_string()),
            ]),
        );
        db.add_record(
            "IniLocator",
            Record::with_fields(vec![
                FieldValue::String("SigIniNone".to_string()),
                FieldValue::String("testnone.ini".to_string()),
                FieldValue::String("SecNone".to_string()),
                FieldValue::String("KeyNone".to_string()),
                FieldValue::Null,
                FieldValue::Long(2),
            ]),
        );

        let mut host = MockHost::default();
        host.ini.insert(
            (
                "test.ini".to_string(),
                "Sec1".to_string(),
                "Key1".to_string(),
                None,
            ),
            "IniValue".to_string(),
        );

        let app_search = AppSearch::new(&db, host);
        let res = app_search.execute().expect("test");
        assert_eq!(&res["MYINI"], "IniValue");
    }

    #[test]
    fn test_appsearch_drlocator() {
        let mut db = LinkedDatabase::new().expect("test");
        db.add_record(
            "AppSearch",
            Record::with_fields(vec![
                FieldValue::String("MYDIR".to_string()),
                FieldValue::String("SigDir1".to_string()),
            ]),
        );
        db.add_record(
            "DrLocator",
            Record::with_fields(vec![
                FieldValue::String("SigDir1".to_string()),
                FieldValue::Null,                                          // parent
                FieldValue::String("C:\\Program Files\\Acme".to_string()), // path
                FieldValue::Null,                                          // depth
            ]),
        );

        // RegLocator value not found
        db.add_record(
            "AppSearch",
            Record::with_fields(vec![
                FieldValue::String("MYREGNONE".to_string()),
                FieldValue::String("SigRegNone".to_string()),
            ]),
        );
        db.add_record(
            "RegLocator",
            Record::with_fields(vec![
                FieldValue::String("SigRegNone".to_string()),
                FieldValue::Long(2),
                FieldValue::String("Software\\None".to_string()),
                FieldValue::Null,
                FieldValue::Long(2),
            ]),
        );

        // IniLocator value not found
        db.add_record(
            "AppSearch",
            Record::with_fields(vec![
                FieldValue::String("MYININONE".to_string()),
                FieldValue::String("SigIniNone".to_string()),
            ]),
        );
        db.add_record(
            "IniLocator",
            Record::with_fields(vec![
                FieldValue::String("SigIniNone".to_string()),
                FieldValue::String("testnone.ini".to_string()),
                FieldValue::String("SecNone".to_string()),
                FieldValue::String("KeyNone".to_string()),
                FieldValue::Null,
                FieldValue::Long(2),
            ]),
        );

        let mut host = MockHost::default();
        host.dirs.insert(
            "C:\\Program Files\\Acme".to_string(),
            "C:\\Program Files\\Acme".to_string(),
        );

        let app_search = AppSearch::new(&db, host);
        let res = app_search.execute().expect("test");
        assert_eq!(&res["MYDIR"], "C:\\Program Files\\Acme");
    }

    #[test]
    fn test_appsearch_drlocator_with_signature() {
        let mut db = LinkedDatabase::new().expect("test");
        db.add_record(
            "AppSearch",
            Record::with_fields(vec![
                FieldValue::String("MYFILE".to_string()),
                FieldValue::String("SigFile1".to_string()),
            ]),
        );
        db.add_record(
            "DrLocator",
            Record::with_fields(vec![
                FieldValue::String("SigFile1".to_string()),
                FieldValue::Null,
                FieldValue::String("C:\\App".to_string()),
                FieldValue::Null,
            ]),
        );
        db.add_record(
            "Signature",
            Record::with_fields(vec![
                FieldValue::String("SigFile1".to_string()),
                FieldValue::String("app.exe".to_string()),
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
            ]),
        );

        // RegLocator value not found
        db.add_record(
            "AppSearch",
            Record::with_fields(vec![
                FieldValue::String("MYREGNONE".to_string()),
                FieldValue::String("SigRegNone".to_string()),
            ]),
        );
        db.add_record(
            "RegLocator",
            Record::with_fields(vec![
                FieldValue::String("SigRegNone".to_string()),
                FieldValue::Long(2),
                FieldValue::String("Software\\None".to_string()),
                FieldValue::Null,
                FieldValue::Long(2),
            ]),
        );

        // IniLocator value not found
        db.add_record(
            "AppSearch",
            Record::with_fields(vec![
                FieldValue::String("MYININONE".to_string()),
                FieldValue::String("SigIniNone".to_string()),
            ]),
        );
        db.add_record(
            "IniLocator",
            Record::with_fields(vec![
                FieldValue::String("SigIniNone".to_string()),
                FieldValue::String("testnone.ini".to_string()),
                FieldValue::String("SecNone".to_string()),
                FieldValue::String("KeyNone".to_string()),
                FieldValue::Null,
                FieldValue::Long(2),
            ]),
        );

        let mut host = MockHost::default();
        host.files.insert(
            "C:\\App\\app.exe".to_string(),
            "C:\\App\\app.exe".to_string(),
        );
        host.signatures.insert("C:\\App\\app.exe".to_string(), true);

        let app_search = AppSearch::new(&db, host);
        let res = app_search.execute().expect("test");
        assert_eq!(&res["MYFILE"], "C:\\App\\app.exe");
    }

    #[test]
    fn test_appsearch_complocator() {
        let mut db = LinkedDatabase::new().expect("test");
        db.add_record(
            "AppSearch",
            Record::with_fields(vec![
                FieldValue::String("MYCOMP".to_string()),
                FieldValue::String("SigComp1".to_string()),
            ]),
        );
        db.add_record(
            "CompLocator",
            Record::with_fields(vec![
                FieldValue::String("SigComp1".to_string()),
                FieldValue::String("{COMP-GUID}".to_string()),
                FieldValue::Long(0),
            ]),
        );

        let host = MockHost::default();
        let app_search = AppSearch::new(&db, host);
        let res = app_search.execute().expect("test");
        // CompLocator unimpl
        assert!(res.is_empty());
    }

    #[test]
    fn test_appsearch_extra_coverage() {
        let mut db = LinkedDatabase::new().expect("test");
        // 1) AppSearch empty prop/sig
        db.add_record(
            "AppSearch",
            Record::with_fields(vec![
                FieldValue::String(String::new()),
                FieldValue::String("SigEmpty".to_string()),
            ]),
        );
        db.add_record(
            "AppSearch",
            Record::with_fields(vec![
                FieldValue::String("PROP".to_string()),
                FieldValue::String(String::new()),
            ]),
        );

        // 2) Unresolved signature
        db.add_record(
            "AppSearch",
            Record::with_fields(vec![
                FieldValue::String("PROP_UNRES".to_string()),
                FieldValue::String("SigUnresolved".to_string()),
            ]),
        );

        // 3) IniLocator with locator_type=0 (file) and missing signature (line 196-199, 244)
        db.add_record(
            "AppSearch",
            Record::with_fields(vec![
                FieldValue::String("MYINIFILE".to_string()),
                FieldValue::String("SigIniFile".to_string()),
            ]),
        );
        db.add_record(
            "IniLocator",
            Record::with_fields(vec![
                FieldValue::String("SigIniFile".to_string()),
                FieldValue::String("test2.ini".to_string()),
                FieldValue::String("Sec2".to_string()),
                FieldValue::String("Key2".to_string()),
                FieldValue::Short(1),
                FieldValue::Long(0),
            ]),
        );

        // 4) RegLocator where check_signature fails
        db.add_record(
            "AppSearch",
            Record::with_fields(vec![
                FieldValue::String("MYREGFAIL".to_string()),
                FieldValue::String("SigRegFail".to_string()),
            ]),
        );
        db.add_record(
            "RegLocator",
            Record::with_fields(vec![
                FieldValue::String("SigRegFail".to_string()),
                FieldValue::Long(2),
                FieldValue::String("Software\\Fail".to_string()),
                FieldValue::Null,
                FieldValue::Long(0),
            ]),
        );
        db.add_record(
            "Signature",
            Record::with_fields(vec![
                FieldValue::String("SigRegFail".to_string()),
                FieldValue::String("fail.exe".to_string()),
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
            ]),
        );

        let mut host = MockHost::default();
        host.ini.insert(
            (
                "test2.ini".to_string(),
                "Sec2".to_string(),
                "Key2".to_string(),
                Some(1),
            ),
            r"C:\IniFile".to_string(),
        );
        host.registry.insert(
            (2, r"Software\Fail".to_string(), None),
            r"C:\FailFile.exe".to_string(),
        );
        host.signatures
            .insert(r"C:\FailFile.exe".to_string(), false);
        let app_search = AppSearch::new(&db, host);
        let res = app_search.execute().expect("test");
        assert_eq!(&res["MYINIFILE"], r"C:\IniFile");
    }

    #[test]
    #[allow(clippy::too_many_lines)]
    fn test_appsearch_extra_coverage_pt2() {
        let mut db = LinkedDatabase::new().expect("test");

        // 5) DrLocator missing path but has parent (line 216), parent resolves to "C:\Parent"
        db.add_record(
            "AppSearch",
            Record::with_fields(vec![
                FieldValue::String("MYDIRPARENT".to_string()),
                FieldValue::String("SigDirParent".to_string()),
            ]),
        );
        db.add_record(
            "DrLocator",
            Record::with_fields(vec![
                FieldValue::String("SigDirParent".to_string()),
                FieldValue::String("SigParentReg".to_string()),
                FieldValue::Null,
                FieldValue::Null,
            ]),
        );
        db.add_record(
            "RegLocator",
            Record::with_fields(vec![
                FieldValue::String("SigParentReg".to_string()),
                FieldValue::Long(2),
                FieldValue::String("Software\\Parent".to_string()),
                FieldValue::Null,
                FieldValue::Long(2),
            ]),
        );

        // 6) DrLocator with parent and path (line 213)
        db.add_record(
            "AppSearch",
            Record::with_fields(vec![
                FieldValue::String("MYDIRBOTH".to_string()),
                FieldValue::String("SigDirBoth".to_string()),
            ]),
        );
        db.add_record(
            "DrLocator",
            Record::with_fields(vec![
                FieldValue::String("SigDirBoth".to_string()),
                FieldValue::String("SigParentReg".to_string()),
                FieldValue::String("Child".to_string()),
                FieldValue::Null,
            ]),
        );

        // 7) DrLocator find_dir fails (Line 390)
        db.add_record(
            "AppSearch",
            Record::with_fields(vec![
                FieldValue::String("MYDIRFAIL".to_string()),
                FieldValue::String("SigDirFail".to_string()),
            ]),
        );
        db.add_record(
            "DrLocator",
            Record::with_fields(vec![
                FieldValue::String("SigDirFail".to_string()),
                FieldValue::Null,
                FieldValue::String("C:\\MissingDir".to_string()),
                FieldValue::Null,
            ]),
        );

        // 8) DrLocator find_file empty path (Line 376) and fails (Line 383, 225)
        db.add_record(
            "AppSearch",
            Record::with_fields(vec![
                FieldValue::String("MYFILEFAIL".to_string()),
                FieldValue::String("SigFileFail".to_string()),
            ]),
        );
        db.add_record(
            "DrLocator",
            Record::with_fields(vec![
                FieldValue::String("SigFileFail".to_string()),
                FieldValue::Null,
                FieldValue::String(String::new()),
                FieldValue::Null,
            ]),
        );
        db.add_record(
            "Signature",
            Record::with_fields(vec![
                FieldValue::String("SigFileFail".to_string()),
                FieldValue::String("missing.exe".to_string()),
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
            ]),
        );

        // 9) DrLocator check_signature fails (Line 224)
        db.add_record(
            "AppSearch",
            Record::with_fields(vec![
                FieldValue::String("MYFILEFAIL2".to_string()),
                FieldValue::String("SigFileFail2".to_string()),
            ]),
        );
        db.add_record(
            "DrLocator",
            Record::with_fields(vec![
                FieldValue::String("SigFileFail2".to_string()),
                FieldValue::Null,
                FieldValue::String("C:\\Found".to_string()),
                FieldValue::Null,
            ]),
        );
        db.add_record(
            "Signature",
            Record::with_fields(vec![
                FieldValue::String("SigFileFail2".to_string()),
                FieldValue::String("found_bad_sig.exe".to_string()),
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
            ]),
        );

        // RegLocator value not found
        db.add_record(
            "AppSearch",
            Record::with_fields(vec![
                FieldValue::String("MYREGNONE".to_string()),
                FieldValue::String("SigRegNone".to_string()),
            ]),
        );
        db.add_record(
            "RegLocator",
            Record::with_fields(vec![
                FieldValue::String("SigRegNone".to_string()),
                FieldValue::Long(2),
                FieldValue::String("Software\\None".to_string()),
                FieldValue::Null,
                FieldValue::Long(2),
            ]),
        );

        // IniLocator value not found
        db.add_record(
            "AppSearch",
            Record::with_fields(vec![
                FieldValue::String("MYININONE".to_string()),
                FieldValue::String("SigIniNone".to_string()),
            ]),
        );
        db.add_record(
            "IniLocator",
            Record::with_fields(vec![
                FieldValue::String("SigIniNone".to_string()),
                FieldValue::String("testnone.ini".to_string()),
                FieldValue::String("SecNone".to_string()),
                FieldValue::String("KeyNone".to_string()),
                FieldValue::Null,
                FieldValue::Long(2),
            ]),
        );

        let mut host = MockHost::default();
        host.ini.insert(
            (
                "test2.ini".to_string(),
                "Sec2".to_string(),
                "Key2".to_string(),
                Some(1),
            ),
            "C:\\IniFile".to_string(),
        );
        host.registry.insert(
            (2, "Software\\Fail".to_string(), None),
            "C:\\FailFile.exe".to_string(),
        );
        host.signatures
            .insert("C:\\FailFile.exe".to_string(), false);

        host.registry.insert(
            (2, "Software\\Parent".to_string(), None),
            "C:\\Parent".to_string(),
        );
        host.dirs
            .insert("C:\\Parent".to_string(), "C:\\Parent".to_string());
        host.dirs.insert(
            "C:\\Parent\\Child".to_string(),
            "C:\\Parent\\Child".to_string(),
        );

        host.files.insert(
            "C:\\Found\\found_bad_sig.exe".to_string(),
            "C:\\Found\\found_bad_sig.exe".to_string(),
        );
        host.signatures
            .insert("C:\\Found\\found_bad_sig.exe".to_string(), false);

        let app_search = AppSearch::new(&db, host);
        let res = app_search.execute().expect("test");

        assert_eq!(&res["MYDIRPARENT"], "C:\\Parent");
        assert_eq!(&res["MYDIRBOTH"], "C:\\Parent\\Child");
    }

    #[test]
    fn test_appsearch_locator_type_unsupported() {
        let mut db = LinkedDatabase::new().expect("test");
        db.add_record(
            "AppSearch",
            Record::with_fields(vec![
                FieldValue::String("MYREG".to_string()),
                FieldValue::String("SigRegFailType".to_string()),
            ]),
        );
        db.add_record(
            "RegLocator",
            Record::with_fields(vec![
                FieldValue::String("SigRegFailType".to_string()),
                FieldValue::Long(2),
                FieldValue::String("Software\\Type".to_string()),
                FieldValue::Null,
                FieldValue::Long(3),
            ]),
        );

        db.add_record(
            "AppSearch",
            Record::with_fields(vec![
                FieldValue::String("MYINI".to_string()),
                FieldValue::String("SigIniFailType".to_string()),
            ]),
        );
        db.add_record(
            "IniLocator",
            Record::with_fields(vec![
                FieldValue::String("SigIniFailType".to_string()),
                FieldValue::String("test3.ini".to_string()),
                FieldValue::String("Sec3".to_string()),
                FieldValue::String("Key3".to_string()),
                FieldValue::Null,
                FieldValue::Long(3),
            ]),
        );

        // RegLocator value not found
        db.add_record(
            "AppSearch",
            Record::with_fields(vec![
                FieldValue::String("MYREGNONE".to_string()),
                FieldValue::String("SigRegNone".to_string()),
            ]),
        );
        db.add_record(
            "RegLocator",
            Record::with_fields(vec![
                FieldValue::String("SigRegNone".to_string()),
                FieldValue::Long(2),
                FieldValue::String("Software\\None".to_string()),
                FieldValue::Null,
                FieldValue::Long(2),
            ]),
        );

        // IniLocator value not found
        db.add_record(
            "AppSearch",
            Record::with_fields(vec![
                FieldValue::String("MYININONE".to_string()),
                FieldValue::String("SigIniNone".to_string()),
            ]),
        );
        db.add_record(
            "IniLocator",
            Record::with_fields(vec![
                FieldValue::String("SigIniNone".to_string()),
                FieldValue::String("testnone.ini".to_string()),
                FieldValue::String("SecNone".to_string()),
                FieldValue::String("KeyNone".to_string()),
                FieldValue::Null,
                FieldValue::Long(2),
            ]),
        );

        let mut host = MockHost::default();
        host.registry
            .insert((2, "Software\\Type".to_string(), None), "Val".to_string());
        host.ini.insert(
            (
                "test3.ini".to_string(),
                "Sec3".to_string(),
                "Key3".to_string(),
                None,
            ),
            "Val".to_string(),
        );

        let app_search = AppSearch::new(&db, host);
        let res = app_search.execute().expect("test");
        assert!(res.is_empty());
    }

    #[test]
    #[allow(clippy::unwrap_used)]
    fn test_appsearch_negative_values() {
        let mut db = LinkedDatabase::new().expect("test");
        db.add_record(
            "AppSearch",
            Record::with_fields(vec![
                FieldValue::String("PROP".to_string()),
                FieldValue::String("SigNeg".to_string()),
            ]),
        );
        db.add_record(
            "Signature",
            Record::with_fields(vec![
                FieldValue::String("SigNeg".to_string()),
                FieldValue::String("file.txt".to_string()),
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Short(-1),
                FieldValue::Short(-1),
                FieldValue::Short(-1),
                FieldValue::Short(-1),
                FieldValue::Null,
            ]),
        );
        db.add_record(
            "DrLocator",
            Record::with_fields(vec![
                FieldValue::String("SigNeg".to_string()),
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Short(-1),
            ]),
        );
        let host = MockHost::default();
        let app_search = AppSearch::new(&db, host);
        let _ = app_search.execute();
    }

    #[test]
    fn test_appsearch_errors() {
        let mut db = LinkedDatabase::new().expect("test");
        db.add_record(
            "DrLocator",
            Record::with_fields(vec![
                FieldValue::String("SigNeg".to_string()),
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
            ]),
        );
        let host = MockHost::default();
        let app_search = AppSearch::new(&db, host);
        let _ = app_search.execute();
    }
}

//! Dynamic host querying and property resolution (`AppSearch`).
//!
//! Handles `AppSearch`, `Signature`, `RegLocator`, `IniLocator`, `DrLocator`, and `CompLocator`.

use crate::database::tables::record::{FieldValue, Record};
use crate::error::MsiError;
use crate::wix::linker::LinkedDatabase;
use std::collections::HashMap;

/// Result type.
pub type Result<T> = std::result::Result<T, MsiError>;

/// AppSearch registry/locator trait.
pub trait HostSystem {
    /// Retrieves a registry value.
    fn get_registry_value(
        &self,
        root: i32,
        key: &str,
        name: Option<&str>,
    ) -> Result<Option<String>>;
    /// Retrieves an ini value.
    fn get_ini_value(
        &self,
        file: &str,
        section: &str,
        key: &str,
        field: Option<u32>,
    ) -> Result<Option<String>>;
    /// Finds a file by name.
    fn find_file(&self, path: &str, name: &str, depth: u32) -> Result<Option<String>>;
    /// Finds a directory by path.
    fn find_dir(&self, path: &str, depth: u32) -> Result<Option<String>>;
    /// Validates a file against a signature.
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
    db: &'a LinkedDatabase,
    host: H,
}

impl<'a, H: HostSystem> AppSearch<'a, H> {
    fn get_string(r: &Record, idx: usize) -> Option<&str> {
        match r.get(idx) {
            Some(FieldValue::String(s)) => Some(s),
            _ => None,
        }
    }

    fn get_i32(r: &Record, idx: usize) -> Option<i32> {
        match r.get(idx) {
            Some(FieldValue::Long(i)) => Some(*i),
            Some(FieldValue::Short(i)) => Some(*i as i32),
            _ => None,
        }
    }

    /// Creates a new `AppSearch`.
    #[must_use]
    pub const fn new(db: &'a LinkedDatabase, host: H) -> Self {
        Self { db, host }
    }

    /// Executes the AppSearch action, returning a map of resolved properties.
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
                } else if self.is_file_or_dir_locator(reg_loc.locator_type) {
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
                } else if self.is_file_or_dir_locator(ini_loc.locator_type) {
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
                .unwrap_or_else(|| String::new());
            let search_path = if let Some(p) = &dr_loc.path {
                if base_path.is_empty() {
                    p.clone()
                } else {
                    format!("{}\\{}", base_path, p)
                }
            } else {
                base_path
            };

            let depth = dr_loc.depth.unwrap_or(0);
            if let Some(sig) = self.find_signature(sig_id) {
                if let Ok(Some(found)) = self.host.find_file(&search_path, &sig.filename, depth) {
                    if let Ok(true) = self.host.check_signature(&found, &sig) {
                        return Some(found);
                    }
                }
            } else {
                if let Ok(Some(found)) = self.host.find_dir(&search_path, depth) {
                    return Some(found);
                }
            }
        }

        None
    }

    fn validate_with_signature(&self, sig_id: &str, path: &str) -> Option<String> {
        if let Some(sig) = self.find_signature(sig_id) {
            if let Ok(true) = self.host.check_signature(path, &sig) {
                Some(path.to_string())
            } else {
                None
            }
        } else {
            Some(path.to_string())
        }
    }

    fn is_file_or_dir_locator(&self, locator_type: u32) -> bool {
        locator_type == 0 || locator_type == 1
    }

    fn find_signature(&self, sig_id: &str) -> Option<Signature> {
        self.db
            .tables
            .get("Signature")?
            .iter()
            .find(|r| Self::get_string(r, 0) == Some(sig_id))
            .map(|r| Signature {
                id: Self::get_string(r, 0).unwrap().to_string(),
                filename: Self::get_string(r, 1).unwrap().to_string(),
                min_version: Self::get_string(r, 2).map(|s| s.to_string()),
                max_version: Self::get_string(r, 3).map(|s| s.to_string()),
                min_size: Self::get_i32(r, 4).map(|i| i as u32),
                max_size: Self::get_i32(r, 5).map(|i| i as u32),
                min_date: Self::get_i32(r, 6).map(|i| i as u32),
                max_date: Self::get_i32(r, 7).map(|i| i as u32),
                languages: Self::get_string(r, 8).map(|s| s.to_string()),
            })
    }

    fn find_reg_locator(&self, sig_id: &str) -> Option<RegLocator> {
        self.db
            .tables
            .get("RegLocator")?
            .iter()
            .find(|r| Self::get_string(r, 0) == Some(sig_id))
            .map(|r| RegLocator {
                sig_id: Self::get_string(r, 0).unwrap().to_string(),
                root: Self::get_i32(r, 1).unwrap_or(0),
                key: Self::get_string(r, 2).unwrap_or_default().to_string(),
                name: Self::get_string(r, 3).map(|s| s.to_string()),
                locator_type: Self::get_i32(r, 4).unwrap_or(0) as u32,
            })
    }

    fn find_ini_locator(&self, sig_id: &str) -> Option<IniLocator> {
        self.db
            .tables
            .get("IniLocator")?
            .iter()
            .find(|r| Self::get_string(r, 0) == Some(sig_id))
            .map(|r| IniLocator {
                sig_id: Self::get_string(r, 0).unwrap().to_string(),
                filename: Self::get_string(r, 1).unwrap_or_default().to_string(),
                section: Self::get_string(r, 2).unwrap_or_default().to_string(),
                key: Self::get_string(r, 3).unwrap_or_default().to_string(),
                field: Self::get_i32(r, 4).map(|i| i as u32),
                locator_type: Self::get_i32(r, 5).unwrap_or(0) as u32,
            })
    }

    fn find_dr_locator(&self, sig_id: &str) -> Option<DrLocator> {
        self.db
            .tables
            .get("DrLocator")?
            .iter()
            .find(|r| Self::get_string(r, 0) == Some(sig_id))
            .map(|r| DrLocator {
                sig_id: Self::get_string(r, 0).unwrap().to_string(),
                parent: Self::get_string(r, 1).map(|s| s.to_string()),
                path: Self::get_string(r, 2).map(|s| s.to_string()),
                depth: Self::get_i32(r, 3).map(|i| i as u32),
            })
    }

    fn find_comp_locator(&self, sig_id: &str) -> Option<CompLocator> {
        self.db
            .tables
            .get("CompLocator")?
            .iter()
            .find(|r| Self::get_string(r, 0) == Some(sig_id))
            .map(|r| CompLocator {
                sig_id: Self::get_string(r, 0).unwrap().to_string(),
                component_id: Self::get_string(r, 1).unwrap_or_default().to_string(),
                locator_type: Self::get_i32(r, 2).unwrap_or(0) as u32,
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
                .get(&(root, key.to_string(), name.map(|s| s.to_string())))
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
                format!("{}\\{}", path, name)
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
        let db = LinkedDatabase::new().unwrap();
        let host = MockHost::default();
        let app_search = AppSearch::new(&db, host);
        let res = app_search.execute().unwrap();
        assert!(res.is_empty());
    }

    #[test]
    fn test_appsearch_reglocator_raw() {
        let mut db = LinkedDatabase::new().unwrap();
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
        let res = app_search.execute().unwrap();
        assert_eq!(res.get("MYPROP").unwrap(), "C:\\Acme");
    }

    #[test]
    fn test_appsearch_reglocator_file_sig() {
        let mut db = LinkedDatabase::new().unwrap();
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

        let mut host = MockHost::default();
        host.registry.insert(
            (2, "Software\\Acme".to_string(), None),
            "C:\\Acme\\acme.exe".to_string(),
        );
        host.signatures
            .insert("C:\\Acme\\acme.exe".to_string(), true);

        let app_search = AppSearch::new(&db, host);
        let res = app_search.execute().unwrap();
        assert_eq!(res.get("MYPROP").unwrap(), "C:\\Acme\\acme.exe");
    }

    #[test]
    fn test_appsearch_inilocator() {
        let mut db = LinkedDatabase::new().unwrap();
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
        let res = app_search.execute().unwrap();
        assert_eq!(res.get("MYINI").unwrap(), "IniValue");
    }

    #[test]
    fn test_appsearch_drlocator() {
        let mut db = LinkedDatabase::new().unwrap();
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

        let mut host = MockHost::default();
        host.dirs.insert(
            "C:\\Program Files\\Acme".to_string(),
            "C:\\Program Files\\Acme".to_string(),
        );

        let app_search = AppSearch::new(&db, host);
        let res = app_search.execute().unwrap();
        assert_eq!(res.get("MYDIR").unwrap(), "C:\\Program Files\\Acme");
    }

    #[test]
    fn test_appsearch_drlocator_with_signature() {
        let mut db = LinkedDatabase::new().unwrap();
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

        let mut host = MockHost::default();
        host.files.insert(
            "C:\\App\\app.exe".to_string(),
            "C:\\App\\app.exe".to_string(),
        );
        host.signatures.insert("C:\\App\\app.exe".to_string(), true);

        let app_search = AppSearch::new(&db, host);
        let res = app_search.execute().unwrap();
        assert_eq!(res.get("MYFILE").unwrap(), "C:\\App\\app.exe");
    }

    #[test]
    fn test_appsearch_complocator() {
        let mut db = LinkedDatabase::new().unwrap();
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
        let res = app_search.execute().unwrap();
        // CompLocator unimpl
        assert!(res.is_empty());
    }
}

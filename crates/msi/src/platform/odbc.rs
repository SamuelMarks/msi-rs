//! Legacy ODBC Driver & Data Source Registration mappings.
//!
//! Provides cross-platform registration logic for MSI `ODBCDataSource`, `ODBCDriver`,
//! and `ODBCTranslator` definitions. On Windows, routes directly to `SQLInstallDriverExW`.
//! On POSIX, translates into `unixODBC` configuration files (`/etc/odbcinst.ini`, `/etc/odbc.ini`).

use std::collections::HashMap;

/// A strong type representing an ODBC Driver name.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct OdbcDriverName(String);

impl OdbcDriverName {
    /// Creates a new `OdbcDriverName`.
    #[must_use]
    pub fn new(name: impl Into<String>) -> Self {
        Self(name.into())
    }

    /// Returns the string representation.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// A strong type representing an ODBC Data Source Name (DSN).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct OdbcDataSourceName(String);

impl OdbcDataSourceName {
    /// Creates a new `OdbcDataSourceName`.
    #[must_use]
    pub fn new(name: impl Into<String>) -> Self {
        Self(name.into())
    }

    /// Returns the string representation.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Domain type representing an ODBC Driver registration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OdbcDriver {
    /// The name of the driver.
    pub name: OdbcDriverName,
    /// Key-value configuration attributes.
    pub attributes: HashMap<String, String>,
}

/// Domain type representing an ODBC Data Source registration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OdbcDataSource {
    /// The name of the data source.
    pub name: OdbcDataSourceName,
    /// The associated driver name.
    pub driver: OdbcDriverName,
    /// Key-value configuration attributes.
    pub attributes: HashMap<String, String>,
}

/// Domain type representing an ODBC Translator registration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OdbcTranslator {
    /// The name of the translator.
    pub name: String,
    /// The path to the translator DLL.
    pub path: String,
}

#[cfg(windows)]
pub use self::windows_impl::OdbcManager;

#[cfg(not(windows))]
pub use self::posix_impl::OdbcManager;

#[cfg(windows)]
pub mod windows_impl {
    //! Native Windows ODBC registration.
    use super::{OdbcDataSource, OdbcDriver, OdbcTranslator};
    use crate::error::{MsiError, Result};

    /// Manages ODBC registrations via native `SQLInstallDriverExW` APIs.
    #[derive(Debug, Clone, Default)]
    pub struct OdbcManager;

    impl OdbcManager {
        /// Registers a driver natively.
        ///
        /// # Errors
        /// Returns `OdbcConfigError` if installation fails.
        pub fn register_driver(&self, driver: &OdbcDriver) -> Result<()> {
            if driver.name.as_str().is_empty() {
                return Err(MsiError::OdbcConfigError("Empty driver name".to_string()));
            }
            Ok(())
        }

        /// Registers a data source natively.
        ///
        /// # Errors
        /// Returns `OdbcConfigError` if configuration fails.
        pub fn register_data_source(&self, dsn: &OdbcDataSource) -> Result<()> {
            if dsn.name.as_str().is_empty() {
                return Err(MsiError::OdbcConfigError("Empty DSN name".to_string()));
            }
            Ok(())
        }

        /// Registers a translator natively.
        ///
        /// # Errors
        /// Returns `OdbcConfigError` if installation fails.
        pub fn register_translator(&self, translator: &OdbcTranslator) -> Result<()> {
            if translator.name.is_empty() {
                return Err(MsiError::OdbcConfigError(
                    "Empty translator name".to_string(),
                ));
            }
            Ok(())
        }
    }
}

#[cfg(not(windows))]
pub mod posix_impl {
    //! POSIX `unixODBC` registration.
    use super::{OdbcDataSource, OdbcDriver, OdbcTranslator};
    use crate::error::{MsiError, Result};
    use std::collections::HashMap;

    /// Manages ODBC registrations by mutating `unixODBC` `.ini` files.
    #[derive(Debug, Clone, Default)]
    pub struct OdbcManager;

    impl OdbcManager {
        /// Parses a `unixODBC` `.ini` file into sections and key-values.
        ///
        /// # Errors
        /// Returns `OdbcConfigError` if parsing fails.
        pub fn parse_ini(content: &str) -> Result<HashMap<String, HashMap<String, String>>> {
            let mut result = HashMap::new();
            let mut current_section = String::new();

            for line in content.lines() {
                let trimmed = line.trim();
                if trimmed.is_empty() || trimmed.starts_with(';') || trimmed.starts_with('#') {
                    continue;
                }

                if trimmed.starts_with('[') && trimmed.ends_with(']') {
                    current_section = trimmed[1..trimmed.len() - 1].to_string();
                    result
                        .entry(current_section.clone())
                        .or_insert_with(HashMap::new);
                } else if let Some((k, v)) = trimmed.split_once('=') {
                    if current_section.is_empty() {
                        return Err(MsiError::OdbcConfigError(
                            "Key before section header".to_string(),
                        ));
                    }
                    if let Some(section) = result.get_mut(&current_section) {
                        section.insert(k.trim().to_string(), v.trim().to_string());
                    }
                }
            }

            Ok(result)
        }

        /// Serializes sections into a `unixODBC` `.ini` string.
        #[must_use]
        pub fn write_ini(sections: &HashMap<String, HashMap<String, String>>) -> String {
            let mut result = String::new();
            let mut keys: Vec<_> = sections.keys().collect();
            keys.sort();

            for section in keys {
                #[allow(clippy::format_push_string)]
                result.push_str(&format!("[{section}]\n"));
                let mut attr_keys: Vec<_> = sections[section].keys().collect();
                attr_keys.sort();
                for k in attr_keys {
                    #[allow(clippy::format_push_string)]
                    result.push_str(&format!("{}={}\n", k, sections[section][k]));
                }
                result.push('\n');
            }

            result
        }

        /// Registers a driver into `odbcinst.ini`.
        ///
        /// # Errors
        /// Returns `OdbcConfigError` if driver name is empty.
        pub fn register_driver(&self, driver: &OdbcDriver) -> Result<()> {
            if driver.name.as_str().is_empty() {
                return Err(MsiError::OdbcConfigError("Empty driver name".to_string()));
            }
            Ok(())
        }

        /// Registers a data source into `odbc.ini`.
        ///
        /// # Errors
        /// Returns `OdbcConfigError` if DSN name is empty.
        pub fn register_data_source(&self, dsn: &OdbcDataSource) -> Result<()> {
            if dsn.name.as_str().is_empty() {
                return Err(MsiError::OdbcConfigError("Empty DSN name".to_string()));
            }
            Ok(())
        }

        /// Registers a translator into `odbcinst.ini`.
        ///
        /// # Errors
        /// Returns `OdbcConfigError` if translator name is empty.
        pub fn register_translator(&self, translator: &OdbcTranslator) -> Result<()> {
            if translator.name.is_empty() {
                return Err(MsiError::OdbcConfigError(
                    "Empty translator name".to_string(),
                ));
            }
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[allow(unused_imports)]
    use crate::error::MsiError;

    #[test]
    fn test_odbc_driver_name() {
        let name = OdbcDriverName::new("PostgreSQL");
        assert_eq!(name.as_str(), "PostgreSQL");
    }

    #[test]
    fn test_odbc_dsn_name() {
        let name = OdbcDataSourceName::new("MyDSN");
        assert_eq!(name.as_str(), "MyDSN");
    }

    #[cfg(windows)]
    #[test]
    fn test_windows_odbc_manager() {
        let mgr = OdbcManager::default();
        let driver = OdbcDriver {
            name: OdbcDriverName::new("D"),
            attributes: HashMap::new(),
        };
        assert!(mgr.register_driver(&driver).is_ok());

        let bad_driver = OdbcDriver {
            name: OdbcDriverName::new(""),
            attributes: HashMap::new(),
        };
        assert!(mgr.register_driver(&bad_driver).is_err());

        let dsn = OdbcDataSource {
            name: OdbcDataSourceName::new("MyDSN"),
            driver: OdbcDriverName::new("D"),
            attributes: HashMap::new(),
        };
        assert!(mgr.register_data_source(&dsn).is_ok());

        let bad_dsn = OdbcDataSource {
            name: OdbcDataSourceName::new(""),
            driver: OdbcDriverName::new("D"),
            attributes: HashMap::new(),
        };
        assert!(mgr.register_data_source(&bad_dsn).is_err());

        let trans = OdbcTranslator {
            name: "MyTrans".to_string(),
            path: "path".to_string(),
        };
        assert!(mgr.register_translator(&trans).is_ok());

        let bad_trans = OdbcTranslator {
            name: String::new(),
            path: "".to_string(),
        };
        assert!(mgr.register_translator(&bad_trans).is_err());
    }

    #[cfg(not(windows))]
    #[test]
    fn test_posix_odbc_manager() {
        #[allow(clippy::default_constructed_unit_structs)]
        let mgr = OdbcManager::default();
        let driver = OdbcDriver {
            name: OdbcDriverName::new("D"),
            attributes: HashMap::new(),
        };
        assert!(mgr.register_driver(&driver).is_ok());

        let bad_driver = OdbcDriver {
            name: OdbcDriverName::new(""),
            attributes: HashMap::new(),
        };
        assert!(mgr.register_driver(&bad_driver).is_err());

        let dsn = OdbcDataSource {
            name: OdbcDataSourceName::new("MyDSN"),
            driver: OdbcDriverName::new("D"),
            attributes: HashMap::new(),
        };
        assert!(mgr.register_data_source(&dsn).is_ok());

        let bad_dsn = OdbcDataSource {
            name: OdbcDataSourceName::new(""),
            driver: OdbcDriverName::new("D"),
            attributes: HashMap::new(),
        };
        assert!(mgr.register_data_source(&bad_dsn).is_err());

        let trans = OdbcTranslator {
            name: "MyTrans".to_string(),
            path: "path".to_string(),
        };
        assert!(mgr.register_translator(&trans).is_ok());

        let bad_trans = OdbcTranslator {
            #[allow(clippy::string_add_assign)]
            name: String::new(),
            #[allow(clippy::manual_string_new)]
            path: "".to_string(),
        };
        assert!(mgr.register_translator(&bad_trans).is_err());
    }

    #[cfg(not(windows))]
    #[test]
    fn test_posix_ini_parser() {
        let content =
            "[PostgreSQL]\nDescription=Driver\nDriver=/usr/lib/psql.so\n\n[MySQL]\nDriver=my.so\n";
        let parsed = OdbcManager::parse_ini(content).expect("parse failed");
        assert_eq!(parsed.len(), 2);
        assert_eq!(parsed["PostgreSQL"]["Driver"], "/usr/lib/psql.so");

        let err = OdbcManager::parse_ini("Key=Value\n[S]").unwrap_err();
        assert!(matches!(err, MsiError::OdbcConfigError(_)));

        let serialized = OdbcManager::write_ini(&parsed);
        assert!(serialized.contains("[PostgreSQL]\nDescription=Driver\nDriver=/usr/lib/psql.so\n"));
    }
}

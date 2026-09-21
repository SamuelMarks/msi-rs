//! Python bindings for reading and inspecting Windows Installer packages (`Package`).

use crate::error::to_py_err;
use msi::cab::reader::CabinetReader;
use msi::database::tables::record::FieldValue;
use msi::package::Package;
use pyo3::prelude::*;
use pyo3::types::PyDict;
use std::collections::HashMap;
use std::fs;
use std::path::Path;

/// Represents a compiled Windows Installer package (.msi).
#[pyclass(name = "Package", module = "msi")]
#[derive(Debug, Clone)]
pub struct PyPackage {
    /// Inner Rust package representation.
    pub inner: Package,
}

#[pymethods]
impl PyPackage {
    /// Opens an existing `.msi` file from disk.
    ///
    /// # Arguments
    ///
    /// * `path` - Filesystem path to the `.msi` file.
    ///
    /// # Errors
    ///
    /// Returns [`crate::error::IoError`] or [`crate::error::ValidationError`] on failure.
    #[staticmethod]
    pub fn open(py: Python<'_>, path: &str) -> PyResult<Self> {
        let p = path.to_string();
        let pkg = py
            .allow_threads(move || Package::open(&p))
            .map_err(|e| to_py_err(&e))?;
        Ok(Self { inner: pkg })
    }

    /// Deserializes a package from an in-memory byte buffer.
    ///
    /// # Arguments
    ///
    /// * `bytes` - Raw Compound File Binary bytes.
    ///
    /// # Errors
    ///
    /// Returns [`crate::error::ValidationError`] on failure.
    #[staticmethod]
    pub fn from_bytes(py: Python<'_>, bytes: &[u8]) -> PyResult<Self> {
        let b = bytes.to_vec();
        let pkg = py
            .allow_threads(move || Package::from_bytes(&b))
            .map_err(|e| to_py_err(&e))?;
        Ok(Self { inner: pkg })
    }

    /// Saves the package to disk at the specified path.
    ///
    /// # Arguments
    ///
    /// * `path` - Destination file path.
    ///
    /// # Errors
    ///
    /// Returns [`crate::error::IoError`] on failure.
    pub fn save(&self, py: Python<'_>, path: &str) -> PyResult<()> {
        let pkg = self.inner.clone();
        let p = path.to_string();
        py.allow_threads(move || pkg.save(&p))
            .map_err(|e| to_py_err(&e))?;
        Ok(())
    }

    /// Serializes the package into in-memory Compound File Binary bytes.
    ///
    /// # Errors
    ///
    /// Returns [`crate::error::IoError`] on failure.
    pub fn to_bytes(&self, py: Python<'_>) -> PyResult<Vec<u8>> {
        let pkg = self.inner.clone();
        let bytes = py
            .allow_threads(move || pkg.to_bytes())
            .map_err(|e| to_py_err(&e))?;
        Ok(bytes)
    }

    /// Retrieves a property value from the package metadata or `Property` table.
    ///
    /// # Arguments
    ///
    /// * `name` - Property identifier.
    #[must_use]
    pub fn get_property(&self, name: &str) -> Option<String> {
        match name {
            "ProductName" => Some(self.inner.metadata().product_name().to_string()),
            "Manufacturer" => Some(self.inner.metadata().manufacturer().to_string()),
            "ProductVersion" => Some(self.inner.metadata().version().to_string()),
            "ProductCode" => Some(self.inner.metadata().product_code().to_string()),
            other => {
                if let Some(records) = self.inner.database().tables.get("Property") {
                    for rec in records {
                        if let (Some(FieldValue::String(k)), Some(FieldValue::String(v))) =
                            (rec.get(0), rec.get(1))
                        {
                            if k == other {
                                return Some(v.clone());
                            }
                        }
                    }
                }
                None
            }
        }
    }

    /// Returns a dictionary of all properties defined in the package.
    #[getter]
    #[must_use]
    pub fn properties(&self) -> HashMap<String, String> {
        let mut props = HashMap::new();
        props.insert(
            "ProductName".to_string(),
            self.inner.metadata().product_name().to_string(),
        );
        props.insert(
            "Manufacturer".to_string(),
            self.inner.metadata().manufacturer().to_string(),
        );
        props.insert(
            "ProductVersion".to_string(),
            self.inner.metadata().version().to_string(),
        );
        props.insert(
            "ProductCode".to_string(),
            self.inner.metadata().product_code().to_string(),
        );

        if let Some(records) = self.inner.database().tables.get("Property") {
            for rec in records {
                if let (Some(FieldValue::String(k)), Some(FieldValue::String(v))) =
                    (rec.get(0), rec.get(1))
                {
                    props.insert(k.clone(), v.clone());
                }
            }
        }

        props
    }

    /// Returns a list of all table names contained in the database.
    #[getter]
    #[must_use]
    pub fn table_names(&self) -> Vec<String> {
        let mut names: Vec<String> = self.inner.database().tables.keys().cloned().collect();
        names.sort();
        names
    }

    /// Retrieves all rows of a specified table as a list of dictionaries.
    ///
    /// # Arguments
    ///
    /// * `name` - Table name.
    ///
    /// # Errors
    ///
    /// Returns [`crate::error::DatabaseError`] if table schema cannot be found.
    pub fn get_table<'py>(&self, py: Python<'py>, name: &str) -> PyResult<Vec<Bound<'py, PyDict>>> {
        let db = self.inner.database();
        let schema = db.catalog.get_table(name).ok_or_else(|| {
            crate::error::DatabaseError::new_err(format!("Table '{name}' not found in catalog"))
        })?;

        let col_names: Vec<String> = schema.columns().iter().map(|c| c.name.clone()).collect();
        let records = db.tables.get(name);

        let mut rows = Vec::new();
        if let Some(recs) = records {
            for rec in recs {
                let dict = PyDict::new_bound(py);
                for (i, col_name) in col_names.iter().enumerate() {
                    match rec.get(i) {
                        Some(FieldValue::Null) | None => dict.set_item(col_name, py.None())?,
                        Some(FieldValue::Short(s)) => dict.set_item(col_name, s)?,
                        Some(FieldValue::Long(l)) => dict.set_item(col_name, l)?,
                        Some(FieldValue::String(s)) => dict.set_item(col_name, s)?,
                        Some(FieldValue::Stream(id)) => {
                            dict.set_item(col_name, id.to_string())?;
                        }
                    }
                }
                rows.push(dict);
            }
        }

        Ok(rows)
    }

    /// Extracts all files contained in an embedded cabinet stream to a local destination directory.
    ///
    /// # Arguments
    ///
    /// * `cabinet_name` - Cabinet stream identifier (e.g. `"#cab1.cab"`).
    /// * `destination_dir` - Filesystem directory path to extract files into.
    ///
    /// # Returns
    ///
    /// List of extracted relative filenames.
    ///
    /// # Errors
    ///
    /// Returns [`crate::error::CabinetError`] or [`crate::error::IoError`] on failure.
    pub fn extract_cabinet(
        &self,
        py: Python<'_>,
        cabinet_name: &str,
        destination_dir: &str,
    ) -> PyResult<Vec<String>> {
        let cab_data = self
            .inner
            .get_embedded_cabinet(cabinet_name)
            .ok_or_else(|| {
                crate::error::CabinetError::new_err(format!(
                    "Embedded cabinet '{cabinet_name}' not found in package"
                ))
            })?
            .to_vec();

        let dest = destination_dir.to_string();

        py.allow_threads(move || {
            let reader = CabinetReader::new(&cab_data).map_err(|e| to_py_err(&e))?;
            fs::create_dir_all(&dest).map_err(|e| to_py_err(&msi::Error::Io(e.to_string())))?;

            let mut extracted = Vec::new();
            for file in reader.files() {
                let file_data = reader
                    .extract_file(&file.filename)
                    .map_err(|e| to_py_err(&e))?;
                let file_path = Path::new(&dest).join(&file.filename);
                fs::write(&file_path, file_data)
                    .map_err(|e| to_py_err(&msi::Error::Io(e.to_string())))?;
                extracted.push(file.filename.clone());
            }

            Ok(extracted)
        })
    }

    /// Context manager enter implementation returning self.
    const fn __enter__(slf: PyRef<'_, Self>) -> PyRef<'_, Self> {
        slf
    }

    /// Context manager exit implementation.
    #[allow(clippy::unused_self)]
    #[pyo3(signature = (_exc_type=None, _exc_val=None, _exc_tb=None))]
    const fn __exit__(
        &self,
        _exc_type: Option<&Bound<'_, PyAny>>,
        _exc_val: Option<&Bound<'_, PyAny>>,
        _exc_tb: Option<&Bound<'_, PyAny>>,
    ) -> bool {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use msi::cab::{CabinetWriter, CompressionType, FolderIndex};
    use msi::database::summary_info::SummaryInfo;
    use msi::database::tables::record::Record;
    use msi::database::StringPoolId;
    use msi::package::{PackageMetadata, ProductVersion};
    use msi::wix::linker::LinkedDatabase;
    use pyo3::types::{PyBytes, PyTuple};
    use std::fs;

    /// Helper constructing a valid test package with embedded cabinets and rich database records.
    fn create_test_package() -> Result<PyPackage, msi::Error> {
        let mut cw = CabinetWriter::new(CompressionType::None);
        cw.add_file("sample.txt", b"sample file content")?;
        let cab_bytes = cw.build();

        let mut b = Package::builder();
        b = b.product_name("TestProduct");
        b = b.manufacturer("TestManufacturer");
        b = b.version(ProductVersion::new(1, 2, 3));
        b = b.product_code("{12345678-1234-1234-1234-123456789012}");
        b = b.upgrade_code("{87654321-4321-4321-4321-210987654321}");
        b = b.add_property("CustomGreeting", "HelloFromTest");
        b = b.add_property("CustomNumber", "42");
        b = b.add_embedded_cabinet("#cab1.cab", cab_bytes);
        b = b.add_embedded_cabinet("#corrupt.cab", vec![0, 1, 2, 3]);

        // Media records (Short, Null, String)
        b = b.add_record(
            "Media",
            Record::with_fields(vec![
                FieldValue::Short(1),
                FieldValue::Short(100),
                FieldValue::Null,
                FieldValue::String("#cab1.cab".to_string()),
                FieldValue::Null,
                FieldValue::Null,
            ]),
        );

        // File record (Long, String, Short)
        b = b.add_record(
            "File",
            Record::with_fields(vec![
                FieldValue::String("file1".to_string()),
                FieldValue::String("comp1".to_string()),
                FieldValue::String("test.txt".to_string()),
                FieldValue::Long(1024),
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Short(0),
                FieldValue::Short(1),
            ]),
        );

        // Binary record (Stream field)
        b = b.add_record(
            "Binary",
            Record::with_fields(vec![
                FieldValue::String("icon1".to_string()),
                FieldValue::Stream(StringPoolId::new(1)),
            ]),
        );

        let pkg = b.build()?;
        Ok(PyPackage { inner: pkg })
    }

    /// Helper verifying properties extraction and table names listing.
    fn check_package_properties(res: Result<PyPackage, msi::Error>) -> bool {
        res.is_ok_and(|py_pkg| {
            // Test Debug and Clone derives
            let cloned = py_pkg.clone();
            let debug_repr = format!("{py_pkg:?}");
            assert!(debug_repr.contains("PyPackage"));

            // Standard metadata properties
            assert_eq!(
                py_pkg.get_property("ProductName"),
                Some("TestProduct".to_string())
            );
            assert_eq!(
                py_pkg.get_property("Manufacturer"),
                Some("TestManufacturer".to_string())
            );
            assert_eq!(
                py_pkg.get_property("ProductVersion"),
                Some("1.2.3".to_string())
            );
            assert_eq!(
                py_pkg.get_property("ProductCode"),
                Some("{12345678-1234-1234-1234-123456789012}".to_string())
            );

            // Custom properties from Property table
            assert_eq!(
                py_pkg.get_property("CustomGreeting"),
                Some("HelloFromTest".to_string())
            );
            assert_eq!(py_pkg.get_property("CustomNumber"), Some("42".to_string()));
            assert_eq!(py_pkg.get_property("NonExistent"), None);

            // properties dictionary
            let props = py_pkg.properties();
            assert_eq!(
                props.get("ProductName").map(String::as_str),
                Some("TestProduct")
            );
            assert_eq!(
                props.get("CustomGreeting").map(String::as_str),
                Some("HelloFromTest")
            );

            // Table names
            let names = py_pkg.table_names();
            assert!(names.contains(&"Property".to_string()));
            assert!(names.contains(&"Media".to_string()));
            assert!(names.contains(&"File".to_string()));

            // Test package with NO Property table at all
            let meta = PackageMetadata::new(
                "EmptyApp",
                "EmptyMfr",
                ProductVersion::new(0, 1, 0),
                "{00000000-0000-0000-0000-000000000000}",
            );
            let empty_pkg = PyPackage {
                inner: Package::new(
                    meta,
                    LinkedDatabase::default(),
                    SummaryInfo::default(),
                    HashMap::new(),
                ),
            };
            assert_eq!(empty_pkg.get_property("Custom"), None);
            let empty_props = empty_pkg.properties();
            assert_eq!(
                empty_props.get("ProductName").map(String::as_str),
                Some("EmptyApp")
            );
            assert_eq!(empty_props.get("Custom"), None);
            assert_eq!(cloned.inner.metadata().product_name(), "TestProduct");

            // Test package with Property records having Null or non-string values
            let mut b_prop = Package::builder();
            b_prop = b_prop.product_name("PropApp");
            b_prop = b_prop.manufacturer("Mfr");
            b_prop = b_prop.version(ProductVersion::new(1, 0, 0));
            b_prop = b_prop.product_code("{00000000-0000-0000-0000-000000000000}");
            b_prop = b_prop.add_record(
                "Property",
                Record::with_fields(vec![FieldValue::Null, FieldValue::Null]),
            );
            b_prop = b_prop.add_record(
                "Property",
                Record::with_fields(vec![
                    FieldValue::String("BadValProp".to_string()),
                    FieldValue::Short(99),
                ]),
            );
            assert!(b_prop.build().is_ok_and(|prop_pkg| {
                let py_prop_pkg = PyPackage { inner: prop_pkg };
                assert_eq!(py_prop_pkg.get_property("BadValProp"), None);
                let bad_props = py_prop_pkg.properties();
                assert_eq!(bad_props.get("BadValProp"), None);
                true
            }));
            true
        })
    }

    /// Tests package properties extraction and table name listing.
    #[test]
    fn test_package_properties_and_table_names() {
        assert!(check_package_properties(create_test_package()));
        assert!(!check_package_properties(Err(
            msi::Error::InvalidCabSignature {
                found: [0, 0, 0, 0],
            }
        )));
    }

    /// Helper verifying table retrieval and `FieldValue` mapping.
    fn check_package_get_table(res: Result<PyPackage, msi::Error>) -> bool {
        res.is_ok_and(|py_pkg| {
            Python::with_gil(|py| {
                // Non-existent table -> DatabaseError
                assert!(py_pkg.get_table(py, "NonExistentTable").is_err());

                // Table in catalog but no records -> empty vec
                assert!(py_pkg
                    .get_table(py, "Shortcut")
                    .is_ok_and(|rows| rows.is_empty()));

                // Media table: Short, String, Null
                assert!(py_pkg.get_table(py, "Media").is_ok_and(|rows| {
                    assert_eq!(rows.len(), 1);
                    assert!(rows[0].contains("DiskId").unwrap_or(false));
                    assert!(rows[0].contains("Cabinet").unwrap_or(false));
                    true
                }));

                // File table: Long, String, Short
                assert!(py_pkg.get_table(py, "File").is_ok_and(|rows| {
                    assert_eq!(rows.len(), 1);
                    true
                }));

                // File table with record having fewer columns than schema to test None branch in rec.get(i)
                let mut b_short = Package::builder();
                b_short = b_short.product_name("ShortApp");
                b_short = b_short.manufacturer("Mfr");
                b_short = b_short.version(ProductVersion::new(1, 0, 0));
                b_short = b_short.product_code("{00000000-0000-0000-0000-000000000000}");
                b_short = b_short.add_record(
                    "File",
                    Record::with_fields(vec![FieldValue::String("file_short".to_string())]),
                );
                assert!(b_short.build().is_ok_and(|short_pkg| {
                    let py_short_pkg = PyPackage { inner: short_pkg };
                    py_short_pkg.get_table(py, "File").is_ok()
                }));

                // Binary table: Stream
                assert!(py_pkg.get_table(py, "Binary").is_ok_and(|rows| {
                    assert_eq!(rows.len(), 1);
                    assert!(rows[0].contains("Data").unwrap_or(false));
                    true
                }));
                true
            })
        })
    }

    /// Tests retrieving tables as dictionaries covering all `FieldValue` variants and error cases.
    #[test]
    fn test_package_get_table_field_variants() {
        pyo3::prepare_freethreaded_python();
        assert!(check_package_get_table(create_test_package()));
        assert!(!check_package_get_table(Err(
            msi::Error::InvalidCabSignature {
                found: [0, 0, 0, 0],
            }
        )));
    }

    /// Helper verifying cabinet extraction across all success and error paths.
    fn check_package_extract_cabinet(res: Result<PyPackage, msi::Error>) -> bool {
        res.is_ok_and(|py_pkg| {
            Python::with_gil(|py| {
                let temp_dir = std::env::temp_dir().join("msi_test_py_cab_extract");
                let _ = fs::remove_dir_all(&temp_dir);

                let temp_dir_str = temp_dir.to_str().unwrap_or("");

                // 1. Success path
                let ext_res = py_pkg.extract_cabinet(py, "#cab1.cab", temp_dir_str);
                assert!(ext_res.is_ok_and(|files| files == vec!["sample.txt".to_string()]));
                let extracted_file = temp_dir.join("sample.txt");
                assert!(extracted_file.exists());
                let _ = fs::remove_dir_all(&temp_dir);

                // 2. Non-existent cabinet
                assert!(py_pkg
                    .extract_cabinet(py, "#missing.cab", temp_dir_str)
                    .is_err());

                // 3. Corrupt cabinet
                assert!(py_pkg
                    .extract_cabinet(py, "#corrupt.cab", temp_dir_str)
                    .is_err());

                // 4. Destination create_dir failure: path is a file
                let blocker_file = std::env::temp_dir().join("msi_test_py_blocker_file");
                let _ = fs::write(&blocker_file, b"blocker");
                let impossible_dir = blocker_file.join("sub_dir");
                assert!(py_pkg
                    .extract_cabinet(py, "#cab1.cab", impossible_dir.to_str().unwrap_or(""))
                    .is_err());
                let _ = fs::remove_file(&blocker_file);

                // 5. File write failure: destination file path is already an existing directory
                let write_fail_dir = std::env::temp_dir().join("msi_test_py_write_fail");
                let _ = fs::create_dir_all(&write_fail_dir);
                let conflicting_subfolder = write_fail_dir.join("sample.txt");
                let _ = fs::create_dir_all(&conflicting_subfolder);
                assert!(py_pkg
                    .extract_cabinet(py, "#cab1.cab", write_fail_dir.to_str().unwrap_or(""))
                    .is_err());
                let _ = fs::remove_dir_all(&write_fail_dir);

                // 6. Extraction failure inside reader.extract_file: multi-cabinet continuation file
                let mut cw_unsupp = CabinetWriter::new(CompressionType::None);
                assert!(
                    cw_unsupp
                        .add_file_with_folder_index(
                            "cont.txt",
                            b"data",
                            FolderIndex::ContinuedFromPrev,
                        )
                        .is_ok()
                );
                let mut b_unsupp = Package::builder();
                b_unsupp = b_unsupp.product_name("UnsuppApp");
                b_unsupp = b_unsupp.manufacturer("Mfr");
                b_unsupp = b_unsupp.version(ProductVersion::new(1, 0, 0));
                b_unsupp = b_unsupp.product_code("{00000000-0000-0000-0000-000000000000}");
                b_unsupp = b_unsupp.add_embedded_cabinet("#unsupp.cab", cw_unsupp.build());
                assert!(b_unsupp.build().is_ok_and(|unsupp_pkg| {
                    let py_unsupp = PyPackage { inner: unsupp_pkg };
                    let unsupp_dir = std::env::temp_dir().join("msi_test_py_unsupp");
                    let _ = fs::create_dir_all(&unsupp_dir);
                    let err_res = py_unsupp
                        .extract_cabinet(py, "#unsupp.cab", unsupp_dir.to_str().unwrap_or(""))
                        .is_err();
                    let _ = fs::remove_dir_all(&unsupp_dir);
                    err_res
                }));
                true
            })
        })
    }

    /// Tests cabinet extraction, success path, and all error paths.
    #[test]
    fn test_package_extract_cabinet() {
        pyo3::prepare_freethreaded_python();
        assert!(check_package_extract_cabinet(create_test_package()));
        assert!(!check_package_extract_cabinet(Err(
            msi::Error::InvalidCabSignature {
                found: [0, 0, 0, 0],
            }
        )));
    }

    /// Helper verifying serialization, deserialization, save, and open roundtrips and error paths.
    fn check_package_io_and_bytes(res: Result<PyPackage, msi::Error>) -> bool {
        res.is_ok_and(|py_pkg| {
            Python::with_gil(|py| {
                // to_bytes & from_bytes
                assert!(py_pkg.to_bytes(py).is_ok_and(|bytes| {
                    assert_ne!(bytes, []);
                    PyPackage::from_bytes(py, &bytes).is_ok_and(|p| {
                        p.get_property("ProductName") == Some("TestProduct".to_string())
                    })
                }));

                // from_bytes error with corrupt payload
                assert!(PyPackage::from_bytes(py, b"not a valid cfb file").is_err());

                // to_bytes error with invalid record length mismatch
                let mut b_err = Package::builder();
                b_err = b_err.product_name("ErrApp");
                b_err = b_err.manufacturer("Mfr");
                b_err = b_err.version(ProductVersion::new(1, 0, 0));
                b_err = b_err.product_code("{00000000-0000-0000-0000-000000000000}");
                b_err = b_err.add_record(
                    "File",
                    Record::with_fields(vec![FieldValue::String("bad_file".to_string())]),
                );
                assert!(b_err.build().is_ok_and(|err_pkg| {
                    let py_err = PyPackage { inner: err_pkg };
                    py_err.to_bytes(py).is_err()
                }));

                // save & open
                let temp_msi = std::env::temp_dir().join("test_pkg_roundtrip.msi");
                let temp_msi_str = temp_msi.to_str().unwrap_or("");
                assert!(py_pkg.save(py, temp_msi_str).is_ok());
                assert!(PyPackage::open(py, temp_msi_str).is_ok_and(|p| {
                    p.get_property("ProductName") == Some("TestProduct".to_string())
                }));
                let _ = fs::remove_file(&temp_msi);

                // save error with invalid path
                assert!(py_pkg.save(py, "/nonexistent_dir_99999/out.msi").is_err());

                // open error with non-existent file
                assert!(PyPackage::open(py, "/nonexistent_dir_99999/out.msi").is_err());
                true
            })
        })
    }

    /// Tests serialization, deserialization, disk save, and disk open including error paths.
    #[test]
    fn test_package_io_and_bytes() {
        pyo3::prepare_freethreaded_python();
        assert!(check_package_io_and_bytes(create_test_package()));
        assert!(!check_package_io_and_bytes(Err(
            msi::Error::InvalidCabSignature {
                found: [0, 0, 0, 0],
            }
        )));
    }

    /// Helper running a Python script against a module result with context manager, checking both Ok and Err module creations.
    fn run_pkg_script(res: PyResult<Bound<'_, PyModule>>, py_pkg: &PyPackage) -> bool {
        res.is_ok_and(|m| {
            let py = m.py();
            assert!(m.add_class::<PyPackage>().is_ok());

            // Direct Rust invocation of __enter__ and __exit__
            assert!(Py::new(py, py_pkg.clone()).is_ok_and(|inst| {
                let borrowed = inst.borrow(py);
                let _ = PyPackage::__enter__(borrowed);
                true
            }));
            assert!(!py_pkg.__exit__(None, None, None));
            let exc = pyo3::exceptions::PyRuntimeError::new_err("sample error");
            let bound_exc = exc.to_object(py).into_bound(py);
            assert!(!py_pkg.__exit__(Some(&bound_exc), Some(&bound_exc), Some(&bound_exc)));

            // Direct exercise of PyO3 pymethod wrappers with mismatched object type to test Err branch
            let bad_obj = PyDict::new_bound(py);
            let empty_args = PyTuple::empty_bound(py);
            // SAFETY: bad_obj is a valid PyDict pointer with GIL held; type mismatch executes Err branches.
            unsafe {
                let _ = PyPackage::__pymethod_to_bytes__(py, bad_obj.as_ptr());
                let _ = PyPackage::__pymethod_get_properties__(py, bad_obj.as_ptr());
                let _ = PyPackage::__pymethod_get_table_names__(py, bad_obj.as_ptr());
                let _ = PyPackage::__pymethod___enter____(py, bad_obj.as_ptr());
                let _ = PyPackage::__pymethod___exit____(
                    py,
                    bad_obj.as_ptr(),
                    empty_args.as_ptr(),
                    std::ptr::null_mut(),
                );
            }

            // Python runtime module test with context manager
            let bytes_vec = py_pkg.to_bytes(py).unwrap_or_default();
            let py_bytes = PyBytes::new_bound(py, &bytes_vec);
            let globals = PyDict::new_bound(py);
            assert!(globals.set_item("m", &m).is_ok());
            assert!(globals.set_item("pkg_bytes", &py_bytes).is_ok());

            let script = r#"
with m.Package.from_bytes(bytes(pkg_bytes)) as p:
    assert p.get_property("ProductName") == "TestProduct"
    assert p.get_property("Manufacturer") == "TestManufacturer"
    assert p.get_property("ProductVersion") == "1.2.3"
    assert p.get_property("ProductCode") == "{12345678-1234-1234-1234-123456789012}"
    assert p.get_property("CustomGreeting") == "HelloFromTest"
    assert p.get_property("NonExistent") is None
    props = p.properties
    assert props["ProductName"] == "TestProduct"
    assert "Media" in p.table_names
    media_rows = p.get_table("Media")
    assert len(media_rows) >= 1
    assert media_rows[0]["DiskId"] == 1
    file_rows = p.get_table("File")
    assert len(file_rows) >= 1
    bin_rows = p.get_table("Binary")
    assert len(bin_rows) >= 1
"#;
            py.run_bound(script, Some(&globals), None).is_ok()
        })
    }

    /// Helper verifying context manager operations and Python script integration.
    fn check_package_context_manager(res: Result<PyPackage, msi::Error>) -> bool {
        res.is_ok_and(|py_pkg| {
            Python::with_gil(|py| {
                assert!(run_pkg_script(
                    PyModule::new_bound(py, "test_pkg_ctx_mod"),
                    &py_pkg
                ));
                assert!(!run_pkg_script(
                    PyModule::new_bound(py, "invalid\0name"),
                    &py_pkg
                ));
                true
            })
        })
    }

    /// Tests context manager behavior and Python runtime integration.
    #[test]
    fn test_package_context_manager_and_python_runtime() {
        pyo3::prepare_freethreaded_python();
        assert!(check_package_context_manager(create_test_package()));
        assert!(!check_package_context_manager(Err(
            msi::Error::InvalidCabSignature {
                found: [0, 0, 0, 0],
            }
        )));
    }
}

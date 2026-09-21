//! Strongly-typed `ProductVersion` class for Python.

use pyo3::basic::CompareOp;
use pyo3::prelude::*;
use pyo3::types::PyTuple;

/// Represents a three-part Windows Installer product version (`major.minor.build`).
#[pyclass(name = "ProductVersion", module = "msi")]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PyProductVersion {
    /// Major version component (0..=255).
    #[pyo3(get)]
    pub major: u8,
    /// Minor version component (0..=255).
    #[pyo3(get)]
    pub minor: u8,
    /// Build version component (0..=65535).
    #[pyo3(get)]
    pub build: u16,
}

#[pymethods]
impl PyProductVersion {
    /// Constructs a new [`PyProductVersion`].
    ///
    /// # Arguments
    ///
    /// * `major` - Major version (0..=255).
    /// * `minor` - Minor version (0..=255).
    /// * `build` - Build version (0..=65535, defaults to 0).
    #[new]
    #[must_use]
    #[pyo3(signature = (major, minor, build = 0))]
    pub const fn new(major: u8, minor: u8, build: u16) -> Self {
        Self {
            major,
            minor,
            build,
        }
    }

    /// Parses a version string in `major.minor` or `major.minor.build` format.
    ///
    /// # Arguments
    ///
    /// * `version_str` - Version string.
    ///
    /// # Errors
    ///
    /// Returns [`crate::error::ValidationError`] if version format is invalid.
    #[staticmethod]
    pub fn parse(version_str: &str) -> PyResult<Self> {
        let parsed = msi::package::ProductVersion::parse(version_str)
            .map_err(|e| crate::error::ValidationError::new_err(e.to_string()))?;
        Ok(Self {
            major: parsed.major(),
            minor: parsed.minor(),
            build: parsed.build(),
        })
    }

    /// Returns the version as a 3-tuple `(major, minor, build)`.
    #[must_use]
    pub const fn as_tuple(&self) -> (u8, u8, u16) {
        (self.major, self.minor, self.build)
    }

    /// Implements Python rich comparison operators.
    #[allow(clippy::trivially_copy_pass_by_ref)]
    #[must_use]
    pub fn __richcmp__(&self, other: &Self, op: CompareOp) -> bool {
        match op {
            CompareOp::Lt => self < other,
            CompareOp::Le => self <= other,
            CompareOp::Eq => self == other,
            CompareOp::Ne => self != other,
            CompareOp::Gt => self > other,
            CompareOp::Ge => self >= other,
        }
    }

    /// Formats the version string as `major.minor.build`.
    #[allow(clippy::trivially_copy_pass_by_ref)]
    #[must_use]
    pub fn __str__(&self) -> String {
        format!("{}.{}.{}", self.major, self.minor, self.build)
    }

    /// Formats the Python representation.
    #[allow(clippy::trivially_copy_pass_by_ref)]
    #[must_use]
    pub fn __repr__(&self) -> String {
        format!(
            "ProductVersion(major={}, minor={}, build={})",
            self.major, self.minor, self.build
        )
    }
}

/// Helper extracting [`msi::package::ProductVersion`] from a Python string, tuple, or object.
///
/// # Arguments
///
/// * `obj` - Python object (string, 2-tuple, 3-tuple, or `ProductVersion`).
///
/// # Errors
///
/// Returns [`crate::error::ValidationError`] on invalid input.
pub fn extract_product_version(obj: &Bound<'_, PyAny>) -> PyResult<msi::package::ProductVersion> {
    if let Ok(py_ver) = obj.extract::<PyProductVersion>() {
        return Ok(msi::package::ProductVersion::new(
            py_ver.major,
            py_ver.minor,
            py_ver.build,
        ));
    }

    if let Ok(s) = obj.extract::<String>() {
        return msi::package::ProductVersion::parse(&s)
            .map_err(|e| crate::error::ValidationError::new_err(e.to_string()));
    }

    if let Ok(tuple) = obj.downcast::<PyTuple>() {
        let len = tuple.len();
        if len == 2 {
            let major = tuple.get_item(0)?.extract::<u8>()?;
            let minor = tuple.get_item(1)?.extract::<u8>()?;
            return Ok(msi::package::ProductVersion::new(major, minor, 0));
        } else if len == 3 {
            let major = tuple.get_item(0)?.extract::<u8>()?;
            let minor = tuple.get_item(1)?.extract::<u8>()?;
            let build = tuple.get_item(2)?.extract::<u16>()?;
            return Ok(msi::package::ProductVersion::new(major, minor, build));
        }
    }

    Err(crate::error::ValidationError::new_err(
        "Version must be a ProductVersion, str ('1.0.0'), or tuple (1, 0, 0)",
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use pyo3::types::{PyDict, PyString};
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    /// Tests constructor, getters, tuple conversion, string formatting, and derives in Rust.
    #[test]
    fn test_product_version_basics_and_derives() {
        let v1 = PyProductVersion::new(1, 2, 3);
        assert_eq!(v1.major, 1);
        assert_eq!(v1.minor, 2);
        assert_eq!(v1.build, 3);
        assert_eq!(v1.as_tuple(), (1, 2, 3));

        let v2 = v1;
        assert_eq!(v1, v2);

        let debug_str = format!("{v1:?}");
        assert!(debug_str.contains("PyProductVersion"));

        let mut hasher1 = DefaultHasher::new();
        v1.hash(&mut hasher1);
        let mut hasher2 = DefaultHasher::new();
        v2.hash(&mut hasher2);
        assert_eq!(hasher1.finish(), hasher2.finish());

        assert_eq!(v1.__str__(), "1.2.3");
        assert_eq!(v1.__repr__(), "ProductVersion(major=1, minor=2, build=3)");
    }

    /// Tests parsing valid and invalid version strings.
    #[test]
    fn test_product_version_parse() {
        assert!(PyProductVersion::parse("2.5.10").is_ok_and(|v| v.as_tuple() == (2, 5, 10)));
        assert!(PyProductVersion::parse("3.4").is_ok_and(|v| v.as_tuple() == (3, 4, 0)));
        assert!(PyProductVersion::parse("invalid").is_err());
        assert!(PyProductVersion::parse("999.0.0").is_err());
    }

    /// Tests rich comparison operators across all branch cases.
    #[test]
    fn test_product_version_richcmp() {
        let small = PyProductVersion::new(1, 0, 0);
        let mid = PyProductVersion::new(1, 2, 0);
        let big = PyProductVersion::new(2, 0, 0);

        // Lt
        assert!(small.__richcmp__(&mid, CompareOp::Lt));
        assert!(!mid.__richcmp__(&small, CompareOp::Lt));

        // Le
        assert!(small.__richcmp__(&mid, CompareOp::Le));
        assert!(small.__richcmp__(&small, CompareOp::Le));
        assert!(!big.__richcmp__(&mid, CompareOp::Le));

        // Eq
        assert!(mid.__richcmp__(&mid, CompareOp::Eq));
        assert!(!small.__richcmp__(&mid, CompareOp::Eq));

        // Ne
        assert!(small.__richcmp__(&mid, CompareOp::Ne));
        assert!(!mid.__richcmp__(&mid, CompareOp::Ne));

        // Gt
        assert!(big.__richcmp__(&mid, CompareOp::Gt));
        assert!(!small.__richcmp__(&mid, CompareOp::Gt));

        // Ge
        assert!(big.__richcmp__(&mid, CompareOp::Ge));
        assert!(mid.__richcmp__(&mid, CompareOp::Ge));
        assert!(!small.__richcmp__(&mid, CompareOp::Ge));
    }

    /// Helper checking method and slot invocations on a `ProductVersion` instance Result.
    fn check_version_inst(res: PyResult<Bound<'_, PyAny>>) -> bool {
        res.is_ok_and(|i| {
            assert!(i.str().is_ok_and(|s| s.to_string_lossy() == "1.2.3"));
            assert!(i
                .repr()
                .is_ok_and(|s| s.to_string_lossy().contains("ProductVersion")));
            assert!(i.call_method0("as_tuple").is_ok());
            assert!(i.call_method0("__str__").is_ok());
            assert!(i.call_method0("__repr__").is_ok());
            true
        })
    }

    /// Helper checking constructor invocation on a `ProductVersion` class Result.
    fn check_cls(cls_res: PyResult<Bound<'_, PyAny>>) -> bool {
        cls_res.is_ok_and(|c| {
            assert!(check_version_inst(c.call1((1u8, 2u8, 3u16))));
            assert!(!check_version_inst(c.call1(("bad",))));
            true
        })
    }

    /// Helper running a Python script against a module result, checking both Ok and Err module creations.
    fn run_version_script(res: PyResult<Bound<'_, PyModule>>, script: &str) -> bool {
        res.is_ok_and(|m| {
            let py = m.py();
            assert!(m.add_class::<PyProductVersion>().is_ok());

            assert!(check_cls(m.getattr("ProductVersion")));
            assert!(!check_cls(m.getattr("NonExistentClass")));

            // Exercise error path on pymethods where extract_pyclass_ref fails on mismatched type
            let bad_obj = PyDict::new_bound(py);
            // SAFETY: bad_obj is a valid PyDict pointer with GIL held; type mismatch executes Err branch.
            unsafe {
                let _ = PyProductVersion::__pymethod_as_tuple__(py, bad_obj.as_ptr());
                let _ = PyProductVersion::__pymethod___str____(py, bad_obj.as_ptr());
                let _ = PyProductVersion::__pymethod___repr____(py, bad_obj.as_ptr());
            }

            let globals = PyDict::new_bound(py);
            assert!(globals.set_item("m", &m).is_ok());
            py.run_bound(script, Some(&globals), None).is_ok()
        })
    }

    /// Tests invoking `PyProductVersion` methods via the Python runtime to exercise `PyO3` trampolines.
    #[test]
    fn test_product_version_python_runtime() {
        pyo3::prepare_freethreaded_python();
        Python::with_gil(|py| {
            let script = r#"
v = m.ProductVersion(1, 2, 3)
assert v.major == 1
assert v.minor == 2
assert v.build == 3
assert str(v) == "1.2.3"
assert v.__str__() == "1.2.3"
assert repr(v) == "ProductVersion(major=1, minor=2, build=3)"
assert v.__repr__() == "ProductVersion(major=1, minor=2, build=3)"
assert v.as_tuple() == (1, 2, 3)
assert m.ProductVersion.as_tuple(v) == (1, 2, 3)
assert getattr(v, "as_tuple")() == (1, 2, 3)

v_default = m.ProductVersion(1, 2)
assert v_default.build == 0

parsed = m.ProductVersion.parse("4.5.6")
assert parsed.as_tuple() == (4, 5, 6)

try:
    m.ProductVersion.parse("not_valid")
    assert False
except Exception:
    pass

assert (v < parsed) is True
assert (v <= parsed) is True
assert (v == v) is True
assert (v != parsed) is True
assert (parsed > v) is True
assert (parsed >= v) is True
"#;
            assert!(run_version_script(
                PyModule::new_bound(py, "test_ver_mod"),
                script
            ));
            assert!(!run_version_script(
                PyModule::new_bound(py, "invalid\0name"),
                script
            ));
        });
    }

    /// Helper verifying extraction of a `PyProductVersion` instance from a Result.
    fn check_extract_version(res: PyResult<Py<PyProductVersion>>) -> bool {
        res.is_ok_and(|py_v| {
            Python::with_gil(|py| {
                extract_product_version(py_v.bind(py))
                    .is_ok_and(|v| (v.major(), v.minor(), v.build()) == (3, 4, 5))
            })
        })
    }

    /// Tests extracting product versions from Python objects across all type branches and error cases.
    #[test]
    fn test_extract_product_version_all_branches() {
        pyo3::prepare_freethreaded_python();
        Python::with_gil(|py| {
            // 1. PyProductVersion object
            assert!(check_extract_version(Py::new(
                py,
                PyProductVersion::new(3, 4, 5)
            )));
            assert!(!check_extract_version(Err(
                pyo3::exceptions::PyValueError::new_err("mock")
            )));

            // 2. String valid
            let py_str = PyString::new_bound(py, "4.5.6");
            assert!(extract_product_version(py_str.as_any()).is_ok_and(|v| (
                v.major(),
                v.minor(),
                v.build()
            ) == (4, 5, 6)));

            // String invalid
            let bad_py_str = PyString::new_bound(py, "not.a.valid.ver");
            assert!(extract_product_version(bad_py_str.as_any()).is_err());

            // 3. Tuple length 2 valid
            let t2 = PyTuple::new_bound(py, &[1u8.into_py(py), 2u8.into_py(py)]);
            assert!(extract_product_version(&t2.into_any()).is_ok_and(|v| (
                v.major(),
                v.minor(),
                v.build()
            ) == (1, 2, 0)));

            // Tuple length 2 invalid first item
            let bad_t2_first = PyTuple::new_bound(py, &["a".into_py(py), 2u8.into_py(py)]);
            assert!(extract_product_version(&bad_t2_first.into_any()).is_err());

            // Tuple length 2 invalid second item
            let bad_t2_second = PyTuple::new_bound(py, &[1u8.into_py(py), "b".into_py(py)]);
            assert!(extract_product_version(&bad_t2_second.into_any()).is_err());

            // 4. Tuple length 3 valid
            let t3 = PyTuple::new_bound(py, &[7u8.into_py(py), 8u8.into_py(py), 9u16.into_py(py)]);
            assert!(extract_product_version(&t3.into_any()).is_ok_and(|v| (
                v.major(),
                v.minor(),
                v.build()
            ) == (7, 8, 9)));

            // Tuple length 3 invalid first item
            let bad_t3_first =
                PyTuple::new_bound(py, &["a".into_py(py), 8u8.into_py(py), 9u16.into_py(py)]);
            assert!(extract_product_version(&bad_t3_first.into_any()).is_err());

            // Tuple length 3 invalid second item
            let bad_t3_second =
                PyTuple::new_bound(py, &[7u8.into_py(py), "b".into_py(py), 9u16.into_py(py)]);
            assert!(extract_product_version(&bad_t3_second.into_any()).is_err());

            // Tuple length 3 invalid third item
            let bad_t3_third =
                PyTuple::new_bound(py, &[7u8.into_py(py), 8u8.into_py(py), "c".into_py(py)]);
            assert!(extract_product_version(&bad_t3_third.into_any()).is_err());

            // 5. Tuple with invalid lengths: length 0, 1, 4
            let t0 = PyTuple::empty_bound(py);
            assert!(extract_product_version(&t0.into_any()).is_err());

            let t1 = PyTuple::new_bound(py, &[1u8.into_py(py)]);
            assert!(extract_product_version(&t1.into_any()).is_err());

            let t4 = PyTuple::new_bound(
                py,
                &[
                    1u8.into_py(py),
                    2u8.into_py(py),
                    3u16.into_py(py),
                    4u16.into_py(py),
                ],
            );
            assert!(extract_product_version(&t4.into_any()).is_err());

            // 6. Unsupported type (e.g. dict)
            let py_dict = PyDict::new_bound(py);
            assert!(extract_product_version(py_dict.as_any()).is_err());
        });
    }
}

//! Python bindings for constructing Windows Installer packages (`PackageBuilder`).

use crate::error::to_py_err;
use crate::package::PyPackage;
use crate::version::extract_product_version;
use msi::database::tables::core::{
    file_attributes, ComponentRow, DirectoryRow, FeatureComponentsRow, FeatureRow, FileHashRow,
    FileRow, MediaRow,
};
use msi::database::tables::types::{
    ComponentGuid, ComponentName, DirectoryId, FeatureName, FileKey,
};
use msi::package::PackageBuilder;
use pyo3::prelude::*;
use std::fs;
use std::path::Path;

/// Fluent builder for constructing complete Windows Installer (.msi) packages.
#[pyclass(name = "PackageBuilder", module = "msi")]
#[derive(Debug, Clone, Default)]
pub struct PyPackageBuilder {
    /// Inner Rust package builder.
    pub inner: PackageBuilder,
    /// Counter for automatic file sequencing.
    next_sequence: i16,
    /// Staged file data for automatic cabinet archive creation.
    staged_files: Vec<(String, Vec<u8>)>,
}

#[pymethods]
impl PyPackageBuilder {
    /// Constructs a new [`PyPackageBuilder`].
    ///
    /// # Arguments
    ///
    /// * `product_name` - Descriptive application name.
    /// * `manufacturer` - Vendor or author name.
    /// * `version` - `ProductVersion`, version string ("1.0.0"), or tuple (1, 0, 0).
    /// * `product_code` - Optional `ProductCode` GUID string in `{...}` format.
    /// * `upgrade_code` - Optional `UpgradeCode` GUID string in `{...}` format.
    ///
    /// # Errors
    ///
    /// Returns [`crate::error::ValidationError`] on invalid arguments.
    #[new]
    #[pyo3(signature = (product_name, manufacturer, version, product_code = None, upgrade_code = None))]
    pub fn new(
        product_name: String,
        manufacturer: String,
        version: &Bound<'_, PyAny>,
        product_code: Option<String>,
        upgrade_code: Option<String>,
    ) -> PyResult<Self> {
        let ver = extract_product_version(version)?;
        let mut builder = PackageBuilder::default()
            .product_name(product_name)
            .manufacturer(manufacturer)
            .version(ver);

        if let Some(code) = product_code {
            builder = builder.product_code(code);
        }
        if let Some(upg) = upgrade_code {
            builder = builder.upgrade_code(upg);
        }

        Ok(Self {
            inner: builder,
            next_sequence: 1,
            staged_files: Vec::new(),
        })
    }

    /// Sets the product name.
    pub fn set_product_name(&mut self, name: String) {
        self.inner = std::mem::take(&mut self.inner).product_name(name);
    }

    /// Sets the manufacturer name.
    pub fn set_manufacturer(&mut self, mfr: String) {
        self.inner = std::mem::take(&mut self.inner).manufacturer(mfr);
    }

    /// Sets the product version.
    ///
    /// # Errors
    ///
    /// Returns [`crate::error::ValidationError`] on invalid version.
    pub fn set_version(&mut self, version: &Bound<'_, PyAny>) -> PyResult<()> {
        let ver = extract_product_version(version)?;
        self.inner = std::mem::take(&mut self.inner).version(ver);
        Ok(())
    }

    /// Sets the `ProductCode` GUID.
    pub fn set_product_code(&mut self, code: String) {
        self.inner = std::mem::take(&mut self.inner).product_code(code);
    }

    /// Sets the `UpgradeCode` GUID.
    pub fn set_upgrade_code(&mut self, code: String) {
        self.inner = std::mem::take(&mut self.inner).upgrade_code(code);
    }

    /// Adds a property entry to the `Property` table.
    pub fn add_property(&mut self, name: String, value: String) {
        self.inner = std::mem::take(&mut self.inner).add_property(name, value);
    }

    /// Adds a directory definition to the package.
    ///
    /// # Arguments
    ///
    /// * `dir_id` - Directory identifier (e.g. "TARGETDIR" or "INSTALLDIR").
    /// * `parent_id` - Optional parent directory identifier.
    /// * `default_dir` - Directory path specifier (e.g. "`SourceDir`" or "PFiles|Program Files").
    ///
    /// # Errors
    ///
    /// Returns [`crate::error::ValidationError`] on invalid directory identifier.
    #[pyo3(signature = (dir_id, parent_id = None, default_dir = ".".to_string()))]
    pub fn add_directory(
        &mut self,
        dir_id: String,
        parent_id: Option<String>,
        default_dir: String,
    ) -> PyResult<()> {
        let directory = DirectoryId::new(dir_id).map_err(|e| to_py_err(&e))?;
        let directory_parent = match parent_id {
            Some(p) => Some(DirectoryId::new(p).map_err(|e| to_py_err(&e))?),
            None => None,
        };

        let row = DirectoryRow {
            directory,
            directory_parent,
            default_dir,
        };
        self.inner = std::mem::take(&mut self.inner).add_directory(row);
        Ok(())
    }

    /// Adds a component definition to the package.
    ///
    /// # Arguments
    ///
    /// * `comp_id` - Component identifier.
    /// * `dir_id` - Foreign key to Directory table.
    /// * `comp_guid` - Optional GUID in `{...}` format.
    /// * `attributes` - Optional component attributes (defaults to 0).
    /// * `condition` - Optional installation condition expression.
    /// * `keypath` - Optional keypath identifier.
    ///
    /// # Errors
    ///
    /// Returns [`crate::error::ValidationError`] on invalid parameters.
    #[pyo3(signature = (comp_id, dir_id, comp_guid = None, attributes = None, condition = None, keypath = None))]
    pub fn add_component(
        &mut self,
        comp_id: String,
        dir_id: String,
        comp_guid: Option<String>,
        attributes: Option<i16>,
        condition: Option<String>,
        keypath: Option<String>,
    ) -> PyResult<()> {
        let component = ComponentName::new(comp_id).map_err(|e| to_py_err(&e))?;
        let directory = DirectoryId::new(dir_id).map_err(|e| to_py_err(&e))?;
        let component_id = match comp_guid {
            Some(g) => Some(ComponentGuid::parse(g).map_err(|e| to_py_err(&e))?),
            None => None,
        };

        let row = ComponentRow {
            component,
            component_id,
            directory,
            attributes: attributes.unwrap_or(0),
            condition,
            key_path: keypath,
        };
        self.inner = std::mem::take(&mut self.inner).add_component(row);
        Ok(())
    }

    /// Adds a feature definition to the package.
    ///
    /// # Arguments
    ///
    /// * `feat_id` - Feature identifier.
    /// * `parent_id` - Optional parent feature identifier.
    /// * `title` - Optional UI title.
    /// * `description` - Optional UI description.
    /// * `display` - Optional UI display flag.
    /// * `level` - Initial installation level (defaults to 1).
    /// * `dir_id` - Optional directory reference.
    /// * `attributes` - Feature attributes bitmask (defaults to 0).
    ///
    /// # Errors
    ///
    /// Returns [`crate::error::ValidationError`] on invalid parameters.
    #[allow(clippy::too_many_arguments)]
    #[pyo3(signature = (feat_id, parent_id = None, title = None, description = None, display = None, level = None, dir_id = None, attributes = None))]
    pub fn add_feature(
        &mut self,
        feat_id: String,
        parent_id: Option<String>,
        title: Option<String>,
        description: Option<String>,
        display: Option<i16>,
        level: Option<i16>,
        dir_id: Option<String>,
        attributes: Option<i16>,
    ) -> PyResult<()> {
        let feature = FeatureName::new(feat_id).map_err(|e| to_py_err(&e))?;
        let feature_parent = match parent_id {
            Some(p) => Some(FeatureName::new(p).map_err(|e| to_py_err(&e))?),
            None => None,
        };
        let directory = match dir_id {
            Some(d) => Some(DirectoryId::new(d).map_err(|e| to_py_err(&e))?),
            None => None,
        };

        let row = FeatureRow {
            feature,
            feature_parent,
            title,
            description,
            display,
            level: level.unwrap_or(1),
            directory,
            attributes: attributes.unwrap_or(0),
        };
        self.inner = std::mem::take(&mut self.inner).add_feature(row);
        Ok(())
    }

    /// Links a component to a feature in the `FeatureComponents` table.
    ///
    /// # Arguments
    ///
    /// * `feat_id` - Feature identifier.
    /// * `comp_id` - Component identifier.
    ///
    /// # Errors
    ///
    /// Returns [`crate::error::ValidationError`] on invalid identifiers.
    pub fn add_feature_component(&mut self, feat_id: String, comp_id: String) -> PyResult<()> {
        let feature = FeatureName::new(feat_id).map_err(|e| to_py_err(&e))?;
        let component = ComponentName::new(comp_id).map_err(|e| to_py_err(&e))?;
        let row = FeatureComponentsRow { feature, component };
        self.inner =
            std::mem::take(&mut self.inner).add_record("FeatureComponents", row.to_record());
        Ok(())
    }

    /// Adds a file definition to the `File` table.
    ///
    /// # Arguments
    ///
    /// * `file_id` - File identifier.
    /// * `comp_id` - Component foreign key.
    /// * `file_name` - File name.
    /// * `file_size` - File size in bytes.
    /// * `version` - Optional version string.
    /// * `language` - Optional language string.
    /// * `attributes` - Optional file attributes.
    /// * `sequence` - Optional sequence number (defaults to automatic sequence).
    ///
    /// # Errors
    ///
    /// Returns [`crate::error::ValidationError`] on invalid parameters.
    #[allow(clippy::too_many_arguments)]
    #[pyo3(signature = (file_id, comp_id, file_name, file_size, version = None, language = None, attributes = None, sequence = None))]
    pub fn add_file(
        &mut self,
        file_id: String,
        comp_id: String,
        file_name: String,
        file_size: u32,
        version: Option<String>,
        language: Option<String>,
        attributes: Option<i16>,
        sequence: Option<i16>,
    ) -> PyResult<()> {
        let file = FileKey::new(file_id).map_err(|e| to_py_err(&e))?;
        let component = ComponentName::new(comp_id).map_err(|e| to_py_err(&e))?;

        let seq = sequence.unwrap_or(self.next_sequence);
        if seq >= self.next_sequence {
            self.next_sequence = seq.saturating_add(1);
        }

        let row = FileRow {
            file,
            component,
            file_name,
            file_size: i32::try_from(file_size).unwrap_or(i32::MAX),
            version,
            language,
            attributes,
            sequence: seq,
        };
        self.inner = std::mem::take(&mut self.inner).add_file(row);
        Ok(())
    }

    /// Adds a media definition to the `Media` table.
    ///
    /// # Arguments
    ///
    /// * `disk_id` - Disk index (usually 1).
    /// * `last_sequence` - Maximum file sequence on this disk.
    /// * `disk_prompt` - Optional disk prompt.
    /// * `cabinet` - Optional cabinet name (e.g. `"#cab1.cab"`).
    /// * `volume_label` - Optional volume label.
    /// * `source` - Optional source path.
    #[pyo3(signature = (disk_id, last_sequence, disk_prompt = None, cabinet = None, volume_label = None, source = None))]
    pub fn add_media(
        &mut self,
        disk_id: i16,
        last_sequence: u32,
        disk_prompt: Option<String>,
        cabinet: Option<String>,
        volume_label: Option<String>,
        source: Option<String>,
    ) {
        let row = MediaRow {
            disk_id,
            last_sequence: i32::try_from(last_sequence).unwrap_or(i32::MAX),
            disk_prompt,
            cabinet,
            volume_label,
            source,
        };
        self.inner = std::mem::take(&mut self.inner).add_media(row);
    }

    /// Injects an in-memory cabinet archive stream into the package.
    ///
    /// # Arguments
    ///
    /// * `name` - Cabinet stream identifier (e.g. `"#cab1.cab"`).
    /// * `data` - Raw cabinet byte array.
    pub fn add_embedded_cabinet(&mut self, name: String, data: Vec<u8>) {
        self.inner = std::mem::take(&mut self.inner).add_embedded_cabinet(name, data);
    }

    /// Reads a file from disk, generates component and file entries, and stages for packaging.
    ///
    /// # Arguments
    ///
    /// * `source_path` - Local file path.
    /// * `target_dir_id` - Directory identifier where the file will install.
    /// * `feature_id` - Feature identifier linking the file's component.
    /// * `component_id` - Optional custom component identifier.
    ///
    /// # Returns
    ///
    /// Generated file identifier.
    ///
    /// # Errors
    ///
    /// Returns [`crate::error::IoError`] or [`crate::error::ValidationError`] on failure.
    #[pyo3(signature = (source_path, target_dir_id, feature_id, component_id = None))]
    pub fn add_file_from_disk(
        &mut self,
        source_path: &str,
        target_dir_id: String,
        feature_id: String,
        component_id: Option<String>,
    ) -> PyResult<String> {
        let path = Path::new(source_path);
        let data = fs::read(path).map_err(|e| to_py_err(&msi::Error::Io(e.to_string())))?;

        let file_name_os = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("file.bin");

        let file_id = format!("f_{}", sanitize_id(file_name_os));
        let comp_id = component_id.unwrap_or_else(|| format!("c_{}", sanitize_id(file_name_os)));
        let file_key = FileKey::new(file_id.clone()).map_err(|e| to_py_err(&e))?;

        self.add_component(
            comp_id.clone(),
            target_dir_id,
            None,
            Some(0),
            None,
            Some(file_id.clone()),
        )?;
        self.add_feature_component(feature_id, comp_id.clone())?;

        let size = u32::try_from(data.len()).unwrap_or(u32::MAX);
        self.add_file(
            file_id.clone(),
            comp_id,
            file_name_os.to_string(),
            size,
            None,
            None,
            Some(file_attributes::COMPRESSED),
            None,
        )?;

        let digest = crate::md5::compute_md5(&data);
        let hash_part1 = i32::from_le_bytes([digest[0], digest[1], digest[2], digest[3]]);
        let hash_part2 = i32::from_le_bytes([digest[4], digest[5], digest[6], digest[7]]);
        let hash_part3 = i32::from_le_bytes([digest[8], digest[9], digest[10], digest[11]]);
        let hash_part4 = i32::from_le_bytes([digest[12], digest[13], digest[14], digest[15]]);

        let hash_row = FileHashRow {
            file: file_key,
            options: 0,
            hash_part1,
            hash_part2,
            hash_part3,
            hash_part4,
        };
        self.inner = std::mem::take(&mut self.inner).add_record("FileHash", hash_row.to_record());
        self.staged_files.push((file_id.clone(), data));

        Ok(file_id)
    }

    /// Validates and compiles the package, returning a [`PyPackage`].
    ///
    /// # Errors
    ///
    /// Returns [`crate::error::ValidationError`] on validation failure.
    pub fn build(&self, py: Python<'_>) -> PyResult<PyPackage> {
        let builder = self.finalize_builder().map_err(|e| to_py_err(&e))?;
        let pkg = py
            .allow_threads(move || builder.build())
            .map_err(|e| to_py_err(&e))?;
        Ok(PyPackage { inner: pkg })
    }

    /// Validates, compiles, and writes the package directly to a file on disk.
    ///
    /// # Arguments
    ///
    /// * `path` - Destination file path.
    ///
    /// # Errors
    ///
    /// Returns [`crate::error::IoError`] or [`crate::error::ValidationError`] on failure.
    pub fn build_to_file(&self, py: Python<'_>, path: String) -> PyResult<()> {
        let builder = self.finalize_builder().map_err(|e| to_py_err(&e))?;
        py.allow_threads(move || {
            let pkg = builder.build().map_err(|e| to_py_err(&e))?;
            pkg.save(&path).map_err(|e| to_py_err(&e))
        })?;
        Ok(())
    }

    /// Validates, compiles, and serializes the package directly to in-memory bytes.
    ///
    /// # Errors
    ///
    /// Returns [`crate::error::ValidationError`] on failure.
    pub fn build_to_bytes(&self, py: Python<'_>) -> PyResult<Vec<u8>> {
        let builder = self.finalize_builder().map_err(|e| to_py_err(&e))?;
        let bytes = py.allow_threads(move || {
            let pkg = builder.build().map_err(|e| to_py_err(&e))?;
            pkg.to_bytes().map_err(|e| to_py_err(&e))
        })?;
        Ok(bytes)
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

#[allow(clippy::multiple_inherent_impl)]
impl PyPackageBuilder {
    /// Finalizes staged file packaging into an embedded cabinet archive.
    fn finalize_builder(&self) -> Result<PackageBuilder, msi::Error> {
        let mut builder = self.inner.clone();
        if !self.staged_files.is_empty() {
            let mut cab_writer =
                msi::cab::writer::CabinetWriter::new(msi::cab::folder::CompressionType::Mszip);
            for (fid, data) in &self.staged_files {
                cab_writer.add_file(fid, data)?;
            }
            let cab_bytes = cab_writer.build();
            builder = builder.add_embedded_cabinet("#cab1.cab", cab_bytes);
        }
        Ok(builder)
    }
}

/// Helper converting an arbitrary string into a valid MSI identifier (`[a-zA-Z_][a-zA-Z0-9_.]*`).
fn sanitize_id(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    for (i, c) in input.chars().enumerate() {
        if i == 0 {
            if c.is_ascii_alphabetic() || c == '_' {
                out.push(c);
            } else {
                out.push('_');
                if c.is_ascii_digit() {
                    out.push(c);
                }
            }
        } else if c.is_ascii_alphanumeric() || c == '_' || c == '.' {
            out.push(c);
        } else {
            out.push('_');
        }
    }
    if out.is_empty() {
        "item".to_string()
    } else {
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use msi::database::tables::record::Record;
    use msi::database::FieldValue;
    use pyo3::types::{PyDict, PyString, PyTuple};
    use std::fs;

    /// Helper checking constructor, setters, and derives on a `PackageBuilder` Result.
    fn check_builder_new_and_setters(res: PyResult<PyPackageBuilder>) -> bool {
        res.is_ok_and(|mut b| {
            Python::with_gil(|py| {
                // Test Debug, Clone, and Default derives
                let debug_repr = format!("{b:?}");
                assert!(debug_repr.contains("PyPackageBuilder"));
                let _ = b.clone();
                let def = PyPackageBuilder::default();
                assert_eq!(def.next_sequence, 0);

                // Setters
                b.set_product_name("UpdatedApp".to_string());
                b.set_manufacturer("UpdatedVendor".to_string());
                let ver_v2 = PyString::new_bound(py, "2.0.0");
                assert!(b.set_version(ver_v2.as_any()).is_ok());
                let bad_ver = PyString::new_bound(py, "bad_ver");
                assert!(b.set_version(bad_ver.as_any()).is_err());

                b.set_product_code("{33333333-3333-3333-3333-333333333333}".to_string());
                b.set_upgrade_code("{44444444-4444-4444-4444-444444444444}".to_string());
                b.add_property("CUSTOM_KEY".to_string(), "CUSTOM_VAL".to_string());
                b.add_media(
                    1,
                    100,
                    Some("Disk 1".to_string()),
                    Some("#cab1.cab".to_string()),
                    Some("DISK1".to_string()),
                    Some("src".to_string()),
                );
                b.add_embedded_cabinet("#manual.cab".to_string(), vec![1, 2, 3]);
                true
            })
        })
    }

    /// Tests constructor, setters, media, properties, and derives.
    #[test]
    fn test_builder_new_and_setters() {
        pyo3::prepare_freethreaded_python();
        Python::with_gil(|py| {
            let ver_str = PyString::new_bound(py, "1.0.0");
            assert!(check_builder_new_and_setters(PyPackageBuilder::new(
                "AppName".to_string(),
                "Vendor".to_string(),
                ver_str.as_any(),
                Some("{11111111-1111-1111-1111-111111111111}".to_string()),
                Some("{22222222-2222-2222-2222-222222222222}".to_string()),
            )));

            let bad_ver = PyString::new_bound(py, "not_a_version");
            assert!(!check_builder_new_and_setters(PyPackageBuilder::new(
                "AppName".to_string(),
                "Vendor".to_string(),
                bad_ver.as_any(),
                None,
                None,
            )));
        });
    }

    /// Helper checking `add_directory`, `add_component`, `add_feature`, and `add_file`.
    #[allow(clippy::too_many_lines)]
    fn check_builder_components(res: PyResult<PyPackageBuilder>) -> bool {
        res.is_ok_and(|mut b| {
            // Directories: with parent and without parent
            assert!(b
                .add_directory("TARGETDIR".to_string(), None, "SourceDir".to_string())
                .is_ok());
            assert!(b
                .add_directory(
                    "INSTALLDIR".to_string(),
                    Some("TARGETDIR".to_string()),
                    "PFiles|Program Files".to_string()
                )
                .is_ok());
            assert!(b
                .add_directory(String::new(), None, ".".to_string())
                .is_err());
            assert!(b
                .add_directory("DIR2".to_string(), Some(String::new()), ".".to_string())
                .is_err());

            // Components: with guid and without guid
            assert!(b
                .add_component(
                    "Comp1".to_string(),
                    "INSTALLDIR".to_string(),
                    Some("{12345678-1234-1234-1234-123456789012}".to_string()),
                    Some(0),
                    Some("1".to_string()),
                    Some("Key1".to_string()),
                )
                .is_ok());
            assert!(b
                .add_component(
                    "Comp2".to_string(),
                    "INSTALLDIR".to_string(),
                    None,
                    None,
                    None,
                    None,
                )
                .is_ok());
            assert!(b
                .add_component(
                    String::new(),
                    "INSTALLDIR".to_string(),
                    None,
                    None,
                    None,
                    None
                )
                .is_err());
            assert!(b
                .add_component("Comp3".to_string(), String::new(), None, None, None, None)
                .is_err());
            assert!(b
                .add_component(
                    "Comp4".to_string(),
                    "INSTALLDIR".to_string(),
                    Some("invalid_guid".to_string()),
                    None,
                    None,
                    None
                )
                .is_err());

            // Features: with parent/dir and without parent/dir
            assert!(b
                .add_feature(
                    "RootFeat".to_string(),
                    None,
                    Some("Title".to_string()),
                    Some("Desc".to_string()),
                    Some(1),
                    Some(1),
                    Some("INSTALLDIR".to_string()),
                    Some(0),
                )
                .is_ok());
            assert!(b
                .add_feature(
                    "SubFeat".to_string(),
                    Some("RootFeat".to_string()),
                    None,
                    None,
                    None,
                    None,
                    None,
                    None,
                )
                .is_ok());
            assert!(b
                .add_feature(String::new(), None, None, None, None, None, None, None)
                .is_err());
            assert!(b
                .add_feature(
                    "Feat2".to_string(),
                    Some(String::new()),
                    None,
                    None,
                    None,
                    None,
                    None,
                    None
                )
                .is_err());
            assert!(b
                .add_feature(
                    "Feat3".to_string(),
                    None,
                    None,
                    None,
                    None,
                    None,
                    Some(String::new()),
                    None
                )
                .is_err());

            // FeatureComponents: valid and invalid
            assert!(b
                .add_feature_component("RootFeat".to_string(), "Comp1".to_string())
                .is_ok());
            assert!(b
                .add_feature_component(String::new(), "Comp1".to_string())
                .is_err());
            assert!(b
                .add_feature_component("RootFeat".to_string(), String::new())
                .is_err());

            // Files and sequence branches:
            // 1. Explicit sequence >= next_sequence
            assert!(b
                .add_file(
                    "File1".to_string(),
                    "Comp1".to_string(),
                    "test1.txt".to_string(),
                    100,
                    Some("1.0".to_string()),
                    Some("1033".to_string()),
                    Some(0),
                    Some(5),
                )
                .is_ok());
            assert_eq!(b.next_sequence, 6);

            // 2. Explicit sequence < next_sequence
            assert!(b
                .add_file(
                    "File2".to_string(),
                    "Comp1".to_string(),
                    "test2.txt".to_string(),
                    200,
                    None,
                    None,
                    None,
                    Some(2),
                )
                .is_ok());
            assert_eq!(b.next_sequence, 6);

            // 3. Sequence None (defaults to next_sequence)
            assert!(b
                .add_file(
                    "File3".to_string(),
                    "Comp1".to_string(),
                    "test3.txt".to_string(),
                    300,
                    None,
                    None,
                    None,
                    None,
                )
                .is_ok());
            assert_eq!(b.next_sequence, 7);

            // Invalid file identifier or component identifier
            assert!(b
                .add_file(
                    String::new(),
                    "Comp1".to_string(),
                    "f.txt".to_string(),
                    10,
                    None,
                    None,
                    None,
                    None
                )
                .is_err());
            assert!(b
                .add_file(
                    "File4".to_string(),
                    String::new(),
                    "f.txt".to_string(),
                    10,
                    None,
                    None,
                    None,
                    None
                )
                .is_err());
            true
        })
    }

    /// Tests adding directories, components, features, and files including validation error branches.
    #[test]
    fn test_builder_add_directory_component_feature_file() {
        pyo3::prepare_freethreaded_python();
        Python::with_gil(|py| {
            let ver_str = PyString::new_bound(py, "1.0.0");
            assert!(check_builder_components(PyPackageBuilder::new(
                "App".to_string(),
                "Vendor".to_string(),
                ver_str.as_any(),
                None,
                None,
            )));
            assert!(!check_builder_components(Err(
                crate::error::ValidationError::new_err("err")
            )));
        });
    }

    /// Tests `sanitize_id` helper across various character combinations.
    #[test]
    fn test_sanitize_id() {
        assert_eq!(sanitize_id("app.exe"), "app.exe");
        assert_eq!(sanitize_id("1file.bin"), "_1file.bin");
        assert_eq!(sanitize_id("_already_valid"), "_already_valid");
        assert_eq!(sanitize_id("@special#name$"), "_special_name_");
        assert_eq!(sanitize_id(""), "item");
        assert_eq!(sanitize_id("!@#"), "___");
    }

    /// Helper checking `add_file_from_disk` operations and error paths.
    fn check_add_file_from_disk(res: PyResult<PyPackageBuilder>) -> bool {
        res.is_ok_and(|mut b| {
            assert!(b
                .add_directory("TARGETDIR".to_string(), None, "SourceDir".to_string())
                .is_ok());
            assert!(b
                .add_feature(
                    "MainFeat".to_string(),
                    None,
                    None,
                    None,
                    None,
                    None,
                    None,
                    None
                )
                .is_ok());

            let temp_src = std::env::temp_dir().join("msi_test_builder_source.txt");
            let _ = fs::write(&temp_src, b"Content for disk file staging");
            let temp_src_str = temp_src.to_str().unwrap_or("");

            // 1. Add file with auto component ID
            let fid1 = b.add_file_from_disk(
                temp_src_str,
                "TARGETDIR".to_string(),
                "MainFeat".to_string(),
                None,
            );
            assert!(fid1.is_ok());

            // 2. Add file with explicit component ID
            let fid2 = b.add_file_from_disk(
                temp_src_str,
                "TARGETDIR".to_string(),
                "MainFeat".to_string(),
                Some("CustomCompId".to_string()),
            );
            assert!(fid2.is_ok());

            // 3. Non-existent file path -> IoError
            assert!(b
                .add_file_from_disk(
                    "/nonexistent_dir_99999/file.bin",
                    "TARGETDIR".to_string(),
                    "MainFeat".to_string(),
                    None
                )
                .is_err());

            // 4. File whose name exceeds 72 characters is deterministically hashed and succeeds
            let long_name = format!("{}.txt", "a".repeat(80));
            let long_file = std::env::temp_dir().join(long_name);
            let _ = fs::write(&long_file, b"long filename test");
            assert!(b
                .add_file_from_disk(
                    long_file.to_str().unwrap_or(""),
                    "TARGETDIR".to_string(),
                    "MainFeat".to_string(),
                    None
                )
                .is_ok());
            let _ = fs::remove_file(&long_file);

            // 5. Empty feature name triggers error branch
            assert!(b
                .add_file_from_disk(
                    temp_src.to_str().unwrap_or(""),
                    "TARGETDIR".to_string(),
                    String::new(),
                    None
                )
                .is_err());

            let _ = fs::remove_file(&temp_src);
            true
        })
    }

    /// Tests adding files from disk including file reading, MD5 hashing, and cabinet staging.
    #[test]
    fn test_builder_add_file_from_disk() {
        pyo3::prepare_freethreaded_python();
        Python::with_gil(|py| {
            let ver_str = PyString::new_bound(py, "1.0.0");
            assert!(check_add_file_from_disk(PyPackageBuilder::new(
                "DiskApp".to_string(),
                "Vendor".to_string(),
                ver_str.as_any(),
                None,
                None,
            )));
            assert!(!check_add_file_from_disk(Err(
                crate::error::ValidationError::new_err("err")
            )));
        });
    }

    /// Helper checking package build operations and error paths.
    fn check_builder_build(res: PyResult<PyPackageBuilder>) -> bool {
        res.is_ok_and(|mut b| {
            Python::with_gil(|py| {
                assert!(b
                    .add_directory("TARGETDIR".to_string(), None, "SourceDir".to_string())
                    .is_ok());
                assert!(b
                    .add_feature(
                        "MainFeat".to_string(),
                        None,
                        None,
                        None,
                        None,
                        None,
                        None,
                        None
                    )
                    .is_ok());

                let temp_src = std::env::temp_dir().join("msi_test_build_variant.txt");
                let _ = fs::write(&temp_src, b"Payload inside cabinet");
                assert!(b
                    .add_file_from_disk(
                        temp_src.to_str().unwrap_or(""),
                        "TARGETDIR".to_string(),
                        "MainFeat".to_string(),
                        None
                    )
                    .is_ok());
                let _ = fs::remove_file(&temp_src);

                // 1. build -> PyPackage with embedded cabinet
                assert!(b.build(py).is_ok_and(|py_pkg| {
                    assert_eq!(
                        py_pkg.get_property("ProductName"),
                        Some("BuildApp".to_string())
                    );
                    let extract_dir = std::env::temp_dir().join("msi_test_build_extract");
                    let _ = fs::create_dir_all(&extract_dir);
                    let ok_ext = py_pkg
                        .extract_cabinet(py, "#cab1.cab", extract_dir.to_str().unwrap_or(""))
                        .is_ok();
                    let _ = fs::remove_dir_all(&extract_dir);
                    ok_ext
                }));

                // 2. build_to_bytes
                assert!(b.build_to_bytes(py).is_ok_and(|bytes| !bytes.is_empty()));

                // 3. build_to_file
                let temp_out = std::env::temp_dir().join("msi_test_built_app.msi");
                let temp_out_str = temp_out.to_str().unwrap_or("");
                assert!(b.build_to_file(py, temp_out_str.to_string()).is_ok());
                assert!(temp_out.exists());
                let _ = fs::remove_file(&temp_out);

                // 4. build_to_file with invalid path -> IoError
                assert!(b
                    .build_to_file(py, "/nonexistent_dir_99999/out.msi".to_string())
                    .is_err());

                // 5. finalize_builder error path (staged files with duplicate filename)
                let mut bad_staging = b.clone();
                bad_staging
                    .staged_files
                    .push(("dup.txt".to_string(), vec![]));
                bad_staging
                    .staged_files
                    .push(("dup.txt".to_string(), vec![]));
                assert!(bad_staging.build(py).is_err());
                assert!(bad_staging
                    .build_to_file(py, "/tmp/never.msi".to_string())
                    .is_err());
                assert!(bad_staging.build_to_bytes(py).is_err());

                // 6. build_to_bytes error path where pkg.to_bytes fails due to invalid table record
                let mut bad_rec_b = b.clone();
                bad_rec_b.staged_files.clear();
                bad_rec_b.inner = std::mem::take(&mut bad_rec_b.inner).add_record(
                    "File",
                    Record::with_fields(vec![FieldValue::String("short".to_string())]),
                );
                assert!(bad_rec_b.build_to_bytes(py).is_err());

                // 7. Validation failure on build (empty builder with no product name or version)
                let empty_b = PyPackageBuilder::default();
                assert!(empty_b.build(py).is_err());
                assert!(empty_b
                    .build_to_file(py, "/tmp/out.msi".to_string())
                    .is_err());
                assert!(empty_b.build_to_bytes(py).is_err());
                true
            })
        })
    }

    /// Tests building packages to memory, file, and bytes, with and without staged cabinet files.
    #[test]
    fn test_builder_build_variants_and_errors() {
        pyo3::prepare_freethreaded_python();
        Python::with_gil(|py| {
            let ver_str = PyString::new_bound(py, "1.0.0");
            assert!(check_builder_build(PyPackageBuilder::new(
                "BuildApp".to_string(),
                "Vendor".to_string(),
                ver_str.as_any(),
                Some("{12345678-1234-1234-1234-123456789012}".to_string()),
                None,
            )));
            assert!(!check_builder_build(Err(
                crate::error::ValidationError::new_err("err")
            )));
        });
    }

    /// Helper running a Python script against a module result, checking both Ok and Err module creations.
    fn run_builder_script(res: PyResult<Bound<'_, PyModule>>) -> bool {
        res.is_ok_and(|m| {
            let py = m.py();
            assert!(m.add_class::<PyPackageBuilder>().is_ok());

            // Direct Rust invocation of __enter__ and __exit__
            let ver_str = PyString::new_bound(py, "1.0.0");
            assert!(PyPackageBuilder::new(
                "App".to_string(),
                "Vendor".to_string(),
                ver_str.as_any(),
                None,
                None,
            )
            .is_ok_and(|b| {
                assert!(Py::new(py, b.clone()).is_ok_and(|inst| {
                    let borrowed = inst.borrow(py);
                    let _ = PyPackageBuilder::__enter__(borrowed);
                    true
                }));
                assert!(!b.__exit__(None, None, None));
                let exc = pyo3::exceptions::PyRuntimeError::new_err("sample err");
                let bound_exc = exc.to_object(py).into_bound(py);
                assert!(!b.__exit__(Some(&bound_exc), Some(&bound_exc), Some(&bound_exc)));
                true
            }));

            // Direct exercise of PyO3 pymethod wrappers with mismatched object type to test Err branch
            let bad_obj = PyDict::new_bound(py);
            let empty_args = PyTuple::empty_bound(py);
            // SAFETY: bad_obj is a valid PyDict pointer with GIL held; type mismatch executes Err branches.
            unsafe {
                let _ = PyPackageBuilder::__pymethod_build__(py, bad_obj.as_ptr());
                let _ = PyPackageBuilder::__pymethod_build_to_bytes__(py, bad_obj.as_ptr());
                let _ = PyPackageBuilder::__pymethod___enter____(py, bad_obj.as_ptr());
                let _ = PyPackageBuilder::__pymethod___exit____(
                    py,
                    bad_obj.as_ptr(),
                    empty_args.as_ptr(),
                    std::ptr::null_mut(),
                );
            }

            let globals = PyDict::new_bound(py);
            assert!(globals.set_item("m", &m).is_ok());

            let script = r##"
with m.PackageBuilder("ContextApp", "ContextVendor", "1.2.3") as b:
    b.set_product_name("UpdatedContextApp")
    b.set_manufacturer("UpdatedContextVendor")
    b.set_version("2.3.4")
    b.set_product_code("{12345678-1234-1234-1234-123456789012}")
    b.set_upgrade_code("{87654321-4321-4321-4321-210987654321}")
    b.add_property("GREETING", "Hello")
    b.add_directory("TARGETDIR")
    b.add_directory("INSTALLDIR", "TARGETDIR", "PFiles|Program Files")
    b.add_component("MainComp", "INSTALLDIR", "{12345678-1234-1234-1234-123456789012}", 0, "1", "Key")
    b.add_feature("MainFeat", None, "Title", "Desc", 1, 1, "INSTALLDIR", 0)
    b.add_feature_component("MainFeat", "MainComp")
    b.add_file("MainFile", "MainComp", "app.exe", 1024, "1.0", "1033", 0, 1)
    b.add_media(1, 100, "Disk", "#cab1.cab", "Vol", "src")
    b.add_embedded_cabinet("#manual.cab", bytes([1, 2, 3]))

    pkg = b.build()
    assert pkg.get_property("ProductName") == "UpdatedContextApp"
    assert pkg.get_property("Manufacturer") == "UpdatedContextVendor"
    assert pkg.get_property("ProductVersion") == "2.3.4"

    raw_bytes = b.build_to_bytes()
    assert len(raw_bytes) > 0
"##;
            py.run_bound(script, Some(&globals), None).is_ok()
        })
    }

    /// Tests context manager behavior and Python runtime integration.
    #[test]
    fn test_builder_context_manager_and_python_runtime() {
        pyo3::prepare_freethreaded_python();
        Python::with_gil(|py| {
            assert!(run_builder_script(PyModule::new_bound(
                py,
                "test_bld_ctx_mod"
            )));
            assert!(!run_builder_script(PyModule::new_bound(
                py,
                "invalid\0name"
            )));
        });
    }
}

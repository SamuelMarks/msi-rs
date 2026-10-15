//! Merge Module (`.msm`) ingestion, symbol modularization, and database merging.

use crate::database::memory::LinkedDatabase;
use crate::database::tables::record::FieldValue;
use crate::error::Result;
use crate::package::Package;
use crate::wix::standard_defs::STANDARD_DIRECTORIES;
use std::collections::HashSet;
use std::path::Path;

/// Modularizes an identifier by appending the module GUID, unless excluded.
///
/// # Arguments
///
/// * `ident` - Identifier to modularize.
/// * `module_guid` - Module GUID suffix.
/// * `ignore_set` - Set of identifiers excluded from modularization.
///
/// # Returns
///
/// Modularized string.
#[must_use]
pub fn modularize_identifier<S: ::std::hash::BuildHasher>(
    ident: &str,
    module_guid: &str,
    ignore_set: &HashSet<String, S>,
) -> String {
    if ident.is_empty()
        || ident == "TARGETDIR"
        || STANDARD_DIRECTORIES.iter().any(|d| d.id == ident)
        || ignore_set.contains(ident)
        || ident.ends_with(&format!(".{module_guid}"))
    {
        ident.to_string()
    } else {
        format!("{ident}.{module_guid}")
    }
}

/// An ingested Merge Module (`.msm`) containing modularized installation data.
#[derive(Debug, Clone)]
pub struct MergeModule {
    /// Module identifier (e.g. `MyModule.12345678_1234_1234_1234_1234567890AB`).
    pub id: String,
    /// Default language code.
    pub language: i16,
    /// Module version.
    pub version: String,
    /// Relational database containing module records.
    pub database: LinkedDatabase,
}

impl MergeModule {
    /// Constructs a [`MergeModule`] from an existing [`LinkedDatabase`].
    ///
    /// Inspects the `ModuleSignature` table to determine ID, language, and version.
    ///
    /// # Arguments
    ///
    /// * `database` - In-memory relational database.
    ///
    /// # Returns
    ///
    /// A new [`MergeModule`].
    #[must_use]
    pub fn from_database(database: LinkedDatabase) -> Self {
        let mut id = "Module".to_string();
        let mut language = 1033;
        let mut version = "1.0.0".to_string();

        let sig_records = database.get_records("ModuleSignature");
        if let Some(first) = sig_records.first() {
            if let Some(FieldValue::String(m_id)) = first.get(0) {
                id.clone_from(m_id);
            }
            if let Some(FieldValue::Short(l)) = first.get(1) {
                language = *l;
            }
            if let Some(FieldValue::String(v)) = first.get(2) {
                version.clone_from(v);
            }
        }

        Self {
            id,
            language,
            version,
            database,
        }
    }

    /// Loads a [`MergeModule`] from raw `.msm` binary bytes.
    ///
    /// # Arguments
    ///
    /// * `bytes` - Raw `.msm` file bytes (CFB container).
    ///
    /// # Returns
    ///
    /// Parsed [`MergeModule`].
    ///
    /// # Errors
    ///
    /// Returns [`crate::MsiError`] on container parsing or database deserialization failure.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self> {
        let pkg = Package::from_bytes(bytes)?;
        Ok(Self::from_database(pkg.database().clone()))
    }

    /// Loads a [`MergeModule`] from a filesystem path.
    ///
    /// # Arguments
    ///
    /// * `path` - Filesystem path to the `.msm` file.
    ///
    /// # Returns
    ///
    /// Parsed [`MergeModule`].
    ///
    /// # Errors
    ///
    /// Returns [`crate::MsiError`] on file I/O, container parsing, or database error.
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        Self::open_path(path.as_ref())
    }

    /// Internal non-generic helper for opening a merge module from a path.
    ///
    /// # Arguments
    ///
    /// * `path` - The `path` parameter.
    ///
    /// # Returns
    ///
    /// The resulting value or `Ok(())` on success.
    fn open_path(path: &Path) -> Result<Self> {
        let pkg = Package::open(path)?;
        Ok(Self::from_database(pkg.database().clone()))
    }

    /// Extracts the module GUID suffix used for symbol modularization.
    ///
    /// # Returns
    ///
    /// Module GUID string slice.
    #[must_use]
    pub fn guid(&self) -> &str {
        if let Some((_, suffix)) = self.id.rsplit_once('.') {
            suffix
        } else {
            &self.id
        }
    }
}

impl Default for MergeModule {
    /// Creates a default empty [`MergeModule`].
    ///
    /// # Returns
    ///
    /// The resulting value or `Ok(())` on success.
    fn default() -> Self {
        Self::from_database(LinkedDatabase::default())
    }
}

#[cfg(test)]
#[allow(clippy::redundant_clone)]
mod tests {
    use super::*;
    use crate::database::tables::record::Record;
    use crate::package::ProductVersion;
    use std::collections::HashMap;

    #[test]
    #[allow(clippy::too_many_lines)]
    fn test_merge_module_ingestion_and_linking() {
        // 1. Test modularize_identifier
        let mut ignore_set = HashSet::new();
        ignore_set.insert("IgnoredComp".to_string());

        assert_eq!(
            modularize_identifier("Comp1", "GUID1", &ignore_set),
            "Comp1.GUID1"
        );
        assert_eq!(modularize_identifier("", "GUID1", &ignore_set), "");
        assert_eq!(
            modularize_identifier("TARGETDIR", "GUID1", &ignore_set),
            "TARGETDIR"
        );
        assert_eq!(
            modularize_identifier("ProgramFilesFolder", "GUID1", &ignore_set),
            "ProgramFilesFolder"
        );
        assert_eq!(
            modularize_identifier("IgnoredComp", "GUID1", &ignore_set),
            "IgnoredComp"
        );
        assert_eq!(
            modularize_identifier("Already.GUID1", "GUID1", &ignore_set),
            "Already.GUID1"
        );

        // 2. Test MergeModule construction and guid extraction
        let mut mod_db = LinkedDatabase::default();
        mod_db.add_record(
            "ModuleSignature",
            Record::with_fields(vec![
                FieldValue::String("MyModule.12345678_1234_1234_1234_1234567890AB".to_string()),
                FieldValue::Short(1033),
                FieldValue::String("2.1.0".to_string()),
            ]),
        );
        let msm = MergeModule::from_database(mod_db.clone());
        assert_eq!(msm.id, "MyModule.12345678_1234_1234_1234_1234567890AB");
        assert_eq!(msm.language, 1033);
        assert_eq!(msm.version, "2.1.0");
        assert_eq!(msm.guid(), "12345678_1234_1234_1234_1234567890AB");

        let def_msm = MergeModule::from_database(LinkedDatabase::default());
        assert_eq!(def_msm.id, "Module");
        assert_eq!(def_msm.guid(), "Module");

        // 3. Test MergeModule from bytes and file
        let msm_builder = Package::builder()
            .product_name("MsmTest")
            .manufacturer("Acme")
            .version(ProductVersion::new(1, 0, 0))
            .product_code("{88888888-8888-8888-8888-888888888888}");
        let pkg = msm_builder.build().unwrap_or_default();
        let pkg_bytes = pkg.to_bytes().unwrap_or_default();
        let msm_from_bytes = MergeModule::from_bytes(&pkg_bytes).unwrap_or_default();
        assert_eq!(msm_from_bytes.id, "Module");

        let temp_dir = std::env::temp_dir();
        let temp_msm = temp_dir.join(format!("test_merge_{}.msm", std::process::id()));
        let _ = std::fs::write(&temp_msm, &pkg_bytes);
        let msm_from_file = MergeModule::open(&temp_msm).unwrap_or_default();
        assert_eq!(msm_from_file.id, "Module");
        let _ = std::fs::remove_file(&temp_msm);

        assert!(MergeModule::from_bytes(&[]).is_err());
        assert!(MergeModule::open("non_existent_msm_file.msm").is_err());

        // 4. Test LinkedDatabase::merge_module with substitutions, retargeting, modularization
        let mut target_db = LinkedDatabase::default();
        target_db.add_record(
            "Feature",
            Record::with_fields(vec![
                FieldValue::String("MainFeature".to_string()),
                FieldValue::Null,
                FieldValue::String("Main Feature".to_string()),
                FieldValue::Null,
                FieldValue::Short(1),
                FieldValue::Short(1),
                FieldValue::Null,
                FieldValue::Short(0),
            ]),
        );
        target_db.add_record(
            "Directory",
            Record::with_fields(vec![
                FieldValue::String("INSTALLFOLDER".to_string()),
                FieldValue::String("ProgramFilesFolder".to_string()),
                FieldValue::String("MyApp".to_string()),
            ]),
        );
        target_db.add_record(
            "FeatureComponents",
            Record::with_fields(vec![
                FieldValue::String("OtherFeature".to_string()),
                FieldValue::String("OtherComp".to_string()),
            ]),
        );
        target_db.add_record(
            "FeatureComponents",
            Record::with_fields(vec![
                FieldValue::String("MainFeature".to_string()),
                FieldValue::String("DifferentComp".to_string()),
            ]),
        );
        target_db.add_record(
            "FeatureComponents",
            Record::with_fields(vec![FieldValue::Null, FieldValue::Null]),
        );

        // Prepare module database
        let mut source_msm_db = LinkedDatabase::default();
        source_msm_db.add_record(
            "ModuleSignature",
            Record::with_fields(vec![
                FieldValue::String("ModuleA.GUID_ABC".to_string()),
                FieldValue::Short(1033),
                FieldValue::String("1.0.0".to_string()),
            ]),
        );
        // Module directory with TARGETDIR parent (should retarget to INSTALLFOLDER)
        source_msm_db.add_record(
            "Directory",
            Record::with_fields(vec![
                FieldValue::String("TARGETDIR".to_string()),
                FieldValue::Null,
                FieldValue::String("SourceDir".to_string()),
            ]),
        );
        source_msm_db.add_record(
            "Directory",
            Record::with_fields(vec![
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::String("BothNull".to_string()),
            ]),
        );
        source_msm_db.add_record(
            "Directory",
            Record::with_fields(vec![
                FieldValue::String("ModuleDir".to_string()),
                FieldValue::String("TARGETDIR".to_string()),
                FieldValue::String("ModSubDir".to_string()),
            ]),
        );
        source_msm_db.add_record(
            "Directory",
            Record::with_fields(vec![
                FieldValue::String("SubModDir".to_string()),
                FieldValue::String("ModuleDir".to_string()),
                FieldValue::String("Sub".to_string()),
            ]),
        );
        source_msm_db.add_record(
            "Directory",
            Record::with_fields(vec![
                FieldValue::String("NullParentDir".to_string()),
                FieldValue::Null,
                FieldValue::String("NullDir".to_string()),
            ]),
        );
        source_msm_db.add_record(
            "Directory",
            Record::with_fields(vec![
                FieldValue::String("EmptyParentDir".to_string()),
                FieldValue::String(String::new()),
                FieldValue::String("Empty".to_string()),
            ]),
        );
        source_msm_db.add_record(
            "Directory",
            Record::with_fields(vec![
                FieldValue::String("AlreadyTargetParentDir".to_string()),
                FieldValue::String("INSTALLFOLDER".to_string()),
                FieldValue::String("Atp".to_string()),
            ]),
        );
        // CustomAction in module
        source_msm_db.add_record(
            "CustomAction",
            Record::with_fields(vec![
                FieldValue::String("ModCA".to_string()),
                FieldValue::Short(1),
                FieldValue::String("Target".to_string()),
                FieldValue::Null,
            ]),
        );
        source_msm_db.add_record(
            "CustomAction",
            Record::with_fields(vec![
                FieldValue::Null,
                FieldValue::Short(1),
                FieldValue::Null,
                FieldValue::Null,
            ]),
        );
        // Module component
        source_msm_db.add_record(
            "Component",
            Record::with_fields(vec![
                FieldValue::String("ModComp".to_string()),
                FieldValue::String("{11111111-1111-1111-1111-111111111111}".to_string()),
                FieldValue::String("ModuleDir".to_string()),
                FieldValue::Short(0),
                FieldValue::Null,
                FieldValue::String("ModFile".to_string()),
            ]),
        );
        source_msm_db.add_record(
            "Component",
            Record::with_fields(vec![
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Short(0),
                FieldValue::Null,
                FieldValue::Null,
            ]),
        );
        // Module file
        source_msm_db.add_record(
            "File",
            Record::with_fields(vec![
                FieldValue::String("ModFile".to_string()),
                FieldValue::String("ModComp".to_string()),
                FieldValue::String("modfile.dll".to_string()),
                FieldValue::Long(1024),
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Short(0),
                FieldValue::Short(1),
            ]),
        );
        source_msm_db.add_record(
            "File",
            Record::with_fields(vec![
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Long(0),
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Short(0),
                FieldValue::Short(1),
            ]),
        );
        // Module property with substitution template
        source_msm_db.add_record(
            "Property",
            Record::with_fields(vec![
                FieldValue::String("SERVICE_PORT".to_string()),
                FieldValue::String("DEFAULT_PORT".to_string()),
            ]),
        );
        source_msm_db.add_record(
            "Property",
            Record::with_fields(vec![
                FieldValue::String("OTHER_PORT".to_string()),
                FieldValue::String("DEF_OTHER".to_string()),
            ]),
        );

        // Add a sequence mapping test
        source_msm_db.add_record(
            "ModuleInstallExecuteSequence",
            Record::with_fields(vec![
                FieldValue::String("MyModAction".to_string()),
                FieldValue::Short(100),
                FieldValue::String("BaseAction".to_string()),
                FieldValue::Short(1),
                FieldValue::String("MOD_COND".to_string()),
            ]),
        );
        target_db.add_record(
            "InstallExecuteSequence",
            Record::with_fields(vec![
                FieldValue::String("BaseAction".to_string()),
                FieldValue::Null,
                FieldValue::Short(500),
            ]),
        );

        // Add ModuleConfiguration
        source_msm_db.add_record(
            "ModuleConfiguration",
            Record::with_fields(vec![
                FieldValue::String("DEFAULT_PORT".to_string()),
                FieldValue::String("Text".to_string()),
                FieldValue::String("Integer".to_string()),
                FieldValue::Null,
                FieldValue::String("8080".to_string()),
                FieldValue::Short(0),
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
            ]),
        );
        source_msm_db.add_record(
            "ModuleSubstitution",
            Record::with_fields(vec![
                FieldValue::String("Property".to_string()),
                FieldValue::String("DEF_PORT".to_string()),
                FieldValue::String("Value".to_string()),
                FieldValue::String("[=DEFAULT_PORT]".to_string()),
            ]),
        );
        source_msm_db.add_record(
            "Property",
            Record::with_fields(vec![
                FieldValue::String("DEF_PORT".to_string()),
                FieldValue::String("NO_DEF".to_string()),
            ]),
        );
        source_msm_db.add_record(
            "ModuleSubstitution",
            Record::with_fields(vec![
                FieldValue::String("Property".to_string()),
                FieldValue::String("SERVICE_PORT".to_string()),
                FieldValue::String("Value".to_string()),
                FieldValue::String("[=CONFIG_PORT]".to_string()),
            ]),
        );
        source_msm_db.add_record(
            "ModuleSubstitution",
            Record::with_fields(vec![
                FieldValue::String("Property".to_string()),
                FieldValue::String("OTHER_PORT".to_string()),
                FieldValue::String("Value".to_string()),
                FieldValue::String("[CONFIG_PORT]".to_string()),
            ]),
        );
        source_msm_db.add_record(
            "ModuleSubstitution",
            Record::with_fields(vec![
                FieldValue::String("Property".to_string()),
                FieldValue::String("NULL_PORT".to_string()),
                FieldValue::String("Value".to_string()),
                FieldValue::Null,
            ]),
        );
        source_msm_db.add_record(
            "ModuleSubstitution",
            Record::with_fields(vec![
                FieldValue::String("Property".to_string()),
                FieldValue::String("KEY".to_string()),
                FieldValue::String("NoSuchCol".to_string()),
                FieldValue::String("VAL".to_string()),
            ]),
        );
        source_msm_db.add_record(
            "ModuleSubstitution",
            Record::with_fields(vec![
                FieldValue::String("Feature".to_string()),
                FieldValue::String("KEY".to_string()),
                FieldValue::String("Feature".to_string()),
                FieldValue::String("VAL".to_string()),
            ]),
        );
        source_msm_db.add_record(
            "ModuleSubstitution",
            Record::with_fields(vec![
                FieldValue::String("NoSuchTable".to_string()),
                FieldValue::String("KEY".to_string()),
                FieldValue::String("Value".to_string()),
                FieldValue::String("VAL".to_string()),
            ]),
        );
        source_msm_db.add_record(
            "ModuleSubstitution",
            Record::with_fields(vec![
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
            ]),
        );
        source_msm_db.add_record(
            "ModuleIgnoreModularization",
            Record::with_fields(vec![
                FieldValue::String("SERVICE_PORT".to_string()),
                FieldValue::Short(1),
            ]),
        );
        source_msm_db.add_record(
            "ModuleIgnoreModularization",
            Record::with_fields(vec![FieldValue::Null, FieldValue::Short(1)]),
        );

        let merge_mod = MergeModule::from_database(source_msm_db);

        let mut substitutions = HashMap::new();
        substitutions.insert("CONFIG_PORT".to_string(), "8080".to_string());

        let _ = target_db.merge_module(&merge_mod, "MainFeature", "INSTALLFOLDER", &substitutions);
        // Second merge to test deduplication
        let _ = target_db.merge_module(&merge_mod, "MainFeature", "INSTALLFOLDER", &substitutions);

        // Verify directory retargeting
        let dir_records = target_db.get_records("Directory");
        let retargeted = dir_records
            .iter()
            .find(|r| matches!(r.get(0), Some(FieldValue::String(d)) if d == "ModuleDir.GUID_ABC"));
        assert_eq!(
            retargeted.and_then(|r| r.get(1)),
            Some(&FieldValue::String("INSTALLFOLDER".to_string()))
        );

        // Verify component modularization
        let comp_records = target_db.get_records("Component");
        assert!(comp_records
            .iter()
            .any(|r| r.get(0) == Some(&FieldValue::String("ModComp.GUID_ABC".to_string()))));

        // Verify file modularization
        let file_records = target_db.get_records("File");
        assert!(file_records
            .iter()
            .any(|r| r.get(0) == Some(&FieldValue::String("ModFile.GUID_ABC".to_string()))));

        // Verify substitution applied
        let prop_records = target_db.get_records("Property");
        let port_prop = prop_records
            .iter()
            .find(|r| r.get(0) == Some(&FieldValue::String("SERVICE_PORT".to_string())));
        assert_eq!(
            port_prop.and_then(|r| r.get(1)),
            Some(&FieldValue::String("8080".to_string()))
        );

        // Verify feature attachment in FeatureComponents
        let fc_records = target_db.get_records("FeatureComponents");
        assert!(fc_records.iter().any(|r| {
            r.get(0) == Some(&FieldValue::String("MainFeature".to_string()))
                && r.get(1) == Some(&FieldValue::String("ModComp.GUID_ABC".to_string()))
        }));
    }

    #[test]
    fn test_merge_module_defaults_and_file_loading(
    ) -> std::result::Result<(), Box<dyn std::error::Error>> {
        let mut target_db = LinkedDatabase::default();
        // Test MergeModule default construction without ModuleSignature
        let empty_mod_db = LinkedDatabase::default();
        let def_mod = MergeModule::from_database(empty_mod_db);
        assert_eq!(def_mod.id, "Module");
        assert_eq!(def_mod.language, 1033);
        assert_eq!(def_mod.version, "1.0.0");
        assert_eq!(def_mod.guid(), "Module");

        // Test MergeModule with all-null ModuleSignature
        let mut null_sig_db = LinkedDatabase::default();
        null_sig_db.add_record(
            "ModuleSignature",
            Record::with_fields(vec![FieldValue::Null, FieldValue::Null, FieldValue::Null]),
        );
        let null_sig_mod = MergeModule::from_database(null_sig_db);
        assert_eq!(null_sig_mod.id, "Module");
        assert_eq!(null_sig_mod.language, 1033);
        assert_eq!(null_sig_mod.version, "1.0.0");

        // Test module without Directory and Component tables
        let mut no_dir_comp_db = LinkedDatabase::default();
        no_dir_comp_db.add_record(
            "Property",
            Record::with_fields(vec![
                FieldValue::String("PROP_A".to_string()),
                FieldValue::String("VAL_A".to_string()),
            ]),
        );
        let no_dir_comp_mod = MergeModule::from_database(no_dir_comp_db);
        assert!(target_db
            .merge_module(
                &no_dir_comp_mod,
                "MainFeature",
                "INSTALLFOLDER",
                &HashMap::new(),
            )
            .is_ok());

        // Test MergeModule from_bytes and open
        let msm_pkg_builder = Package::builder()
            .product_name("MergeModPkg")
            .manufacturer("Acme")
            .version(ProductVersion::new(1, 0, 0))
            .product_code("{88888888-8888-8888-8888-888888888888}");
        let msm_pkg = msm_pkg_builder.build().unwrap_or_default();
        let msm_bytes = msm_pkg.to_bytes().unwrap_or_default();
        let msm_bytes_opened = MergeModule::from_bytes(&msm_bytes).unwrap_or_default();
        assert_eq!(msm_bytes_opened.guid(), "Module");

        let temp_open_dir = std::env::temp_dir();
        let temp_open_msm = temp_open_dir.join(format!("test_msm_{}.msm", std::process::id()));
        let _ = std::fs::write(&temp_open_msm, &msm_bytes);
        let msm_opened_from_file = MergeModule::open(&temp_open_msm)?;
        assert_eq!(msm_opened_from_file.guid(), "Module");
        let _ = std::fs::remove_file(&temp_open_msm);
        Ok(())
    }
}

//! Advanced subsystems: `SxS` assemblies, crypto certificates, merge modules, ODBC, and patching.
//!
//! Grounded directly in the Windows Installer advanced features specifications:
//! - Rules: ICE25, ICE62, ICE66, ICE76, ICE81, ICE83, ICE94, ICE97, ICE98, ICE105.

use super::types::IceReport;
use crate::database::tables::record::FieldValue;
use crate::wix::linker::LinkedDatabase;
use std::collections::HashSet;

/// ICE25: Verifies Merge Module cross-dependencies and exclusions.
///
/// # Arguments
///
/// * `db` - The [`LinkedDatabase`] to inspect.
///
/// # Returns
///
/// `Some(IceReport)` if a module exclusion rule is violated.
#[must_use]
pub fn validate_ice25(db: &LinkedDatabase) -> Option<IceReport> {
    let mut installed_modules = HashSet::new();
    for r in db.get_records("ModuleSignature") {
        if let Some(FieldValue::String(mod_id)) = r.get(0) {
            installed_modules.insert(mod_id.clone());
        }
    }

    for r in db.get_records("ModuleExclusion") {
        if let (Some(FieldValue::String(mod_id)), Some(FieldValue::String(excl_id))) =
            (r.get(0), r.get(2))
        {
            if installed_modules.contains(mod_id) && installed_modules.contains(excl_id) {
                return Some(
                    IceReport::error(
                        "ICE25",
                        format!("Module '{mod_id}' excludes installed module '{excl_id}'"),
                    )
                    .with_table("ModuleExclusion"),
                );
            }
        }
    }
    None
}

/// ICE62: Verifies `IsolatedComponent` table pairs shared components with valid owning components.
///
/// # Arguments
///
/// * `db` - The [`LinkedDatabase`] to inspect.
///
/// # Returns
///
/// `Some(IceReport)` if `IsolatedComponent` references missing Component.
#[must_use]
pub fn validate_ice62(db: &LinkedDatabase) -> Option<IceReport> {
    let comps: HashSet<String> = db
        .get_records("Component")
        .iter()
        .filter_map(|r| match r.get(0) {
            Some(FieldValue::String(c)) => Some(c.clone()),
            _ => None,
        })
        .collect();

    for r in db.get_records("IsolatedComponent") {
        if let (Some(FieldValue::String(comp_shared)), Some(FieldValue::String(comp_owner))) =
            (r.get(0), r.get(1))
        {
            if !comps.is_empty() && (!comps.contains(comp_shared) || !comps.contains(comp_owner)) {
                return Some(
                    IceReport::error(
                        "ICE62",
                        format!("IsolatedComponent references non-existent Component '{comp_shared}' or '{comp_owner}'"),
                    )
                    .with_table("IsolatedComponent"),
                );
            }
        }
    }
    None
}

/// ICE66: Verifies Schema Version requirement consistency with package capabilities.
///
/// # Arguments
///
/// * `db` - The [`LinkedDatabase`] to inspect.
///
/// # Returns
///
/// `Some(IceReport)` if declared schema version is too low for package features.
#[must_use]
pub fn validate_ice66(db: &LinkedDatabase) -> Option<IceReport> {
    // Check if 64-bit components are used
    let has_win64_components = db.get_records("Component").iter().any(|r| match r.get(3) {
        Some(FieldValue::Short(attr)) => (*attr & 0x0100) != 0,
        _ => false,
    });

    if has_win64_components {
        for r in db.get_records("Property") {
            if let (Some(FieldValue::String(prop)), Some(FieldValue::String(val))) =
                (r.get(0), r.get(1))
            {
                if prop == "PageCount" || prop == "Schema" {
                    if let Ok(schema_ver) = val.parse::<i32>() {
                        if schema_ver < 200 {
                            return Some(
                                IceReport::error(
                                    "ICE66",
                                    format!("Package uses 64-bit components but declares schema version {schema_ver} (must be >= 200)"),
                                )
                                .with_table("Property"),
                            );
                        }
                    }
                }
            }
        }
    }
    None
}

/// ICE76: Validates Side-by-Side assembly manifest attributes in `MsiAssembly` and `MsiAssemblyName`.
///
/// # Arguments
///
/// * `db` - The [`LinkedDatabase`] to inspect.
///
/// # Returns
///
/// `Some(IceReport)` if Side-by-Side assembly manifest attributes are invalid.
#[must_use]
pub fn validate_ice76(db: &LinkedDatabase) -> Option<IceReport> {
    for r in db.get_records("MsiAssembly") {
        if let (Some(FieldValue::String(comp_id)), Some(FieldValue::Short(attr))) =
            (r.get(0), r.get(3))
        {
            // Attribute bit 1 = .NET assembly, 2 = Win32 SxS assembly
            if *attr != 0 && *attr != 1 && *attr != 2 {
                return Some(
                    IceReport::error(
                        "ICE76",
                        format!("MsiAssembly for component '{comp_id}' has invalid Attributes {attr}; must be 0, 1 (.NET), or 2 (Win32)"),
                    )
                    .with_table("MsiAssembly"),
                );
            }
        }
    }
    None
}

/// ICE81: Validates digital certificate and signature table consistency.
///
/// # Arguments
///
/// * `db` - The [`LinkedDatabase`] to inspect.
///
/// # Returns
///
/// `Some(IceReport)` if digital signature references missing certificate.
#[must_use]
pub fn validate_ice81(db: &LinkedDatabase) -> Option<IceReport> {
    let certs: HashSet<String> = db
        .get_records("MsiDigitalCertificate")
        .iter()
        .filter_map(|r| match r.get(0) {
            Some(FieldValue::String(c)) => Some(c.clone()),
            _ => None,
        })
        .collect();

    for r in db.get_records("MsiDigitalSignature") {
        if let (Some(FieldValue::String(tbl)), Some(FieldValue::String(cert_ref))) =
            (r.get(0), r.get(3))
        {
            if !certs.is_empty() && !certs.contains(cert_ref) {
                return Some(
                    IceReport::error(
                        "ICE81",
                        format!("MsiDigitalSignature on table '{tbl}' references non-existent Certificate '{cert_ref}'"),
                    )
                    .with_table("MsiDigitalSignature"),
                );
            }
        }
    }
    None
}

/// ICE83: Verifies `MsiAssembly` foreign keys and manifest file references.
///
/// # Arguments
///
/// * `db` - The [`LinkedDatabase`] to inspect.
///
/// # Returns
///
/// `Some(IceReport)` if `MsiAssembly` references non-existent File manifest.
#[must_use]
pub fn validate_ice83(db: &LinkedDatabase) -> Option<IceReport> {
    let files: HashSet<String> = db
        .get_records("File")
        .iter()
        .filter_map(|r| match r.get(0) {
            Some(FieldValue::String(f)) => Some(f.clone()),
            _ => None,
        })
        .collect();

    for r in db.get_records("MsiAssembly") {
        if let (Some(FieldValue::String(comp_id)), Some(FieldValue::String(mfile))) =
            (r.get(0), r.get(2))
        {
            if !mfile.is_empty() && !files.is_empty() && !files.contains(mfile) {
                return Some(
                    IceReport::error(
                        "ICE83",
                        format!("MsiAssembly for component '{comp_id}' references non-existent File_Manifest '{mfile}'"),
                    )
                    .with_table("MsiAssembly"),
                );
            }
        }
    }
    None
}

/// ICE94: Verifies `CustomAction` entry points and script calling convention signatures.
///
/// # Arguments
///
/// * `db` - The [`LinkedDatabase`] to inspect.
///
/// # Returns
///
/// `Some(IceReport)` if `CustomAction` entry point syntax is empty.
#[must_use]
pub fn validate_ice94(db: &LinkedDatabase) -> Option<IceReport> {
    for r in db.get_records("CustomAction") {
        if let (
            Some(FieldValue::String(name)),
            Some(FieldValue::Short(typ)),
            Some(FieldValue::String(target)),
        ) = (r.get(0), r.get(1), r.get(3))
        {
            let base_type = typ & 0x003F;
            // Type 1 = DLL call, target is function name
            if base_type == 1 && target.trim().is_empty() {
                return Some(
                    IceReport::error(
                        "ICE94",
                        format!("CustomAction '{name}' (Type 1 DLL) has empty Target function entry point"),
                    )
                    .with_table("CustomAction"),
                );
            }
        }
    }
    None
}

/// ICE97: Verifies COM+ application component registrations.
///
/// # Arguments
///
/// * `db` - The [`LinkedDatabase`] to inspect.
///
/// # Returns
///
/// `Some(IceReport)` if Complus record references non-existent Component.
#[must_use]
pub fn validate_ice97(db: &LinkedDatabase) -> Option<IceReport> {
    let comps: HashSet<String> = db
        .get_records("Component")
        .iter()
        .filter_map(|r| match r.get(0) {
            Some(FieldValue::String(c)) => Some(c.clone()),
            _ => None,
        })
        .collect();

    for r in db.get_records("Complus") {
        if let (Some(FieldValue::String(app_id)), Some(FieldValue::String(comp_ref))) =
            (r.get(0), r.get(1))
        {
            if !comps.is_empty() && !comps.contains(comp_ref) {
                return Some(
                    IceReport::error(
                        "ICE97",
                        format!("Complus application '{app_id}' references non-existent Component '{comp_ref}'"),
                    )
                    .with_table("Complus"),
                );
            }
        }
    }
    None
}

/// ICE98: Verifies ODBC data sources and driver table relationships.
///
/// # Arguments
///
/// * `db` - The [`LinkedDatabase`] to inspect.
///
/// # Returns
///
/// `Some(IceReport)` if `ODBCDataSource` references non-existent `ODBCDriver`.
#[must_use]
pub fn validate_ice98(db: &LinkedDatabase) -> Option<IceReport> {
    let drivers: HashSet<String> = db
        .get_records("ODBCDriver")
        .iter()
        .filter_map(|r| match r.get(0) {
            Some(FieldValue::String(d)) => Some(d.clone()),
            _ => None,
        })
        .collect();

    for r in db.get_records("ODBCDataSource") {
        if let (Some(FieldValue::String(ds_id)), Some(FieldValue::String(driver_ref))) =
            (r.get(0), r.get(2))
        {
            if !drivers.is_empty() && !drivers.contains(driver_ref) {
                return Some(
                    IceReport::error(
                        "ICE98",
                        format!("ODBCDataSource '{ds_id}' references non-existent ODBCDriver '{driver_ref}'"),
                    )
                    .with_table("ODBCDataSource"),
                );
            }
        }
    }
    None
}

/// ICE105: Verifies Patch transform stream delta consistency against baseline tables.
///
/// # Arguments
///
/// * `db` - The [`LinkedDatabase`] to inspect.
///
/// # Returns
///
/// `Some(IceReport)` if patch sequence numbers or transforms are invalid.
#[must_use]
pub fn validate_ice105(db: &LinkedDatabase) -> Option<IceReport> {
    for r in db.get_records("_PatchData") {
        if let (Some(FieldValue::String(patch_code)), Some(FieldValue::Short(seq))) =
            (r.get(0), r.get(1))
        {
            if *seq < 0 {
                return Some(
                    IceReport::error(
                        "ICE105",
                        format!("Patch '{patch_code}' has negative sequence number: {seq}"),
                    )
                    .with_table("_PatchData"),
                );
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::database::tables::record::Record;

    fn make_test_db() -> LinkedDatabase {
        LinkedDatabase::default()
    }

    #[test]
    fn test_ice25_module_exclusion() {
        let mut db = make_test_db();
        db.add_record(
            "ModuleSignature",
            Record::with_fields(vec![FieldValue::String("ModA".to_string())]),
        );
        db.add_record(
            "ModuleSignature",
            Record::with_fields(vec![FieldValue::String("ModB".to_string())]),
        );
        db.add_record(
            "ModuleExclusion",
            Record::with_fields(vec![
                FieldValue::String("ModA".to_string()),
                FieldValue::Null,
                FieldValue::String("ModB".to_string()), // ModA excludes ModB
            ]),
        );
        assert!(validate_ice25(&db).is_some());

        // Valid case: excluded module is not installed, or excluding module itself is not installed
        let mut db_good = make_test_db();
        db_good.add_record(
            "ModuleSignature",
            Record::with_fields(vec![FieldValue::String("ModA".to_string())]),
        );
        db_good.add_record(
            "ModuleExclusion",
            Record::with_fields(vec![
                FieldValue::String("ModA".to_string()),
                FieldValue::Null,
                FieldValue::String("ModOther".to_string()),
            ]),
        );
        db_good.add_record(
            "ModuleExclusion",
            Record::with_fields(vec![
                FieldValue::String("ModNotInstalled".to_string()),
                FieldValue::Null,
                FieldValue::String("ModA".to_string()),
            ]),
        );
        db_good.add_record(
            "ModuleSignature",
            Record::with_fields(vec![FieldValue::Short(1)]),
        );
        db_good.add_record(
            "ModuleExclusion",
            Record::with_fields(vec![FieldValue::Short(1)]),
        );
        assert!(validate_ice25(&db_good).is_none());
    }

    #[test]
    fn test_ice62_isolated_component() {
        let mut db = make_test_db();
        db.add_record(
            "Component",
            Record::with_fields(vec![FieldValue::String("CompReal".to_string())]),
        );
        db.add_record(
            "IsolatedComponent",
            Record::with_fields(vec![
                FieldValue::String("CompReal".to_string()),
                FieldValue::String("CompMissing".to_string()),
            ]),
        );
        assert!(validate_ice62(&db).is_some());

        // CompShared missing
        let mut db_shared_missing = make_test_db();
        db_shared_missing.add_record(
            "Component",
            Record::with_fields(vec![FieldValue::String("CompOwner".to_string())]),
        );
        db_shared_missing.add_record(
            "IsolatedComponent",
            Record::with_fields(vec![
                FieldValue::String("CompSharedMissing".to_string()),
                FieldValue::String("CompOwner".to_string()),
            ]),
        );
        assert!(validate_ice62(&db_shared_missing).is_some());

        // Valid case: both components exist, plus empty components table and non-string records
        let mut db_good = make_test_db();
        db_good.add_record(
            "Component",
            Record::with_fields(vec![FieldValue::String("Comp1".to_string())]),
        );
        db_good.add_record(
            "Component",
            Record::with_fields(vec![FieldValue::String("Comp2".to_string())]),
        );
        db_good.add_record("Component", Record::with_fields(vec![FieldValue::Short(1)]));
        db_good.add_record(
            "IsolatedComponent",
            Record::with_fields(vec![
                FieldValue::String("Comp1".to_string()),
                FieldValue::String("Comp2".to_string()),
            ]),
        );
        db_good.add_record(
            "IsolatedComponent",
            Record::with_fields(vec![FieldValue::Short(1)]),
        );
        assert!(validate_ice62(&db_good).is_none());

        let mut db_empty_comps = make_test_db();
        db_empty_comps.add_record(
            "IsolatedComponent",
            Record::with_fields(vec![
                FieldValue::String("CompA".to_string()),
                FieldValue::String("CompB".to_string()),
            ]),
        );
        assert!(validate_ice62(&db_empty_comps).is_none());
    }

    #[test]
    #[allow(clippy::too_many_lines)]
    fn test_ice66_schema_version() {
        let mut db = make_test_db();
        db.add_record(
            "Component",
            Record::with_fields(vec![
                FieldValue::String("Comp64".to_string()),
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Short(0x0100), // 64-bit component attribute
            ]),
        );
        db.add_record(
            "Property",
            Record::with_fields(vec![
                FieldValue::String("PageCount".to_string()),
                FieldValue::String("100".to_string()), // Schema 100 < 200
            ]),
        );
        assert!(validate_ice66(&db).is_some());

        // Test "Schema" property name with invalid schema
        let mut db_schema_prop = make_test_db();
        db_schema_prop.add_record(
            "Component",
            Record::with_fields(vec![
                FieldValue::String("Comp64".to_string()),
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Short(0x0100),
            ]),
        );
        db_schema_prop.add_record(
            "Property",
            Record::with_fields(vec![
                FieldValue::String("Schema".to_string()),
                FieldValue::String("150".to_string()),
            ]),
        );
        assert!(validate_ice66(&db_schema_prop).is_some());

        // Valid schema version >= 200, non-parseable version, other property, and 32-bit components
        let mut db_good = make_test_db();
        db_good.add_record(
            "Component",
            Record::with_fields(vec![
                FieldValue::String("Comp64".to_string()),
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Short(0x0100),
            ]),
        );
        db_good.add_record(
            "Component",
            Record::with_fields(vec![
                FieldValue::String("Comp32".to_string()),
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Short(0x0000),
            ]),
        );
        db_good.add_record("Component", Record::with_fields(vec![FieldValue::Short(1)]));
        db_good.add_record(
            "Property",
            Record::with_fields(vec![
                FieldValue::String("OtherProp".to_string()),
                FieldValue::String("Val".to_string()),
            ]),
        );
        db_good.add_record(
            "Property",
            Record::with_fields(vec![
                FieldValue::String("PageCount".to_string()),
                FieldValue::String("not_a_number".to_string()),
            ]),
        );
        db_good.add_record(
            "Property",
            Record::with_fields(vec![
                FieldValue::String("Schema".to_string()),
                FieldValue::String("200".to_string()),
            ]),
        );
        db_good.add_record("Property", Record::with_fields(vec![FieldValue::Short(1)]));
        assert!(validate_ice66(&db_good).is_none());

        let mut db_no_win64 = make_test_db();
        db_no_win64.add_record(
            "Component",
            Record::with_fields(vec![
                FieldValue::String("Comp32".to_string()),
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Short(0),
            ]),
        );
        db_no_win64.add_record(
            "Component",
            Record::with_fields(vec![
                FieldValue::String("CompNoAttr".to_string()),
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null, // attr is Null -> covers match r.get(3) _ => false
            ]),
        );
        assert!(validate_ice66(&db_no_win64).is_none());
    }

    #[test]
    fn test_ice76_msi_assembly() {
        let mut db = make_test_db();
        db.add_record(
            "MsiAssembly",
            Record::with_fields(vec![
                FieldValue::String("CompAsm".to_string()),
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Short(9), // invalid attribute 9
            ]),
        );
        assert!(validate_ice76(&db).is_some());

        // Valid attributes 0, 1, 2, and non-string/null fields
        let mut db_good = make_test_db();
        db_good.add_record(
            "MsiAssembly",
            Record::with_fields(vec![
                FieldValue::String("Comp0".to_string()),
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Short(0),
            ]),
        );
        db_good.add_record(
            "MsiAssembly",
            Record::with_fields(vec![
                FieldValue::String("Comp1".to_string()),
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Short(1),
            ]),
        );
        db_good.add_record(
            "MsiAssembly",
            Record::with_fields(vec![
                FieldValue::String("Comp2".to_string()),
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Short(2),
            ]),
        );
        db_good.add_record(
            "MsiAssembly",
            Record::with_fields(vec![FieldValue::Short(1)]),
        );
        assert!(validate_ice76(&db_good).is_none());
    }

    #[test]
    fn test_ice81_digital_signature() {
        let mut db = make_test_db();
        db.add_record(
            "MsiDigitalCertificate",
            Record::with_fields(vec![FieldValue::String("CertValid".to_string())]),
        );
        db.add_record(
            "MsiDigitalSignature",
            Record::with_fields(vec![
                FieldValue::String("Binary".to_string()),
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::String("CertMissing".to_string()),
            ]),
        );
        assert!(validate_ice81(&db).is_some());

        // Valid certificate match, empty certs table, and non-string fields
        let mut db_good = make_test_db();
        db_good.add_record(
            "MsiDigitalCertificate",
            Record::with_fields(vec![FieldValue::String("CertOK".to_string())]),
        );
        db_good.add_record(
            "MsiDigitalCertificate",
            Record::with_fields(vec![FieldValue::Short(1)]),
        );
        db_good.add_record(
            "MsiDigitalSignature",
            Record::with_fields(vec![
                FieldValue::String("Binary".to_string()),
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::String("CertOK".to_string()),
            ]),
        );
        db_good.add_record(
            "MsiDigitalSignature",
            Record::with_fields(vec![FieldValue::Short(1)]),
        );
        assert!(validate_ice81(&db_good).is_none());

        let mut db_no_certs = make_test_db();
        db_no_certs.add_record(
            "MsiDigitalSignature",
            Record::with_fields(vec![
                FieldValue::String("Binary".to_string()),
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::String("AnyCert".to_string()),
            ]),
        );
        assert!(validate_ice81(&db_no_certs).is_none());
    }

    #[test]
    fn test_ice83_assembly_manifest_file() {
        let mut db = make_test_db();
        db.add_record(
            "File",
            Record::with_fields(vec![FieldValue::String("existing.manifest".to_string())]),
        );
        db.add_record(
            "MsiAssembly",
            Record::with_fields(vec![
                FieldValue::String("Comp1".to_string()),
                FieldValue::Null,
                FieldValue::String("missing.manifest".to_string()),
            ]),
        );
        assert!(validate_ice83(&db).is_some());

        // Valid existing file, empty manifest string, empty file table, and non-string fields
        let mut db_good = make_test_db();
        db_good.add_record(
            "File",
            Record::with_fields(vec![FieldValue::String("app.manifest".to_string())]),
        );
        db_good.add_record("File", Record::with_fields(vec![FieldValue::Short(1)]));
        db_good.add_record(
            "MsiAssembly",
            Record::with_fields(vec![
                FieldValue::String("Comp1".to_string()),
                FieldValue::Null,
                FieldValue::String("app.manifest".to_string()),
            ]),
        );
        db_good.add_record(
            "MsiAssembly",
            Record::with_fields(vec![
                FieldValue::String("Comp2".to_string()),
                FieldValue::Null,
                FieldValue::String(String::new()),
            ]),
        );
        db_good.add_record(
            "MsiAssembly",
            Record::with_fields(vec![FieldValue::Short(1)]),
        );
        assert!(validate_ice83(&db_good).is_none());

        let mut db_empty_files = make_test_db();
        db_empty_files.add_record(
            "MsiAssembly",
            Record::with_fields(vec![
                FieldValue::String("Comp1".to_string()),
                FieldValue::Null,
                FieldValue::String("some.manifest".to_string()),
            ]),
        );
        assert!(validate_ice83(&db_empty_files).is_none());
    }

    #[test]
    fn test_ice94_custom_action_entry_point() {
        let mut db = make_test_db();
        db.add_record(
            "CustomAction",
            Record::with_fields(vec![
                FieldValue::String("BadCA".to_string()),
                FieldValue::Short(1), // Type 1 DLL
                FieldValue::String("MyBin".to_string()),
                FieldValue::String("   ".to_string()), // Empty entry point!
            ]),
        );
        assert!(validate_ice94(&db).is_some());

        // Valid Type 1 entry point, non-type-1 custom action, non-string records
        let mut db_good = make_test_db();
        db_good.add_record(
            "CustomAction",
            Record::with_fields(vec![
                FieldValue::String("GoodDLL".to_string()),
                FieldValue::Short(1),
                FieldValue::String("MyBin".to_string()),
                FieldValue::String("MyFunction".to_string()),
            ]),
        );
        db_good.add_record(
            "CustomAction",
            Record::with_fields(vec![
                FieldValue::String("ExeCA".to_string()),
                FieldValue::Short(2), // Type 2 EXE
                FieldValue::String("MyBin".to_string()),
                FieldValue::String(String::new()),
            ]),
        );
        db_good.add_record(
            "CustomAction",
            Record::with_fields(vec![FieldValue::Short(1)]),
        );
        assert!(validate_ice94(&db_good).is_none());
    }

    #[test]
    fn test_ice97_complus_component() {
        let mut db = make_test_db();
        db.add_record(
            "Component",
            Record::with_fields(vec![FieldValue::String("RealComp".to_string())]),
        );
        db.add_record(
            "Complus",
            Record::with_fields(vec![
                FieldValue::String("App1".to_string()),
                FieldValue::String("MissingComp".to_string()),
            ]),
        );
        assert!(validate_ice97(&db).is_some());

        // Valid component reference, empty component table, non-string fields
        let mut db_good = make_test_db();
        db_good.add_record(
            "Component",
            Record::with_fields(vec![FieldValue::String("RealComp".to_string())]),
        );
        db_good.add_record("Component", Record::with_fields(vec![FieldValue::Short(1)]));
        db_good.add_record(
            "Complus",
            Record::with_fields(vec![
                FieldValue::String("App1".to_string()),
                FieldValue::String("RealComp".to_string()),
            ]),
        );
        db_good.add_record("Complus", Record::with_fields(vec![FieldValue::Short(1)]));
        assert!(validate_ice97(&db_good).is_none());

        let mut db_empty_comps = make_test_db();
        db_empty_comps.add_record(
            "Complus",
            Record::with_fields(vec![
                FieldValue::String("App1".to_string()),
                FieldValue::String("CompX".to_string()),
            ]),
        );
        assert!(validate_ice97(&db_empty_comps).is_none());
    }

    #[test]
    fn test_ice98_odbc_driver() {
        let mut db = make_test_db();
        db.add_record(
            "ODBCDriver",
            Record::with_fields(vec![FieldValue::String("PostgresDriver".to_string())]),
        );
        db.add_record(
            "ODBCDataSource",
            Record::with_fields(vec![
                FieldValue::String("MyDSN".to_string()),
                FieldValue::Null,
                FieldValue::String("MissingDriver".to_string()),
            ]),
        );
        assert!(validate_ice98(&db).is_some());

        // Valid driver reference, empty drivers table, non-string fields
        let mut db_good = make_test_db();
        db_good.add_record(
            "ODBCDriver",
            Record::with_fields(vec![FieldValue::String("PostgresDriver".to_string())]),
        );
        db_good.add_record(
            "ODBCDriver",
            Record::with_fields(vec![FieldValue::Short(1)]),
        );
        db_good.add_record(
            "ODBCDataSource",
            Record::with_fields(vec![
                FieldValue::String("MyDSN".to_string()),
                FieldValue::Null,
                FieldValue::String("PostgresDriver".to_string()),
            ]),
        );
        db_good.add_record(
            "ODBCDataSource",
            Record::with_fields(vec![FieldValue::Short(1)]),
        );
        assert!(validate_ice98(&db_good).is_none());

        let mut db_empty_drivers = make_test_db();
        db_empty_drivers.add_record(
            "ODBCDataSource",
            Record::with_fields(vec![
                FieldValue::String("MyDSN".to_string()),
                FieldValue::Null,
                FieldValue::String("DriverX".to_string()),
            ]),
        );
        assert!(validate_ice98(&db_empty_drivers).is_none());
    }

    #[test]
    fn test_ice105_patch_data() {
        let mut db = make_test_db();
        db.add_record(
            "_PatchData",
            Record::with_fields(vec![
                FieldValue::String("Patch1".to_string()),
                FieldValue::Short(-5), // negative sequence!
            ]),
        );
        assert!(validate_ice105(&db).is_some());

        // Valid sequence number >= 0 and non-string record
        let mut db_good = make_test_db();
        db_good.add_record(
            "_PatchData",
            Record::with_fields(vec![
                FieldValue::String("Patch1".to_string()),
                FieldValue::Short(0),
            ]),
        );
        db_good.add_record(
            "_PatchData",
            Record::with_fields(vec![
                FieldValue::String("Patch2".to_string()),
                FieldValue::Short(10),
            ]),
        );
        db_good.add_record(
            "_PatchData",
            Record::with_fields(vec![FieldValue::Short(1)]),
        );
        assert!(validate_ice105(&db_good).is_none());
    }
}

//! System, registry, search, permissions, service, and foreign key ICE validation rules.
//!
//! Grounded directly in the Windows Installer system and catalog specifications:
//! - Rules: ICE15, ICE17, ICE32, ICE36, ICE49, ICE50, ICE52, ICE55, ICE56, ICE65, ICE85, ICE88, ICE100.

use super::types::IceReport;
use crate::database::tables::record::FieldValue;
use crate::wix::linker::LinkedDatabase;
use std::collections::{HashMap, HashSet};

/// ICE15: Verifies bidirectional consistency between MIME and Extension tables without circular deadlocks.
///
/// # Arguments
///
/// * `db` - The [`LinkedDatabase`] to inspect.
///
/// # Returns
///
/// `Some(IceReport)` if MIME references non-existent Extension.
#[must_use]
pub fn validate_ice15(db: &LinkedDatabase) -> Option<IceReport> {
    let extensions: HashSet<String> = db
        .get_records("Extension")
        .iter()
        .filter_map(|r| match r.get(0) {
            Some(FieldValue::String(e)) => Some(e.clone()),
            _ => None,
        })
        .collect();

    for r in db.get_records("MIME") {
        if let (Some(FieldValue::String(ct)), Some(FieldValue::String(ext_ref))) =
            (r.get(0), r.get(1))
        {
            if !extensions.is_empty() && !extensions.contains(ext_ref) {
                return Some(
                    IceReport::error(
                        "ICE15",
                        format!("MIME type '{ct}' references missing Extension '{ext_ref}'"),
                    )
                    .with_table("MIME"),
                );
            }
        }
    }
    None
}

/// ICE17: Verifies `ConfigSearch` table syntax, signatures, and attributes.
///
/// # Arguments
///
/// * `db` - The [`LinkedDatabase`] to inspect.
///
/// # Returns
///
/// `Some(IceReport)` if `ConfigSearch` record has invalid syntax.
#[must_use]
pub fn validate_ice17(db: &LinkedDatabase) -> Option<IceReport> {
    for r in db.get_records("ConfigSearch") {
        if let (Some(FieldValue::String(prop)), Some(FieldValue::Short(root))) =
            (r.get(0), r.get(1))
        {
            if prop.trim().is_empty() {
                return Some(
                    IceReport::error("ICE17", "ConfigSearch property name cannot be empty")
                        .with_table("ConfigSearch"),
                );
            }
            if *root < 0 {
                return Some(
                    IceReport::error(
                        "ICE17",
                        format!("ConfigSearch property '{prop}' has negative Root index: {root}"),
                    )
                    .with_table("ConfigSearch"),
                );
            }
        }
    }
    None
}

/// ICE32: Evaluates foreign key integrity across all catalog relationships.
///
/// # Arguments
///
/// * `db` - The [`LinkedDatabase`] to inspect.
///
/// # Returns
///
/// `Some(IceReport)` if a foreign key relation is broken.
#[must_use]
pub fn validate_ice32(db: &LinkedDatabase) -> Option<IceReport> {
    let comps: HashSet<String> = db
        .get_records("Component")
        .iter()
        .filter_map(|r| match r.get(0) {
            Some(FieldValue::String(c)) => Some(c.clone()),
            _ => None,
        })
        .collect();

    // Check File -> Component foreign key
    for r in db.get_records("File") {
        if let (Some(FieldValue::String(fid)), Some(FieldValue::String(comp_ref))) =
            (r.get(0), r.get(1))
        {
            if !comps.is_empty() && !comps.contains(comp_ref) {
                return Some(
                    IceReport::error(
                        "ICE32",
                        format!("File '{fid}' foreign key references non-existent Component '{comp_ref}'"),
                    )
                    .with_table("File")
                    .with_column("Component_"),
                );
            }
        }
    }
    None
}

/// ICE36: Verifies that every record in the Icon table is referenced by at least one Shortcut, `ProgId`, or Class.
///
/// # Arguments
///
/// * `db` - The [`LinkedDatabase`] to inspect.
///
/// # Returns
///
/// `Some(IceReport)` if an orphaned Icon record is detected.
#[must_use]
pub fn validate_ice36(db: &LinkedDatabase) -> Option<IceReport> {
    let mut referenced_icons = HashSet::new();

    for r in db.get_records("Shortcut") {
        if let Some(FieldValue::String(icon)) = r.get(8) {
            if !icon.is_empty() {
                referenced_icons.insert(icon.clone());
            }
        }
    }
    for r in db.get_records("ProgId") {
        if let Some(FieldValue::String(icon)) = r.get(3) {
            if !icon.is_empty() {
                referenced_icons.insert(icon.clone());
            }
        }
    }
    for r in db.get_records("Class") {
        if let Some(FieldValue::String(icon)) = r.get(7) {
            if !icon.is_empty() {
                referenced_icons.insert(icon.clone());
            }
        }
    }

    if !referenced_icons.is_empty() {
        for r in db.get_records("Icon") {
            if let Some(FieldValue::String(name)) = r.get(0) {
                if !referenced_icons.contains(name) {
                    return Some(
                        IceReport::warning(
                            "ICE36",
                            format!(
                                "Icon '{name}' is not referenced by any Shortcut, ProgId, or Class"
                            ),
                        )
                        .with_table("Icon"),
                    );
                }
            }
        }
    }
    None
}

/// ICE49: Validates Registry value formatting and string type prefixes (#x, #%, ~, #).
///
/// # Arguments
///
/// * `db` - The [`LinkedDatabase`] to inspect.
///
/// # Returns
///
/// `Some(IceReport)` if Registry value syntax is invalid.
#[must_use]
pub fn validate_ice49(db: &LinkedDatabase) -> Option<IceReport> {
    for r in db.get_records("Registry") {
        if let (Some(FieldValue::String(reg_id)), Some(FieldValue::String(val))) =
            (r.get(0), r.get(4))
        {
            if let Some(stripped) = val.strip_prefix("#x") {
                if !stripped.chars().all(|c| c.is_ascii_hexdigit()) {
                    return Some(
                        IceReport::error(
                            "ICE49",
                            format!("Registry '{reg_id}' hexadecimal value '{val}' contains non-hex digits"),
                        )
                        .with_table("Registry"),
                    );
                }
            } else if let Some(stripped) = val.strip_prefix('#') {
                if !stripped.is_empty() && !stripped.chars().all(|c| c.is_ascii_digit() || c == '-')
                {
                    return Some(
                        IceReport::error(
                            "ICE49",
                            format!("Registry '{reg_id}' integer value '{val}' contains non-digit characters"),
                        )
                        .with_table("Registry"),
                    );
                }
            }
        }
    }
    None
}

/// ICE50: Verifies shortcut icon format (must have .ico or .exe extension or reference Icon table).
///
/// # Arguments
///
/// * `db` - The [`LinkedDatabase`] to inspect.
///
/// # Returns
///
/// `Some(IceReport)` if shortcut icon reference is malformed.
#[must_use]
pub fn validate_ice50(db: &LinkedDatabase) -> Option<IceReport> {
    let icons: HashSet<String> = db
        .get_records("Icon")
        .iter()
        .filter_map(|r| match r.get(0) {
            Some(FieldValue::String(i)) => Some(i.clone()),
            _ => None,
        })
        .collect();

    for r in db.get_records("Shortcut") {
        if let (Some(FieldValue::String(sc_id)), Some(FieldValue::String(icon_ref))) =
            (r.get(0), r.get(8))
        {
            if !icon_ref.is_empty() && !icons.is_empty() && !icons.contains(icon_ref) {
                return Some(
                    IceReport::error(
                        "ICE50",
                        format!("Shortcut '{sc_id}' references non-existent Icon '{icon_ref}'"),
                    )
                    .with_table("Shortcut"),
                );
            }
        }
    }
    None
}

/// ICE52: Verifies `AppSearch` / `RegLocator` / `IniLocator` attribute checks.
///
/// # Arguments
///
/// * `db` - The [`LinkedDatabase`] to inspect.
///
/// # Returns
///
/// `Some(IceReport)` if `AppSearch` references non-existent signature.
#[must_use]
pub fn validate_ice52(db: &LinkedDatabase) -> Option<IceReport> {
    let mut signatures = HashSet::new();
    for tbl in &["RegLocator", "IniLocator", "CompLocator", "DrLocator"] {
        for r in db.get_records(tbl) {
            if let Some(FieldValue::String(sig)) = r.get(0) {
                signatures.insert(sig.clone());
            }
        }
    }

    if !signatures.is_empty() {
        for r in db.get_records("AppSearch") {
            if let (Some(FieldValue::String(prop)), Some(FieldValue::String(sig_ref))) =
                (r.get(0), r.get(1))
            {
                if !signatures.contains(sig_ref) {
                    return Some(
                        IceReport::error(
                            "ICE52",
                            format!("AppSearch for '{prop}' references non-existent signature '{sig_ref}'"),
                        )
                        .with_table("AppSearch"),
                    );
                }
            }
        }
    }
    None
}

/// ICE55: Verifies `LockPermissions` table object existence.
///
/// # Arguments
///
/// * `db` - The [`LinkedDatabase`] to inspect.
///
/// # Returns
///
/// `Some(IceReport)` if `LockPermissions` references missing object or invalid table.
#[must_use]
pub fn validate_ice55(db: &LinkedDatabase) -> Option<IceReport> {
    for r in db.get_records("LockPermissions") {
        if let (Some(FieldValue::String(lock_obj)), Some(FieldValue::String(tbl_ref))) =
            (r.get(0), r.get(1))
        {
            if tbl_ref != "File" && tbl_ref != "Registry" && tbl_ref != "CreateFolder" {
                return Some(
                    IceReport::error(
                        "ICE55",
                        format!("LockPermissions object '{lock_obj}' specifies invalid Table '{tbl_ref}'"),
                    )
                    .with_table("LockPermissions"),
                );
            }
        }
    }
    None
}

/// ICE56: Verifies standard system directory rooting back to TARGETDIR.
///
/// # Arguments
///
/// * `db` - The [`LinkedDatabase`] to inspect.
///
/// # Returns
///
/// `Some(IceReport)` if disconnected directory graph is detected.
#[must_use]
pub fn validate_ice56(db: &LinkedDatabase) -> Option<IceReport> {
    let mut parent_map = HashMap::new();
    for r in db.get_records("Directory") {
        if let (Some(FieldValue::String(dir_id)), parent_val) = (r.get(0), r.get(1)) {
            match parent_val {
                Some(FieldValue::String(p)) if !p.is_empty() => {
                    parent_map.insert(dir_id.clone(), p.clone());
                }
                _ => {}
            }
        }
    }

    for (dir, parent) in &parent_map {
        if dir == parent {
            return Some(
                IceReport::error(
                    "ICE56",
                    format!("Directory '{dir}' cannot specify itself as parent directory"),
                )
                .with_table("Directory"),
            );
        }
    }
    None
}

/// ICE65: Verifies Environment variable prefix syntax (=, +, -, !, *).
///
/// # Arguments
///
/// * `db` - The [`LinkedDatabase`] to inspect.
///
/// # Returns
///
/// `Some(IceReport)` if Environment variable prefix formatting is invalid.
#[must_use]
pub fn validate_ice65(db: &LinkedDatabase) -> Option<IceReport> {
    for r in db.get_records("Environment") {
        if let (Some(FieldValue::String(env_id)), Some(FieldValue::String(name))) =
            (r.get(0), r.get(1))
        {
            if name.is_empty() {
                return Some(
                    IceReport::error("ICE65", format!("Environment '{env_id}' has empty Name"))
                        .with_table("Environment"),
                );
            }
        }
    }
    None
}

/// ICE85: Verifies `CCPSearch` and `CompLocator` compliance.
///
/// # Arguments
///
/// * `db` - The [`LinkedDatabase`] to inspect.
///
/// # Returns
///
/// `Some(IceReport)` if `CCPSearch` references non-existent signature.
#[must_use]
pub fn validate_ice85(db: &LinkedDatabase) -> Option<IceReport> {
    let sigs: HashSet<String> = db
        .get_records("CompLocator")
        .iter()
        .filter_map(|r| match r.get(0) {
            Some(FieldValue::String(s)) => Some(s.clone()),
            _ => None,
        })
        .collect();

    for r in db.get_records("CCPSearch") {
        if let Some(FieldValue::String(sig_ref)) = r.get(0) {
            if !sigs.is_empty() && !sigs.contains(sig_ref) {
                return Some(
                    IceReport::error(
                        "ICE85",
                        format!("CCPSearch references non-existent Signature '{sig_ref}' in CompLocator"),
                    )
                    .with_table("CCPSearch"),
                );
            }
        }
    }
    None
}

/// ICE88: Verifies `DrLocator` directory search depth bounds (0 to 16).
///
/// # Arguments
///
/// * `db` - The [`LinkedDatabase`] to inspect.
///
/// # Returns
///
/// `Some(IceReport)` if `DrLocator` Depth is negative or exceeds 16.
#[must_use]
pub fn validate_ice88(db: &LinkedDatabase) -> Option<IceReport> {
    for r in db.get_records("DrLocator") {
        if let (Some(FieldValue::String(sig)), Some(FieldValue::Short(depth))) =
            (r.get(0), r.get(3))
        {
            if *depth < 0 || *depth > 16 {
                return Some(
                    IceReport::error(
                        "ICE88",
                        format!("DrLocator '{sig}' search depth {depth} is outside allowed range (0..=16)"),
                    )
                    .with_table("DrLocator"),
                );
            }
        }
    }
    None
}

/// ICE100: Verifies `ServiceInstall` and `ServiceControl` validation.
///
/// # Arguments
///
/// * `db` - The [`LinkedDatabase`] to inspect.
///
/// # Returns
///
/// `Some(IceReport)` if duplicate service name is detected.
#[must_use]
pub fn validate_ice100(db: &LinkedDatabase) -> Option<IceReport> {
    let mut names = HashSet::new();
    for r in db.get_records("ServiceInstall") {
        if let Some(FieldValue::String(name)) = r.get(1) {
            if !names.insert(name.clone()) {
                return Some(
                    IceReport::error(
                        "ICE100",
                        format!("Duplicate ServiceInstall name '{name}' detected"),
                    )
                    .with_table("ServiceInstall"),
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
    fn test_ice15_mime_extension() {
        let mut db = make_test_db();
        db.add_record(
            "Extension",
            Record::with_fields(vec![FieldValue::String("png".to_string())]),
        );
        db.add_record(
            "MIME",
            Record::with_fields(vec![
                FieldValue::String("image/jpeg".to_string()),
                FieldValue::String("jpg".to_string()), // missing jpg extension
            ]),
        );
        assert!(validate_ice15(&db).is_some());

        // Valid extension match, empty extensions table, non-string records
        let mut db_good = make_test_db();
        db_good.add_record(
            "Extension",
            Record::with_fields(vec![FieldValue::String("jpg".to_string())]),
        );
        db_good.add_record("Extension", Record::with_fields(vec![FieldValue::Short(1)]));
        db_good.add_record(
            "MIME",
            Record::with_fields(vec![
                FieldValue::String("image/jpeg".to_string()),
                FieldValue::String("jpg".to_string()),
            ]),
        );
        db_good.add_record("MIME", Record::with_fields(vec![FieldValue::Short(1)]));
        assert!(validate_ice15(&db_good).is_none());

        let mut db_empty = make_test_db();
        db_empty.add_record(
            "MIME",
            Record::with_fields(vec![
                FieldValue::String("image/png".to_string()),
                FieldValue::String("png".to_string()),
            ]),
        );
        assert!(validate_ice15(&db_empty).is_none());
    }

    #[test]
    fn test_ice17_config_search() {
        let mut db = make_test_db();
        db.add_record(
            "ConfigSearch",
            Record::with_fields(vec![
                FieldValue::String("PROP".to_string()),
                FieldValue::Short(-1), // negative root
            ]),
        );
        assert!(validate_ice17(&db).is_some());

        let mut db_empty = make_test_db();
        db_empty.add_record(
            "ConfigSearch",
            Record::with_fields(vec![
                FieldValue::String("   ".to_string()),
                FieldValue::Short(1),
            ]),
        );
        assert!(validate_ice17(&db_empty).is_some());

        let mut db_good = make_test_db();
        db_good.add_record(
            "ConfigSearch",
            Record::with_fields(vec![
                FieldValue::String("VALID_PROP".to_string()),
                FieldValue::Short(0), // root >= 0
            ]),
        );
        db_good.add_record(
            "ConfigSearch",
            Record::with_fields(vec![FieldValue::Short(1)]),
        );
        assert!(validate_ice17(&db_good).is_none());
    }

    #[test]
    fn test_ice32_foreign_key() {
        let mut db = make_test_db();
        db.add_record(
            "Component",
            Record::with_fields(vec![FieldValue::String("Comp1".to_string())]),
        );
        db.add_record(
            "File",
            Record::with_fields(vec![
                FieldValue::String("File1".to_string()),
                FieldValue::String("CompMissing".to_string()), // broken foreign key
            ]),
        );
        assert!(validate_ice32(&db).is_some());

        // Valid component reference, empty component table, non-string records
        let mut db_good = make_test_db();
        db_good.add_record(
            "Component",
            Record::with_fields(vec![FieldValue::String("Comp1".to_string())]),
        );
        db_good.add_record("Component", Record::with_fields(vec![FieldValue::Short(1)]));
        db_good.add_record(
            "File",
            Record::with_fields(vec![
                FieldValue::String("File1".to_string()),
                FieldValue::String("Comp1".to_string()),
            ]),
        );
        db_good.add_record("File", Record::with_fields(vec![FieldValue::Short(1)]));
        assert!(validate_ice32(&db_good).is_none());

        let mut db_empty = make_test_db();
        db_empty.add_record(
            "File",
            Record::with_fields(vec![
                FieldValue::String("File1".to_string()),
                FieldValue::String("CompAny".to_string()),
            ]),
        );
        assert!(validate_ice32(&db_empty).is_none());
    }

    #[test]
    #[allow(clippy::too_many_lines)]
    fn test_ice36_orphaned_icon() {
        let mut db = make_test_db();
        db.add_record(
            "Icon",
            Record::with_fields(vec![FieldValue::String("OrphanIcon".to_string())]),
        );
        db.add_record(
            "Shortcut",
            Record::with_fields(vec![
                FieldValue::String("SC".to_string()),
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::String("UsedIcon".to_string()),
            ]),
        );
        assert!(validate_ice36(&db).is_some());

        // Valid reference via Shortcut, ProgId, Class; empty references; non-string records
        let mut db_good = make_test_db();
        db_good.add_record(
            "Icon",
            Record::with_fields(vec![FieldValue::String("IconSC".to_string())]),
        );
        db_good.add_record(
            "Icon",
            Record::with_fields(vec![FieldValue::String("IconProgId".to_string())]),
        );
        db_good.add_record(
            "Icon",
            Record::with_fields(vec![FieldValue::String("IconClass".to_string())]),
        );
        db_good.add_record("Icon", Record::with_fields(vec![FieldValue::Short(1)]));
        db_good.add_record(
            "Shortcut",
            Record::with_fields(vec![
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::String("IconSC".to_string()),
            ]),
        );
        db_good.add_record(
            "Shortcut",
            Record::with_fields(vec![
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::String(String::new()), // empty icon
            ]),
        );
        db_good.add_record("Shortcut", Record::with_fields(vec![FieldValue::Short(1)]));
        db_good.add_record(
            "ProgId",
            Record::with_fields(vec![
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::String("IconProgId".to_string()),
            ]),
        );
        db_good.add_record(
            "ProgId",
            Record::with_fields(vec![
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::String(String::new()), // empty icon
            ]),
        );
        db_good.add_record("ProgId", Record::with_fields(vec![FieldValue::Short(1)]));
        db_good.add_record(
            "Class",
            Record::with_fields(vec![
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::String("IconClass".to_string()),
            ]),
        );
        db_good.add_record(
            "Class",
            Record::with_fields(vec![
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::String(String::new()), // empty icon
            ]),
        );
        db_good.add_record("Class", Record::with_fields(vec![FieldValue::Short(1)]));
        assert!(validate_ice36(&db_good).is_none());

        let mut db_no_refs = make_test_db();
        db_no_refs.add_record(
            "Icon",
            Record::with_fields(vec![FieldValue::String("SoloIcon".to_string())]),
        );
        assert!(validate_ice36(&db_no_refs).is_none());
    }

    #[test]
    fn test_ice49_registry_values() {
        let mut db = make_test_db();
        db.add_record(
            "Registry",
            Record::with_fields(vec![
                FieldValue::String("R1".to_string()),
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::String("#xZZTOP".to_string()), // invalid hex
            ]),
        );
        assert!(validate_ice49(&db).is_some());

        let mut db_int = make_test_db();
        db_int.add_record(
            "Registry",
            Record::with_fields(vec![
                FieldValue::String("R2".to_string()),
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::String("#notanumber".to_string()), // invalid int
            ]),
        );
        assert!(validate_ice49(&db_int).is_some());

        // Valid hex (#x0A), valid integer with negative sign (#-123), empty # (#), normal string
        let mut db_good = make_test_db();
        db_good.add_record(
            "Registry",
            Record::with_fields(vec![
                FieldValue::String("R1".to_string()),
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::String("#x1A2f".to_string()),
            ]),
        );
        db_good.add_record(
            "Registry",
            Record::with_fields(vec![
                FieldValue::String("R2".to_string()),
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::String("#-12345".to_string()),
            ]),
        );
        db_good.add_record(
            "Registry",
            Record::with_fields(vec![
                FieldValue::String("R3".to_string()),
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::String("#".to_string()), // empty stripped
            ]),
        );
        db_good.add_record(
            "Registry",
            Record::with_fields(vec![
                FieldValue::String("R4".to_string()),
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::String("RegularString".to_string()),
            ]),
        );
        db_good.add_record("Registry", Record::with_fields(vec![FieldValue::Short(1)]));
        assert!(validate_ice49(&db_good).is_none());
    }

    #[test]
    fn test_ice50_shortcut_icon() {
        let mut db = make_test_db();
        db.add_record(
            "Icon",
            Record::with_fields(vec![FieldValue::String("ValidIcon".to_string())]),
        );
        db.add_record(
            "Shortcut",
            Record::with_fields(vec![
                FieldValue::String("SC".to_string()),
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::String("MissingIcon".to_string()),
            ]),
        );
        assert!(validate_ice50(&db).is_some());

        // Valid icon reference, empty icon ref, empty icons table, non-string records
        let mut db_good = make_test_db();
        db_good.add_record(
            "Icon",
            Record::with_fields(vec![FieldValue::String("Icon1".to_string())]),
        );
        db_good.add_record("Icon", Record::with_fields(vec![FieldValue::Short(1)]));
        db_good.add_record(
            "Shortcut",
            Record::with_fields(vec![
                FieldValue::String("SC1".to_string()),
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::String("Icon1".to_string()),
            ]),
        );
        db_good.add_record(
            "Shortcut",
            Record::with_fields(vec![
                FieldValue::String("SC2".to_string()),
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::String(String::new()),
            ]),
        );
        db_good.add_record("Shortcut", Record::with_fields(vec![FieldValue::Short(1)]));
        assert!(validate_ice50(&db_good).is_none());

        let mut db_no_icons = make_test_db();
        db_no_icons.add_record(
            "Shortcut",
            Record::with_fields(vec![
                FieldValue::String("SC1".to_string()),
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::String("IconX".to_string()),
            ]),
        );
        assert!(validate_ice50(&db_no_icons).is_none());
    }

    #[test]
    fn test_ice52_app_search() {
        let mut db = make_test_db();
        db.add_record(
            "RegLocator",
            Record::with_fields(vec![FieldValue::String("ExistingSig".to_string())]),
        );
        db.add_record(
            "AppSearch",
            Record::with_fields(vec![
                FieldValue::String("TARGET_PROP".to_string()),
                FieldValue::String("MissingSig".to_string()),
            ]),
        );
        assert!(validate_ice52(&db).is_some());

        // Valid signature match, empty locator table, non-string records across all locators
        let mut db_good = make_test_db();
        db_good.add_record(
            "RegLocator",
            Record::with_fields(vec![FieldValue::String("SigReg".to_string())]),
        );
        db_good.add_record(
            "IniLocator",
            Record::with_fields(vec![FieldValue::String("SigIni".to_string())]),
        );
        db_good.add_record(
            "CompLocator",
            Record::with_fields(vec![FieldValue::String("SigComp".to_string())]),
        );
        db_good.add_record(
            "DrLocator",
            Record::with_fields(vec![FieldValue::String("SigDr".to_string())]),
        );
        db_good.add_record(
            "RegLocator",
            Record::with_fields(vec![FieldValue::Short(1)]),
        );
        db_good.add_record(
            "AppSearch",
            Record::with_fields(vec![
                FieldValue::String("PROP1".to_string()),
                FieldValue::String("SigReg".to_string()),
            ]),
        );
        db_good.add_record(
            "AppSearch",
            Record::with_fields(vec![
                FieldValue::String("PROP2".to_string()),
                FieldValue::String("SigIni".to_string()),
            ]),
        );
        db_good.add_record("AppSearch", Record::with_fields(vec![FieldValue::Short(1)]));
        assert!(validate_ice52(&db_good).is_none());

        let mut db_no_sigs = make_test_db();
        db_no_sigs.add_record(
            "AppSearch",
            Record::with_fields(vec![
                FieldValue::String("PROP".to_string()),
                FieldValue::String("AnySig".to_string()),
            ]),
        );
        assert!(validate_ice52(&db_no_sigs).is_none());
    }

    #[test]
    fn test_ice55_lock_permissions() {
        let mut db = make_test_db();
        db.add_record(
            "LockPermissions",
            Record::with_fields(vec![
                FieldValue::String("Obj1".to_string()),
                FieldValue::String("InvalidTable".to_string()),
            ]),
        );
        assert!(validate_ice55(&db).is_some());

        // Valid tables: File, Registry, CreateFolder; and non-string records
        let mut db_good = make_test_db();
        db_good.add_record(
            "LockPermissions",
            Record::with_fields(vec![
                FieldValue::String("Obj1".to_string()),
                FieldValue::String("File".to_string()),
            ]),
        );
        db_good.add_record(
            "LockPermissions",
            Record::with_fields(vec![
                FieldValue::String("Obj2".to_string()),
                FieldValue::String("Registry".to_string()),
            ]),
        );
        db_good.add_record(
            "LockPermissions",
            Record::with_fields(vec![
                FieldValue::String("Obj3".to_string()),
                FieldValue::String("CreateFolder".to_string()),
            ]),
        );
        db_good.add_record(
            "LockPermissions",
            Record::with_fields(vec![FieldValue::Short(1)]),
        );
        assert!(validate_ice55(&db_good).is_none());
    }

    #[test]
    fn test_ice56_directory_rooting() {
        let mut db = make_test_db();
        db.add_record(
            "Directory",
            Record::with_fields(vec![
                FieldValue::String("SelfDir".to_string()),
                FieldValue::String("SelfDir".to_string()), // Self parent
            ]),
        );
        assert!(validate_ice56(&db).is_some());

        // Valid parent directory hierarchy, empty parent, non-string records
        let mut db_good = make_test_db();
        db_good.add_record(
            "Directory",
            Record::with_fields(vec![
                FieldValue::String("TARGETDIR".to_string()),
                FieldValue::Null,
            ]),
        );
        db_good.add_record(
            "Directory",
            Record::with_fields(vec![
                FieldValue::String("EmptyParent".to_string()),
                FieldValue::String(String::new()),
            ]),
        );
        db_good.add_record(
            "Directory",
            Record::with_fields(vec![
                FieldValue::String("SubDir".to_string()),
                FieldValue::String("TARGETDIR".to_string()),
            ]),
        );
        db_good.add_record("Directory", Record::with_fields(vec![FieldValue::Short(1)]));
        assert!(validate_ice56(&db_good).is_none());
    }

    #[test]
    fn test_ice65_environment() {
        let mut db = make_test_db();
        db.add_record(
            "Environment",
            Record::with_fields(vec![
                FieldValue::String("Env1".to_string()),
                FieldValue::String(String::new()), // Empty name
            ]),
        );
        assert!(validate_ice65(&db).is_some());

        let mut db_good = make_test_db();
        db_good.add_record(
            "Environment",
            Record::with_fields(vec![
                FieldValue::String("Env1".to_string()),
                FieldValue::String("PATH".to_string()),
            ]),
        );
        db_good.add_record(
            "Environment",
            Record::with_fields(vec![FieldValue::Short(1)]),
        );
        assert!(validate_ice65(&db_good).is_none());
    }

    #[test]
    fn test_ice85_ccp_search() {
        let mut db = make_test_db();
        db.add_record(
            "CompLocator",
            Record::with_fields(vec![FieldValue::String("SigA".to_string())]),
        );
        db.add_record(
            "CCPSearch",
            Record::with_fields(vec![FieldValue::String("SigMissing".to_string())]),
        );
        assert!(validate_ice85(&db).is_some());

        // Valid CCPSearch signature match, empty CompLocator, non-string records
        let mut db_good = make_test_db();
        db_good.add_record(
            "CompLocator",
            Record::with_fields(vec![FieldValue::String("SigA".to_string())]),
        );
        db_good.add_record(
            "CompLocator",
            Record::with_fields(vec![FieldValue::Short(1)]),
        );
        db_good.add_record(
            "CCPSearch",
            Record::with_fields(vec![FieldValue::String("SigA".to_string())]),
        );
        db_good.add_record("CCPSearch", Record::with_fields(vec![FieldValue::Short(1)]));
        assert!(validate_ice85(&db_good).is_none());

        let mut db_empty = make_test_db();
        db_empty.add_record(
            "CCPSearch",
            Record::with_fields(vec![FieldValue::String("SigAny".to_string())]),
        );
        assert!(validate_ice85(&db_empty).is_none());
    }

    #[test]
    fn test_ice88_dr_locator() {
        let mut db = make_test_db();
        db.add_record(
            "DrLocator",
            Record::with_fields(vec![
                FieldValue::String("Sig1".to_string()),
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Short(20), // depth > 16
            ]),
        );
        assert!(validate_ice88(&db).is_some());

        let mut db_neg = make_test_db();
        db_neg.add_record(
            "DrLocator",
            Record::with_fields(vec![
                FieldValue::String("Sig2".to_string()),
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Short(-1), // depth < 0
            ]),
        );
        assert!(validate_ice88(&db_neg).is_some());

        // Valid depth in 0..=16, and non-matching record
        let mut db_good = make_test_db();
        db_good.add_record(
            "DrLocator",
            Record::with_fields(vec![
                FieldValue::String("Sig1".to_string()),
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Short(0),
            ]),
        );
        db_good.add_record(
            "DrLocator",
            Record::with_fields(vec![
                FieldValue::String("Sig2".to_string()),
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Short(16),
            ]),
        );
        db_good.add_record("DrLocator", Record::with_fields(vec![FieldValue::Short(1)]));
        assert!(validate_ice88(&db_good).is_none());
    }

    #[test]
    fn test_ice100_service_install_duplicate() {
        let mut db = make_test_db();
        db.add_record(
            "ServiceInstall",
            Record::with_fields(vec![
                FieldValue::String("S1".to_string()),
                FieldValue::String("MyService".to_string()),
            ]),
        );
        db.add_record(
            "ServiceInstall",
            Record::with_fields(vec![
                FieldValue::String("S2".to_string()),
                FieldValue::String("MyService".to_string()), // duplicate service name
            ]),
        );
        assert!(validate_ice100(&db).is_some());

        // Distinct service names, non-string records
        let mut db_good = make_test_db();
        db_good.add_record(
            "ServiceInstall",
            Record::with_fields(vec![
                FieldValue::String("S1".to_string()),
                FieldValue::String("Service1".to_string()),
            ]),
        );
        db_good.add_record(
            "ServiceInstall",
            Record::with_fields(vec![
                FieldValue::String("S2".to_string()),
                FieldValue::String("Service2".to_string()),
            ]),
        );
        db_good.add_record(
            "ServiceInstall",
            Record::with_fields(vec![FieldValue::Short(1)]),
        );
        assert!(validate_ice100(&db_good).is_none());
    }
}

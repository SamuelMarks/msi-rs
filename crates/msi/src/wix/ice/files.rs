//! File, font placement, companion file, and file removal ICE validation rules.
//!
//! Grounded directly in the Windows Installer File table and font specifications:
//! - Rules: ICE31, ICE54, ICE60, ICE96.

use super::types::IceReport;
use crate::database::tables::record::FieldValue;
use crate::wix::linker::LinkedDatabase;
use std::collections::{HashMap, HashSet};

/// ICE31: Verifies that components containing font files install strictly to `FontsFolder`.
///
/// # Arguments
///
/// * `db` - The [`LinkedDatabase`] to inspect.
///
/// # Returns
///
/// `Some(IceReport)` if font component directory is not `FontsFolder`.
#[must_use]
pub fn validate_ice31(db: &LinkedDatabase) -> Option<IceReport> {
    let mut file_components = HashMap::new();
    for r in db.get_records("File") {
        if let (Some(FieldValue::String(file_id)), Some(FieldValue::String(comp_id))) =
            (r.get(0), r.get(1))
        {
            file_components.insert(file_id.clone(), comp_id.clone());
        }
    }

    let mut comp_dirs = HashMap::new();
    for r in db.get_records("Component") {
        if let (Some(FieldValue::String(comp_id)), Some(FieldValue::String(dir_id))) =
            (r.get(0), r.get(2))
        {
            comp_dirs.insert(comp_id.clone(), dir_id.clone());
        }
    }

    for r in db.get_records("Font") {
        if let Some(FieldValue::String(file_ref)) = r.get(0) {
            if let Some(comp_id) = file_components.get(file_ref) {
                if let Some(dir_id) = comp_dirs.get(comp_id) {
                    if !dir_id.eq_ignore_ascii_case("FontsFolder") {
                        return Some(
                            IceReport::error(
                                "ICE31",
                                format!("Font file '{file_ref}' in component '{comp_id}' installs to directory '{dir_id}'; must install to FontsFolder"),
                            )
                            .with_table("Font"),
                        );
                    }
                }
            }
        }
    }
    None
}

/// ICE54: Verifies companion file dependency graph and detects circular version references.
///
/// # Arguments
///
/// * `db` - The [`LinkedDatabase`] to inspect.
///
/// # Returns
///
/// `Some(IceReport)` if circular companion file dependency is detected.
#[must_use]
pub fn validate_ice54(db: &LinkedDatabase) -> Option<IceReport> {
    let mut companion_map: HashMap<String, String> = HashMap::new();
    let file_ids: HashSet<String> = db
        .get_records("File")
        .iter()
        .filter_map(|r| match r.get(0) {
            Some(FieldValue::String(f)) => Some(f.clone()),
            _ => None,
        })
        .collect();

    for r in db.get_records("File") {
        if let (Some(FieldValue::String(file_id)), Some(FieldValue::String(ver))) =
            (r.get(0), r.get(3))
        {
            // If Version references another File ID, it is a companion file reference
            if file_ids.contains(ver) && ver != file_id {
                companion_map.insert(file_id.clone(), ver.clone());
            }
        }
    }

    for start in companion_map.keys() {
        let mut visited: HashSet<String> = HashSet::new();
        let mut curr = start;
        visited.insert(curr.clone());

        while let Some(next) = companion_map.get(curr) {
            if visited.contains(next) {
                return Some(
                    IceReport::error(
                        "ICE54",
                        format!("Circular companion file version reference detected involving file '{next}'"),
                    )
                    .with_table("File"),
                );
            }
            visited.insert(next.clone());
            curr = next;
        }
    }

    None
}

/// ICE60: Verifies that versioned files have a non-empty language specification.
///
/// # Arguments
///
/// * `db` - The [`LinkedDatabase`] to inspect.
///
/// # Returns
///
/// `Some(IceReport)` if a versioned file has an empty language.
#[must_use]
pub fn validate_ice60(db: &LinkedDatabase) -> Option<IceReport> {
    for r in db.get_records("File") {
        if let (Some(FieldValue::String(file_id)), Some(ver_val), Some(lang_val)) =
            (r.get(0), r.get(3), r.get(4))
        {
            let has_version = match ver_val {
                FieldValue::String(v) => !v.trim().is_empty(),
                _ => false,
            };
            let has_language = match lang_val {
                FieldValue::String(l) => !l.trim().is_empty(),
                FieldValue::Short(l) => *l != 0,
                FieldValue::Long(l) => *l != 0,
                _ => false,
            };

            if has_version && !has_language {
                return Some(
                    IceReport::warning(
                        "ICE60",
                        format!("File '{file_id}' has a version but no Language specified"),
                    )
                    .with_table("File"),
                );
            }
        }
    }
    None
}

/// ICE96: Verifies `RemoveFile` table install mode flags (1 = install, 2 = uninstall, 3 = both) and wildcards.
///
/// # Arguments
///
/// * `db` - The [`LinkedDatabase`] to inspect.
///
/// # Returns
///
/// `Some(IceReport)` if `RemoveFile` `InstallMode` flag is invalid.
#[must_use]
pub fn validate_ice96(db: &LinkedDatabase) -> Option<IceReport> {
    for r in db.get_records("RemoveFile") {
        if let (Some(FieldValue::String(rf_id)), Some(FieldValue::Short(mode))) =
            (r.get(0), r.get(4))
        {
            if *mode != 1 && *mode != 2 && *mode != 3 {
                return Some(
                    IceReport::error(
                        "ICE96",
                        format!("RemoveFile '{rf_id}' has invalid InstallMode {mode}; must be 1 (install), 2 (uninstall), or 3 (both)"),
                    )
                    .with_table("RemoveFile"),
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
    fn test_ice31_fonts_folder() {
        let mut db = make_test_db();
        db.add_record(
            "File",
            Record::with_fields(vec![
                FieldValue::String("MyFont.ttf".to_string()),
                FieldValue::String("FontComp".to_string()),
            ]),
        );
        db.add_record(
            "Component",
            Record::with_fields(vec![
                FieldValue::String("FontComp".to_string()),
                FieldValue::Null,
                FieldValue::String("AppFolder".to_string()), // Not FontsFolder!
            ]),
        );
        db.add_record(
            "Font",
            Record::with_fields(vec![
                FieldValue::String("MyFont.ttf".to_string()),
                FieldValue::String("My Font".to_string()),
            ]),
        );
        assert!(validate_ice31(&db).is_some());

        let mut db_good = make_test_db();
        db_good.add_record(
            "File",
            Record::with_fields(vec![
                FieldValue::String("MyFont.ttf".to_string()),
                FieldValue::String("FontComp".to_string()),
            ]),
        );
        db_good.add_record(
            "Component",
            Record::with_fields(vec![
                FieldValue::String("FontComp".to_string()),
                FieldValue::Null,
                FieldValue::String("FontsFolder".to_string()),
            ]),
        );
        db_good.add_record(
            "Font",
            Record::with_fields(vec![
                FieldValue::String("MyFont.ttf".to_string()),
                FieldValue::String("My Font".to_string()),
            ]),
        );
        assert!(validate_ice31(&db_good).is_none());

        // Font file not in File table
        let mut db_missing_file = make_test_db();
        db_missing_file.add_record(
            "Font",
            Record::with_fields(vec![FieldValue::String("Ghost.ttf".to_string())]),
        );
        assert!(validate_ice31(&db_missing_file).is_none());

        // Font file references comp not in Component table
        let mut db_missing_comp = make_test_db();
        db_missing_comp.add_record(
            "File",
            Record::with_fields(vec![
                FieldValue::String("Font.ttf".to_string()),
                FieldValue::String("GhostComp".to_string()),
            ]),
        );
        db_missing_comp.add_record(
            "Font",
            Record::with_fields(vec![FieldValue::String("Font.ttf".to_string())]),
        );
        assert!(validate_ice31(&db_missing_comp).is_none());

        // Non-string fields in tables
        let mut db_non_string = make_test_db();
        db_non_string.add_record("File", Record::with_fields(vec![FieldValue::Short(1)]));
        db_non_string.add_record("Component", Record::with_fields(vec![FieldValue::Short(1)]));
        db_non_string.add_record("Font", Record::with_fields(vec![FieldValue::Short(1)]));
        assert!(validate_ice31(&db_non_string).is_none());
    }

    #[test]
    fn test_ice54_companion_file_cycle() {
        let mut db = make_test_db();
        db.add_record(
            "File",
            Record::with_fields(vec![
                FieldValue::String("FileA".to_string()),
                FieldValue::String("CompA".to_string()),
                FieldValue::Null,
                FieldValue::String("FileB".to_string()), // Companion to B
            ]),
        );
        db.add_record(
            "File",
            Record::with_fields(vec![
                FieldValue::String("FileB".to_string()),
                FieldValue::String("CompB".to_string()),
                FieldValue::Null,
                FieldValue::String("FileA".to_string()), // Companion back to A!
            ]),
        );
        assert!(validate_ice54(&db).is_some());

        // A -> B where B has no companion (acyclic chain)
        let mut db_acyclic = make_test_db();
        db_acyclic.add_record(
            "File",
            Record::with_fields(vec![
                FieldValue::String("FileA".to_string()),
                FieldValue::String("CompA".to_string()),
                FieldValue::Null,
                FieldValue::String("FileB".to_string()),
            ]),
        );
        db_acyclic.add_record(
            "File",
            Record::with_fields(vec![
                FieldValue::String("FileB".to_string()),
                FieldValue::String("CompB".to_string()),
                FieldValue::Null,
                FieldValue::String("1.0.0".to_string()), // Not companion
            ]),
        );
        // Self-referencing version (ver == file_id) and non-string field
        db_acyclic.add_record(
            "File",
            Record::with_fields(vec![
                FieldValue::String("FileC".to_string()),
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::String("FileC".to_string()),
            ]),
        );
        db_acyclic.add_record("File", Record::with_fields(vec![FieldValue::Short(123)]));
        assert!(validate_ice54(&db_acyclic).is_none());
    }

    #[test]
    fn test_ice60_versioned_file_language() {
        let mut db = make_test_db();
        db.add_record(
            "File",
            Record::with_fields(vec![
                FieldValue::String("File1".to_string()),
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::String("1.0.0.0".to_string()),
                FieldValue::Null, // No language!
            ]),
        );
        assert!(validate_ice60(&db).is_some());

        let mut db_good = make_test_db();
        db_good.add_record(
            "File",
            Record::with_fields(vec![
                FieldValue::String("File1".to_string()),
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::String("1.0.0.0".to_string()),
                FieldValue::String("1033".to_string()),
            ]),
        );
        // Test Short and Long language
        db_good.add_record(
            "File",
            Record::with_fields(vec![
                FieldValue::String("File2".to_string()),
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::String("1.0.0.0".to_string()),
                FieldValue::Short(1033),
            ]),
        );
        db_good.add_record(
            "File",
            Record::with_fields(vec![
                FieldValue::String("File3".to_string()),
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::String("1.0.0.0".to_string()),
                FieldValue::Long(1033),
            ]),
        );
        // Empty version string and non-string version/record fields
        db_good.add_record(
            "File",
            Record::with_fields(vec![
                FieldValue::String("File4".to_string()),
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::String("   ".to_string()),
                FieldValue::Null,
            ]),
        );
        db_good.add_record(
            "File",
            Record::with_fields(vec![
                FieldValue::String("File5".to_string()),
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Short(1),
                FieldValue::Null,
            ]),
        );
        db_good.add_record(
            "File",
            Record::with_fields(vec![
                FieldValue::Short(1),
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
            ]),
        );
        assert!(validate_ice60(&db_good).is_none());
    }

    #[test]
    fn test_ice96_remove_file_modes() {
        let mut db = make_test_db();
        db.add_record(
            "RemoveFile",
            Record::with_fields(vec![
                FieldValue::String("RF1".to_string()),
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Short(5), // Invalid mode 5
            ]),
        );
        assert!(validate_ice96(&db).is_some());

        let mut db_good = make_test_db();
        db_good.add_record(
            "RemoveFile",
            Record::with_fields(vec![
                FieldValue::String("RF1".to_string()),
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Short(1), // Valid mode 1
            ]),
        );
        db_good.add_record(
            "RemoveFile",
            Record::with_fields(vec![
                FieldValue::String("RF2".to_string()),
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Short(2), // Valid mode 2
            ]),
        );
        db_good.add_record(
            "RemoveFile",
            Record::with_fields(vec![
                FieldValue::String("RF3".to_string()),
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Short(3), // Valid mode 3
            ]),
        );
        // Non-string or null mode field
        db_good.add_record(
            "RemoveFile",
            Record::with_fields(vec![
                FieldValue::Short(1),
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
            ]),
        );
        assert!(validate_ice96(&db_good).is_none());
    }
}

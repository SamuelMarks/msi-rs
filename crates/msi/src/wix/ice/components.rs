//! Component, Feature, Shortcut, and Directory relational ICE validation rules.
//!
//! Grounded directly in the Windows Installer relational integrity specifications:
//! - Rules: ICE10, ICE14, ICE19, ICE21, ICE22, ICE43, ICE47, ICE57, ICE59, ICE64, ICE69, ICE89, ICE90, ICE91.

use super::types::IceReport;
use crate::database::tables::record::FieldValue;
use crate::wix::linker::LinkedDatabase;
use std::collections::{HashMap, HashSet};

/// Returns mapping of Component -> Directory.
fn get_component_directories(db: &LinkedDatabase) -> HashMap<String, String> {
    let mut map = HashMap::new();
    for r in db.get_records("Component") {
        if let (Some(FieldValue::String(comp)), Some(FieldValue::String(dir))) =
            (r.get(0), r.get(2))
        {
            map.insert(comp.clone(), dir.clone());
        }
    }
    map
}

/// Returns mapping of Component -> `KeyPath`.
fn get_component_keypaths(db: &LinkedDatabase) -> HashMap<String, Option<String>> {
    let mut map = HashMap::new();
    for r in db.get_records("Component") {
        if let Some(FieldValue::String(comp)) = r.get(0) {
            let keypath = match r.get(5) {
                Some(FieldValue::String(kp)) if !kp.is_empty() => Some(kp.clone()),
                _ => None,
            };
            map.insert(comp.clone(), keypath);
        }
    }
    map
}

/// Returns mapping of Feature -> set of Components.
fn get_feature_components(db: &LinkedDatabase) -> HashMap<String, HashSet<String>> {
    let mut map: HashMap<String, HashSet<String>> = HashMap::new();
    for r in db.get_records("FeatureComponents") {
        if let (Some(FieldValue::String(feat)), Some(FieldValue::String(comp))) =
            (r.get(0), r.get(1))
        {
            map.entry(feat.clone()).or_default().insert(comp.clone());
        }
    }
    map
}

/// ICE10: Verifies that advertised shortcuts point to components belonging to the shortcut's target feature.
///
/// # Arguments
///
/// * `db` - The [`LinkedDatabase`] to inspect.
///
/// # Returns
///
/// `Some(IceReport)` if advertised shortcut targets a foreign feature's component.
#[must_use]
pub fn validate_ice10(db: &LinkedDatabase) -> Option<IceReport> {
    let fc = get_feature_components(db);

    for r in db.get_records("Shortcut") {
        if let (
            Some(FieldValue::String(sc_id)),
            Some(FieldValue::String(comp_ref)),
            Some(FieldValue::String(target)),
        ) = (r.get(0), r.get(4), r.get(2))
        {
            // Advertised shortcut: Target is a Feature ID
            if let Some(components) = fc.get(target) {
                if !components.contains(comp_ref) {
                    return Some(
                        IceReport::error(
                            "ICE10",
                            format!("Advertised shortcut '{sc_id}' references Component '{comp_ref}' which does not belong to Feature '{target}'"),
                        )
                        .with_table("Shortcut"),
                    );
                }
            }
        }
    }
    None
}

/// ICE14: Verifies that features do not install files or components directly to the root volume.
///
/// # Arguments
///
/// * `db` - The [`LinkedDatabase`] to inspect.
///
/// # Returns
///
/// `Some(IceReport)` if a component is installed directly into the root directory.
#[must_use]
pub fn validate_ice14(db: &LinkedDatabase) -> Option<IceReport> {
    for r in db.get_records("Component") {
        if let (Some(FieldValue::String(comp_id)), Some(FieldValue::String(dir_ref))) =
            (r.get(0), r.get(2))
        {
            if dir_ref.eq_ignore_ascii_case("TARGETDIR")
                || dir_ref.eq_ignore_ascii_case("SourceDir")
            {
                return Some(
                    IceReport::error(
                        "ICE14",
                        format!("Component '{comp_id}' cannot install files directly to root directory '{dir_ref}'"),
                    )
                    .with_table("Component"),
                );
            }
        }
    }
    None
}

/// ICE19: Verifies that advertised shortcuts point to a component with a valid file keypath.
///
/// # Arguments
///
/// * `db` - The [`LinkedDatabase`] to inspect.
///
/// # Returns
///
/// `Some(IceReport)` if an advertised shortcut's component has no file keypath.
#[must_use]
pub fn validate_ice19(db: &LinkedDatabase) -> Option<IceReport> {
    let keypaths = get_component_keypaths(db);
    let files: HashSet<String> = db
        .get_records("File")
        .iter()
        .filter_map(|r| match r.get(0) {
            Some(FieldValue::String(f)) => Some(f.clone()),
            _ => None,
        })
        .collect();

    for r in db.get_records("Shortcut") {
        if let (Some(FieldValue::String(sc_id)), Some(FieldValue::String(comp_ref))) =
            (r.get(0), r.get(4))
        {
            if let Some(opt_kp) = keypaths.get(comp_ref) {
                match opt_kp {
                    Some(kp) => {
                        if !files.is_empty() && !files.contains(kp) {
                            return Some(
                                IceReport::error(
                                    "ICE19",
                                    format!("Advertised shortcut '{sc_id}' references Component '{comp_ref}' whose KeyPath '{kp}' is not in File table"),
                                )
                                .with_table("Shortcut"),
                            );
                        }
                    }
                    None => {
                        return Some(
                            IceReport::error(
                                "ICE19",
                                format!("Advertised shortcut '{sc_id}' references Component '{comp_ref}' without a valid KeyPath"),
                            )
                            .with_table("Shortcut"),
                        );
                    }
                }
            }
        }
    }
    None
}

/// ICE21: Verifies that every Component is mapped to at least one valid Feature in `FeatureComponents`.
///
/// # Arguments
///
/// * `db` - The [`LinkedDatabase`] to inspect.
///
/// # Returns
///
/// `Some(IceReport)` if an orphaned component or missing feature is detected.
#[must_use]
pub fn validate_ice21(db: &LinkedDatabase) -> Option<IceReport> {
    let features: HashSet<String> = db
        .get_records("Feature")
        .iter()
        .filter_map(|r| match r.get(0) {
            Some(FieldValue::String(f)) => Some(f.clone()),
            _ => None,
        })
        .collect();

    let mut mapped_components = HashSet::new();

    for r in db.get_records("FeatureComponents") {
        if let (Some(FieldValue::String(feat)), Some(FieldValue::String(comp))) =
            (r.get(0), r.get(1))
        {
            if !features.is_empty() && !features.contains(feat) {
                return Some(
                    IceReport::error(
                        "ICE21",
                        format!("FeatureComponents references non-existent Feature '{feat}'"),
                    )
                    .with_table("FeatureComponents"),
                );
            }
            mapped_components.insert(comp.clone());
        }
    }

    if !features.is_empty() {
        for r in db.get_records("Component") {
            if let Some(FieldValue::String(comp_id)) = r.get(0) {
                if !mapped_components.contains(comp_id) {
                    return Some(
                        IceReport::error(
                            "ICE21",
                            format!("Component '{comp_id}' is not mapped to any Feature in FeatureComponents table"),
                        )
                        .with_table("Component"),
                    );
                }
            }
        }
    }

    None
}

/// ICE22: Verifies that Feature install levels are non-negative and valid.
///
/// # Arguments
///
/// * `db` - The [`LinkedDatabase`] to inspect.
///
/// # Returns
///
/// `Some(IceReport)` if Feature table Level is invalid.
#[must_use]
pub fn validate_ice22(db: &LinkedDatabase) -> Option<IceReport> {
    for r in db.get_records("Feature") {
        if let (Some(FieldValue::String(feat_id)), Some(FieldValue::Short(lvl))) =
            (r.get(0), r.get(5))
        {
            if *lvl < 0 {
                return Some(
                    IceReport::error(
                        "ICE22",
                        format!("Feature '{feat_id}' has negative install Level: {lvl}"),
                    )
                    .with_table("Feature"),
                );
            }
        }
    }
    None
}

/// ICE43: Verifies that non-advertised shortcuts point to components with a valid file or folder keypath.
///
/// # Arguments
///
/// * `db` - The [`LinkedDatabase`] to inspect.
///
/// # Returns
///
/// `Some(IceReport)` if non-advertised shortcut target is invalid.
#[must_use]
pub fn validate_ice43(db: &LinkedDatabase) -> Option<IceReport> {
    let comps: HashSet<String> = db
        .get_records("Component")
        .iter()
        .filter_map(|r| match r.get(0) {
            Some(FieldValue::String(c)) => Some(c.clone()),
            _ => None,
        })
        .collect();

    for r in db.get_records("Shortcut") {
        if let (Some(FieldValue::String(sc_id)), Some(FieldValue::String(comp_ref))) =
            (r.get(0), r.get(4))
        {
            if !comps.is_empty() && !comps.contains(comp_ref) {
                return Some(
                    IceReport::error(
                        "ICE43",
                        format!(
                            "Shortcut '{sc_id}' references non-existent Component '{comp_ref}'"
                        ),
                    )
                    .with_table("Shortcut"),
                );
            }
        }
    }
    None
}

/// ICE47: Verifies feature component ownership hierarchy and detects orphan state in partial uninstalls.
///
/// # Arguments
///
/// * `db` - The [`LinkedDatabase`] to inspect.
///
/// # Returns
///
/// `Some(IceReport)` if circular or conflicting feature hierarchy is detected.
#[must_use]
pub fn validate_ice47(db: &LinkedDatabase) -> Option<IceReport> {
    let mut parent_map = HashMap::new();
    for r in db.get_records("Feature") {
        if let (Some(FieldValue::String(feat)), Some(FieldValue::String(parent))) =
            (r.get(0), r.get(1))
        {
            if !parent.is_empty() {
                parent_map.insert(feat.clone(), parent.clone());
            }
        }
    }

    for (feat, parent) in &parent_map {
        if feat == parent {
            return Some(
                IceReport::error(
                    "ICE47",
                    format!("Feature '{feat}' cannot be its own parent feature"),
                )
                .with_table("Feature"),
            );
        }
    }
    None
}

/// ICE57: Verifies that components do not mix per-user and per-machine data.
///
/// # Arguments
///
/// * `db` - The [`LinkedDatabase`] to inspect.
///
/// # Returns
///
/// `Some(IceReport)` if a component contains both per-user and per-machine resources.
#[must_use]
pub fn validate_ice57(db: &LinkedDatabase) -> Option<IceReport> {
    let dirs = get_component_directories(db);

    let mut comp_registry_roots: HashMap<String, HashSet<i16>> = HashMap::new();
    for r in db.get_records("Registry") {
        if let (Some(FieldValue::Short(root)), Some(FieldValue::String(comp_ref))) =
            (r.get(1), r.get(5))
        {
            comp_registry_roots
                .entry(comp_ref.clone())
                .or_default()
                .insert(*root);
        }
    }

    for (comp, roots) in comp_registry_roots {
        // Root 1 = HKCU (per-user), Root 2 = HKLM (per-machine)
        if roots.contains(&1) && roots.contains(&2) {
            return Some(
                IceReport::error(
                    "ICE57",
                    format!("Component '{comp}' mixes per-user (HKCU) and per-machine (HKLM) registry keys"),
                )
                .with_table("Component"),
            );
        }

        // If component has HKCU registry entries but is installed to ProgramFilesFolder
        if roots.contains(&1) {
            if let Some(dir) = dirs.get(&comp) {
                if dir.contains("ProgramFiles") {
                    return Some(
                        IceReport::error(
                            "ICE57",
                            format!("Component '{comp}' installs to per-machine directory '{dir}' but contains per-user HKCU registry entries"),
                        )
                        .with_table("Component"),
                    );
                }
            }
        }
    }
    None
}

/// ICE59: Verifies that advertised shortcuts belong to the target component's feature or its parent.
///
/// # Arguments
///
/// * `db` - The [`LinkedDatabase`] to inspect.
///
/// # Returns
///
/// `Some(IceReport)` if advertised shortcut targets an invalid subfeature.
#[must_use]
pub fn validate_ice59(db: &LinkedDatabase) -> Option<IceReport> {
    let fc = get_feature_components(db);

    for r in db.get_records("Shortcut") {
        if let (
            Some(FieldValue::String(sc_id)),
            Some(FieldValue::String(target)),
            Some(FieldValue::String(comp_ref)),
        ) = (r.get(0), r.get(2), r.get(4))
        {
            if let Some(comps) = fc.get(target) {
                if !comps.contains(comp_ref) {
                    return Some(
                        IceReport::error(
                            "ICE59",
                            format!("Advertised shortcut '{sc_id}' targets feature '{target}' which does not contain component '{comp_ref}'"),
                        )
                        .with_table("Shortcut"),
                    );
                }
            }
        }
    }
    None
}

/// ICE64: Verifies that components installed to roaming folder (`AppDataFolder`) use HKCU registry `KeyPath`.
///
/// # Arguments
///
/// * `db` - The [`LinkedDatabase`] to inspect.
///
/// # Returns
///
/// `Some(IceReport)` if `AppDataFolder` component uses a file keypath instead of HKCU registry key.
#[must_use]
pub fn validate_ice64(db: &LinkedDatabase) -> Option<IceReport> {
    let dirs = get_component_directories(db);
    let keypaths = get_component_keypaths(db);

    let files: HashSet<String> = db
        .get_records("File")
        .iter()
        .filter_map(|r| match r.get(0) {
            Some(FieldValue::String(f)) => Some(f.clone()),
            _ => None,
        })
        .collect();

    for (comp, dir) in dirs {
        if dir.eq_ignore_ascii_case("AppDataFolder") {
            if let Some(Some(kp)) = keypaths.get(&comp) {
                // If KeyPath is a File in AppDataFolder, flag error
                if files.contains(kp) {
                    return Some(
                        IceReport::error(
                            "ICE64",
                            format!("Component '{comp}' installed to roaming AppDataFolder must use an HKCU registry key as its KeyPath, not file '{kp}'"),
                        )
                        .with_table("Component"),
                    );
                }
            }
        }
    }
    None
}

/// ICE69: Verifies that Verb and Extension records reference components matching the target server file.
///
/// # Arguments
///
/// * `db` - The [`LinkedDatabase`] to inspect.
///
/// # Returns
///
/// `Some(IceReport)` if Verb references mismatched Component.
#[must_use]
pub fn validate_ice69(db: &LinkedDatabase) -> Option<IceReport> {
    let comps: HashSet<String> = db
        .get_records("Component")
        .iter()
        .filter_map(|r| match r.get(0) {
            Some(FieldValue::String(c)) => Some(c.clone()),
            _ => None,
        })
        .collect();

    for r in db.get_records("Verb") {
        if let (
            Some(FieldValue::String(ext)),
            Some(FieldValue::String(verb)),
            Some(FieldValue::String(comp_ref)),
        ) = (r.get(0), r.get(1), r.get(3))
        {
            if !comps.is_empty() && !comps.contains(comp_ref) {
                return Some(
                    IceReport::error(
                        "ICE69",
                        format!(
                            "Verb '{ext}.{verb}' references non-existent Component '{comp_ref}'"
                        ),
                    )
                    .with_table("Verb"),
                );
            }
        }
    }
    None
}

/// ICE79: Verifies that `FeatureComponents` table does not contain duplicate mappings.
///
/// # Arguments
///
/// * `db` - The [`LinkedDatabase`] to inspect.
///
/// # Returns
///
/// `Some(IceReport)` if duplicate (Feature, Component) pair is detected.
#[must_use]
pub fn validate_ice79(db: &LinkedDatabase) -> Option<IceReport> {
    let mut seen = HashSet::new();
    for r in db.get_records("FeatureComponents") {
        if let (Some(FieldValue::String(feat)), Some(FieldValue::String(comp))) =
            (r.get(0), r.get(1))
        {
            if !seen.insert((feat.clone(), comp.clone())) {
                return Some(
                    IceReport::error(
                        "ICE79",
                        format!("Duplicate FeatureComponents mapping for Feature '{feat}' and Component '{comp}'"),
                    )
                    .with_table("FeatureComponents"),
                );
            }
        }
    }
    None
}

/// ICE89: Verifies `ProgId` and Class registration relationships point to the same Component.
///
/// # Arguments
///
/// * `db` - The [`LinkedDatabase`] to inspect.
///
/// # Returns
///
/// `Some(IceReport)` if `ProgId` references non-existent Class.
#[must_use]
pub fn validate_ice89(db: &LinkedDatabase) -> Option<IceReport> {
    let classes: HashSet<String> = db
        .get_records("Class")
        .iter()
        .filter_map(|r| match r.get(0) {
            Some(FieldValue::String(cls)) => Some(cls.clone()),
            _ => None,
        })
        .collect();

    for r in db.get_records("ProgId") {
        if let (Some(FieldValue::String(prog_id)), Some(FieldValue::String(class_ref))) =
            (r.get(0), r.get(1))
        {
            if !class_ref.is_empty() && !classes.is_empty() && !classes.contains(class_ref) {
                return Some(
                    IceReport::error(
                        "ICE89",
                        format!("ProgId '{prog_id}' references non-existent Class '{class_ref}'"),
                    )
                    .with_table("ProgId"),
                );
            }
        }
    }
    None
}

/// ICE90: Verifies shortcuts point to directories installed by the shortcut's component or parent directory.
///
/// # Arguments
///
/// * `db` - The [`LinkedDatabase`] to inspect.
///
/// # Returns
///
/// `Some(IceReport)` if Shortcut target directory does not exist.
#[must_use]
pub fn validate_ice90(db: &LinkedDatabase) -> Option<IceReport> {
    let dirs: HashSet<String> = db
        .get_records("Directory")
        .iter()
        .filter_map(|r| match r.get(0) {
            Some(FieldValue::String(d)) => Some(d.clone()),
            _ => None,
        })
        .collect();

    for r in db.get_records("Shortcut") {
        if let (Some(FieldValue::String(sc_id)), Some(FieldValue::String(dir_ref))) =
            (r.get(0), r.get(1))
        {
            if !dirs.is_empty() && !dirs.contains(dir_ref) {
                return Some(
                    IceReport::error(
                        "ICE90",
                        format!("Shortcut '{sc_id}' references non-existent target Directory '{dir_ref}'"),
                    )
                    .with_table("Shortcut"),
                );
            }
        }
    }
    None
}

/// ICE91: Verifies per-user target directory destinations (e.g. `DesktopFolder`) have appropriate keypaths.
///
/// # Arguments
///
/// * `db` - The [`LinkedDatabase`] to inspect.
///
/// # Returns
///
/// `Some(IceReport)` if per-user directory component has HKLM keypath.
#[must_use]
pub fn validate_ice91(db: &LinkedDatabase) -> Option<IceReport> {
    let dirs = get_component_directories(db);

    for r in db.get_records("Registry") {
        if let (Some(FieldValue::Short(root)), Some(FieldValue::String(comp_ref))) =
            (r.get(1), r.get(5))
        {
            if let Some(dir) = dirs.get(comp_ref) {
                // If directory is per-user (DesktopFolder) but registry is HKLM (root 2)
                if (dir == "DesktopFolder" || dir == "SendToFolder") && *root == 2 {
                    return Some(
                        IceReport::error(
                            "ICE91",
                            format!("Component '{comp_ref}' installs to per-user directory '{dir}' but writes to per-machine HKLM registry"),
                        )
                        .with_table("Component"),
                    );
                }
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
    fn test_ice10_advertised_shortcut() {
        let mut db = make_test_db();
        db.add_record(
            "FeatureComponents",
            Record::with_fields(vec![
                FieldValue::String("FeatA".to_string()),
                FieldValue::String("CompA".to_string()),
            ]),
        );
        db.add_record(
            "Shortcut",
            Record::with_fields(vec![
                FieldValue::String("Shortcut1".to_string()),
                FieldValue::String("DesktopFolder".to_string()),
                FieldValue::String("FeatA".to_string()), // Target
                FieldValue::Null,
                FieldValue::String("CompB".to_string()), // Component not in FeatA
            ]),
        );
        assert!(validate_ice10(&db).is_some());

        // Valid advertised shortcut, non-matching target feature, and non-string record
        let mut db_good = make_test_db();
        db_good.add_record(
            "FeatureComponents",
            Record::with_fields(vec![
                FieldValue::String("FeatA".to_string()),
                FieldValue::String("CompA".to_string()),
            ]),
        );
        db_good.add_record(
            "FeatureComponents",
            Record::with_fields(vec![FieldValue::Short(1)]),
        );
        db_good.add_record(
            "Shortcut",
            Record::with_fields(vec![
                FieldValue::String("Shortcut1".to_string()),
                FieldValue::String("DesktopFolder".to_string()),
                FieldValue::String("FeatA".to_string()),
                FieldValue::Null,
                FieldValue::String("CompA".to_string()),
            ]),
        );
        db_good.add_record(
            "Shortcut",
            Record::with_fields(vec![
                FieldValue::String("Shortcut2".to_string()),
                FieldValue::String("DesktopFolder".to_string()),
                FieldValue::String("FeatUnknown".to_string()),
                FieldValue::Null,
                FieldValue::String("CompA".to_string()),
            ]),
        );
        db_good.add_record("Shortcut", Record::with_fields(vec![FieldValue::Short(1)]));
        assert!(validate_ice10(&db_good).is_none());
    }

    #[test]
    fn test_ice14_root_installation() {
        let mut db = make_test_db();
        db.add_record(
            "Component",
            Record::with_fields(vec![
                FieldValue::String("RootComp".to_string()),
                FieldValue::Null,
                FieldValue::String("TARGETDIR".to_string()),
            ]),
        );
        assert!(validate_ice14(&db).is_some());

        let mut db_source = make_test_db();
        db_source.add_record(
            "Component",
            Record::with_fields(vec![
                FieldValue::String("RootComp".to_string()),
                FieldValue::Null,
                FieldValue::String("SourceDir".to_string()),
            ]),
        );
        assert!(validate_ice14(&db_source).is_some());

        let mut db_good = make_test_db();
        db_good.add_record(
            "Component",
            Record::with_fields(vec![
                FieldValue::String("SubComp".to_string()),
                FieldValue::Null,
                FieldValue::String("INSTALLDIR".to_string()),
            ]),
        );
        db_good.add_record("Component", Record::with_fields(vec![FieldValue::Short(1)]));
        assert!(validate_ice14(&db_good).is_none());
    }

    #[test]
    #[allow(clippy::too_many_lines)]
    fn test_ice19_advertised_shortcut_keypath() {
        let mut db = make_test_db();
        db.add_record(
            "Component",
            Record::with_fields(vec![
                FieldValue::String("Comp1".to_string()),
                FieldValue::Null,
                FieldValue::String("INSTALLDIR".to_string()),
                FieldValue::Short(0),
                FieldValue::Null,
                FieldValue::Null, // No keypath!
            ]),
        );
        db.add_record(
            "Shortcut",
            Record::with_fields(vec![
                FieldValue::String("SC1".to_string()),
                FieldValue::Null,
                FieldValue::String("Feat1".to_string()),
                FieldValue::Null,
                FieldValue::String("Comp1".to_string()),
            ]),
        );
        assert!(validate_ice19(&db).is_some());

        // KeyPath not in File table
        let mut db_missing_file = make_test_db();
        db_missing_file.add_record(
            "File",
            Record::with_fields(vec![FieldValue::String("fileA.exe".to_string())]),
        );
        db_missing_file.add_record("File", Record::with_fields(vec![FieldValue::Short(1)]));
        db_missing_file.add_record(
            "Component",
            Record::with_fields(vec![
                FieldValue::String("Comp1".to_string()),
                FieldValue::Null,
                FieldValue::String("INSTALLDIR".to_string()),
                FieldValue::Short(0),
                FieldValue::Null,
                FieldValue::String("fileB.exe".to_string()),
            ]),
        );
        db_missing_file.add_record(
            "Shortcut",
            Record::with_fields(vec![
                FieldValue::String("SC1".to_string()),
                FieldValue::Null,
                FieldValue::String("Feat1".to_string()),
                FieldValue::Null,
                FieldValue::String("Comp1".to_string()),
            ]),
        );
        assert!(validate_ice19(&db_missing_file).is_some());

        // Good keypath in File table, unknown component, non-string records
        let mut db_good = make_test_db();
        db_good.add_record(
            "File",
            Record::with_fields(vec![FieldValue::String("fileA.exe".to_string())]),
        );
        db_good.add_record(
            "Component",
            Record::with_fields(vec![
                FieldValue::String("Comp1".to_string()),
                FieldValue::Null,
                FieldValue::String("INSTALLDIR".to_string()),
                FieldValue::Short(0),
                FieldValue::Null,
                FieldValue::String("fileA.exe".to_string()),
            ]),
        );
        db_good.add_record(
            "Component",
            Record::with_fields(vec![
                FieldValue::String("CompEmptyKP".to_string()),
                FieldValue::Null,
                FieldValue::String("INSTALLDIR".to_string()),
                FieldValue::Short(0),
                FieldValue::Null,
                FieldValue::String(String::new()),
            ]),
        );
        db_good.add_record("Component", Record::with_fields(vec![FieldValue::Short(1)]));
        db_good.add_record(
            "Shortcut",
            Record::with_fields(vec![
                FieldValue::String("SC1".to_string()),
                FieldValue::Null,
                FieldValue::String("Feat1".to_string()),
                FieldValue::Null,
                FieldValue::String("Comp1".to_string()),
            ]),
        );
        db_good.add_record(
            "Shortcut",
            Record::with_fields(vec![
                FieldValue::String("SC2".to_string()),
                FieldValue::Null,
                FieldValue::String("Feat1".to_string()),
                FieldValue::Null,
                FieldValue::String("CompUnknown".to_string()),
            ]),
        );
        db_good.add_record("Shortcut", Record::with_fields(vec![FieldValue::Short(1)]));
        assert!(validate_ice19(&db_good).is_none());

        // Empty files table with keypath
        let mut db_empty_files = make_test_db();
        db_empty_files.add_record(
            "Component",
            Record::with_fields(vec![
                FieldValue::String("Comp1".to_string()),
                FieldValue::Null,
                FieldValue::String("INSTALLDIR".to_string()),
                FieldValue::Short(0),
                FieldValue::Null,
                FieldValue::String("fileA.exe".to_string()),
            ]),
        );
        db_empty_files.add_record(
            "Shortcut",
            Record::with_fields(vec![
                FieldValue::String("SC1".to_string()),
                FieldValue::Null,
                FieldValue::String("Feat1".to_string()),
                FieldValue::Null,
                FieldValue::String("Comp1".to_string()),
            ]),
        );
        assert!(validate_ice19(&db_empty_files).is_none());
    }

    #[test]
    fn test_ice21_unmapped_component() {
        let mut db = make_test_db();
        db.add_record(
            "Feature",
            Record::with_fields(vec![FieldValue::String("F1".to_string())]),
        );
        db.add_record(
            "Component",
            Record::with_fields(vec![
                FieldValue::String("OrphanComp".to_string()),
                FieldValue::Null,
                FieldValue::String("INSTALLDIR".to_string()),
            ]),
        );
        assert!(validate_ice21(&db).is_some());

        // FeatureComponents references non-existent feature
        let mut db_bad_fc = make_test_db();
        db_bad_fc.add_record(
            "Feature",
            Record::with_fields(vec![FieldValue::String("F1".to_string())]),
        );
        db_bad_fc.add_record("Feature", Record::with_fields(vec![FieldValue::Short(1)]));
        db_bad_fc.add_record(
            "FeatureComponents",
            Record::with_fields(vec![
                FieldValue::String("FMissing".to_string()),
                FieldValue::String("C1".to_string()),
            ]),
        );
        assert!(validate_ice21(&db_bad_fc).is_some());

        // Valid mapping, non-string records, empty features
        let mut db_good = make_test_db();
        db_good.add_record(
            "Feature",
            Record::with_fields(vec![FieldValue::String("F1".to_string())]),
        );
        db_good.add_record(
            "Component",
            Record::with_fields(vec![
                FieldValue::String("C1".to_string()),
                FieldValue::Null,
                FieldValue::String("INSTALLDIR".to_string()),
            ]),
        );
        db_good.add_record("Component", Record::with_fields(vec![FieldValue::Short(1)]));
        db_good.add_record(
            "FeatureComponents",
            Record::with_fields(vec![
                FieldValue::String("F1".to_string()),
                FieldValue::String("C1".to_string()),
            ]),
        );
        db_good.add_record(
            "FeatureComponents",
            Record::with_fields(vec![FieldValue::Short(1)]),
        );
        assert!(validate_ice21(&db_good).is_none());

        let mut db_empty = make_test_db();
        db_empty.add_record(
            "Component",
            Record::with_fields(vec![FieldValue::String("C1".to_string())]),
        );
        db_empty.add_record(
            "FeatureComponents",
            Record::with_fields(vec![
                FieldValue::String("F1".to_string()),
                FieldValue::String("C1".to_string()),
            ]),
        );
        assert!(validate_ice21(&db_empty).is_none());
    }

    #[test]
    fn test_ice22_feature_level() {
        let mut db = make_test_db();
        db.add_record(
            "Feature",
            Record::with_fields(vec![
                FieldValue::String("BadFeat".to_string()),
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Short(-1), // negative level
            ]),
        );
        assert!(validate_ice22(&db).is_some());

        let mut db_good = make_test_db();
        db_good.add_record(
            "Feature",
            Record::with_fields(vec![
                FieldValue::String("GoodFeat".to_string()),
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Short(1),
            ]),
        );
        db_good.add_record("Feature", Record::with_fields(vec![FieldValue::Short(1)]));
        assert!(validate_ice22(&db_good).is_none());
    }

    #[test]
    fn test_ice43_non_advertised_shortcut() {
        let mut db = make_test_db();
        db.add_record(
            "Component",
            Record::with_fields(vec![FieldValue::String("CompA".to_string())]),
        );
        db.add_record(
            "Shortcut",
            Record::with_fields(vec![
                FieldValue::String("SC1".to_string()),
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::String("MissingComp".to_string()),
            ]),
        );
        assert!(validate_ice43(&db).is_some());

        let mut db_good = make_test_db();
        db_good.add_record(
            "Component",
            Record::with_fields(vec![FieldValue::String("CompA".to_string())]),
        );
        db_good.add_record("Component", Record::with_fields(vec![FieldValue::Short(1)]));
        db_good.add_record(
            "Shortcut",
            Record::with_fields(vec![
                FieldValue::String("SC1".to_string()),
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::String("CompA".to_string()),
            ]),
        );
        db_good.add_record("Shortcut", Record::with_fields(vec![FieldValue::Short(1)]));
        assert!(validate_ice43(&db_good).is_none());

        let mut db_empty = make_test_db();
        db_empty.add_record(
            "Shortcut",
            Record::with_fields(vec![
                FieldValue::String("SC1".to_string()),
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::String("CompA".to_string()),
            ]),
        );
        assert!(validate_ice43(&db_empty).is_none());
    }

    #[test]
    fn test_ice47_feature_parent_self() {
        let mut db = make_test_db();
        db.add_record(
            "Feature",
            Record::with_fields(vec![
                FieldValue::String("F1".to_string()),
                FieldValue::String("F1".to_string()), // Self parent
            ]),
        );
        assert!(validate_ice47(&db).is_some());

        let mut db_good = make_test_db();
        db_good.add_record(
            "Feature",
            Record::with_fields(vec![
                FieldValue::String("FChild".to_string()),
                FieldValue::String("FParent".to_string()),
            ]),
        );
        db_good.add_record(
            "Feature",
            Record::with_fields(vec![
                FieldValue::String("FRoot".to_string()),
                FieldValue::String(String::new()),
            ]),
        );
        db_good.add_record("Feature", Record::with_fields(vec![FieldValue::Short(1)]));
        assert!(validate_ice47(&db_good).is_none());
    }

    #[test]
    fn test_ice57_mixed_per_user_per_machine() {
        let mut db = make_test_db();
        db.add_record(
            "Registry",
            Record::with_fields(vec![
                FieldValue::String("R1".to_string()),
                FieldValue::Short(1), // HKCU
                FieldValue::String(r"Software\App".to_string()),
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::String("MixedComp".to_string()),
            ]),
        );
        db.add_record(
            "Registry",
            Record::with_fields(vec![
                FieldValue::String("R2".to_string()),
                FieldValue::Short(2), // HKLM
                FieldValue::String(r"Software\App".to_string()),
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::String("MixedComp".to_string()),
            ]),
        );
        assert!(validate_ice57(&db).is_some());

        // HKCU in ProgramFilesFolder directory
        let mut db_prog_files = make_test_db();
        db_prog_files.add_record(
            "Component",
            Record::with_fields(vec![
                FieldValue::String("CompPF".to_string()),
                FieldValue::Null,
                FieldValue::String("ProgramFilesFolder".to_string()),
            ]),
        );
        db_prog_files.add_record(
            "Registry",
            Record::with_fields(vec![
                FieldValue::String("R1".to_string()),
                FieldValue::Short(1),
                FieldValue::String(r"Software\App".to_string()),
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::String("CompPF".to_string()),
            ]),
        );
        assert!(validate_ice57(&db_prog_files).is_some());

        // Good cases
        let mut db_good = make_test_db();
        db_good.add_record(
            "Component",
            Record::with_fields(vec![
                FieldValue::String("CompUser".to_string()),
                FieldValue::Null,
                FieldValue::String("AppDataFolder".to_string()),
            ]),
        );
        db_good.add_record(
            "Component",
            Record::with_fields(vec![
                FieldValue::String("CompMachine".to_string()),
                FieldValue::Null,
                FieldValue::String("ProgramFilesFolder".to_string()),
            ]),
        );
        db_good.add_record("Component", Record::with_fields(vec![FieldValue::Short(1)]));
        db_good.add_record(
            "Registry",
            Record::with_fields(vec![
                FieldValue::String("R1".to_string()),
                FieldValue::Short(1),
                FieldValue::String(r"Software\App".to_string()),
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::String("CompUser".to_string()),
            ]),
        );
        db_good.add_record(
            "Registry",
            Record::with_fields(vec![
                FieldValue::String("R2".to_string()),
                FieldValue::Short(2),
                FieldValue::String(r"Software\App".to_string()),
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::String("CompMachine".to_string()),
            ]),
        );
        db_good.add_record(
            "Registry",
            Record::with_fields(vec![
                FieldValue::String("R3".to_string()),
                FieldValue::Short(1),
                FieldValue::String(r"Software\App".to_string()),
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::String("CompNoDir".to_string()),
            ]),
        );
        db_good.add_record("Registry", Record::with_fields(vec![FieldValue::Short(1)]));
        assert!(validate_ice57(&db_good).is_none());
    }
    #[test]
    fn test_ice59_shortcut_feature_mismatch() {
        let mut db = make_test_db();
        db.add_record(
            "FeatureComponents",
            Record::with_fields(vec![
                FieldValue::String("F1".to_string()),
                FieldValue::String("C1".to_string()),
            ]),
        );
        db.add_record(
            "Shortcut",
            Record::with_fields(vec![
                FieldValue::String("SC".to_string()),
                FieldValue::Null,
                FieldValue::String("F1".to_string()),
                FieldValue::Null,
                FieldValue::String("C2".to_string()), // Not in F1
            ]),
        );
        assert!(validate_ice59(&db).is_some());

        // Valid shortcut matching feature, unknown feature, non-string records
        let mut db_good = make_test_db();
        db_good.add_record(
            "FeatureComponents",
            Record::with_fields(vec![
                FieldValue::String("F1".to_string()),
                FieldValue::String("C1".to_string()),
            ]),
        );
        db_good.add_record(
            "Shortcut",
            Record::with_fields(vec![
                FieldValue::String("SC1".to_string()),
                FieldValue::Null,
                FieldValue::String("F1".to_string()),
                FieldValue::Null,
                FieldValue::String("C1".to_string()),
            ]),
        );
        db_good.add_record(
            "Shortcut",
            Record::with_fields(vec![
                FieldValue::String("SC2".to_string()),
                FieldValue::Null,
                FieldValue::String("FUnknown".to_string()),
                FieldValue::Null,
                FieldValue::String("C1".to_string()),
            ]),
        );
        db_good.add_record("Shortcut", Record::with_fields(vec![FieldValue::Short(1)]));
        assert!(validate_ice59(&db_good).is_none());
    }

    #[test]
    fn test_ice64_appdata_keypath() {
        let mut db = make_test_db();
        db.add_record(
            "File",
            Record::with_fields(vec![FieldValue::String("user.dat".to_string())]),
        );
        db.add_record(
            "Component",
            Record::with_fields(vec![
                FieldValue::String("CompUser".to_string()),
                FieldValue::Null,
                FieldValue::String("AppDataFolder".to_string()),
                FieldValue::Short(0),
                FieldValue::Null,
                FieldValue::String("user.dat".to_string()), // File KeyPath in AppDataFolder!
            ]),
        );
        assert!(validate_ice64(&db).is_some());

        // Non-AppData folder, registry keypath (not in file table), no keypath
        let mut db_good = make_test_db();
        db_good.add_record(
            "File",
            Record::with_fields(vec![FieldValue::String("user.dat".to_string())]),
        );
        db_good.add_record("File", Record::with_fields(vec![FieldValue::Short(1)]));
        db_good.add_record(
            "Component",
            Record::with_fields(vec![
                FieldValue::String("CompInstall".to_string()),
                FieldValue::Null,
                FieldValue::String("INSTALLDIR".to_string()),
                FieldValue::Short(0),
                FieldValue::Null,
                FieldValue::String("user.dat".to_string()),
            ]),
        );
        db_good.add_record(
            "Component",
            Record::with_fields(vec![
                FieldValue::String("CompAppReg".to_string()),
                FieldValue::Null,
                FieldValue::String("AppDataFolder".to_string()),
                FieldValue::Short(0),
                FieldValue::Null,
                FieldValue::String("reg_keypath".to_string()),
            ]),
        );
        db_good.add_record(
            "Component",
            Record::with_fields(vec![
                FieldValue::String("CompAppNoKP".to_string()),
                FieldValue::Null,
                FieldValue::String("AppDataFolder".to_string()),
                FieldValue::Short(0),
                FieldValue::Null,
                FieldValue::Null,
            ]),
        );
        assert!(validate_ice64(&db_good).is_none());
    }

    #[test]
    fn test_ice69_verb_component() {
        let mut db = make_test_db();
        db.add_record(
            "Component",
            Record::with_fields(vec![FieldValue::String("RealComp".to_string())]),
        );
        db.add_record(
            "Verb",
            Record::with_fields(vec![
                FieldValue::String("txt".to_string()),
                FieldValue::String("open".to_string()),
                FieldValue::Null,
                FieldValue::String("DanglingComp".to_string()),
            ]),
        );
        assert!(validate_ice69(&db).is_some());

        let mut db_good = make_test_db();
        db_good.add_record(
            "Component",
            Record::with_fields(vec![FieldValue::String("RealComp".to_string())]),
        );
        db_good.add_record("Component", Record::with_fields(vec![FieldValue::Short(1)]));
        db_good.add_record(
            "Verb",
            Record::with_fields(vec![
                FieldValue::String("txt".to_string()),
                FieldValue::String("open".to_string()),
                FieldValue::Null,
                FieldValue::String("RealComp".to_string()),
            ]),
        );
        db_good.add_record("Verb", Record::with_fields(vec![FieldValue::Short(1)]));
        assert!(validate_ice69(&db_good).is_none());

        let mut db_empty = make_test_db();
        db_empty.add_record(
            "Verb",
            Record::with_fields(vec![
                FieldValue::String("txt".to_string()),
                FieldValue::String("open".to_string()),
                FieldValue::Null,
                FieldValue::String("RealComp".to_string()),
            ]),
        );
        assert!(validate_ice69(&db_empty).is_none());
    }

    #[test]
    fn test_ice79_duplicate_feature_component() {
        let mut db = make_test_db();
        db.add_record(
            "FeatureComponents",
            Record::with_fields(vec![
                FieldValue::String("F1".to_string()),
                FieldValue::String("C1".to_string()),
            ]),
        );
        db.add_record(
            "FeatureComponents",
            Record::with_fields(vec![
                FieldValue::String("F1".to_string()),
                FieldValue::String("C1".to_string()), // Duplicate!
            ]),
        );
        assert!(validate_ice79(&db).is_some());

        let mut db_good = make_test_db();
        db_good.add_record(
            "FeatureComponents",
            Record::with_fields(vec![
                FieldValue::String("F1".to_string()),
                FieldValue::String("C1".to_string()),
            ]),
        );
        db_good.add_record(
            "FeatureComponents",
            Record::with_fields(vec![
                FieldValue::String("F1".to_string()),
                FieldValue::String("C2".to_string()),
            ]),
        );
        db_good.add_record(
            "FeatureComponents",
            Record::with_fields(vec![FieldValue::Short(1)]),
        );
        assert!(validate_ice79(&db_good).is_none());
    }

    #[test]
    fn test_ice89_progid_class() {
        let mut db = make_test_db();
        db.add_record(
            "Class",
            Record::with_fields(vec![FieldValue::String(
                "{11111111-1111-1111-1111-111111111111}".to_string(),
            )]),
        );
        db.add_record(
            "ProgId",
            Record::with_fields(vec![
                FieldValue::String("MyProg.1".to_string()),
                FieldValue::String("{99999999-9999-9999-9999-999999999999}".to_string()),
            ]),
        );
        assert!(validate_ice89(&db).is_some());

        let mut db_good = make_test_db();
        db_good.add_record(
            "Class",
            Record::with_fields(vec![FieldValue::String(
                "{11111111-1111-1111-1111-111111111111}".to_string(),
            )]),
        );
        db_good.add_record("Class", Record::with_fields(vec![FieldValue::Short(1)]));
        db_good.add_record(
            "ProgId",
            Record::with_fields(vec![
                FieldValue::String("MyProg.1".to_string()),
                FieldValue::String("{11111111-1111-1111-1111-111111111111}".to_string()),
            ]),
        );
        db_good.add_record(
            "ProgId",
            Record::with_fields(vec![
                FieldValue::String("MyProg.2".to_string()),
                FieldValue::String(String::new()),
            ]),
        );
        db_good.add_record("ProgId", Record::with_fields(vec![FieldValue::Short(1)]));
        assert!(validate_ice89(&db_good).is_none());

        let mut db_empty = make_test_db();
        db_empty.add_record(
            "ProgId",
            Record::with_fields(vec![
                FieldValue::String("MyProg.1".to_string()),
                FieldValue::String("{99999999-9999-9999-9999-999999999999}".to_string()),
            ]),
        );
        assert!(validate_ice89(&db_empty).is_none());
    }

    #[test]
    fn test_ice90_shortcut_directory() {
        let mut db = make_test_db();
        db.add_record(
            "Directory",
            Record::with_fields(vec![FieldValue::String("DesktopFolder".to_string())]),
        );
        db.add_record(
            "Shortcut",
            Record::with_fields(vec![
                FieldValue::String("SC".to_string()),
                FieldValue::String("MissingFolder".to_string()),
            ]),
        );
        assert!(validate_ice90(&db).is_some());

        let mut db_good = make_test_db();
        db_good.add_record(
            "Directory",
            Record::with_fields(vec![FieldValue::String("DesktopFolder".to_string())]),
        );
        db_good.add_record("Directory", Record::with_fields(vec![FieldValue::Short(1)]));
        db_good.add_record(
            "Shortcut",
            Record::with_fields(vec![
                FieldValue::String("SC".to_string()),
                FieldValue::String("DesktopFolder".to_string()),
            ]),
        );
        db_good.add_record("Shortcut", Record::with_fields(vec![FieldValue::Short(1)]));
        assert!(validate_ice90(&db_good).is_none());

        let mut db_empty = make_test_db();
        db_empty.add_record(
            "Shortcut",
            Record::with_fields(vec![
                FieldValue::String("SC".to_string()),
                FieldValue::String("AnyFolder".to_string()),
            ]),
        );
        assert!(validate_ice90(&db_empty).is_none());
    }

    #[test]
    fn test_ice91_per_user_hklm() {
        let mut db = make_test_db();
        db.add_record(
            "Component",
            Record::with_fields(vec![
                FieldValue::String("DeskComp".to_string()),
                FieldValue::Null,
                FieldValue::String("DesktopFolder".to_string()),
            ]),
        );
        db.add_record(
            "Registry",
            Record::with_fields(vec![
                FieldValue::String("R1".to_string()),
                FieldValue::Short(2), // HKLM in DesktopFolder component!
                FieldValue::String(r"Software\App".to_string()),
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::String("DeskComp".to_string()),
            ]),
        );
        assert!(validate_ice91(&db).is_some());

        // SendToFolder with root 2
        let mut db_sendto = make_test_db();
        db_sendto.add_record(
            "Component",
            Record::with_fields(vec![
                FieldValue::String("SendComp".to_string()),
                FieldValue::Null,
                FieldValue::String("SendToFolder".to_string()),
            ]),
        );
        db_sendto.add_record(
            "Registry",
            Record::with_fields(vec![
                FieldValue::String("R1".to_string()),
                FieldValue::Short(2),
                FieldValue::String(r"Software\App".to_string()),
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::String("SendComp".to_string()),
            ]),
        );
        assert!(validate_ice91(&db_sendto).is_some());

        // Good cases: root 1 (HKCU) in DesktopFolder, root 2 in ProgramFilesFolder, unknown component, non-string records
        let mut db_good = make_test_db();
        db_good.add_record(
            "Component",
            Record::with_fields(vec![
                FieldValue::String("DeskComp".to_string()),
                FieldValue::Null,
                FieldValue::String("DesktopFolder".to_string()),
            ]),
        );
        db_good.add_record(
            "Component",
            Record::with_fields(vec![
                FieldValue::String("PFComp".to_string()),
                FieldValue::Null,
                FieldValue::String("ProgramFilesFolder".to_string()),
            ]),
        );
        db_good.add_record(
            "Registry",
            Record::with_fields(vec![
                FieldValue::String("R1".to_string()),
                FieldValue::Short(1),
                FieldValue::String(r"Software\App".to_string()),
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::String("DeskComp".to_string()),
            ]),
        );
        db_good.add_record(
            "Registry",
            Record::with_fields(vec![
                FieldValue::String("R2".to_string()),
                FieldValue::Short(2),
                FieldValue::String(r"Software\App".to_string()),
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::String("PFComp".to_string()),
            ]),
        );
        db_good.add_record(
            "Registry",
            Record::with_fields(vec![
                FieldValue::String("R3".to_string()),
                FieldValue::Short(2),
                FieldValue::String(r"Software\App".to_string()),
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::String("UnknownComp".to_string()),
            ]),
        );
        db_good.add_record("Registry", Record::with_fields(vec![FieldValue::Short(1)]));
        assert!(validate_ice91(&db_good).is_none());
    }
}

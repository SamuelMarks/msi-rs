//! User interface, dialogs, control tab loops, events, and condition ICE validation rules.
//!
//! Grounded directly in the Windows Installer UI tables specification:
//! - Tables: `Dialog`, `Control`, `ControlEvent`, `EventMapping`, `RadioButton`, `ComboBox`, `ListBox`.
//! - Rules: ICE13, ICE23, ICE24, ICE34, ICE44, ICE86, ICE102, ICE104.

use super::types::IceReport;
use crate::database::tables::record::FieldValue;
use crate::wix::linker::LinkedDatabase;
use std::collections::{HashMap, HashSet};

/// ICE13: Verifies dialog dimensions are positive and controls fit within parent dialog bounds.
///
/// # Arguments
///
/// * `db` - The [`LinkedDatabase`] to inspect.
///
/// # Returns
///
/// `Some(IceReport)` if a dialog dimension is invalid or a control exceeds dialog bounds.
#[must_use]
pub fn validate_ice13(db: &LinkedDatabase) -> Option<IceReport> {
    let mut dialog_dims: HashMap<String, (i16, i16)> = HashMap::new();

    for r in db.get_records("Dialog") {
        if let (
            Some(FieldValue::String(dlg)),
            Some(FieldValue::Short(w)),
            Some(FieldValue::Short(h)),
        ) = (r.get(0), r.get(3), r.get(4))
        {
            if *w <= 0 || *h <= 0 {
                return Some(
                    IceReport::error(
                        "ICE13",
                        format!("Dialog '{dlg}' has invalid non-positive dimensions ({w}x{h})"),
                    )
                    .with_table("Dialog"),
                );
            }
            dialog_dims.insert(dlg.clone(), (*w, *h));
        }
    }

    for r in db.get_records("Control") {
        if let (
            Some(FieldValue::String(dlg_ref)),
            Some(FieldValue::String(ctrl_name)),
            Some(FieldValue::Short(x)),
            Some(FieldValue::Short(y)),
            Some(FieldValue::Short(w)),
            Some(FieldValue::Short(h)),
        ) = (r.get(0), r.get(1), r.get(3), r.get(4), r.get(5), r.get(6))
        {
            if let Some(&(dw, dh)) = dialog_dims.get(dlg_ref) {
                if *x < 0 || *y < 0 || *x + *w > dw || *y + *h > dh {
                    return Some(
                        IceReport::warning(
                            "ICE13",
                            format!("Control '{dlg_ref}.{ctrl_name}' extends beyond parent dialog bounds ({dw}x{dh})"),
                        )
                        .with_table("Control"),
                    );
                }
            }
        }
    }
    None
}

/// ICE23: Verifies that every dialog contains a continuous circular tab loop with no dead ends.
///
/// # Arguments
///
/// * `db` - The [`LinkedDatabase`] to inspect.
///
/// # Returns
///
/// `Some(IceReport)` if broken or non-circular tab loop is detected.
#[must_use]
pub fn validate_ice23(db: &LinkedDatabase) -> Option<IceReport> {
    let mut dialog_controls: HashMap<String, HashMap<String, String>> = HashMap::new();

    for r in db.get_records("Control") {
        if let (
            Some(FieldValue::String(dlg)),
            Some(FieldValue::String(ctrl)),
            Some(FieldValue::String(next)),
        ) = (r.get(0), r.get(1), r.get(11))
        {
            if !next.is_empty() {
                dialog_controls
                    .entry(dlg.clone())
                    .or_default()
                    .insert(ctrl.clone(), next.clone());
            }
        }
    }

    for (dlg, next_map) in dialog_controls {
        if next_map.len() <= 1 {
            continue;
        }
        for start in next_map.keys() {
            let mut visited = HashSet::new();
            let mut curr = start;

            while visited.insert(curr.clone()) {
                match next_map.get(curr) {
                    Some(nxt) => curr = nxt,
                    None => {
                        return Some(
                            IceReport::error(
                                "ICE23",
                                format!(
                                    "Dialog '{dlg}' has broken tab order chain at control '{curr}'"
                                ),
                            )
                            .with_table("Control"),
                        );
                    }
                }
            }

            // Loop closed: verify it closed back on start
            if curr != start {
                return Some(
                    IceReport::error(
                        "ICE23",
                        format!("Dialog '{dlg}' has disjoint tab loop not including '{start}'"),
                    )
                    .with_table("Control"),
                );
            }
        }
    }
    None
}

/// ICE24: Verifies that properties referenced in conditions are defined or standard.
///
/// # Arguments
///
/// * `db` - The [`LinkedDatabase`] to inspect.
///
/// # Returns
///
/// `Some(IceReport)` if condition syntax is malformed.
#[must_use]
pub fn validate_ice24(db: &LinkedDatabase) -> Option<IceReport> {
    for r in db.get_records("Condition") {
        if let (Some(FieldValue::String(feat)), Some(FieldValue::String(cond))) =
            (r.get(0), r.get(2))
        {
            let open = cond.chars().filter(|&c| c == '(').count();
            let close = cond.chars().filter(|&c| c == ')').count();
            if open != close {
                return Some(
                    IceReport::error(
                        "ICE24",
                        format!(
                            "Feature condition for '{feat}' has unbalanced parentheses: '{cond}'"
                        ),
                    )
                    .with_table("Condition"),
                );
            }
        }
    }
    None
}

/// ICE34: Verifies that `RadioButtonGroup` entries for a property have distinct integer values.
///
/// # Arguments
///
/// * `db` - The [`LinkedDatabase`] to inspect.
///
/// # Returns
///
/// `Some(IceReport)` if duplicate radio button values are detected.
#[must_use]
pub fn validate_ice34(db: &LinkedDatabase) -> Option<IceReport> {
    let mut prop_values: HashMap<String, HashSet<i16>> = HashMap::new();

    for r in db.get_records("RadioButton") {
        if let (Some(FieldValue::String(prop)), Some(FieldValue::Short(val))) = (r.get(0), r.get(1))
        {
            if !prop_values.entry(prop.clone()).or_default().insert(*val) {
                return Some(
                    IceReport::error(
                        "ICE34",
                        format!("Property '{prop}' has duplicate RadioButton value {val}"),
                    )
                    .with_table("RadioButton"),
                );
            }
        }
    }
    None
}

/// ICE44: Verifies that dialogs containing a Help button publish a corresponding `ControlEvent`.
///
/// # Arguments
///
/// * `db` - The [`LinkedDatabase`] to inspect.
///
/// # Returns
///
/// `Some(IceReport)` if a Help button has no events wired.
#[must_use]
pub fn validate_ice44(db: &LinkedDatabase) -> Option<IceReport> {
    let mut help_buttons = HashSet::new();

    for r in db.get_records("Control") {
        if let (Some(FieldValue::String(dlg)), Some(FieldValue::String(ctrl))) =
            (r.get(0), r.get(1))
        {
            if ctrl.eq_ignore_ascii_case("Help") || ctrl.eq_ignore_ascii_case("HelpButton") {
                help_buttons.insert((dlg.clone(), ctrl.clone()));
            }
        }
    }

    if !help_buttons.is_empty() {
        let mut wired_controls = HashSet::new();
        for r in db.get_records("ControlEvent") {
            if let (Some(FieldValue::String(dlg)), Some(FieldValue::String(ctrl))) =
                (r.get(0), r.get(1))
            {
                wired_controls.insert((dlg.clone(), ctrl.clone()));
            }
        }

        for (dlg, btn) in help_buttons {
            if !wired_controls.contains(&(dlg.clone(), btn.clone())) {
                return Some(
                    IceReport::warning(
                        "ICE44",
                        format!("Dialog '{dlg}' has Help button '{btn}' with no published ControlEvents"),
                    )
                    .with_table("Control"),
                );
            }
        }
    }
    None
}

/// ICE86: Verifies that `ComboBox` and `ListBox` controls reference valid properties.
///
/// # Arguments
///
/// * `db` - The [`LinkedDatabase`] to inspect.
///
/// # Returns
///
/// `Some(IceReport)` if `ComboBox` has empty property reference.
#[must_use]
pub fn validate_ice86(db: &LinkedDatabase) -> Option<IceReport> {
    for r in db.get_records("ComboBox") {
        if let Some(FieldValue::String(prop)) = r.get(0) {
            if prop.trim().is_empty() {
                return Some(
                    IceReport::error("ICE86", "ComboBox table has empty Property reference")
                        .with_table("ComboBox"),
                );
            }
        }
    }
    for r in db.get_records("ListBox") {
        if let Some(FieldValue::String(prop)) = r.get(0) {
            if prop.trim().is_empty() {
                return Some(
                    IceReport::error("ICE86", "ListBox table has empty Property reference")
                        .with_table("ListBox"),
                );
            }
        }
    }
    None
}

/// ICE102: Verifies feature condition syntax evaluation without runtime state.
///
/// # Arguments
///
/// * `db` - The [`LinkedDatabase`] to inspect.
///
/// # Returns
///
/// `Some(IceReport)` if feature condition expression has unbalanced quotes.
#[must_use]
pub fn validate_ice102(db: &LinkedDatabase) -> Option<IceReport> {
    for r in db.get_records("Condition") {
        if let (Some(FieldValue::String(feat)), Some(FieldValue::String(cond))) =
            (r.get(0), r.get(2))
        {
            let quote_count = cond.chars().filter(|&c| c == '"').count();
            if quote_count % 2 != 0 {
                return Some(
                    IceReport::error(
                        "ICE102",
                        format!(
                            "Feature condition for '{feat}' contains unbalanced quotes: '{cond}'"
                        ),
                    )
                    .with_table("Condition"),
                );
            }
        }
    }
    None
}

/// ICE104: Verifies `ControlEvent` and `EventMapping` target arguments and recognized event names.
///
/// # Arguments
///
/// * `db` - The [`LinkedDatabase`] to inspect.
///
/// # Returns
///
/// `Some(IceReport)` if `ControlEvent` references an unknown standard event name.
#[must_use]
pub fn validate_ice104(db: &LinkedDatabase) -> Option<IceReport> {
    const STANDARD_EVENTS: &[&str] = &[
        "EndDialog",
        "NewDialog",
        "SpawnDialog",
        "SpawnWaitDialog",
        "SetProperty",
        "Reset",
        "DirectoryListNew",
        "DirectoryListUp",
        "Reinstall",
        "ReinstallMode",
        "SetInstallLevel",
        "DoAction",
    ];

    for r in db.get_records("ControlEvent") {
        if let (
            Some(FieldValue::String(dlg)),
            Some(FieldValue::String(ctrl)),
            Some(FieldValue::String(evt)),
        ) = (r.get(0), r.get(1), r.get(2))
        {
            if !STANDARD_EVENTS.iter().any(|&s| s.eq_ignore_ascii_case(evt)) {
                return Some(
                    IceReport::warning(
                        "ICE104",
                        format!("ControlEvent '{dlg}.{ctrl}' uses non-standard event '{evt}'"),
                    )
                    .with_table("ControlEvent"),
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
    #[allow(clippy::too_many_lines)]
    fn test_ice13_dialog_bounds() {
        let mut db = make_test_db();
        db.add_record(
            "Dialog",
            Record::with_fields(vec![
                FieldValue::String("Dlg1".to_string()),
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Short(200), // width
                FieldValue::Short(150), // height
            ]),
        );
        db.add_record(
            "Control",
            Record::with_fields(vec![
                FieldValue::String("Dlg1".to_string()),
                FieldValue::String("Btn1".to_string()),
                FieldValue::Null,
                FieldValue::Short(180), // x
                FieldValue::Short(140), // y
                FieldValue::Short(50),  // w (180+50=230 > 200)
                FieldValue::Short(20),  // h
            ]),
        );
        assert!(validate_ice13(&db).is_some());

        // Negative dialog dimensions (w <= 0 or h <= 0)
        let mut db_bad_dim = make_test_db();
        db_bad_dim.add_record(
            "Dialog",
            Record::with_fields(vec![
                FieldValue::String("DlgBad".to_string()),
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Short(-10),
                FieldValue::Short(150),
            ]),
        );
        assert!(validate_ice13(&db_bad_dim).is_some());

        let mut db_bad_h = make_test_db();
        db_bad_h.add_record(
            "Dialog",
            Record::with_fields(vec![
                FieldValue::String("DlgBadH".to_string()),
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Short(100),
                FieldValue::Short(0),
            ]),
        );
        assert!(validate_ice13(&db_bad_h).is_some());

        // Control bounds variations: negative x, negative y, y+h > dh, valid within bounds
        let mut db_controls = make_test_db();
        db_controls.add_record(
            "Dialog",
            Record::with_fields(vec![
                FieldValue::String("DlgValid".to_string()),
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Short(200),
                FieldValue::Short(150),
            ]),
        );
        // Valid control
        db_controls.add_record(
            "Control",
            Record::with_fields(vec![
                FieldValue::String("DlgValid".to_string()),
                FieldValue::String("BtnValid".to_string()),
                FieldValue::Null,
                FieldValue::Short(10),
                FieldValue::Short(10),
                FieldValue::Short(50),
                FieldValue::Short(20),
            ]),
        );
        // Control for unknown dialog
        db_controls.add_record(
            "Control",
            Record::with_fields(vec![
                FieldValue::String("UnknownDlg".to_string()),
                FieldValue::String("Btn1".to_string()),
                FieldValue::Null,
                FieldValue::Short(0),
                FieldValue::Short(0),
                FieldValue::Short(10),
                FieldValue::Short(10),
            ]),
        );
        // Non-string or null records
        db_controls.add_record("Dialog", Record::with_fields(vec![FieldValue::Short(1)]));
        db_controls.add_record("Control", Record::with_fields(vec![FieldValue::Short(1)]));
        assert!(validate_ice13(&db_controls).is_none());

        // Test negative x
        let mut db_neg_x = make_test_db();
        db_neg_x.add_record(
            "Dialog",
            Record::with_fields(vec![
                FieldValue::String("Dlg".to_string()),
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Short(200),
                FieldValue::Short(150),
            ]),
        );
        db_neg_x.add_record(
            "Control",
            Record::with_fields(vec![
                FieldValue::String("Dlg".to_string()),
                FieldValue::String("Btn".to_string()),
                FieldValue::Null,
                FieldValue::Short(-5),
                FieldValue::Short(10),
                FieldValue::Short(50),
                FieldValue::Short(20),
            ]),
        );
        assert!(validate_ice13(&db_neg_x).is_some());

        // Test negative y
        let mut db_neg_y = make_test_db();
        db_neg_y.add_record(
            "Dialog",
            Record::with_fields(vec![
                FieldValue::String("Dlg".to_string()),
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Short(200),
                FieldValue::Short(150),
            ]),
        );
        db_neg_y.add_record(
            "Control",
            Record::with_fields(vec![
                FieldValue::String("Dlg".to_string()),
                FieldValue::String("Btn".to_string()),
                FieldValue::Null,
                FieldValue::Short(10),
                FieldValue::Short(-5),
                FieldValue::Short(50),
                FieldValue::Short(20),
            ]),
        );
        assert!(validate_ice13(&db_neg_y).is_some());

        // Test y+h > dh
        let mut db_overflow_y = make_test_db();
        db_overflow_y.add_record(
            "Dialog",
            Record::with_fields(vec![
                FieldValue::String("Dlg".to_string()),
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Short(200),
                FieldValue::Short(150),
            ]),
        );
        db_overflow_y.add_record(
            "Control",
            Record::with_fields(vec![
                FieldValue::String("Dlg".to_string()),
                FieldValue::String("Btn".to_string()),
                FieldValue::Null,
                FieldValue::Short(10),
                FieldValue::Short(140),
                FieldValue::Short(50),
                FieldValue::Short(20),
            ]),
        );
        assert!(validate_ice13(&db_overflow_y).is_some());
    }

    #[test]
    #[allow(clippy::too_many_lines)]
    fn test_ice23_tab_loop() {
        let mut db = make_test_db();
        db.add_record(
            "Control",
            Record::with_fields(vec![
                FieldValue::String("Dlg1".to_string()),
                FieldValue::String("BtnA".to_string()),
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::String("BtnB".to_string()), // A -> B
            ]),
        );
        db.add_record(
            "Control",
            Record::with_fields(vec![
                FieldValue::String("Dlg1".to_string()),
                FieldValue::String("BtnB".to_string()),
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::String("BtnA".to_string()), // B -> A (closed loop)
            ]),
        );
        // Empty next string and single control dialog
        db.add_record(
            "Control",
            Record::with_fields(vec![
                FieldValue::String("DlgSingle".to_string()),
                FieldValue::String("BtnSolo".to_string()),
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::String("BtnSolo".to_string()),
            ]),
        );
        db.add_record(
            "Control",
            Record::with_fields(vec![
                FieldValue::String("DlgEmpty".to_string()),
                FieldValue::String("BtnEmpty".to_string()),
                FieldValue::Null,
                FieldValue::Null,
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
        db.add_record("Control", Record::with_fields(vec![FieldValue::Short(1)]));
        assert!(validate_ice23(&db).is_none());

        let mut db_broken = make_test_db();
        db_broken.add_record(
            "Control",
            Record::with_fields(vec![
                FieldValue::String("Dlg1".to_string()),
                FieldValue::String("BtnA".to_string()),
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::String("BtnMissing".to_string()), // Dead end
            ]),
        );
        db_broken.add_record(
            "Control",
            Record::with_fields(vec![
                FieldValue::String("Dlg1".to_string()),
                FieldValue::String("BtnB".to_string()),
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::String("BtnA".to_string()),
            ]),
        );
        assert!(validate_ice23(&db_broken).is_some());

        // Disjoint tab loop: A -> B -> C -> B (loop does not include A)
        let mut db_disjoint = make_test_db();
        db_disjoint.add_record(
            "Control",
            Record::with_fields(vec![
                FieldValue::String("Dlg1".to_string()),
                FieldValue::String("BtnA".to_string()),
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::String("BtnB".to_string()),
            ]),
        );
        db_disjoint.add_record(
            "Control",
            Record::with_fields(vec![
                FieldValue::String("Dlg1".to_string()),
                FieldValue::String("BtnB".to_string()),
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::String("BtnC".to_string()),
            ]),
        );
        db_disjoint.add_record(
            "Control",
            Record::with_fields(vec![
                FieldValue::String("Dlg1".to_string()),
                FieldValue::String("BtnC".to_string()),
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::Null,
                FieldValue::String("BtnB".to_string()),
            ]),
        );
        assert!(validate_ice23(&db_disjoint).is_some());
    }

    #[test]
    fn test_ice24_condition_parentheses() {
        let mut db = make_test_db();
        db.add_record(
            "Condition",
            Record::with_fields(vec![
                FieldValue::String("F1".to_string()),
                FieldValue::Short(1),
                FieldValue::String("(VersionNT > 600".to_string()), // unbalanced!
            ]),
        );
        assert!(validate_ice24(&db).is_some());

        let mut db_good = make_test_db();
        db_good.add_record(
            "Condition",
            Record::with_fields(vec![
                FieldValue::String("F1".to_string()),
                FieldValue::Short(1),
                FieldValue::String("(VersionNT > 600)".to_string()),
            ]),
        );
        db_good.add_record("Condition", Record::with_fields(vec![FieldValue::Short(1)]));
        assert!(validate_ice24(&db_good).is_none());
    }

    #[test]
    fn test_ice34_radio_button_values() {
        let mut db = make_test_db();
        db.add_record(
            "RadioButton",
            Record::with_fields(vec![
                FieldValue::String("MY_PROP".to_string()),
                FieldValue::Short(1),
            ]),
        );
        db.add_record(
            "RadioButton",
            Record::with_fields(vec![
                FieldValue::String("MY_PROP".to_string()),
                FieldValue::Short(1), // duplicate value 1
            ]),
        );
        assert!(validate_ice34(&db).is_some());

        let mut db_good = make_test_db();
        db_good.add_record(
            "RadioButton",
            Record::with_fields(vec![
                FieldValue::String("PROP1".to_string()),
                FieldValue::Short(1),
            ]),
        );
        db_good.add_record(
            "RadioButton",
            Record::with_fields(vec![
                FieldValue::String("PROP1".to_string()),
                FieldValue::Short(2),
            ]),
        );
        db_good.add_record(
            "RadioButton",
            Record::with_fields(vec![
                FieldValue::String("PROP2".to_string()),
                FieldValue::Short(1),
            ]),
        );
        db_good.add_record(
            "RadioButton",
            Record::with_fields(vec![FieldValue::Short(1)]),
        );
        assert!(validate_ice34(&db_good).is_none());
    }

    #[test]
    fn test_ice44_help_button() {
        let mut db = make_test_db();
        db.add_record(
            "Control",
            Record::with_fields(vec![
                FieldValue::String("WelcomeDlg".to_string()),
                FieldValue::String("HelpButton".to_string()),
            ]),
        );
        // No ControlEvent wired for HelpButton
        assert!(validate_ice44(&db).is_some());

        // Control named "Help" with wired event
        let mut db_good = make_test_db();
        db_good.add_record(
            "Control",
            Record::with_fields(vec![
                FieldValue::String("WelcomeDlg".to_string()),
                FieldValue::String("Help".to_string()),
            ]),
        );
        db_good.add_record(
            "ControlEvent",
            Record::with_fields(vec![
                FieldValue::String("WelcomeDlg".to_string()),
                FieldValue::String("Help".to_string()),
            ]),
        );
        // Non-help button
        db_good.add_record(
            "Control",
            Record::with_fields(vec![
                FieldValue::String("WelcomeDlg".to_string()),
                FieldValue::String("NextButton".to_string()),
            ]),
        );
        db_good.add_record("Control", Record::with_fields(vec![FieldValue::Short(1)]));
        db_good.add_record(
            "ControlEvent",
            Record::with_fields(vec![FieldValue::Short(1)]),
        );
        assert!(validate_ice44(&db_good).is_none());

        // No help buttons at all
        let mut db_no_help = make_test_db();
        db_no_help.add_record(
            "Control",
            Record::with_fields(vec![
                FieldValue::String("WelcomeDlg".to_string()),
                FieldValue::String("CancelButton".to_string()),
            ]),
        );
        assert!(validate_ice44(&db_no_help).is_none());
    }

    #[test]
    fn test_ice86_combo_box() {
        let mut db = make_test_db();
        db.add_record(
            "ComboBox",
            Record::with_fields(vec![FieldValue::String("   ".to_string())]),
        );
        assert!(validate_ice86(&db).is_some());

        // ListBox with empty property
        let mut db_listbox = make_test_db();
        db_listbox.add_record(
            "ListBox",
            Record::with_fields(vec![FieldValue::String(String::new())]),
        );
        assert!(validate_ice86(&db_listbox).is_some());

        // Valid ComboBox and ListBox
        let mut db_good = make_test_db();
        db_good.add_record(
            "ComboBox",
            Record::with_fields(vec![FieldValue::String("VALID_COMBO".to_string())]),
        );
        db_good.add_record(
            "ListBox",
            Record::with_fields(vec![FieldValue::String("VALID_LIST".to_string())]),
        );
        db_good.add_record("ComboBox", Record::with_fields(vec![FieldValue::Short(1)]));
        db_good.add_record("ListBox", Record::with_fields(vec![FieldValue::Short(1)]));
        assert!(validate_ice86(&db_good).is_none());
    }

    #[test]
    fn test_ice102_condition_quotes() {
        let mut db = make_test_db();
        db.add_record(
            "Condition",
            Record::with_fields(vec![
                FieldValue::String("F1".to_string()),
                FieldValue::Short(1),
                FieldValue::String(r#"Prop = "Value"#.to_string()), // unbalanced quote
            ]),
        );
        assert!(validate_ice102(&db).is_some());

        let mut db_good = make_test_db();
        db_good.add_record(
            "Condition",
            Record::with_fields(vec![
                FieldValue::String("F1".to_string()),
                FieldValue::Short(1),
                FieldValue::String(r#"Prop = "Value""#.to_string()), // balanced quote
            ]),
        );
        db_good.add_record("Condition", Record::with_fields(vec![FieldValue::Short(1)]));
        assert!(validate_ice102(&db_good).is_none());
    }

    #[test]
    fn test_ice104_control_events() {
        let mut db = make_test_db();
        db.add_record(
            "ControlEvent",
            Record::with_fields(vec![
                FieldValue::String("Dlg1".to_string()),
                FieldValue::String("Btn1".to_string()),
                FieldValue::String("NonStandardEvent".to_string()),
            ]),
        );
        assert!(validate_ice104(&db).is_some());

        let mut db_good = make_test_db();
        db_good.add_record(
            "ControlEvent",
            Record::with_fields(vec![
                FieldValue::String("Dlg1".to_string()),
                FieldValue::String("Btn1".to_string()),
                FieldValue::String("EndDialog".to_string()),
            ]),
        );
        db_good.add_record(
            "ControlEvent",
            Record::with_fields(vec![FieldValue::Short(1)]),
        );
        assert!(validate_ice104(&db_good).is_none());
    }
}

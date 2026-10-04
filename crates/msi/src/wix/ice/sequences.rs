//! Phase 2: Sequence tables, action graph analysis, and execution pipeline ICE validation rules.
//!
//! Grounded directly in the Windows Installer execution pipeline specifications:
//! - Sequence tables: `InstallExecuteSequence`, `InstallUISequence`, `AdminExecuteSequence`, `AdvtExecuteSequence`.
//! - Rules: ICE11, ICE12, ICE26, ICE27, ICE28, ICE42, ICE63, ICE67, ICE68, ICE72, ICE75, ICE77, ICE78.

use super::types::IceReport;
use crate::database::tables::record::FieldValue;
use crate::wix::linker::LinkedDatabase;
use std::collections::{HashMap, HashSet};

/// In-memory representation of an ordered sequence table action entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SequenceEntry {
    /// Action identifier or custom action name.
    pub action: String,
    /// Optional execution condition expression.
    pub condition: Option<String>,
    /// Integer execution sequence position.
    pub sequence: i16,
}

/// Strongly-typed sequence table graph analyzer.
#[derive(Debug, Clone, Default)]
pub struct SequenceGraph {
    /// Sequence entries in ascending execution order.
    pub entries: Vec<SequenceEntry>,
}

impl SequenceGraph {
    /// Builds a [`SequenceGraph`] from a specific sequence table in a [`LinkedDatabase`].
    ///
    /// # Arguments
    ///
    /// * `db` - Database to query.
    /// * `table` - Name of sequence table (e.g. `InstallExecuteSequence`).
    ///
    /// # Returns
    ///
    /// A sorted [`SequenceGraph`].
    #[must_use]
    pub fn from_table(db: &LinkedDatabase, table: &str) -> Self {
        let mut entries = Vec::new();
        for r in db.get_records(table) {
            if let (Some(FieldValue::String(action)), Some(FieldValue::Short(seq))) =
                (r.get(0), r.get(2))
            {
                let condition = match r.get(1) {
                    Some(FieldValue::String(c)) if !c.trim().is_empty() => Some(c.clone()),
                    _ => None,
                };
                entries.push(SequenceEntry {
                    action: action.clone(),
                    condition,
                    sequence: *seq,
                });
            }
        }
        entries.sort_by_key(|e| e.sequence);
        Self { entries }
    }

    /// Finds the sequence number of an action, if present.
    ///
    /// # Arguments
    ///
    /// * `action` - Action name to find.
    ///
    /// # Returns
    ///
    /// Optional sequence number.
    #[must_use]
    pub fn sequence_of(&self, action: &str) -> Option<i16> {
        self.entries
            .iter()
            .find(|e| e.action.eq_ignore_ascii_case(action))
            .map(|e| e.sequence)
    }

    /// Returns whether an action is scheduled strictly between two boundary actions.
    ///
    /// # Arguments
    ///
    /// * `action` - Action name to test.
    /// * `start_action` - Beginning boundary action (e.g. `InstallInitialize`).
    /// * `end_action` - Ending boundary action (e.g. `InstallFinalize`).
    ///
    /// # Returns
    ///
    /// `true` if `action` sequence position is strictly greater than `start_action`
    /// and strictly less than `end_action`.
    #[must_use]
    pub fn is_between(&self, action: &str, start_action: &str, end_action: &str) -> bool {
        match (
            self.sequence_of(action),
            self.sequence_of(start_action),
            self.sequence_of(end_action),
        ) {
            (Some(act_seq), Some(start_seq), Some(end_seq)) => {
                act_seq > start_seq && act_seq < end_seq
            }
            _ => false,
        }
    }
}

/// Helper returning map of custom action types from `CustomAction` table.
fn get_custom_actions(db: &LinkedDatabase) -> HashMap<String, (i16, String, String)> {
    let mut map = HashMap::new();
    for r in db.get_records("CustomAction") {
        if let (
            Some(FieldValue::String(name)),
            Some(FieldValue::Short(action_type)),
            Some(FieldValue::String(source)),
            Some(FieldValue::String(target)),
        ) = (r.get(0), r.get(1), r.get(2), r.get(3))
        {
            map.insert(name.clone(), (*action_type, source.clone(), target.clone()));
        }
    }
    map
}

/// ICE11: Verifies that nested MSI custom actions (type 7) are not scheduled inside the main installation transaction.
///
/// # Arguments
///
/// * `db` - The [`LinkedDatabase`] to inspect.
///
/// # Returns
///
/// `Some(IceReport)` if nested installation action is sequenced inside transaction boundaries.
#[must_use]
pub fn validate_ice11(db: &LinkedDatabase) -> Option<IceReport> {
    let cas = get_custom_actions(db);
    let graph = SequenceGraph::from_table(db, "InstallExecuteSequence");

    for (name, &(typ, _, _)) in &cas {
        // Type 7 = Concurrent/Nested installation
        if (typ & 0x0007) == 7 && graph.is_between(name, "InstallInitialize", "InstallFinalize") {
            return Some(
                IceReport::error(
                    "ICE11",
                    format!("Nested installation action '{name}' cannot be scheduled between InstallInitialize and InstallFinalize"),
                )
                .with_table("InstallExecuteSequence"),
            );
        }
    }
    None
}

/// ICE12: Validates target existence in File, Property, or Directory for custom action types 17, 18, 19, 21, 22.
///
/// # Arguments
///
/// * `db` - The [`LinkedDatabase`] to inspect.
///
/// # Returns
///
/// `Some(IceReport)` if custom action target is missing.
#[must_use]
pub fn validate_ice12(db: &LinkedDatabase) -> Option<IceReport> {
    let cas = get_custom_actions(db);

    let files: HashSet<String> = db
        .get_records("File")
        .iter()
        .filter_map(|r| match r.get(0) {
            Some(FieldValue::String(s)) => Some(s.clone()),
            _ => None,
        })
        .collect();

    let properties: HashSet<String> = db
        .get_records("Property")
        .iter()
        .filter_map(|r| match r.get(0) {
            Some(FieldValue::String(s)) => Some(s.clone()),
            _ => None,
        })
        .collect();

    for (name, &(typ, ref source, _)) in &cas {
        let base_type = typ & 0x003F;
        if (base_type == 17 || base_type == 21) && !files.is_empty() && !files.contains(source) {
            return Some(
                IceReport::error(
                    "ICE12",
                    format!("Custom action '{name}' references non-existent File '{source}'"),
                )
                .with_table("CustomAction"),
            );
        }
        if (base_type == 18 || base_type == 22)
            && !properties.is_empty()
            && !properties.contains(source)
        {
            return Some(
                IceReport::error(
                    "ICE12",
                    format!("Custom action '{name}' references non-existent Property '{source}'"),
                )
                .with_table("CustomAction"),
            );
        }
    }
    None
}

/// ICE26: Verifies sequence order of actions requiring administrative elevation.
///
/// # Arguments
///
/// * `db` - The [`LinkedDatabase`] to inspect.
///
/// # Returns
///
/// `Some(IceReport)` if elevated standard actions are sequenced outside the execution transaction.
#[must_use]
pub fn validate_ice26(db: &LinkedDatabase) -> Option<IceReport> {
    const ELEVATED_ACTIONS: &[&str] = &[
        "InstallFiles",
        "WriteRegistryValues",
        "RegisterProduct",
        "InstallServices",
        "StartServices",
        "StopServices",
        "DeleteServices",
    ];

    let graph = SequenceGraph::from_table(db, "InstallExecuteSequence");
    let init_seq = graph.sequence_of("InstallInitialize");
    let fin_seq = graph.sequence_of("InstallFinalize");

    if let (Some(init), Some(fin)) = (init_seq, fin_seq) {
        for &act in ELEVATED_ACTIONS {
            if let Some(seq) = graph.sequence_of(act) {
                if seq <= init || seq >= fin {
                    return Some(
                        IceReport::error(
                            "ICE26",
                            format!("Action '{act}' requires administrative elevation and must be sequenced between InstallInitialize and InstallFinalize"),
                        )
                        .with_table("InstallExecuteSequence"),
                    );
                }
            }
        }
    }
    None
}

/// ICE27: Verifies sequence organization continuity between `InstallUISequence` and `InstallExecuteSequence`.
///
/// # Arguments
///
/// * `db` - The [`LinkedDatabase`] to inspect.
///
/// # Returns
///
/// `Some(IceReport)` if costing or validation sequences are desynchronized.
#[must_use]
pub fn validate_ice27(db: &LinkedDatabase) -> Option<IceReport> {
    let ui_graph = SequenceGraph::from_table(db, "InstallUISequence");
    let exec_graph = SequenceGraph::from_table(db, "InstallExecuteSequence");

    let cost_init_ui = ui_graph.sequence_of("CostInitialize");
    let file_cost_ui = ui_graph.sequence_of("FileCost");
    let cost_fin_ui = ui_graph.sequence_of("CostFinalize");

    if let (Some(c_init), Some(f_cost)) = (cost_init_ui, file_cost_ui) {
        if c_init >= f_cost {
            return Some(
                IceReport::error(
                    "ICE27",
                    "CostInitialize must be sequenced before FileCost in InstallUISequence",
                )
                .with_table("InstallUISequence"),
            );
        }
    }
    if let (Some(f_cost), Some(c_fin)) = (file_cost_ui, cost_fin_ui) {
        if f_cost >= c_fin {
            return Some(
                IceReport::error(
                    "ICE27",
                    "FileCost must be sequenced before CostFinalize in InstallUISequence",
                )
                .with_table("InstallUISequence"),
            );
        }
    }

    let cost_init_ex = exec_graph.sequence_of("CostInitialize");
    let file_cost_ex = exec_graph.sequence_of("FileCost");
    let cost_fin_ex = exec_graph.sequence_of("CostFinalize");

    if let (Some(c_init), Some(f_cost)) = (cost_init_ex, file_cost_ex) {
        if c_init >= f_cost {
            return Some(
                IceReport::error(
                    "ICE27",
                    "CostInitialize must be sequenced before FileCost in InstallExecuteSequence",
                )
                .with_table("InstallExecuteSequence"),
            );
        }
    }
    if let (Some(f_cost), Some(c_fin)) = (file_cost_ex, cost_fin_ex) {
        if f_cost >= c_fin {
            return Some(
                IceReport::error(
                    "ICE27",
                    "FileCost must be sequenced before CostFinalize in InstallExecuteSequence",
                )
                .with_table("InstallExecuteSequence"),
            );
        }
    }

    None
}

/// ICE28: Verifies that `ForceReboot` is sequenced between `InstallInitialize` and `InstallFinalize` and is conditional.
///
/// # Arguments
///
/// * `db` - The [`LinkedDatabase`] to inspect.
///
/// # Returns
///
/// `Some(IceReport)` if `ForceReboot` is placed or conditioned incorrectly.
#[must_use]
pub fn validate_ice28(db: &LinkedDatabase) -> Option<IceReport> {
    let graph = SequenceGraph::from_table(db, "InstallExecuteSequence");
    for entry in &graph.entries {
        if entry.action.eq_ignore_ascii_case("ForceReboot") {
            if entry.condition.is_none() {
                return Some(
                    IceReport::error(
                        "ICE28",
                        "ForceReboot action in InstallExecuteSequence must have a conditional expression",
                    )
                    .with_table("InstallExecuteSequence"),
                );
            }
            if !graph.is_between("ForceReboot", "InstallInitialize", "InstallFinalize") {
                return Some(
                    IceReport::error(
                        "ICE28",
                        "ForceReboot action must be sequenced between InstallInitialize and InstallFinalize",
                    )
                    .with_table("InstallExecuteSequence"),
                );
            }
        }
    }
    None
}

/// ICE42: Verifies that in-script deferred actions do not query standard non-CustomActionData properties.
///
/// # Arguments
///
/// * `db` - The [`LinkedDatabase`] to inspect.
///
/// # Returns
///
/// `Some(IceReport)` if deferred custom action directly accesses forbidden session property.
#[must_use]
pub fn validate_ice42(db: &LinkedDatabase) -> Option<IceReport> {
    let cas = get_custom_actions(db);
    for (name, &(typ, _, ref target)) in &cas {
        // Deferred flag = 0x0400
        if (typ & 0x0400) != 0 {
            let lower = target.to_ascii_lowercase();
            if lower.contains("session.property") && !lower.contains("customactiondata") {
                return Some(
                    IceReport::error(
                        "ICE42",
                        format!("Deferred custom action '{name}' cannot access Session.Property directly; must use CustomActionData"),
                    )
                    .with_table("CustomAction"),
                );
            }
        }
    }
    None
}

/// ICE63: Verifies that duplicate actions in sequence tables have mutually exclusive conditions.
///
/// # Arguments
///
/// * `db` - The [`LinkedDatabase`] to inspect.
///
/// # Returns
///
/// `Some(IceReport)` if duplicate sequence actions have identical or missing conditions.
#[must_use]
pub fn validate_ice63(db: &LinkedDatabase) -> Option<IceReport> {
    let graph = SequenceGraph::from_table(db, "InstallExecuteSequence");
    let mut action_conditions: HashMap<String, Vec<Option<String>>> = HashMap::new();

    for entry in &graph.entries {
        action_conditions
            .entry(entry.action.clone())
            .or_default()
            .push(entry.condition.clone());
    }

    for (act, conditions) in action_conditions {
        if conditions.len() > 1 {
            if conditions.iter().any(Option::is_none) {
                return Some(
                    IceReport::error(
                        "ICE63",
                        format!("Duplicate action '{act}' in InstallExecuteSequence must have conditional expressions on all instances"),
                    )
                    .with_table("InstallExecuteSequence"),
                );
            }
            let mut unique_conditions = HashSet::new();
            for cond in conditions {
                if !unique_conditions.insert(cond) {
                    return Some(
                        IceReport::error(
                            "ICE63",
                            format!("Duplicate action '{act}' in InstallExecuteSequence has identical conditional expressions"),
                        )
                        .with_table("InstallExecuteSequence"),
                    );
                }
            }
        }
    }
    None
}

/// ICE67: Verifies that non-standard custom actions are not scheduled outside allowed sequence boundaries.
///
/// # Arguments
///
/// * `db` - The [`LinkedDatabase`] to inspect.
///
/// # Returns
///
/// `Some(IceReport)` if custom action is scheduled before `CostInitialize`.
#[must_use]
pub fn validate_ice67(db: &LinkedDatabase) -> Option<IceReport> {
    let cas = get_custom_actions(db);
    let graph = SequenceGraph::from_table(db, "InstallExecuteSequence");

    if let Some(cost_init) = graph.sequence_of("CostInitialize") {
        for entry in &graph.entries {
            if cas.contains_key(&entry.action) && entry.sequence > 0 && entry.sequence < cost_init {
                return Some(
                    IceReport::error(
                        "ICE67",
                        format!(
                            "Custom action '{}' cannot be scheduled before CostInitialize",
                            entry.action
                        ),
                    )
                    .with_table("InstallExecuteSequence"),
                );
            }
        }
    }
    None
}

/// ICE68: Validates custom action execution flags (Commit, Rollback, Deferred pairing).
///
/// # Arguments
///
/// * `db` - The [`LinkedDatabase`] to inspect.
///
/// # Returns
///
/// `Some(IceReport)` if custom action execution flags are conflicting or rollback action is unpaired.
#[must_use]
pub fn validate_ice68(db: &LinkedDatabase) -> Option<IceReport> {
    let cas = get_custom_actions(db);
    for (name, &(typ, _, _)) in &cas {
        let is_deferred = (typ & 0x0400) != 0;
        let is_rollback = (typ & 0x0100) != 0;
        let is_commit = (typ & 0x0200) != 0;

        if is_rollback && is_commit {
            return Some(
                IceReport::error(
                    "ICE68",
                    format!("Custom action '{name}' cannot have both Rollback (0x100) and Commit (0x200) flags set"),
                )
                .with_table("CustomAction"),
            );
        }
        if (is_rollback || is_commit) && !is_deferred {
            return Some(
                IceReport::error(
                    "ICE68",
                    format!("Custom action '{name}' with Rollback or Commit must also specify the Deferred (0x400) flag"),
                )
                .with_table("CustomAction"),
            );
        }
    }
    None
}

/// ICE72: Validates custom action Source type consistency with referenced tables.
///
/// # Arguments
///
/// * `db` - The [`LinkedDatabase`] to inspect.
///
/// # Returns
///
/// `Some(IceReport)` if `CustomAction` Source references a missing record in the appropriate table.
#[must_use]
pub fn validate_ice72(db: &LinkedDatabase) -> Option<IceReport> {
    let binaries: HashSet<String> = db
        .get_records("Binary")
        .iter()
        .filter_map(|r| match r.get(0) {
            Some(FieldValue::String(s)) => Some(s.clone()),
            _ => None,
        })
        .collect();

    for (name, &(typ, ref source, _)) in &get_custom_actions(db) {
        let base_type = typ & 0x0007;
        // Base type 1 = DLL stored in Binary table
        if base_type == 1 && !binaries.is_empty() && !binaries.contains(source) {
            return Some(
                IceReport::error(
                    "ICE72",
                    format!("Custom action '{name}' references non-existent Binary '{source}'"),
                )
                .with_table("CustomAction"),
            );
        }
    }
    None
}

/// ICE75: Verifies that custom actions modifying system state are marked Deferred.
///
/// # Arguments
///
/// * `db` - The [`LinkedDatabase`] to inspect.
///
/// # Returns
///
/// `Some(IceReport)` if modifying action is executed immediately.
#[must_use]
pub fn validate_ice75(db: &LinkedDatabase) -> Option<IceReport> {
    let graph = SequenceGraph::from_table(db, "InstallExecuteSequence");
    let cas = get_custom_actions(db);

    for (name, &(typ, _, ref target)) in &cas {
        let is_deferred = (typ & 0x0400) != 0;
        let lower = target.to_ascii_lowercase();
        let modifies_system =
            lower.contains("delete") || lower.contains("create") || lower.contains("install");

        if modifies_system && !is_deferred && graph.sequence_of(name).is_some() {
            return Some(
                IceReport::error(
                    "ICE75",
                    format!("Custom action '{name}' modifies system state and must be marked as Deferred (0x400)"),
                )
                .with_table("CustomAction"),
            );
        }
    }
    None
}

/// ICE77: Verifies that all deferred custom actions are scheduled strictly between `InstallInitialize` and `InstallFinalize`.
///
/// # Arguments
///
/// * `db` - The [`LinkedDatabase`] to inspect.
///
/// # Returns
///
/// `Some(IceReport)` if deferred action is scheduled outside the transaction script.
#[must_use]
pub fn validate_ice77(db: &LinkedDatabase) -> Option<IceReport> {
    let graph = SequenceGraph::from_table(db, "InstallExecuteSequence");
    let cas = get_custom_actions(db);

    for (name, &(typ, _, _)) in &cas {
        if (typ & 0x0400) != 0
            && graph.sequence_of(name).is_some()
            && !graph.is_between(name, "InstallInitialize", "InstallFinalize")
        {
            return Some(
                IceReport::error(
                    "ICE77",
                    format!("Deferred custom action '{name}' must be scheduled between InstallInitialize and InstallFinalize"),
                )
                .with_table("InstallExecuteSequence"),
            );
        }
    }
    None
}

/// ICE78: Verifies that `AdvtExecuteSequence` only contains actions supported during advertisement.
///
/// # Arguments
///
/// * `db` - The [`LinkedDatabase`] to inspect.
///
/// # Returns
///
/// `Some(IceReport)` if disallowed action is found in `AdvtExecuteSequence`.
#[must_use]
pub fn validate_ice78(db: &LinkedDatabase) -> Option<IceReport> {
    const DISALLOWED_ACTIONS: &[&str] = &[
        "InstallFiles",
        "WriteRegistryValues",
        "CreateShortcuts",
        "InstallServices",
        "StartServices",
    ];

    for r in db.get_records("AdvtExecuteSequence") {
        if let Some(FieldValue::String(act)) = r.get(0) {
            if DISALLOWED_ACTIONS.iter().any(|&d| d == act) {
                return Some(
                    IceReport::error(
                        "ICE78",
                        format!("Action '{act}' is not permitted in AdvtExecuteSequence"),
                    )
                    .with_table("AdvtExecuteSequence"),
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
    fn test_sequence_graph_basic() {
        let mut db = make_test_db();
        db.add_record(
            "InstallExecuteSequence",
            Record::with_fields(vec![
                FieldValue::String("CostInitialize".to_string()),
                FieldValue::String("NOT Installed".to_string()),
                FieldValue::Short(800),
            ]),
        );
        db.add_record(
            "InstallExecuteSequence",
            Record::with_fields(vec![
                FieldValue::String("InstallInitialize".to_string()),
                FieldValue::String("   ".to_string()),
                FieldValue::Short(1500),
            ]),
        );
        db.add_record(
            "InstallExecuteSequence",
            Record::with_fields(vec![
                FieldValue::String("InstallFiles".to_string()),
                FieldValue::Null,
                FieldValue::Short(2000),
            ]),
        );
        db.add_record(
            "InstallExecuteSequence",
            Record::with_fields(vec![
                FieldValue::String("InstallFinalize".to_string()),
                FieldValue::Null,
                FieldValue::Short(6600),
            ]),
        );
        db.add_record(
            "InstallExecuteSequence",
            Record::with_fields(vec![FieldValue::Short(1)]),
        );

        let graph = SequenceGraph::from_table(&db, "InstallExecuteSequence");
        assert_eq!(graph.sequence_of("CostInitialize"), Some(800));
        assert_eq!(graph.sequence_of("MissingAction"), None);
        assert!(graph.is_between("InstallFiles", "InstallInitialize", "InstallFinalize"));
        assert!(!graph.is_between("CostInitialize", "InstallInitialize", "InstallFinalize"));
        assert!(!graph.is_between("Missing", "InstallInitialize", "InstallFinalize"));
        assert!(!graph.is_between("InstallFiles", "Missing", "InstallFinalize"));
        assert!(!graph.is_between("InstallFiles", "InstallInitialize", "Missing"));

        // Traits
        let entry = SequenceEntry {
            action: "Act".to_string(),
            condition: Some("1".to_string()),
            sequence: 10,
        };
        assert_eq!(entry, entry.clone());
        assert!(format!("{entry:?}").contains("SequenceEntry"));
        #[allow(clippy::redundant_clone)]
        let graph_clone = graph.clone();
        assert_eq!(graph.entries, graph_clone.entries);
        assert!(format!("{graph:?}").contains("SequenceGraph"));
        let default_graph = SequenceGraph::default();
        assert!(default_graph.entries.is_empty());
    }

    #[test]
    fn test_ice11_nested_installer() {
        let mut db = make_test_db();
        db.add_record(
            "CustomAction",
            Record::with_fields(vec![
                FieldValue::String("NestedMsi".to_string()),
                FieldValue::Short(7), // Nested type 7
                FieldValue::String("SubPackage".to_string()),
                FieldValue::String("TRANSFORMS=1.mst".to_string()),
            ]),
        );
        db.add_record(
            "InstallExecuteSequence",
            Record::with_fields(vec![
                FieldValue::String("InstallInitialize".to_string()),
                FieldValue::Null,
                FieldValue::Short(1500),
            ]),
        );
        db.add_record(
            "InstallExecuteSequence",
            Record::with_fields(vec![
                FieldValue::String("NestedMsi".to_string()),
                FieldValue::Null,
                FieldValue::Short(2000),
            ]),
        );
        db.add_record(
            "InstallExecuteSequence",
            Record::with_fields(vec![
                FieldValue::String("InstallFinalize".to_string()),
                FieldValue::Null,
                FieldValue::Short(6600),
            ]),
        );

        assert!(validate_ice11(&db).is_some());

        // Good cases: outside transaction, non-type-7 CA, non-string records
        let mut db_good = make_test_db();
        db_good.add_record(
            "CustomAction",
            Record::with_fields(vec![
                FieldValue::String("NestedMsi".to_string()),
                FieldValue::Short(7),
                FieldValue::String("SubPackage".to_string()),
                FieldValue::String("TRANSFORMS=1.mst".to_string()),
            ]),
        );
        db_good.add_record(
            "CustomAction",
            Record::with_fields(vec![
                FieldValue::String("RegularCA".to_string()),
                FieldValue::Short(1),
                FieldValue::String("Bin".to_string()),
                FieldValue::String("Func".to_string()),
            ]),
        );
        db_good.add_record(
            "CustomAction",
            Record::with_fields(vec![FieldValue::Short(1)]),
        );
        db_good.add_record(
            "InstallExecuteSequence",
            Record::with_fields(vec![
                FieldValue::String("NestedMsi".to_string()),
                FieldValue::Null,
                FieldValue::Short(1000),
            ]),
        );
        db_good.add_record(
            "InstallExecuteSequence",
            Record::with_fields(vec![
                FieldValue::String("InstallInitialize".to_string()),
                FieldValue::Null,
                FieldValue::Short(1500),
            ]),
        );
        db_good.add_record(
            "InstallExecuteSequence",
            Record::with_fields(vec![
                FieldValue::String("RegularCA".to_string()),
                FieldValue::Null,
                FieldValue::Short(2000),
            ]),
        );
        db_good.add_record(
            "InstallExecuteSequence",
            Record::with_fields(vec![
                FieldValue::String("InstallFinalize".to_string()),
                FieldValue::Null,
                FieldValue::Short(6600),
            ]),
        );
        assert!(validate_ice11(&db_good).is_none());
    }

    #[test]
    #[allow(clippy::too_many_lines)]
    fn test_ice12_custom_action_targets() {
        let mut db = make_test_db();
        db.add_record(
            "CustomAction",
            Record::with_fields(vec![
                FieldValue::String("RunExe".to_string()),
                FieldValue::Short(17), // File type 17
                FieldValue::String("missing.exe".to_string()),
                FieldValue::String(String::new()),
            ]),
        );
        db.add_record(
            "File",
            Record::with_fields(vec![FieldValue::String("existing.exe".to_string())]),
        );

        assert!(validate_ice12(&db).is_some());

        // Missing Property target for type 18 and 22
        let mut db_prop = make_test_db();
        db_prop.add_record(
            "CustomAction",
            Record::with_fields(vec![
                FieldValue::String("SetProp".to_string()),
                FieldValue::Short(18),
                FieldValue::String("MISSING_PROP".to_string()),
                FieldValue::String("val".to_string()),
            ]),
        );
        db_prop.add_record(
            "Property",
            Record::with_fields(vec![FieldValue::String("EXISTING_PROP".to_string())]),
        );
        assert!(validate_ice12(&db_prop).is_some());

        let mut db_prop22 = make_test_db();
        db_prop22.add_record(
            "CustomAction",
            Record::with_fields(vec![
                FieldValue::String("SetProp22".to_string()),
                FieldValue::Short(22),
                FieldValue::String("MISSING_PROP".to_string()),
                FieldValue::String("val".to_string()),
            ]),
        );
        db_prop22.add_record(
            "Property",
            Record::with_fields(vec![FieldValue::String("EXISTING_PROP".to_string())]),
        );
        assert!(validate_ice12(&db_prop22).is_some());

        let mut db_file21 = make_test_db();
        db_file21.add_record(
            "CustomAction",
            Record::with_fields(vec![
                FieldValue::String("RunFile21".to_string()),
                FieldValue::Short(21),
                FieldValue::String("missing.exe".to_string()),
                FieldValue::String(String::new()),
            ]),
        );
        db_file21.add_record(
            "File",
            Record::with_fields(vec![FieldValue::String("existing.exe".to_string())]),
        );
        assert!(validate_ice12(&db_file21).is_some());

        // Good cases: files and properties match, other CA types, empty tables, non-string records
        let mut db_good = make_test_db();
        db_good.add_record(
            "CustomAction",
            Record::with_fields(vec![
                FieldValue::String("RunExe".to_string()),
                FieldValue::Short(17),
                FieldValue::String("existing.exe".to_string()),
                FieldValue::String(String::new()),
            ]),
        );
        db_good.add_record(
            "CustomAction",
            Record::with_fields(vec![
                FieldValue::String("SetProp".to_string()),
                FieldValue::Short(18),
                FieldValue::String("EXISTING_PROP".to_string()),
                FieldValue::String("val".to_string()),
            ]),
        );
        db_good.add_record(
            "CustomAction",
            Record::with_fields(vec![
                FieldValue::String("DllAction".to_string()),
                FieldValue::Short(1),
                FieldValue::String("my.dll".to_string()),
                FieldValue::String("Entry".to_string()),
            ]),
        );
        db_good.add_record(
            "File",
            Record::with_fields(vec![FieldValue::String("existing.exe".to_string())]),
        );
        db_good.add_record("File", Record::with_fields(vec![FieldValue::Short(1)]));
        db_good.add_record(
            "Property",
            Record::with_fields(vec![FieldValue::String("EXISTING_PROP".to_string())]),
        );
        db_good.add_record("Property", Record::with_fields(vec![FieldValue::Short(1)]));
        assert!(validate_ice12(&db_good).is_none());

        let mut db_empty = make_test_db();
        db_empty.add_record(
            "CustomAction",
            Record::with_fields(vec![
                FieldValue::String("RunExe".to_string()),
                FieldValue::Short(17),
                FieldValue::String("any.exe".to_string()),
                FieldValue::String(String::new()),
            ]),
        );
        db_empty.add_record(
            "CustomAction",
            Record::with_fields(vec![
                FieldValue::String("SetProp".to_string()),
                FieldValue::Short(18),
                FieldValue::String("PROP".to_string()),
                FieldValue::String("val".to_string()),
            ]),
        );
        assert!(validate_ice12(&db_empty).is_none());
    }

    #[test]
    fn test_ice26_elevation_requirements() {
        let mut db = make_test_db();
        db.add_record(
            "InstallExecuteSequence",
            Record::with_fields(vec![
                FieldValue::String("InstallFiles".to_string()),
                FieldValue::Null,
                FieldValue::Short(1000), // premature
            ]),
        );
        db.add_record(
            "InstallExecuteSequence",
            Record::with_fields(vec![
                FieldValue::String("InstallInitialize".to_string()),
                FieldValue::Null,
                FieldValue::Short(1500),
            ]),
        );
        db.add_record(
            "InstallExecuteSequence",
            Record::with_fields(vec![
                FieldValue::String("InstallFinalize".to_string()),
                FieldValue::Null,
                FieldValue::Short(6600),
            ]),
        );
        assert!(validate_ice26(&db).is_some());

        // Sequenced >= InstallFinalize
        let mut db_late = make_test_db();
        db_late.add_record(
            "InstallExecuteSequence",
            Record::with_fields(vec![
                FieldValue::String("InstallInitialize".to_string()),
                FieldValue::Null,
                FieldValue::Short(1500),
            ]),
        );
        db_late.add_record(
            "InstallExecuteSequence",
            Record::with_fields(vec![
                FieldValue::String("InstallFinalize".to_string()),
                FieldValue::Null,
                FieldValue::Short(6600),
            ]),
        );
        db_late.add_record(
            "InstallExecuteSequence",
            Record::with_fields(vec![
                FieldValue::String("WriteRegistryValues".to_string()),
                FieldValue::Null,
                FieldValue::Short(7000),
            ]),
        );
        assert!(validate_ice26(&db_late).is_some());

        // Good case: between init and fin
        let mut db_good = make_test_db();
        db_good.add_record(
            "InstallExecuteSequence",
            Record::with_fields(vec![
                FieldValue::String("InstallInitialize".to_string()),
                FieldValue::Null,
                FieldValue::Short(1500),
            ]),
        );
        db_good.add_record(
            "InstallExecuteSequence",
            Record::with_fields(vec![
                FieldValue::String("InstallFiles".to_string()),
                FieldValue::Null,
                FieldValue::Short(2000),
            ]),
        );
        db_good.add_record(
            "InstallExecuteSequence",
            Record::with_fields(vec![
                FieldValue::String("InstallFinalize".to_string()),
                FieldValue::Null,
                FieldValue::Short(6600),
            ]),
        );
        assert!(validate_ice26(&db_good).is_none());

        // Missing InstallInitialize or InstallFinalize
        let mut db_no_init = make_test_db();
        db_no_init.add_record(
            "InstallExecuteSequence",
            Record::with_fields(vec![
                FieldValue::String("InstallFiles".to_string()),
                FieldValue::Null,
                FieldValue::Short(2000),
            ]),
        );
        assert!(validate_ice26(&db_no_init).is_none());
    }

    #[test]
    #[allow(clippy::too_many_lines)]
    fn test_ice27_sequence_continuity() {
        let mut db = make_test_db();
        db.add_record(
            "InstallUISequence",
            Record::with_fields(vec![
                FieldValue::String("FileCost".to_string()),
                FieldValue::Null,
                FieldValue::Short(800),
            ]),
        );
        db.add_record(
            "InstallUISequence",
            Record::with_fields(vec![
                FieldValue::String("CostInitialize".to_string()),
                FieldValue::Null,
                FieldValue::Short(900),
            ]),
        );
        assert!(validate_ice27(&db).is_some());

        // UI sequence: FileCost >= CostFinalize
        let mut db_ui_fin = make_test_db();
        db_ui_fin.add_record(
            "InstallUISequence",
            Record::with_fields(vec![
                FieldValue::String("CostInitialize".to_string()),
                FieldValue::Null,
                FieldValue::Short(800),
            ]),
        );
        db_ui_fin.add_record(
            "InstallUISequence",
            Record::with_fields(vec![
                FieldValue::String("FileCost".to_string()),
                FieldValue::Null,
                FieldValue::Short(1000),
            ]),
        );
        db_ui_fin.add_record(
            "InstallUISequence",
            Record::with_fields(vec![
                FieldValue::String("CostFinalize".to_string()),
                FieldValue::Null,
                FieldValue::Short(900),
            ]),
        );
        assert!(validate_ice27(&db_ui_fin).is_some());

        // Execute sequence: CostInitialize >= FileCost
        let mut db_ex_init = make_test_db();
        db_ex_init.add_record(
            "InstallExecuteSequence",
            Record::with_fields(vec![
                FieldValue::String("CostInitialize".to_string()),
                FieldValue::Null,
                FieldValue::Short(1000),
            ]),
        );
        db_ex_init.add_record(
            "InstallExecuteSequence",
            Record::with_fields(vec![
                FieldValue::String("FileCost".to_string()),
                FieldValue::Null,
                FieldValue::Short(900),
            ]),
        );
        assert!(validate_ice27(&db_ex_init).is_some());

        // Execute sequence: FileCost >= CostFinalize
        let mut db_ex_fin = make_test_db();
        db_ex_fin.add_record(
            "InstallExecuteSequence",
            Record::with_fields(vec![
                FieldValue::String("CostInitialize".to_string()),
                FieldValue::Null,
                FieldValue::Short(800),
            ]),
        );
        db_ex_fin.add_record(
            "InstallExecuteSequence",
            Record::with_fields(vec![
                FieldValue::String("FileCost".to_string()),
                FieldValue::Null,
                FieldValue::Short(1000),
            ]),
        );
        db_ex_fin.add_record(
            "InstallExecuteSequence",
            Record::with_fields(vec![
                FieldValue::String("CostFinalize".to_string()),
                FieldValue::Null,
                FieldValue::Short(900),
            ]),
        );
        assert!(validate_ice27(&db_ex_fin).is_some());

        // Good cases
        let mut db_good = make_test_db();
        db_good.add_record(
            "InstallUISequence",
            Record::with_fields(vec![
                FieldValue::String("CostInitialize".to_string()),
                FieldValue::Null,
                FieldValue::Short(800),
            ]),
        );
        db_good.add_record(
            "InstallUISequence",
            Record::with_fields(vec![
                FieldValue::String("FileCost".to_string()),
                FieldValue::Null,
                FieldValue::Short(900),
            ]),
        );
        db_good.add_record(
            "InstallUISequence",
            Record::with_fields(vec![
                FieldValue::String("CostFinalize".to_string()),
                FieldValue::Null,
                FieldValue::Short(1000),
            ]),
        );
        db_good.add_record(
            "InstallExecuteSequence",
            Record::with_fields(vec![
                FieldValue::String("CostInitialize".to_string()),
                FieldValue::Null,
                FieldValue::Short(800),
            ]),
        );
        db_good.add_record(
            "InstallExecuteSequence",
            Record::with_fields(vec![
                FieldValue::String("FileCost".to_string()),
                FieldValue::Null,
                FieldValue::Short(900),
            ]),
        );
        db_good.add_record(
            "InstallExecuteSequence",
            Record::with_fields(vec![
                FieldValue::String("CostFinalize".to_string()),
                FieldValue::Null,
                FieldValue::Short(1000),
            ]),
        );
        assert!(validate_ice27(&db_good).is_none());
    }

    #[test]
    fn test_ice28_force_reboot() {
        let mut db = make_test_db();
        db.add_record(
            "InstallExecuteSequence",
            Record::with_fields(vec![
                FieldValue::String("ForceReboot".to_string()),
                FieldValue::Null, // Missing condition
                FieldValue::Short(2000),
            ]),
        );
        assert!(validate_ice28(&db).is_some());

        // Conditioned but outside transaction boundaries
        let mut db_outside = make_test_db();
        db_outside.add_record(
            "InstallExecuteSequence",
            Record::with_fields(vec![
                FieldValue::String("ForceReboot".to_string()),
                FieldValue::String("REBOOT_NEEDED".to_string()),
                FieldValue::Short(1000),
            ]),
        );
        db_outside.add_record(
            "InstallExecuteSequence",
            Record::with_fields(vec![
                FieldValue::String("InstallInitialize".to_string()),
                FieldValue::Null,
                FieldValue::Short(1500),
            ]),
        );
        db_outside.add_record(
            "InstallExecuteSequence",
            Record::with_fields(vec![
                FieldValue::String("InstallFinalize".to_string()),
                FieldValue::Null,
                FieldValue::Short(6600),
            ]),
        );
        assert!(validate_ice28(&db_outside).is_some());

        // Good case
        let mut db_good = make_test_db();
        db_good.add_record(
            "InstallExecuteSequence",
            Record::with_fields(vec![
                FieldValue::String("InstallInitialize".to_string()),
                FieldValue::Null,
                FieldValue::Short(1500),
            ]),
        );
        db_good.add_record(
            "InstallExecuteSequence",
            Record::with_fields(vec![
                FieldValue::String("ForceReboot".to_string()),
                FieldValue::String("REBOOT_NEEDED".to_string()),
                FieldValue::Short(2000),
            ]),
        );
        db_good.add_record(
            "InstallExecuteSequence",
            Record::with_fields(vec![
                FieldValue::String("InstallFinalize".to_string()),
                FieldValue::Null,
                FieldValue::Short(6600),
            ]),
        );
        assert!(validate_ice28(&db_good).is_none());
    }

    #[test]
    fn test_ice42_deferred_property_access() {
        let mut db = make_test_db();
        db.add_record(
            "CustomAction",
            Record::with_fields(vec![
                FieldValue::String("BadDeferred".to_string()),
                FieldValue::Short(0x0406), // Deferred VBScript
                FieldValue::String(String::new()),
                FieldValue::String(r#"val = Session.Property("MY_PROP")"#.to_string()),
            ]),
        );
        assert!(validate_ice42(&db).is_some());

        // Good cases: customactiondata in deferred, non-deferred CA with Session.Property, target without Session.Property
        let mut db_good = make_test_db();
        db_good.add_record(
            "CustomAction",
            Record::with_fields(vec![
                FieldValue::String("GoodDeferred".to_string()),
                FieldValue::Short(0x0406),
                FieldValue::String(String::new()),
                FieldValue::String(r#"val = Session.Property("CustomActionData")"#.to_string()),
            ]),
        );
        db_good.add_record(
            "CustomAction",
            Record::with_fields(vec![
                FieldValue::String("ImmediateCA".to_string()),
                FieldValue::Short(0x0006),
                FieldValue::String(String::new()),
                FieldValue::String(r#"val = Session.Property("MY_PROP")"#.to_string()),
            ]),
        );
        db_good.add_record(
            "CustomAction",
            Record::with_fields(vec![
                FieldValue::String("DllCA".to_string()),
                FieldValue::Short(0x0401),
                FieldValue::String("MyDll".to_string()),
                FieldValue::String("MyEntryPoint".to_string()),
            ]),
        );
        assert!(validate_ice42(&db_good).is_none());
    }

    #[test]
    fn test_ice63_duplicate_conditions() {
        let mut db = make_test_db();
        db.add_record(
            "InstallExecuteSequence",
            Record::with_fields(vec![
                FieldValue::String("MyAction".to_string()),
                FieldValue::String("VersionNT > 500".to_string()),
                FieldValue::Short(1000),
            ]),
        );
        db.add_record(
            "InstallExecuteSequence",
            Record::with_fields(vec![
                FieldValue::String("MyAction".to_string()),
                FieldValue::String("VersionNT > 500".to_string()), // duplicate condition
                FieldValue::Short(2000),
            ]),
        );
        assert!(validate_ice63(&db).is_some());

        // Duplicate action where one has no condition
        let mut db_no_cond = make_test_db();
        db_no_cond.add_record(
            "InstallExecuteSequence",
            Record::with_fields(vec![
                FieldValue::String("MyAction".to_string()),
                FieldValue::String("VersionNT > 500".to_string()),
                FieldValue::Short(1000),
            ]),
        );
        db_no_cond.add_record(
            "InstallExecuteSequence",
            Record::with_fields(vec![
                FieldValue::String("MyAction".to_string()),
                FieldValue::Null,
                FieldValue::Short(2000),
            ]),
        );
        assert!(validate_ice63(&db_no_cond).is_some());

        // Good cases: unique actions, or duplicate actions with distinct conditions
        let mut db_good = make_test_db();
        db_good.add_record(
            "InstallExecuteSequence",
            Record::with_fields(vec![
                FieldValue::String("Act1".to_string()),
                FieldValue::Null,
                FieldValue::Short(1000),
            ]),
        );
        db_good.add_record(
            "InstallExecuteSequence",
            Record::with_fields(vec![
                FieldValue::String("Act2".to_string()),
                FieldValue::String("VersionNT > 500".to_string()),
                FieldValue::Short(2000),
            ]),
        );
        db_good.add_record(
            "InstallExecuteSequence",
            Record::with_fields(vec![
                FieldValue::String("Act2".to_string()),
                FieldValue::String("VersionNT <= 500".to_string()),
                FieldValue::Short(3000),
            ]),
        );
        assert!(validate_ice63(&db_good).is_none());
    }

    #[test]
    fn test_ice67_premature_action() {
        let mut db = make_test_db();
        db.add_record(
            "CustomAction",
            Record::with_fields(vec![
                FieldValue::String("PreAction".to_string()),
                FieldValue::Short(1),
                FieldValue::String("Bin".to_string()),
                FieldValue::String("Func".to_string()),
            ]),
        );
        db.add_record(
            "InstallExecuteSequence",
            Record::with_fields(vec![
                FieldValue::String("PreAction".to_string()),
                FieldValue::Null,
                FieldValue::Short(500),
            ]),
        );
        db.add_record(
            "InstallExecuteSequence",
            Record::with_fields(vec![
                FieldValue::String("CostInitialize".to_string()),
                FieldValue::Null,
                FieldValue::Short(800),
            ]),
        );
        assert!(validate_ice67(&db).is_some());

        // Good cases: CA after CostInitialize, sequence <= 0, no CostInitialize
        let mut db_good = make_test_db();
        db_good.add_record(
            "CustomAction",
            Record::with_fields(vec![
                FieldValue::String("PostAction".to_string()),
                FieldValue::Short(1),
                FieldValue::String("Bin".to_string()),
                FieldValue::String("Func".to_string()),
            ]),
        );
        db_good.add_record(
            "CustomAction",
            Record::with_fields(vec![
                FieldValue::String("DisabledAction".to_string()),
                FieldValue::Short(1),
                FieldValue::String("Bin".to_string()),
                FieldValue::String("Func".to_string()),
            ]),
        );
        db_good.add_record(
            "InstallExecuteSequence",
            Record::with_fields(vec![
                FieldValue::String("CostInitialize".to_string()),
                FieldValue::Null,
                FieldValue::Short(800),
            ]),
        );
        db_good.add_record(
            "InstallExecuteSequence",
            Record::with_fields(vec![
                FieldValue::String("PostAction".to_string()),
                FieldValue::Null,
                FieldValue::Short(900),
            ]),
        );
        db_good.add_record(
            "InstallExecuteSequence",
            Record::with_fields(vec![
                FieldValue::String("DisabledAction".to_string()),
                FieldValue::Null,
                FieldValue::Short(-1),
            ]),
        );
        assert!(validate_ice67(&db_good).is_none());

        let mut db_no_cost = make_test_db();
        db_no_cost.add_record(
            "InstallExecuteSequence",
            Record::with_fields(vec![
                FieldValue::String("SomeAction".to_string()),
                FieldValue::Null,
                FieldValue::Short(500),
            ]),
        );
        assert!(validate_ice67(&db_no_cost).is_none());
    }

    #[test]
    fn test_ice68_custom_action_flags() {
        let mut db = make_test_db();
        db.add_record(
            "CustomAction",
            Record::with_fields(vec![
                FieldValue::String("ConflictingCA".to_string()),
                FieldValue::Short(0x0100 | 0x0200), // Both rollback and commit
                FieldValue::String(String::new()),
                FieldValue::String(String::new()),
            ]),
        );
        assert!(validate_ice68(&db).is_some());

        // Rollback without deferred (0x400)
        let mut db_no_def = make_test_db();
        db_no_def.add_record(
            "CustomAction",
            Record::with_fields(vec![
                FieldValue::String("RollbackNoDef".to_string()),
                FieldValue::Short(0x0100),
                FieldValue::String(String::new()),
                FieldValue::String(String::new()),
            ]),
        );
        assert!(validate_ice68(&db_no_def).is_some());

        // Good cases: deferred with rollback, deferred with commit, standard immediate
        let mut db_good = make_test_db();
        db_good.add_record(
            "CustomAction",
            Record::with_fields(vec![
                FieldValue::String("GoodRollback".to_string()),
                FieldValue::Short(0x0100 | 0x0400),
                FieldValue::String(String::new()),
                FieldValue::String(String::new()),
            ]),
        );
        db_good.add_record(
            "CustomAction",
            Record::with_fields(vec![
                FieldValue::String("GoodCommit".to_string()),
                FieldValue::Short(0x0200 | 0x0400),
                FieldValue::String(String::new()),
                FieldValue::String(String::new()),
            ]),
        );
        db_good.add_record(
            "CustomAction",
            Record::with_fields(vec![
                FieldValue::String("GoodImmediate".to_string()),
                FieldValue::Short(1),
                FieldValue::String(String::new()),
                FieldValue::String(String::new()),
            ]),
        );
        assert!(validate_ice68(&db_good).is_none());
    }

    #[test]
    fn test_ice72_source_consistency() {
        let mut db = make_test_db();
        db.add_record(
            "CustomAction",
            Record::with_fields(vec![
                FieldValue::String("CallDll".to_string()),
                FieldValue::Short(1), // Binary DLL
                FieldValue::String("missing_binary".to_string()),
                FieldValue::String("Func".to_string()),
            ]),
        );
        db.add_record(
            "Binary",
            Record::with_fields(vec![FieldValue::String("other_bin".to_string())]),
        );
        assert!(validate_ice72(&db).is_some());

        // Good cases: binary exists, empty binaries table, non-type-1 CA, non-string records
        let mut db_good = make_test_db();
        db_good.add_record(
            "CustomAction",
            Record::with_fields(vec![
                FieldValue::String("CallDll".to_string()),
                FieldValue::Short(1),
                FieldValue::String("my_bin".to_string()),
                FieldValue::String("Func".to_string()),
            ]),
        );
        db_good.add_record(
            "CustomAction",
            Record::with_fields(vec![
                FieldValue::String("ExeCA".to_string()),
                FieldValue::Short(2),
                FieldValue::String("my_exe".to_string()),
                FieldValue::String(String::new()),
            ]),
        );
        db_good.add_record(
            "Binary",
            Record::with_fields(vec![FieldValue::String("my_bin".to_string())]),
        );
        db_good.add_record("Binary", Record::with_fields(vec![FieldValue::Short(1)]));
        assert!(validate_ice72(&db_good).is_none());

        let mut db_empty = make_test_db();
        db_empty.add_record(
            "CustomAction",
            Record::with_fields(vec![
                FieldValue::String("CallDll".to_string()),
                FieldValue::Short(1),
                FieldValue::String("missing_bin".to_string()),
                FieldValue::String("Func".to_string()),
            ]),
        );
        assert!(validate_ice72(&db_empty).is_none());
    }

    #[test]
    #[allow(clippy::too_many_lines)]
    fn test_ice75_modifying_immediate_action() {
        let mut db = make_test_db();
        db.add_record(
            "CustomAction",
            Record::with_fields(vec![
                FieldValue::String("CreateState".to_string()),
                FieldValue::Short(1), // Immediate (not deferred 0x400)
                FieldValue::String("Bin".to_string()),
                FieldValue::String("CreateFilesNow".to_string()),
            ]),
        );
        db.add_record(
            "InstallExecuteSequence",
            Record::with_fields(vec![
                FieldValue::String("CreateState".to_string()),
                FieldValue::Null,
                FieldValue::Short(2000),
            ]),
        );
        assert!(validate_ice75(&db).is_some());

        // Variations: target with "delete" or "install"
        let mut db_del = make_test_db();
        db_del.add_record(
            "CustomAction",
            Record::with_fields(vec![
                FieldValue::String("DeleteState".to_string()),
                FieldValue::Short(1),
                FieldValue::String("Bin".to_string()),
                FieldValue::String("DeleteRegistryKey".to_string()),
            ]),
        );
        db_del.add_record(
            "InstallExecuteSequence",
            Record::with_fields(vec![
                FieldValue::String("DeleteState".to_string()),
                FieldValue::Null,
                FieldValue::Short(2000),
            ]),
        );
        assert!(validate_ice75(&db_del).is_some());

        let mut db_inst = make_test_db();
        db_inst.add_record(
            "CustomAction",
            Record::with_fields(vec![
                FieldValue::String("InstallState".to_string()),
                FieldValue::Short(1),
                FieldValue::String("Bin".to_string()),
                FieldValue::String("InstallCert".to_string()),
            ]),
        );
        db_inst.add_record(
            "InstallExecuteSequence",
            Record::with_fields(vec![
                FieldValue::String("InstallState".to_string()),
                FieldValue::Null,
                FieldValue::Short(2000),
            ]),
        );
        assert!(validate_ice75(&db_inst).is_some());

        // Good cases: marked deferred (0x400), not modifying, not sequenced in InstallExecuteSequence
        let mut db_good = make_test_db();
        db_good.add_record(
            "CustomAction",
            Record::with_fields(vec![
                FieldValue::String("DeferredMod".to_string()),
                FieldValue::Short(0x0401),
                FieldValue::String("Bin".to_string()),
                FieldValue::String("CreateFilesNow".to_string()),
            ]),
        );
        db_good.add_record(
            "CustomAction",
            Record::with_fields(vec![
                FieldValue::String("ReadAction".to_string()),
                FieldValue::Short(1),
                FieldValue::String("Bin".to_string()),
                FieldValue::String("QuerySystemStatus".to_string()),
            ]),
        );
        db_good.add_record(
            "CustomAction",
            Record::with_fields(vec![
                FieldValue::String("UnsequencedMod".to_string()),
                FieldValue::Short(1),
                FieldValue::String("Bin".to_string()),
                FieldValue::String("DeleteFiles".to_string()),
            ]),
        );
        db_good.add_record(
            "InstallExecuteSequence",
            Record::with_fields(vec![
                FieldValue::String("DeferredMod".to_string()),
                FieldValue::Null,
                FieldValue::Short(2000),
            ]),
        );
        db_good.add_record(
            "InstallExecuteSequence",
            Record::with_fields(vec![
                FieldValue::String("ReadAction".to_string()),
                FieldValue::Null,
                FieldValue::Short(2100),
            ]),
        );
        assert!(validate_ice75(&db_good).is_none());
    }

    #[test]
    fn test_ice77_deferred_placement() {
        let mut db = make_test_db();
        db.add_record(
            "CustomAction",
            Record::with_fields(vec![
                FieldValue::String("DeferredAction".to_string()),
                FieldValue::Short(0x0401), // Deferred
                FieldValue::String("Bin".to_string()),
                FieldValue::String("Func".to_string()),
            ]),
        );
        db.add_record(
            "InstallExecuteSequence",
            Record::with_fields(vec![
                FieldValue::String("DeferredAction".to_string()),
                FieldValue::Null,
                FieldValue::Short(1000), // outside InstallInitialize
            ]),
        );
        db.add_record(
            "InstallExecuteSequence",
            Record::with_fields(vec![
                FieldValue::String("InstallInitialize".to_string()),
                FieldValue::Null,
                FieldValue::Short(1500),
            ]),
        );
        db.add_record(
            "InstallExecuteSequence",
            Record::with_fields(vec![
                FieldValue::String("InstallFinalize".to_string()),
                FieldValue::Null,
                FieldValue::Short(6600),
            ]),
        );
        assert!(validate_ice77(&db).is_some());

        // Good cases: deferred between init and fin, unsequenced deferred, immediate CA
        let mut db_good = make_test_db();
        db_good.add_record(
            "CustomAction",
            Record::with_fields(vec![
                FieldValue::String("DeferredAction".to_string()),
                FieldValue::Short(0x0401),
                FieldValue::String("Bin".to_string()),
                FieldValue::String("Func".to_string()),
            ]),
        );
        db_good.add_record(
            "CustomAction",
            Record::with_fields(vec![
                FieldValue::String("ImmediateAction".to_string()),
                FieldValue::Short(1),
                FieldValue::String("Bin".to_string()),
                FieldValue::String("Func".to_string()),
            ]),
        );
        db_good.add_record(
            "CustomAction",
            Record::with_fields(vec![
                FieldValue::String("UnsequencedDeferred".to_string()),
                FieldValue::Short(0x0401),
                FieldValue::String("Bin".to_string()),
                FieldValue::String("Func".to_string()),
            ]),
        );
        db_good.add_record(
            "InstallExecuteSequence",
            Record::with_fields(vec![
                FieldValue::String("ImmediateAction".to_string()),
                FieldValue::Null,
                FieldValue::Short(1000),
            ]),
        );
        db_good.add_record(
            "InstallExecuteSequence",
            Record::with_fields(vec![
                FieldValue::String("InstallInitialize".to_string()),
                FieldValue::Null,
                FieldValue::Short(1500),
            ]),
        );
        db_good.add_record(
            "InstallExecuteSequence",
            Record::with_fields(vec![
                FieldValue::String("DeferredAction".to_string()),
                FieldValue::Null,
                FieldValue::Short(2000),
            ]),
        );
        db_good.add_record(
            "InstallExecuteSequence",
            Record::with_fields(vec![
                FieldValue::String("InstallFinalize".to_string()),
                FieldValue::Null,
                FieldValue::Short(6600),
            ]),
        );
        assert!(validate_ice77(&db_good).is_none());
    }

    #[test]
    fn test_ice78_advt_disallowed_actions() {
        let mut db = make_test_db();
        db.add_record(
            "AdvtExecuteSequence",
            Record::with_fields(vec![
                FieldValue::String("InstallFiles".to_string()),
                FieldValue::Null,
                FieldValue::Short(1000),
            ]),
        );
        assert!(validate_ice78(&db).is_some());

        // Good cases: allowed actions and non-string records
        let mut db_good = make_test_db();
        db_good.add_record(
            "AdvtExecuteSequence",
            Record::with_fields(vec![
                FieldValue::String("CostInitialize".to_string()),
                FieldValue::Null,
                FieldValue::Short(800),
            ]),
        );
        db_good.add_record(
            "AdvtExecuteSequence",
            Record::with_fields(vec![
                FieldValue::String("RegisterProduct".to_string()),
                FieldValue::Null,
                FieldValue::Short(1200),
            ]),
        );
        db_good.add_record(
            "AdvtExecuteSequence",
            Record::with_fields(vec![FieldValue::Short(1)]),
        );
        assert!(validate_ice78(&db_good).is_none());
    }
}

//! Symbol graph resolution, component group flattening, standard directory resolution, and action sequencing for the `WiX` Linker.

use super::{
    Linker, SequenceConstraint, STANDARD_DIRECTORIES, STANDARD_INSTALL_EXECUTE_ACTIONS,
    STANDARD_INSTALL_UI_ACTIONS,
};
use crate::database::memory::LinkedDatabase;
use crate::database::tables::record::{FieldValue, Record};
use crate::error::{MsiError, Result};
use crate::wix::wixobj::{IntermediateSection, Reference, SectionType, Symbol};
use std::collections::{HashMap, HashSet, VecDeque};

/// Checks whether an unresolved symbol reference is a standard built-in symbol, action,
/// dialog, property, or UI set.
pub(super) fn is_special_reference(
    rf: &Reference,
    defined_symbols: &HashMap<Symbol, Vec<usize>>,
) -> bool {
    match rf.namespace.as_str() {
        "Directory" => STANDARD_DIRECTORIES.iter().any(|d| d.id == rf.id),
        "Action" => {
            STANDARD_INSTALL_EXECUTE_ACTIONS
                .iter()
                .any(|a| a.name == rf.id)
                || STANDARD_INSTALL_UI_ACTIONS.iter().any(|a| a.name == rf.id)
                || defined_symbols.contains_key(&Symbol::new("Dialog", &rf.id))
                || defined_symbols.contains_key(&Symbol::new("CustomAction", &rf.id))
        }
        "UI" => rf.id.starts_with("WixUI_") || rf.id == "WixUI",
        "Property" => {
            rf.id.starts_with("WIXUI_") || rf.id.starts_with("ARP") || rf.id == "ALLUSERS"
        }
        _ => false,
    }
}

/// Solves relative sequencing directives (`Before`, `After`, `OnExit`) across execution and UI sequences.
pub(super) fn solve_relative_sequences(db: &mut LinkedDatabase) -> Result<()> {
    let rel_records = db.tables.remove("_WixSequenceRelative").unwrap_or_default();
    if rel_records.is_empty() {
        return Ok(());
    }

    let constraints_by_table = group_sequence_constraints(&rel_records);

    for (table_name, constraints) in constraints_by_table {
        let existing_records = db.tables.entry(table_name.clone()).or_default();
        apply_onexit_constraints(existing_records, &constraints);

        let mut action_seqs = build_initial_action_sequences(&table_name, existing_records);
        validate_sequence_dag(&table_name, &constraints)?;
        assign_relative_sequence_numbers(existing_records, &mut action_seqs, &constraints);

        existing_records.sort_by(|a, b| {
            let seq_a = match a.get(2) {
                Some(FieldValue::Short(s)) => *s,
                _ => 0,
            };
            let seq_b = match b.get(2) {
                Some(FieldValue::Short(s)) => *s,
                _ => 0,
            };
            seq_a.cmp(&seq_b)
        });
    }

    Ok(())
}

/// Groups `_WixSequenceRelative` records by sequence table name.
fn group_sequence_constraints(rel_records: &[Record]) -> HashMap<String, Vec<SequenceConstraint>> {
    let mut constraints_by_table: HashMap<String, Vec<SequenceConstraint>> = HashMap::new();
    for r in rel_records {
        if let (
            Some(FieldValue::String(tbl)),
            Some(FieldValue::String(action)),
            Some(FieldValue::String(anchor)),
            Some(FieldValue::String(pos)),
        ) = (r.get(0), r.get(1), r.get(2), r.get(3))
        {
            let maybe_cond = match r.get(4) {
                Some(FieldValue::String(c)) => Some(c.clone()),
                _ => None,
            };
            constraints_by_table.entry(tbl.clone()).or_default().push((
                action.clone(),
                anchor.clone(),
                pos.clone(),
                maybe_cond,
            ));
        }
    }
    constraints_by_table
}

/// Applies `OnExit` sequence numbers (`-1`, `-2`, `-3`, `-4`) to matching sequence records.
fn apply_onexit_constraints(existing_records: &mut [Record], constraints: &[SequenceConstraint]) {
    for (action, anchor, pos, maybe_cond) in constraints {
        if pos == "OnExit" {
            let on_exit_seq: i16 = match anchor.as_str() {
                "cancel" => -2,
                "error" => -3,
                "suspend" => -4,
                _ => -1,
            };
            for rec in existing_records.iter_mut() {
                let matches_cond = maybe_cond
                    .as_ref()
                    .is_none_or(|c| rec.get(1) == Some(&FieldValue::String(c.clone())));
                if rec.get(0) == Some(&FieldValue::String(action.clone())) && matches_cond {
                    rec.set(2, FieldValue::Short(on_exit_seq));
                }
            }
        }
    }
}

/// Builds the initial map of action names to sequence numbers.
fn build_initial_action_sequences(
    table_name: &str,
    existing_records: &[Record],
) -> HashMap<String, i16> {
    let mut action_seqs: HashMap<String, i16> = HashMap::new();
    for rec in existing_records {
        if let (Some(FieldValue::String(act)), Some(FieldValue::Short(s))) =
            (rec.get(0), rec.get(2))
        {
            action_seqs.insert(act.clone(), *s);
        }
    }

    if table_name == "InstallUISequence" {
        for std in STANDARD_INSTALL_UI_ACTIONS {
            action_seqs
                .entry(std.name.to_string())
                .or_insert(std.sequence);
        }
    } else if table_name == "InstallExecuteSequence" {
        for std in STANDARD_INSTALL_EXECUTE_ACTIONS {
            action_seqs
                .entry(std.name.to_string())
                .or_insert(std.sequence);
        }
    }
    action_seqs
}

/// Validates that `Before`/`After` constraints form a DAG without cycles.
fn validate_sequence_dag(table_name: &str, constraints: &[SequenceConstraint]) -> Result<()> {
    let mut in_degree: HashMap<String, usize> = HashMap::new();
    let mut graph: HashMap<String, Vec<String>> = HashMap::new();
    for (action, anchor, pos, _) in constraints {
        if pos == "After" {
            graph
                .entry(anchor.clone())
                .or_default()
                .push(action.clone());
            *in_degree.entry(action.clone()).or_insert(0) += 1;
            in_degree.entry(anchor.clone()).or_insert(0);
        } else if pos == "Before" {
            graph
                .entry(action.clone())
                .or_default()
                .push(anchor.clone());
            *in_degree.entry(anchor.clone()).or_insert(0) += 1;
            in_degree.entry(action.clone()).or_insert(0);
        }
    }

    let mut queue: VecDeque<String> = VecDeque::new();
    for (node, &deg) in &in_degree {
        if deg == 0 {
            queue.push_back(node.clone());
        }
    }

    let mut sorted_count = 0;
    while let Some(node) = queue.pop_front() {
        sorted_count += 1;
        if let Some(neighbors) = graph.get(&node) {
            for n in neighbors {
                let d = in_degree.entry(n.clone()).or_default();
                *d -= 1;
                if *d == 0 {
                    queue.push_back(n.clone());
                }
            }
        }
    }

    if sorted_count < in_degree.len() {
        return Err(MsiError::WixLinker {
            message: format!("cycle detected in relative sequencing constraints for {table_name}"),
        });
    }
    Ok(())
}

/// Iteratively assigns concrete sequence numbers for `After` and `Before` constraints.
fn assign_relative_sequence_numbers(
    existing_records: &mut [Record],
    action_seqs: &mut HashMap<String, i16>,
    constraints: &[SequenceConstraint],
) {
    let mut remaining: Vec<SequenceConstraint> = constraints
        .iter()
        .filter(|(_, _, pos, _)| pos == "After" || pos == "Before")
        .cloned()
        .collect();

    for pass in 0..=remaining.len() {
        if remaining.is_empty() {
            break;
        }
        let mut deferred: Vec<SequenceConstraint> = Vec::new();
        for (action, anchor, pos, maybe_cond) in remaining {
            let maybe_base = action_seqs.get(&anchor).copied();
            let base_seq = if let Some(b) = maybe_base {
                b
            } else if pass == 0 {
                deferred.push((action, anchor, pos, maybe_cond));
                continue;
            } else {
                1000
            };
            let assigned = if pos == "After" {
                let step = if base_seq >= 6550 { 1 } else { 25 };
                let mut cand = base_seq + step;
                while action_seqs.values().any(|&v| v == cand) {
                    cand += 1;
                }
                cand
            } else {
                let mut cand = (base_seq - 25).max(1);
                while action_seqs.values().any(|&v| v == cand) {
                    cand = (cand - 1).max(1);
                }
                cand
            };
            action_seqs.insert(action.clone(), assigned);
            for rec in existing_records.iter_mut() {
                let matches_cond = maybe_cond
                    .as_ref()
                    .is_none_or(|c| rec.get(1) == Some(&FieldValue::String(c.clone())));
                if rec.get(0) == Some(&FieldValue::String(action.clone())) && matches_cond {
                    rec.set(2, FieldValue::Short(assigned));
                }
            }
        }
        remaining = deferred;
    }
}

/// Solves the symbol graph: identifies the single entry point (`Product` or `Module`),
/// detects duplicate symbols, and resolves all required references across sections.
pub(super) fn solve_symbol_graph(linker: &Linker) -> Result<Vec<IntermediateSection>> {
    let mut all_sections: Vec<&IntermediateSection> = Vec::new();
    for obj in &linker.objects {
        for sec in &obj.sections {
            all_sections.push(sec);
        }
    }

    let entry_sections: Vec<&&IntermediateSection> = all_sections
        .iter()
        .filter(|s| s.section_type == SectionType::Product || s.section_type == SectionType::Module)
        .collect();

    if entry_sections.is_empty() {
        return Err(MsiError::WixLinker {
            message: "no entry section (Product or Module) found in input objects".to_string(),
        });
    }
    if entry_sections.len() > 1 {
        return Err(MsiError::WixLinker {
            message: format!(
                "multiple entry sections found: expected 1, found {}",
                entry_sections.len()
            ),
        });
    }

    let defined_symbols = collect_defined_symbols(&all_sections)?;
    let feature_extenders = collect_feature_extenders(&all_sections);

    let mut included_section_indices: HashSet<usize> = HashSet::new();
    let mut queue: Vec<usize> = Vec::new();

    let entry_idx = all_sections
        .iter()
        .position(|s| {
            s.section_type == SectionType::Product || s.section_type == SectionType::Module
        })
        .unwrap_or(0);

    included_section_indices.insert(entry_idx);
    queue.push(entry_idx);

    while let Some(current_idx) = queue.pop() {
        let current_sec = all_sections[current_idx];

        for sym in &current_sec.symbols {
            if sym.namespace == "Feature" {
                if let Some(extender_indices) = feature_extenders.get(&sym.id) {
                    for &ext_idx in extender_indices {
                        if included_section_indices.insert(ext_idx) {
                            queue.push(ext_idx);
                        }
                    }
                }
            }
        }

        for rf in &current_sec.references {
            let target_sym = Symbol::new(&rf.namespace, &rf.id);
            let mut target_sec_indices = defined_symbols.get(&target_sym).cloned();
            if target_sec_indices.is_none() && rf.namespace == "Action" {
                target_sec_indices = defined_symbols
                    .get(&Symbol::new("CustomAction", &rf.id))
                    .or_else(|| defined_symbols.get(&Symbol::new("Dialog", &rf.id)))
                    .cloned();
            }
            if let Some(target_indices) = target_sec_indices {
                for target_idx in target_indices {
                    if included_section_indices.insert(target_idx) {
                        queue.push(target_idx);
                    }
                }
            } else if !is_special_reference(rf, &defined_symbols) {
                let loc_info = rf.span.map_or_else(String::new, |span| {
                    format!(" at line {}, column {}", span.line, span.column)
                });
                return Err(MsiError::WixLinker {
                    message: format!(
                        "unresolved symbol reference '{rf}' in section {:?}{loc_info}",
                        current_sec.id
                    ),
                });
            }
        }
    }

    let mut result = Vec::with_capacity(included_section_indices.len());
    for (i, sec) in all_sections.into_iter().enumerate() {
        if included_section_indices.contains(&i) {
            result.push(sec.clone());
        }
    }

    Ok(result)
}

/// Collects all defined symbols across sections and detects illegal duplicates.
fn collect_defined_symbols(
    all_sections: &[&IntermediateSection],
) -> Result<HashMap<Symbol, Vec<usize>>> {
    let mut defined_symbols: HashMap<Symbol, Vec<usize>> = HashMap::new();
    for (sec_idx, sec) in all_sections.iter().enumerate() {
        for sym in &sec.symbols {
            if let Some(existing_indices) = defined_symbols.get_mut(sym) {
                if existing_indices.contains(&sec_idx)
                    || sym.namespace == "Property"
                    || sym.namespace == "ComponentGroup"
                    || sym.namespace == "Directory"
                {
                    if !existing_indices.contains(&sec_idx) {
                        existing_indices.push(sec_idx);
                    }
                    continue;
                }
                let first_idx = existing_indices[0];
                return Err(MsiError::WixLinker {
                    message: format!(
                        "duplicate symbol definition '{sym}' across sections {first_idx} and {sec_idx}"
                    ),
                });
            }
            defined_symbols.insert(sym.clone(), vec![sec_idx]);
        }
    }
    Ok(defined_symbols)
}

/// Collects sections that extend features via `FeatureRef`.
fn collect_feature_extenders(all_sections: &[&IntermediateSection]) -> HashMap<String, Vec<usize>> {
    let mut feature_extenders: HashMap<String, Vec<usize>> = HashMap::new();
    for (sec_idx, sec) in all_sections.iter().enumerate() {
        for rf in &sec.references {
            if rf.namespace == "Feature" {
                let has_extension = sec.tables.iter().any(|t| {
                    (t.name == "FeatureComponents" || t.name == "_FeatureComponentGroupRef")
                        && t.records
                            .iter()
                            .any(|r| r.get(0) == Some(&FieldValue::String(rf.id.clone())))
                });
                if has_extension {
                    feature_extenders
                        .entry(rf.id.clone())
                        .or_default()
                        .push(sec_idx);
                }
            }
        }
    }
    feature_extenders
}

/// Resolves multi-level nested `ComponentGroupRef` hierarchies and links components to features.
pub(super) fn resolve_component_groups(db: &mut LinkedDatabase) -> Result<()> {
    let members = db.get_records("_ComponentGroupMember").to_vec();
    let nested = db.get_records("_ComponentGroupNested").to_vec();
    let feat_refs = db.get_records("_FeatureComponentGroupRef").to_vec();

    if feat_refs.is_empty() && members.is_empty() {
        let _ = db.tables.remove("_ComponentGroupMember");
        let _ = db.tables.remove("_ComponentGroupNested");
        let _ = db.tables.remove("_FeatureComponentGroupRef");
        return Ok(());
    }

    let mut group_to_components: HashMap<String, HashSet<String>> = HashMap::new();
    for rec in &members {
        if let (Some(FieldValue::String(grp)), Some(FieldValue::String(comp))) =
            (rec.get(0), rec.get(1))
        {
            group_to_components
                .entry(grp.clone())
                .or_default()
                .insert(comp.clone());
        }
    }

    let mut group_to_children: HashMap<String, HashSet<String>> = HashMap::new();
    for rec in &nested {
        if let (Some(FieldValue::String(parent)), Some(FieldValue::String(child))) =
            (rec.get(0), rec.get(1))
        {
            group_to_children
                .entry(parent.clone())
                .or_default()
                .insert(child.clone());
        }
    }

    for rec in &feat_refs {
        if let (Some(FieldValue::String(feat)), Some(FieldValue::String(grp))) =
            (rec.get(0), rec.get(1))
        {
            let mut visited = HashSet::new();
            let comps = collect_transitive_components(
                grp,
                &group_to_components,
                &group_to_children,
                &mut visited,
            );
            for comp_id in comps {
                let fc = crate::database::tables::core::FeatureComponentsRow {
                    feature: crate::database::tables::types::FeatureName::new(feat)?,
                    component: crate::database::tables::types::ComponentName::new(&comp_id)?,
                };
                db.add_record("FeatureComponents", fc.to_record());
            }
        }
    }

    let _ = db.tables.remove("_ComponentGroupMember");
    let _ = db.tables.remove("_ComponentGroupNested");
    let _ = db.tables.remove("_FeatureComponentGroupRef");
    Ok(())
}

/// Recursively collects all transitive component IDs from a component group graph.
pub(super) fn collect_transitive_components(
    group: &str,
    group_to_components: &HashMap<String, HashSet<String>>,
    group_to_children: &HashMap<String, HashSet<String>>,
    visited: &mut HashSet<String>,
) -> HashSet<String> {
    if !visited.insert(group.to_string()) {
        return HashSet::new();
    }
    let mut result = group_to_components.get(group).cloned().unwrap_or_default();
    if let Some(children) = group_to_children.get(group) {
        for child in children {
            let child_comps = collect_transitive_components(
                child,
                group_to_components,
                group_to_children,
                visited,
            );
            result.extend(child_comps);
        }
    }
    result
}

/// Resolves standard directory hierarchy rooted at `TARGETDIR`.
pub(super) fn resolve_standard_directories(db: &mut LinkedDatabase) {
    let existing_dirs: HashSet<String> = db
        .get_records("Directory")
        .iter()
        .filter_map(|r| match r.get(0) {
            Some(FieldValue::String(s)) => Some(s.clone()),
            _ => None,
        })
        .collect();

    if !existing_dirs.contains("TARGETDIR") {
        let target_dir_rec = Record::with_fields(vec![
            FieldValue::String("TARGETDIR".to_string()),
            FieldValue::Null,
            FieldValue::String("SourceDir".to_string()),
        ]);
        db.add_record("Directory", target_dir_rec);
    }

    let mut needed_dirs: HashSet<String> = db
        .get_records("Component")
        .iter()
        .filter_map(|r| match r.get(2) {
            Some(FieldValue::String(d)) => Some(d.clone()),
            _ => None,
        })
        .collect();
    for r in db.get_records("Directory") {
        if let Some(FieldValue::String(parent_d)) = r.get(1) {
            if !parent_d.is_empty() {
                needed_dirs.insert(parent_d.clone());
            }
        }
    }

    for needed in needed_dirs {
        if !existing_dirs.contains(&needed) {
            if let Some(std_dir) = STANDARD_DIRECTORIES
                .iter()
                .find(|d| d.id == needed.as_str())
            {
                let parent = std_dir
                    .parent_id
                    .map_or(FieldValue::Null, |p| FieldValue::String(p.to_string()));
                let rec = Record::with_fields(vec![
                    FieldValue::String(std_dir.id.to_string()),
                    parent,
                    FieldValue::String(std_dir.default_dir.to_string()),
                ]);
                db.add_record("Directory", rec);
            }
        }
    }
}

/// Injects standard action sequences into `InstallExecuteSequence`.
pub(super) fn sequence_standard_actions(db: &mut LinkedDatabase) {
    let existing_actions: HashSet<String> = db
        .get_records("InstallExecuteSequence")
        .iter()
        .filter_map(|r| match r.get(0) {
            Some(FieldValue::String(s)) => Some(s.clone()),
            _ => None,
        })
        .collect();

    let has_services = !db.get_records("ServiceInstall").is_empty()
        || !db.get_records("ServiceControl").is_empty();
    let has_env = !db.get_records("Environment").is_empty();
    let has_create_folders = !db.get_records("CreateFolder").is_empty();
    let has_remove_folders = !db.get_records("RemoveFolder").is_empty();
    let has_remove_files = !db.get_records("RemoveFile").is_empty();
    let has_shortcuts = !db.get_records("Shortcut").is_empty();
    let has_move_files =
        !db.get_records("MoveFile").is_empty() || !db.get_records("DuplicateFile").is_empty();
    let has_ini =
        !db.get_records("IniFile").is_empty() || !db.get_records("RemoveIniFile").is_empty();

    for std_action in STANDARD_INSTALL_EXECUTE_ACTIONS {
        let should_include = match std_action.name {
            "StopServices" | "DeleteServices" | "InstallServices" | "StartServices" => has_services,
            "WriteEnvironmentStrings" => has_env,
            "CreateFolders" => has_create_folders,
            "RemoveFolders" => has_remove_folders,
            "RemoveFiles" => has_remove_files,
            "RemoveShortcuts" => has_shortcuts,
            "MoveFiles" => has_move_files,
            "WriteIniValues" => has_ini,
            _ => true,
        };

        if should_include && !existing_actions.contains(std_action.name) {
            let rec = Record::with_fields(vec![
                FieldValue::String(std_action.name.to_string()),
                std_action
                    .condition
                    .map_or(FieldValue::Null, |c| FieldValue::String(c.to_string())),
                FieldValue::Short(std_action.sequence),
            ]);
            db.add_record("InstallExecuteSequence", rec);
        }
    }

    if !db.get_records("InstallUISequence").is_empty() || !db.get_records("Dialog").is_empty() {
        let existing_ui_actions: HashSet<String> = db
            .get_records("InstallUISequence")
            .iter()
            .filter_map(|r| match r.get(0) {
                Some(FieldValue::String(s)) => Some(s.clone()),
                _ => None,
            })
            .collect();

        for std_ui_action in STANDARD_INSTALL_UI_ACTIONS {
            if !existing_ui_actions.contains(std_ui_action.name) {
                let rec = Record::with_fields(vec![
                    FieldValue::String(std_ui_action.name.to_string()),
                    FieldValue::Null,
                    FieldValue::Short(std_ui_action.sequence),
                ]);
                db.add_record("InstallUISequence", rec);
            }
        }
    }
}

/// Generates the standard `_Validation` table for all core MSI SDK tables.
pub(super) fn generate_validation_table(db: &mut LinkedDatabase) {
    let core_tables = [
        "ActionText",
        "AdminExecuteSequence",
        "AdminUISequence",
        "AdvtExecuteSequence",
        "Component",
        "CreateFolder",
        "CustomAction",
        "Directory",
        "Feature",
        "FeatureComponents",
        "File",
        "InstallExecuteSequence",
        "InstallUISequence",
        "LaunchCondition",
        "Media",
        "MsiFileHash",
        "Property",
        "Registry",
        "Upgrade",
    ];

    let mut validation_records = Vec::new();

    for tbl in &core_tables {
        if let Some(schema) = db.catalog.get_table(tbl) {
            for col in schema.columns() {
                let nullable = if col.nullable { "Y" } else { "N" };
                let rec = Record::with_fields(vec![
                    FieldValue::String((*tbl).to_string()),
                    FieldValue::String(col.name.clone()),
                    FieldValue::String(nullable.to_string()),
                    FieldValue::Null,
                    FieldValue::Null,
                    FieldValue::Null,
                    FieldValue::Null,
                    FieldValue::Null,
                    FieldValue::Null,
                    FieldValue::Null,
                ]);
                validation_records.push(rec);
            }
        }
    }

    for rec in validation_records {
        db.add_record("_Validation", rec);
    }
}

//! File, folder, shortcut, INI, and administrative image action preparation for MSI transactions.

use super::{parse_file_name, EvaluationContext, MediaDisk, RollbackOp, ScriptOp};
use crate::database::tables::record::FieldValue;
use crate::execution::script::{InstallScript, RollbackScript};
use crate::wix::linker::LinkedDatabase;
use std::collections::HashMap;
use std::path::PathBuf;

/// Context for preparing file, folder, shortcut, and INI actions.
pub(super) struct FilePrepContext<'a> {
    /// Linked MSI database.
    pub(super) database: &'a LinkedDatabase,
    /// Active evaluation context.
    pub(super) context: &'a EvaluationContext,
    /// Resolved directory paths.
    pub(super) resolved_dirs: &'a HashMap<String, PathBuf>,
    /// Component ID to directory ID mapping.
    pub(super) comp_to_dir: &'a HashMap<String, String>,
    /// Media table entries.
    pub(super) media_disks: &'a [MediaDisk],
    /// Generated installation script.
    pub(super) install_script: &'a mut InstallScript,
    /// Generated rollback script.
    pub(super) rollback_script: &'a mut RollbackScript,
}

/// Prepares file, folder, shortcut, INI, and admin package actions if `action` matches one of them.
///
/// Returns `true` if `action` was handled by this module.
pub(super) fn prepare_file_action(action: &str, ctx: &mut FilePrepContext<'_>) -> bool {
    match action {
        "CreateFolders" => {
            prepare_create_folders(ctx);
            true
        }
        "InstallFiles" => {
            prepare_install_files(ctx);
            true
        }
        "CreateShortcuts" => {
            prepare_create_shortcuts(ctx);
            true
        }
        "RemoveFiles" => {
            prepare_remove_files(ctx);
            true
        }
        "InstallAdminPackage" => {
            prepare_install_admin_package(ctx);
            true
        }
        "DuplicateFiles" => {
            prepare_duplicate_files(ctx);
            true
        }
        "RemoveDuplicateFiles" => {
            prepare_remove_duplicate_files(ctx);
            true
        }
        "MoveFiles" => {
            prepare_move_files(ctx);
            true
        }
        "PatchFiles" => {
            prepare_patch_files(ctx);
            true
        }
        "WriteIniValues" => {
            prepare_write_ini_values(ctx);
            true
        }
        "RemoveIniValues" => {
            prepare_remove_ini_values(ctx);
            true
        }
        "RemoveFolders" => {
            prepare_remove_folders(ctx);
            true
        }
        "RemoveShortcuts" => {
            prepare_remove_shortcuts(ctx);
            true
        }
        "IsolateComponents" => {
            prepare_isolate_components(ctx);
            true
        }
        _ => false,
    }
}

/// Prepares `CreateFolders` operations.
fn prepare_create_folders(ctx: &mut FilePrepContext<'_>) {
    for d in ctx.database.get_records("Directory") {
        if let Some(FieldValue::String(dir_name)) = d.get(0) {
            let path = ctx
                .resolved_dirs
                .get(dir_name)
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_default();
            ctx.install_script
                .push(ScriptOp::CreateFolder { path: path.clone() });
            ctx.rollback_script
                .push(RollbackOp::DeleteCreatedFolder { path });
        }
    }
}

/// Prepares `InstallFiles` operations.
fn prepare_install_files(ctx: &mut FilePrepContext<'_>) {
    for f in ctx.database.get_records("File") {
        let file_id = match f.get(0) {
            Some(FieldValue::String(id)) => id.clone(),
            _ => continue,
        };
        let comp_id = match f.get(1) {
            Some(FieldValue::String(c)) => c.clone(),
            _ => String::new(),
        };
        let filename_spec = match f.get(2) {
            Some(FieldValue::String(name)) => name.clone(),
            _ => continue,
        };
        let sequence = match f.get(7) {
            Some(&FieldValue::Short(s)) => i32::from(s),
            Some(&FieldValue::Long(l)) => l,
            _ => 1,
        };

        let filename = parse_file_name(&filename_spec);
        let target_path_buf = ctx
            .comp_to_dir
            .get(&comp_id)
            .and_then(|dir_id| ctx.resolved_dirs.get(dir_id))
            .map_or_else(
                || PathBuf::from(format!(r"C:\Program Files\App\{filename}")),
                |dir_path| dir_path.join(filename),
            );
        let target_path = target_path_buf.to_string_lossy().to_string();
        let quarantine_path = format!("{target_path}.rbf");

        ctx.install_script.push(ScriptOp::BackupFile {
            target_path: target_path.clone(),
            quarantine_path: quarantine_path.clone(),
        });
        ctx.rollback_script
            .push(RollbackOp::RestoreQuarantinedFile {
                target_path: target_path.clone(),
                quarantine_path,
            });

        let matched_cab = ctx
            .media_disks
            .iter()
            .find(|m| m.last_sequence >= sequence)
            .and_then(|m| m.cabinet.as_ref());

        if let Some(cab_name) = matched_cab {
            ctx.install_script.push(ScriptOp::ExtractCabinetFile {
                cabinet: cab_name.clone(),
                file_key: file_id,
                destination: target_path.clone(),
            });
        } else {
            ctx.install_script.push(ScriptOp::WriteFile {
                destination: target_path.clone(),
                content: format!("Payload of {filename}").into_bytes(),
            });
        }

        ctx.rollback_script
            .push(RollbackOp::DeleteCreatedFile { path: target_path });
    }
}

/// Prepares `CreateShortcuts` operations.
fn prepare_create_shortcuts(ctx: &mut FilePrepContext<'_>) {
    for s in ctx.database.get_records("Shortcut") {
        if let Some(FieldValue::String(target)) = s.get(4) {
            let link_path = match s.get(1) {
                Some(FieldValue::String(dir_id)) => {
                    let dir = ctx
                        .resolved_dirs
                        .get(dir_id)
                        .cloned()
                        .unwrap_or_else(|| PathBuf::from(r"C:\Users\Public\Desktop"));
                    dir.join(format!("{target}.lnk"))
                        .to_string_lossy()
                        .to_string()
                }
                _ => format!(r"C:\Users\Public\Desktop\{target}.lnk"),
            };
            ctx.install_script.push(ScriptOp::CreateShortcut {
                target: target.clone(),
                link_path: link_path.clone(),
                arguments: None,
                icon_path: None,
                icon_index: None,
            });
            ctx.rollback_script
                .push(RollbackOp::DeleteShortcut { link_path });
        }
    }
}

/// Prepares `RemoveFiles` operations.
fn prepare_remove_files(ctx: &mut FilePrepContext<'_>) {
    for rf in ctx.database.get_records("RemoveFile") {
        let dir_prop = match rf.get(3) {
            Some(FieldValue::String(d)) => d.as_str(),
            _ => continue,
        };
        let base_dir = ctx.resolved_dirs.get(dir_prop).cloned().unwrap_or_else(|| {
            PathBuf::from(ctx.context.get_property(dir_prop).unwrap_or(r"C:\App"))
        });
        let file_name = match rf.get(2) {
            Some(FieldValue::String(n)) if !n.is_empty() => n.clone(),
            _ => "*.*".to_string(),
        };
        let target_path = base_dir.join(&file_name).to_string_lossy().to_string();
        let quarantine_path = format!("{target_path}.rbf");

        ctx.install_script.push(ScriptOp::BackupFile {
            target_path: target_path.clone(),
            quarantine_path: quarantine_path.clone(),
        });
        ctx.install_script.push(ScriptOp::DeleteFile {
            path: target_path.clone(),
        });
        ctx.rollback_script
            .push(RollbackOp::RestoreQuarantinedFile {
                target_path,
                quarantine_path,
            });
    }
}

/// Prepares `InstallAdminPackage` operations.
fn prepare_install_admin_package(ctx: &mut FilePrepContext<'_>) {
    let target_dir = resolve_dir_or_default(
        ctx.resolved_dirs,
        Some(&FieldValue::String("TARGETDIR".to_string())),
        r"C:\Admin",
    )
    .to_string_lossy()
    .to_string();
    let admin_msi = format!(r"{target_dir}\admin.msi");
    ctx.install_script.push(ScriptOp::WriteFile {
        destination: admin_msi.clone(),
        content: b"ADMIN_PACKAGE".to_vec(),
    });
    ctx.rollback_script
        .push(RollbackOp::DeleteCreatedFile { path: admin_msi });
}

/// Resolves a folder column value against `resolved_dirs` with a default fallback.
fn resolve_dir_or_default(
    resolved_dirs: &HashMap<String, PathBuf>,
    field: Option<&FieldValue>,
    fallback: &str,
) -> PathBuf {
    field
        .and_then(|f| match f {
            FieldValue::String(d) => resolved_dirs.get(d).cloned(),
            _ => None,
        })
        .unwrap_or_else(|| PathBuf::from(fallback))
}

/// Prepares `DuplicateFiles` operations.
fn prepare_duplicate_files(ctx: &mut FilePrepContext<'_>) {
    for rec in ctx.database.get_records("DuplicateFile") {
        let Some(FieldValue::String(file_id)) = rec.get(2) else {
            continue;
        };
        let dest_name = match rec.get(3) {
            Some(FieldValue::String(n)) if !n.is_empty() => n.clone(),
            _ => format!("{file_id}.dup"),
        };
        let dest_dir =
            resolve_dir_or_default(ctx.resolved_dirs, rec.get(4), r"C:\Program Files\App");
        let dest_path = dest_dir.join(&dest_name).to_string_lossy().to_string();
        ctx.install_script.push(ScriptOp::CopyFile {
            source: file_id.clone(),
            destination: dest_path.clone(),
            overwrite: true,
        });
        ctx.rollback_script
            .push(RollbackOp::DeleteCreatedFile { path: dest_path });
    }
}

/// Prepares `RemoveDuplicateFiles` operations.
fn prepare_remove_duplicate_files(ctx: &mut FilePrepContext<'_>) {
    for rec in ctx.database.get_records("DuplicateFile") {
        let Some(FieldValue::String(file_id)) = rec.get(2) else {
            continue;
        };
        let dest_name = match rec.get(3) {
            Some(FieldValue::String(n)) if !n.is_empty() => n.clone(),
            _ => format!("{file_id}.dup"),
        };
        let dest_dir =
            resolve_dir_or_default(ctx.resolved_dirs, rec.get(4), r"C:\Program Files\App");
        let dest_path = dest_dir.join(&dest_name).to_string_lossy().to_string();
        let quarantine_path = format!("{dest_path}.rbf");
        ctx.install_script.push(ScriptOp::DeleteFile {
            path: dest_path.clone(),
        });
        ctx.rollback_script
            .push(RollbackOp::RestoreQuarantinedFile {
                target_path: dest_path,
                quarantine_path,
            });
    }
}

/// Prepares `MoveFiles` operations.
fn prepare_move_files(ctx: &mut FilePrepContext<'_>) {
    for rec in ctx.database.get_records("MoveFile") {
        let src_name = match rec.get(2) {
            Some(FieldValue::String(s)) if !s.is_empty() => s.clone(),
            _ => "source.dat".to_string(),
        };
        let dest_name = match rec.get(3) {
            Some(FieldValue::String(d)) if !d.is_empty() => d.clone(),
            _ => src_name.clone(),
        };
        let src_folder = resolve_dir_or_default(ctx.resolved_dirs, rec.get(4), r"C:\Temp");
        let dest_folder =
            resolve_dir_or_default(ctx.resolved_dirs, rec.get(5), r"C:\Program Files\App");
        let opts = match rec.get(6) {
            Some(&FieldValue::Short(o)) => o,
            _ => 0,
        };
        let src_path = src_folder.join(&src_name).to_string_lossy().to_string();
        let dest_path = dest_folder.join(&dest_name).to_string_lossy().to_string();
        ctx.install_script.push(ScriptOp::CopyFile {
            source: src_path.clone(),
            destination: dest_path.clone(),
            overwrite: true,
        });
        if (opts & 1) != 0 {
            ctx.install_script
                .push(ScriptOp::DeleteFile { path: src_path });
        }
        ctx.rollback_script
            .push(RollbackOp::DeleteCreatedFile { path: dest_path });
    }
}

/// Prepares `PatchFiles` operations.
fn prepare_patch_files(ctx: &mut FilePrepContext<'_>) {
    for rec in ctx.database.get_records("Patch") {
        if let Some(FieldValue::String(file_id)) = rec.get(0) {
            let dest_path = format!(r"C:\Program Files\App\{file_id}");
            let quarantine_path = format!("{dest_path}.rbf");
            ctx.install_script.push(ScriptOp::BackupFile {
                target_path: dest_path.clone(),
                quarantine_path: quarantine_path.clone(),
            });
            ctx.install_script.push(ScriptOp::WriteFile {
                destination: dest_path.clone(),
                content: format!("Patched {file_id}").into_bytes(),
            });
            ctx.rollback_script
                .push(RollbackOp::RestoreQuarantinedFile {
                    target_path: dest_path,
                    quarantine_path,
                });
        }
    }
}

/// Prepares `WriteIniValues` operations.
fn prepare_write_ini_values(ctx: &mut FilePrepContext<'_>) {
    for rec in ctx.database.get_records("IniFile") {
        let file_name = match rec.get(1) {
            Some(FieldValue::String(f)) => f.clone(),
            _ => continue,
        };
        let section = match rec.get(3) {
            Some(FieldValue::String(s)) => s.clone(),
            _ => String::new(),
        };
        let key = match rec.get(4) {
            Some(FieldValue::String(k)) => k.clone(),
            _ => String::new(),
        };
        let val = match rec.get(5) {
            Some(FieldValue::String(v)) => v.clone(),
            _ => String::new(),
        };
        let target = format!("{file_name}|{section}|{key}={val}");
        ctx.install_script.push(ScriptOp::CustomAction {
            action: "@@WriteIniValue".to_string(),
            action_type: 0,
            source: file_name,
            target: target.clone(),
        });
        ctx.rollback_script.push(RollbackOp::RollbackCustomAction {
            action: "@@RemoveIniValue".to_string(),
            action_type: 0,
            source: String::new(),
            target,
        });
    }
}

/// Prepares `RemoveIniValues` operations.
fn prepare_remove_ini_values(ctx: &mut FilePrepContext<'_>) {
    for rec in ctx.database.get_records("RemoveIniFile") {
        let file_name = match rec.get(1) {
            Some(FieldValue::String(f)) => f.clone(),
            _ => continue,
        };
        let section = match rec.get(3) {
            Some(FieldValue::String(s)) => s.clone(),
            _ => String::new(),
        };
        let key = match rec.get(4) {
            Some(FieldValue::String(k)) => k.clone(),
            _ => String::new(),
        };
        let target = format!("{file_name}|{section}|{key}");
        ctx.install_script.push(ScriptOp::CustomAction {
            action: "@@RemoveIniValue".to_string(),
            action_type: 0,
            source: file_name,
            target: target.clone(),
        });
        ctx.rollback_script.push(RollbackOp::RollbackCustomAction {
            action: "@@WriteIniValue".to_string(),
            action_type: 0,
            source: String::new(),
            target,
        });
    }
}

/// Prepares `RemoveFolders` operations.
fn prepare_remove_folders(ctx: &mut FilePrepContext<'_>) {
    for rec in ctx.database.get_records("CreateFolder") {
        if let Some(FieldValue::String(dir_id)) = rec.get(0) {
            let path = resolve_dir_or_default(
                ctx.resolved_dirs,
                Some(&FieldValue::String(dir_id.clone())),
                r"C:\",
            )
            .to_string_lossy()
            .to_string();
            ctx.install_script.push(ScriptOp::RemoveFolder { path });
        }
    }
}

/// Prepares `RemoveShortcuts` operations.
fn prepare_remove_shortcuts(ctx: &mut FilePrepContext<'_>) {
    for s in ctx.database.get_records("Shortcut") {
        if let Some(FieldValue::String(target)) = s.get(4) {
            let dir =
                resolve_dir_or_default(ctx.resolved_dirs, s.get(1), r"C:\Users\Public\Desktop");
            let link_path = dir
                .join(format!("{target}.lnk"))
                .to_string_lossy()
                .to_string();
            ctx.install_script
                .push(ScriptOp::DeleteShortcut { link_path });
        }
    }
}

/// Prepares `IsolateComponents` operations.
fn prepare_isolate_components(ctx: &mut FilePrepContext<'_>) {
    for rec in ctx.database.get_records("IsolatedComponent") {
        if let (Some(FieldValue::String(shared)), Some(FieldValue::String(app))) =
            (rec.get(0), rec.get(1))
        {
            let dest = format!(r"C:\Program Files\App\{app}.local");
            ctx.install_script.push(ScriptOp::WriteFile {
                destination: dest.clone(),
                content: shared.as_bytes().to_vec(),
            });
            ctx.rollback_script
                .push(RollbackOp::DeleteCreatedFile { path: dest });
        }
    }
}

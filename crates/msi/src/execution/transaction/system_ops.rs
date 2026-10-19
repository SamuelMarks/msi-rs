//! System services, ODBC, fonts, assemblies, `AppSearch`, upgrades, and custom action preparation.

use super::{EvaluationContext, RollbackOp, ScriptOp};
use crate::database::tables::record::FieldValue;
use crate::error::{MsiError, Result};
use crate::execution::script::{InstallScript, RollbackScript};
use crate::wix::linker::LinkedDatabase;
use std::collections::HashMap;
use std::path::Path;

/// Prepares system services, ODBC, fonts, assemblies, search, and custom actions.
pub(super) fn prepare_system_or_custom_action(
    action: &str,
    database: &LinkedDatabase,
    context: &mut EvaluationContext,
    comp_keypaths: &HashMap<String, String>,
    custom_actions_map: &HashMap<String, (u32, String, String)>,
    install_script: &mut InstallScript,
    rollback_script: &mut RollbackScript,
) -> Result<()> {
    if prepare_service_or_odbc_or_font_action(
        action,
        database,
        context,
        comp_keypaths,
        install_script,
        rollback_script,
    ) || prepare_search_or_upgrade_action(action, database, context, install_script)?
        || prepare_system_registration_or_control_action(
            action,
            database,
            context,
            install_script,
            rollback_script,
        )
    {
        return Ok(());
    }

    prepare_custom_action(
        action,
        database,
        context,
        custom_actions_map,
        install_script,
        rollback_script,
    )
}

/// Prepares service control/install, ODBC, and font registration actions.
fn prepare_service_or_odbc_or_font_action(
    action: &str,
    database: &LinkedDatabase,
    context: &mut EvaluationContext,
    comp_keypaths: &HashMap<String, String>,
    install_script: &mut InstallScript,
    rollback_script: &mut RollbackScript,
) -> bool {
    match action {
        "StopServices" | "DeleteServices" | "InstallServices" | "StartServices" => {
            prepare_service_action(
                action,
                database,
                comp_keypaths,
                install_script,
                rollback_script,
            );
            true
        }
        "InstallODBC" | "SetODBCFolders" | "RemoveODBC" => {
            prepare_odbc_action(action, database, context, install_script, rollback_script);
            true
        }
        "RegisterFonts" | "UnregisterFonts" => {
            prepare_font_action(action, database, context, install_script, rollback_script);
            true
        }
        _ => false,
    }
}

/// Prepares `StopServices`, `DeleteServices`, `InstallServices`, and `StartServices`.
fn prepare_service_action(
    action: &str,
    database: &LinkedDatabase,
    comp_keypaths: &HashMap<String, String>,
    install_script: &mut InstallScript,
    rollback_script: &mut RollbackScript,
) {
    match action {
        "StopServices" => {
            for sc in database.get_records("ServiceControl") {
                let event = match sc.get(2) {
                    Some(&FieldValue::Short(e)) => e,
                    _ => 0,
                };
                if (event & 0x0022) != 0 {
                    if let Some(FieldValue::String(name)) = sc.get(1) {
                        install_script.push(ScriptOp::StopService { name: name.clone() });
                    }
                }
            }
        }
        "DeleteServices" => {
            for sc in database.get_records("ServiceControl") {
                let event = match sc.get(2) {
                    Some(&FieldValue::Short(e)) => e,
                    _ => 0,
                };
                if (event & 0x0044) != 0 {
                    if let Some(FieldValue::String(name)) = sc.get(1) {
                        install_script.push(ScriptOp::DeleteService { name: name.clone() });
                    }
                }
            }
        }
        "InstallServices" => {
            for si in database.get_records("ServiceInstall") {
                let name = match si.get(1) {
                    Some(FieldValue::String(n)) => n.clone(),
                    _ => continue,
                };
                let display_name = match si.get(2) {
                    Some(FieldValue::String(d)) => d.clone(),
                    _ => name.clone(),
                };
                let service_type = match si.get(3) {
                    Some(&FieldValue::Long(t)) => u32::try_from(t).unwrap_or(0x10),
                    _ => 0x10,
                };
                let start_type = match si.get(4) {
                    Some(&FieldValue::Long(s)) => u32::try_from(s).unwrap_or(2),
                    _ => 2,
                };
                let comp = match si.get(10) {
                    Some(FieldValue::String(c)) => c.as_str(),
                    _ => "",
                };
                let binary_path = comp_keypaths
                    .get(comp)
                    .cloned()
                    .unwrap_or_else(|| format!(r"C:\Program Files\App\{name}.exe"));

                install_script.push(ScriptOp::InstallService {
                    name: name.clone(),
                    display_name,
                    service_type,
                    start_type,
                    binary_path,
                });
                rollback_script.push(RollbackOp::DeleteService { name });
            }
        }
        _ => {
            for sc in database.get_records("ServiceControl") {
                let event = match sc.get(2) {
                    Some(&FieldValue::Short(e)) => e,
                    _ => 0,
                };
                if (event & 0x0011) != 0 {
                    if let Some(FieldValue::String(name)) = sc.get(1) {
                        let args = match sc.get(3) {
                            Some(FieldValue::String(a)) => Some(a.clone()),
                            _ => None,
                        };
                        install_script.push(ScriptOp::StartService {
                            name: name.clone(),
                            arguments: args,
                        });
                        rollback_script.push(RollbackOp::StopService { name: name.clone() });
                    }
                }
            }
        }
    }
}

/// Prepares `InstallODBC`, `SetODBCFolders`, and `RemoveODBC`.
fn prepare_odbc_action(
    action: &str,
    database: &LinkedDatabase,
    context: &mut EvaluationContext,
    install_script: &mut InstallScript,
    rollback_script: &mut RollbackScript,
) {
    match action {
        "InstallODBC" => {
            for rec in database.get_records("ODBCDriver") {
                if let Some(FieldValue::String(driver_name)) = rec.get(1) {
                    install_script.push(ScriptOp::CustomAction {
                        action: "@@InstallODBCDriver".to_string(),
                        action_type: 0,
                        source: String::new(),
                        target: driver_name.clone(),
                    });
                    rollback_script.push(RollbackOp::RollbackCustomAction {
                        action: "@@UninstallODBCDriver".to_string(),
                        action_type: 0,
                        source: String::new(),
                        target: driver_name.clone(),
                    });
                }
            }
            for rec in database.get_records("ODBCDataSource") {
                if let Some(FieldValue::String(ds_name)) = rec.get(1) {
                    install_script.push(ScriptOp::CustomAction {
                        action: "@@InstallODBCDataSource".to_string(),
                        action_type: 0,
                        source: String::new(),
                        target: ds_name.clone(),
                    });
                    rollback_script.push(RollbackOp::RollbackCustomAction {
                        action: "@@UninstallODBCDataSource".to_string(),
                        action_type: 0,
                        source: String::new(),
                        target: ds_name.clone(),
                    });
                }
            }
        }
        "SetODBCFolders" => {
            context.set_property("ODBCFoldersInitialized", "1");
        }
        _ => {
            for rec in database.get_records("ODBCDriver") {
                if let Some(FieldValue::String(drv)) = rec.get(1) {
                    install_script.push(ScriptOp::CustomAction {
                        action: "@@UninstallODBCDriver".to_string(),
                        action_type: 0,
                        source: String::new(),
                        target: drv.clone(),
                    });
                }
            }
        }
    }
}

/// Prepares `RegisterFonts` and `UnregisterFonts`.
fn prepare_font_action(
    action: &str,
    database: &LinkedDatabase,
    context: &EvaluationContext,
    install_script: &mut InstallScript,
    rollback_script: &mut RollbackScript,
) {
    let (main_act, rb_act) = if action == "RegisterFonts" {
        ("@@RegisterFont", "@@UnregisterFont")
    } else {
        ("@@UnregisterFont", "@@RegisterFont")
    };
    for rec in database.get_records("Font") {
        if let (Some(FieldValue::String(file_id)), Some(FieldValue::String(font_title))) =
            (rec.get(0), rec.get(1))
        {
            let file_path = context.get_property(file_id).map_or_else(
                || format!(r"C:\Windows\Fonts\{file_id}"),
                ToString::to_string,
            );
            let target = format!("{file_path}|{font_title}");
            install_script.push(ScriptOp::CustomAction {
                action: main_act.to_string(),
                action_type: 0,
                source: String::new(),
                target: target.clone(),
            });
            rollback_script.push(RollbackOp::RollbackCustomAction {
                action: rb_act.to_string(),
                action_type: 0,
                source: String::new(),
                target,
            });
        }
    }
}

/// Prepares `AppSearch`, `CCPSearch`, `FindRelatedProducts`, `MigrateFeatureStates`, `RemoveExistingProducts`, `LaunchConditions`, and `ValidateProductID`.
fn prepare_search_or_upgrade_action(
    action: &str,
    database: &LinkedDatabase,
    context: &mut EvaluationContext,
    install_script: &mut InstallScript,
) -> Result<bool> {
    match action {
        "AppSearch" => {
            let host = crate::execution::appsearch::DefaultHostSystem::new();
            let search = crate::execution::appsearch::AppSearch::new(database, host);
            for (prop, val) in search.execute().unwrap_or_default() {
                context.set_property(prop, val);
            }
            Ok(true)
        }
        "CCPSearch" | "RMCCPSearch" => {
            let host = crate::execution::appsearch::DefaultHostSystem::new();
            let search = crate::execution::appsearch::AppSearch::new(database, host);
            let matched = database.get_records("CCPSearch").iter().any(|rec| {
                let sig = match rec.get(0) {
                    Some(FieldValue::String(s)) => s.as_str(),
                    _ => return false,
                };
                search.resolve_signature(sig).is_some() || context.get_property(sig).is_some()
            });
            if matched {
                context.set_property("CCP_SUCCESS", "1");
            }
            Ok(true)
        }
        "FindRelatedProducts" | "MigrateFeatureStates" | "RemoveExistingProducts" => {
            prepare_upgrade_action(action, database, context, install_script);
            Ok(true)
        }
        "LaunchConditions" => {
            for rec in database.get_records("LaunchCondition") {
                let Some(FieldValue::String(lc_expr)) = rec.get(0) else {
                    continue;
                };
                let desc = match rec.get(1) {
                    Some(FieldValue::String(d)) => d.as_str(),
                    _ => lc_expr.as_str(),
                };
                if !context.evaluate_condition(lc_expr)? {
                    return Err(MsiError::LaunchConditionFailed {
                        condition: lc_expr.clone(),
                        description: desc.to_string(),
                    });
                }
            }
            Ok(true)
        }
        "ValidateProductID" => {
            if let Some(pid) = context
                .get_property("PIDKEY")
                .filter(|s| !s.is_empty())
                .map(ToString::to_string)
            {
                context.set_property("ProductID", pid);
            }
            Ok(true)
        }
        _ => Ok(false),
    }
}

/// Prepares `FindRelatedProducts`, `MigrateFeatureStates`, and `RemoveExistingProducts`.
fn prepare_upgrade_action(
    action: &str,
    database: &LinkedDatabase,
    context: &mut EvaluationContext,
    install_script: &mut InstallScript,
) {
    match action {
        "FindRelatedProducts" => {
            for rec in database.get_records("Upgrade") {
                let Some(FieldValue::String(upg_code)) = rec.get(0) else {
                    continue;
                };
                let Some(FieldValue::String(action_prop)) = rec.get(6) else {
                    continue;
                };
                let found_prod = context
                    .get_property(upg_code)
                    .map_or_else(|| upg_code.clone(), ToString::to_string);
                context.set_property(action_prop, found_prod);
                context.set_property("Preselected", "1");
            }
        }
        "MigrateFeatureStates" => {
            for rec in database.get_records("Upgrade") {
                let attrs = match rec.get(4) {
                    Some(&FieldValue::Long(a)) => a,
                    Some(&FieldValue::Short(a)) => i32::from(a),
                    _ => 0,
                };
                if (attrs & 1) != 0 {
                    context.set_property("MigratedFeatureStates", "1");
                }
            }
        }
        _ => {
            for rec in database.get_records("Upgrade") {
                let attrs = match rec.get(4) {
                    Some(&FieldValue::Long(a)) => a,
                    Some(&FieldValue::Short(a)) => i32::from(a),
                    _ => 0,
                };
                if let Some((prop, prod)) =
                    rec.get(6)
                        .filter(|_| (attrs & 2) == 0)
                        .and_then(|f| match f {
                            FieldValue::String(p) => context
                                .get_property(p)
                                .map(|prod| (p.clone(), prod.to_string())),
                            _ => None,
                        })
                {
                    install_script.push(ScriptOp::CustomAction {
                        action: "@@RemoveExistingProduct".to_string(),
                        action_type: 0,
                        source: prop,
                        target: prod,
                    });
                }
            }
        }
    }
}

/// Prepares `SelfReg`, `ComPlus`, `MsiAssembly`, `BindImage`, `SFPCatalog`, and control actions.
fn prepare_system_registration_or_control_action(
    action: &str,
    database: &LinkedDatabase,
    context: &mut EvaluationContext,
    install_script: &mut InstallScript,
    rollback_script: &mut RollbackScript,
) -> bool {
    if prepare_control_or_reboot_action(action, context) {
        return true;
    }

    match action {
        "SelfRegModules" | "SelfUnregModules" => {
            let (main_act, main_fn, rb_act, rb_fn) = if action == "SelfRegModules" {
                (
                    "@@SelfRegModule",
                    "DllRegisterServer",
                    "@@SelfUnregModule",
                    "DllUnregisterServer",
                )
            } else {
                (
                    "@@SelfUnregModule",
                    "DllUnregisterServer",
                    "@@SelfRegModule",
                    "DllRegisterServer",
                )
            };
            for rec in database.get_records("SelfReg") {
                if let Some(FieldValue::String(file_id)) = rec.get(0) {
                    install_script.push(ScriptOp::CustomAction {
                        action: main_act.to_string(),
                        action_type: 0,
                        source: file_id.clone(),
                        target: main_fn.to_string(),
                    });
                    rollback_script.push(RollbackOp::RollbackCustomAction {
                        action: rb_act.to_string(),
                        action_type: 0,
                        source: file_id.clone(),
                        target: rb_fn.to_string(),
                    });
                }
            }
            true
        }
        "RegisterComPlus"
        | "UnregisterComPlus"
        | "MsiPublishAssemblies"
        | "MsiUnpublishAssemblies" => {
            let tbl = if action.contains("ComPlus") {
                "ComPlus"
            } else {
                "MsiAssembly"
            };
            for rec in database.get_records(tbl) {
                if let Some(FieldValue::String(comp)) = rec.get(0) {
                    install_script.push(ScriptOp::CustomAction {
                        action: format!("@@{action}"),
                        action_type: 0,
                        source: comp.clone(),
                        target: String::new(),
                    });
                }
            }
            true
        }
        "BindImage" => {
            for rec in database.get_records("BindImage") {
                if let Some(FieldValue::String(file_id)) = rec.get(0) {
                    let path_str = context
                        .get_property(file_id)
                        .unwrap_or(file_id.as_str())
                        .to_string();
                    let _ = crate::execution::bind_image::BindImageEngine
                        .bind_image(Path::new(&path_str), None);
                    install_script.push(ScriptOp::CustomAction {
                        action: "@@BindImage".to_string(),
                        action_type: 0,
                        source: file_id.clone(),
                        target: path_str,
                    });
                }
            }
            true
        }
        "InstallSFPCatalogFile" => {
            for rec in database.get_records("SFPCatalog") {
                if let Some(FieldValue::String(cat)) = rec.get(0) {
                    install_script.push(ScriptOp::CustomAction {
                        action: "@@InstallSFPCatalogFile".to_string(),
                        action_type: 0,
                        source: cat.clone(),
                        target: String::new(),
                    });
                }
            }
            true
        }
        _ => false,
    }
}

/// Prepares top-level sequence control, source resolution, rollback disablement, and reboot flags.
fn prepare_control_or_reboot_action(action: &str, context: &mut EvaluationContext) -> bool {
    match action {
        "INSTALL" | "ADMIN" | "ADVERTISE" | "ExecuteAction" => {
            context.set_property("ExecutedSequenceAction", action);
            true
        }
        "InstallExecute" | "InstallExecuteAgain" => {
            context.set_property("InstallExecuteFlushed", "1");
            true
        }
        "ResolveSource" => {
            if context.get_property("SourceDir").is_none() {
                let mut root = String::from("C:");
                root.push('\x5c');
                context.set_property("SourceDir", root);
            }
            true
        }
        "AllocateRegistrySpace" => {
            context.set_property("RegistrySpaceAllocated", "1");
            true
        }
        "DisableRollback" => {
            context.set_property("RollbackDisabled", "1");
            true
        }
        "ScheduleReboot" => {
            context.set_property("REBOOT", "Force");
            true
        }
        "ForceReboot" => {
            context.set_property("REBOOT", "Force");
            context.set_property("ForceRebootTriggered", "1");
            true
        }
        _ => false,
    }
}

/// Schedules or executes a custom action during `Transaction::prepare`.
fn prepare_custom_action(
    custom_act: &str,
    database: &LinkedDatabase,
    context: &mut EvaluationContext,
    custom_actions_map: &HashMap<String, (u32, String, String)>,
    install_script: &mut InstallScript,
    rollback_script: &mut RollbackScript,
) -> Result<()> {
    let (action_type, source, target) = if let Some(ca) = custom_actions_map.get(custom_act) {
        ca.clone()
    } else {
        install_script.push(ScriptOp::CustomAction {
            action: custom_act.to_string(),
            action_type: 1,
            source: "BinaryTable".to_string(),
            target: "EntryFn".to_string(),
        });
        rollback_script.push(RollbackOp::RollbackCustomAction {
            action: format!("{custom_act}_Rollback"),
            action_type: 0x501,
            source: "BinaryTable".to_string(),
            target: "RollbackFn".to_string(),
        });
        return Ok(());
    };

    if action_type & 0x003F == 19 {
        let error_msg = if target.is_empty() {
            format!("Installation aborted by custom action '{custom_act}'.")
        } else {
            context.format_string(&target)?
        };
        return Err(MsiError::CustomActionFailed {
            action: custom_act.to_string(),
            reason: error_msg,
        });
    }

    if (action_type & 0x0500) == 0x0500 {
        rollback_script.push(RollbackOp::RollbackCustomAction {
            action: custom_act.to_string(),
            action_type,
            source,
            target,
        });
    } else if action_type & 0x0400 == 0 {
        if let Ok(ca_def) = crate::execution::custom_action::CustomActionDefinition::parse(
            custom_act,
            action_type,
            &source,
            &target,
        ) {
            let mut ca_exec = crate::execution::custom_action::CustomActionExecutor::new();
            for b in database.get_records("Binary") {
                if let Some(FieldValue::String(b_name)) = b.get(0) {
                    let b_data = match b.get(1) {
                        Some(FieldValue::String(s)) => s.as_bytes().to_vec(),
                        _ => Vec::new(),
                    };
                    ca_exec.add_binary(b_name.clone(), b_data);
                }
            }
            ca_exec.execute(&ca_def, context)?;
        }

        install_script.push(ScriptOp::CustomAction {
            action: custom_act.to_string(),
            action_type,
            source,
            target,
        });
    } else {
        install_script.push(ScriptOp::CustomAction {
            action: custom_act.to_string(),
            action_type,
            source: source.clone(),
            target,
        });
        rollback_script.push(RollbackOp::RollbackCustomAction {
            action: format!("{custom_act}_Rollback"),
            action_type: 0x0501,
            source,
            target: "RollbackFn".to_string(),
        });
    }
    Ok(())
}

//! Registry, environment, COM class/ProgId/Extension/MIME/TypeLib, and product publishing action preparation.

use super::{EvaluationContext, RollbackOp, ScriptOp};
use crate::database::tables::record::FieldValue;
use crate::execution::script::{InstallScript, RollbackScript};
use crate::wix::linker::LinkedDatabase;

/// Prepares registry, environment, COM registration, and product publishing actions if `action` matches one of them.
///
/// Returns `true` if `action` was handled by this module.
pub(super) fn prepare_registry_action(
    action: &str,
    database: &LinkedDatabase,
    context: &EvaluationContext,
    install_script: &mut InstallScript,
    rollback_script: &mut RollbackScript,
) -> bool {
    match action {
        "WriteRegistryValues" => {
            prepare_write_registry_values(database, context, install_script, rollback_script);
            true
        }
        "RemoveRegistryValues" => {
            prepare_remove_registry_values(database, context, install_script, rollback_script);
            true
        }
        "WriteEnvironmentStrings" => {
            prepare_write_env_strings(database, context, install_script, rollback_script);
            true
        }
        "RemoveEnvironmentStrings" => {
            prepare_remove_env_strings(database, install_script, rollback_script);
            true
        }
        _ => prepare_com_or_publish_action(
            action,
            database,
            context,
            install_script,
            rollback_script,
        ),
    }
}

/// Prepares `WriteRegistryValues` operations.
fn prepare_write_registry_values(
    database: &LinkedDatabase,
    context: &EvaluationContext,
    install_script: &mut InstallScript,
    rollback_script: &mut RollbackScript,
) {
    let allusers = context.get_property("ALLUSERS").unwrap_or_default();
    let is_per_machine = allusers == "1" || allusers == "2";

    for r in database.get_records("Registry") {
        let root = match r.get(1) {
            Some(&FieldValue::Short(-1)) => {
                if is_per_machine {
                    2 // msidbRegistryRootLocalMachine
                } else {
                    1 // msidbRegistryRootCurrentUser
                }
            }
            Some(&FieldValue::Short(n)) => u32::from_ne_bytes(i32::from(n).to_ne_bytes()),
            _ => 2,
        };
        let key = match r.get(2) {
            Some(FieldValue::String(k)) => k.clone(),
            _ => r"Software\App".to_string(),
        };
        let name = match r.get(3) {
            Some(FieldValue::String(n)) => Some(n.clone()),
            _ => None,
        };
        let val = match r.get(4) {
            Some(FieldValue::String(v)) => Some(v.clone()),
            _ => None,
        };

        install_script.push(ScriptOp::WriteRegistry {
            root,
            key: key.clone(),
            name: name.clone(),
            value: val,
        });
        rollback_script.push(RollbackOp::RestoreRegistry {
            root,
            key,
            name,
            previous_value: None,
            existed: false,
        });
    }
}

/// Prepares `RemoveRegistryValues` operations.
fn prepare_remove_registry_values(
    database: &LinkedDatabase,
    context: &EvaluationContext,
    install_script: &mut InstallScript,
    rollback_script: &mut RollbackScript,
) {
    let allusers = context.get_property("ALLUSERS").unwrap_or_default();
    let is_per_machine = allusers == "1" || allusers == "2";

    for r in database.get_records("RemoveRegistry") {
        let root = match r.get(1) {
            Some(&FieldValue::Short(-1)) => {
                if is_per_machine {
                    2
                } else {
                    1
                }
            }
            Some(&FieldValue::Short(n)) => u32::from_ne_bytes(i32::from(n).to_ne_bytes()),
            _ => 2,
        };
        let key = match r.get(2) {
            Some(FieldValue::String(k)) => k.clone(),
            _ => continue,
        };
        let name = match r.get(3) {
            Some(FieldValue::String(n)) => Some(n.clone()),
            _ => None,
        };

        install_script.push(ScriptOp::DeleteRegistry {
            root,
            key: key.clone(),
            name: name.clone(),
        });
        rollback_script.push(RollbackOp::RestoreRegistry {
            root,
            key,
            name,
            previous_value: None,
            existed: true,
        });
    }
}

/// Prepares `WriteEnvironmentStrings` operations.
fn prepare_write_env_strings(
    database: &LinkedDatabase,
    context: &EvaluationContext,
    install_script: &mut InstallScript,
    rollback_script: &mut RollbackScript,
) {
    for env_rec in database.get_records("Environment") {
        let name_raw = match env_rec.get(1) {
            Some(FieldValue::String(n)) => n.clone(),
            _ => continue,
        };
        let val_raw = match env_rec.get(2) {
            Some(FieldValue::String(v)) => v.clone(),
            _ => String::new(),
        };
        let formatted_val = context.format_string(&val_raw).unwrap_or(val_raw);
        let clean_name = name_raw.trim_start_matches(['=', '+', '-', '!', '*']);

        let env_key = r"SYSTEM\CurrentControlSet\Control\Session Manager\Environment".to_string();
        install_script.push(ScriptOp::WriteRegistry {
            root: 2,
            key: env_key.clone(),
            name: Some(clean_name.to_string()),
            value: Some(formatted_val),
        });
        rollback_script.push(RollbackOp::RestoreRegistry {
            root: 2,
            key: env_key,
            name: Some(clean_name.to_string()),
            previous_value: None,
            existed: false,
        });
    }
}

/// Prepares `RemoveEnvironmentStrings` operations.
fn prepare_remove_env_strings(
    database: &LinkedDatabase,
    install_script: &mut InstallScript,
    rollback_script: &mut RollbackScript,
) {
    for env_rec in database.get_records("Environment") {
        let Some(FieldValue::String(name_raw)) = env_rec.get(1) else {
            continue;
        };
        let clean_name = name_raw
            .trim_start_matches(['=', '+', '-', '!', '*'])
            .to_string();
        let env_key = r"SYSTEM\CurrentControlSet\Control\Session Manager\Environment".to_string();
        install_script.push(ScriptOp::DeleteRegistry {
            root: 2,
            key: env_key.clone(),
            name: Some(clean_name.clone()),
        });
        rollback_script.push(RollbackOp::RestoreRegistry {
            root: 2,
            key: env_key,
            name: Some(clean_name),
            previous_value: None,
            existed: true,
        });
    }
}

/// Dispatches COM registration or product/feature/component publishing actions.
fn prepare_com_or_publish_action(
    action: &str,
    database: &LinkedDatabase,
    context: &EvaluationContext,
    install_script: &mut InstallScript,
    rollback_script: &mut RollbackScript,
) -> bool {
    match action {
        "RegisterProgIdInfo" => {
            prepare_register_progid(database, context, install_script);
            true
        }
        "RegisterExtensionInfo" => {
            prepare_register_extension(database, context, install_script);
            true
        }
        "RegisterMIMEInfo" => {
            prepare_register_mime(database, context, install_script);
            true
        }
        "RegisterClassInfo"
        | "UnregisterClassInfo"
        | "UnregisterExtensionInfo"
        | "UnregisterProgIdInfo"
        | "UnregisterMIMEInfo"
        | "RegisterTypeLibraries"
        | "UnregisterTypeLibraries" => {
            prepare_com_class_or_typelib(action, database, install_script, rollback_script);
            true
        }
        "PublishComponents"
        | "UnpublishComponents"
        | "PublishFeatures"
        | "UnpublishFeatures"
        | "PublishProduct"
        | "UnpublishProduct"
        | "RegisterProduct"
        | "RegisterUser" => {
            prepare_publishing_action(action, database, context, install_script, rollback_script);
            true
        }
        _ => false,
    }
}

/// Prepares `RegisterProgIdInfo` operations.
fn prepare_register_progid(
    database: &LinkedDatabase,
    context: &EvaluationContext,
    install_script: &mut InstallScript,
) {
    for r in database.get_records("ProgId") {
        let progid = match r.get(0) {
            Some(FieldValue::String(p)) => context.format_string(p).unwrap_or_default(),
            _ => continue,
        };
        let desc = match r.get(3) {
            Some(FieldValue::String(d)) => Some(context.format_string(d).unwrap_or_default()),
            _ => None,
        };
        let clsid = match r.get(2) {
            Some(FieldValue::String(c)) => Some(context.format_string(c).unwrap_or_default()),
            _ => None,
        };

        install_script.push(ScriptOp::WriteRegistry {
            root: 0,
            key: progid.clone(),
            name: None,
            value: desc,
        });

        if let Some(c) = clsid {
            install_script.push(ScriptOp::WriteRegistry {
                root: 0,
                key: format!(r"{progid}\CLSID"),
                name: None,
                value: Some(c),
            });
        }
    }
}

/// Prepares `RegisterExtensionInfo` operations.
fn prepare_register_extension(
    database: &LinkedDatabase,
    context: &EvaluationContext,
    install_script: &mut InstallScript,
) {
    let ext_records = database.get_records("Extension");
    let verb_records = database.get_records("Verb");

    for ext in ext_records {
        let ext_name = match ext.get(0) {
            Some(FieldValue::String(e)) => context.format_string(e).unwrap_or_default(),
            _ => continue,
        };
        let progid = match ext.get(2) {
            Some(FieldValue::String(p)) => Some(context.format_string(p).unwrap_or_default()),
            _ => None,
        };

        install_script.push(ScriptOp::WriteRegistry {
            root: 0,
            key: format!(".{ext_name}"),
            name: None,
            value: progid.clone(),
        });

        if let Some(pid) = progid {
            for verb_rec in verb_records
                .iter()
                .filter(|v| matches!(v.get(0), Some(FieldValue::String(e)) if e == &ext_name))
            {
                let verb = match verb_rec.get(1) {
                    Some(FieldValue::String(v)) => context.format_string(v).unwrap_or_default(),
                    _ => continue,
                };
                let command = match verb_rec.get(3) {
                    Some(FieldValue::String(c)) => context.format_string(c).unwrap_or_default(),
                    _ => String::new(),
                };
                let argument = match verb_rec.get(4) {
                    Some(FieldValue::String(a)) => context.format_string(a).unwrap_or_default(),
                    _ => String::new(),
                };

                let full_cmd = if argument.is_empty() {
                    command
                } else {
                    format!("{command} {argument}")
                };

                install_script.push(ScriptOp::WriteRegistry {
                    root: 0,
                    key: format!(r"{pid}\shell\{verb}\command"),
                    name: None,
                    value: Some(full_cmd),
                });
            }
        }
    }
}

/// Prepares `RegisterMIMEInfo` operations.
fn prepare_register_mime(
    database: &LinkedDatabase,
    context: &EvaluationContext,
    install_script: &mut InstallScript,
) {
    for mime in database.get_records("MIME") {
        let content_type = match mime.get(0) {
            Some(FieldValue::String(c)) => context.format_string(c).unwrap_or_default(),
            _ => continue,
        };
        let ext = match mime.get(1) {
            Some(FieldValue::String(e)) => context.format_string(e).unwrap_or_default(),
            _ => continue,
        };

        install_script.push(ScriptOp::WriteRegistry {
            root: 0,
            key: format!(r"MIME\Database\Content Type\{content_type}"),
            name: Some("Extension".to_string()),
            value: Some(format!(".{ext}")),
        });
    }
}

/// Prepares COM class, unregistration, and `TypeLib` actions.
fn prepare_com_class_or_typelib(
    action: &str,
    database: &LinkedDatabase,
    install_script: &mut InstallScript,
    rollback_script: &mut RollbackScript,
) {
    match action {
        "RegisterClassInfo" => {
            for rec in database.get_records("Class") {
                let Some(FieldValue::String(clsid)) = rec.get(0) else {
                    continue;
                };
                let ctx = match rec.get(1) {
                    Some(FieldValue::String(c)) => c.clone(),
                    _ => "InprocServer32".to_string(),
                };
                let key = format!(r"CLSID\{clsid}\{ctx}");
                install_script.push(ScriptOp::WriteRegistry {
                    root: 0,
                    key: key.clone(),
                    name: None,
                    value: Some(clsid.clone()),
                });
                rollback_script.push(RollbackOp::RestoreRegistry {
                    root: 0,
                    key,
                    name: None,
                    previous_value: None,
                    existed: false,
                });
            }
        }
        "UnregisterClassInfo" => {
            for rec in database.get_records("Class") {
                let Some(FieldValue::String(clsid)) = rec.get(0) else {
                    continue;
                };
                let key = format!(r"CLSID\{clsid}");
                install_script.push(ScriptOp::DeleteRegistry {
                    root: 0,
                    key: key.clone(),
                    name: None,
                });
                rollback_script.push(RollbackOp::RestoreRegistry {
                    root: 0,
                    key,
                    name: None,
                    previous_value: None,
                    existed: true,
                });
            }
        }
        "UnregisterExtensionInfo" | "UnregisterProgIdInfo" | "UnregisterMIMEInfo" => {
            prepare_unregister_com_metadata(action, database, install_script, rollback_script);
        }
        _ => {
            prepare_typelib_action(action, database, install_script, rollback_script);
        }
    }
}

/// Prepares `UnregisterExtensionInfo`, `UnregisterProgIdInfo`, and `UnregisterMIMEInfo`.
fn prepare_unregister_com_metadata(
    action: &str,
    database: &LinkedDatabase,
    install_script: &mut InstallScript,
    rollback_script: &mut RollbackScript,
) {
    match action {
        "UnregisterExtensionInfo" => {
            for ext in database.get_records("Extension") {
                if let Some(FieldValue::String(ext_name)) = ext.get(0) {
                    let key = format!(".{ext_name}");
                    install_script.push(ScriptOp::DeleteRegistry {
                        root: 0,
                        key: key.clone(),
                        name: None,
                    });
                    rollback_script.push(RollbackOp::RestoreRegistry {
                        root: 0,
                        key,
                        name: None,
                        previous_value: None,
                        existed: true,
                    });
                }
            }
        }
        "UnregisterProgIdInfo" => {
            for r in database.get_records("ProgId") {
                if let Some(FieldValue::String(progid)) = r.get(0) {
                    install_script.push(ScriptOp::DeleteRegistry {
                        root: 0,
                        key: progid.clone(),
                        name: None,
                    });
                    rollback_script.push(RollbackOp::RestoreRegistry {
                        root: 0,
                        key: progid.clone(),
                        name: None,
                        previous_value: None,
                        existed: true,
                    });
                }
            }
        }
        _ => {
            for mime in database.get_records("MIME") {
                if let Some(FieldValue::String(content_type)) = mime.get(0) {
                    let key = format!(r"MIME\Database\Content Type\{content_type}");
                    install_script.push(ScriptOp::DeleteRegistry {
                        root: 0,
                        key: key.clone(),
                        name: Some("Extension".to_string()),
                    });
                    rollback_script.push(RollbackOp::RestoreRegistry {
                        root: 0,
                        key,
                        name: Some("Extension".to_string()),
                        previous_value: None,
                        existed: true,
                    });
                }
            }
        }
    }
}

/// Prepares `RegisterTypeLibraries` and `UnregisterTypeLibraries`.
fn prepare_typelib_action(
    action: &str,
    database: &LinkedDatabase,
    install_script: &mut InstallScript,
    rollback_script: &mut RollbackScript,
) {
    let is_reg = action == "RegisterTypeLibraries";
    for rec in database.get_records("TypeLib") {
        if let Some(FieldValue::String(lib_id)) = rec.get(0) {
            let key = format!(r"TypeLib\{lib_id}");
            if is_reg {
                install_script.push(ScriptOp::WriteRegistry {
                    root: 0,
                    key: key.clone(),
                    name: None,
                    value: Some(lib_id.clone()),
                });
            } else {
                install_script.push(ScriptOp::DeleteRegistry {
                    root: 0,
                    key: key.clone(),
                    name: None,
                });
            }
            rollback_script.push(RollbackOp::RestoreRegistry {
                root: 0,
                key,
                name: None,
                previous_value: None,
                existed: !is_reg,
            });
        }
    }
}

/// Prepares product, feature, component, and user publishing actions.
fn prepare_publishing_action(
    action: &str,
    database: &LinkedDatabase,
    context: &EvaluationContext,
    install_script: &mut InstallScript,
    rollback_script: &mut RollbackScript,
) {
    let prod_code = context
        .get_property("ProductCode")
        .unwrap_or("{00000000-0000-0000-0000-000000000000}")
        .to_string();
    let prod_name = context
        .get_property("ProductName")
        .unwrap_or("App")
        .to_string();

    match action {
        "PublishComponents" | "UnpublishComponents" | "PublishFeatures" | "UnpublishFeatures" => {
            prepare_component_or_feature_publish(
                action,
                database,
                &prod_code,
                install_script,
                rollback_script,
            );
        }
        "PublishProduct" => {
            let key = format!(r"Installer\Products\{prod_code}");
            install_script.push(ScriptOp::WriteRegistry {
                root: 0,
                key: key.clone(),
                name: Some("ProductName".to_string()),
                value: Some(prod_name),
            });
            rollback_script.push(RollbackOp::RestoreRegistry {
                root: 0,
                key,
                name: Some("ProductName".to_string()),
                previous_value: None,
                existed: false,
            });
        }
        "UnpublishProduct" => {
            let key = format!(r"Installer\Products\{prod_code}");
            install_script.push(ScriptOp::DeleteRegistry {
                root: 0,
                key: key.clone(),
                name: Some("ProductName".to_string()),
            });
            rollback_script.push(RollbackOp::RestoreRegistry {
                root: 0,
                key,
                name: Some("ProductName".to_string()),
                previous_value: None,
                existed: true,
            });
        }
        "RegisterProduct" => {
            let key = format!(r"Software\Microsoft\Windows\CurrentVersion\Uninstall\{prod_code}");
            install_script.push(ScriptOp::WriteRegistry {
                root: 2,
                key: key.clone(),
                name: Some("DisplayName".to_string()),
                value: Some(prod_name),
            });
            rollback_script.push(RollbackOp::RestoreRegistry {
                root: 2,
                key,
                name: Some("DisplayName".to_string()),
                previous_value: None,
                existed: false,
            });
        }
        _ => {
            let owner = context
                .get_property("USERNAME")
                .unwrap_or("User")
                .to_string();
            let key = format!(
                r"Software\Microsoft\Windows\CurrentVersion\Installer\UserData\S-1-5-18\Products\{prod_code}\InstallProperties"
            );
            install_script.push(ScriptOp::WriteRegistry {
                root: 2,
                key: key.clone(),
                name: Some("RegOwner".to_string()),
                value: Some(owner),
            });
            rollback_script.push(RollbackOp::RestoreRegistry {
                root: 2,
                key,
                name: Some("RegOwner".to_string()),
                previous_value: None,
                existed: false,
            });
        }
    }
}

/// Prepares `PublishComponents`, `UnpublishComponents`, `PublishFeatures`, and `UnpublishFeatures`.
fn prepare_component_or_feature_publish(
    action: &str,
    database: &LinkedDatabase,
    prod_code: &str,
    install_script: &mut InstallScript,
    rollback_script: &mut RollbackScript,
) {
    match action {
        "PublishComponents" => {
            for rec in database.get_records("PublishComponent") {
                if let (Some(FieldValue::String(cat_id)), Some(FieldValue::String(qual))) =
                    (rec.get(0), rec.get(1))
                {
                    let key = format!(r"Installer\Components\{cat_id}");
                    install_script.push(ScriptOp::WriteRegistry {
                        root: 0,
                        key: key.clone(),
                        name: Some(qual.clone()),
                        value: Some(cat_id.clone()),
                    });
                    rollback_script.push(RollbackOp::RestoreRegistry {
                        root: 0,
                        key,
                        name: Some(qual.clone()),
                        previous_value: None,
                        existed: false,
                    });
                }
            }
        }
        "UnpublishComponents" => {
            for rec in database.get_records("PublishComponent") {
                if let (Some(FieldValue::String(cat_id)), Some(FieldValue::String(qual))) =
                    (rec.get(0), rec.get(1))
                {
                    let key = format!(r"Installer\Components\{cat_id}");
                    install_script.push(ScriptOp::DeleteRegistry {
                        root: 0,
                        key: key.clone(),
                        name: Some(qual.clone()),
                    });
                    rollback_script.push(RollbackOp::RestoreRegistry {
                        root: 0,
                        key,
                        name: Some(qual.clone()),
                        previous_value: None,
                        existed: true,
                    });
                }
            }
        }
        "PublishFeatures" => {
            let key = format!(r"Installer\Features\{prod_code}");
            for rec in database.get_records("Feature") {
                if let Some(FieldValue::String(feat_id)) = rec.get(0) {
                    let parent = match rec.get(1) {
                        Some(FieldValue::String(p)) => p.clone(),
                        _ => String::new(),
                    };
                    install_script.push(ScriptOp::WriteRegistry {
                        root: 0,
                        key: key.clone(),
                        name: Some(feat_id.clone()),
                        value: Some(parent),
                    });
                }
            }
        }
        _ => {
            let key = format!(r"Installer\Features\{prod_code}");
            for rec in database.get_records("Feature") {
                if let Some(FieldValue::String(feat_id)) = rec.get(0) {
                    install_script.push(ScriptOp::DeleteRegistry {
                        root: 0,
                        key: key.clone(),
                        name: Some(feat_id.clone()),
                    });
                }
            }
        }
    }
}

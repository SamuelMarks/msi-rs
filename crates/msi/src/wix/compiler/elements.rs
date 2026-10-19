//! Registry, shortcut, service, SQL, POSIX, COM, INI, and merge-module element compilation for the `WiX` Compiler.

use super::{format_msi_filename, parse_registry_root};
use crate::database::tables::config::RegistryRow;
use crate::database::tables::core::MediaRow;
use crate::database::tables::file_mgmt::RemoveFileRow;
use crate::database::tables::record::{FieldValue, Record};
use crate::database::tables::types::{generate_deterministic_id, ComponentName, DirectoryId};
use crate::error::{MsiError, Result};
use crate::wix::wixobj::{IntermediateSection, IntermediateTable, Reference, Symbol};
use crate::wix::xml::XmlNode;
use std::collections::HashMap;

/// Compiles a `<RegistryValue>` element.
pub(super) fn compile_registry_value(
    child: &XmlNode,
    parent_id: Option<&str>,
    inherited_root: Option<&str>,
    inherited_key: Option<&str>,
    section: &mut IntermediateSection,
    tables: &mut HashMap<String, IntermediateTable>,
) {
    let comp_name = parent_id.unwrap_or("DefaultComp");
    let comp_obj = parent_id.map_or_else(
        || ComponentName::from_static("DefaultComp"),
        ComponentName::from_validated,
    );
    let root_str = child.attribute("Root").or(inherited_root).unwrap_or("HKLM");
    let root = parse_registry_root(root_str);
    let key = child
        .attribute("Key")
        .or(inherited_key)
        .unwrap_or(r"Software\Product");
    let name = child.attribute("Name").map(ToString::to_string);
    let value = child.attribute("Value").map(ToString::to_string);

    let reg_id = child.attribute("Id").map_or_else(
        || {
            let seed = format!(
                "{}_{}_{}_{}_{}",
                comp_name,
                root_str,
                key,
                name.as_deref().unwrap_or(""),
                value.as_deref().unwrap_or("")
            );
            generate_deterministic_id("reg", &seed)
        },
        ToString::to_string,
    );

    section.add_symbol(Symbol::new("Registry", &reg_id));
    section.add_reference(Reference::new("Component", comp_name));

    let row = RegistryRow {
        registry: reg_id,
        root,
        key: key.to_string(),
        name,
        value,
        component: comp_obj,
    };
    tables
        .entry("Registry".to_string())
        .or_insert_with(|| IntermediateTable::new("Registry"))
        .push_record(row.to_record());
}

/// Compiles a `<Shortcut>` element.
pub(super) fn compile_shortcut(
    child: &XmlNode,
    parent_id: Option<&str>,
    section: &mut IntermediateSection,
    tables: &mut HashMap<String, IntermediateTable>,
) {
    let comp_name = parent_id.unwrap_or("DefaultComp");
    let sc_name_attr = child.attribute("Name");
    let sc_target = child.attribute("Target").unwrap_or("");
    let sc_dir = child.attribute("Directory").unwrap_or("ProgramMenuFolder");
    let sc_id = child.attribute("Id").map_or_else(
        || {
            let seed = format!(
                "{}_{}_{}_{}",
                comp_name,
                sc_dir,
                sc_name_attr.unwrap_or(""),
                sc_target
            );
            generate_deterministic_id("sc", &seed)
        },
        ToString::to_string,
    );
    let sc_name = sc_name_attr.unwrap_or(&sc_id);
    let sc_desc = child.attribute("Description").map(ToString::to_string);
    let sc_args = child.attribute("Arguments").map(ToString::to_string);
    let sc_icon = child.attribute("Icon").map(ToString::to_string);
    let sc_icon_index = child
        .attribute("IconIndex")
        .and_then(|v| v.parse::<i16>().ok());
    let sc_show = match child
        .attribute("Show")
        .map(str::to_ascii_lowercase)
        .as_deref()
    {
        Some("minimized") => Some(7i16),
        Some("maximized") => Some(3i16),
        Some("normal") => Some(1i16),
        _ => None,
    };
    let sc_wkdir = child
        .attribute("WorkingDirectory")
        .or_else(|| child.attribute("WkDir"))
        .map(ToString::to_string);

    section.add_symbol(Symbol::new("Shortcut", &sc_id));
    section.add_reference(Reference::new("Component", comp_name));
    section.add_reference(Reference::new("Directory", sc_dir));
    if let Some(ref wd) = sc_wkdir {
        section.add_reference(Reference::new("Directory", wd));
    }
    if let Some(ref ic) = sc_icon {
        section.add_reference(Reference::new("Icon", ic));
    }

    let sc_short_name = child.attribute("ShortName");
    let existing_sc_names: Vec<String> = tables
        .get("Shortcut")
        .map(|t| {
            t.records
                .iter()
                .filter(|r| r.get(1) == Some(&FieldValue::String(sc_dir.to_string())))
                .filter_map(|r| r.get(2).map(ToString::to_string))
                .collect()
        })
        .unwrap_or_default();
    let sc_name_str = format_msi_filename(sc_short_name, sc_name, &existing_sc_names);
    let rec = Record::with_fields(vec![
        FieldValue::String(sc_id),
        FieldValue::String(sc_dir.to_string()),
        FieldValue::String(sc_name_str),
        FieldValue::String(comp_name.to_string()),
        FieldValue::String(sc_target.to_string()),
        sc_args.map_or(FieldValue::Null, FieldValue::String),
        sc_desc.map_or(FieldValue::Null, FieldValue::String),
        FieldValue::Null,
        sc_icon.map_or(FieldValue::Null, FieldValue::String),
        sc_icon_index.map_or(FieldValue::Null, FieldValue::Short),
        sc_show.map_or(FieldValue::Null, FieldValue::Short),
        sc_wkdir.map_or(FieldValue::Null, FieldValue::String),
    ]);
    tables
        .entry("Shortcut".to_string())
        .or_insert_with(|| IntermediateTable::new("Shortcut"))
        .push_record(rec);
}

/// Compiles a `<ServiceInstall>` element.
pub(super) fn compile_service_install(
    child: &XmlNode,
    parent_id: Option<&str>,
    section: &mut IntermediateSection,
    tables: &mut HashMap<String, IntermediateTable>,
) {
    let svc_id = child
        .attribute("Id")
        .or_else(|| child.attribute("Name"))
        .unwrap_or("Service1");
    let svc_name = child.attribute("Name").unwrap_or(svc_id);
    let svc_display = child.attribute("DisplayName").map(ToString::to_string);
    let comp_name = parent_id.unwrap_or("DefaultComp");
    let svc_type = match child.attribute("Type").unwrap_or("ownProcess") {
        "shareProcess" => 32,
        "kernelDriver" => 1,
        "systemDriver" => 2,
        _ => 16,
    };
    let start_type = match child.attribute("Start").unwrap_or("auto") {
        "boot" => 0,
        "system" => 1,
        "demand" | "manual" => 3,
        "disabled" => 4,
        _ => 2,
    };
    let error_control = match child.attribute("ErrorControl").unwrap_or("normal") {
        "ignore" => 0,
        "severe" => 2,
        "critical" => 3,
        _ => 1,
    };

    let load_order = child.attribute("LoadOrderGroup").map(ToString::to_string);
    let deps = child.attribute("Dependencies").map(ToString::to_string);
    let account = child
        .attribute("Account")
        .or_else(|| child.attribute("StartName"))
        .map(ToString::to_string);
    let password = child.attribute("Password").map(ToString::to_string);

    section.add_symbol(Symbol::new("ServiceInstall", svc_id));
    section.add_reference(Reference::new("Component", comp_name));

    let rec = Record::with_fields(vec![
        FieldValue::String(svc_id.to_string()),
        FieldValue::String(svc_name.to_string()),
        svc_display.map_or(FieldValue::Null, FieldValue::String),
        FieldValue::Long(svc_type),
        FieldValue::Long(start_type),
        FieldValue::Long(error_control),
        load_order.map_or(FieldValue::Null, FieldValue::String),
        deps.map_or(FieldValue::Null, FieldValue::String),
        account.map_or(FieldValue::Null, FieldValue::String),
        password.map_or(FieldValue::Null, FieldValue::String),
        FieldValue::String(comp_name.to_string()),
    ]);
    tables
        .entry("ServiceInstall".to_string())
        .or_insert_with(|| IntermediateTable::new("ServiceInstall"))
        .push_record(rec);

    if let Some(desc) = child.attribute("Description") {
        let sc_rec = Record::with_fields(vec![
            FieldValue::String(format!("{svc_id}_Config")),
            FieldValue::String(svc_name.to_string()),
            FieldValue::Long(1),
            FieldValue::Long(1),
            FieldValue::String(desc.to_string()),
            FieldValue::String(comp_name.to_string()),
        ]);
        tables
            .entry("MsiServiceConfig".to_string())
            .or_insert_with(|| IntermediateTable::new("MsiServiceConfig"))
            .push_record(sc_rec);
    }
}

/// Compiles a `<ServiceControl>` element.
pub(super) fn compile_service_control(
    child: &XmlNode,
    parent_id: Option<&str>,
    section: &mut IntermediateSection,
    tables: &mut HashMap<String, IntermediateTable>,
) {
    let comp_name = parent_id.unwrap_or("DefaultComp");
    let ctrl_name_attr = child.attribute("Name");
    let mut event: i16 = 0;

    if let Some(s) = child.attribute("Start") {
        match s.to_ascii_lowercase().as_str() {
            "install" | "yes" => event |= 0x0001,
            "uninstall" => event |= 0x0010,
            "both" => event |= 0x0011,
            _ => {}
        }
    }
    if let Some(s) = child.attribute("Stop") {
        match s.to_ascii_lowercase().as_str() {
            "install" => event |= 0x0002,
            "uninstall" => event |= 0x0020,
            "both" | "yes" => event |= 0x0022,
            _ => {}
        }
    }
    if let Some(s) = child.attribute("Remove") {
        match s.to_ascii_lowercase().as_str() {
            "install" => event |= 0x0008,
            "uninstall" | "yes" => event |= 0x0080,
            "both" => event |= 0x0088,
            _ => {}
        }
    }

    let ctrl_id = child.attribute("Id").map_or_else(
        || {
            let seed = format!("{comp_name}_{}_{event}", ctrl_name_attr.unwrap_or(""));
            generate_deterministic_id("svc_ctrl", &seed)
        },
        ToString::to_string,
    );
    let ctrl_name = ctrl_name_attr.unwrap_or(&ctrl_id);

    let arguments = child.attribute("Arguments").map(ToString::to_string);
    let wait = match child
        .attribute("Wait")
        .map(str::to_ascii_lowercase)
        .as_deref()
    {
        Some("yes" | "1") => Some(1i16),
        Some("no" | "0") => Some(0i16),
        _ => None,
    };

    let ctrl_name_str = ctrl_name.to_string();
    section.add_symbol(Symbol::new("ServiceControl", &ctrl_id));
    section.add_reference(Reference::new("Component", comp_name));

    let rec = Record::with_fields(vec![
        FieldValue::String(ctrl_id),
        FieldValue::String(ctrl_name_str),
        FieldValue::Short(event),
        arguments.map_or(FieldValue::Null, FieldValue::String),
        wait.map_or(FieldValue::Null, FieldValue::Short),
        FieldValue::String(comp_name.to_string()),
    ]);
    tables
        .entry("ServiceControl".to_string())
        .or_insert_with(|| IntermediateTable::new("ServiceControl"))
        .push_record(rec);
}

/// Compiles an `<EmbeddedChainer>` element into `MsiEmbeddedChainer` table.
pub(super) fn compile_embedded_chainer(
    child: &XmlNode,
    section: &mut IntermediateSection,
    tables: &mut HashMap<String, IntermediateTable>,
) -> Result<()> {
    let id = child.attribute("Id").ok_or_else(|| MsiError::WixCompiler {
        element: "EmbeddedChainer".to_string(),
        message: "missing required 'Id' attribute".to_string(),
    })?;

    let binary_key = child.attribute("BinaryKey");
    let file_key = child.attribute("FileKey");
    let source_file = child.attribute("SourceFile");

    let (source, chainer_type) = if let Some(bin) = binary_key {
        section.add_reference(Reference::new("Binary", bin));
        (bin.to_string(), 1)
    } else if let Some(file) = file_key {
        section.add_reference(Reference::new("File", file));
        (file.to_string(), 2)
    } else if let Some(src) = source_file {
        let auto_bin = format!("{id}_Binary");
        section.add_symbol(Symbol::new("Binary", &auto_bin));
        let bin_row = crate::database::tables::core::BinaryRow {
            name: auto_bin.clone(),
            data: crate::database::tables::types::StringPoolId::new(1),
        };
        tables
            .entry("Binary".to_string())
            .or_insert_with(|| IntermediateTable::new("Binary"))
            .push_record(bin_row.to_record());
        let mut wix_bin = Record::new();
        wix_bin.push(FieldValue::String(auto_bin.clone()));
        wix_bin.push(FieldValue::String(src.to_string()));
        tables
            .entry("WixBinary".to_string())
            .or_insert_with(|| IntermediateTable::new("WixBinary"))
            .push_record(wix_bin);
        (auto_bin, 1)
    } else {
        let def_bin = format!("{id}_Binary");
        section.add_reference(Reference::new("Binary", &def_bin));
        (def_bin, 1)
    };

    let cmd_line = child.attribute("CommandLine").map(ToString::to_string);
    let condition = child.attribute("Condition").map(ToString::to_string);

    section.add_symbol(Symbol::new("EmbeddedChainer", id));

    let rec = Record::with_fields(vec![
        FieldValue::String(id.to_string()),
        condition.map_or(FieldValue::Null, FieldValue::String),
        cmd_line.map_or(FieldValue::Null, FieldValue::String),
        FieldValue::String(source),
        FieldValue::Long(chainer_type),
    ]);

    tables
        .entry("MsiEmbeddedChainer".to_string())
        .or_insert_with(|| IntermediateTable::new("MsiEmbeddedChainer"))
        .push_record(rec);

    Ok(())
}

/// Compiles a `<SqlDatabase>` or `<sql:SqlDatabase>` element.
pub(super) fn compile_sql_database(
    child: &XmlNode,
    parent_id: Option<&str>,
    section: &mut IntermediateSection,
    tables: &mut HashMap<String, IntermediateTable>,
) -> Result<()> {
    let db_id = child.attribute("Id").ok_or_else(|| MsiError::WixCompiler {
        element: "SqlDatabase".to_string(),
        message: "missing required 'Id' attribute".to_string(),
    })?;

    let server = child.attribute("Server").unwrap_or("127.0.0.1");
    let instance = child.attribute("Instance").map(ToString::to_string);
    let database = child.attribute("Database").unwrap_or(db_id);
    let comp_name = parent_id.unwrap_or("DefaultComp");
    let user = child.attribute("User").map(ToString::to_string);

    let mut attrs: i32 = 0;
    if child.attribute("CreateOnInstall") == Some("yes") {
        attrs |= 0x0001;
    }
    if child.attribute("DropOnUninstall") == Some("yes") {
        attrs |= 0x0002;
    }
    if child.attribute("ContinueOnError") == Some("yes") {
        attrs |= 0x0004;
    }
    if child.attribute("DropOnInstall") == Some("yes") {
        attrs |= 0x0008;
    }
    if child.attribute("CreateOnUninstall") == Some("yes") {
        attrs |= 0x0010;
    }

    section.add_symbol(Symbol::new("SqlDatabase", db_id));
    section.add_reference(Reference::new("Component", comp_name));

    let rec = Record::with_fields(vec![
        FieldValue::String(db_id.to_string()),
        FieldValue::String(server.to_string()),
        instance.map_or(FieldValue::Null, FieldValue::String),
        FieldValue::String(database.to_string()),
        FieldValue::String(comp_name.to_string()),
        user.map_or(FieldValue::Null, FieldValue::String),
        FieldValue::Long(attrs),
    ]);

    tables
        .entry("SqlDatabase".to_string())
        .or_insert_with(|| IntermediateTable::new("SqlDatabase"))
        .push_record(rec);

    Ok(())
}

/// Compiles a `<SqlString>` or `<sql:SqlString>` element.
pub(super) fn compile_sql_string(
    child: &XmlNode,
    parent_id: Option<&str>,
    section: &mut IntermediateSection,
    tables: &mut HashMap<String, IntermediateTable>,
) -> Result<()> {
    let str_id = child.attribute("Id").ok_or_else(|| MsiError::WixCompiler {
        element: "SqlString".to_string(),
        message: "missing required 'Id' attribute".to_string(),
    })?;

    let sql_db = child
        .attribute("SqlDb")
        .or(parent_id)
        .unwrap_or("DefaultSqlDb");

    let sql_text = if child.text.trim().is_empty() {
        child
            .attribute("SQL")
            .map(ToString::to_string)
            .unwrap_or_default()
    } else {
        child.text.trim().to_string()
    };

    if sql_text.is_empty() {
        return Err(MsiError::WixCompiler {
            element: "SqlString".to_string(),
            message: format!("missing SQL statement text in SqlString '{str_id}'"),
        });
    }

    let user = child.attribute("User").map(ToString::to_string);
    let mut attrs: i32 = 0;
    if child.attribute("ExecuteOnInstall") == Some("yes")
        || child.attribute("ExecuteOnInstall").is_none()
    {
        attrs |= 0x0001;
    }
    if child.attribute("ExecuteOnUninstall") == Some("yes") {
        attrs |= 0x0002;
    }
    if child.attribute("Rollback") == Some("yes") {
        attrs |= 0x0004;
    }
    if child.attribute("ContinueOnError") == Some("yes") {
        attrs |= 0x0008;
    }

    let sequence = child
        .attribute("Sequence")
        .and_then(|s| s.parse::<i32>().ok());

    section.add_symbol(Symbol::new("SqlString", str_id));
    section.add_reference(Reference::new("SqlDatabase", sql_db));

    let rec = Record::with_fields(vec![
        FieldValue::String(str_id.to_string()),
        FieldValue::String(sql_db.to_string()),
        FieldValue::String(sql_text),
        user.map_or(FieldValue::Null, FieldValue::String),
        FieldValue::Long(attrs),
        sequence.map_or(FieldValue::Null, FieldValue::Long),
    ]);

    tables
        .entry("SqlString".to_string())
        .or_insert_with(|| IntermediateTable::new("SqlString"))
        .push_record(rec);

    Ok(())
}

/// Compiles a `<SqlScript>` or `<sql:SqlScript>` element.
pub(super) fn compile_sql_script(
    child: &XmlNode,
    parent_id: Option<&str>,
    section: &mut IntermediateSection,
    tables: &mut HashMap<String, IntermediateTable>,
) -> Result<()> {
    let script_id = child.attribute("Id").ok_or_else(|| MsiError::WixCompiler {
        element: "SqlScript".to_string(),
        message: "missing required 'Id' attribute".to_string(),
    })?;

    let sql_db = child.attribute("SqlDb").unwrap_or("DefaultSqlDb");

    let script_file = child
        .attribute("ScriptFile")
        .or_else(|| child.attribute("BinaryKey"))
        .unwrap_or("script.sql");

    let comp_name = parent_id.unwrap_or("DefaultComp");
    let user = child.attribute("User").map(ToString::to_string);

    let mut attrs: i32 = 0;
    if child.attribute("ExecuteOnInstall") == Some("yes")
        || child.attribute("ExecuteOnInstall").is_none()
    {
        attrs |= 0x0001;
    }
    if child.attribute("ExecuteOnUninstall") == Some("yes") {
        attrs |= 0x0002;
    }
    if child.attribute("Rollback") == Some("yes") {
        attrs |= 0x0004;
    }
    if child.attribute("ContinueOnError") == Some("yes") {
        attrs |= 0x0008;
    }

    let sequence = child
        .attribute("Sequence")
        .and_then(|s| s.parse::<i32>().ok());

    section.add_symbol(Symbol::new("SqlScript", script_id));
    section.add_reference(Reference::new("SqlDatabase", sql_db));
    section.add_reference(Reference::new("Component", comp_name));

    let rec = Record::with_fields(vec![
        FieldValue::String(script_id.to_string()),
        FieldValue::String(sql_db.to_string()),
        FieldValue::String(comp_name.to_string()),
        FieldValue::String(script_file.to_string()),
        user.map_or(FieldValue::Null, FieldValue::String),
        FieldValue::Long(attrs),
        sequence.map_or(FieldValue::Null, FieldValue::Long),
    ]);

    tables
        .entry("SqlScript".to_string())
        .or_insert_with(|| IntermediateTable::new("SqlScript"))
        .push_record(rec);

    Ok(())
}

/// Compiles cross-platform POSIX extension elements.
pub(super) fn compile_posix_element(
    child: &XmlNode,
    parent_id: Option<&str>,
    section: &mut IntermediateSection,
    tables: &mut HashMap<String, IntermediateTable>,
) {
    match child.tag.as_str() {
        "posix:File" | "PosixFile" => {
            compile_posix_file(child, parent_id, tables);
        }
        "posix:Symlink" | "PosixSymlink" => {
            compile_posix_symlink(child, parent_id, section, tables);
        }
        "posix:Daemon" | "PosixDaemon" => {
            compile_posix_daemon(child, section, tables);
        }
        "posix:Acl" | "PosixAcl" => {
            compile_posix_acl(child, section, tables);
        }
        "posix:Desktop" | "PosixDesktop" => {
            compile_posix_desktop(child, section, tables);
        }
        _ => {}
    }
}

/// Compiles `<posix:File>` element.
pub(super) fn compile_posix_file(
    child: &XmlNode,
    parent_id: Option<&str>,
    tables: &mut HashMap<String, IntermediateTable>,
) {
    let file_key = child.attribute("File").or(parent_id).unwrap_or("AppFile");
    let mode: i32 = child
        .attribute("Mode")
        .and_then(|m| i32::from_str_radix(m.trim_start_matches("0o"), 8).ok())
        .unwrap_or(0o755);
    let owner = child.attribute("Owner").map(ToString::to_string);
    let group = child.attribute("Group").map(ToString::to_string);
    let rec = Record::with_fields(vec![
        FieldValue::String(file_key.to_string()),
        FieldValue::Long(mode),
        owner.map_or(FieldValue::Null, FieldValue::String),
        group.map_or(FieldValue::Null, FieldValue::String),
        FieldValue::Null,
    ]);
    tables
        .entry("PosixFile".to_string())
        .or_insert_with(|| IntermediateTable::new("PosixFile"))
        .push_record(rec);
}

/// Compiles `<posix:Symlink>` element.
pub(super) fn compile_posix_symlink(
    child: &XmlNode,
    parent_id: Option<&str>,
    section: &mut IntermediateSection,
    tables: &mut HashMap<String, IntermediateTable>,
) {
    let sym_id = child.attribute("Id").unwrap_or("Symlink1");
    let target = child.attribute("Target").unwrap_or("");
    let dir = child.attribute("LinkDirectory").unwrap_or("TARGETDIR");
    let name = child.attribute("LinkName").unwrap_or(sym_id);
    let comp = parent_id.unwrap_or("DefaultComp");

    section.add_symbol(Symbol::new("PosixSymlink", sym_id));
    section.add_reference(Reference::new("Component", comp));

    let rec = Record::with_fields(vec![
        FieldValue::String(sym_id.to_string()),
        FieldValue::String(target.to_string()),
        FieldValue::String(dir.to_string()),
        FieldValue::String(name.to_string()),
        FieldValue::String(comp.to_string()),
    ]);
    tables
        .entry("PosixSymlink".to_string())
        .or_insert_with(|| IntermediateTable::new("PosixSymlink"))
        .push_record(rec);
}

/// Compiles `<posix:Daemon>` element.
pub(super) fn compile_posix_daemon(
    child: &XmlNode,
    section: &mut IntermediateSection,
    tables: &mut HashMap<String, IntermediateTable>,
) {
    let svc = child.attribute("Service").unwrap_or("Service1");
    let sup_str = child.attribute("SupervisorType").unwrap_or("Systemd");
    let sup_code: i32 = match sup_str.to_ascii_lowercase().as_str() {
        "launchd" => 1,
        "rcd" | "rc.d" => 2,
        "smf" => 3,
        _ => 0,
    };
    let restart: i32 = match child.attribute("RestartPolicy").unwrap_or("always") {
        "on-failure" => 1,
        "no" => 2,
        _ => 0,
    };
    let user = child.attribute("RunAsUser").map(ToString::to_string);

    section.add_symbol(Symbol::new("PosixDaemon", svc));

    let rec = Record::with_fields(vec![
        FieldValue::String(svc.to_string()),
        FieldValue::Long(sup_code),
        FieldValue::Null,
        FieldValue::Long(restart),
        user.map_or(FieldValue::Null, FieldValue::String),
    ]);
    tables
        .entry("PosixDaemon".to_string())
        .or_insert_with(|| IntermediateTable::new("PosixDaemon"))
        .push_record(rec);
}

/// Compiles `<posix:Acl>` element.
pub(super) fn compile_posix_acl(
    child: &XmlNode,
    section: &mut IntermediateSection,
    tables: &mut HashMap<String, IntermediateTable>,
) {
    let acl_id = child.attribute("Id").unwrap_or("Acl1");
    let file = child.attribute("File").unwrap_or("File1");
    let p_type = child.attribute("PrincipalType").unwrap_or("user");
    let p_name = child.attribute("PrincipalName").unwrap_or("root");
    let perms: i32 = child
        .attribute("Permissions")
        .and_then(|p| p.parse().ok())
        .unwrap_or(7);

    section.add_symbol(Symbol::new("PosixAcl", acl_id));

    let rec = Record::with_fields(vec![
        FieldValue::String(acl_id.to_string()),
        FieldValue::String(file.to_string()),
        FieldValue::String(p_type.to_string()),
        FieldValue::String(p_name.to_string()),
        FieldValue::Long(perms),
        FieldValue::Long(0),
    ]);
    tables
        .entry("PosixAcl".to_string())
        .or_insert_with(|| IntermediateTable::new("PosixAcl"))
        .push_record(rec);
}

/// Compiles `<posix:Desktop>` element.
pub(super) fn compile_posix_desktop(
    child: &XmlNode,
    section: &mut IntermediateSection,
    tables: &mut HashMap<String, IntermediateTable>,
) {
    let sc = child.attribute("Shortcut").unwrap_or("Shortcut1");
    let cat = child.attribute("Categories").map(ToString::to_string);
    let mime = child.attribute("MimeTypes").map(ToString::to_string);
    let term: i16 = i16::from(
        child
            .attribute("Terminal")
            .is_some_and(|v| v == "yes" || v == "true"),
    );

    section.add_symbol(Symbol::new("PosixDesktop", sc));

    let rec = Record::with_fields(vec![
        FieldValue::String(sc.to_string()),
        cat.map_or(FieldValue::Null, FieldValue::String),
        mime.map_or(FieldValue::Null, FieldValue::String),
        FieldValue::Null,
        FieldValue::Short(term),
    ]);
    tables
        .entry("PosixDesktop".to_string())
        .or_insert_with(|| IntermediateTable::new("PosixDesktop"))
        .push_record(rec);
}

/// Compiles a `<Configuration>` element in a Merge Module.
pub(super) fn compile_module_configuration(
    child: &XmlNode,
    tables: &mut HashMap<String, IntermediateTable>,
) {
    let name = child.attribute("Name").unwrap_or("Param1");
    let format: i32 = child
        .attribute("Format")
        .and_then(|f| f.parse().ok())
        .unwrap_or(0);
    let row = crate::database::tables::core::ModuleConfigurationRow {
        name: name.to_string(),
        format,
        type_: child.attribute("Type").map(ToString::to_string),
        context_data: child.attribute("ContextData").map(ToString::to_string),
        default_value: child.attribute("DefaultValue").map(ToString::to_string),
        attributes: child.attribute("Attributes").and_then(|a| a.parse().ok()),
        display_name: child.attribute("DisplayName").map(ToString::to_string),
        description: child.attribute("Description").map(ToString::to_string),
        help_keyword: child.attribute("HelpKeyword").map(ToString::to_string),
    };
    tables
        .entry("ModuleConfiguration".to_string())
        .or_insert_with(|| IntermediateTable::new("ModuleConfiguration"))
        .push_record(row.to_record());
}

/// Compiles a `<Substitution>` element in a Merge Module.
pub(super) fn compile_module_substitution(
    child: &XmlNode,
    tables: &mut HashMap<String, IntermediateTable>,
) {
    let table = child.attribute("Table").unwrap_or("Property");
    let row_key = child.attribute("Row").unwrap_or("");
    let column = child.attribute("Column").unwrap_or("Value");
    let value = child.attribute("Value").map(ToString::to_string);
    let row = crate::database::tables::core::ModuleSubstitutionRow {
        table: table.to_string(),
        row: row_key.to_string(),
        column: column.to_string(),
        value,
    };
    tables
        .entry("ModuleSubstitution".to_string())
        .or_insert_with(|| IntermediateTable::new("ModuleSubstitution"))
        .push_record(row.to_record());
}

/// Compiles an `<IgnoreModularization>` element in a Merge Module.
pub(super) fn compile_module_ignore_modularization(
    child: &XmlNode,
    tables: &mut HashMap<String, IntermediateTable>,
) {
    let name = child.attribute("Name").unwrap_or("");
    let type_val = child.attribute("Type").and_then(|t| t.parse().ok());
    let row = crate::database::tables::core::ModuleIgnoreModularizationRow {
        name: name.to_string(),
        type_: type_val,
    };
    tables
        .entry("ModuleIgnoreModularization".to_string())
        .or_insert_with(|| IntermediateTable::new("ModuleIgnoreModularization"))
        .push_record(row.to_record());
}

/// Compiles a `<Dependency>` or `<ModuleDependency>` element in a Merge Module.
pub(super) fn compile_module_dependency(
    child: &XmlNode,
    tables: &mut HashMap<String, IntermediateTable>,
) {
    let req_id = child
        .attribute("RequiredId")
        .or_else(|| child.attribute("Id"))
        .unwrap_or("");
    let req_lang = child
        .attribute("RequiredLanguage")
        .or_else(|| child.attribute("Language"))
        .and_then(|l| l.parse().ok())
        .unwrap_or(1033);
    let req_ver = child
        .attribute("RequiredVersion")
        .or_else(|| child.attribute("Version"))
        .map(ToString::to_string);
    let mod_id = child.attribute("ModuleId").unwrap_or("Module");
    let mod_lang = child
        .attribute("ModuleLanguage")
        .and_then(|l| l.parse().ok())
        .unwrap_or(1033);

    let row = crate::database::tables::core::ModuleDependencyRow {
        module_id: mod_id.to_string(),
        module_language: mod_lang,
        required_id: req_id.to_string(),
        required_language: req_lang,
        required_version: req_ver,
    };
    tables
        .entry("ModuleDependency".to_string())
        .or_insert_with(|| IntermediateTable::new("ModuleDependency"))
        .push_record(row.to_record());
}

/// Compiles an `<Exclusion>` or `<ModuleExclusion>` element in a Merge Module.
pub(super) fn compile_module_exclusion(
    child: &XmlNode,
    tables: &mut HashMap<String, IntermediateTable>,
) {
    let excl_id = child
        .attribute("ExcludedId")
        .or_else(|| child.attribute("Id"))
        .unwrap_or("");
    let excl_lang = child
        .attribute("ExcludedLanguage")
        .or_else(|| child.attribute("Language"))
        .and_then(|l| l.parse().ok())
        .unwrap_or(1033);
    let ver_min = child
        .attribute("ExcludedVersionMin")
        .or_else(|| child.attribute("VersionMin"))
        .map(ToString::to_string);
    let ver_max = child
        .attribute("ExcludedVersionMax")
        .or_else(|| child.attribute("VersionMax"))
        .map(ToString::to_string);
    let mod_id = child.attribute("ModuleId").unwrap_or("Module");
    let mod_lang = child
        .attribute("ModuleLanguage")
        .and_then(|l| l.parse().ok())
        .unwrap_or(1033);

    let row = crate::database::tables::core::ModuleExclusionRow {
        module_id: mod_id.to_string(),
        module_language: mod_lang,
        excluded_id: excl_id.to_string(),
        excluded_language: excl_lang,
        excluded_version_min: ver_min,
        excluded_version_max: ver_max,
    };
    tables
        .entry("ModuleExclusion".to_string())
        .or_insert_with(|| IntermediateTable::new("ModuleExclusion"))
        .push_record(row.to_record());
}

/// Compiles a `<CopyFile>` element into `DuplicateFile` or `MoveFile` table.
pub(super) fn compile_copy_file(
    child: &XmlNode,
    parent_id: Option<&str>,
    section: &mut IntermediateSection,
    tables: &mut HashMap<String, IntermediateTable>,
) {
    let id = child.attribute("Id").unwrap_or("CopyFile1");
    let comp = parent_id.unwrap_or("DefaultComp");
    section.add_symbol(Symbol::new("CopyFile", id));
    section.add_reference(Reference::new("Component", comp));

    if let Some(file_id) = child.attribute("FileId") {
        let dest_name = child.attribute("DestinationName");
        let dest_folder = child
            .attribute("DestinationDirectory")
            .or_else(|| child.attribute("DestinationProperty"));
        let rec = Record::with_fields(vec![
            FieldValue::String(id.to_string()),
            FieldValue::String(comp.to_string()),
            FieldValue::String(file_id.to_string()),
            dest_name.map_or(FieldValue::Null, |s| FieldValue::String(s.to_string())),
            dest_folder.map_or(FieldValue::Null, |s| FieldValue::String(s.to_string())),
        ]);
        tables
            .entry("DuplicateFile".to_string())
            .or_insert_with(|| IntermediateTable::new("DuplicateFile"))
            .push_record(rec);
    } else {
        let src_name = child.attribute("SourceName").unwrap_or("*");
        let dest_name = child.attribute("DestinationName").unwrap_or(src_name);
        let src_folder = child
            .attribute("SourceDirectory")
            .or_else(|| child.attribute("SourceProperty"))
            .unwrap_or("TARGETDIR");
        let dest_folder = child
            .attribute("DestinationDirectory")
            .or_else(|| child.attribute("DestinationProperty"))
            .unwrap_or("TARGETDIR");
        let delete = child
            .attribute("Delete")
            .is_some_and(|s| s.eq_ignore_ascii_case("yes"));
        let options: i16 = i16::from(delete);
        let rec = Record::with_fields(vec![
            FieldValue::String(id.to_string()),
            FieldValue::String(comp.to_string()),
            FieldValue::String(src_name.to_string()),
            FieldValue::String(dest_name.to_string()),
            FieldValue::String(src_folder.to_string()),
            FieldValue::String(dest_folder.to_string()),
            FieldValue::Short(options),
        ]);
        tables
            .entry("MoveFile".to_string())
            .or_insert_with(|| IntermediateTable::new("MoveFile"))
            .push_record(rec);
    }
}

/// Compiles a `<MoveFile>` element into `MoveFile` table.
pub(super) fn compile_move_file(
    child: &XmlNode,
    parent_id: Option<&str>,
    section: &mut IntermediateSection,
    tables: &mut HashMap<String, IntermediateTable>,
) {
    let id = child.attribute("Id").unwrap_or("MoveFile1");
    let comp = parent_id.unwrap_or("DefaultComp");
    let src_name = child.attribute("SourceName").unwrap_or("*");
    let dest_name = child.attribute("DestName").unwrap_or(src_name);
    let src_folder = child.attribute("SourceFolder").unwrap_or("TARGETDIR");
    let dest_folder = child.attribute("DestFolder").unwrap_or("TARGETDIR");

    section.add_symbol(Symbol::new("MoveFile", id));
    section.add_reference(Reference::new("Component", comp));

    let rec = Record::with_fields(vec![
        FieldValue::String(id.to_string()),
        FieldValue::String(comp.to_string()),
        FieldValue::String(src_name.to_string()),
        FieldValue::String(dest_name.to_string()),
        FieldValue::String(src_folder.to_string()),
        FieldValue::String(dest_folder.to_string()),
        FieldValue::Short(1),
    ]);
    tables
        .entry("MoveFile".to_string())
        .or_insert_with(|| IntermediateTable::new("MoveFile"))
        .push_record(rec);
}

/// Compiles a `<RemoveFile>` element into `RemoveFile` table.
pub(super) fn compile_remove_file(
    child: &XmlNode,
    parent_id: Option<&str>,
    section: &mut IntermediateSection,
    tables: &mut HashMap<String, IntermediateTable>,
) {
    let id = child.attribute("Id").unwrap_or("RemoveFile1");
    let comp = parent_id.unwrap_or("DefaultComp");
    let comp_obj = parent_id.map_or_else(
        || ComponentName::from_static("DefaultComp"),
        ComponentName::from_validated,
    );
    let name = child.attribute("Name");
    let dir = child
        .attribute("Directory")
        .or_else(|| child.attribute("Property"))
        .unwrap_or("TARGETDIR");
    let on = child.attribute("On").unwrap_or("uninstall");
    let mode: i16 = match on {
        "install" => 1,
        "both" => 3,
        _ => 2,
    };

    section.add_symbol(Symbol::new("RemoveFile", id));
    section.add_reference(Reference::new("Component", comp));

    let row = RemoveFileRow {
        file_key: id.to_string(),
        component: comp_obj,
        file_name: name.map(ToString::to_string),
        dir_property: dir.to_string(),
        install_mode: mode,
    };
    tables
        .entry("RemoveFile".to_string())
        .or_insert_with(|| IntermediateTable::new("RemoveFile"))
        .push_record(row.to_record());
}

/// Compiles a `<SymbolicLink>` or `<Hardlink>` element into `PosixSymlink` table.
pub(super) fn compile_symlink(
    child: &XmlNode,
    parent_id: Option<&str>,
    section: &mut IntermediateSection,
    tables: &mut HashMap<String, IntermediateTable>,
) -> Result<()> {
    let id = child.attribute("Id").unwrap_or("Symlink1");
    let target = child
        .attribute("Target")
        .or_else(|| child.attribute("TargetPath"))
        .unwrap_or("");
    let dir = child
        .attribute("Directory")
        .or(parent_id)
        .unwrap_or("TARGETDIR");
    let name = child.attribute("Name").unwrap_or(id);
    let comp = parent_id.unwrap_or("DefaultComp");
    let comp_obj = parent_id.map_or_else(
        || ComponentName::from_static("DefaultComp"),
        ComponentName::from_validated,
    );

    section.add_symbol(Symbol::new("SymbolicLink", id));
    section.add_reference(Reference::new("Component", comp));

    let row = crate::database::tables::posix::PosixSymlinkRow {
        symlink_key: id.to_string(),
        target_path: target.to_string(),
        link_directory: DirectoryId::new(dir)?,
        link_name: name.to_string(),
        component: comp_obj,
    };
    tables
        .entry("PosixSymlink".to_string())
        .or_insert_with(|| IntermediateTable::new("PosixSymlink"))
        .push_record(row.to_record());
    Ok(())
}

/// Compiles a `<MediaTemplate>` element into a synthesized `Media` table entry.
pub(super) fn compile_media_template(
    child: &XmlNode,
    tables: &mut HashMap<String, IntermediateTable>,
) {
    let cab_template = child.attribute("CabinetTemplate").unwrap_or("cab1.cab");
    let cab_name = cab_template.replace("{0}", "1");
    let disk_prompt = child.attribute("DiskPrompt").map(ToString::to_string);
    let row = MediaRow {
        disk_id: 1,
        last_sequence: 1,
        disk_prompt,
        cabinet: Some(cab_name),
        volume_label: None,
        source: None,
    };
    tables
        .entry("Media".to_string())
        .or_insert_with(|| IntermediateTable::new("Media"))
        .push_record(row.to_record());
}

/// Compiles `<RemoveRegistryKey>` or `<RemoveRegistryValue>` into `RemoveRegistry` table.
pub(super) fn compile_remove_registry(
    child: &XmlNode,
    parent_id: Option<&str>,
    section: &mut IntermediateSection,
    tables: &mut HashMap<String, IntermediateTable>,
) {
    let id = child.attribute("Id").unwrap_or("RemoveReg1");
    let root_str = child.attribute("Root").unwrap_or("HKLM");
    let root = parse_registry_root(root_str);
    let key = child.attribute("Key").unwrap_or("");
    let name = child.attribute("Name").map(ToString::to_string);
    let comp = parent_id.unwrap_or("DefaultComp");

    section.add_symbol(Symbol::new("RemoveRegistry", id));
    section.add_reference(Reference::new("Component", comp));

    let rec = Record::with_fields(vec![
        FieldValue::String(id.to_string()),
        FieldValue::Short(root),
        FieldValue::String(key.to_string()),
        name.map_or(FieldValue::Null, FieldValue::String),
        FieldValue::String(comp.to_string()),
    ]);
    tables
        .entry("RemoveRegistry".to_string())
        .or_insert_with(|| IntermediateTable::new("RemoveRegistry"))
        .push_record(rec);
}

/// Compiles an `<IniFile>` element into `IniFile` table.
pub(super) fn compile_ini_file(
    child: &XmlNode,
    parent_id: Option<&str>,
    section: &mut IntermediateSection,
    tables: &mut HashMap<String, IntermediateTable>,
) {
    let id = child.attribute("Id").unwrap_or("Ini1");
    let file_name = child.attribute("Name").unwrap_or("config.ini");
    let dir = child.attribute("Directory");
    let section_name = child.attribute("Section").unwrap_or("Settings");
    let key = child.attribute("Key").unwrap_or("Key");
    let value = child.attribute("Value").unwrap_or("");
    let action_str = child.attribute("Action").unwrap_or("addLine");
    let action: i16 = match action_str {
        "createLine" => 1,
        "replaceLine" => 2,
        "addTag" => 3,
        "removeTag" => 4,
        _ => 0,
    };
    let comp = parent_id.unwrap_or("DefaultComp");

    section.add_symbol(Symbol::new("IniFile", id));
    section.add_reference(Reference::new("Component", comp));

    let rec = Record::with_fields(vec![
        FieldValue::String(id.to_string()),
        FieldValue::String(file_name.to_string()),
        dir.map_or(FieldValue::Null, |d| FieldValue::String(d.to_string())),
        FieldValue::String(section_name.to_string()),
        FieldValue::String(key.to_string()),
        FieldValue::String(value.to_string()),
        FieldValue::Short(action),
        FieldValue::String(comp.to_string()),
    ]);
    tables
        .entry("IniFile".to_string())
        .or_insert_with(|| IntermediateTable::new("IniFile"))
        .push_record(rec);
}

/// Compiles a `<RemoveIniFile>` element into `RemoveIniFile` table.
pub(super) fn compile_remove_ini_file(
    child: &XmlNode,
    parent_id: Option<&str>,
    section: &mut IntermediateSection,
    tables: &mut HashMap<String, IntermediateTable>,
) {
    let id = child.attribute("Id").unwrap_or("RemoveIni1");
    let file_name = child.attribute("Name").unwrap_or("config.ini");
    let dir = child.attribute("Directory");
    let section_name = child.attribute("Section").unwrap_or("Settings");
    let key = child.attribute("Key").unwrap_or("Key");
    let value = child.attribute("Value");
    let comp = parent_id.unwrap_or("DefaultComp");

    section.add_symbol(Symbol::new("RemoveIniFile", id));
    section.add_reference(Reference::new("Component", comp));

    let rec = Record::with_fields(vec![
        FieldValue::String(id.to_string()),
        FieldValue::String(file_name.to_string()),
        dir.map_or(FieldValue::Null, |d| FieldValue::String(d.to_string())),
        FieldValue::String(section_name.to_string()),
        FieldValue::String(key.to_string()),
        value.map_or(FieldValue::Null, |v| FieldValue::String(v.to_string())),
        FieldValue::Short(4),
        FieldValue::String(comp.to_string()),
    ]);
    tables
        .entry("RemoveIniFile".to_string())
        .or_insert_with(|| IntermediateTable::new("RemoveIniFile"))
        .push_record(rec);
}

/// Compiles an `<Icon>` element into `Icon` table.
pub(super) fn compile_icon(
    child: &XmlNode,
    section: &mut IntermediateSection,
    tables: &mut HashMap<String, IntermediateTable>,
) {
    let id = child.attribute("Id").unwrap_or("AppIcon");
    section.add_symbol(Symbol::new("Icon", id));
    let rec = Record::with_fields(vec![
        FieldValue::String(id.to_string()),
        FieldValue::Stream(crate::database::tables::types::StringPoolId::new(1)),
    ]);
    tables
        .entry("Icon".to_string())
        .or_insert_with(|| IntermediateTable::new("Icon"))
        .push_record(rec);

    if let Some(src) = child
        .attribute("SourceFile")
        .or_else(|| child.attribute("Source"))
    {
        let mut wix_icon = Record::new();
        wix_icon.push(FieldValue::String(id.to_string()));
        wix_icon.push(FieldValue::String(src.to_string()));
        tables
            .entry("WixIcon".to_string())
            .or_insert_with(|| IntermediateTable::new("WixIcon"))
            .push_record(wix_icon);
    }
}

/// Compiles a `<ProgId>` element into `ProgId` table.
pub(super) fn compile_prog_id(
    child: &XmlNode,
    parent_id: Option<&str>,
    section: &mut IntermediateSection,
    tables: &mut HashMap<String, IntermediateTable>,
) {
    let id = child.attribute("Id").unwrap_or("App.Document");
    let desc = child.attribute("Description").map(ToString::to_string);
    let icon = child.attribute("Icon").map(ToString::to_string);
    let icon_idx: Option<i16> = child.attribute("IconIndex").and_then(|i| i.parse().ok());
    let class_id = parent_id.map(ToString::to_string);

    section.add_symbol(Symbol::new("ProgId", id));

    let rec = Record::with_fields(vec![
        FieldValue::String(id.to_string()),
        FieldValue::Null,
        class_id.map_or(FieldValue::Null, FieldValue::String),
        desc.map_or(FieldValue::Null, FieldValue::String),
        icon.map_or(FieldValue::Null, FieldValue::String),
        icon_idx.map_or(FieldValue::Null, FieldValue::Short),
    ]);
    tables
        .entry("ProgId".to_string())
        .or_insert_with(|| IntermediateTable::new("ProgId"))
        .push_record(rec);
}

/// Compiles an `<Extension>` element into `Extension` and `Verb` tables.
pub(super) fn compile_extension(
    child: &XmlNode,
    parent_id: Option<&str>,
    section: &mut IntermediateSection,
    tables: &mut HashMap<String, IntermediateTable>,
) {
    let ext = child.attribute("Id").unwrap_or("txt");
    let comp = parent_id.unwrap_or("DefaultComp");
    let prog_id = child.attribute("ProgId");
    let mime = child
        .attribute("MIME")
        .or_else(|| child.attribute("ContentType"));

    section.add_symbol(Symbol::new("Extension", ext));
    section.add_reference(Reference::new("Component", comp));

    let rec = Record::with_fields(vec![
        FieldValue::String(ext.to_string()),
        FieldValue::String(comp.to_string()),
        prog_id.map_or(FieldValue::Null, |p| FieldValue::String(p.to_string())),
        mime.map_or(FieldValue::Null, |m| FieldValue::String(m.to_string())),
        FieldValue::Null,
    ]);
    tables
        .entry("Extension".to_string())
        .or_insert_with(|| IntermediateTable::new("Extension"))
        .push_record(rec);

    for sub in &child.children {
        if sub.tag == "Verb" {
            let verb_id = sub.attribute("Id").unwrap_or("open");
            let cmd = sub.attribute("Command").map(ToString::to_string);
            let arg = sub.attribute("Argument").map(ToString::to_string);
            let seq: Option<i16> = sub.attribute("Sequence").and_then(|s| s.parse().ok());

            let verb_rec = Record::with_fields(vec![
                FieldValue::String(ext.to_string()),
                FieldValue::String(verb_id.to_string()),
                seq.map_or(FieldValue::Null, FieldValue::Short),
                cmd.map_or(FieldValue::Null, FieldValue::String),
                arg.map_or(FieldValue::Null, FieldValue::String),
            ]);
            tables
                .entry("Verb".to_string())
                .or_insert_with(|| IntermediateTable::new("Verb"))
                .push_record(verb_rec);
        }
    }
}

/// Compiles a `<MIME>` element into `MIME` table.
pub(super) fn compile_mime(
    child: &XmlNode,
    section: &mut IntermediateSection,
    tables: &mut HashMap<String, IntermediateTable>,
) {
    let content_type = child
        .attribute("ContentType")
        .unwrap_or("application/octet-stream");
    let ext = child
        .attribute("DefaultExtension")
        .or_else(|| child.attribute("Extension"))
        .unwrap_or("");
    let clsid = child
        .attribute("CLSID")
        .or_else(|| child.attribute("Class"));

    section.add_symbol(Symbol::new("MIME", content_type));

    let rec = Record::with_fields(vec![
        FieldValue::String(content_type.to_string()),
        FieldValue::String(ext.to_string()),
        clsid.map_or(FieldValue::Null, |c| FieldValue::String(c.to_string())),
    ]);
    tables
        .entry("MIME".to_string())
        .or_insert_with(|| IntermediateTable::new("MIME"))
        .push_record(rec);
}

/// Compiles a `<Class>` element into `Class` table.
pub(super) fn compile_class(
    child: &XmlNode,
    parent_id: Option<&str>,
    section: &mut IntermediateSection,
    tables: &mut HashMap<String, IntermediateTable>,
) {
    let clsid = child
        .attribute("Id")
        .unwrap_or("{00000000-0000-0000-0000-000000000000}");
    let context = child.attribute("Context").unwrap_or("InprocServer32");
    let comp = parent_id.unwrap_or("DefaultComp");
    let desc = child.attribute("Description").map(ToString::to_string);
    let prog_id = child.attribute("ProgId");
    let app_id = child.attribute("AppId");

    section.add_symbol(Symbol::new("Class", clsid));
    section.add_reference(Reference::new("Component", comp));

    let rec = Record::with_fields(vec![
        FieldValue::String(clsid.to_string()),
        FieldValue::String(context.to_string()),
        FieldValue::String(comp.to_string()),
        prog_id.map_or(FieldValue::Null, |p| FieldValue::String(p.to_string())),
        desc.map_or(FieldValue::Null, FieldValue::String),
        app_id.map_or(FieldValue::Null, |a| FieldValue::String(a.to_string())),
        FieldValue::Null,
        FieldValue::Null,
        FieldValue::Null,
        FieldValue::Null,
        FieldValue::Null,
        FieldValue::Null,
    ]);
    tables
        .entry("Class".to_string())
        .or_insert_with(|| IntermediateTable::new("Class"))
        .push_record(rec);
}

/// Compiles a `<TypeLib>` element into `TypeLib` table.
pub(super) fn compile_type_lib(
    child: &XmlNode,
    parent_id: Option<&str>,
    section: &mut IntermediateSection,
    tables: &mut HashMap<String, IntermediateTable>,
) {
    let lib_id = child
        .attribute("Id")
        .unwrap_or("{00000000-0000-0000-0000-000000000000}");
    let lang: i16 = child
        .attribute("Language")
        .and_then(|l| l.parse().ok())
        .unwrap_or(0);
    let comp = parent_id.unwrap_or("DefaultComp");
    let desc = child.attribute("Description").map(ToString::to_string);
    let dir = child.attribute("Directory");

    section.add_symbol(Symbol::new("TypeLib", lib_id));
    section.add_reference(Reference::new("Component", comp));

    let rec = Record::with_fields(vec![
        FieldValue::String(lib_id.to_string()),
        FieldValue::Short(lang),
        FieldValue::String(comp.to_string()),
        FieldValue::Null,
        desc.map_or(FieldValue::Null, FieldValue::String),
        dir.map_or(FieldValue::Null, |d| FieldValue::String(d.to_string())),
        FieldValue::Null,
        FieldValue::Null,
    ]);
    tables
        .entry("TypeLib".to_string())
        .or_insert_with(|| IntermediateTable::new("TypeLib"))
        .push_record(rec);
}

/// Compiles an `<AppId>` element into `AppId` table.
pub(super) fn compile_app_id(
    child: &XmlNode,
    section: &mut IntermediateSection,
    tables: &mut HashMap<String, IntermediateTable>,
) {
    let app_id = child
        .attribute("Id")
        .unwrap_or("{00000000-0000-0000-0000-000000000000}");
    let remote_server = child.attribute("RemoteServerName").map(ToString::to_string);
    let local_service = child.attribute("LocalService").map(ToString::to_string);

    section.add_symbol(Symbol::new("AppId", app_id));

    let rec = Record::with_fields(vec![
        FieldValue::String(app_id.to_string()),
        remote_server.map_or(FieldValue::Null, FieldValue::String),
        local_service.map_or(FieldValue::Null, FieldValue::String),
        FieldValue::Null,
        FieldValue::Null,
        FieldValue::Null,
        FieldValue::Null,
    ]);
    tables
        .entry("AppId".to_string())
        .or_insert_with(|| IntermediateTable::new("AppId"))
        .push_record(rec);
}

//! UI dialogs, controls, custom actions, sequences, upgrades, and `AppSearch` compilation for the `WiX` Compiler.

use super::{convert_text_to_rtf, parse_registry_root, Compiler};
use crate::database::tables::record::{FieldValue, Record};
use crate::database::tables::sequence::SequenceRow;
use crate::database::tables::types::generate_deterministic_id;
use crate::error::{MsiError, Result};
use crate::wix::wixobj::{IntermediateSection, IntermediateTable, Reference, Symbol};
use crate::wix::xml::XmlNode;
use std::collections::HashMap;

/// Compiles a `<CustomAction>` element.
pub(super) fn compile_custom_action(
    child: &XmlNode,
    section: &mut IntermediateSection,
    tables: &mut HashMap<String, IntermediateTable>,
) -> Result<()> {
    let ca_id = child.attribute("Id").ok_or_else(|| MsiError::WixCompiler {
        element: "CustomAction".to_string(),
        message: "missing required 'Id' attribute".to_string(),
    })?;
    let (source, target, mut ca_type) = match (
        child.attribute("BinaryKey"),
        child.attribute("FileKey"),
        child.attribute("Property"),
        child.attribute("Directory"),
        child.attribute("Script"),
        child.attribute("Error"),
    ) {
        (Some(bin), _, _, _, _, _) => {
            section.add_reference(Reference::new("Binary", bin));
            match (
                child.attribute("VBScriptCall"),
                child.attribute("JScriptCall"),
                child.attribute("DllEntry"),
                child.attribute("ExeCommand"),
            ) {
                (Some(vbs), _, _, _) => (bin, vbs, 6),
                (None, Some(js), _, _) => (bin, js, 5),
                (None, None, Some(dll), _) => (bin, dll, 1),
                (None, None, None, Some(exe)) => (bin, exe, 2),
                _ => (bin, "", 1),
            }
        }
        (None, Some(file), _, _, _, _) => {
            section.add_reference(Reference::new("File", file));
            (file, child.attribute("ExeCommand").unwrap_or(""), 18)
        }
        (None, None, Some(prop), _, _, _) => (prop, child.attribute("Value").unwrap_or(""), 51),
        (None, None, None, Some(dir), _, _) => {
            section.add_reference(Reference::new("Directory", dir));
            child.attribute("Value").map_or_else(
                || (dir, child.attribute("ExeCommand").unwrap_or(""), 34),
                |val| (dir, val, 35),
            )
        }
        (None, None, None, None, Some("vbscript"), _) => ("", child.text.as_str(), 6),
        (None, None, None, None, Some("jscript"), _) => ("", child.text.as_str(), 5),
        (None, None, None, None, _, Some(err)) => ("", err, 19),
        _ => ("", "", 1),
    };

    match child.attribute("Execute") {
        Some("deferred") => ca_type |= 0x0400,
        Some("rollback") => ca_type |= 0x0500,
        Some("commit") => ca_type |= 0x0600,
        Some("firstSequence") => ca_type |= 0x0100,
        Some("oncePerProcess") => ca_type |= 0x0200,
        _ => {}
    }

    match child.attribute("Return") {
        Some("ignore") => ca_type |= 0x0040,
        Some("asyncWait") => ca_type |= 0x0080,
        Some("asyncNoWait") => ca_type |= 0x00C0,
        _ => {}
    }

    if child
        .attribute("Impersonate")
        .is_some_and(|s| s.eq_ignore_ascii_case("no"))
    {
        ca_type |= 0x0800;
    }

    section.add_symbol(Symbol::new("CustomAction", ca_id));

    let rec = Record::with_fields(vec![
        FieldValue::String(ca_id.to_string()),
        FieldValue::Short(ca_type),
        FieldValue::String(source.to_string()),
        FieldValue::String(target.to_string()),
        FieldValue::Null,
    ]);
    tables
        .entry("CustomAction".to_string())
        .or_insert_with(|| IntermediateTable::new("CustomAction"))
        .push_record(rec);
    Ok(())
}

/// Compiles a `<Dialog>` element.
pub(super) fn compile_dialog(
    child: &XmlNode,
    section: &mut IntermediateSection,
    tables: &mut HashMap<String, IntermediateTable>,
) -> Result<()> {
    let dlg_id = child.attribute("Id").ok_or_else(|| MsiError::WixCompiler {
        element: "Dialog".to_string(),
        message: "missing required 'Id' attribute".to_string(),
    })?;
    let width: i16 = child
        .attribute("Width")
        .and_then(|w| w.parse().ok())
        .unwrap_or(370);
    let height: i16 = child
        .attribute("Height")
        .and_then(|h| h.parse().ok())
        .unwrap_or(270);
    let title = child.attribute("Title").map(ToString::to_string);

    let mut first_ctrl = None;
    let mut default_ctrl = None;
    let mut cancel_ctrl = None;

    for c in &child.children {
        if c.tag == "Control" {
            if let Some(cid) = c.attribute("Id") {
                if first_ctrl.is_none() {
                    first_ctrl = Some(cid.to_string());
                }
                if c.attribute("Default")
                    .is_some_and(|v| v.eq_ignore_ascii_case("yes"))
                {
                    default_ctrl = Some(cid.to_string());
                }
                if c.attribute("Cancel")
                    .is_some_and(|v| v.eq_ignore_ascii_case("yes"))
                {
                    cancel_ctrl = Some(cid.to_string());
                }
            }
        }
    }

    let first = first_ctrl.unwrap_or_else(|| "FirstControl".to_string());

    section.add_symbol(Symbol::new("Dialog", dlg_id));

    let rec = Record::with_fields(vec![
        FieldValue::String(dlg_id.to_string()),
        FieldValue::Short(50),
        FieldValue::Short(50),
        FieldValue::Short(width),
        FieldValue::Short(height),
        FieldValue::Long(3),
        title.map_or(FieldValue::Null, FieldValue::String),
        FieldValue::String(first),
        default_ctrl.map_or(FieldValue::Null, FieldValue::String),
        cancel_ctrl.map_or(FieldValue::Null, FieldValue::String),
    ]);
    tables
        .entry("Dialog".to_string())
        .or_insert_with(|| IntermediateTable::new("Dialog"))
        .push_record(rec);
    Ok(())
}

/// Compiles a `<Control>` element.
pub(super) fn compile_control(
    compiler: &Compiler,
    child: &XmlNode,
    parent_id: Option<&str>,
    section: &mut IntermediateSection,
    tables: &mut HashMap<String, IntermediateTable>,
) -> Result<()> {
    let ctrl_id = child.attribute("Id").ok_or_else(|| MsiError::WixCompiler {
        element: "Control".to_string(),
        message: "missing required 'Id' attribute".to_string(),
    })?;
    let ctrl_type = child.attribute("Type").unwrap_or("PushButton");
    let x: i16 = child
        .attribute("X")
        .and_then(|v| v.parse().ok())
        .unwrap_or(0);
    let y: i16 = child
        .attribute("Y")
        .and_then(|v| v.parse().ok())
        .unwrap_or(0);
    let w: i16 = child
        .attribute("Width")
        .and_then(|v| v.parse().ok())
        .unwrap_or(56);
    let h: i16 = child
        .attribute("Height")
        .and_then(|v| v.parse().ok())
        .unwrap_or(17);

    let resolved_text = resolve_control_text(compiler, child, ctrl_type);
    let prop = child.attribute("Property").map(ToString::to_string);
    let dlg_name = parent_id.unwrap_or("DefaultDialog");
    let attributes = parse_control_attributes(child);

    section.add_symbol(Symbol::new("Control", format!("{dlg_name}.{ctrl_id}")));

    let rec = Record::with_fields(vec![
        FieldValue::String(dlg_name.to_string()),
        FieldValue::String(ctrl_id.to_string()),
        FieldValue::String(ctrl_type.to_string()),
        FieldValue::Short(x),
        FieldValue::Short(y),
        FieldValue::Short(w),
        FieldValue::Short(h),
        FieldValue::Long(attributes),
        prop.map_or(FieldValue::Null, FieldValue::String),
        resolved_text.map_or(FieldValue::Null, FieldValue::String),
        FieldValue::Null,
        FieldValue::Null,
    ]);
    tables
        .entry("Control".to_string())
        .or_insert_with(|| IntermediateTable::new("Control"))
        .push_record(rec);

    compile_control_children(child, dlg_name, ctrl_id, w, section, tables);
    Ok(())
}

/// Resolves the text or RTF payload for a `<Control>` element.
fn resolve_control_text(compiler: &Compiler, child: &XmlNode, ctrl_type: &str) -> Option<String> {
    let mut resolved_text = child.attribute("Text").map(ToString::to_string);
    if resolved_text.is_none() {
        if let Some(text_sub) = child.children.iter().find(|c| c.tag == "Text") {
            if let Some(src) = text_sub.attribute("SourceFile") {
                let candidate_content = compiler
                    .source_dir
                    .as_ref()
                    .and_then(|dir| std::fs::read_to_string(dir.join(src)).ok())
                    .or_else(|| std::fs::read_to_string(src).ok());
                if let Some(content) = candidate_content {
                    if std::path::Path::new(src)
                        .extension()
                        .is_some_and(|ext| ext.eq_ignore_ascii_case("rtf"))
                    {
                        resolved_text = Some(content);
                    } else {
                        resolved_text = Some(convert_text_to_rtf(&content));
                    }
                } else if ctrl_type.eq_ignore_ascii_case("ScrollableText") {
                    resolved_text = Some(format!("!(bindpath.rtf:{src})"));
                } else {
                    resolved_text = Some(src.to_string());
                }
            } else if !text_sub.text.is_empty() {
                if ctrl_type.eq_ignore_ascii_case("ScrollableText")
                    && !text_sub.text.trim_start().starts_with("{\x5crtf")
                    && !text_sub.text.trim_start().starts_with("!(wix.")
                {
                    resolved_text = Some(convert_text_to_rtf(&text_sub.text));
                } else {
                    resolved_text = Some(text_sub.text.clone());
                }
            }
        }
    } else if ctrl_type.eq_ignore_ascii_case("ScrollableText") {
        let attr_val = resolved_text.as_deref().unwrap_or("");
        let trimmed = attr_val.trim_start();
        if !trimmed.is_empty()
            && !trimmed.starts_with("{\x5crtf")
            && !trimmed.starts_with("!(wix.")
            && !trimmed.starts_with("!(bindpath.rtf:")
        {
            resolved_text = Some(convert_text_to_rtf(attr_val));
        }
    }
    resolved_text
}

/// Parses control attribute bitflags for a `<Control>` element.
fn parse_control_attributes(child: &XmlNode) -> i32 {
    let mut attributes: i32 = 3; // Visible | Enabled
    if child
        .attribute("Hidden")
        .is_some_and(|v| v.eq_ignore_ascii_case("yes"))
    {
        attributes &= !1;
    }
    if child
        .attribute("Disabled")
        .is_some_and(|v| v.eq_ignore_ascii_case("yes"))
    {
        attributes &= !2;
    }
    for (attr, bit) in [
        ("Sunken", 0x0000_0004),
        ("Transparent", 0x0001_0000),
        ("RightAligned", 0x0000_0040),
        ("FormatSize", 0x0000_0400),
        ("HasBorder", 0x0100_0000),
        ("Multiline", 0x0004_0000),
        ("Password", 0x0020_0000),
        ("NoPrefix", 0x0002_0000),
        ("NoWrap", 0x0004_0000),
    ] {
        if child
            .attribute(attr)
            .is_some_and(|v| v.eq_ignore_ascii_case("yes"))
        {
            attributes |= bit;
        }
    }
    attributes
}

/// Compiles child elements (`Publish`, `Condition`, `Subscribe`, `RadioButtonGroup`, `RadioButton`) inside a `<Control>`.
fn compile_control_children(
    child: &XmlNode,
    dlg_name: &str,
    ctrl_id: &str,
    w: i16,
    section: &mut IntermediateSection,
    tables: &mut HashMap<String, IntermediateTable>,
) {
    for sub in &child.children {
        match sub.tag.as_str() {
            "Publish" | "ControlEvent" => {
                let event = sub.attribute("Property").map_or_else(
                    || sub.attribute("Event").unwrap_or("NewDialog").to_string(),
                    |target_prop| format!("[{target_prop}]"),
                );
                let arg = sub
                    .attribute("Argument")
                    .or_else(|| sub.attribute("Value"))
                    .unwrap_or("");
                let cond = sub
                    .attribute("Condition")
                    .map(ToString::to_string)
                    .or_else(|| {
                        if sub.text.is_empty() {
                            None
                        } else {
                            Some(sub.text.clone())
                        }
                    });
                let order: Option<i16> = sub.attribute("Order").and_then(|o| o.parse().ok());
                let ce_rec = Record::with_fields(vec![
                    FieldValue::String(dlg_name.to_string()),
                    FieldValue::String(ctrl_id.to_string()),
                    FieldValue::String(event),
                    FieldValue::String(arg.to_string()),
                    cond.map_or(FieldValue::Null, FieldValue::String),
                    order.map_or(FieldValue::Null, FieldValue::Short),
                ]);
                tables
                    .entry("ControlEvent".to_string())
                    .or_insert_with(|| IntermediateTable::new("ControlEvent"))
                    .push_record(ce_rec);
            }
            "Condition" | "ControlCondition" => {
                let action = sub.attribute("Action").unwrap_or("enable");
                let cond = sub.attribute("Condition").unwrap_or_else(|| {
                    if sub.text.is_empty() {
                        "1"
                    } else {
                        &sub.text
                    }
                });
                let cc_rec = Record::with_fields(vec![
                    FieldValue::String(dlg_name.to_string()),
                    FieldValue::String(ctrl_id.to_string()),
                    FieldValue::String(action.to_string()),
                    FieldValue::String(cond.to_string()),
                ]);
                tables
                    .entry("ControlCondition".to_string())
                    .or_insert_with(|| IntermediateTable::new("ControlCondition"))
                    .push_record(cc_rec);
            }
            "Subscribe" => {
                let event = sub.attribute("Event").unwrap_or("SetProgress");
                let attr = sub.attribute("Attribute").unwrap_or("Progress");
                let em_rec = Record::with_fields(vec![
                    FieldValue::String(dlg_name.to_string()),
                    FieldValue::String(ctrl_id.to_string()),
                    FieldValue::String(event.to_string()),
                    FieldValue::String(attr.to_string()),
                ]);
                tables
                    .entry("EventMapping".to_string())
                    .or_insert_with(|| IntermediateTable::new("EventMapping"))
                    .push_record(em_rec);
            }
            "RadioButtonGroup" => {
                let rbg_prop = sub
                    .attribute("Property")
                    .or_else(|| child.attribute("Property"))
                    .unwrap_or("");
                for (idx, rb) in sub
                    .children
                    .iter()
                    .filter(|c| c.tag == "RadioButton")
                    .enumerate()
                {
                    let order = i16::try_from(idx + 1).unwrap_or(1);
                    push_radio_button_record(rb, rbg_prop, order, w, section, tables);
                }
            }
            "RadioButton" => {
                let rbg_prop = child.attribute("Property").unwrap_or("");
                let order: i16 = sub
                    .attribute("Order")
                    .and_then(|o| o.parse().ok())
                    .unwrap_or(1);
                push_radio_button_record(sub, rbg_prop, order, w, section, tables);
            }
            _ => {}
        }
    }
}

/// Emits a `RadioButton` table record.
fn push_radio_button_record(
    rb: &XmlNode,
    rbg_prop: &str,
    order: i16,
    default_w: i16,
    section: &mut IntermediateSection,
    tables: &mut HashMap<String, IntermediateTable>,
) {
    let val = rb.attribute("Value").unwrap_or("");
    let rx: i16 = rb.attribute("X").and_then(|v| v.parse().ok()).unwrap_or(0);
    let ry: i16 = rb.attribute("Y").and_then(|v| v.parse().ok()).unwrap_or(0);
    let rw: i16 = rb
        .attribute("Width")
        .and_then(|v| v.parse().ok())
        .unwrap_or(default_w);
    let rh: i16 = rb
        .attribute("Height")
        .and_then(|v| v.parse().ok())
        .unwrap_or(14);
    let rtext = rb.attribute("Text").map_or_else(|| rb.text.as_str(), |t| t);
    let rhelp = rb.attribute("Help");

    section.add_symbol(Symbol::new("RadioButton", format!("{rbg_prop}.{val}")));

    let rb_rec = Record::with_fields(vec![
        FieldValue::String(rbg_prop.to_string()),
        FieldValue::Short(order),
        FieldValue::String(val.to_string()),
        FieldValue::Short(rx),
        FieldValue::Short(ry),
        FieldValue::Short(rw),
        FieldValue::Short(rh),
        FieldValue::String(rtext.to_string()),
        rhelp.map_or(FieldValue::Null, |help_text| {
            FieldValue::String(help_text.to_string())
        }),
    ]);
    tables
        .entry("RadioButton".to_string())
        .or_insert_with(|| IntermediateTable::new("RadioButton"))
        .push_record(rb_rec);
}

/// Compiles a `<TextStyle>` element.
pub(super) fn compile_text_style(
    child: &XmlNode,
    section: &mut IntermediateSection,
    tables: &mut HashMap<String, IntermediateTable>,
) {
    let style_id = child.attribute("Id").unwrap_or("DefaultStyle");
    let face = child.attribute("FaceName").unwrap_or("Tahoma");
    let size: i16 = child
        .attribute("Size")
        .and_then(|s| s.parse().ok())
        .unwrap_or(9);

    section.add_symbol(Symbol::new("TextStyle", style_id));

    let rec = Record::with_fields(vec![
        FieldValue::String(style_id.to_string()),
        FieldValue::String(face.to_string()),
        FieldValue::Short(size),
        FieldValue::Null,
        FieldValue::Null,
    ]);
    tables
        .entry("TextStyle".to_string())
        .or_insert_with(|| IntermediateTable::new("TextStyle"))
        .push_record(rec);
}

/// Compiles an `<Upgrade>` element.
pub(super) fn compile_upgrade(
    child: &XmlNode,
    section: &mut IntermediateSection,
    tables: &mut HashMap<String, IntermediateTable>,
) {
    let upg_id = child
        .attribute("Id")
        .unwrap_or("{00000000-0000-0000-0000-000000000000}");
    section.add_symbol(Symbol::new("Upgrade", upg_id));

    for sub in &child.children {
        if sub.tag == "UpgradeVersion" {
            let min = sub.attribute("Minimum").map(ToString::to_string);
            let max = sub.attribute("Maximum").map(ToString::to_string);
            let prop = sub.attribute("Property").unwrap_or("NEWPRODUCTFOUND");

            let mut attrs: i32 = 0;
            let inc_min = sub
                .attribute("IncludeMinimum")
                .is_none_or(|v| v.eq_ignore_ascii_case("yes"));
            if inc_min {
                attrs |= 0x0000_0100;
            }
            if sub
                .attribute("IncludeMaximum")
                .is_some_and(|v| v.eq_ignore_ascii_case("yes"))
            {
                attrs |= 0x0000_0200;
            }
            if sub
                .attribute("OnlyDetect")
                .is_some_and(|v| v.eq_ignore_ascii_case("yes"))
            {
                attrs |= 0x0000_0002;
            }
            if sub
                .attribute("MigrateFeatures")
                .is_some_and(|v| v.eq_ignore_ascii_case("yes"))
            {
                attrs |= 0x0000_0001;
            }
            if sub
                .attribute("IgnoreRemoveFailure")
                .is_some_and(|v| v.eq_ignore_ascii_case("yes"))
            {
                attrs |= 0x0000_0004;
            }

            let rec = Record::with_fields(vec![
                FieldValue::String(upg_id.to_string()),
                min.map_or(FieldValue::Null, FieldValue::String),
                max.map_or(FieldValue::Null, FieldValue::String),
                FieldValue::Null,
                FieldValue::Long(attrs),
                FieldValue::Null,
                FieldValue::String(prop.to_string()),
            ]);
            tables
                .entry("Upgrade".to_string())
                .or_insert_with(|| IntermediateTable::new("Upgrade"))
                .push_record(rec);
        }
    }
}

/// Compiles a `<MajorUpgrade>` syntactic macro.
pub(super) fn compile_major_upgrade(
    child: &XmlNode,
    tables: &mut HashMap<String, IntermediateTable>,
) {
    let downgrade_err = child.attribute("DowngradeErrorMessage");
    let schedule = child
        .attribute("Schedule")
        .unwrap_or("afterInstallInitialize");
    let allow_same = child
        .attribute("AllowSameVersionUpgrades")
        .is_some_and(|s| s.eq_ignore_ascii_case("yes"));

    let older_attrs: i32 = if allow_same { 256 | 1 } else { 256 };
    let older_rec = Record::with_fields(vec![
        FieldValue::String("{00000000-0000-0000-0000-000000000000}".to_string()),
        FieldValue::Null,
        FieldValue::Null,
        FieldValue::Null,
        FieldValue::Long(older_attrs),
        FieldValue::Null,
        FieldValue::String("WIX_UPGRADE_DETECTED".to_string()),
    ]);
    let mut upg_records = vec![older_rec];

    if let Some(err_msg) = downgrade_err {
        let newer_rec = Record::with_fields(vec![
            FieldValue::String("{00000000-0000-0000-0000-000000000000}".to_string()),
            FieldValue::Null,
            FieldValue::Null,
            FieldValue::Null,
            FieldValue::Long(2),
            FieldValue::Null,
            FieldValue::String("WIX_DOWNGRADE_DETECTED".to_string()),
        ]);
        upg_records.push(newer_rec);

        let lc_rec = Record::with_fields(vec![
            FieldValue::String("NOT WIX_DOWNGRADE_DETECTED".to_string()),
            FieldValue::String(err_msg.to_string()),
        ]);
        tables
            .entry("LaunchCondition".to_string())
            .or_insert_with(|| IntermediateTable::new("LaunchCondition"))
            .push_record(lc_rec);
    }

    let upg_tbl = tables
        .entry("Upgrade".to_string())
        .or_insert_with(|| IntermediateTable::new("Upgrade"));
    for r in upg_records {
        upg_tbl.push_record(r);
    }

    let prop_rec = Record::with_fields(vec![
        FieldValue::String("SecureCustomProperties".to_string()),
        FieldValue::String("WIX_UPGRADE_DETECTED;WIX_DOWNGRADE_DETECTED".to_string()),
    ]);
    tables
        .entry("Property".to_string())
        .or_insert_with(|| IntermediateTable::new("Property"))
        .push_record(prop_rec);

    let rep_seq: i16 = match schedule {
        "afterInstallValidate" => 1400,
        "afterInstallExecute" => 6550,
        "afterInstallFinalize" => 6601,
        _ => 1501,
    };
    let rep_rec = Record::with_fields(vec![
        FieldValue::String("RemoveExistingProducts".to_string()),
        FieldValue::Null,
        FieldValue::Short(rep_seq),
    ]);
    let frp_rec = Record::with_fields(vec![
        FieldValue::String("FindRelatedProducts".to_string()),
        FieldValue::Null,
        FieldValue::Short(200),
    ]);
    let ies_tbl = tables
        .entry("InstallExecuteSequence".to_string())
        .or_insert_with(|| IntermediateTable::new("InstallExecuteSequence"));
    ies_tbl.push_record(rep_rec);
    ies_tbl.push_record(frp_rec);
}

/// Compiles a `<SetProperty>` element (syntactic sugar for Type 51 custom action).
pub(super) fn compile_set_property(
    child: &XmlNode,
    section: &mut IntermediateSection,
    tables: &mut HashMap<String, IntermediateTable>,
) -> Result<()> {
    let prop = child
        .attribute("Id")
        .or_else(|| child.attribute("Property"))
        .unwrap_or("PROP");
    let val = child.attribute("Value").unwrap_or("");
    let action_name = child
        .attribute("Action")
        .map_or_else(|| format!("Set_{prop}"), ToString::to_string);

    section.add_symbol(Symbol::new("CustomAction", &action_name));

    let ca_rec = Record::with_fields(vec![
        FieldValue::String(action_name.clone()),
        FieldValue::Short(51),
        FieldValue::String(prop.to_string()),
        FieldValue::String(val.to_string()),
        FieldValue::Null,
    ]);
    tables
        .entry("CustomAction".to_string())
        .or_insert_with(|| IntermediateTable::new("CustomAction"))
        .push_record(ca_rec);

    let seq_num = child.attribute("Sequence").and_then(|s| s.parse().ok());
    let cond = child
        .children
        .iter()
        .find(|c| c.tag == "Condition")
        .map(|c| c.text.clone())
        .or_else(|| child.attribute("Condition").map(ToString::to_string));
    let seq_row = SequenceRow::new(&action_name, cond, seq_num)?;
    tables
        .entry("InstallExecuteSequence".to_string())
        .or_insert_with(|| IntermediateTable::new("InstallExecuteSequence"))
        .push_record(seq_row.to_record());
    Ok(())
}

/// Compiles `<AppSearch>` element into `AppSearch` table.
pub(super) fn compile_app_search(
    child: &XmlNode,
    section: &mut IntermediateSection,
    tables: &mut HashMap<String, IntermediateTable>,
) {
    let prop = child.attribute("Property").unwrap_or("PROP");
    let sig = child.attribute("Id").unwrap_or(prop);
    section.add_symbol(Symbol::new("AppSearch", prop));
    let rec = Record::with_fields(vec![
        FieldValue::String(prop.to_string()),
        FieldValue::String(sig.to_string()),
    ]);
    tables
        .entry("AppSearch".to_string())
        .or_insert_with(|| IntermediateTable::new("AppSearch"))
        .push_record(rec);
}

/// Compiles `<RegistrySearch>` element into `RegLocator` table.
pub(super) fn compile_registry_search(
    child: &XmlNode,
    parent_id: Option<&str>,
    section: &mut IntermediateSection,
    tables: &mut HashMap<String, IntermediateTable>,
) {
    let root_str = child.attribute("Root").unwrap_or("HKLM");
    let root = parse_registry_root(root_str);
    let key = child.attribute("Key").unwrap_or("");
    let name = child.attribute("Name").map(ToString::to_string);
    let sig = child.attribute("Id").map_or_else(
        || {
            let seed = format!(
                "{}_{}_{}_{}",
                parent_id.unwrap_or(""),
                root_str,
                key,
                name.as_deref().unwrap_or("")
            );
            generate_deterministic_id("reg_search", &seed)
        },
        ToString::to_string,
    );
    let type_str = child.attribute("Type").unwrap_or("raw");
    let mut type_num: i16 = match type_str {
        "directory" => 0,
        "file" => 1,
        _ => 2,
    };

    if child
        .attribute("Win64")
        .is_some_and(|v| v.eq_ignore_ascii_case("yes"))
    {
        type_num |= 0x0010;
    }

    let mut nested_file_searches = Vec::new();
    for sub in &child.children {
        if sub.tag == "FileSearch" {
            type_num &= 0x0010;
            let file_name = sub.attribute("Name").unwrap_or("target.exe");
            let file_sig = sub.attribute("Id").map_or_else(
                || {
                    let seed = format!("{sig}_{file_name}");
                    generate_deterministic_id("file_search", &seed)
                },
                ToString::to_string,
            );
            let min_ver = sub.attribute("MinVersion").map(ToString::to_string);
            let max_ver = sub.attribute("MaxVersion").map(ToString::to_string);
            nested_file_searches.push((file_sig, file_name, min_ver, max_ver));
        }
    }

    section.add_symbol(Symbol::new("Signature", &sig));
    if let Some(parent_prop) = parent_id {
        let app_search_rec = Record::with_fields(vec![
            FieldValue::String(parent_prop.to_string()),
            FieldValue::String(sig.clone()),
        ]);
        tables
            .entry("AppSearch".to_string())
            .or_insert_with(|| IntermediateTable::new("AppSearch"))
            .push_record(app_search_rec);
    }

    let rec = Record::with_fields(vec![
        FieldValue::String(sig),
        FieldValue::Short(root),
        FieldValue::String(key.to_string()),
        name.map_or(FieldValue::Null, FieldValue::String),
        FieldValue::Short(type_num),
    ]);
    tables
        .entry("RegLocator".to_string())
        .or_insert_with(|| IntermediateTable::new("RegLocator"))
        .push_record(rec);

    for (file_sig, file_name, min_ver, max_ver) in nested_file_searches {
        section.add_symbol(Symbol::new("Signature", &file_sig));
        let sig_rec = Record::with_fields(vec![
            FieldValue::String(file_sig),
            FieldValue::String(file_name.to_string()),
            min_ver.map_or(FieldValue::Null, FieldValue::String),
            max_ver.map_or(FieldValue::Null, FieldValue::String),
            FieldValue::Null,
            FieldValue::Null,
            FieldValue::Null,
            FieldValue::Null,
            FieldValue::Null,
        ]);
        tables
            .entry("Signature".to_string())
            .or_insert_with(|| IntermediateTable::new("Signature"))
            .push_record(sig_rec);
    }
}

/// Compiles `<DirectorySearch>` element into `DrLocator` table.
pub(super) fn compile_directory_search(
    child: &XmlNode,
    parent_id: Option<&str>,
    section: &mut IntermediateSection,
    tables: &mut HashMap<String, IntermediateTable>,
) {
    let path = child.attribute("Path").map(ToString::to_string);
    let depth: Option<i16> = child.attribute("Depth").and_then(|d| d.parse().ok());
    let parent = child.attribute("Parent").or(parent_id);
    let sig = child.attribute("Id").map_or_else(
        || {
            let seed = format!("{}_{}", parent.unwrap_or(""), path.as_deref().unwrap_or(""));
            generate_deterministic_id("dir_search", &seed)
        },
        ToString::to_string,
    );

    section.add_symbol(Symbol::new("Signature", &sig));

    let rec = Record::with_fields(vec![
        FieldValue::String(sig),
        parent.map_or(FieldValue::Null, |p| FieldValue::String(p.to_string())),
        path.map_or(FieldValue::Null, FieldValue::String),
        depth.map_or(FieldValue::Null, FieldValue::Short),
    ]);
    tables
        .entry("DrLocator".to_string())
        .or_insert_with(|| IntermediateTable::new("DrLocator"))
        .push_record(rec);
}

/// Compiles `<FileSearch>` element into `FileSearch` and `Signature` tables.
pub(super) fn compile_file_search(
    child: &XmlNode,
    parent_id: Option<&str>,
    section: &mut IntermediateSection,
    tables: &mut HashMap<String, IntermediateTable>,
) {
    let name = child.attribute("Name").unwrap_or("target.exe");
    let sig = child.attribute("Id").map_or_else(
        || {
            let seed = format!("{}_{}", parent_id.unwrap_or(""), name);
            generate_deterministic_id("file_search", &seed)
        },
        ToString::to_string,
    );
    let min_ver = child.attribute("MinVersion").map(ToString::to_string);
    let max_ver = child.attribute("MaxVersion").map(ToString::to_string);

    section.add_symbol(Symbol::new("Signature", &sig));

    let rec = Record::with_fields(vec![
        FieldValue::String(sig.clone()),
        FieldValue::String(name.to_string()),
        min_ver.map_or(FieldValue::Null, FieldValue::String),
        max_ver.map_or(FieldValue::Null, FieldValue::String),
        FieldValue::Null,
        FieldValue::Null,
        FieldValue::Null,
        FieldValue::Null,
        FieldValue::Null,
    ]);
    tables
        .entry("Signature".to_string())
        .or_insert_with(|| IntermediateTable::new("Signature"))
        .push_record(rec);

    if let Some(prop) = parent_id {
        let app_rec = Record::with_fields(vec![
            FieldValue::String(prop.to_string()),
            FieldValue::String(sig),
        ]);
        tables
            .entry("AppSearch".to_string())
            .or_insert_with(|| IntermediateTable::new("AppSearch"))
            .push_record(app_rec);
    }
}

/// Compiles `<IniFileSearch>` element into `IniLocator` table.
pub(super) fn compile_ini_file_search(
    child: &XmlNode,
    parent_id: Option<&str>,
    section: &mut IntermediateSection,
    tables: &mut HashMap<String, IntermediateTable>,
) {
    let file = child
        .attribute("Name")
        .or_else(|| child.attribute("FileName"))
        .unwrap_or("app.ini");
    let section_name = child.attribute("Section").unwrap_or("Config");
    let key = child.attribute("Key").unwrap_or("Key");
    let sig = child.attribute("Id").map_or_else(
        || {
            let seed = format!(
                "{}_{}_{}_{}",
                parent_id.unwrap_or(""),
                file,
                section_name,
                key
            );
            generate_deterministic_id("ini_search", &seed)
        },
        ToString::to_string,
    );
    let field: Option<i16> = child.attribute("Field").and_then(|f| f.parse().ok());
    let type_num: Option<i16> = child.attribute("Type").and_then(|t| t.parse().ok());

    section.add_symbol(Symbol::new("Signature", &sig));

    let rec = Record::with_fields(vec![
        FieldValue::String(sig.clone()),
        FieldValue::String(file.to_string()),
        FieldValue::String(section_name.to_string()),
        FieldValue::String(key.to_string()),
        field.map_or(FieldValue::Null, FieldValue::Short),
        type_num.map_or(FieldValue::Null, FieldValue::Short),
    ]);
    tables
        .entry("IniLocator".to_string())
        .or_insert_with(|| IntermediateTable::new("IniLocator"))
        .push_record(rec);

    if let Some(prop) = parent_id {
        let app_rec = Record::with_fields(vec![
            FieldValue::String(prop.to_string()),
            FieldValue::String(sig),
        ]);
        tables
            .entry("AppSearch".to_string())
            .or_insert_with(|| IntermediateTable::new("AppSearch"))
            .push_record(app_rec);
    }
}

/// Compiles `<ComponentSearch>` element into `CompLocator` table.
pub(super) fn compile_component_search(
    child: &XmlNode,
    parent_id: Option<&str>,
    section: &mut IntermediateSection,
    tables: &mut HashMap<String, IntermediateTable>,
) {
    let guid = child
        .attribute("Guid")
        .unwrap_or("{00000000-0000-0000-0000-000000000000}");
    let sig = child.attribute("Id").map_or_else(
        || {
            let seed = format!("{}_{}", parent_id.unwrap_or(""), guid);
            generate_deterministic_id("comp_search", &seed)
        },
        ToString::to_string,
    );
    let type_num: Option<i16> = child.attribute("Type").and_then(|t| t.parse().ok());

    section.add_symbol(Symbol::new("Signature", &sig));

    let rec = Record::with_fields(vec![
        FieldValue::String(sig.clone()),
        FieldValue::String(guid.to_string()),
        type_num.map_or(FieldValue::Null, FieldValue::Short),
    ]);
    tables
        .entry("CompLocator".to_string())
        .or_insert_with(|| IntermediateTable::new("CompLocator"))
        .push_record(rec);

    if let Some(prop) = parent_id {
        let app_rec = Record::with_fields(vec![
            FieldValue::String(prop.to_string()),
            FieldValue::String(sig),
        ]);
        tables
            .entry("AppSearch".to_string())
            .or_insert_with(|| IntermediateTable::new("AppSearch"))
            .push_record(app_rec);
    }
}

/// Compiles a `<Launch>` or top-level `<Condition>` element into `LaunchCondition` table.
pub(super) fn compile_launch_condition(
    child: &XmlNode,
    tables: &mut HashMap<String, IntermediateTable>,
) {
    let message = child
        .attribute("Message")
        .unwrap_or("System requirements not met.");
    let trimmed_text = child.text.trim();
    let condition = child
        .attribute("Condition")
        .unwrap_or(if trimmed_text.is_empty() {
            "1"
        } else {
            trimmed_text
        });
    let rec = Record::with_fields(vec![
        FieldValue::String(condition.to_string()),
        FieldValue::String(message.to_string()),
    ]);
    tables
        .entry("LaunchCondition".to_string())
        .or_insert_with(|| IntermediateTable::new("LaunchCondition"))
        .push_record(rec);
}

/// Compiles a sequence table container (e.g. `<InstallExecuteSequence>`, `<InstallUISequence>`).
pub(super) fn compile_sequence_table(
    child: &XmlNode,
    section: &mut IntermediateSection,
    tables: &mut HashMap<String, IntermediateTable>,
) -> Result<()> {
    let table_name = &child.tag;
    for sub in &child.children {
        let action_name = sub
            .attribute("Action")
            .or_else(|| sub.attribute("Dialog"))
            .or_else(|| sub.attribute("Id"))
            .unwrap_or(&sub.tag);
        let cond = sub
            .attribute("Condition")
            .map(ToString::to_string)
            .or_else(|| {
                if sub.text.is_empty() {
                    None
                } else {
                    Some(sub.text.clone())
                }
            });
        let seq: Option<i16> = sub.attribute("Sequence").and_then(|s| s.parse().ok());
        section.add_reference(Reference::new("Action", action_name));

        let cond_field = cond.clone().map_or(FieldValue::Null, FieldValue::String);
        if let Some(after) = sub.attribute("After") {
            let rel_rec = Record::with_fields(vec![
                FieldValue::String(table_name.clone()),
                FieldValue::String(action_name.to_string()),
                FieldValue::String(after.to_string()),
                FieldValue::String("After".to_string()),
                cond_field,
            ]);
            tables
                .entry("_WixSequenceRelative".to_string())
                .or_insert_with(|| IntermediateTable::new("_WixSequenceRelative"))
                .push_record(rel_rec);
        } else if let Some(before) = sub.attribute("Before") {
            let rel_rec = Record::with_fields(vec![
                FieldValue::String(table_name.clone()),
                FieldValue::String(action_name.to_string()),
                FieldValue::String(before.to_string()),
                FieldValue::String("Before".to_string()),
                cond_field,
            ]);
            tables
                .entry("_WixSequenceRelative".to_string())
                .or_insert_with(|| IntermediateTable::new("_WixSequenceRelative"))
                .push_record(rel_rec);
        } else if let Some(on_exit) = sub.attribute("OnExit") {
            let rel_rec = Record::with_fields(vec![
                FieldValue::String(table_name.clone()),
                FieldValue::String(action_name.to_string()),
                FieldValue::String(on_exit.to_string()),
                FieldValue::String("OnExit".to_string()),
                cond_field,
            ]);
            tables
                .entry("_WixSequenceRelative".to_string())
                .or_insert_with(|| IntermediateTable::new("_WixSequenceRelative"))
                .push_record(rel_rec);
        }

        let seq_row = SequenceRow::new(action_name, cond, seq)?;
        tables
            .entry(table_name.clone())
            .or_insert_with(|| IntermediateTable::new(table_name))
            .push_record(seq_row.to_record());
    }
    Ok(())
}

/// Compiles patch-related authoring elements.
pub(super) fn compile_patch_element(
    child: &XmlNode,
    section: &mut IntermediateSection,
    tables: &mut HashMap<String, IntermediateTable>,
) {
    let id = child.attribute("Id").unwrap_or("Patch1");
    section.add_symbol(Symbol::new("Patch", id));
    let row = crate::database::tables::core::PatchPackageRow {
        patch_id: id.to_string(),
        media: 1,
    };
    tables
        .entry("PatchPackage".to_string())
        .or_insert_with(|| IntermediateTable::new("PatchPackage"))
        .push_record(row.to_record());
}

/// Compiles a `<UIRef>` element, adding a reference to the target UI dialog set.
pub(super) fn compile_ui_ref(
    child: &XmlNode,
    section: &mut IntermediateSection,
    _tables: &mut HashMap<String, IntermediateTable>,
) {
    let id = child.attribute("Id").unwrap_or("WixUI_InstallDir");
    section.add_reference(Reference::new("UI", id));
}

/// Compiles `<ControlEvent>` or `<Publish>` element into `ControlEvent` table.
pub(super) fn compile_control_event(
    child: &XmlNode,
    parent_id: Option<&str>,
    tables: &mut HashMap<String, IntermediateTable>,
) {
    let dialog = child.attribute("Dialog").unwrap_or("Dialog1");
    let control = parent_id
        .or_else(|| child.attribute("Control"))
        .unwrap_or("BtnNext");
    let event = child.attribute("Property").map_or_else(
        || child.attribute("Event").unwrap_or("NewDialog").to_string(),
        |target_prop| format!("[{target_prop}]"),
    );
    let arg = child
        .attribute("Argument")
        .or_else(|| child.attribute("Value"))
        .unwrap_or("");
    let cond = child
        .attribute("Condition")
        .map(ToString::to_string)
        .or_else(|| {
            if child.text.is_empty() {
                None
            } else {
                Some(child.text.clone())
            }
        });
    let order: Option<i16> = child.attribute("Order").and_then(|o| o.parse().ok());

    let rec = Record::with_fields(vec![
        FieldValue::String(dialog.to_string()),
        FieldValue::String(control.to_string()),
        FieldValue::String(event),
        FieldValue::String(arg.to_string()),
        cond.map_or(FieldValue::Null, FieldValue::String),
        order.map_or(FieldValue::Null, FieldValue::Short),
    ]);
    tables
        .entry("ControlEvent".to_string())
        .or_insert_with(|| IntermediateTable::new("ControlEvent"))
        .push_record(rec);
}

/// Compiles `<ControlCondition>` into `ControlCondition` table.
pub(super) fn compile_control_condition(
    child: &XmlNode,
    parent_id: Option<&str>,
    tables: &mut HashMap<String, IntermediateTable>,
) {
    let dialog = child.attribute("Dialog").unwrap_or("Dialog1");
    let control = parent_id
        .or_else(|| child.attribute("Control"))
        .unwrap_or("Control1");
    let action = child.attribute("Action").unwrap_or("enable");
    let cond = child.attribute("Condition").map_or_else(
        || {
            if child.text.is_empty() {
                "1"
            } else {
                &child.text
            }
        },
        |c| c,
    );

    let rec = Record::with_fields(vec![
        FieldValue::String(dialog.to_string()),
        FieldValue::String(control.to_string()),
        FieldValue::String(action.to_string()),
        FieldValue::String(cond.to_string()),
    ]);
    tables
        .entry("ControlCondition".to_string())
        .or_insert_with(|| IntermediateTable::new("ControlCondition"))
        .push_record(rec);
}

/// Compiles `<Subscribe>` element into `EventMapping` table.
pub(super) fn compile_subscribe(
    child: &XmlNode,
    parent_id: Option<&str>,
    tables: &mut HashMap<String, IntermediateTable>,
) {
    let dialog = child.attribute("Dialog").unwrap_or("Dialog1");
    let control = parent_id
        .or_else(|| child.attribute("Control"))
        .unwrap_or("Control1");
    let event = child.attribute("Event").unwrap_or("SetProgress");
    let attr = child.attribute("Attribute").unwrap_or("Progress");

    let rec = Record::with_fields(vec![
        FieldValue::String(dialog.to_string()),
        FieldValue::String(control.to_string()),
        FieldValue::String(event.to_string()),
        FieldValue::String(attr.to_string()),
    ]);
    tables
        .entry("EventMapping".to_string())
        .or_insert_with(|| IntermediateTable::new("EventMapping"))
        .push_record(rec);
}

/// Compiles a `<Binary>` element into `Binary` table.
pub(super) fn compile_binary(
    child: &XmlNode,
    section: &mut IntermediateSection,
    tables: &mut HashMap<String, IntermediateTable>,
) {
    let id = child.attribute("Id").unwrap_or("Binary1");
    section.add_symbol(Symbol::new("Binary", id));
    let mut rec = Record::new();
    rec.push(FieldValue::String(id.to_string()));
    rec.push(FieldValue::Stream(
        crate::database::tables::types::StringPoolId::new(1),
    ));
    let _ = tables
        .entry("Binary".to_string())
        .or_insert_with(|| IntermediateTable::new("Binary"));
    let _ = rec;

    if let Some(src) = child.attribute("SourceFile") {
        let mut wix_bin = Record::new();
        wix_bin.push(FieldValue::String(id.to_string()));
        wix_bin.push(FieldValue::String(src.to_string()));
        tables
            .entry("WixBinary".to_string())
            .or_insert_with(|| IntermediateTable::new("WixBinary"))
            .push_record(wix_bin);
    }
}

/// Compiles `<Billboard>` and `<BillboardAction>` elements.
pub(super) fn compile_billboard(
    child: &XmlNode,
    section: &mut IntermediateSection,
    tables: &mut HashMap<String, IntermediateTable>,
) {
    let id = child.attribute("Id").unwrap_or("BB1");
    let feat = child.attribute("Feature").unwrap_or("Main");
    let action = child.attribute("Action").unwrap_or("InstallFiles");
    let ordering: Option<i16> = child.attribute("Ordering").and_then(|o| o.parse().ok());

    section.add_symbol(Symbol::new("Billboard", id));

    let rec = Record::with_fields(vec![
        FieldValue::String(id.to_string()),
        FieldValue::String(feat.to_string()),
        FieldValue::String(action.to_string()),
        ordering.map_or(FieldValue::Null, FieldValue::Short),
    ]);
    tables
        .entry("Billboard".to_string())
        .or_insert_with(|| IntermediateTable::new("Billboard"))
        .push_record(rec);
}

/// Compiles a `<ProgressText>` element into `ActionText` table.
pub(super) fn compile_progress_text(
    child: &XmlNode,
    tables: &mut HashMap<String, IntermediateTable>,
) {
    let action = child.attribute("Action").unwrap_or("InstallFiles");
    let template = child.attribute("Template").map(ToString::to_string);
    let desc = if child.text.is_empty() {
        action.to_string()
    } else {
        child.text.clone()
    };
    let rec = Record::with_fields(vec![
        FieldValue::String(action.to_string()),
        FieldValue::String(desc),
        template.map_or(FieldValue::Null, FieldValue::String),
    ]);
    tables
        .entry("ActionText".to_string())
        .or_insert_with(|| IntermediateTable::new("ActionText"))
        .push_record(rec);
}

/// Compiles an `<Error>` element into `Error` table.
pub(super) fn compile_error(child: &XmlNode, tables: &mut HashMap<String, IntermediateTable>) {
    let id: i16 = child
        .attribute("Id")
        .and_then(|i| i.parse().ok())
        .unwrap_or(1001);
    let message = child.attribute("Message").map_or_else(
        || {
            if child.text.is_empty() {
                "Error occurred"
            } else {
                &child.text
            }
        },
        |m| m,
    );
    let rec = Record::with_fields(vec![
        FieldValue::Short(id),
        FieldValue::String(message.to_string()),
    ]);
    tables
        .entry("Error".to_string())
        .or_insert_with(|| IntermediateTable::new("Error"))
        .push_record(rec);
}

/// Compiles common `WiX` extension elements.
pub(super) fn compile_extension_element(
    child: &XmlNode,
    parent_id: Option<&str>,
    section: &mut IntermediateSection,
    tables: &mut HashMap<String, IntermediateTable>,
) {
    let id = child.attribute("Id").unwrap_or("ExtensionItem");
    section.add_symbol(Symbol::new("Extension", id));
    if let Some(parent) = parent_id {
        section.add_reference(Reference::new("Component", parent));
    }

    let tag = &child.tag;
    let mut fields = vec![
        FieldValue::String(id.to_string()),
        FieldValue::String(tag.clone()),
    ];
    if let Some(val) = child
        .attribute("Value")
        .or_else(|| child.attribute("Name"))
        .or_else(|| child.attribute("Target"))
    {
        fields.push(FieldValue::String(val.to_string()));
    }
    let rec = Record::with_fields(fields);
    tables
        .entry(format!("_{tag}"))
        .or_insert_with(|| IntermediateTable::new(format!("_{tag}")))
        .push_record(rec);
}

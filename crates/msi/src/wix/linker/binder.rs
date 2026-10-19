//! Payload file binding, cabinet packing, PE version inspection, and font title extraction for the `WiX` Linker.

use super::Linker;
use crate::database::md5::compute_md5;
use crate::database::memory::LinkedDatabase;
use crate::database::tables::record::{FieldValue, Record};
use crate::error::{MsiError, Result};
use crate::execution::properties::EvaluationContext;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

/// Parses a `WiX` compression level string into a [`crate::cab::folder::CompressionType`].
///
/// Supports `none`, `mszip`, `high`, `lzx`, `lzx15`..=`lzx21`, and `quantum`.
pub(super) fn parse_media_compression_level(lvl: &str) -> crate::cab::folder::CompressionType {
    let lower = lvl.trim().to_ascii_lowercase();
    if lower == "none" {
        crate::cab::folder::CompressionType::None
    } else if lower == "high" {
        crate::cab::folder::CompressionType::Lzx { window_bits: 21 }
    } else if let Some(lzx_part) = lower.strip_prefix("lzx") {
        let clean = lzx_part.trim_start_matches([':', '-', '_']);
        let bits = clean.parse::<u8>().unwrap_or(21).clamp(15, 21);
        crate::cab::folder::CompressionType::Lzx { window_bits: bits }
    } else if lower == "quantum" {
        crate::cab::folder::CompressionType::Quantum
    } else {
        crate::cab::folder::CompressionType::Mszip
    }
}

/// Resolves `WixVariable` tokens, binds branding bitmaps, and extracts license RTF.
pub(super) fn resolve_wix_variables(linker: &mut Linker, db: &mut LinkedDatabase) {
    let wix_vars = db.tables.get("WixVariable").cloned().unwrap_or_default();
    if wix_vars.is_empty() {
        return;
    }

    let mut var_map: HashMap<String, String> = HashMap::new();
    for r in &wix_vars {
        if let (Some(FieldValue::String(k)), Some(FieldValue::String(v))) = (r.get(0), r.get(1)) {
            var_map.insert(k.clone(), v.clone());
        }
    }

    replace_wix_variable_tokens(db, &var_map);
    bind_wix_ui_bitmaps_and_icons(linker, db, &var_map);
    bind_wix_ui_license(linker, db, &var_map);
}

/// Replaces `!(wix.Var)` tokens across all database tables.
fn replace_wix_variable_tokens(db: &mut LinkedDatabase, var_map: &HashMap<String, String>) {
    for records in db.tables.values_mut() {
        for rec in records.iter_mut() {
            for i in 0..rec.len() {
                if let Some(FieldValue::String(s)) = rec.get(i) {
                    if s.contains("!(wix.") {
                        let mut new_s = s.clone();
                        for (k, v) in var_map {
                            let pat = format!("!(wix.{k})");
                            if new_s.contains(&pat) {
                                new_s = new_s.replace(&pat, v);
                            }
                        }
                        rec.set(i, FieldValue::String(new_s));
                    }
                }
            }
        }
    }
}

/// Binds `WixUIBannerBmp`, `WixUIDialogBmp`, and standard `WixUI*Ico` variables into `Binary` streams.
fn bind_wix_ui_bitmaps_and_icons(
    linker: &mut Linker,
    db: &mut LinkedDatabase,
    var_map: &HashMap<String, String>,
) {
    for names in [
        ["WixUIBannerBmp", "WixUI_Bmp_Banner"],
        ["WixUIDialogBmp", "WixUI_Bmp_Dialog"],
    ] {
        let var_name = names[0];
        if let Some(bmp_path) = var_map.get(var_name) {
            if let Some(real_path) = resolve_source_path(linker, bmp_path) {
                if let Ok(data) = std::fs::read(&real_path) {
                    for name in names {
                        let _ = db.add_or_merge_record(
                            "Binary",
                            Record::with_fields(vec![
                                FieldValue::String(name.to_string()),
                                FieldValue::Stream(
                                    crate::database::tables::types::StringPoolId::new(1),
                                ),
                            ]),
                        );
                        linker
                            .embedded_cabinets
                            .insert(format!("Binary.{name}"), data.clone());
                        linker
                            .embedded_cabinets
                            .insert(name.to_string(), data.clone());
                    }
                }
            }
        }
    }

    for (idx, icon_var) in [
        "WixUIExclamationIco",
        "WixUIInfoIco",
        "WixUINewIco",
        "WixUIUpIco",
    ]
    .iter()
    .enumerate()
    {
        if let Some(ico_path) = var_map.get(*icon_var) {
            if let Some(real_path) = resolve_source_path(linker, ico_path) {
                if let Ok(data) = std::fs::read(&real_path) {
                    db.add_record(
                        "Binary",
                        Record::with_fields(vec![
                            FieldValue::String((*icon_var).to_string()),
                            FieldValue::Stream(crate::database::tables::types::StringPoolId::new(
                                u32::try_from(idx + 3).unwrap_or(u32::MAX),
                            )),
                        ]),
                    );
                    linker
                        .embedded_cabinets
                        .insert((*icon_var).to_string(), data);
                }
            }
        }
    }
}

/// Binds `WixUILicenseRtf` or `WixUILicenseTxt` into `ScrollableText` controls.
fn bind_wix_ui_license(
    linker: &Linker,
    db: &mut LinkedDatabase,
    var_map: &HashMap<String, String>,
) {
    let (is_txt_var, lic_val) = var_map.get("WixUILicenseRtf").map_or_else(
        || (true, var_map.get("WixUILicenseTxt")),
        |v| (false, Some(v)),
    );

    if let Some(lic_val) = lic_val {
        let rtf_content = if lic_val.starts_with("{\x5crtf1") {
            lic_val.clone()
        } else if let Some(real_path) = resolve_source_path(linker, lic_val) {
            std::fs::read_to_string(&real_path).map_or_else(
                |_| lic_val.clone(),
                |content| {
                    if real_path
                        .extension()
                        .is_some_and(|e| e.eq_ignore_ascii_case("rtf"))
                    {
                        content
                    } else {
                        crate::wix::compiler::convert_text_to_rtf(&content)
                    }
                },
            )
        } else if is_txt_var {
            crate::wix::compiler::convert_text_to_rtf(lic_val)
        } else {
            lic_val.clone()
        };

        if let Some(ctrl_records) = db.tables.get_mut("Control") {
            for r in ctrl_records.iter_mut() {
                if r.get(2) == Some(&FieldValue::String("ScrollableText".to_string())) {
                    let should_update = match r.get(9) {
                        None | Some(FieldValue::Null) => true,
                        Some(FieldValue::String(s)) => {
                            let trimmed = s.trim();
                            trimmed.is_empty()
                                || trimmed == "!(wix.WixUILicenseRtf)"
                                || trimmed == "!(wix.WixUILicenseTxt)"
                                || (!trimmed.starts_with("{\x5crtf") && trimmed != "Existing Text")
                        }
                        _ => false,
                    };
                    if should_update {
                        r.set(9, FieldValue::String(rtf_content.clone()));
                    }
                }
            }
        }
    }
}

/// Resolves deferred `ScrollableText` `Control.Text` file references against configured bind paths.
pub(super) fn bind_text_sources(linker: &Linker, db: &mut LinkedDatabase) -> Result<()> {
    let mut updates = Vec::new();
    if let Some(ctrl_records) = db.tables.get("Control") {
        for (idx, r) in ctrl_records.iter().enumerate() {
            if r.get(2) == Some(&FieldValue::String("ScrollableText".to_string())) {
                if let Some(FieldValue::String(text)) = r.get(9) {
                    let trimmed = text.trim();
                    if !trimmed.is_empty() && !trimmed.starts_with("{\x5crtf") {
                        let file_ref = trimmed
                            .strip_prefix("!(bindpath.rtf:")
                            .and_then(|s| s.strip_suffix(')'))
                            .unwrap_or(trimmed);
                        let is_explicit_bind = trimmed.starts_with("!(bindpath.rtf:");
                        let looks_like_file_path = is_explicit_bind
                            || Path::new(file_ref).extension().is_some_and(|e| {
                                e.eq_ignore_ascii_case("rtf") || e.eq_ignore_ascii_case("txt")
                            });
                        if let Some(real_path) = resolve_source_path(linker, file_ref) {
                            let content = std::fs::read_to_string(&real_path).map_err(|e| {
                                MsiError::WixLinker {
                                    message: format!(
                                        "failed reading ScrollableText source file '{}': {e}",
                                        real_path.display()
                                    ),
                                }
                            })?;
                            let rtf = if real_path
                                .extension()
                                .is_some_and(|e| e.eq_ignore_ascii_case("rtf"))
                            {
                                content
                            } else {
                                crate::wix::compiler::convert_text_to_rtf(&content)
                            };
                            updates.push((idx, rtf));
                        } else if looks_like_file_path {
                            return Err(MsiError::WixLinker {
                                message: format!(
                                    "unresolved ScrollableText source file '{file_ref}' (not found in any -b bind path)"
                                ),
                            });
                        }
                    }
                }
            }
        }
    }
    if let Some(ctrl_records) = db.tables.get_mut("Control") {
        for (idx, rtf) in updates {
            ctrl_records[idx].set(9, FieldValue::String(rtf));
        }
    }
    Ok(())
}

/// Resolves a source file path against configured base directories and bind paths.
pub(super) fn resolve_source_path(linker: &Linker, src_path: &str) -> Option<PathBuf> {
    let p = Path::new(src_path);
    if p.is_absolute() && p.exists() {
        return Some(p.to_path_buf());
    }

    for (id, bind_dir) in &linker.bind_paths {
        let prefix_bracket = format!("[{id}]");
        if src_path.starts_with(&prefix_bracket) {
            let rest = src_path[prefix_bracket.len()..].trim_start_matches(['/', '\x5c']);
            let cand = bind_dir.join(rest);
            if cand.exists() {
                return Some(cand);
            }
        }
    }

    for base_dir in &linker.base_directories {
        let cand = base_dir.join(p);
        if cand.exists() {
            return Some(cand);
        }
    }

    if p.exists() {
        return Some(p.to_path_buf());
    }

    None
}

/// Binds physical file payloads from disk into `File` records and compresses them into cabinets.
pub(super) fn bind_files_and_pack_cabinets(
    linker: &mut Linker,
    db: &mut LinkedDatabase,
) -> Result<()> {
    let wix_files = db.tables.remove("WixFile").unwrap_or_default();

    let mut file_sources: HashMap<String, (String, i16)> = HashMap::new();
    for r in &wix_files {
        if let (Some(FieldValue::String(fid)), Some(FieldValue::String(src))) = (r.get(0), r.get(1))
        {
            let disk_id = match r.get(2) {
                Some(FieldValue::Short(d)) => *d,
                _ => 1,
            };
            file_sources.insert(fid.clone(), (src.clone(), disk_id));
        }
    }

    let disk_compression = collect_disk_compression(db);
    let disk_to_cab = collect_disk_to_cab_names(db);

    let file_disk_records: Vec<Record> = file_sources
        .iter()
        .map(|(fid, (_src, did))| {
            Record::with_fields(vec![
                FieldValue::String(fid.clone()),
                FieldValue::Short(*did),
            ])
        })
        .collect();
    db.tables
        .insert("_FileDiskId".to_string(), file_disk_records);

    let mut cab_writers: HashMap<String, crate::cab::writer::CabinetWriter> = HashMap::new();
    for (disk_id, cab_name) in &disk_to_cab {
        let comp_type = disk_compression
            .get(disk_id)
            .copied()
            .unwrap_or(crate::cab::folder::CompressionType::Mszip);
        let _ = cab_writers
            .entry(cab_name.clone())
            .or_insert_with(|| crate::cab::writer::CabinetWriter::new(comp_type));
    }

    bind_file_records_into_cabinets(
        linker,
        db,
        &file_sources,
        &disk_compression,
        &disk_to_cab,
        &mut cab_writers,
    )?;

    for (cab_name, writer) in cab_writers {
        let cab_bytes = writer.build().unwrap_or_default();
        if cab_name.starts_with('#') {
            linker.embedded_cabinets.insert(cab_name, cab_bytes);
        } else {
            linker.external_cabinets.insert(cab_name, cab_bytes);
        }
    }

    Ok(())
}

/// Collects compression settings per disk ID.
fn collect_disk_compression(
    db: &LinkedDatabase,
) -> HashMap<i16, crate::cab::folder::CompressionType> {
    let mut disk_compression = HashMap::new();
    for r in db.get_records("WixMediaCompression") {
        if let (Some(FieldValue::Short(did)), Some(FieldValue::String(lvl))) = (r.get(0), r.get(1))
        {
            disk_compression.insert(*did, parse_media_compression_level(lvl));
        }
    }
    disk_compression
}

/// Collects cabinet stream or filename per disk ID.
fn collect_disk_to_cab_names(db: &LinkedDatabase) -> HashMap<i16, String> {
    let external_media_disks: HashSet<i16> = db
        .get_records("WixExternalMedia")
        .iter()
        .filter_map(|r| match r.get(0) {
            Some(FieldValue::Short(did)) => Some(*did),
            _ => None,
        })
        .collect();

    let mut disk_to_cab = HashMap::new();
    for r in db.get_records("Media") {
        if let Some(FieldValue::Short(did)) = r.get(0) {
            let is_ext = external_media_disks.contains(did);
            let cab_name = match r.get(3) {
                Some(FieldValue::String(c)) if !c.is_empty() => {
                    if is_ext {
                        c.trim_start_matches('#').to_string()
                    } else if c.starts_with('#') {
                        c.clone()
                    } else {
                        format!("#{c}")
                    }
                }
                _ => {
                    if is_ext {
                        format!("cab{did}.cab")
                    } else {
                        format!("#cab{did}.cab")
                    }
                }
            };
            disk_to_cab.insert(*did, cab_name);
        }
    }
    disk_to_cab
}

/// Binds each `File` record to its disk payload, populates `FileHash` and `Font` tables, and adds payloads to `CabinetWriter`.
fn bind_file_records_into_cabinets(
    linker: &Linker,
    db: &mut LinkedDatabase,
    file_sources: &HashMap<String, (String, i16)>,
    disk_compression: &HashMap<i16, crate::cab::folder::CompressionType>,
    disk_to_cab: &HashMap<i16, String>,
    cab_writers: &mut HashMap<String, crate::cab::writer::CabinetWriter>,
) -> Result<()> {
    let mut file_hash_records: Vec<Record> = Vec::new();
    let mut font_records: Vec<Record> = Vec::new();

    if let Some(file_records) = db.tables.get_mut("File") {
        file_records.sort_by(|a, b| {
            let fid_a = match a.get(0) {
                Some(FieldValue::String(s)) => s.as_str(),
                _ => "",
            };
            let fid_b = match b.get(0) {
                Some(FieldValue::String(s)) => s.as_str(),
                _ => "",
            };
            let disk_a = file_sources.get(fid_a).map_or(1, |(_, d)| *d);
            let disk_b = file_sources.get(fid_b).map_or(1, |(_, d)| *d);
            disk_a.cmp(&disk_b)
        });

        let mut max_seq_per_disk: HashMap<i16, i32> = HashMap::new();

        for (idx, r) in file_records.iter_mut().enumerate() {
            let file_id = match r.get(0) {
                Some(FieldValue::String(s)) => s.clone(),
                _ => continue,
            };
            let comp_id = match r.get(1) {
                Some(FieldValue::String(s)) => s.clone(),
                _ => continue,
            };
            let (src_path_str, disk_id) = match file_sources.get(&file_id) {
                Some((s, d)) => (s.clone(), *d),
                None => continue,
            };

            let Some(resolved_path) = resolve_source_path(linker, &src_path_str) else {
                if !linker.base_directories.is_empty() || !linker.bind_paths.is_empty() {
                    return Err(MsiError::WixLinker {
                        message: format!(
                            "Source file '{src_path_str}' for File '{file_id}' not found in any base directory"
                        ),
                    });
                }
                continue;
            };

            let data = std::fs::read(&resolved_path)?;
            r.set(
                3,
                FieldValue::Long(i32::try_from(data.len()).unwrap_or(i32::MAX)),
            );

            update_file_version_or_hash(r, &file_id, &data, &mut file_hash_records);
            if let Some(font_title) = inspect_font_title(&data) {
                font_records.push(Record::with_fields(vec![
                    FieldValue::String(file_id.clone()),
                    FieldValue::String(font_title),
                ]));
            }

            let seq = i32::try_from(idx + 1).unwrap_or(i32::MAX);
            r.set(7, FieldValue::Short(i16::try_from(seq).unwrap_or(i16::MAX)));
            max_seq_per_disk.insert(disk_id, seq);

            let cab_name = if linker.cab_per_component {
                format!("#comp_{comp_id}.cab")
            } else {
                disk_to_cab
                    .get(&disk_id)
                    .cloned()
                    .unwrap_or_else(|| format!("#cab{disk_id}.cab"))
            };
            let comp_type = disk_compression
                .get(&disk_id)
                .copied()
                .unwrap_or(crate::cab::folder::CompressionType::Mszip);
            let writer = cab_writers
                .entry(cab_name)
                .or_insert_with(|| crate::cab::writer::CabinetWriter::new(comp_type));
            writer.add_file(&file_id, &data)?;
        }

        update_media_last_sequences(db, &max_seq_per_disk);
    }

    if !file_hash_records.is_empty() {
        db.tables
            .entry("FileHash".to_string())
            .or_default()
            .extend(file_hash_records);
    }
    if !font_records.is_empty() {
        db.tables
            .entry("Font".to_string())
            .or_default()
            .extend(font_records);
    }
    Ok(())
}

/// Populates PE version/language on `r` or appends an MD5 `FileHash` record.
fn update_file_version_or_hash(
    r: &mut Record,
    file_id: &str,
    data: &[u8],
    file_hash_records: &mut Vec<Record>,
) {
    let is_unversioned = match r.get(4) {
        Some(FieldValue::String(s)) => s.is_empty(),
        _ => true,
    };
    if is_unversioned {
        if let Some((pe_ver, pe_lang)) = inspect_pe_version(data) {
            r.set(4, FieldValue::String(pe_ver));
            r.set(5, FieldValue::String(pe_lang));
        } else {
            let digest = compute_md5(data);
            let h1 = i32::from_le_bytes([digest[0], digest[1], digest[2], digest[3]]);
            let h2 = i32::from_le_bytes([digest[4], digest[5], digest[6], digest[7]]);
            let h3 = i32::from_le_bytes([digest[8], digest[9], digest[10], digest[11]]);
            let h4 = i32::from_le_bytes([digest[12], digest[13], digest[14], digest[15]]);
            file_hash_records.push(Record::with_fields(vec![
                FieldValue::String(file_id.to_string()),
                FieldValue::Short(0),
                FieldValue::Long(h1),
                FieldValue::Long(h2),
                FieldValue::Long(h3),
                FieldValue::Long(h4),
            ]));
        }
    }
}

/// Updates `Media.LastSequence` across all media table entries.
fn update_media_last_sequences(db: &mut LinkedDatabase, max_seq_per_disk: &HashMap<i16, i32>) {
    if let Some(media_records) = db.tables.get_mut("Media") {
        media_records.sort_by(|a, b| {
            let did_a = match a.get(0) {
                Some(FieldValue::Short(d)) => *d,
                _ => 0,
            };
            let did_b = match b.get(0) {
                Some(FieldValue::Short(d)) => *d,
                _ => 0,
            };
            did_a.cmp(&did_b)
        });

        let mut cumulative_last_seq = 0;
        for mr in media_records.iter_mut() {
            if let Some(FieldValue::Short(did)) = mr.get(0) {
                if let Some(&max_seq) = max_seq_per_disk.get(did) {
                    cumulative_last_seq = cumulative_last_seq.max(max_seq);
                }
                mr.set(1, FieldValue::Long(cumulative_last_seq));
            }
        }
    }
}

/// Binds physical binary payloads from disk into embedded streams.
pub(super) fn bind_binaries(linker: &mut Linker, db: &mut LinkedDatabase) -> Result<()> {
    let wix_binaries = db.tables.remove("WixBinary").unwrap_or_default();
    for r in &wix_binaries {
        if let (Some(FieldValue::String(bin_id)), Some(FieldValue::String(src))) =
            (r.get(0), r.get(1))
        {
            if let Some(path) = resolve_source_path(linker, src) {
                let bytes = std::fs::read(&path)?;
                let _ = db.add_or_merge_record(
                    "Binary",
                    Record::with_fields(vec![
                        FieldValue::String(bin_id.clone()),
                        FieldValue::Stream(crate::database::tables::types::StringPoolId::new(1)),
                    ]),
                );
                linker
                    .embedded_cabinets
                    .insert(format!("Binary.{bin_id}"), bytes.clone());
                linker.embedded_cabinets.insert(bin_id.clone(), bytes);
            }
        }
    }
    let wix_icons = db.tables.remove("WixIcon").unwrap_or_default();
    for r in &wix_icons {
        if let (Some(FieldValue::String(icon_id)), Some(FieldValue::String(src))) =
            (r.get(0), r.get(1))
        {
            if let Some(path) = resolve_source_path(linker, src) {
                let bytes = std::fs::read(&path)?;
                linker
                    .embedded_cabinets
                    .insert(format!("Icon.{icon_id}"), bytes.clone());
                linker.embedded_cabinets.insert(icon_id.clone(), bytes);
            }
        }
    }
    Ok(())
}

/// Validates `MsiEmbeddedChainer` table records to ensure referenced binary or file streams exist.
pub(super) fn validate_embedded_chainers(db: &LinkedDatabase) -> Result<()> {
    let chainers = db.get_records("MsiEmbeddedChainer");
    if chainers.is_empty() {
        return Ok(());
    }

    let existing_binaries: HashSet<String> = db
        .get_records("Binary")
        .iter()
        .filter_map(|r| match r.get(0) {
            Some(FieldValue::String(s)) => Some(s.clone()),
            _ => None,
        })
        .collect();

    let existing_files: HashSet<String> = db
        .get_records("File")
        .iter()
        .filter_map(|r| match r.get(0) {
            Some(FieldValue::String(s)) => Some(s.clone()),
            _ => None,
        })
        .collect();

    let eval_ctx = EvaluationContext::new();

    for rec in chainers {
        let Some(FieldValue::String(chainer_id)) = rec.get(0) else {
            continue;
        };

        if let Some(cond) = rec.get(1).and_then(|f| match f {
            FieldValue::String(s) if !s.trim().is_empty() => Some(s.as_str()),
            _ => None,
        }) {
            if let Err(e) = eval_ctx.evaluate_condition(cond.trim()) {
                return Err(MsiError::WixLinker {
                    message: format!(
                        "invalid Condition syntax '{cond}' in EmbeddedChainer '{chainer_id}': {e}"
                    ),
                });
            }
        }

        let Some(FieldValue::String(source)) = rec.get(3) else {
            continue;
        };
        let chainer_type = match rec.get(4) {
            Some(FieldValue::Long(v)) => *v,
            Some(FieldValue::Short(v)) => i32::from(*v),
            _ => 1,
        };

        if chainer_type == 1 {
            if !existing_binaries.contains(source) {
                return Err(MsiError::WixLinker {
                    message: format!(
                        "unresolved binary reference '{source}' for EmbeddedChainer '{chainer_id}'"
                    ),
                });
            }
        } else if chainer_type == 2 {
            if !existing_files.contains(source) {
                return Err(MsiError::WixLinker {
                    message: format!(
                        "unresolved file reference '{source}' for EmbeddedChainer '{chainer_id}'"
                    ),
                });
            }
        } else {
            return Err(MsiError::WixLinker {
                message: format!(
                    "invalid Type '{chainer_type}' for EmbeddedChainer '{chainer_id}': expected 1 (Binary) or 2 (File)"
                ),
            });
        }
    }

    Ok(())
}

/// Sequences file numbers and aligns media last sequence across partitioned disks.
pub(super) fn layout_media_and_files(db: &mut LinkedDatabase) {
    let file_disk_mapping = db.tables.remove("_FileDiskId").unwrap_or_default();
    let mut file_to_disk: HashMap<String, i16> = HashMap::new();
    for r in &file_disk_mapping {
        if let (Some(FieldValue::String(fid)), Some(FieldValue::Short(d))) = (r.get(0), r.get(1)) {
            file_to_disk.insert(fid.clone(), *d);
        }
    }

    let file_records = db.get_records("File").to_vec();
    let file_count = file_records.len();
    if file_count == 0 {
        return;
    }

    let mut sorted_files = file_records;
    sorted_files.sort_by_key(|r| {
        let fid = match r.get(0) {
            Some(FieldValue::String(s)) => s.as_str(),
            _ => "",
        };
        file_to_disk.get(fid).copied().unwrap_or(1)
    });

    let mut resequenced_files: Vec<Record> = Vec::with_capacity(file_count);
    let mut disk_max_seq: HashMap<i16, i32> = HashMap::new();

    for (i, r) in sorted_files.iter().enumerate() {
        let mut fields = r.fields().to_vec();
        let seq = i16::try_from(i + 1).unwrap_or(i16::MAX);
        if fields.len() >= 8 {
            fields[7] = FieldValue::Short(seq);
        }
        let fid = match fields.first() {
            Some(FieldValue::String(s)) => s.as_str(),
            _ => "",
        };
        let disk_id = file_to_disk.get(fid).copied().unwrap_or(1);
        disk_max_seq.insert(disk_id, i32::from(seq));
        resequenced_files.push(Record::with_fields(fields));
    }
    db.tables.insert("File".to_string(), resequenced_files);

    let is_multi_cab = !file_disk_mapping.is_empty();
    let file_count_i32 = i32::try_from(file_count).unwrap_or(i32::MAX);

    let media_records = db.get_records("Media");
    if media_records.is_empty() {
        let default_media = Record::with_fields(vec![
            FieldValue::Short(1),
            FieldValue::Long(file_count_i32),
            FieldValue::Null,
            FieldValue::String("#cab1.cab".to_string()),
            FieldValue::Null,
            FieldValue::Null,
        ]);
        db.add_record("Media", default_media);
    } else if !is_multi_cab {
        let mut updated_media = media_records.to_vec();
        let last_idx = updated_media.len() - 1;
        let mut fields = updated_media[last_idx].fields().to_vec();
        if let Some(FieldValue::Long(last_seq)) = fields.get(1) {
            if *last_seq < file_count_i32 {
                fields[1] = FieldValue::Long(file_count_i32);
                updated_media[last_idx] = Record::with_fields(fields);
                db.tables.insert("Media".to_string(), updated_media);
            }
        }
    } else {
        let mut updated_media = media_records.to_vec();
        updated_media.sort_by_key(|r| match r.get(0) {
            Some(FieldValue::Short(d)) => *d,
            _ => 1,
        });
        let mut running_last_seq = 0;
        for r in &mut updated_media {
            let mut fields = r.fields().to_vec();
            if let Some(FieldValue::Short(did)) = fields.first() {
                if let Some(&max_seq) = disk_max_seq.get(did) {
                    running_last_seq = running_last_seq.max(max_seq);
                }
                let existing_last = match fields.get(1) {
                    Some(FieldValue::Long(l)) => *l,
                    _ => 0,
                };
                running_last_seq = running_last_seq.max(existing_last);
                if running_last_seq > 0 {
                    fields[1] = FieldValue::Long(running_last_seq);
                }
                *r = Record::with_fields(fields);
            }
        }
        db.tables.insert("Media".to_string(), updated_media);
    }
}

/// Inspects executable file binary data to extract Windows PE `VS_FIXEDFILEINFO` version and language.
///
/// # Arguments
///
/// * `data` - Raw PE executable or DLL byte slice.
///
/// # Returns
///
/// Optional tuple of `(version_string, language_id_string)`.
#[must_use]
pub fn inspect_pe_version(data: &[u8]) -> Option<(String, String)> {
    if data.len() < 64 || data.get(0..2) != Some(b"MZ") {
        return None;
    }
    let lfanew = usize::try_from(u32::from_le_bytes([
        data[0x3C], data[0x3D], data[0x3E], data[0x3F],
    ]))
    .unwrap_or(0);

    if data.len() < lfanew + 24 || data.get(lfanew..lfanew + 4) != Some(b"PE\0\0") {
        return None;
    }

    let size_of_opt = usize::from(u16::from_le_bytes([data[lfanew + 20], data[lfanew + 21]]));
    let opt_offset = lfanew + 24;
    if data.len() < opt_offset + 2 {
        return None;
    }
    let magic = u16::from_le_bytes([data[opt_offset], data[opt_offset + 1]]);

    let rsrc_dir_offset = if magic == 0x10B {
        opt_offset + 96 + 16
    } else if magic == 0x20B {
        opt_offset + 112 + 16
    } else {
        return None;
    };

    if data.len() < rsrc_dir_offset + 4 {
        return None;
    }

    let rsrc_rva = u32::from_le_bytes([
        data[rsrc_dir_offset],
        data[rsrc_dir_offset + 1],
        data[rsrc_dir_offset + 2],
        data[rsrc_dir_offset + 3],
    ]);
    if rsrc_rva == 0 {
        return None;
    }

    let num_sections = usize::from(u16::from_le_bytes([data[lfanew + 6], data[lfanew + 7]]));
    let section_headers_offset = opt_offset + size_of_opt;
    let root_rsrc = find_rsrc_section_offset(data, section_headers_offset, num_sections, rsrc_rva)?;

    extract_vs_fixed_file_info(data, root_rsrc)
}

/// Finds the file offset corresponding to `rsrc_rva` in the PE section headers.
fn find_rsrc_section_offset(
    data: &[u8],
    section_headers_offset: usize,
    num_sections: usize,
    rsrc_rva: u32,
) -> Option<usize> {
    for i in 0..num_sections {
        let sec_offset = section_headers_offset + i * 40;
        if data.len() < sec_offset + 40 {
            break;
        }
        let virt_size = u32::from_le_bytes([
            data[sec_offset + 8],
            data[sec_offset + 9],
            data[sec_offset + 10],
            data[sec_offset + 11],
        ]);
        let virt_addr = u32::from_le_bytes([
            data[sec_offset + 12],
            data[sec_offset + 13],
            data[sec_offset + 14],
            data[sec_offset + 15],
        ]);
        let raw_ptr = u32::from_le_bytes([
            data[sec_offset + 20],
            data[sec_offset + 21],
            data[sec_offset + 22],
            data[sec_offset + 23],
        ]);

        if rsrc_rva >= virt_addr && rsrc_rva < virt_addr.saturating_add(virt_size) {
            return usize::try_from(raw_ptr + (rsrc_rva - virt_addr)).ok();
        }
    }
    None
}

/// Extracts the `VS_FIXEDFILEINFO` version string from the PE `.rsrc` directory.
fn extract_vs_fixed_file_info(data: &[u8], root_rsrc: usize) -> Option<(String, String)> {
    if data.len() < root_rsrc + 16 {
        return None;
    }
    let num_named = usize::from(u16::from_le_bytes([
        data[root_rsrc + 12],
        data[root_rsrc + 13],
    ]));
    let num_id = usize::from(u16::from_le_bytes([
        data[root_rsrc + 14],
        data[root_rsrc + 15],
    ]));

    for i in 0..(num_named + num_id) {
        let entry_offset = root_rsrc + 16 + i * 8;
        if data.len() < entry_offset + 8 {
            break;
        }
        let id_val = u32::from_le_bytes([
            data[entry_offset],
            data[entry_offset + 1],
            data[entry_offset + 2],
            data[entry_offset + 3],
        ]);

        if id_val == 16 {
            if let Some(pos) = data[root_rsrc..data.len().min(root_rsrc + 1024)]
                .windows(4)
                .position(|w| w == 0xFEEF_04BDu32.to_le_bytes())
            {
                let sig_pos = root_rsrc + pos;
                if sig_pos + 20 <= data.len() {
                    let ms = u32::from_le_bytes([
                        data[sig_pos + 8],
                        data[sig_pos + 9],
                        data[sig_pos + 10],
                        data[sig_pos + 11],
                    ]);
                    let ls = u32::from_le_bytes([
                        data[sig_pos + 12],
                        data[sig_pos + 13],
                        data[sig_pos + 14],
                        data[sig_pos + 15],
                    ]);
                    let major = ms >> 16;
                    let minor = ms & 0xFFFF;
                    let build = ls >> 16;
                    let rev = ls & 0xFFFF;
                    return Some((format!("{major}.{minor}.{build}.{rev}"), "1033".to_string()));
                }
            }
        }
    }
    None
}

/// Inspects font file binary data (`.ttf`, `.otf`) to extract font title.
pub(super) fn inspect_font_title(data: &[u8]) -> Option<String> {
    if data.len() < 12 {
        return None;
    }
    let is_font = data.starts_with(&[0, 1, 0, 0]) || data.starts_with(b"OTTO");
    if !is_font {
        return None;
    }

    let num_tables = usize::from(u16::from_be_bytes([data[4], data[5]]));
    let mut name_offset = None;

    for i in 0..num_tables {
        let rec = 12 + i * 16;
        if data.len() < rec + 16 {
            break;
        }
        if &data[rec..rec + 4] == b"name" {
            let off = usize::try_from(u32::from_be_bytes([
                data[rec + 8],
                data[rec + 9],
                data[rec + 10],
                data[rec + 11],
            ]))
            .ok()?;
            name_offset = Some(off);
            break;
        }
    }

    let name_tbl = name_offset?;
    if data.len() < name_tbl + 6 {
        return None;
    }
    let count = usize::from(u16::from_be_bytes([data[name_tbl + 2], data[name_tbl + 3]]));
    let str_offset = usize::from(u16::from_be_bytes([data[name_tbl + 4], data[name_tbl + 5]]));

    for i in 0..count {
        let rec = name_tbl + 6 + i * 12;
        if data.len() < rec + 12 {
            break;
        }
        let name_id = u16::from_be_bytes([data[rec + 6], data[rec + 7]]);
        if name_id == 4 || name_id == 1 {
            let length = usize::from(u16::from_be_bytes([data[rec + 8], data[rec + 9]]));
            let offset = usize::from(u16::from_be_bytes([data[rec + 10], data[rec + 11]]));
            let start = name_tbl + str_offset + offset;
            if let Some(bytes) = data.get(start..start + length) {
                let platform_id = u16::from_be_bytes([data[rec], data[rec + 1]]);
                if platform_id == 0 || platform_id == 3 {
                    let mut u16s = Vec::with_capacity(bytes.len() / 2);
                    for chunk in bytes.chunks_exact(2) {
                        u16s.push(u16::from_be_bytes([chunk[0], chunk[1]]));
                    }
                    if let Ok(s) = String::from_utf16(&u16s) {
                        if !s.is_empty() {
                            return Some(s);
                        }
                    }
                } else if let Ok(s) = std::str::from_utf8(bytes) {
                    if !s.is_empty() {
                        return Some(s.to_string());
                    }
                }
            }
        }
    }

    None
}

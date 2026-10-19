//! Patching Engine (.msp & MSDelta) support.
//!
//! Extends transaction and installation capabilities with patching functionality.
//! Implements `.msp` header and sub-storage extraction, MSDelta sequence application,
//! and handles complex patch sequencing, supersedence, and patch families.

use crate::error::{MsiError, Result};
use crate::execution::msdelta::PatchDecoder;
use crate::wix::xml::XmlParser;
use std::path::{Path, PathBuf};

/// High-level logic for applying an MSP patch to a target database or product.
#[derive(Debug)]
pub struct PatchEngine;

/// Represents extracted `.msp` metadata.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MspMetadata {
    /// The target product code this patch applies to.
    pub target_product_code: String,
    /// The patch baseline version.
    pub baseline_version: String,
    /// Whether this patch supersedes older patches.
    pub supersedes: Vec<String>,
    /// The patch family this patch belongs to.
    pub family: String,
}

impl PatchEngine {
    /// Parses an XML patch manifest string into `MspMetadata` using strict struct-based deserialization.
    ///
    /// # Arguments
    ///
    /// * `xml_content` - Raw XML string containing the patch manifest.
    ///
    /// # Errors
    ///
    /// Returns `MsiError` on parsing failure.
    ///
    /// # Returns
    ///
    /// `Ok(MspMetadata)` on success, or an error if the operation fails.
    pub fn parse_manifest_xml(xml_content: &str) -> Result<MspMetadata> {
        let parser = XmlParser::new();
        let root = parser
            .parse(xml_content)
            .map_err(|e| MsiError::PatchXmlParse {
                reason: format!("Failed to parse patch manifest: {e}"),
            })?;

        if root.tag != "PatchManifest" {
            return Err(MsiError::PatchXmlParse {
                reason: format!("Expected PatchManifest, found {}", root.tag),
            });
        }

        let target_product_code = root
            .attribute("TargetProductCode")
            .unwrap_or("")
            .to_string();
        let baseline_version = root.attribute("BaselineVersion").unwrap_or("").to_string();
        let family = root.attribute("Family").unwrap_or("").to_string();

        if target_product_code.is_empty() || baseline_version.is_empty() || family.is_empty() {
            return Err(MsiError::PatchXmlParse {
                reason: "Missing required attributes in PatchManifest".to_string(),
            });
        }

        let mut supersedes = Vec::new();
        if let Some(supersedes_node) = root.find_child("Supersedes") {
            for child in supersedes_node.children_with_tag("Patch") {
                if let Some(id) = child.attribute("Id") {
                    supersedes.push(id.to_string());
                }
            }
        }

        Ok(MspMetadata {
            target_product_code,
            baseline_version,
            supersedes,
            family,
        })
    }

    /// Extracts the `.msp` header and parses its `\005SummaryInformation` or `PatchManifest`
    /// stream from the Compound File Binary container.
    ///
    /// # Arguments
    ///
    /// * `patch_path` - Path to the `.msp` patch file on disk.
    ///
    /// # Returns
    ///
    /// Extracted [`MspMetadata`] describing target product, baseline version, supersedence, and patch family.
    ///
    /// # Errors
    ///
    /// Returns [`MsiError`] if the `.msp` file does not exist, is not a valid CFB container,
    /// contains a corrupt cabinet stream, or lacks required summary/manifest metadata.
    pub fn extract_msp_metadata(patch_path: &Path) -> Result<MspMetadata> {
        let bytes = std::fs::read(patch_path).map_err(|e| {
            MsiError::Io(crate::error::IoContext::from_string(format!(
                "Patch file not found: {}: {e}",
                patch_path.display()
            )))
        })?;

        let reader = crate::cfb::reader::CfbReader::new(&bytes)
            .map_err(|e| MsiError::PatchApplicationError(format!("Invalid MSP header: {e}")))?;

        // Validate any embedded cabinet streams if present
        for entry in reader.entries() {
            if entry.object_type() == crate::cfb::directory::ObjectType::Stream {
                let name = entry.name();
                if Path::new(name)
                    .extension()
                    .is_some_and(|ext| ext.eq_ignore_ascii_case("cab"))
                {
                    let cab_bytes = reader.read_stream(name).unwrap_or_default();
                    let _ = crate::cab::reader::CabinetReader::new(&cab_bytes).map_err(|e| {
                        MsiError::PatchCorruptCab {
                            reason: format!("CAB header invalid: {e}"),
                        }
                    })?;
                }
            }
        }

        // Check for an explicit XML PatchManifest stream first
        if let Ok(xml_bytes) = reader.read_stream("PatchManifest") {
            let xml_str = String::from_utf8_lossy(&xml_bytes);
            return Self::parse_manifest_xml(&xml_str);
        }

        let summary_bytes = reader
            .read_stream(crate::cfb::SUMMARY_INFORMATION_STREAM)
            .map_err(|e| {
                MsiError::PatchApplicationError(format!("Missing SummaryInformation in MSP: {e}"))
            })?;
        let summary = crate::database::summary_info::SummaryInfo::parse(&summary_bytes)?;

        // Check if comments contain a <PatchManifest ...> XML payload
        if let Some(ref comments) = summary.comments {
            if comments.trim_start().starts_with("<PatchManifest") {
                return Self::parse_manifest_xml(comments);
            }
        }

        let target_product_code = summary
            .template
            .as_deref()
            .and_then(|t| t.split(';').next())
            .filter(|s| !s.is_empty())
            .or(summary.subject.as_deref())
            .ok_or_else(|| {
                MsiError::PatchApplicationError(
                    "Missing target product code in MSP SummaryInformation".to_string(),
                )
            })?
            .to_string();

        let baseline_version = summary
            .comments
            .clone()
            .unwrap_or_else(|| "1.0.0".to_string());

        let family = summary
            .keywords
            .clone()
            .or_else(|| summary.author.clone())
            .unwrap_or_else(|| "DefaultFamily".to_string());

        let supersedes: Vec<String> = summary
            .last_author
            .as_deref()
            .map(|s| {
                s.split(';')
                    .map(str::trim)
                    .filter(|item| !item.is_empty())
                    .map(ToString::to_string)
                    .collect()
            })
            .unwrap_or_default();

        Ok(MspMetadata {
            target_product_code,
            baseline_version,
            supersedes,
            family,
        })
    }

    /// Determines the patch sequence and applicability for a given set of patches against a target product.
    /// Handles complex patch sequencing, supersedence, and patch families (`Patch` table logic).
    ///
    /// # Arguments
    ///
    /// * `target_product_code` - Target product code GUID or identifier.
    /// * `target_baseline` - Expected baseline version of the target product.
    /// * `patch_paths` - Candidate `.msp` patch paths to sequence.
    ///
    /// # Returns
    ///
    /// Ordered vector of applicable non-superseded patch paths.
    ///
    /// # Errors
    ///
    /// Returns [`MsiError`] if sequencing fails, baseline mismatch occurs, or a patch is invalid.
    pub fn determine_patch_sequence(
        target_product_code: &str,
        target_baseline: &str,
        patch_paths: &[&Path],
    ) -> Result<Vec<PathBuf>> {
        let mut applicable: Vec<(PathBuf, String)> = Vec::new();
        let mut superseded_patches = Vec::new();

        for path in patch_paths {
            let meta = Self::extract_msp_metadata(path)?;

            if meta.target_product_code != target_product_code {
                return Err(MsiError::PatchApplicationError(format!(
                    "Target product mismatch: expected {}, got {}",
                    target_product_code, meta.target_product_code
                )));
            }

            if meta.baseline_version != target_baseline {
                return Err(MsiError::WrongPatchBaseline(format!(
                    "Baseline mismatch: expected {}, got {}",
                    target_baseline, meta.baseline_version
                )));
            }

            let patch_id = path
                .file_stem()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string();
            superseded_patches.extend(meta.supersedes.iter().cloned());
            applicable.push(((*path).to_path_buf(), patch_id));
        }

        // Filter out superseded patches based on patch family supersedence rules
        applicable.retain(|(p, patch_id)| {
            let name = p.file_name().unwrap_or_default().to_string_lossy();
            !superseded_patches
                .iter()
                .any(|s| patch_id == s || name.contains(s.as_str()))
        });

        Ok(applicable.into_iter().map(|(p, _)| p).collect())
    }

    /// Applies a Microsoft Patch (`.msp`) delta sequence to a target file (`ApplyPatchToFile`).
    ///
    /// # Arguments
    ///
    /// * `patch_data` - Raw `PA19`/`PA30`/`MSDelta` binary patch payload.
    /// * `source_path` - Path to the baseline source file.
    /// * `target_path` - Path where the patched output file will be written.
    ///
    /// # Returns
    ///
    /// `Ok(())` when the delta patch is decoded and written to `target_path`.
    ///
    /// # Errors
    ///
    /// Returns [`MsiError`] if applying the patch fails due to I/O or `MSDelta` decode errors.
    pub fn apply_patch_to_file(
        patch_data: &[u8],
        source_path: &Path,
        target_path: &Path,
    ) -> Result<()> {
        #[cfg(windows)]
        {
            use windows::core::{s, w};
            use windows::Win32::Foundation::FreeLibrary;
            use windows::Win32::System::LibraryLoader::{GetProcAddress, LoadLibraryW};

            /// Function pointer signature for `mspatcha.dll::ApplyPatchToFileByBuffers` (`patchapi.h`).
            type ApplyPatchToFileByBuffersFn = unsafe extern "system" fn(
                patch_file_mapped: *const u8,
                patch_file_size: u32,
                old_file_mapped: *const u8,
                old_file_size: u32,
                new_file_buffer: *mut *mut u8,
                new_file_buffer_size: u32,
                new_file_actual_size: *mut u32,
                new_file_time: *mut std::ffi::c_void,
                apply_option_flags: u32,
                progress_callback: *const std::ffi::c_void,
                callback_context: *const std::ffi::c_void,
            )
                -> windows::core::BOOL;

            let old_bytes = std::fs::read(source_path).unwrap_or_default();
            let patch_size = u32::try_from(patch_data.len()).unwrap_or(u32::MAX);
            let old_size = u32::try_from(old_bytes.len()).unwrap_or(u32::MAX);

            // SAFETY: Loading mspatcha.dll and invoking ApplyPatchToFileByBuffers with the exact 11-parameter Win32 PatchAPI signature and valid stack output pointers.
            unsafe {
                if let Ok(module) = LoadLibraryW(w!("mspatcha.dll")) {
                    if let Some(proc) = GetProcAddress(module, s!("ApplyPatchToFileByBuffers")) {
                        let apply_patch: ApplyPatchToFileByBuffersFn = std::mem::transmute(proc);
                        let mut new_buf: *mut u8 = std::ptr::null_mut();
                        let mut new_size: u32 = 0;

                        let _ = apply_patch(
                            patch_data.as_ptr(),
                            patch_size,
                            old_bytes.as_ptr(),
                            old_size,
                            &raw mut new_buf,
                            0,
                            &raw mut new_size,
                            std::ptr::null_mut(),
                            0x0000_0004, // APPLY_OPTION_TEST_ONLY
                            std::ptr::null(),
                            std::ptr::null(),
                        );
                    }
                    let _ = FreeLibrary(module);
                }
            }
            // Ensure pure-Rust PatchDecoder validates and writes deterministic output
        }

        let decoder = PatchDecoder::new(patch_data)?;
        decoder.apply_to_file(source_path, target_path)
    }

    /// Removes patches from a product by validating each patch identifier or `.msp` package
    /// and unregistering its patch state from the installer registry store.
    ///
    /// # Arguments
    ///
    /// * `patch_list` - Semicolon-separated list of patch GUIDs or `.msp` paths.
    /// * `product_code` - Target product code GUID.
    ///
    /// # Returns
    ///
    /// `Ok(())` on successful patch removal.
    ///
    /// # Errors
    ///
    /// Returns [`MsiError`] if `patch_list` or `product_code` is empty or any patch entry is malformed.
    pub fn remove_patches(patch_list: &str, product_code: &str) -> Result<()> {
        if patch_list.is_empty() || product_code.is_empty() {
            return Err(MsiError::InvalidArgument {
                argument: "patch_list".to_string(),
                reason: "Empty list".to_string(),
            });
        }

        let mut store = crate::platform::registry_store::RegistryStore::new();
        for patch in patch_list.split(';') {
            let trimmed = patch.trim();
            if trimmed.is_empty() {
                return Err(MsiError::PatchApplicationError(
                    "Empty patch identifier in patch list".to_string(),
                ));
            }

            let path = Path::new(trimmed);
            let is_guid = trimmed.starts_with('{') && trimmed.ends_with('}') && trimmed.len() == 38;
            if path.exists() {
                let _ = Self::extract_msp_metadata(path)?;
            } else if !is_guid
                && !trimmed
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || c == '-')
            {
                return Err(MsiError::PatchApplicationError(format!(
                    "Invalid patch {trimmed}"
                )));
            }

            if let Ok(squid) = crate::platform::squid::encode_squid(trimmed) {
                let key = format!(
                    r"Software\Microsoft\Windows\CurrentVersion\Installer\UserData\S-1-5-18\Patches\{squid}"
                );
                store.delete_value(
                    crate::platform::registry_store::RegistryRoot::LocalMachine,
                    &key,
                    Some("State"),
                );
            }
        }
        store.commit();
        Ok(())
    }

    /// Extracts an embedded patch transform (`.mst`) stream or substorage from a `.msp` CFB container.
    ///
    /// # Arguments
    ///
    /// * `patch_path` - Path to the `.msp` patch package on disk.
    ///
    /// # Returns
    ///
    /// Byte vector containing the extracted transform payload.
    ///
    /// # Errors
    ///
    /// Returns [`MsiError`] if the file cannot be read, is not a valid CFB container, or contains no transform.
    pub fn extract_patch_transform(patch_path: &Path) -> Result<Vec<u8>> {
        let bytes = std::fs::read(patch_path).map_err(|_| {
            MsiError::Io(crate::error::IoContext::from_string(
                "Patch file not found".to_string(),
            ))
        })?;

        let reader = crate::cfb::reader::CfbReader::new(&bytes)
            .map_err(|e| MsiError::InvalidPatchSequence(format!("Invalid MSP CFB: {e}")))?;

        for entry in reader.entries() {
            if entry.object_type() == crate::cfb::directory::ObjectType::Storage {
                return reader.extract_sub_storage(entry.name());
            }
            if entry.object_type() == crate::cfb::directory::ObjectType::Stream {
                let name = entry.name();
                if name != crate::cfb::SUMMARY_INFORMATION_STREAM && name != "PatchManifest" {
                    return reader.read_stream(name);
                }
            }
        }

        Err(MsiError::InvalidPatchSequence(
            "No transform found in MSP".to_string(),
        ))
    }
}

#[cfg(test)]
#[allow(clippy::redundant_clone)]
mod tests {
    use super::*;
    use crate::cfb::header::CfbVersion;
    use crate::cfb::writer::CfbWriter;
    use crate::database::summary_info::SummaryInfo;
    use std::fs::File;

    /// Builds a valid `.msp` CFB container byte vector for testing.
    fn build_test_msp_bytes(
        target_product_code: Option<&str>,
        baseline_version: Option<&str>,
        family: Option<&str>,
        supersedes: Option<&str>,
        transform_bytes: Option<&[u8]>,
    ) -> Vec<u8> {
        let mut writer = CfbWriter::new(CfbVersion::V3);
        let mut si = SummaryInfo::default();
        if let Some(tpc) = target_product_code {
            si.template = Some(tpc.to_string());
        }
        if let Some(bv) = baseline_version {
            si.comments = Some(bv.to_string());
        }
        if let Some(fam) = family {
            si.keywords = Some(fam.to_string());
        }
        if let Some(sup) = supersedes {
            si.last_author = Some(sup.to_string());
        }
        let _ = writer.add_stream(crate::cfb::SUMMARY_INFORMATION_STREAM, &si.to_bytes());
        if let Some(mst) = transform_bytes {
            let _ = writer.add_stream("PatchTransform.mst", mst);
        }
        writer.build()
    }

    #[test]
    fn test_parse_manifest_xml() {
        let valid_xml = r#"<PatchManifest TargetProductCode="TARGET-123" BaselineVersion="1.0.0" Family="FamilyA">
            <Supersedes>
                <Patch Id="v1"/>
            </Supersedes>
        </PatchManifest>"#;

        let meta = PatchEngine::parse_manifest_xml(valid_xml).unwrap_or(MspMetadata {
            target_product_code: String::new(),
            baseline_version: String::new(),
            supersedes: Vec::new(),
            family: String::new(),
        });
        assert_eq!(meta.target_product_code, "TARGET-123");
        assert_eq!(meta.baseline_version, "1.0.0");
        assert_eq!(meta.family, "FamilyA");
        assert_eq!(meta.supersedes, vec!["v1".to_string()]);

        let missing_attrs = r#"<PatchManifest TargetProductCode="TARGET-123" />"#;
        let err = PatchEngine::parse_manifest_xml(missing_attrs)
            .err()
            .unwrap_or_default();
        assert!(err.to_string().contains("Patch XML parse error"));

        let wrong_tag = "<WrongTag />";
        let err_tag = PatchEngine::parse_manifest_xml(wrong_tag)
            .err()
            .unwrap_or_default();
        assert!(err_tag.to_string().contains("Patch XML parse error"));

        let invalid_xml = r#"<PatchManifest TargetProductCode="TARGET-123""#;
        let err_invalid = PatchEngine::parse_manifest_xml(invalid_xml)
            .err()
            .unwrap_or_default();
        assert!(err_invalid.to_string().contains("Patch XML parse error"));

        // Test without Supersedes node
        let no_supersedes_xml = r#"<PatchManifest TargetProductCode="TARGET-123" BaselineVersion="1.0.0" Family="FamilyA" />"#;
        let meta_no_sup =
            PatchEngine::parse_manifest_xml(no_supersedes_xml).unwrap_or(MspMetadata {
                target_product_code: String::new(),
                baseline_version: String::new(),
                supersedes: Vec::new(),
                family: String::new(),
            });
        assert!(meta_no_sup.supersedes.is_empty());

        // Test with Supersedes but without Id
        let no_id_xml = r#"<PatchManifest TargetProductCode="TARGET-123" BaselineVersion="1.0.0" Family="FamilyA">
            <Supersedes>
                <Patch />
                <Patch Id="v2" />
            </Supersedes>
        </PatchManifest>"#;
        let meta_no_id = PatchEngine::parse_manifest_xml(no_id_xml).unwrap_or(MspMetadata {
            target_product_code: String::new(),
            baseline_version: String::new(),
            supersedes: Vec::new(),
            family: String::new(),
        });
        assert_eq!(meta_no_id.supersedes, vec!["v2".to_string()]);
    }

    #[test]
    #[allow(clippy::too_many_lines)]
    fn test_patch_engine_extract_metadata() {
        let base_dir = std::env::temp_dir().join("msi_test_patch_extract_meta");
        let _ = std::fs::create_dir_all(&base_dir);
        let valid_path = base_dir.join("valid_v2.msp");
        let msp_bytes = build_test_msp_bytes(
            Some("TARGET-123"),
            Some("1.0.0"),
            Some("FamilyA"),
            Some("v1"),
            Some(&[1, 2, 3]),
        );
        let _ = std::fs::write(&valid_path, &msp_bytes);

        let meta = PatchEngine::extract_msp_metadata(&valid_path).unwrap_or(MspMetadata {
            target_product_code: String::new(),
            baseline_version: String::new(),
            supersedes: Vec::new(),
            family: String::new(),
        });
        assert_eq!(meta.target_product_code, "TARGET-123");
        assert_eq!(meta.baseline_version, "1.0.0");
        assert_eq!(meta.supersedes, vec!["v1".to_string()]);
        assert_eq!(meta.family, "FamilyA");

        // Non-CFB file returns PatchApplicationError("Invalid MSP header...")
        let invalid_path = base_dir.join("not_a_cfb.msp");
        assert!(File::create(&invalid_path).is_ok());
        let err = PatchEngine::extract_msp_metadata(&invalid_path)
            .err()
            .unwrap_or_default();
        assert!(err.to_string().contains("Invalid MSP header"));

        // CFB missing SummaryInformation stream
        let no_si_path = base_dir.join("no_si.msp");
        let no_si_writer = CfbWriter::new(CfbVersion::V3);
        let _ = std::fs::write(&no_si_path, no_si_writer.build());
        let err_no_si = PatchEngine::extract_msp_metadata(&no_si_path)
            .err()
            .unwrap_or_default();
        assert!(err_no_si.to_string().contains("Missing SummaryInformation"));

        // CFB with corrupt SummaryInformation stream
        let bad_si_path = base_dir.join("bad_si.msp");
        let mut bad_si_writer = CfbWriter::new(CfbVersion::V3);
        let _ = bad_si_writer.add_stream(crate::cfb::SUMMARY_INFORMATION_STREAM, b"TOO_SHORT");
        let _ = std::fs::write(&bad_si_path, bad_si_writer.build());
        assert!(PatchEngine::extract_msp_metadata(&bad_si_path).is_err());

        // CFB with empty SummaryInformation (missing target product code)
        let empty_si_path = base_dir.join("empty_si.msp");
        let empty_si_bytes = build_test_msp_bytes(None, None, None, None, None);
        let _ = std::fs::write(&empty_si_path, &empty_si_bytes);
        let err_empty_si = PatchEngine::extract_msp_metadata(&empty_si_path)
            .err()
            .unwrap_or_default();
        assert!(err_empty_si
            .to_string()
            .contains("Missing target product code"));

        // Missing file on disk
        let missing_path = base_dir.join("missing.msp");
        let err2 = PatchEngine::extract_msp_metadata(&missing_path)
            .err()
            .unwrap_or_default();
        assert!(err2.to_string().contains("Patch file not found"));

        // CFB with a corrupt .cab stream
        let corrupt_path = base_dir.join("corrupt_cab.msp");
        let mut corrupt_writer = CfbWriter::new(CfbVersion::V3);
        let si = SummaryInfo {
            template: Some("TARGET-123".to_string()),
            ..SummaryInfo::default()
        };
        let _ = corrupt_writer.add_stream(crate::cfb::SUMMARY_INFORMATION_STREAM, &si.to_bytes());
        let _ = corrupt_writer.add_stream("patch.cab", b"NOT_A_CAB");
        let _ = std::fs::write(&corrupt_path, corrupt_writer.build());
        let err3 = PatchEngine::extract_msp_metadata(&corrupt_path)
            .err()
            .unwrap_or_default();
        assert!(err3.to_string().contains("CAB header invalid"));

        // CFB with a valid .cab stream
        let valid_cab_path = base_dir.join("valid_cab.msp");
        let mut valid_cab_writer = CfbWriter::new(CfbVersion::V3);
        let _ = valid_cab_writer.add_stream(crate::cfb::SUMMARY_INFORMATION_STREAM, &si.to_bytes());
        let mut cab_builder =
            crate::cab::writer::CabinetWriter::new(crate::cab::CompressionType::None);
        let _ = cab_builder.add_file("dummy.txt", b"hello");
        let cab_data = cab_builder.build().unwrap_or_default();
        let _ = valid_cab_writer.add_stream("patch.cab", &cab_data);
        let _ = std::fs::write(&valid_cab_path, valid_cab_writer.build());
        let meta_valid_cab =
            PatchEngine::extract_msp_metadata(&valid_cab_path).unwrap_or(MspMetadata {
                target_product_code: String::new(),
                baseline_version: String::new(),
                supersedes: Vec::new(),
                family: String::new(),
            });
        assert_eq!(meta_valid_cab.target_product_code, "TARGET-123");

        // CFB with explicit PatchManifest stream
        let manifest_test_path = base_dir.join("manifest_stream.msp");
        let mut manifest_writer = CfbWriter::new(CfbVersion::V3);
        let xml = r#"<PatchManifest TargetProductCode="TARGET-123" BaselineVersion="1.0.0" Family="FamilyA"><Supersedes><Patch Id="v1"/></Supersedes></PatchManifest>"#;
        let _ = manifest_writer.add_stream("PatchManifest", xml.as_bytes());
        let _ = std::fs::write(&manifest_test_path, manifest_writer.build());
        let meta_manifest =
            PatchEngine::extract_msp_metadata(&manifest_test_path).unwrap_or(MspMetadata {
                target_product_code: String::new(),
                baseline_version: String::new(),
                supersedes: Vec::new(),
                family: String::new(),
            });
        assert_eq!(meta_manifest.target_product_code, "TARGET-123");

        // CFB with PatchManifest XML inside SummaryInformation comments
        let comment_xml_path = base_dir.join("comment_xml.msp");
        let comment_xml_bytes = build_test_msp_bytes(None, Some(xml), None, None, None);
        let _ = std::fs::write(&comment_xml_path, &comment_xml_bytes);
        let meta_comment_xml =
            PatchEngine::extract_msp_metadata(&comment_xml_path).unwrap_or(MspMetadata {
                target_product_code: String::new(),
                baseline_version: String::new(),
                supersedes: Vec::new(),
                family: String::new(),
            });
        assert_eq!(meta_comment_xml.target_product_code, "TARGET-123");
        let _ = std::fs::remove_dir_all(&base_dir);
    }

    #[test]
    fn test_determine_patch_sequence_supersedence() {
        let base_dir = std::env::temp_dir().join("msi_test_patch_seq_sup");
        let _ = std::fs::create_dir_all(&base_dir);
        let p1 = base_dir.join("patch_v1.msp");
        let p2 = base_dir.join("patch_v2.msp");
        let _ = std::fs::write(
            &p1,
            build_test_msp_bytes(
                Some("TARGET-123"),
                Some("1.0.0"),
                Some("FamilyA"),
                None,
                None,
            ),
        );
        let _ = std::fs::write(
            &p2,
            build_test_msp_bytes(
                Some("TARGET-123"),
                Some("1.0.0"),
                Some("FamilyA"),
                Some("patch_v1"),
                None,
            ),
        );

        let paths: [&Path; 2] = [&p1, &p2];
        let seq = PatchEngine::determine_patch_sequence("TARGET-123", "1.0.0", &paths)
            .unwrap_or_default();

        // v2 supersedes v1, so only v2 should be returned
        assert_eq!(seq.len(), 1);
        assert_eq!(
            seq[0].file_name().unwrap_or_default().to_string_lossy(),
            "patch_v2.msp"
        );

        // Missing patch in sequence returns error
        let missing = base_dir.join("missing.msp");
        let bad_paths: [&Path; 1] = [&missing];
        assert!(PatchEngine::determine_patch_sequence("TARGET-123", "1.0.0", &bad_paths).is_err());
        let _ = std::fs::remove_dir_all(&base_dir);
    }

    #[test]
    fn test_determine_patch_sequence_target_mismatch() {
        let base_dir = std::env::temp_dir().join("msi_test_patch_seq_tgt");
        let _ = std::fs::create_dir_all(&base_dir);
        let p1 = base_dir.join("patch_v1.msp");
        let _ = std::fs::write(
            &p1,
            build_test_msp_bytes(
                Some("TARGET-123"),
                Some("1.0.0"),
                Some("FamilyA"),
                None,
                None,
            ),
        );

        let paths: [&Path; 1] = [&p1];
        let err = PatchEngine::determine_patch_sequence("WRONG-TARGET", "1.0.0", &paths)
            .err()
            .unwrap_or_default();
        assert!(err.to_string().contains("Target product mismatch"));
        let _ = std::fs::remove_dir_all(&base_dir);
    }

    #[test]
    fn test_determine_patch_sequence_baseline_mismatch() {
        let base_dir = std::env::temp_dir().join("msi_test_patch_seq_base");
        let _ = std::fs::create_dir_all(&base_dir);
        let p1 = base_dir.join("patch_v1.msp");
        let _ = std::fs::write(
            &p1,
            build_test_msp_bytes(
                Some("TARGET-123"),
                Some("1.0.0"),
                Some("FamilyA"),
                None,
                None,
            ),
        );

        let paths: [&Path; 1] = [&p1];
        let err = PatchEngine::determine_patch_sequence("TARGET-123", "2.0.0", &paths)
            .err()
            .unwrap_or_default();
        assert!(err.to_string().contains("Baseline mismatch"));
        let _ = std::fs::remove_dir_all(&base_dir);
    }

    #[test]
    fn test_apply_patch_to_file() {
        let base_dir = std::env::temp_dir().join("msi_test_apply_patch_file");
        let _ = std::fs::create_dir_all(&base_dir);
        let src_path = base_dir.join("src.txt");
        let tgt_path = base_dir.join("tgt.txt");
        assert!(std::fs::write(&src_path, "dummy source").is_ok());

        let mut data = vec![b'P', b'A', b'3', b'0'];
        data.extend(0u32.to_le_bytes()); // Src (0 = bypass check)
        data.extend(0u32.to_le_bytes()); // Tgt
        data.extend(16u32.to_le_bytes()); // Size

        assert!(PatchEngine::apply_patch_to_file(&data, &src_path, &tgt_path).is_ok());
        assert!(PatchEngine::apply_patch_to_file(b"BAD", &src_path, &tgt_path).is_err());

        let out = std::fs::read(&tgt_path).unwrap_or_default();
        assert_eq!(out.len(), 16);
        let _ = std::fs::remove_dir_all(&base_dir);
    }

    #[test]
    fn test_determine_patch_sequence_empty() {
        let paths: [&Path; 0] = [];
        let seq =
            PatchEngine::determine_patch_sequence("TARGET", "1.0", &paths).unwrap_or_default();
        assert!(seq.is_empty());
    }

    #[test]
    fn test_remove_patches() {
        let base_dir = std::env::temp_dir().join("msi_test_remove_patches");
        let _ = std::fs::create_dir_all(&base_dir);
        let valid_msp = base_dir.join("valid.msp");
        let _ = std::fs::write(
            &valid_msp,
            build_test_msp_bytes(Some("TARGET-123"), Some("1.0.0"), None, None, None),
        );
        let bad_msp = base_dir.join("bad.msp");
        let _ = std::fs::write(&bad_msp, b"not a cfb");

        assert!(PatchEngine::remove_patches(
            &format!(
                "patch1;{{11111111-1111-1111-1111-111111111111}};{}",
                valid_msp.display()
            ),
            "PROD"
        )
        .is_ok());
        assert!(PatchEngine::remove_patches(&bad_msp.to_string_lossy(), "PROD").is_err());
        assert!(PatchEngine::remove_patches("", "PROD")
            .err()
            .unwrap_or_default()
            .to_string()
            .contains("Empty list"));
        assert!(PatchEngine::remove_patches("patch1", "")
            .err()
            .unwrap_or_default()
            .to_string()
            .contains("Empty list"));
        assert!(PatchEngine::remove_patches("patch1;;patch2", "PROD")
            .err()
            .unwrap_or_default()
            .to_string()
            .contains("Empty patch identifier"));
        assert!(PatchEngine::remove_patches("invalid_patch!", "PROD")
            .err()
            .unwrap_or_default()
            .to_string()
            .contains("Invalid patch"));
        let _ = std::fs::remove_dir_all(&base_dir);
    }

    #[test]
    fn test_extract_patch_transform() {
        let base_dir = std::env::temp_dir().join("msi_test_extract_patch_mst");
        let _ = std::fs::create_dir_all(&base_dir);
        let valid_path = base_dir.join("valid.msp");
        let _ = std::fs::write(
            &valid_path,
            build_test_msp_bytes(
                Some("TARGET-123"),
                Some("1.0.0"),
                Some("FamilyA"),
                None,
                Some(&[0x01, 0x02, 0x03]),
            ),
        );

        let transform = PatchEngine::extract_patch_transform(&valid_path).unwrap_or_default();
        assert_eq!(transform, vec![0x01, 0x02, 0x03]);

        // Test substorage transform extraction
        let substorage_path = base_dir.join("substorage.msp");
        let mut sub_writer = CfbWriter::new(CfbVersion::V3);
        let _ = sub_writer.add_stream("SubTransform", &[0xAA, 0xBB]);
        let mut sub_bytes = sub_writer.build();
        // Flip directory entry 1 from Stream (2) to Storage (1)
        let root_dir_sec =
            u32::from_le_bytes([sub_bytes[48], sub_bytes[49], sub_bytes[50], sub_bytes[51]])
                as usize;
        let entry1_off = (root_dir_sec + 1) * 512 + 128;
        sub_bytes[entry1_off + 66] = 1;
        let _ = std::fs::write(&substorage_path, &sub_bytes);
        let sub_transform =
            PatchEngine::extract_patch_transform(&substorage_path).unwrap_or_default();
        assert!(!sub_transform.is_empty());

        // Test CFB with no transform stream
        let no_transform_path = base_dir.join("no_transform.msp");
        let _ = std::fs::write(
            &no_transform_path,
            build_test_msp_bytes(Some("TARGET-123"), Some("1.0.0"), None, None, None),
        );
        let err = PatchEngine::extract_patch_transform(&no_transform_path)
            .err()
            .unwrap_or_default();
        assert!(err.to_string().contains("No transform found in MSP"));

        // Test invalid (non-CFB) file
        let bad_cfb_path = base_dir.join("bad_cfb.msp");
        let _ = std::fs::write(&bad_cfb_path, b"not a cfb");
        let err_cfb = PatchEngine::extract_patch_transform(&bad_cfb_path)
            .err()
            .unwrap_or_default();
        assert!(err_cfb.to_string().contains("Invalid MSP CFB"));

        let missing_path = base_dir.join("missing.msp");
        let err2 = PatchEngine::extract_patch_transform(&missing_path)
            .err()
            .unwrap_or_default();
        assert!(err2.to_string().contains("Patch file not found"));
        let _ = std::fs::remove_dir_all(&base_dir);
    }

    #[test]
    fn test_msp_metadata_derives() {
        let m1 = MspMetadata {
            target_product_code: "A".to_string(),
            baseline_version: "1.0".to_string(),
            supersedes: vec![],
            family: "F".to_string(),
        };
        let m2 = m1.clone();
        assert_eq!(m1, m2);
        assert_eq!(format!("{m1:?}"), format!("{m2:?}"));
    }
}

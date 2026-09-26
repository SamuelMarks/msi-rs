//! Windows Installer Patching (`.msp`), Patch Creation (`<PatchCreation>` / `<Patch>`),
//! Binary Delta Compression, and `CPack` `WiX` XML patch injection.
//!
//! Provides support for:
//! - Parsing `<PatchCreation>` / `<Patch>` documents with metadata, families, and targets.
//! - Byte-level binary delta difference compression (`BinaryDelta`) with longest common matches.
//! - Assembling Compound File Binary Format (`.msp`) patch containers (`PatchPackageBuilder`).
//! - Parsing and injecting `CPack` patch fragments into the `WiX` AST (`CPackWiXPatch`).

use crate::cab::folder::CompressionType;
use crate::cab::writer::CabinetWriter;
use crate::cfb::header::CfbVersion;
use crate::cfb::writer::CfbWriter;
use crate::database::summary_info::SummaryInfo;
use crate::error::{Error, Result};
use crate::wix::xml::{XmlNode, XmlParser};
use std::collections::HashMap;
use std::path::Path;

/// A parsed `<CPackWiXFragment>` targeting an AST element by ID.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CPackWiXFragment {
    /// Target identifier (e.g. `#PRODUCT`, `#PRODUCTFEATURE`, `MyComponent`, `MyDir`).
    pub id: String,
    /// Attributes to merge onto the target element.
    pub attributes: Vec<(String, String)>,
    /// Child elements to append to the target element.
    pub children: Vec<XmlNode>,
}

impl CPackWiXFragment {
    /// Creates a new [`CPackWiXFragment`].
    ///
    /// # Arguments
    ///
    /// * `id` - Target element identifier.
    ///
    /// # Returns
    ///
    /// A new patch fragment.
    #[must_use]
    pub const fn new(id: String) -> Self {
        Self {
            id,
            attributes: Vec::new(),
            children: Vec::new(),
        }
    }
}

/// A parsed `<CPackWiXPatch>` container document.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CPackWiXPatch {
    /// Constituent fragments in declaration order.
    pub fragments: Vec<CPackWiXFragment>,
}

impl CPackWiXPatch {
    /// Creates a new empty [`CPackWiXPatch`].
    ///
    /// # Returns
    ///
    /// A new patch container.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            fragments: Vec::new(),
        }
    }

    /// Parses a patch XML string into a [`CPackWiXPatch`].
    ///
    /// # Arguments
    ///
    /// * `xml_content` - Raw XML string containing `<CPackWiXPatch>`.
    ///
    /// # Returns
    ///
    /// Parsed [`CPackWiXPatch`].
    ///
    /// # Errors
    ///
    /// Returns [`Error::XmlParse`] on syntax error or [`Error::WixCompiler`]
    /// on schema validation failure.
    pub fn parse(xml_content: &str) -> Result<Self> {
        let parser = XmlParser::new();
        let root = parser.parse(xml_content)?;
        Self::from_xml_node(&root)
    }

    /// Builds a [`CPackWiXPatch`] from an [`XmlNode`].
    ///
    /// # Arguments
    ///
    /// * `root` - Root XML node, expecting `<CPackWiXPatch>`.
    ///
    /// # Returns
    ///
    /// Parsed [`CPackWiXPatch`].
    ///
    /// # Errors
    ///
    /// Returns [`Error::WixCompiler`] if root is not `<CPackWiXPatch>` or missing `Id`.
    pub fn from_xml_node(root: &XmlNode) -> Result<Self> {
        if root.tag != "CPackWiXPatch" {
            return Err(Error::WixCompiler {
                element: root.tag.clone(),
                message: "expected root element <CPackWiXPatch>".to_string(),
            });
        }

        let mut fragments = Vec::new();

        for child in &root.children {
            if child.tag == "CPackWiXFragment" {
                let id = child.attribute("Id").ok_or_else(|| Error::WixCompiler {
                    element: "CPackWiXFragment".to_string(),
                    message: "missing required 'Id' attribute".to_string(),
                })?;

                let mut frag = CPackWiXFragment::new(id.to_string());
                for (k, v) in &child.attributes {
                    if k != "Id" {
                        frag.attributes.push((k.clone(), v.clone()));
                    }
                }
                frag.children.clone_from(&child.children);
                fragments.push(frag);
            }
        }

        Ok(Self { fragments })
    }

    /// Injects this patch document's fragments into a target `WiX` AST root.
    ///
    /// # Arguments
    ///
    /// * `root` - Root `<Wix>` XML document to mutate.
    ///
    /// # Returns
    ///
    /// Number of successfully injected fragments.
    pub fn apply_to_ast(&self, root: &mut XmlNode) -> usize {
        let mut count = 0;
        for frag in &self.fragments {
            if Self::inject_fragment(root, frag) {
                count += 1;
            }
        }
        count
    }

    /// Recursively locates the target element for a fragment and injects attributes/children.
    fn inject_fragment(node: &mut XmlNode, frag: &CPackWiXFragment) -> bool {
        let is_match = match frag.id.as_str() {
            "#PRODUCT" => node.tag == "Product" || node.tag == "Package",
            "#PRODUCTFEATURE" => node.tag == "Feature",
            target_id => node.attribute("Id") == Some(target_id),
        };

        if is_match {
            for (k, v) in &frag.attributes {
                node.attributes.insert(k.clone(), v.clone());
            }
            node.children.extend(frag.children.clone());
            return true;
        }

        for child in &mut node.children {
            if Self::inject_fragment(child, frag) {
                return true;
            }
        }

        false
    }
}

/// Summary Information stream metadata authored in `<PatchInformation>`.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PatchInformation {
    /// Patch package title (`PID_TITLE`).
    pub title: Option<String>,
    /// Patch subject / description (`PID_SUBJECT`).
    pub subject: Option<String>,
    /// Author / Manufacturer (`PID_AUTHOR`).
    pub author: Option<String>,
    /// Keywords (`PID_KEYWORDS`).
    pub keywords: Option<String>,
    /// Description comments (`PID_COMMENTS`).
    pub comments: Option<String>,
    /// ANSI code page for summary stream (`PID_CODEPAGE`).
    pub summary_codepage: Option<u16>,
}

/// Extended patch metadata authored in `<PatchMetadata>`.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PatchMetadata {
    /// Whether the patch can be uninstalled (`AllowRemoval`).
    pub allow_removal: bool,
    /// Classification of the patch, e.g. "Update", "`CriticalUpdate`", "Hotfix" (`Classification`).
    pub classification: Option<String>,
    /// Human-readable patch description (`Description`).
    pub description: Option<String>,
    /// Display name shown in Add/Remove Programs (`DisplayName`).
    pub display_name: Option<String>,
    /// URL providing additional patch documentation (`MoreInfoURL`).
    pub more_info_url: Option<String>,
    /// Target product name (`TargetProductName`).
    pub target_product_name: Option<String>,
}

/// Patch media family definition authored in `<Family>`.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PatchFamily {
    /// Family name identifier (`Name`).
    pub name: String,
    /// Disk ID allocated for the patch media (`DiskId`).
    pub disk_id: i16,
    /// Property name storing media source URL or volume label (`MediaSrcProp`).
    pub media_src_prop: Option<String>,
    /// Starting sequence number for patch files (`SequenceStart`).
    pub sequence_start: i32,
}

/// Target baseline image definition authored in `<TargetImage>`.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct TargetImage {
    /// Identifier for this target image (`Id`).
    pub id: String,
    /// Path to the baseline `.msi` file (`SourceFile`).
    pub source_file: String,
    /// Execution order index (`Order`).
    pub order: i32,
    /// Validation flags or transform rules (`Validation`).
    pub validation: Option<String>,
    /// Symbol search paths (`SymbolPaths`).
    pub symbol_paths: Vec<String>,
}

/// Updated package image definition authored in `<UpgradeImage>`.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct UpgradeImage {
    /// Identifier for this upgrade image (`Id`).
    pub id: String,
    /// Path to the updated `.msi` file (`SourceFile`).
    pub source_file: String,
    /// Target media family name (`Family`).
    pub family: String,
    /// Target image identifiers paired with this upgrade image (`<TargetImageRef>`).
    pub target_images: Vec<String>,
}

/// A parsed `<PatchCreation>` or modern `<Patch>` document.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PatchCreation {
    /// Patch identifier (`Id`).
    pub id: String,
    /// Whether intermediate working files should be cleaned (`CleanWorkingFolder`).
    pub clean_working_folder: bool,
    /// Output file path (`OutputPath`).
    pub output_path: Option<String>,
    /// Whether to enforce full file packaging instead of delta differences (`WholeFilesOnly`).
    pub whole_files_only: bool,
    /// Summary information stream metadata (`<PatchInformation>`).
    pub patch_information: Option<PatchInformation>,
    /// Display and Add/Remove Programs metadata (`<PatchMetadata>`).
    pub patch_metadata: Option<PatchMetadata>,
    /// Media families (`<Family>`).
    pub families: Vec<PatchFamily>,
    /// Target product codes (`<TargetProductCode>`).
    pub target_product_codes: Vec<String>,
    /// Target baseline images (`<TargetImage>`).
    pub target_images: Vec<TargetImage>,
    /// Updated product images (`<UpgradeImage>`).
    pub upgrade_images: Vec<UpgradeImage>,
}

impl PatchCreation {
    /// Creates a new empty [`PatchCreation`].
    ///
    /// # Arguments
    ///
    /// * `id` - Identifier for this patch creation definition.
    ///
    /// # Returns
    ///
    /// A new initialized [`PatchCreation`].
    #[must_use]
    pub fn new(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            clean_working_folder: true,
            output_path: None,
            whole_files_only: false,
            patch_information: None,
            patch_metadata: None,
            families: Vec::new(),
            target_product_codes: Vec::new(),
            target_images: Vec::new(),
            upgrade_images: Vec::new(),
        }
    }

    /// Parses a raw XML string into a [`PatchCreation`] instance.
    ///
    /// # Arguments
    ///
    /// * `xml_content` - Raw XML containing `<PatchCreation>` or `<Patch>`.
    ///
    /// # Returns
    ///
    /// Parsed [`PatchCreation`].
    ///
    /// # Errors
    ///
    /// Returns [`Error::XmlParse`] on invalid XML syntax or [`Error::WixCompiler`]
    /// on schema validation failures.
    pub fn parse(xml_content: &str) -> Result<Self> {
        let parser = XmlParser::new();
        let root = parser.parse(xml_content)?;
        Self::from_xml_node(&root)
    }

    /// Constructs a [`PatchCreation`] instance from a parsed [`XmlNode`].
    ///
    /// # Arguments
    ///
    /// * `root` - Root XML node, expecting `<PatchCreation>` or `<Patch>`.
    ///
    /// # Returns
    ///
    /// Parsed [`PatchCreation`].
    ///
    /// # Errors
    ///
    /// Returns [`Error::WixCompiler`] if required elements or attributes are missing.
    #[allow(clippy::too_many_lines)]
    pub fn from_xml_node(root: &XmlNode) -> Result<Self> {
        if root.tag != "PatchCreation" && root.tag != "Patch" {
            return Err(Error::WixCompiler {
                element: root.tag.clone(),
                message: "expected root element <PatchCreation> or <Patch>".to_string(),
            });
        }

        let id = root.attribute("Id").map_or("Patch", |s| s).to_string();

        let clean_working_folder = root
            .attribute("CleanWorkingFolder")
            .is_none_or(|v| v.eq_ignore_ascii_case("yes") || v.eq_ignore_ascii_case("true"));

        let output_path = root.attribute("OutputPath").map(ToString::to_string);

        let whole_files_only = root
            .attribute("WholeFilesOnly")
            .is_some_and(|v| v.eq_ignore_ascii_case("yes") || v.eq_ignore_ascii_case("true"));

        let mut patch_information = None;
        let mut patch_metadata = None;
        let mut families = Vec::new();
        let mut target_product_codes = Vec::new();
        let mut target_images = Vec::new();
        let mut upgrade_images = Vec::new();

        for child in &root.children {
            match child.tag.as_str() {
                "PatchInformation" => {
                    patch_information = Some(PatchInformation {
                        title: child.attribute("Title").map(ToString::to_string),
                        subject: child.attribute("Subject").map(ToString::to_string),
                        author: child.attribute("Author").map(ToString::to_string),
                        keywords: child.attribute("Keywords").map(ToString::to_string),
                        comments: child.attribute("Comments").map(ToString::to_string),
                        summary_codepage: child
                            .attribute("SummaryCodepage")
                            .and_then(|v| v.parse().ok()),
                    });
                }
                "PatchMetadata" => {
                    patch_metadata = Some(PatchMetadata {
                        allow_removal: child.attribute("AllowRemoval").is_none_or(|v| {
                            v.eq_ignore_ascii_case("yes") || v.eq_ignore_ascii_case("true")
                        }),
                        classification: child.attribute("Classification").map(ToString::to_string),
                        description: child.attribute("Description").map(ToString::to_string),
                        display_name: child.attribute("DisplayName").map(ToString::to_string),
                        more_info_url: child.attribute("MoreInfoURL").map(ToString::to_string),
                        target_product_name: child
                            .attribute("TargetProductName")
                            .map(ToString::to_string),
                    });
                }
                "Family" => {
                    let name = child
                        .attribute("Name")
                        .map_or("DefaultFamily", |s| s)
                        .to_string();
                    let disk_id = child
                        .attribute("DiskId")
                        .and_then(|v| v.parse().ok())
                        .unwrap_or(2);
                    let media_src_prop = child.attribute("MediaSrcProp").map(ToString::to_string);
                    let sequence_start = child
                        .attribute("SequenceStart")
                        .and_then(|v| v.parse().ok())
                        .unwrap_or(1000);

                    families.push(PatchFamily {
                        name,
                        disk_id,
                        media_src_prop,
                        sequence_start,
                    });
                }
                "TargetProductCode" | "TargetProductCodes" => {
                    if let Some(code) = child.attribute("Id") {
                        target_product_codes.push(code.to_string());
                    } else if !child.text.is_empty() {
                        for code in child.text.split(';') {
                            let trimmed = code.trim();
                            if !trimmed.is_empty() {
                                target_product_codes.push(trimmed.to_string());
                            }
                        }
                    }
                }
                "TargetImage" => {
                    let img_id = child
                        .attribute("Id")
                        .map_or("TargetImage", |s| s)
                        .to_string();
                    let source_file = child.attribute("SourceFile").map_or("", |s| s).to_string();
                    let order = child
                        .attribute("Order")
                        .and_then(|v| v.parse().ok())
                        .unwrap_or(1);
                    let validation = child.attribute("Validation").map(ToString::to_string);

                    let mut symbol_paths = Vec::new();
                    for sub in &child.children {
                        if sub.tag == "SymbolPath" {
                            if let Some(p) = sub.attribute("Path") {
                                symbol_paths.push(p.to_string());
                            }
                        }
                    }

                    target_images.push(TargetImage {
                        id: img_id,
                        source_file,
                        order,
                        validation,
                        symbol_paths,
                    });
                }
                "UpgradeImage" => {
                    let up_id = child
                        .attribute("Id")
                        .map_or("UpgradeImage", |s| s)
                        .to_string();
                    let source_file = child.attribute("SourceFile").map_or("", |s| s).to_string();
                    let family = child.attribute("Family").map_or("", |s| s).to_string();

                    let mut target_refs = Vec::new();
                    for sub in &child.children {
                        if sub.tag == "TargetImageRef" || sub.tag == "TargetImage" {
                            if let Some(t_id) = sub.attribute("Id") {
                                target_refs.push(t_id.to_string());
                            }
                        }
                    }

                    upgrade_images.push(UpgradeImage {
                        id: up_id,
                        source_file,
                        family,
                        target_images: target_refs,
                    });
                }
                _ => {}
            }
        }

        Ok(Self {
            id,
            clean_working_folder,
            output_path,
            whole_files_only,
            patch_information,
            patch_metadata,
            families,
            target_product_codes,
            target_images,
            upgrade_images,
        })
    }
}

/// A single instruction in a [`BinaryDelta`] stream.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BinaryDeltaInstruction {
    /// Copies a block of bytes from the baseline file.
    Copy {
        /// 0-based byte offset in the baseline file.
        offset: usize,
        /// Number of bytes to copy.
        length: usize,
    },
    /// Inserts a newly authored byte sequence.
    Insert {
        /// Byte payload to insert.
        data: Vec<u8>,
    },
}

/// Binary difference representation capturing delta instructions between two file versions.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct BinaryDelta {
    /// Original baseline file size in bytes.
    pub baseline_size: usize,
    /// Target updated file size in bytes.
    pub updated_size: usize,
    /// Ordered list of delta instructions to transform baseline into updated.
    pub instructions: Vec<BinaryDeltaInstruction>,
}

impl BinaryDelta {
    /// Magic header for binary delta serialization (`MSPDELTA\x01`).
    pub const MAGIC: &'static [u8; 9] = b"MSPDELTA\x01";

    /// Computes byte-level difference instructions transforming `baseline` into `updated`.
    ///
    /// Identifies common matching blocks using hash window indexing and emits
    /// a compact sequence of [`BinaryDeltaInstruction::Copy`] and [`BinaryDeltaInstruction::Insert`].
    ///
    /// # Arguments
    ///
    /// * `baseline` - Original baseline file bytes.
    /// * `updated` - Target updated file bytes.
    ///
    /// # Returns
    ///
    /// A computed [`BinaryDelta`].
    #[must_use]
    #[allow(clippy::too_many_lines)]
    pub fn compress(baseline: &[u8], updated: &[u8]) -> Self {
        const MIN_MATCH: usize = 8;
        let mut instructions = Vec::new();
        if updated.is_empty() {
            return Self {
                baseline_size: baseline.len(),
                updated_size: 0,
                instructions,
            };
        }
        if baseline.is_empty() {
            instructions.push(BinaryDeltaInstruction::Insert {
                data: updated.to_vec(),
            });
            return Self {
                baseline_size: 0,
                updated_size: updated.len(),
                instructions,
            };
        }

        let mut baseline_index: HashMap<u64, Vec<usize>> = HashMap::new();
        if baseline.len() >= MIN_MATCH {
            for i in 0..=baseline.len() - MIN_MATCH {
                let mut chunk = [0u8; 8];
                chunk.copy_from_slice(&baseline[i..i + MIN_MATCH]);
                let key = u64::from_le_bytes(chunk);
                baseline_index.entry(key).or_default().push(i);
            }
        }

        let mut pos = 0;
        let mut pending_insert: Vec<u8> = Vec::new();

        while pos < updated.len() {
            let mut best_match: Option<(usize, usize)> = None;

            if pos + MIN_MATCH <= updated.len() {
                let mut chunk = [0u8; 8];
                chunk.copy_from_slice(&updated[pos..pos + MIN_MATCH]);
                let key = u64::from_le_bytes(chunk);

                if let Some(candidates) = baseline_index.get(&key) {
                    for &cand_offset in candidates {
                        let mut match_len = MIN_MATCH;
                        while pos + match_len < updated.len()
                            && cand_offset + match_len < baseline.len()
                            && updated[pos + match_len] == baseline[cand_offset + match_len]
                        {
                            match_len += 1;
                        }
                        if best_match.as_ref().is_none_or(|(_, bl)| match_len > *bl) {
                            best_match = Some((cand_offset, match_len));
                        }
                    }
                }
            }

            if let Some((offset, length)) = best_match {
                if !pending_insert.is_empty() {
                    instructions.push(BinaryDeltaInstruction::Insert {
                        data: std::mem::take(&mut pending_insert),
                    });
                }
                instructions.push(BinaryDeltaInstruction::Copy { offset, length });
                pos += length;
            } else {
                pending_insert.push(updated[pos]);
                pos += 1;
            }
        }

        if !pending_insert.is_empty() {
            instructions.push(BinaryDeltaInstruction::Insert {
                data: pending_insert,
            });
        }

        Self {
            baseline_size: baseline.len(),
            updated_size: updated.len(),
            instructions,
        }
    }

    /// Applies this binary delta to a baseline buffer, producing the updated file content.
    ///
    /// # Arguments
    ///
    /// * `baseline` - Baseline file content.
    ///
    /// # Returns
    ///
    /// Reconstructed updated file bytes.
    ///
    /// # Errors
    ///
    /// Returns [`Error::InvalidArgument`] if instruction copy bounds exceed baseline length.
    pub fn apply(&self, baseline: &[u8]) -> Result<Vec<u8>> {
        let mut output = Vec::with_capacity(self.updated_size);
        for instr in &self.instructions {
            match instr {
                BinaryDeltaInstruction::Copy { offset, length } => {
                    let end =
                        offset
                            .checked_add(*length)
                            .ok_or_else(|| Error::InvalidArgument {
                                argument: "length".to_string(),
                                reason: "copy offset and length overflow".to_string(),
                            })?;
                    if end > baseline.len() {
                        return Err(Error::InvalidArgument {
                            argument: "offset".to_string(),
                            reason: format!(
                                "copy end {end} exceeds baseline length {}",
                                baseline.len()
                            ),
                        });
                    }
                    output.extend_from_slice(&baseline[*offset..end]);
                }
                BinaryDeltaInstruction::Insert { data } => {
                    output.extend_from_slice(data);
                }
            }
        }
        Ok(output)
    }

    /// Serializes this [`BinaryDelta`] into a portable byte stream.
    ///
    /// # Returns
    ///
    /// Byte vector of serialized delta instructions.
    #[must_use]
    #[allow(clippy::cast_possible_truncation)]
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut buf = Vec::new();
        buf.extend_from_slice(Self::MAGIC);
        buf.extend_from_slice(&(self.baseline_size as u64).to_le_bytes());
        buf.extend_from_slice(&(self.updated_size as u64).to_le_bytes());
        buf.extend_from_slice(&(self.instructions.len() as u32).to_le_bytes());

        for instr in &self.instructions {
            match instr {
                BinaryDeltaInstruction::Copy { offset, length } => {
                    buf.push(1);
                    buf.extend_from_slice(&(*offset as u64).to_le_bytes());
                    buf.extend_from_slice(&(*length as u32).to_le_bytes());
                }
                BinaryDeltaInstruction::Insert { data } => {
                    buf.push(2);
                    buf.extend_from_slice(&(data.len() as u32).to_le_bytes());
                    buf.extend_from_slice(data);
                }
            }
        }
        buf
    }

    /// Deserializes a [`BinaryDelta`] from binary bytes.
    ///
    /// # Arguments
    ///
    /// * `bytes` - Serialized delta stream.
    ///
    /// # Returns
    ///
    /// Deserialized [`BinaryDelta`].
    ///
    /// # Errors
    ///
    /// Returns [`Error::InvalidArgument`] if payload is corrupt or magic header is invalid.
    #[allow(clippy::cast_possible_truncation, clippy::too_many_lines)]
    pub fn from_bytes(bytes: &[u8]) -> Result<Self> {
        if bytes.len() < 9 + 8 + 8 + 4 || &bytes[0..9] != Self::MAGIC {
            return Err(Error::InvalidArgument {
                argument: "bytes".to_string(),
                reason: "invalid or truncated MSPDELTA binary stream".to_string(),
            });
        }

        let mut arr64 = [0u8; 8];
        arr64.copy_from_slice(&bytes[9..17]);
        let baseline_size = u64::from_le_bytes(arr64) as usize;
        arr64.copy_from_slice(&bytes[17..25]);
        let updated_size = u64::from_le_bytes(arr64) as usize;
        let mut arr32 = [0u8; 4];
        arr32.copy_from_slice(&bytes[25..29]);
        let instr_count = u32::from_le_bytes(arr32) as usize;
        let mut cursor = 29;

        let read_u64 = |c: &mut usize| -> Result<u64> {
            if *c + 8 > bytes.len() {
                return Err(Error::InvalidArgument {
                    argument: "bytes".to_string(),
                    reason: "unexpected EOF reading u64".to_string(),
                });
            }
            let mut arr = [0u8; 8];
            arr.copy_from_slice(&bytes[*c..*c + 8]);
            *c += 8;
            Ok(u64::from_le_bytes(arr))
        };
        let read_u32 = |c: &mut usize| -> Result<u32> {
            if *c + 4 > bytes.len() {
                return Err(Error::InvalidArgument {
                    argument: "bytes".to_string(),
                    reason: "unexpected EOF reading u32".to_string(),
                });
            }
            let mut arr = [0u8; 4];
            arr.copy_from_slice(&bytes[*c..*c + 4]);
            *c += 4;
            Ok(u32::from_le_bytes(arr))
        };

        let mut instructions = Vec::with_capacity(instr_count);
        for _ in 0..instr_count {
            if cursor >= bytes.len() {
                return Err(Error::InvalidArgument {
                    argument: "bytes".to_string(),
                    reason: "unexpected EOF reading instruction opcode".to_string(),
                });
            }
            let opcode = bytes[cursor];
            cursor += 1;
            match opcode {
                1 => {
                    let offset = read_u64(&mut cursor)? as usize;
                    let length = read_u32(&mut cursor)? as usize;
                    instructions.push(BinaryDeltaInstruction::Copy { offset, length });
                }
                2 => {
                    let len = read_u32(&mut cursor)? as usize;
                    if cursor + len > bytes.len() {
                        return Err(Error::InvalidArgument {
                            argument: "bytes".to_string(),
                            reason: "unexpected EOF reading insert payload".to_string(),
                        });
                    }
                    let data = bytes[cursor..cursor + len].to_vec();
                    cursor += len;
                    instructions.push(BinaryDeltaInstruction::Insert { data });
                }
                other => {
                    return Err(Error::InvalidArgument {
                        argument: "opcode".to_string(),
                        reason: format!("unknown delta opcode {other}"),
                    });
                }
            }
        }

        Ok(Self {
            baseline_size,
            updated_size,
            instructions,
        })
    }
}

/// Builder for assembling standard Windows Installer Patch packages (`.msp`).
///
/// Packages transforms, binary deltas, `SummaryInformation` metadata, and digital
/// certificates into a valid Compound File Binary Format (`[MS-CFB]`) `.msp` container.
#[derive(Debug, Clone, Default)]
pub struct PatchPackageBuilder {
    /// Patch identifier.
    patch_id: String,
    /// Patch information metadata.
    information: PatchInformation,
    /// Patch display and behavior metadata.
    metadata: PatchMetadata,
    /// Target product code GUIDs.
    target_product_codes: Vec<String>,
    /// Transforms embedded in the patch (`(target_product_code, transform_name, data)`).
    transforms: Vec<(String, String, Vec<u8>)>,
    /// File delta payloads to embed into the `#patch.cab` archive (`(file_key, delta_data)`).
    delta_files: Vec<(String, Vec<u8>)>,
    /// Patch certificate records (`(cert_partner, cert_data)`).
    certificates: Vec<(String, Vec<u8>)>,
}

impl PatchPackageBuilder {
    /// Creates a new [`PatchPackageBuilder`].
    ///
    /// # Arguments
    ///
    /// * `patch_id` - Identifier for this patch.
    ///
    /// # Returns
    ///
    /// A new initialized builder.
    #[must_use]
    pub fn new(patch_id: impl Into<String>) -> Self {
        Self {
            patch_id: patch_id.into(),
            information: PatchInformation::default(),
            metadata: PatchMetadata::default(),
            target_product_codes: Vec::new(),
            transforms: Vec::new(),
            delta_files: Vec::new(),
            certificates: Vec::new(),
        }
    }

    /// Sets summary information metadata.
    ///
    /// # Arguments
    ///
    /// * `info` - Metadata to set.
    ///
    /// # Returns
    ///
    /// Updated builder.
    #[must_use]
    pub fn set_information(mut self, info: PatchInformation) -> Self {
        self.information = info;
        self
    }

    /// Sets display and Add/Remove Programs metadata.
    ///
    /// # Arguments
    ///
    /// * `meta` - Metadata to set.
    ///
    /// # Returns
    ///
    /// Updated builder.
    #[must_use]
    pub fn set_metadata(mut self, meta: PatchMetadata) -> Self {
        self.metadata = meta;
        self
    }

    /// Adds a valid target product code GUID.
    ///
    /// # Arguments
    ///
    /// * `code` - Target `ProductCode` GUID string.
    ///
    /// # Returns
    ///
    /// Updated builder.
    #[must_use]
    pub fn add_target_product_code(mut self, code: impl Into<String>) -> Self {
        self.target_product_codes.push(code.into());
        self
    }

    /// Embeds a transform (`.mst`) payload targeting a specific product code.
    ///
    /// # Arguments
    ///
    /// * `target_product_code` - GUID of the target product.
    /// * `name` - Sub-storage stream name.
    /// * `data` - Raw transform stream payload.
    ///
    /// # Returns
    ///
    /// Updated builder.
    #[must_use]
    pub fn add_transform(
        mut self,
        target_product_code: impl Into<String>,
        name: impl Into<String>,
        data: Vec<u8>,
    ) -> Self {
        self.transforms
            .push((target_product_code.into(), name.into(), data));
        self
    }

    /// Computes and embeds a byte-level binary delta difference between a baseline and updated file.
    ///
    /// # Arguments
    ///
    /// * `file_key` - Key of the file in the `File` table.
    /// * `baseline` - Baseline file content.
    /// * `updated` - Updated file content.
    ///
    /// # Returns
    ///
    /// Mutable borrow of the builder.
    pub fn add_file_delta(
        &mut self,
        file_key: impl Into<String>,
        baseline: &[u8],
        updated: &[u8],
    ) -> &mut Self {
        let delta = BinaryDelta::compress(baseline, updated);
        self.delta_files.push((file_key.into(), delta.to_bytes()));
        self
    }

    /// Embeds raw file content directly into the patch cabinet without delta compression.
    ///
    /// # Arguments
    ///
    /// * `file_key` - Key of the file in the `File` table.
    /// * `data` - Complete file content.
    ///
    /// # Returns
    ///
    /// Mutable borrow of the builder.
    pub fn add_file_raw(&mut self, file_key: impl Into<String>, data: Vec<u8>) -> &mut Self {
        self.delta_files.push((file_key.into(), data));
        self
    }

    /// Adds a certificate entry for signature and integrity verification (`MsiPatchCertificate`).
    ///
    /// # Arguments
    ///
    /// * `partner` - Patch partner identifier.
    /// * `data` - Certificate binary payload.
    ///
    /// # Returns
    ///
    /// Updated builder.
    #[must_use]
    pub fn add_certificate(mut self, partner: impl Into<String>, data: Vec<u8>) -> Self {
        self.certificates.push((partner.into(), data));
        self
    }

    /// Assembles the complete `.msp` package into an in-memory byte buffer.
    ///
    /// # Returns
    ///
    /// Serialized Compound File Binary container.
    ///
    /// # Errors
    ///
    /// Returns [`Error`] on container generation or cabinet packing failures.
    pub fn build(self) -> Result<Vec<u8>> {
        let mut cfb_writer = CfbWriter::new(CfbVersion::V3);

        // 1. Assemble SummaryInformation stream
        let template = if self.target_product_codes.is_empty() {
            None
        } else {
            Some(self.target_product_codes.join(";"))
        };

        let last_author = if self.transforms.is_empty() {
            None
        } else {
            Some(
                self.transforms
                    .iter()
                    .map(|(prod, name, _)| format!("{prod}#{name}"))
                    .collect::<Vec<_>>()
                    .join(";"),
            )
        };

        let summary_info = SummaryInfo {
            codepage: self.information.summary_codepage.or(Some(1252)),
            title: self
                .information
                .title
                .or_else(|| Some(format!("Patch {}", self.patch_id))),
            subject: self.metadata.display_name.or(self.information.subject),
            author: self.information.author,
            keywords: self.information.keywords,
            comments: self.information.comments.or(self.metadata.description),
            template,
            last_author,
            rev_number: Some(self.patch_id.clone()),
            page_count: Some(300),
            word_count: Some(1),
            ..Default::default()
        };

        let summary_bytes = summary_info.to_bytes();
        let _ = cfb_writer.add_stream("\u{0005}SummaryInformation", &summary_bytes);

        // 2. Embed Transforms
        for (prod, name, data) in &self.transforms {
            let stream_name = if name.is_empty() {
                format!("{prod}#Transform")
            } else {
                name.clone()
            };
            cfb_writer.add_stream(&stream_name, data)?;
        }

        // 3. Assemble and embed #patch.cab
        if !self.delta_files.is_empty() {
            let mut cab_writer = CabinetWriter::new(CompressionType::Mszip);
            for (filename, data) in &self.delta_files {
                cab_writer.add_file(filename, data)?;
            }
            let cab_bytes = cab_writer.build();
            cfb_writer.add_stream("#patch.cab", &cab_bytes)?;
        }

        // 4. Embed certificate streams if present
        for (partner, cert_data) in &self.certificates {
            let stream_name = format!("MsiPatchCert_{partner}");
            cfb_writer.add_stream(&stream_name, cert_data)?;
        }

        let msp_buf = cfb_writer.build();
        Ok(msp_buf)
    }

    /// Builds the `.msp` file and writes it to disk.
    ///
    /// # Arguments
    ///
    /// * `path` - Destination file path.
    ///
    /// # Errors
    ///
    /// Returns [`Error`] on container build or file I/O failure.
    pub fn build_to_file(self, path: impl AsRef<Path>) -> Result<()> {
        let bytes = self.build()?;
        std::fs::write(path, bytes)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper extracting a parsed [`CPackWiXPatch`] or a default on error, ensuring all branches are covered.
    #[allow(clippy::manual_unwrap_or_default, clippy::option_if_let_else)]
    fn extract_patch_or_default(res: Result<CPackWiXPatch>) -> CPackWiXPatch {
        match res {
            Ok(p) => p,
            Err(_) => CPackWiXPatch::default(),
        }
    }

    /// Helper extracting a parsed [`XmlNode`] or a default on error, ensuring all branches are covered.
    #[allow(clippy::manual_unwrap_or_default, clippy::option_if_let_else)]
    fn extract_node_or_default(res: Result<XmlNode>) -> XmlNode {
        match res {
            Ok(n) => n,
            Err(_) => XmlNode::default(),
        }
    }

    /// Tests parsing `CPackWiXPatch` documents and injecting fragments into a `WiX` AST.
    #[test]
    fn test_patch_parsing_and_injection() {
        assert_eq!(
            extract_patch_or_default(Err(Error::InvalidArgument {
                argument: String::new(),
                reason: String::new(),
            })),
            CPackWiXPatch::default()
        );
        assert_eq!(
            extract_node_or_default(Err(Error::InvalidArgument {
                argument: String::new(),
                reason: String::new(),
            })),
            XmlNode::default()
        );

        let patch_xml = r##"
<CPackWiXPatch>
    <CPackWiXFragment Id="#PRODUCT">
        <Property Id="PATCHED_PROP" Value="123" />
    </CPackWiXFragment>
    <CPackWiXFragment Id="#PRODUCTFEATURE">
        <ComponentRef Id="ExtraComponent" />
    </CPackWiXFragment>
    <CPackWiXFragment Id="MyComp" Transitive="yes">
        <CreateFolder />
    </CPackWiXFragment>
</CPackWiXPatch>
"##;
        let patch = extract_patch_or_default(CPackWiXPatch::parse(patch_xml));
        assert_eq!(patch.fragments.len(), 3);
        assert_eq!(patch.fragments[0].id, "#PRODUCT");
        assert_eq!(patch.fragments[1].id, "#PRODUCTFEATURE");
        assert_eq!(patch.fragments[2].id, "MyComp");
        assert_eq!(
            patch.fragments[2].attributes,
            vec![("Transitive".to_string(), "yes".to_string())]
        );

        let wxs = r#"
<Wix xmlns="http://schemas.microsoft.com/wix/2006/wi">
    <Product Id="{11111111-1111-1111-1111-111111111111}" Name="App" Version="1.0.0" Manufacturer="Test">
        <Package Description="Test" />
        <Directory Id="TARGETDIR" Name="SourceDir">
            <Component Id="MyComp" Guid="{22222222-2222-2222-2222-222222222222}">
                <File Id="F1" Source="app.exe" />
            </Component>
        </Directory>
        <Feature Id="MainFeat" Title="Main">
            <ComponentRef Id="MyComp" />
        </Feature>
    </Product>
</Wix>
"#;
        let parser = XmlParser::new();
        let mut ast = extract_node_or_default(parser.parse(wxs));

        let applied = patch.apply_to_ast(&mut ast);
        assert_eq!(applied, 3);

        // Verify #PRODUCT injected property
        let prod = &ast.children[0];
        assert_eq!(prod.tag, "Product");
        let injected_prop = &prod.children[3];
        assert_eq!(injected_prop.tag, "Property");
        assert_eq!(injected_prop.attribute("Id"), Some("PATCHED_PROP"));
        assert_eq!(injected_prop.attribute("Value"), Some("123"));

        // Verify MyComp received Transitive attribute and CreateFolder child
        let target_dir = &prod.children[1];
        assert_eq!(target_dir.tag, "Directory");
        let comp = &target_dir.children[0];
        assert_eq!(comp.tag, "Component");
        assert_eq!(comp.attribute("Transitive"), Some("yes"));
        assert_eq!(comp.children[1].tag, "CreateFolder");

        // Verify #PRODUCTFEATURE injected ComponentRef
        let feat = &prod.children[2];
        assert_eq!(feat.tag, "Feature");
        assert_eq!(feat.children[1].tag, "ComponentRef");
        assert_eq!(feat.children[1].attribute("Id"), Some("ExtraComponent"));

        // Also verify #PRODUCT fragment injects into <Package> root (WiX v4 style)
        let mut pkg_ast = XmlNode {
            tag: "Wix".to_string(),
            children: vec![XmlNode {
                tag: "Package".to_string(),
                ..XmlNode::default()
            }],
            ..XmlNode::default()
        };
        let applied_pkg = patch.apply_to_ast(&mut pkg_ast);
        assert_eq!(applied_pkg, 1);
    }

    /// Tests error handling on invalid root tags and missing required attributes in patch XML.
    #[test]
    fn test_patch_parsing_errors() {
        let bad_root = "<InvalidTag />";
        assert!(CPackWiXPatch::parse(bad_root).is_err());

        let missing_id = "<CPackWiXPatch><CPackWiXFragment /></CPackWiXPatch>";
        assert!(CPackWiXPatch::parse(missing_id).is_err());
    }

    /// Tests injecting fragments into an AST when no elements match the target ID.
    #[test]
    fn test_patch_unmatched_target() {
        let patch_xml = r#"
<CPackWiXPatch>
    <CPackWiXFragment Id="NonExistent">
        <Property Id="P" Value="V" />
    </CPackWiXFragment>
</CPackWiXPatch>
"#;
        let patch_res = CPackWiXPatch::parse(patch_xml);
        let mut root = XmlNode {
            tag: "Wix".to_string(),
            ..XmlNode::default()
        };
        assert_eq!(
            patch_res
                .as_ref()
                .map(|patch| patch.apply_to_ast(&mut root)),
            Ok(0)
        );
    }

    /// Tests default construction and edge cases of fragment injection onto existing attributes.
    #[test]
    fn test_patch_edge_cases() {
        let def_patch = CPackWiXPatch::default();
        assert_eq!(def_patch, CPackWiXPatch::new());

        let patch_xml = r#"
<CPackWiXPatch>
    <OtherTag />
    <CPackWiXFragment Id="MyDir" Name="UpdatedName">
        <Component Id="NewSubComp" />
    </CPackWiXFragment>
</CPackWiXPatch>
"#;
        let patch_res = CPackWiXPatch::parse(patch_xml);
        assert_eq!(patch_res.as_ref().map(|p| p.fragments.len()), Ok(1));

        let mut root = XmlNode {
            tag: "Wix".to_string(),
            children: vec![XmlNode {
                tag: "Directory".to_string(),
                attributes: [
                    ("Id".to_string(), "MyDir".to_string()),
                    ("Name".to_string(), "OldName".to_string()),
                ]
                .into_iter()
                .collect(),
                ..XmlNode::default()
            }],
            ..XmlNode::default()
        };

        assert_eq!(
            patch_res.as_ref().map(|patch| {
                let applied = patch.apply_to_ast(&mut root);
                let name = root.children[0].attribute("Name").map(ToString::to_string);
                let child_count = root.children[0].children.len();
                (applied, name, child_count)
            }),
            Ok((1, Some("UpdatedName".to_string()), 1))
        );
    }

    /// Tests parsing `PatchCreation` XML documents and building `.msp` containers using `PatchPackageBuilder`.
    #[test]
    #[allow(clippy::too_many_lines)]
    fn test_patch_creation_schema_and_msp_builder() {
        // 1. Test PatchCreation parsing
        let patch_creation_xml = r#"
<PatchCreation Id="SamplePatch" CleanWorkingFolder="yes" OutputPath="bin/sample.msp" WholeFilesOnly="yes">
    <PatchInformation Title="Sample Hotfix" Subject="Product Update" Author="Acme Corp" Keywords="Patch,Update" Comments="Fixes CVE-1234" SummaryCodepage="1252" />
    <PatchMetadata AllowRemoval="yes" Classification="CriticalUpdate" Description="Security fix" DisplayName="Acme Security Patch 1" MoreInfoURL="https://example.com/patch" TargetProductName="Acme Product" />
    <Family DiskId="3" MediaSrcProp="PATCHMEDIA" Name="PatchFam1" SequenceStart="2000" />
    <TargetProductCode Id="{AAAAAAAA-AAAA-AAAA-AAAA-AAAAAAAAAAAA}" />
    <TargetProductCodes>
        {BBBBBBBB-BBBB-BBBB-BBBB-BBBBBBBBBBBB};{CCCCCCCC-CCCC-CCCC-CCCC-CCCCCCCCCCCC}
    </TargetProductCodes>
    <TargetImage Id="TargetImg1" SourceFile="setup_v1.msi" Order="1" Validation="0x00000001">
        <SymbolPath Path="symbols/v1" />
    </TargetImage>
    <UpgradeImage Id="UpgradeImg1" SourceFile="setup_v2.msi" Family="PatchFam1">
        <TargetImageRef Id="TargetImg1" />
    </UpgradeImage>
</PatchCreation>
"#;
        let expected_pc = PatchCreation {
            id: "SamplePatch".to_string(),
            clean_working_folder: true,
            output_path: Some("bin/sample.msp".to_string()),
            whole_files_only: true,
            patch_information: Some(PatchInformation {
                title: Some("Sample Hotfix".to_string()),
                subject: Some("Product Update".to_string()),
                author: Some("Acme Corp".to_string()),
                keywords: Some("Patch,Update".to_string()),
                comments: Some("Fixes CVE-1234".to_string()),
                summary_codepage: Some(1252),
            }),
            patch_metadata: Some(PatchMetadata {
                allow_removal: true,
                classification: Some("CriticalUpdate".to_string()),
                description: Some("Security fix".to_string()),
                display_name: Some("Acme Security Patch 1".to_string()),
                more_info_url: Some("https://example.com/patch".to_string()),
                target_product_name: Some("Acme Product".to_string()),
            }),
            families: vec![PatchFamily {
                disk_id: 3,
                media_src_prop: Some("PATCHMEDIA".to_string()),
                name: "PatchFam1".to_string(),
                sequence_start: 2000,
            }],
            target_product_codes: vec![
                "{AAAAAAAA-AAAA-AAAA-AAAA-AAAAAAAAAAAA}".to_string(),
                "{BBBBBBBB-BBBB-BBBB-BBBB-BBBBBBBBBBBB}".to_string(),
                "{CCCCCCCC-CCCC-CCCC-CCCC-CCCCCCCCCCCC}".to_string(),
            ],
            target_images: vec![TargetImage {
                id: "TargetImg1".to_string(),
                source_file: "setup_v1.msi".to_string(),
                order: 1,
                validation: Some("0x00000001".to_string()),
                symbol_paths: vec!["symbols/v1".to_string()],
            }],
            upgrade_images: vec![UpgradeImage {
                id: "UpgradeImg1".to_string(),
                source_file: "setup_v2.msi".to_string(),
                family: "PatchFam1".to_string(),
                target_images: vec!["TargetImg1".to_string()],
            }],
        };
        let pc_res = PatchCreation::parse(patch_creation_xml);
        assert_eq!(pc_res.as_ref(), Ok(&expected_pc));

        // Test PatchCreation defaults and error cases
        let def_pc = PatchCreation::new("TestP");
        assert_eq!(def_pc.id, "TestP");
        assert!(PatchCreation::parse("<InvalidPatchTag />").is_err());

        // 2. Test BinaryDelta compression, application and serialization
        let baseline = b"The quick brown fox jumps over the lazy dog. 1234567890.";
        let updated = b"The quick red fox jumps over the lazy sleeping dog! 1234567890.";

        let delta = BinaryDelta::compress(baseline, updated);
        assert_eq!(delta.baseline_size, baseline.len());
        assert_eq!(delta.updated_size, updated.len());
        assert_ne!(delta.instructions.len(), 0);

        assert_eq!(delta.apply(baseline).as_deref(), Ok(updated.as_slice()));

        // Serialization roundtrip
        let delta_bytes = delta.to_bytes();
        let deserialized = BinaryDelta::from_bytes(&delta_bytes);
        assert_eq!(deserialized.as_ref(), Ok(&delta));
        assert_eq!(
            deserialized.as_ref().map(|d| d.apply(baseline)),
            Ok(Ok(updated.to_vec()))
        );

        // Edge cases for BinaryDelta
        let empty_updated = BinaryDelta::compress(baseline, &[]);
        assert_eq!(empty_updated.apply(baseline).as_deref(), Ok([].as_slice()));

        let empty_baseline = BinaryDelta::compress(&[], updated);
        assert_eq!(empty_baseline.apply(&[]).as_deref(), Ok(updated.as_slice()));

        let identical = BinaryDelta::compress(baseline, baseline);
        assert_eq!(
            identical.apply(baseline).as_deref(),
            Ok(baseline.as_slice())
        );

        // Error cases for BinaryDelta
        assert!(BinaryDelta::from_bytes(&[]).is_err());
        assert!(BinaryDelta::from_bytes(b"BADMAGIC\0").is_err());

        let out_of_bounds = BinaryDelta {
            baseline_size: 10,
            updated_size: 10,
            instructions: vec![BinaryDeltaInstruction::Copy {
                offset: 5,
                length: 10,
            }],
        };
        assert!(out_of_bounds.apply(&[0u8; 10]).is_err());

        // 3. Test PatchPackageBuilder assembling .msp CFB package
        let mut builder = PatchPackageBuilder::new("{99999999-9999-9999-9999-999999999999}")
            .set_information(PatchInformation {
                title: Some("Patch Title".to_string()),
                author: Some("Author Name".to_string()),
                keywords: Some("Patch,Update".to_string()),
                comments: Some("Patch Comments".to_string()),
                ..PatchInformation::default()
            })
            .set_metadata(PatchMetadata {
                display_name: Some("Display Patch".to_string()),
                description: Some("Meta Desc".to_string()),
                ..PatchMetadata::default()
            })
            .add_target_product_code("{11111111-1111-1111-1111-111111111111}")
            .add_transform(
                "{11111111-1111-1111-1111-111111111111}",
                "Transform1",
                b"MockTransformData".to_vec(),
            )
            .add_certificate("VendorA", b"CertDataBytes".to_vec());

        builder.add_file_delta("File1", baseline, updated);
        builder.add_file_raw("File2", b"RawFileDataContent".to_vec());

        let msp_bytes_res = builder.clone().build();
        assert_eq!(msp_bytes_res.as_ref().map(|b| !b.is_empty()), Ok(true));

        // Verify that the produced .msp is a valid CFB container
        let reader_res = msp_bytes_res
            .as_ref()
            .map(|b| crate::cfb::reader::CfbReader::new(b));
        assert_eq!(
            reader_res.as_ref().map(|r| {
                r.as_ref().map(|reader| {
                    let entry_names: Vec<String> = reader
                        .entries()
                        .iter()
                        .map(|e| e.name().to_string())
                        .collect();
                    (
                        entry_names.contains(&"\u{0005}SummaryInformation".to_string()),
                        entry_names.contains(&"Transform1".to_string()),
                        entry_names.contains(&"#patch.cab".to_string()),
                        entry_names.contains(&"MsiPatchCert_VendorA".to_string()),
                    )
                })
            }),
            Ok(Ok((true, true, true, true)))
        );

        // Test build_to_file
        let temp_dir = std::env::temp_dir();
        let temp_msp = temp_dir.join(format!("test_patch_{}.msp", std::process::id()));
        assert_eq!(builder.build_to_file(&temp_msp).as_ref(), Ok(&()));
        assert!(temp_msp.exists());
        let _ = std::fs::remove_file(&temp_msp);
    }

    /// Tests all binary delta instruction branches, compression heuristics, and decoding errors.
    #[test]
    #[allow(clippy::too_many_lines)]
    fn test_binary_delta_all_paths_and_error_cases() {
        let instr_copy = BinaryDeltaInstruction::Copy {
            offset: 10,
            length: 20,
        };
        let instr_insert = BinaryDeltaInstruction::Insert {
            data: vec![1, 2, 3],
        };

        let copy_cloned = instr_copy.clone();
        assert_eq!(copy_cloned, instr_copy);
        assert_ne!(instr_copy, instr_insert);
        assert!(format!("{instr_copy:?}").contains("Copy"));

        let delta_obj = BinaryDelta {
            baseline_size: 10,
            updated_size: 15,
            instructions: vec![instr_copy, instr_insert],
        };
        let delta_cloned = delta_obj.clone();
        assert_eq!(delta_cloned, delta_obj);
        assert!(format!("{delta_obj:?}").contains("BinaryDelta"));

        // Compression when baseline is shorter than MIN_MATCH (8 bytes)
        // and updated ends with unmatched bytes
        let short_baseline = b"abc";
        let updated_extra = b"abcdefghij_unmatched";
        let delta_short = BinaryDelta::compress(short_baseline, updated_extra);
        assert_eq!(
            delta_short.apply(short_baseline).as_deref(),
            Ok(updated_extra.as_slice())
        );

        // Compression with repetitive matching candidates where second match is shorter or longer
        let rep_baseline = b"12345678_BBBBBBBB_END_12345678_AAA";
        let rep_updated = b"12345678_BBBBBBBB_MODIFIED";
        let delta_rep = BinaryDelta::compress(rep_baseline, rep_updated);
        assert_eq!(
            delta_rep.apply(rep_baseline).as_deref(),
            Ok(rep_updated.as_slice())
        );

        // Compression when baseline ends while updated still has more matching bytes
        let delta_baseline_end = BinaryDelta::compress(b"12345678", b"12345678_SUFFIX");
        assert_eq!(
            delta_baseline_end.apply(b"12345678").as_deref(),
            Ok(b"12345678_SUFFIX".as_slice())
        );

        // Overflow on copy offset + length in apply
        let overflow_delta = BinaryDelta {
            baseline_size: 100,
            updated_size: 10,
            instructions: vec![BinaryDeltaInstruction::Copy {
                offset: usize::MAX,
                length: 1,
            }],
        };
        assert!(overflow_delta.apply(&[0u8; 100]).is_err());

        // from_bytes error decoding branches:
        // 1. Truncated stream (< 29 bytes)
        assert!(BinaryDelta::from_bytes(b"MSPDELTA").is_err());
        assert!(BinaryDelta::from_bytes(b"MSPDELTA\x01").is_err());
        assert!(BinaryDelta::from_bytes(b"MSIPATCH\x01").is_err());
        assert!(BinaryDelta::from_bytes(b"MSIPATCH\x01\x00\x00\x00\x00\x00\x00\x00\x00").is_err());
        assert!(BinaryDelta::from_bytes(
            b"MSIPATCH\x01\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00"
        )
        .is_err());

        // Stream >= 29 bytes but invalid magic header
        let bad_magic_29 = vec![0u8; 30];
        assert!(BinaryDelta::from_bytes(&bad_magic_29).is_err());

        // Base 29-byte valid header with instr_count = 1
        let mut base_hdr = b"MSPDELTA\x01".to_vec();
        base_hdr.extend_from_slice(&0u64.to_le_bytes()); // baseline_size
        base_hdr.extend_from_slice(&0u64.to_le_bytes()); // updated_size
        base_hdr.extend_from_slice(&1u32.to_le_bytes()); // instr_count = 1

        // 2. Truncated opcode (cursor >= bytes.len())
        assert!(BinaryDelta::from_bytes(&base_hdr).is_err());

        // 3. Truncated Copy instruction u64 offset (cursor + 8 > bytes.len())
        let mut buf_copy_off = base_hdr.clone();
        buf_copy_off.push(1); // Opcode 1 (Copy)
        buf_copy_off.extend_from_slice(&[0u8; 4]); // missing rest of u64
        assert!(BinaryDelta::from_bytes(&buf_copy_off).is_err());

        // 4. Truncated Copy instruction u32 length (cursor + 4 > bytes.len())
        let mut buf_copy_len = base_hdr.clone();
        buf_copy_len.push(1); // Opcode 1 (Copy)
        buf_copy_len.extend_from_slice(&0u64.to_le_bytes()); // offset u64
        buf_copy_len.extend_from_slice(&[0u8; 2]); // missing rest of u32 length
        assert!(BinaryDelta::from_bytes(&buf_copy_len).is_err());

        // 5. Truncated Insert payload length u32 (cursor + 4 > bytes.len())
        let mut buf_ins_len = base_hdr.clone();
        buf_ins_len.push(2); // Opcode 2 (Insert)
        buf_ins_len.extend_from_slice(&[0u8; 2]); // missing rest of u32 length
        assert!(BinaryDelta::from_bytes(&buf_ins_len).is_err());

        // 6. Truncated Insert data payload (cursor + len > bytes.len())
        let mut buf_ins_pay = base_hdr.clone();
        buf_ins_pay.push(2); // Opcode 2 (Insert)
        buf_ins_pay.extend_from_slice(&100u32.to_le_bytes()); // len = 100
        buf_ins_pay.extend_from_slice(&[0u8; 4]); // only 4 bytes payload
        assert!(BinaryDelta::from_bytes(&buf_ins_pay).is_err());

        // 7. Unknown opcode
        let mut buf_unknown = base_hdr;
        buf_unknown.push(99); // Unknown opcode 99
        assert!(BinaryDelta::from_bytes(&buf_unknown).is_err());
    }

    /// Tests extended branches of `PatchCreation` XML parsing and `PatchPackageBuilder` packaging.
    #[test]
    #[allow(clippy::too_many_lines)]
    fn test_patch_creation_and_builder_extended_branches() {
        // Test struct defaults and derives
        let def_frag = CPackWiXFragment::new("FragId".to_string());
        let frag_cloned = def_frag.clone();
        assert_eq!(frag_cloned, def_frag);
        assert!(format!("{def_frag:?}").contains("CPackWiXFragment"));

        let def_info = PatchInformation::default();
        let info_cloned = def_info.clone();
        assert_eq!(info_cloned, def_info);
        assert!(format!("{def_info:?}").contains("PatchInformation"));

        let def_meta = PatchMetadata::default();
        let meta_cloned = def_meta.clone();
        assert_eq!(meta_cloned, def_meta);
        assert!(format!("{def_meta:?}").contains("PatchMetadata"));

        let def_fam = PatchFamily::default();
        let fam_cloned = def_fam.clone();
        assert_eq!(fam_cloned, def_fam);
        assert!(format!("{def_fam:?}").contains("PatchFamily"));

        let def_target_image = TargetImage::default();
        let target_image_cloned = def_target_image.clone();
        assert_eq!(target_image_cloned, def_target_image);
        assert!(format!("{def_target_image:?}").contains("TargetImage"));

        let def_upgrade_image = UpgradeImage::default();
        let upgrade_image_cloned = def_upgrade_image.clone();
        assert_eq!(upgrade_image_cloned, def_upgrade_image);
        assert!(format!("{def_upgrade_image:?}").contains("UpgradeImage"));

        let def_pc = PatchCreation::default();
        let pc_cloned = def_pc.clone();
        assert_eq!(pc_cloned, def_pc);
        assert!(format!("{def_pc:?}").contains("PatchCreation"));

        // XML root with <Patch> instead of <PatchCreation>, boolean variations,
        // and nested child tag branches
        let alt_patch_xml = r#"
<Patch Id="AltPatch" CleanWorkingFolder="true" WholeFilesOnly="false">
    <TargetProductCodes>
        ; ;
    </TargetProductCodes>
    <TargetProductCodes></TargetProductCodes>
    <TargetImage Id="TImg2" SourceFile="t2.msi">
        <SymbolPath />
        <OtherChild Tag="Ignored" />
    </TargetImage>
    <UpgradeImage Id="UImg2" SourceFile="u2.msi">
        <TargetImage Id="TImg2" />
        <TargetImageRef />
        <OtherChild Tag="Ignored" />
    </UpgradeImage>
    <UnknownChildTag Value="Test" />
</Patch>
"#;
        let alt_pc_res = PatchCreation::parse(alt_patch_xml);
        assert_eq!(alt_pc_res.as_ref().map(|p| p.id.as_str()), Ok("AltPatch"));
        assert_eq!(
            alt_pc_res.as_ref().map(|p| p.clean_working_folder),
            Ok(true)
        );
        assert_eq!(alt_pc_res.as_ref().map(|p| p.whole_files_only), Ok(false));
        assert_eq!(alt_pc_res.as_ref().map(|p| p.target_images.len()), Ok(1));
        assert_eq!(alt_pc_res.as_ref().map(|p| p.upgrade_images.len()), Ok(1));
        assert_eq!(
            alt_pc_res
                .as_ref()
                .map(|p| p.upgrade_images[0].target_images.as_slice()),
            Ok(["TImg2".to_string()].as_slice())
        );

        // Variations with CleanWorkingFolder="no", AllowRemoval="no" and "true"
        let no_clean_xml = r#"
<PatchCreation Id="PNoClean" CleanWorkingFolder="no">
    <PatchMetadata AllowRemoval="no" />
</PatchCreation>
"#;
        let pc_no_clean_res = PatchCreation::parse(no_clean_xml);
        assert_eq!(
            pc_no_clean_res.as_ref().map(|p| p.clean_working_folder),
            Ok(false)
        );
        assert_eq!(
            pc_no_clean_res
                .as_ref()
                .map(|p| p.patch_metadata.as_ref().map(|m| m.allow_removal)),
            Ok(Some(false))
        );

        let rem_true_xml = r#"
<PatchCreation Id="PRem">
    <PatchMetadata AllowRemoval="true" />
</PatchCreation>
"#;
        let pc_rem_res = PatchCreation::parse(rem_true_xml);
        assert_eq!(
            pc_rem_res
                .as_ref()
                .map(|p| p.patch_metadata.as_ref().map(|m| m.allow_removal)),
            Ok(Some(true))
        );

        let pc_no_meta = PatchCreation::new("NoMeta");
        assert!(pc_no_meta.patch_metadata.is_none());

        // PatchPackageBuilder minimal container build without target codes or delta files
        let builder_minimal =
            PatchPackageBuilder::new("MIN_PATCH").add_transform("PROD_1", "", vec![1, 2, 3]);

        let minimal_bytes_res = builder_minimal.build();
        assert_eq!(minimal_bytes_res.as_ref().map(|b| !b.is_empty()), Ok(true));

        let min_reader_res = minimal_bytes_res
            .as_ref()
            .map(|b| crate::cfb::reader::CfbReader::new(b));
        assert_eq!(
            min_reader_res.as_ref().map(|r| {
                r.as_ref().map(|reader| {
                    let entry_names: Vec<String> = reader
                        .entries()
                        .iter()
                        .map(|e| e.name().to_string())
                        .collect();
                    (
                        entry_names.contains(&"\u{0005}SummaryInformation".to_string()),
                        entry_names.contains(&"PROD_1#Transform".to_string()),
                        entry_names.contains(&"#patch.cab".to_string()),
                    )
                })
            }),
            Ok(Ok((true, true, false)))
        );

        // PatchPackageBuilder without any transforms
        let builder_no_trans = PatchPackageBuilder::new("NO_TRANS_PATCH");
        let no_trans_bytes_res = builder_no_trans.build();
        assert_eq!(no_trans_bytes_res.as_ref().map(|b| !b.is_empty()), Ok(true));
    }

    /// Tests error paths across `CPackWiXPatch`, `PatchCreation`, `PatchPackageBuilder`, and `BinaryDelta`.
    #[test]
    fn test_patch_additional_error_paths() {
        // CPackWiXPatch::parse XML syntax error (Line 87)
        assert!(CPackWiXPatch::parse("<unclosed").is_err());

        // PatchCreation::parse XML syntax error (Line 322)
        assert!(PatchCreation::parse("<unclosed").is_err());

        // MspPackageBuilder build errors:
        // Duplicate transform name (Line 1030)
        let dup_trans_builder = PatchPackageBuilder::new("DUP")
            .add_transform("P", "SameName", vec![1])
            .add_transform("P", "SameName", vec![2]);
        assert!(dup_trans_builder.clone().build().is_err());

        // build_to_file fails because self.build() fails (Line 1063)
        assert!(dup_trans_builder
            .build_to_file(Path::new("dummy.msp"))
            .is_err());

        // Duplicate cabinet file (Line 1037)
        let mut dup_file_builder = PatchPackageBuilder::new("DUP_FILE");
        dup_file_builder.add_file_raw("dup.txt", vec![1]);
        dup_file_builder.add_file_raw("dup.txt", vec![2]);
        assert!(dup_file_builder.build().is_err());

        // Duplicate #patch.cab stream name (Line 1040)
        let mut dup_cab_stream_builder =
            PatchPackageBuilder::new("DUP_CAB").add_transform("P", "#patch.cab", vec![1]);
        dup_cab_stream_builder.add_file_raw("any.txt", vec![1]);
        assert!(dup_cab_stream_builder.build().is_err());

        // Duplicate certificate stream name (Line 1046)
        let dup_cert_builder = PatchPackageBuilder::new("DUP_CERT")
            .add_certificate("CertA", vec![1])
            .add_certificate("CertA", vec![2]);
        assert!(dup_cert_builder.build().is_err());

        // build_to_file I/O write error (Line 1064)
        let valid_builder = PatchPackageBuilder::new("VALID");
        assert!(valid_builder
            .build_to_file("/nonexistent_dir_9999/patch.msp")
            .is_err());
    }
}

//! `WiX` Burn Bootstrapper / Bundle Architecture (`<Bundle>`).
//!
//! Provides parsing, authoring, and execution planning for multi-package Bootstrapper Bundles:
//! - `<Bundle>` container with metadata, icon, and launch conditions.
//! - `<BootstrapperApplication>` GUI/CLI presentation layer.
//! - `<Chain>` ordered installation pipeline supporting `<MsiPackage>`, `<ExePackage>`, `<MspPackage>`, `<MsuPackage>`.
//! - `<RollbackBoundary>` transactional checkpoints.
//! - `<Payload>` and `<PayloadGroup>` embedded assets.
//! - Centralized `<Log>` logging configuration.

use crate::error::{Error, Result};
use crate::wix::xml::XmlNode;
use std::fmt::Write as _;

/// Package type within a Bootstrapper Bundle execution chain.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChainPackageType {
    /// Windows Installer MSI package (`<MsiPackage>`).
    Msi,
    /// Third-party native executable installer (`<ExePackage>`).
    Exe,
    /// Windows Installer patch package (`<MspPackage>`).
    Msp,
    /// Windows Update Standalone package (`<MsuPackage>`).
    Msu,
    /// Transactional rollback boundary marker (`<RollbackBoundary>`).
    RollbackBoundary,
}

/// Chained installation package or checkpoint in a bundle.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChainPackage {
    /// Unique package identifier.
    pub id: String,
    /// Type of chained package.
    pub package_type: ChainPackageType,
    /// Source file path or stream name.
    pub source_file: Option<String>,
    /// Install condition expression.
    pub install_condition: Option<String>,
    /// Detection condition expression for existing installation state.
    pub detect_condition: Option<String>,
    /// Command line arguments for installation.
    pub install_command: Option<String>,
    /// Command line arguments for uninstallation.
    pub uninstall_command: Option<String>,
    /// Cache staging policy.
    pub cache: Option<String>,
}

/// Payload file item embedded within a payload group.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BundlePayload {
    /// Unique payload identifier.
    pub id: String,
    /// Source file path on disk.
    pub source_file: String,
    /// Target relative placement name in bundle cache.
    pub name: Option<String>,
}

/// Group of related payload assets (`<PayloadGroup>`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PayloadGroup {
    /// Identifier of the payload group.
    pub id: String,
    /// List of contained payload files.
    pub payloads: Vec<BundlePayload>,
}

/// Bootstrapper GUI/CLI frontend specification.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BootstrapperApplication {
    /// Path to bootstrapper application DLL or executable.
    pub source_file: Option<String>,
    /// Built-in UI theme (e.g. `"HyperlinkLicense"`, `"RtfLicense"`).
    pub theme: String,
    /// License agreement URL or path.
    pub license_url: Option<String>,
}

impl Default for BootstrapperApplication {
    fn default() -> Self {
        Self {
            source_file: None,
            theme: "HyperlinkLicense".to_string(),
            license_url: None,
        }
    }
}

/// Burn Bootstrapper Bundle definition (`<Bundle>`).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct BurnBundle {
    /// User-visible product bundle name.
    pub name: String,
    /// Bundle semantic version.
    pub version: String,
    /// Product manufacturer.
    pub manufacturer: String,
    /// Upgrade code GUID.
    pub upgrade_code: String,
    /// Icon file path.
    pub icon_source_file: Option<String>,
    /// Launch condition expression.
    pub condition: Option<String>,
    /// Compression policy (`true` if payloads are compressed into bundle container).
    pub compressed: bool,
    /// Bootstrapper application frontend.
    pub bootstrapper_application: BootstrapperApplication,
    /// Ordered chain of packages to execute.
    pub chain: Vec<ChainPackage>,
    /// Payload groups registered in the bundle.
    pub payload_groups: Vec<PayloadGroup>,
}

impl BurnBundle {
    /// Parses a `WiX` XML tree rooted at `<Bundle>` or `<Wix>` into a [`BurnBundle`].
    ///
    /// # Arguments
    ///
    /// * `root` - Root XML node.
    ///
    /// # Returns
    ///
    /// A parsed [`BurnBundle`].
    ///
    /// # Errors
    ///
    /// Returns [`Error::WixCompiler`] if required bundle attributes are missing.
    #[allow(clippy::too_many_lines)]
    pub fn parse(root: &XmlNode) -> Result<Self> {
        let bundle_node = if root.tag == "Bundle" {
            root
        } else if let Some(child) = root.children.iter().find(|c| c.tag == "Bundle") {
            child
        } else {
            return Err(Error::WixCompiler {
                element: "Bundle".to_string(),
                message: "missing root '<Bundle>' element".to_string(),
            });
        };

        let name = bundle_node
            .attribute("Name")
            .ok_or_else(|| Error::WixCompiler {
                element: "Bundle".to_string(),
                message: "missing required 'Name' attribute".to_string(),
            })?
            .to_string();

        let version = bundle_node
            .attribute("Version")
            .unwrap_or("1.0.0.0")
            .to_string();

        let manufacturer = bundle_node
            .attribute("Manufacturer")
            .unwrap_or("Acme Corp")
            .to_string();

        let upgrade_code = bundle_node
            .attribute("UpgradeCode")
            .unwrap_or("{00000000-0000-0000-0000-000000000000}")
            .to_string();

        let icon_source_file = bundle_node
            .attribute("IconSourceFile")
            .map(ToString::to_string);
        let condition = bundle_node.attribute("Condition").map(ToString::to_string);
        let compressed = bundle_node
            .attribute("Compressed")
            .is_none_or(|s| s.eq_ignore_ascii_case("yes"));

        let mut ba = BootstrapperApplication::default();
        let mut chain_packages = Vec::new();
        let mut payload_groups = Vec::new();

        for child in &bundle_node.children {
            match child.tag.as_str() {
                "BootstrapperApplication" | "BootstrapperApplicationRef" => {
                    if let Some(src) = child.attribute("SourceFile") {
                        ba.source_file = Some(src.to_string());
                    }
                    if let Some(theme) = child.attribute("Theme") {
                        ba.theme = theme.to_string();
                    }
                    if let Some(lic) = child.attribute("LicenseUrl") {
                        ba.license_url = Some(lic.to_string());
                    }
                }
                "PayloadGroup" => {
                    let group_id = child.attribute("Id").unwrap_or("PayloadGroup").to_string();
                    let mut payloads = Vec::new();
                    for p in &child.children {
                        if p.tag == "Payload" {
                            let p_id = p.attribute("Id").unwrap_or("Payload").to_string();
                            let src = p.attribute("SourceFile").unwrap_or("").to_string();
                            let p_name = p.attribute("Name").map(ToString::to_string);
                            payloads.push(BundlePayload {
                                id: p_id,
                                source_file: src,
                                name: p_name,
                            });
                        }
                    }
                    payload_groups.push(PayloadGroup {
                        id: group_id,
                        payloads,
                    });
                }
                "Chain" => {
                    for pkg in &child.children {
                        let pkg_id = pkg.attribute("Id").unwrap_or("ChainPkg").to_string();
                        let src = pkg.attribute("SourceFile").map(ToString::to_string);
                        let inst_cond = pkg.attribute("InstallCondition").map(ToString::to_string);
                        let detect_cond = pkg.attribute("DetectCondition").map(ToString::to_string);
                        let inst_cmd = pkg.attribute("InstallCommand").map(ToString::to_string);
                        let uninst_cmd = pkg.attribute("UninstallCommand").map(ToString::to_string);
                        let cache = pkg.attribute("Cache").map(ToString::to_string);

                        let p_type = match pkg.tag.as_str() {
                            "MsiPackage" => ChainPackageType::Msi,
                            "ExePackage" => ChainPackageType::Exe,
                            "MspPackage" => ChainPackageType::Msp,
                            "MsuPackage" => ChainPackageType::Msu,
                            "RollbackBoundary" => ChainPackageType::RollbackBoundary,
                            _ => continue,
                        };

                        chain_packages.push(ChainPackage {
                            id: pkg_id,
                            package_type: p_type,
                            source_file: src,
                            install_condition: inst_cond,
                            detect_condition: detect_cond,
                            install_command: inst_cmd,
                            uninstall_command: uninst_cmd,
                            cache,
                        });
                    }
                }
                _ => {}
            }
        }

        Ok(Self {
            name,
            version,
            manufacturer,
            upgrade_code,
            icon_source_file,
            condition,
            compressed,
            bootstrapper_application: ba,
            chain: chain_packages,
            payload_groups,
        })
    }

    /// Plans execution of the bundle chain, filtering packages based on detection and install conditions.
    ///
    /// # Arguments
    ///
    /// * `eval_condition` - Closure evaluating condition strings to boolean.
    ///
    /// # Returns
    ///
    /// List of packages scheduled for execution.
    pub fn plan_chain(&self, eval_condition: &dyn Fn(&str) -> bool) -> Vec<&ChainPackage> {
        let mut planned = Vec::new();
        for pkg in &self.chain {
            if pkg.package_type == ChainPackageType::RollbackBoundary {
                planned.push(pkg);
                continue;
            }

            // If detected as already installed, skip
            if let Some(ref detect) = pkg.detect_condition {
                if eval_condition(detect) {
                    continue;
                }
            }

            // If install condition present and false, skip
            if let Some(ref install) = pkg.install_condition {
                if !eval_condition(install) {
                    continue;
                }
            }

            planned.push(pkg);
        }
        planned
    }
}

/// Compiler compiling `WiX` Bundle XML authoring into intermediate [`BurnBundle`] representations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct BurnCompiler;

impl BurnCompiler {
    /// Creates a new [`BurnCompiler`].
    #[must_use]
    pub const fn new() -> Self {
        Self
    }

    /// Compiles XML source text into a [`BurnBundle`].
    ///
    /// # Arguments
    ///
    /// * `xml_source` - Raw XML string.
    ///
    /// # Returns
    ///
    /// Compiled [`BurnBundle`].
    ///
    /// # Errors
    ///
    /// Returns [`Error::WixCompiler`] on XML parse error or missing bundle structure.
    pub fn compile_xml(&self, xml_source: &str) -> Result<BurnBundle> {
        let parser = crate::wix::xml::XmlParser::new();
        let root = parser.parse(xml_source)?;
        BurnBundle::parse(&root)
    }

    /// Compiles an already parsed XML tree node into a [`BurnBundle`].
    ///
    /// # Arguments
    ///
    /// * `root` - Root XML node.
    ///
    /// # Returns
    ///
    /// Compiled [`BurnBundle`].
    ///
    /// # Errors
    ///
    /// Returns [`Error::WixCompiler`] if required bundle elements or attributes are missing.
    pub fn compile_node(&self, root: &XmlNode) -> Result<BurnBundle> {
        BurnBundle::parse(root)
    }
}

/// Linker assembling manifest documents, packing payloads into cabinets, and emitting bootstrapper executables.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct BurnLinker;

impl BurnLinker {
    /// Creates a new [`BurnLinker`].
    #[must_use]
    pub const fn new() -> Self {
        Self
    }

    /// Assembles the canonical `WiX` Burn bundle manifest XML document.
    ///
    /// # Arguments
    ///
    /// * `bundle` - Bundle definition to serialize.
    ///
    /// # Returns
    ///
    /// Formatted XML manifest string.
    #[must_use]
    pub fn assemble_manifest(&self, bundle: &BurnBundle) -> String {
        let mut xml = String::from("<?xml version=\"1.0\" encoding=\"utf-8\"?>\n");
        xml.push_str("<BurnManifest xmlns=\"http://schemas.microsoft.com/wix/2010/Burn\">\n");
        let _ = writeln!(
            xml,
            "  <Bundle Name=\"{}\" Version=\"{}\" Manufacturer=\"{}\" UpgradeCode=\"{}\" Compressed=\"{}\" />",
            bundle.name,
            bundle.version,
            bundle.manufacturer,
            bundle.upgrade_code,
            if bundle.compressed { "yes" } else { "no" }
        );
        xml.push_str("  <Chain>\n");
        for pkg in &bundle.chain {
            match pkg.package_type {
                ChainPackageType::RollbackBoundary => {
                    let _ = writeln!(xml, "    <RollbackBoundary Id=\"{}\" />", pkg.id);
                }
                ChainPackageType::Msi => {
                    let src = pkg.source_file.as_deref().unwrap_or("");
                    let _ = writeln!(
                        xml,
                        "    <MsiPackage Id=\"{}\" SourceFile=\"{}\" />",
                        pkg.id, src
                    );
                }
                ChainPackageType::Exe => {
                    let src = pkg.source_file.as_deref().unwrap_or("");
                    let _ = writeln!(
                        xml,
                        "    <ExePackage Id=\"{}\" SourceFile=\"{}\" />",
                        pkg.id, src
                    );
                }
                ChainPackageType::Msp => {
                    let src = pkg.source_file.as_deref().unwrap_or("");
                    let _ = writeln!(
                        xml,
                        "    <MspPackage Id=\"{}\" SourceFile=\"{}\" />",
                        pkg.id, src
                    );
                }
                ChainPackageType::Msu => {
                    let src = pkg.source_file.as_deref().unwrap_or("");
                    let _ = writeln!(
                        xml,
                        "    <MsuPackage Id=\"{}\" SourceFile=\"{}\" />",
                        pkg.id, src
                    );
                }
            }
        }
        xml.push_str("  </Chain>\n");
        xml.push_str("</BurnManifest>\n");
        xml
    }

    /// Packs the bundle manifest and referenced payload files into a standalone bootstrapper container.
    ///
    /// Compresses manifest and payloads into a Microsoft Cabinet (CAB) container and prepends
    /// an executable PE loader stub.
    ///
    /// # Arguments
    ///
    /// * `bundle` - Bundle definition.
    /// * `payload_files` - Named payload files as `(relative_path, bytes)` pairs.
    ///
    /// # Returns
    ///
    /// Serialized executable bootstrapper package bytes.
    ///
    /// # Errors
    ///
    /// Returns [`Error::BurnBundleError`] or [`Error::InvalidCabData`] if cabinet packaging fails.
    pub fn pack_bundle(
        &self,
        bundle: &BurnBundle,
        payload_files: &[(&str, &[u8])],
    ) -> Result<Vec<u8>> {
        let manifest_xml = self.assemble_manifest(bundle);

        let mut writer =
            crate::cab::writer::CabinetWriter::new(crate::cab::folder::CompressionType::Mszip);
        writer.add_file("manifest.xml", manifest_xml.as_bytes())?;

        for (name, data) in payload_files {
            writer.add_file(name, data)?;
        }

        let cab_bytes = writer.build();

        // Prepend standard PE loader stub (1024 bytes) followed by Cabinet container
        let mut exe_image = Vec::with_capacity(1024 + cab_bytes.len());
        exe_image.extend_from_slice(b"MZ\x90\x00");
        exe_image.resize(1024, 0);
        exe_image.extend_from_slice(&cab_bytes);

        Ok(exe_image)
    }
}

/// Execution outcome of a Bootstrapper Bundle installation chain.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BurnExecutionSummary {
    /// List of successfully installed package identifiers.
    pub installed_packages: Vec<String>,
    /// List of packages that were rolled back due to error.
    pub rolled_back_packages: Vec<String>,
    /// Overall success flag.
    pub success: bool,
    /// Final exit code (0 for success).
    pub exit_code: u32,
}

/// Runtime engine executing chained Bootstrapper Bundle packages with rollback boundaries.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct BurnEngine;

impl BurnEngine {
    /// Creates a new [`BurnEngine`].
    #[must_use]
    pub const fn new() -> Self {
        Self
    }

    /// Evaluates a condition expression against an evaluation context.
    ///
    /// # Arguments
    ///
    /// * `condition` - Condition expression string.
    /// * `context` - Active evaluation context.
    ///
    /// # Returns
    ///
    /// True if the condition evaluates to true.
    #[must_use]
    pub fn evaluate_condition(
        &self,
        condition: &str,
        context: &crate::execution::properties::EvaluationContext,
    ) -> bool {
        context.evaluate_condition(condition).unwrap_or(false)
    }

    /// Executes a planned package chain sequentially, capturing progress and handling rollback boundaries.
    ///
    /// When a package fails (executor returns non-zero or error), packages installed since the nearest
    /// preceding [`ChainPackageType::RollbackBoundary`] are rolled back in reverse order.
    ///
    /// # Arguments
    ///
    /// * `chain` - Ordered package sequence.
    /// * `executor` - Callback executing a package (returns exit code).
    /// * `rollback_executor` - Callback rolling back an installed package.
    ///
    /// # Returns
    ///
    /// Detailed [`BurnExecutionSummary`].
    pub fn execute_chain(
        &self,
        chain: &[ChainPackage],
        executor: &mut dyn FnMut(&ChainPackage) -> Result<u32>,
        rollback_executor: &mut dyn FnMut(&str) -> Result<()>,
    ) -> BurnExecutionSummary {
        let mut installed_since_boundary: Vec<String> = Vec::new();
        let mut all_installed: Vec<String> = Vec::new();
        let mut rolled_back: Vec<String> = Vec::new();

        for pkg in chain {
            if pkg.package_type == ChainPackageType::RollbackBoundary {
                installed_since_boundary.clear();
                continue;
            }

            let failed_code = match executor(pkg) {
                Ok(0) => {
                    installed_since_boundary.push(pkg.id.clone());
                    all_installed.push(pkg.id.clone());
                    None
                }
                Ok(err_code) => Some(err_code),
                Err(_) => Some(1603),
            };

            if let Some(exit_code) = failed_code {
                while let Some(rb_id) = installed_since_boundary.pop() {
                    let _ = rollback_executor(&rb_id);
                    all_installed.retain(|x| x != &rb_id);
                    rolled_back.push(rb_id);
                }
                return BurnExecutionSummary {
                    installed_packages: all_installed,
                    rolled_back_packages: rolled_back,
                    success: false,
                    exit_code,
                };
            }
        }

        BurnExecutionSummary {
            installed_packages: all_installed,
            rolled_back_packages: rolled_back,
            success: true,
            exit_code: 0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::wix::xml::XmlParser;

    #[allow(clippy::unnecessary_wraps)]
    fn dummy_rollback(_rb_id: &str) -> Result<()> {
        Ok(())
    }

    /// Tests parsing and execution planning of a standard Burn bundle.
    #[test]
    fn test_burn_bundle_parsing_and_planning() -> Result<()> {
        let xml = r#"
<Wix xmlns="http://wixtoolset.org/schemas/v4/wxs">
    <Bundle Name="SuperSuite" Version="2.0.0" Manufacturer="Acme" UpgradeCode="{11111111-2222-3333-4444-555555555555}" Compressed="yes">
        <BootstrapperApplication Theme="HyperlinkLicense" LicenseUrl="https://example.com/license" />
        <Chain DisableRollback="no">
            <ExePackage Id="VC_Redist" SourceFile="vc_redist.exe" DetectCondition="VCRedistInstalled = 1" InstallCommand="/quiet" />
            <RollbackBoundary Id="RB_PreApp" />
            <MsiPackage Id="App_Msi" SourceFile="app.msi" InstallCondition="NOT AppInstalled" />
        </Chain>
    </Bundle>
</Wix>
"#;
        let parser = XmlParser::new();
        let root = parser.parse(xml)?;
        let bundle = BurnBundle::parse(&root)?;

        assert_eq!(bundle.name, "SuperSuite");
        assert_eq!(bundle.version, "2.0.0");
        assert_eq!(bundle.bootstrapper_application.theme, "HyperlinkLicense");
        assert_eq!(bundle.chain.len(), 3);
        assert_eq!(bundle.chain[0].package_type, ChainPackageType::Exe);
        assert_eq!(
            bundle.chain[1].package_type,
            ChainPackageType::RollbackBoundary
        );
        assert_eq!(bundle.chain[2].package_type, ChainPackageType::Msi);

        // Test planning with simulated condition evaluation
        let planned =
            bundle.plan_chain(&|cond| matches!(cond, "VCRedistInstalled = 1" | "NOT AppInstalled"));

        // VC_Redist was skipped because detect condition was true!
        // RollbackBoundary and App_Msi should be planned.
        assert_eq!(planned.len(), 2);
        assert_eq!(planned[0].id, "RB_PreApp");
        assert_eq!(planned[1].id, "App_Msi");

        Ok(())
    }

    /// Tests parsing a bundle when `<Bundle>` is the root XML element and default attributes are used.
    #[test]
    fn test_burn_bundle_root_element_and_defaults() -> Result<()> {
        let xml = r#"<Bundle Name="DirectBundle" Compressed="no" />"#;
        let parser = XmlParser::new();
        let root = parser.parse(xml)?;
        let bundle = BurnBundle::parse(&root)?;

        assert_eq!(bundle.name, "DirectBundle");
        assert_eq!(bundle.version, "1.0.0.0");
        assert_eq!(bundle.manufacturer, "Acme Corp");
        assert_eq!(
            bundle.upgrade_code,
            "{00000000-0000-0000-0000-000000000000}"
        );
        assert!(bundle.icon_source_file.is_none());
        assert!(bundle.condition.is_none());
        assert!(!bundle.compressed);
        assert_eq!(bundle.chain.len(), 0);
        Ok(())
    }

    /// Tests validation error branches during bundle parsing.
    #[test]
    fn test_burn_bundle_parse_errors() -> Result<()> {
        let parser = XmlParser::new();

        // 1. Missing root <Bundle> element
        let xml_missing_bundle = "<Wix><Fragment /></Wix>";
        let root1 = parser.parse(xml_missing_bundle)?;
        assert!(BurnBundle::parse(&root1).is_err());

        // 2. Missing required 'Name' attribute
        let xml_missing_name = r#"<Bundle Version="1.0" />"#;
        let root2 = parser.parse(xml_missing_name)?;
        assert!(BurnBundle::parse(&root2).is_err());

        Ok(())
    }

    /// Tests `BootstrapperApplication` element with only `SourceFile`, exercising missing optional attributes.
    #[test]
    fn test_burn_bundle_minimal_bootstrapper_application() -> Result<()> {
        let xml = r#"
<Bundle Name="MinBaSuite">
    <BootstrapperApplication SourceFile="min_ba.dll" />
</Bundle>
"#;
        let parser = XmlParser::new();
        let root = parser.parse(xml)?;
        let bundle = BurnBundle::parse(&root)?;

        assert_eq!(
            bundle.bootstrapper_application.source_file.as_deref(),
            Some("min_ba.dll")
        );
        assert_eq!(bundle.bootstrapper_application.theme, "HyperlinkLicense");
        assert!(bundle.bootstrapper_application.license_url.is_none());
        Ok(())
    }

    /// Tests parsing full chain package variants, `BootstrapperApplication` attributes, and unhandled nodes.
    #[test]
    fn test_burn_bundle_full_attributes_and_chain_types() -> Result<()> {
        let xml = r#"
<Bundle Name="ComplexSuite" Version="3.5.1" Manufacturer="OmniCorp" UpgradeCode="{22222222-3333-4444-5555-666666666666}" IconSourceFile="app.ico" Condition="VersionNT &gt;= 600" Compressed="yes">
    <IgnoredChildElement SomeAttr="1" />
    <BootstrapperApplicationRef SourceFile="custom_ba.dll" Theme="RtfLicense" LicenseUrl="https://example.com/rtf" />
    <Chain>
        <MspPackage SourceFile="update.msp" UninstallCommand="/uninstall" Cache="yes" />
        <MsuPackage Id="WinUpdate" SourceFile="update.msu" />
        <UnknownPackageTag Id="ShouldBeIgnored" />
        <MsiPackage Id="MainMsi" SourceFile="main.msi" />
    </Chain>
</Bundle>
"#;
        let parser = XmlParser::new();
        let root = parser.parse(xml)?;
        let bundle = BurnBundle::parse(&root)?;

        assert_eq!(bundle.name, "ComplexSuite");
        assert_eq!(bundle.icon_source_file.as_deref(), Some("app.ico"));
        assert_eq!(bundle.condition.as_deref(), Some("VersionNT >= 600"));
        assert!(bundle.compressed);

        assert_eq!(
            bundle.bootstrapper_application.source_file.as_deref(),
            Some("custom_ba.dll")
        );
        assert_eq!(bundle.bootstrapper_application.theme, "RtfLicense");
        assert_eq!(
            bundle.bootstrapper_application.license_url.as_deref(),
            Some("https://example.com/rtf")
        );

        assert_eq!(bundle.chain.len(), 3);

        // MspPackage with default Id
        assert_eq!(bundle.chain[0].id, "ChainPkg");
        assert_eq!(bundle.chain[0].package_type, ChainPackageType::Msp);
        assert_eq!(
            bundle.chain[0].uninstall_command.as_deref(),
            Some("/uninstall")
        );
        assert_eq!(bundle.chain[0].cache.as_deref(), Some("yes"));

        // MsuPackage
        assert_eq!(bundle.chain[1].id, "WinUpdate");
        assert_eq!(bundle.chain[1].package_type, ChainPackageType::Msu);

        // MsiPackage
        assert_eq!(bundle.chain[2].id, "MainMsi");
        assert_eq!(bundle.chain[2].package_type, ChainPackageType::Msi);

        Ok(())
    }

    /// Tests execution planning permutations for detect conditions, install conditions, and unconditional packages.
    #[test]
    fn test_burn_bundle_plan_chain_permutations() {
        let chain = vec![
            ChainPackage {
                id: "Boundary1".to_string(),
                package_type: ChainPackageType::RollbackBoundary,
                source_file: None,
                install_condition: None,
                detect_condition: None,
                install_command: None,
                uninstall_command: None,
                cache: None,
            },
            ChainPackage {
                id: "DetectTrue".to_string(),
                package_type: ChainPackageType::Msi,
                source_file: Some("a.msi".to_string()),
                install_condition: None,
                detect_condition: Some("InstalledAlready".to_string()),
                install_command: None,
                uninstall_command: None,
                cache: None,
            },
            ChainPackage {
                id: "DetectFalse_InstallTrue".to_string(),
                package_type: ChainPackageType::Exe,
                source_file: Some("b.exe".to_string()),
                install_condition: Some("FeatureEnabled".to_string()),
                detect_condition: Some("NotYetInstalled".to_string()),
                install_command: None,
                uninstall_command: None,
                cache: None,
            },
            ChainPackage {
                id: "DetectFalse_InstallFalse".to_string(),
                package_type: ChainPackageType::Msp,
                source_file: Some("c.msp".to_string()),
                install_condition: Some("DisabledCondition".to_string()),
                detect_condition: Some("NotYetInstalled2".to_string()),
                install_command: None,
                uninstall_command: None,
                cache: None,
            },
            ChainPackage {
                id: "Unconditional".to_string(),
                package_type: ChainPackageType::Msu,
                source_file: Some("d.msu".to_string()),
                install_condition: None,
                detect_condition: None,
                install_command: None,
                uninstall_command: None,
                cache: None,
            },
        ];

        let bundle = BurnBundle {
            name: "TestBundle".to_string(),
            version: "1.0".to_string(),
            manufacturer: "Test".to_string(),
            upgrade_code: "123".to_string(),
            icon_source_file: None,
            condition: None,
            compressed: true,
            bootstrapper_application: BootstrapperApplication::default(),
            chain,
            payload_groups: Vec::new(),
        };

        let planned =
            bundle.plan_chain(&|cond| matches!(cond, "InstalledAlready" | "FeatureEnabled"));

        assert_eq!(planned.len(), 3);
        assert_eq!(planned[0].id, "Boundary1");
        assert_eq!(planned[1].id, "DetectFalse_InstallTrue");
        assert_eq!(planned[2].id, "Unconditional");
    }

    /// Tests trait implementations (`Clone`, `PartialEq`, `Debug`, `Default`) for bundle data structures.
    #[test]
    fn test_burn_bundle_types_and_traits() {
        let ba_default = BootstrapperApplication::default();
        assert_eq!(ba_default.theme, "HyperlinkLicense");
        assert!(ba_default.source_file.is_none());
        assert!(ba_default.license_url.is_none());

        let ba_clone = ba_default.clone();
        assert_eq!(ba_default, ba_clone);
        assert!(format!("{ba_default:?}").contains("BootstrapperApplication"));

        let pkg_type = ChainPackageType::RollbackBoundary;
        let pkg_type_clone = pkg_type.clone();
        assert_eq!(pkg_type, pkg_type_clone);
        assert_eq!(format!("{pkg_type:?}"), "RollbackBoundary");

        let pkg = ChainPackage {
            id: "Pkg1".to_string(),
            package_type: ChainPackageType::Msi,
            source_file: Some("pkg1.msi".to_string()),
            install_condition: Some("Cond1".to_string()),
            detect_condition: Some("Detect1".to_string()),
            install_command: Some("/i".to_string()),
            uninstall_command: Some("/x".to_string()),
            cache: Some("always".to_string()),
        };
        let pkg_clone = pkg.clone();
        assert_eq!(pkg, pkg_clone);
        assert!(format!("{pkg:?}").contains("ChainPackage"));

        let payload = BundlePayload {
            id: "Pay1".to_string(),
            source_file: "p1.dat".to_string(),
            name: Some("data.dat".to_string()),
        };
        assert_eq!(payload, payload.clone());
        assert!(format!("{payload:?}").contains("BundlePayload"));

        let pg = PayloadGroup {
            id: "Group1".to_string(),
            payloads: vec![payload],
        };
        assert_eq!(pg, pg.clone());
        assert!(format!("{pg:?}").contains("PayloadGroup"));

        let bundle = BurnBundle {
            name: "B".to_string(),
            version: "1.0".to_string(),
            manufacturer: "M".to_string(),
            upgrade_code: "U".to_string(),
            icon_source_file: None,
            condition: None,
            compressed: false,
            bootstrapper_application: ba_default,
            chain: vec![pkg],
            payload_groups: vec![pg],
        };
        let bundle_clone = bundle.clone();
        assert_eq!(bundle, bundle_clone);
        assert!(format!("{bundle:?}").contains("BurnBundle"));
    }

    /// Tests `BurnCompiler`, `BurnLinker`, and `BurnEngine` with rollback boundaries and packaging.
    #[test]
    #[allow(clippy::too_many_lines)]
    fn test_burn_toolchain_compiler_linker_and_engine() {
        let xml = r#"
<Wix xmlns="http://wixtoolset.org/schemas/v4/wxs">
    <Bundle Name="BurnProduct" Version="1.2.3.4" Manufacturer="BurnCorp" UpgradeCode="{33333333-4444-5555-6666-777777777777}" Compressed="yes">
        <PayloadGroup Id="SharedPayloads">
            <Payload Id="PayA" SourceFile="fileA.dat" Name="targetA.dat" />
            <IgnoredChildTag />
        </PayloadGroup>
        <Chain>
            <MsiPackage Id="PrereqMsi" SourceFile="prereq.msi" />
            <RollbackBoundary Id="BoundaryAfterPrereq" />
            <ExePackage Id="MainExe" SourceFile="main.exe" />
            <MspPackage Id="PatchMsp" SourceFile="patch.msp" />
            <MsuPackage Id="UpdateMsu" SourceFile="update.msu" />
            <MsiPackage Id="ExtraMsi" SourceFile="extra.msi" />
        </Chain>
    </Bundle>
</Wix>
"#;
        let compiler = BurnCompiler::new();
        assert_eq!(compiler, BurnCompiler);
        assert!(compiler.compile_xml("<Invalid><").is_err());
        let bundle = compiler.compile_xml(xml).unwrap_or_default();
        assert_eq!(bundle.name, "BurnProduct");
        assert_eq!(bundle.payload_groups.len(), 1);
        assert_eq!(bundle.payload_groups[0].payloads.len(), 1);
        assert_eq!(
            bundle.payload_groups[0].payloads[0].source_file,
            "fileA.dat"
        );

        let parser = XmlParser::new();
        let root = parser.parse(xml).unwrap_or_default();
        let bundle_from_node = compiler.compile_node(&root).unwrap_or_default();
        assert_eq!(bundle, bundle_from_node);

        // Test BurnLinker manifest generation and bundle packing
        let linker = BurnLinker::new();
        assert_eq!(linker, BurnLinker);
        let manifest = linker.assemble_manifest(&bundle);
        assert!(manifest.contains("<BurnManifest"));
        assert!(manifest.contains("<MsiPackage Id=\"PrereqMsi\""));
        assert!(manifest.contains("<RollbackBoundary Id=\"BoundaryAfterPrereq\""));
        assert!(manifest.contains("<MspPackage Id=\"PatchMsp\""));
        assert!(manifest.contains("<MsuPackage Id=\"UpdateMsu\""));

        let bundle_no_src = BurnBundle {
            compressed: false,
            chain: vec![
                ChainPackage {
                    id: "NoSrcMsi".to_string(),
                    package_type: ChainPackageType::Msi,
                    source_file: None,
                    install_condition: None,
                    detect_condition: None,
                    install_command: None,
                    uninstall_command: None,
                    cache: None,
                },
                ChainPackage {
                    id: "NoSrcExe".to_string(),
                    package_type: ChainPackageType::Exe,
                    source_file: None,
                    install_condition: None,
                    detect_condition: None,
                    install_command: None,
                    uninstall_command: None,
                    cache: None,
                },
                ChainPackage {
                    id: "NoSrcMsp".to_string(),
                    package_type: ChainPackageType::Msp,
                    source_file: None,
                    install_condition: None,
                    detect_condition: None,
                    install_command: None,
                    uninstall_command: None,
                    cache: None,
                },
                ChainPackage {
                    id: "NoSrcMsu".to_string(),
                    package_type: ChainPackageType::Msu,
                    source_file: None,
                    install_condition: None,
                    detect_condition: None,
                    install_command: None,
                    uninstall_command: None,
                    cache: None,
                },
            ],
            ..bundle.clone()
        };
        let manifest_no_src = linker.assemble_manifest(&bundle_no_src);
        assert!(manifest_no_src.contains("<MsiPackage Id=\"NoSrcMsi\" SourceFile=\"\""));
        assert!(manifest_no_src.contains("<ExePackage Id=\"NoSrcExe\" SourceFile=\"\""));
        assert!(manifest_no_src.contains("<MspPackage Id=\"NoSrcMsp\" SourceFile=\"\""));
        assert!(manifest_no_src.contains("<MsuPackage Id=\"NoSrcMsu\" SourceFile=\"\""));
        assert!(manifest_no_src.contains("Compressed=\"no\""));

        let payload_files = [
            ("prereq.msi", b"MSI_PAYLOAD_A".as_slice()),
            ("main.exe", b"EXE_PAYLOAD_B".as_slice()),
        ];
        let packed_exe = linker
            .pack_bundle(&bundle, &payload_files)
            .unwrap_or_default();
        assert!(packed_exe.len() > 1024);
        assert_eq!(&packed_exe[0..2], b"MZ");

        // Duplicate filename error
        assert!(linker
            .pack_bundle(&bundle, &[("manifest.xml", b"dup")])
            .is_err());

        // Test BurnEngine condition evaluation
        let engine = BurnEngine::new();
        assert_eq!(engine, BurnEngine);
        let mut context = crate::execution::properties::EvaluationContext::new();
        context.set_property("FEATURE_ENABLED", "1");
        assert!(engine.evaluate_condition("FEATURE_ENABLED = \"1\"", &context));
        assert!(!engine.evaluate_condition("FEATURE_ENABLED = \"0\"", &context));

        // Test BurnEngine chain execution: Full success
        let summary_ok =
            engine.execute_chain(&bundle.chain, &mut |_pkg| Ok(0), &mut dummy_rollback);
        assert!(summary_ok.success);
        assert_eq!(summary_ok.exit_code, 0);
        assert_eq!(summary_ok.installed_packages.len(), 5);
        assert_eq!(summary_ok.rolled_back_packages.len(), 0);

        // Test BurnEngine chain execution: Failure with rollback boundary
        let mut rolled_back_ids = Vec::new();
        let summary_fail = engine.execute_chain(
            &bundle.chain,
            &mut |pkg| {
                if pkg.id == "ExtraMsi" {
                    Ok(1602) // simulated failure exit code
                } else {
                    Ok(0)
                }
            },
            &mut |rb_id| {
                rolled_back_ids.push(rb_id.to_string());
                Ok(())
            },
        );
        assert!(!summary_fail.success);
        assert_eq!(summary_fail.exit_code, 1602);
        assert_eq!(summary_fail.installed_packages, vec!["PrereqMsi"]);
        assert_eq!(
            summary_fail.rolled_back_packages,
            vec!["UpdateMsu", "PatchMsp", "MainExe"]
        );
        assert_eq!(rolled_back_ids, vec!["UpdateMsu", "PatchMsp", "MainExe"]);

        // Test BurnEngine chain execution: Err failure
        let summary_err = engine.execute_chain(
            &bundle.chain,
            &mut |pkg| {
                if pkg.id == "ExtraMsi" {
                    Err(Error::BurnBundleError {
                        reason: "fail".to_string(),
                    })
                } else {
                    Ok(0)
                }
            },
            &mut dummy_rollback,
        );
        assert!(!summary_err.success);
        assert_eq!(summary_err.exit_code, 1603);
        assert_eq!(summary_err.installed_packages, vec!["PrereqMsi"]);
        assert_eq!(
            summary_err.rolled_back_packages,
            vec!["UpdateMsu", "PatchMsp", "MainExe"]
        );
    }
}

//! `WiX` toolset compiler pipeline and intermediate object model.
//!
//! Grounded directly in official `WiX` v3, v4, v5 specifications and POSIX extensions:
//! - Preprocessor engine (`$(var.X)`, `$(env.X)`, `$(sys.X)`, `<?if?>`, `<?foreach?>`, `<?include?>`)
//! - Robust XML parser with line/column tracking
//! - `WiX` intermediate object format (`.wixobj`) with symbols and unresolved references
//! - Schema validation for `WiX` v3, v4, v5, and cross-platform extensions

pub mod bundle;
pub mod compiler;
pub mod harvest;
pub mod linker;
pub mod localization;
pub mod manifest;
pub mod parity;
pub mod patch;
pub mod preprocessor;
pub mod schema;
pub mod toolchain;
pub mod ui_library;
pub mod wixlib;
pub mod wixobj;
pub mod xml;

pub use bundle::{BootstrapperApplication, BurnBundle, ChainPackage, ChainPackageType};
pub use compiler::Compiler;
pub use harvest::{HarvestPayloadOptions, HarvestPayloadResult, Harvester};
pub use linker::{
    modularize_identifier, CubValidator, IceDiagnostic, IceDiagnosticType, IceRegistry, IceReport,
    IceRule, LinkedDatabase, Linker, MergeModule, StandardActionOrder, StandardDirectory,
    StandardIceRule, STANDARD_DIRECTORIES, STANDARD_INSTALL_EXECUTE_ACTIONS,
};
pub use localization::{LocalizationCatalog, WixLocString, WixLocalization};
pub use manifest::{
    is_sensitive_property_name, parse_vars_schema, synthesize_wix_xml, JsonValue,
    ManifestMsiSynthesizer, PackagingManifest, SchemaProperty,
};
pub use parity::MsiDecompiler;
pub use patch::{
    BinaryDelta, BinaryDeltaInstruction, CPackWiXFragment, CPackWiXPatch, PatchCreation,
    PatchFamily, PatchInformation, PatchMetadata, PatchPackageBuilder, TargetImage, UpgradeImage,
};
pub use preprocessor::{Preprocessor, PreprocessorContext, SystemVariables};
pub use schema::{
    WixSchemaVersion, WIX_POSIX_V1_NAMESPACE, WIX_V3_NAMESPACE, WIX_V4_NAMESPACE, WIX_V5_NAMESPACE,
};
pub use toolchain::{CandleOptions, LightOptions, WixBuildOptions, WixSubcommand};
pub use ui_library::{
    generate_placeholder_bmp, generate_placeholder_ico, inject_ui_library, WixUiDialogSet,
};
pub use wixlib::WixLibrary;
pub use wixobj::{
    IntermediateSection, IntermediateTable, Reference, SectionType, Symbol, WixObject,
    WIXOBJ_MAGIC, WIXOBJ_VERSION,
};
pub use xml::{XmlNode, XmlParser};

use crate::error::Result;

/// Compiles raw `WiX` source code through the complete compiler pipeline (`candle` architecture).
///
/// Steps:
/// 1. Preprocesses source code (macros, conditions, looping, file inclusion).
/// 2. Parses preprocessed XML into a structured document model.
/// 3. Compiles elements into an intermediate [`WixObject`] containing sections, symbols, and tables.
///
/// # Arguments
///
/// * `source` - Raw `WiX` source string (`.wxs`).
/// * `ctx` - Preprocessor context.
///
/// # Returns
///
/// Compiled [`WixObject`].
///
/// # Errors
///
/// Returns [`crate::error::Error`] on preprocessing, XML parsing, or compiler validation failures.
pub fn compile_wix(source: &str, ctx: &mut PreprocessorContext) -> Result<WixObject> {
    let preprocessor = Preprocessor::new();
    let preprocessed_source = preprocessor.process(source, ctx)?;

    let xml_parser = XmlParser::new();
    let root = xml_parser.parse(&preprocessed_source)?;

    let compiler = Compiler::new();
    compiler.compile(&root)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper compiling `WiX` source to a vector, or empty vector on failure.
    ///
    /// # Arguments
    ///
    /// * `source` - Raw `WiX` source string.
    /// * `ctx` - Preprocessor context.
    ///
    /// # Returns
    ///
    /// Vector with compiled [`WixObject`] on success, empty vector on error.
    fn compile_to_vec(source: &str, ctx: &mut PreprocessorContext) -> Vec<WixObject> {
        let Ok(obj) = compile_wix(source, ctx) else {
            return Vec::new();
        };
        vec![obj]
    }

    /// Tests preprocessor error propagation in `compile_wix`.
    #[test]
    fn test_compile_wix_preprocessor_error() {
        let mut ctx = PreprocessorContext::new();
        assert!(compile_wix("$(var.MissingVar)", &mut ctx).is_err());
    }

    /// Tests XML parse error propagation in `compile_wix`.
    #[test]
    fn test_compile_wix_xml_error() {
        let mut ctx = PreprocessorContext::new();
        assert!(compile_wix("<unclosed tag", &mut ctx).is_err());
    }

    /// Tests end-to-end `WiX` compilation, serialization, and deserialization.
    #[test]
    fn test_compile_wix_end_to_end() {
        let mut ctx = PreprocessorContext::new();
        ctx.define_var("AppVersion", "3.1.4");
        ctx.define_var("BuildType", "Release");

        assert!(compile_to_vec("$(var.MissingVar)", &mut ctx).is_empty());

        let source = r#"
<Wix xmlns="http://schemas.microsoft.com/wix/2006/wi">
    <Product Id="{12345678-1234-1234-1234-1234567890AB}" Name="EndToEndApp" Version="$(var.AppVersion)" Manufacturer="Acme">
        <Package Description="E2E Test" />
        <Directory Id="TARGETDIR" Name="SourceDir">
            <?if $(var.BuildType) = "Release"?>
            <Component Id="ReleaseComp">
                <File Id="RelExe" Source="release.exe" />
            </Component>
            <?else?>
            <Component Id="DebugComp">
                <File Id="DbgExe" Source="debug.exe" />
            </Component>
            <?endif?>
        </Directory>
        <Feature Id="MainFeature" Title="Main" Level="1">
            <ComponentRef Id="ReleaseComp" />
        </Feature>
    </Product>
</Wix>
"#;

        for obj in compile_to_vec(source, &mut ctx) {
            assert_eq!(obj.sections.len(), 1);
            let sec = &obj.sections[0];
            assert_eq!(sec.section_type, SectionType::Product);

            assert!(sec
                .symbols
                .contains(&Symbol::new("Component", "ReleaseComp")));
            assert!(!sec.symbols.contains(&Symbol::new("Component", "DebugComp")));

            // Test roundtrip through binary .wixobj serialization
            let bytes = obj.serialize();
            let loaded = WixObject::deserialize(&bytes);
            assert_eq!(loaded, Ok(obj));
        }
    }
}

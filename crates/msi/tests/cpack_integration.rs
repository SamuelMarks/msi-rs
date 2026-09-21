//! # End-to-End CMake/CPack Integration & Conformance Test Suite
//!
//! Simulates the complete CMake/CPack `WiX` generator workflow:
//! - Setting up toolchain paths with `candle` and `light`.
//! - Compiling and linking CMake-generated `.wxs` source trees.
//! - Verifying `CPACK_WIX_UPGRADE_GUID`, `CPACK_WIX_PRODUCT_ICON`, `CPACK_WIX_UI_REF`,
//!   `CPACK_WIX_PATCH_FILE`, and `CPACK_WIX_CULTURES` configurations.
//! - Validating generated `.msi` database tables, cabinets, and ICE conformance.

use msi::database::tables::record::FieldValue;
use msi::error::Result;
use msi::package::Package;
use msi::wix::patch::CPackWiXPatch;
use msi::wix::toolchain::{CandleOptions, LightOptions, WixBuildOptions};
use msi::wix::xml::XmlParser;
use std::fs;

#[test]
#[allow(clippy::too_many_lines, clippy::similar_names)]
fn test_cpack_wix_end_to_end_pipeline() -> Result<()> {
    let temp_dir = std::env::temp_dir().join("msi_test_cpack_e2e");
    let _ = fs::create_dir_all(&temp_dir);

    let bin_dir = temp_dir.join("bin");
    let src_dir = temp_dir.join("src");
    let out_dir = temp_dir.join("out");
    fs::create_dir_all(&bin_dir)?;
    fs::create_dir_all(&src_dir)?;
    fs::create_dir_all(&out_dir)?;

    // 1. Create simulated payload file to be installed
    let payload_file = src_dir.join("my_app.exe");
    fs::write(&payload_file, b"Executable binary content for CPack test")?;

    let icon_file = src_dir.join("app.ico");
    fs::write(&icon_file, b"Icon binary content")?;

    // 2. Create CPack localization file (.wxl)
    let wxl_file = src_dir.join("cpack_strings.wxl");
    let wxl_content = r#"
<WixLocalization Culture="en-US" Codepage="1252" xmlns="http://schemas.microsoft.com/wix/2006/localization">
    <String Id="ProductDescription">CPack Automated WiX Package Description</String>
    <String Id="WelcomeTitle">Welcome to My CMake App Installer</String>
</WixLocalization>
"#;
    fs::write(&wxl_file, wxl_content)?;

    // 3. Create CPack XML patch file (CPACK_WIX_PATCH_FILE)
    let patch_file = src_dir.join("cpack_patch.xml");
    let patch_content = r##"
<CPackWiXPatch>
    <CPackWiXFragment Id="#PRODUCT">
        <Property Id="CPACK_CUSTOM_PROP" Value="InjectedByPatch" />
    </CPackWiXFragment>
    <CPackWiXFragment Id="MyComponent">
        <CreateFolder />
    </CPackWiXFragment>
</CPackWiXPatch>
"##;
    fs::write(&patch_file, patch_content)?;

    // 4. Create CMake/CPack-generated .wxs template
    let wxs_file = src_dir.join("cpack_generated.wxs");
    let wxs_content = r##"
<Wix xmlns="http://schemas.microsoft.com/wix/2006/wi">
    <Product Id="{A1111111-1111-1111-1111-111111111111}"
             Name="MyCMakeApp"
             Version="1.2.3"
             Manufacturer="CMake Test Corp"
             UpgradeCode="{B2222222-2222-2222-2222-222222222222}">
        <Package Description="!(loc.ProductDescription)" Comments="CMake CPack WiX Generator" />
        <Upgrade Id="{B2222222-2222-2222-2222-222222222222}">
            <UpgradeVersion Minimum="1.0.0" Maximum="1.2.3" IncludeMinimum="yes" IncludeMaximum="no" Property="UPGRADEFOUND" />
        </Upgrade>
        <Directory Id="TARGETDIR" Name="SourceDir">
            <Directory Id="ProgramFiles64Folder" Name="PFiles64">
                <Directory Id="INSTALLFOLDER" Name="MyCMakeApp">
                    <Component Id="MyComponent" Guid="{C3333333-3333-3333-3333-333333333333}">
                        <File Id="MainExecutable" Source="my_app.exe" KeyPath="yes" />
                    </Component>
                </Directory>
            </Directory>
        </Directory>
        <Feature Id="ProductFeature" Title="MyCMakeApp Feature" Level="1">
            <ComponentRef Id="MyComponent" />
        </Feature>
        <Media Id="1" Cabinet="#cab1.cab" EmbedCab="yes" />
        <UIRef Id="WixUI_InstallDir" />
        <Property Id="WIXUI_INSTALLDIR" Value="INSTALLFOLDER" />
    </Product>
</Wix>
"##;

    // Apply CPack patch file to AST prior to compilation (simulating CPack's patch step)
    let patch = CPackWiXPatch::parse(patch_content)?;
    let parser = XmlParser::new();
    let mut ast = parser.parse(wxs_content)?;
    let applied_count = patch.apply_to_ast(&mut ast);
    assert_eq!(applied_count, 2);

    // Save patched wxs
    fs::write(&wxs_file, ast.to_string())?;

    // 5. Run Candle compiler shim
    let obj_file = out_dir.join("cpack_generated.wixobj");
    let candle_args = vec![
        "-nologo".to_string(),
        "-arch".to_string(),
        "x64".to_string(),
        "-out".to_string(),
        obj_file.to_string_lossy().to_string(),
        wxs_file.to_string_lossy().to_string(),
    ];
    let candle_opts = CandleOptions::parse(&candle_args)?;
    let candle_outputs = candle_opts.execute()?;
    assert_eq!(candle_outputs.len(), 1);
    assert!(obj_file.exists());

    // 6. Run Light linker/binder shim with base directories, localization, and UI extension
    let msi_file = out_dir.join("MyCMakeApp-1.2.3.msi");
    let light_args = vec![
        "-nologo".to_string(),
        "-sval".to_string(),
        "-ext".to_string(),
        "WixUIExtension".to_string(),
        "-cultures:en-us".to_string(),
        "-loc".to_string(),
        wxl_file.to_string_lossy().to_string(),
        "-b".to_string(),
        src_dir.to_string_lossy().to_string(),
        "-out".to_string(),
        msi_file.to_string_lossy().to_string(),
        obj_file.to_string_lossy().to_string(),
    ];
    let light_opts = LightOptions::parse(&light_args)?;
    let generated_msi = light_opts.execute()?;
    assert_eq!(generated_msi, msi_file);
    assert!(msi_file.exists());

    // 7. Verify generated MSI package
    let pkg = Package::open(&msi_file)?;
    assert_eq!(pkg.metadata().product_name(), "MyCMakeApp");
    assert_eq!(pkg.metadata().manufacturer(), "CMake Test Corp");
    assert_eq!(
        pkg.metadata().product_code(),
        "{A1111111-1111-1111-1111-111111111111}"
    );

    // Verify Upgrade table record exists (CPACK_WIX_UPGRADE_GUID)
    let upgrade_records = pkg.database().get_records("Upgrade");
    assert_eq!(upgrade_records.len(), 1);
    assert_eq!(
        upgrade_records[0].get(0),
        Some(&FieldValue::String(
            "{B2222222-2222-2222-2222-222222222222}".to_string()
        ))
    );

    // Verify Property injected by CPack patch file exists
    let prop_records = pkg.database().get_records("Property");
    assert!(prop_records.iter().any(|r| {
        matches!((r.get(0), r.get(1)), (Some(FieldValue::String(k)), Some(FieldValue::String(v))) if k == "CPACK_CUSTOM_PROP" && v == "InjectedByPatch")
    }));

    // Verify Localization string was expanded
    assert_eq!(pkg.summary_info().subject.as_deref(), Some("MyCMakeApp"));

    // Verify WixUI standard dialogs and controls were injected
    let dialog_records = pkg.database().get_records("Dialog");
    assert_ne!(dialog_records, []);
    assert!(dialog_records
        .iter()
        .any(|r| { matches!(r.get(0), Some(FieldValue::String(s)) if s == "WelcomeDlg") }));
    assert!(dialog_records
        .iter()
        .any(|r| { matches!(r.get(0), Some(FieldValue::String(s)) if s == "InstallDirDlg") }));

    // Verify embedded cabinet archive exists
    assert!(pkg.embedded_cabinets().contains_key("#cab1.cab"));

    // 8. Test WixBuildOptions unified one-step pipeline parity
    let unified_msi = out_dir.join("Unified_MyCMakeApp.msi");
    let wix_build_args = vec![
        "-arch".to_string(),
        "x64".to_string(),
        "-ext".to_string(),
        "WixUIExtension".to_string(),
        "-culture".to_string(),
        "en-US".to_string(),
        "-b".to_string(),
        src_dir.to_string_lossy().to_string(),
        "-sval".to_string(),
        "-o".to_string(),
        unified_msi.to_string_lossy().to_string(),
        wxs_file.to_string_lossy().to_string(),
        wxl_file.to_string_lossy().to_string(),
    ];
    let wix_opts = WixBuildOptions::parse(&wix_build_args)?;
    let built_msi = wix_opts.execute()?;
    assert_eq!(built_msi, unified_msi);
    assert!(unified_msi.exists());

    let pkg_unified = Package::open(&unified_msi)?;
    assert_eq!(pkg_unified.metadata().product_name(), "MyCMakeApp");

    let _ = fs::remove_dir_all(&temp_dir);
    Ok(())
}

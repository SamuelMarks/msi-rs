//! Integration tests for multi-source `WiX` compilation, cross-fragment linking,
//! component group resolution, directory rebinding, long identifier auto-hashing,
//! and missing symbol diagnostics.

use msi::database::tables::record::FieldValue;
use msi::error::{MsiError, Result};
use msi::package::Package;
use msi::wix::toolchain::{CandleOptions, WixBuildOptions};
use std::fs;
use std::path::{Path, PathBuf};

/// Creates a temporary directory for test artifacts.
///
/// # Arguments
///
/// * `test_name` - Descriptive prefix for the test folder.
///
/// # Returns
///
/// Created [`PathBuf`].
///
/// # Errors
///
/// Returns [`msi::MsiError`] on filesystem creation failure.
fn create_test_temp_dir(test_name: &str) -> Result<PathBuf> {
    let dir = std::env::temp_dir().join(format!("msi_{test_name}_{}", std::process::id()));
    fs::create_dir_all(&dir)?;
    Ok(dir)
}

/// Helper to write a file and return its path.
///
/// # Arguments
///
/// * `dir` - Target directory.
/// * `filename` - Target filename.
/// * `content` - Content bytes to write.
///
/// # Returns
///
/// Path to the written file.
///
/// # Errors
///
/// Returns [`msi::MsiError`] on filesystem write failure.
fn write_test_file(dir: &Path, filename: &str, content: &str) -> Result<PathBuf> {
    let path = dir.join(filename);
    fs::write(&path, content)?;
    Ok(path)
}

/// Tests multi-source compilation and linking where a product manifest references
/// a component group declared in an external fragment file.
///
/// # Errors
///
/// Returns [`msi::MsiError`] on compilation, linking, or verification failure.
#[test]
fn test_wix_multi_source_cross_fragment_component_group() -> Result<()> {
    let temp_dir = create_test_temp_dir("multi_src_cg")?;
    let dummy_file = write_test_file(&temp_dir, "app.bin", "BINARY_PAYLOAD_1")?;

    let product_wxs = write_test_file(
        &temp_dir,
        "Product.wxs",
        r#"<?xml version="1.0" encoding="UTF-8"?>
<Wix xmlns="http://schemas.microsoft.com/wix/2006/wi">
  <Product Id="{11111111-1111-1111-1111-111111111111}" Name="MultiSourceApp" Language="1033" Version="1.0.0" Manufacturer="Vendor" UpgradeCode="{22222222-2222-2222-2222-222222222222}">
    <Package Description="Multi Source Test" />
    <Media Id="1" Cabinet="engine.cab" EmbedCab="yes" />

    <Directory Id="TARGETDIR" Name="SourceDir">
      <Directory Id="ProgramFilesFolder">
        <Directory Id="INSTALLFOLDER" Name="MultiSourceApp" />
      </Directory>
    </Directory>

    <Feature Id="MainFeature" Title="Core" Level="1">
      <ComponentGroupRef Id="PayloadComponents" />
    </Feature>
  </Product>
</Wix>
"#,
    )?;

    let payload_wxs = write_test_file(
        &temp_dir,
        "Payload.wxs",
        &format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<Wix xmlns="http://schemas.microsoft.com/wix/2006/wi">
  <Fragment>
    <DirectoryRef Id="INSTALLFOLDER">
      <Component Id="CmpAppBin" Guid="33333333-3333-3333-3333-333333333333">
        <File Id="FileAppBin" Source="{}" KeyPath="yes" DiskId="1" />
      </Component>
    </DirectoryRef>

    <ComponentGroup Id="PayloadComponents">
      <ComponentRef Id="CmpAppBin" />
    </ComponentGroup>
  </Fragment>
</Wix>
"#,
            dummy_file.display()
        ),
    )?;

    let out_msi = temp_dir.join("MultiSourceApp.msi");
    let build_opts = WixBuildOptions {
        sources: vec![product_wxs, payload_wxs],
        output: Some(out_msi.clone()),
        suppress_ice: true,
        ..WixBuildOptions::new()
    };

    let result_path = build_opts.execute()?;
    assert!(result_path.exists());

    let pkg = Package::open(&out_msi)?;
    assert_eq!(pkg.metadata().product_name(), "MultiSourceApp");

    let db = pkg.database();
    let comp_records = db.get_records("Component");
    assert!(comp_records
        .iter()
        .any(|r| r.get(0) == Some(&FieldValue::String("CmpAppBin".to_string()))));

    let feat_comp_records = db.get_records("FeatureComponents");
    assert!(feat_comp_records.iter().any(|r| {
        r.get(0) == Some(&FieldValue::String("MainFeature".to_string()))
            && r.get(1) == Some(&FieldValue::String("CmpAppBin".to_string()))
    }));

    let _ = fs::remove_dir_all(&temp_dir);
    Ok(())
}

/// Tests multi-fragment component group merging, where two distinct fragments contribute
/// components to the exact same `ComponentGroup`.
///
/// # Errors
///
/// Returns [`msi::MsiError`] on compilation, linking, or verification failure.
#[test]
fn test_wix_multi_fragment_component_group_merging() -> Result<()> {
    let temp_dir = create_test_temp_dir("cg_merging")?;
    let dummy1 = write_test_file(&temp_dir, "svc1.dat", "SERVICE_ONE")?;
    let dummy2 = write_test_file(&temp_dir, "svc2.dat", "SERVICE_TWO")?;

    let product_wxs = write_test_file(
        &temp_dir,
        "Product.wxs",
        r#"<?xml version="1.0" encoding="UTF-8"?>
<Wix xmlns="http://schemas.microsoft.com/wix/2006/wi">
  <Product Id="{44444444-4444-4444-4444-444444444444}" Name="MergedGroupApp" Language="1033" Version="2.0.0" Manufacturer="Vendor" UpgradeCode="{55555555-5555-5555-5555-555555555555}">
    <Package Description="Merge Test" />
    <Media Id="1" Cabinet="engine.cab" EmbedCab="yes" />

    <Directory Id="TARGETDIR" Name="SourceDir">
      <Directory Id="ProgramFilesFolder">
        <Directory Id="INSTALLFOLDER" Name="MergedApp" />
      </Directory>
    </Directory>

    <Feature Id="MainFeature" Title="All" Level="1">
      <ComponentGroupRef Id="SharedGroup" />
    </Feature>
  </Product>
</Wix>
"#,
    )?;

    let fragment1_wxs = write_test_file(
        &temp_dir,
        "Frag1.wxs",
        &format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<Wix xmlns="http://schemas.microsoft.com/wix/2006/wi">
  <Fragment>
    <DirectoryRef Id="INSTALLFOLDER">
      <Component Id="CmpSvc1" Guid="66666666-6666-6666-6666-666666666666">
        <File Id="FileSvc1" Source="{}" KeyPath="yes" DiskId="1" />
      </Component>
    </DirectoryRef>
    <ComponentGroup Id="SharedGroup">
      <ComponentRef Id="CmpSvc1" />
    </ComponentGroup>
  </Fragment>
</Wix>
"#,
            dummy1.display()
        ),
    )?;

    let fragment2_wxs = write_test_file(
        &temp_dir,
        "Frag2.wxs",
        &format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<Wix xmlns="http://schemas.microsoft.com/wix/2006/wi">
  <Fragment>
    <DirectoryRef Id="INSTALLFOLDER">
      <Component Id="CmpSvc2" Guid="77777777-7777-7777-7777-777777777777">
        <File Id="FileSvc2" Source="{}" KeyPath="yes" DiskId="1" />
      </Component>
    </DirectoryRef>
    <ComponentGroup Id="SharedGroup">
      <ComponentRef Id="CmpSvc2" />
    </ComponentGroup>
  </Fragment>
</Wix>
"#,
            dummy2.display()
        ),
    )?;

    let out_msi = temp_dir.join("MergedGroupApp.msi");
    let build_opts = WixBuildOptions {
        sources: vec![product_wxs, fragment1_wxs, fragment2_wxs],
        output: Some(out_msi.clone()),
        suppress_ice: true,
        ..WixBuildOptions::new()
    };

    let result_path = build_opts.execute()?;
    assert!(result_path.exists());

    let pkg = Package::open(&out_msi)?;
    let db = pkg.database();

    // Verify both components are linked into FeatureComponents via the merged ComponentGroup
    let fc = db.get_records("FeatureComponents");
    let has_c1 = fc.iter().any(|r| {
        r.get(0) == Some(&FieldValue::String("MainFeature".to_string()))
            && r.get(1) == Some(&FieldValue::String("CmpSvc1".to_string()))
    });
    let has_c2 = fc.iter().any(|r| {
        r.get(0) == Some(&FieldValue::String("MainFeature".to_string()))
            && r.get(1) == Some(&FieldValue::String("CmpSvc2".to_string()))
    });
    assert!(has_c1, "Missing CmpSvc1 in FeatureComponents");
    assert!(has_c2, "Missing CmpSvc2 in FeatureComponents");

    let _ = fs::remove_dir_all(&temp_dir);
    Ok(())
}

/// Tests directory reference rebinding where multiple fragments attach child directories
/// to the same parent directory reference.
///
/// # Errors
///
/// Returns [`msi::MsiError`] on compilation, linking, or verification failure.
#[test]
fn test_wix_directory_reference_rebinding() -> Result<()> {
    let temp_dir = create_test_temp_dir("dir_rebinding")?;
    let dummy = write_test_file(&temp_dir, "lib.dll", "LIBRARY")?;

    let product_wxs = write_test_file(
        &temp_dir,
        "Product.wxs",
        r#"<?xml version="1.0" encoding="UTF-8"?>
<Wix xmlns="http://schemas.microsoft.com/wix/2006/wi">
  <Product Id="{88888888-8888-8888-8888-888888888888}" Name="DirRebindApp" Language="1033" Version="1.0.0" Manufacturer="Vendor" UpgradeCode="{99999999-9999-9999-9999-999999999999}">
    <Package Description="Dir Rebind Test" />
    <Media Id="1" Cabinet="engine.cab" EmbedCab="yes" />

    <Directory Id="TARGETDIR" Name="SourceDir">
      <Directory Id="ProgramFilesFolder">
        <Directory Id="APPDIR" Name="DirRebindApp" />
      </Directory>
    </Directory>

    <Feature Id="MainFeature" Title="Core" Level="1">
      <ComponentRef Id="CmpInSubDir" />
    </Feature>
  </Product>
</Wix>
"#,
    )?;

    let fragment_wxs = write_test_file(
        &temp_dir,
        "SubDirFragment.wxs",
        &format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<Wix xmlns="http://schemas.microsoft.com/wix/2006/wi">
  <Fragment>
    <DirectoryRef Id="APPDIR">
      <Directory Id="BINDIR" Name="bin">
        <Component Id="CmpInSubDir" Guid="AAAAAAAA-AAAA-AAAA-AAAA-AAAAAAAAAAAA">
          <File Id="FileLibDll" Source="{}" KeyPath="yes" DiskId="1" />
        </Component>
      </Directory>
    </DirectoryRef>
  </Fragment>
</Wix>
"#,
            dummy.display()
        ),
    )?;

    let out_msi = temp_dir.join("DirRebindApp.msi");
    let build_opts = WixBuildOptions {
        sources: vec![product_wxs, fragment_wxs],
        output: Some(out_msi.clone()),
        suppress_ice: true,
        ..WixBuildOptions::new()
    };

    let result_path = build_opts.execute()?;
    assert!(result_path.exists());

    let pkg = Package::open(&out_msi)?;
    let db = pkg.database();

    // Verify Directory table has BINDIR whose parent is APPDIR
    let dirs = db.get_records("Directory");
    let bin_dir = dirs
        .iter()
        .find(|r| r.get(0) == Some(&FieldValue::String("BINDIR".to_string())));
    assert!(bin_dir.is_some());
    if let Some(r) = bin_dir {
        assert_eq!(r.get(1), Some(&FieldValue::String("APPDIR".to_string())));
    }

    let _ = fs::remove_dir_all(&temp_dir);
    Ok(())
}

/// Tests deterministic auto-hashing of component and file identifiers exceeding 72 characters.
///
/// # Errors
///
/// Returns [`msi::MsiError`] on compilation, linking, or verification failure.
#[test]
fn test_wix_long_identifier_auto_hashing_parity() -> Result<()> {
    let temp_dir = create_test_temp_dir("long_id")?;
    let dummy = write_test_file(&temp_dir, "long.txt", "LONG_IDENTIFIER_TEST")?;

    let long_comp_id = "CMP_very_long_component_name_that_exceeds_the_seventy_two_character_limit_for_identifiers_in_msi";
    let long_file_id =
        "FIL_very_long_file_identifier_that_exceeds_the_standard_column_length_in_database";

    assert!(long_comp_id.len() > 72);
    assert!(long_file_id.len() > 72);

    let product_wxs = write_test_file(
        &temp_dir,
        "Product.wxs",
        &format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<Wix xmlns="http://schemas.microsoft.com/wix/2006/wi">
  <Product Id="{{BBBBBBBB-BBBB-BBBB-BBBB-BBBBBBBBBBBB}}" Name="LongIdApp" Language="1033" Version="1.0.0" Manufacturer="Vendor" UpgradeCode="{{CCCCCCCC-CCCC-CCCC-CCCC-CCCCCCCCCCCC}}">
    <Package Description="Long Id Test" />
    <Media Id="1" Cabinet="engine.cab" EmbedCab="yes" />

    <Directory Id="TARGETDIR" Name="SourceDir">
      <Directory Id="ProgramFilesFolder">
        <Directory Id="INSTALLFOLDER" Name="LongIdApp">
          <Component Id="{long_comp_id}" Guid="DDDDDDDD-DDDD-DDDD-DDDD-DDDDDDDDDDDD">
            <File Id="{long_file_id}" Source="{}" KeyPath="yes" DiskId="1" />
          </Component>
        </Directory>
      </Directory>
    </Directory>

    <Feature Id="MainFeature" Title="Core" Level="1">
      <ComponentRef Id="{long_comp_id}" />
    </Feature>
  </Product>
</Wix>
"#,
            dummy.display()
        ),
    )?;

    let out_msi = temp_dir.join("LongIdApp.msi");
    let build_opts = WixBuildOptions {
        sources: vec![product_wxs],
        output: Some(out_msi.clone()),
        suppress_ice: true,
        ..WixBuildOptions::new()
    };

    let result_path = build_opts.execute()?;
    assert!(result_path.exists());

    let pkg = Package::open(&out_msi)?;
    let db = pkg.database();

    // Verify sanitized Component ID length <= 72
    let comps = db.get_records("Component");
    assert_eq!(comps.len(), 1);
    if let Some(r) = comps.first() {
        if let Some(FieldValue::String(id)) = r.get(0) {
            assert!(id.len() <= 72);
            assert!(id.starts_with(&long_comp_id[..50]));
        } else {
            return Err(MsiError::Validation {
                element: "Component.Component".to_string(),
                reason: "missing component id field".to_string(),
            });
        }
    }

    // Verify sanitized File ID length <= 72
    let files = db.get_records("File");
    assert_eq!(files.len(), 1);
    if let Some(r) = files.first() {
        if let Some(FieldValue::String(id)) = r.get(0) {
            assert!(id.len() <= 72);
            assert!(id.starts_with(&long_file_id[..50]));
        } else {
            return Err(MsiError::Validation {
                element: "File.File".to_string(),
                reason: "missing file id field".to_string(),
            });
        }
    }

    // Verify FeatureComponents contains the matching sanitized Component ID
    let fc = db.get_records("FeatureComponents");
    assert_eq!(fc.len(), 1);
    if let (Some(r_c), Some(r_fc)) = (comps.first(), fc.first()) {
        assert_eq!(r_c.get(0), r_fc.get(1));
    }

    let _ = fs::remove_dir_all(&temp_dir);
    Ok(())
}

/// Tests that referencing an undefined symbol produces a structured `MsiError::WixLinker` diagnostic.
///
/// # Errors
///
/// Returns [`msi::MsiError`] on unexpected success or failure.
#[test]
fn test_wix_missing_symbol_diagnostics() -> Result<()> {
    let temp_dir = create_test_temp_dir("missing_sym")?;

    let product_wxs = write_test_file(
        &temp_dir,
        "Product.wxs",
        r#"<?xml version="1.0" encoding="UTF-8"?>
<Wix xmlns="http://schemas.microsoft.com/wix/2006/wi">
  <Product Id="{EEEEEEEE-EEEE-EEEE-EEEE-EEEEEEEEEEEE}" Name="MissingSymApp" Language="1033" Version="1.0.0" Manufacturer="Vendor" UpgradeCode="{FFFFFFFF-FFFF-FFFF-FFFF-FFFFFFFFFFFF}">
    <Package Description="Missing Sym Test" />
    <Media Id="1" Cabinet="engine.cab" EmbedCab="yes" />

    <Directory Id="TARGETDIR" Name="SourceDir">
      <Directory Id="ProgramFilesFolder">
        <Directory Id="INSTALLFOLDER" Name="MissingSymApp" />
      </Directory>
    </Directory>

    <Feature Id="MainFeature" Title="Core" Level="1">
      <ComponentGroupRef Id="NonExistentComponentGroup" />
    </Feature>
  </Product>
</Wix>
"#,
    )?;

    let out_msi = temp_dir.join("MissingSymApp.msi");
    let build_opts = WixBuildOptions {
        sources: vec![product_wxs],
        output: Some(out_msi),
        suppress_ice: true,
        ..WixBuildOptions::new()
    };

    let result = build_opts.execute();
    assert!(result.is_err());
    if let Err(MsiError::WixLinker { message }) = result {
        assert!(message.contains("unresolved symbol reference"));
        assert!(message.contains("NonExistentComponentGroup"));
    } else {
        return Err(MsiError::Validation {
            element: "test_wix_missing_symbol_diagnostics".to_string(),
            reason: "expected MsiError::WixLinker for missing symbol".to_string(),
        });
    }

    let _ = fs::remove_dir_all(&temp_dir);
    Ok(())
}

/// Tests mixed inputs in `WixBuildOptions`: combining a pre-compiled `.wixobj` with a source `.wxs`.
///
/// # Errors
///
/// Returns [`msi::MsiError`] on compilation, linking, or verification failure.
#[test]
fn test_wix_mixed_wxs_and_wixobj_inputs() -> Result<()> {
    let temp_dir = create_test_temp_dir("mixed_inputs")?;
    let dummy = write_test_file(&temp_dir, "lib.so", "NATIVE_SO")?;

    // 1. Fragment authored and compiled into .wixobj via CandleOptions
    let fragment_wxs = write_test_file(
        &temp_dir,
        "Fragment.wxs",
        &format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<Wix xmlns="http://schemas.microsoft.com/wix/2006/wi">
  <Fragment>
    <DirectoryRef Id="INSTALLFOLDER">
      <Component Id="CmpLibSo" Guid="12121212-1212-1212-1212-121212121212">
        <File Id="FileLibSo" Source="{}" KeyPath="yes" DiskId="1" />
      </Component>
    </DirectoryRef>
    <ComponentGroup Id="NativeLibGroup">
      <ComponentRef Id="CmpLibSo" />
    </ComponentGroup>
  </Fragment>
</Wix>
"#,
            dummy.display()
        ),
    )?;

    let candle_opts = CandleOptions {
        sources: vec![fragment_wxs],
        output: Some(temp_dir.join("Fragment.wixobj")),
        arch: Some("x64".to_string()),
        ..CandleOptions::new()
    };
    let wixobjs = candle_opts.execute()?;
    assert_eq!(wixobjs.len(), 1);
    let compiled_wixobj = &wixobjs[0];

    // 2. Product authored in .wxs referencing NativeLibGroup from .wixobj
    let product_wxs = write_test_file(
        &temp_dir,
        "Product.wxs",
        r#"<?xml version="1.0" encoding="UTF-8"?>
<Wix xmlns="http://schemas.microsoft.com/wix/2006/wi">
  <Product Id="{34343434-3434-3434-3434-343434343434}" Name="MixedInputApp" Language="1033" Version="1.0.0" Manufacturer="Vendor" UpgradeCode="{56565656-5656-5656-5656-565656565656}">
    <Package Description="Mixed Input Test" />
    <Media Id="1" Cabinet="engine.cab" EmbedCab="yes" />

    <Directory Id="TARGETDIR" Name="SourceDir">
      <Directory Id="ProgramFilesFolder">
        <Directory Id="INSTALLFOLDER" Name="MixedApp" />
      </Directory>
    </Directory>

    <Feature Id="MainFeature" Title="Core" Level="1">
      <ComponentGroupRef Id="NativeLibGroup" />
    </Feature>
  </Product>
</Wix>
"#,
    )?;

    // 3. Build via WixBuildOptions combining .wxs and .wixobj
    let out_msi = temp_dir.join("MixedInputApp.msi");
    let build_opts = WixBuildOptions {
        sources: vec![product_wxs, compiled_wixobj.clone()],
        output: Some(out_msi.clone()),
        suppress_ice: true,
        ..WixBuildOptions::new()
    };

    let result_path = build_opts.execute()?;
    assert!(result_path.exists());

    let pkg = Package::open(&out_msi)?;
    assert_eq!(pkg.metadata().product_name(), "MixedInputApp");

    let db = pkg.database();
    let comp_records = db.get_records("Component");
    assert!(comp_records
        .iter()
        .any(|r| r.get(0) == Some(&FieldValue::String("CmpLibSo".to_string()))));

    let _ = fs::remove_dir_all(&temp_dir);
    Ok(())
}

/// Tests multi-hop cross-fragment directory attachment where subdirectories and components
/// attach across multiple separate fragments.
///
/// # Errors
///
/// Returns [`msi::MsiError`] if file I/O, compilation, or verification fails.
#[test]
#[allow(clippy::too_many_lines)]
fn test_wix_cross_fragment_directory_attachment_chain() -> Result<()> {
    let temp_dir = create_test_temp_dir("dir_chain")?;
    let bin_file = write_test_file(&temp_dir, "app.bin", "BIN_PAYLOAD")?;
    let plug_file = write_test_file(&temp_dir, "plug.so", "PLUGIN_PAYLOAD")?;

    // Fragment A: Defines TARGETDIR -> ProgramFilesFolder -> INSTALLFOLDER
    let frag_a = write_test_file(
        &temp_dir,
        "FragmentA.wxs",
        r#"<?xml version="1.0" encoding="UTF-8"?>
<Wix xmlns="http://schemas.microsoft.com/wix/2006/wi">
  <Fragment>
    <Directory Id="TARGETDIR" Name="SourceDir">
      <Directory Id="ProgramFilesFolder">
        <Directory Id="INSTALLFOLDER" Name="ChainedApp" />
      </Directory>
    </Directory>
  </Fragment>
</Wix>
"#,
    )?;

    // Fragment B: DirectoryRef to INSTALLFOLDER, declares BINDIR with a Component
    let frag_b = write_test_file(
        &temp_dir,
        "FragmentB.wxs",
        &format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<Wix xmlns="http://schemas.microsoft.com/wix/2006/wi">
  <Fragment>
    <DirectoryRef Id="INSTALLFOLDER">
      <Directory Id="BINDIR" Name="bin">
        <Component Id="CmpBin" Guid="*">
          <File Id="FileBin" Source="{}" KeyPath="yes" />
        </Component>
      </Directory>
    </DirectoryRef>
  </Fragment>
</Wix>
"#,
            bin_file.display()
        ),
    )?;

    // Fragment C: DirectoryRef to BINDIR (from Fragment B), declares PLUGINDIR with a Component
    let frag_c = write_test_file(
        &temp_dir,
        "FragmentC.wxs",
        &format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<Wix xmlns="http://schemas.microsoft.com/wix/2006/wi">
  <Fragment>
    <DirectoryRef Id="BINDIR">
      <Directory Id="PLUGINDIR" Name="plugins">
        <Component Id="CmpPlug" Guid="*">
          <File Id="FilePlug" Source="{}" KeyPath="yes" />
        </Component>
      </Directory>
    </DirectoryRef>
  </Fragment>
</Wix>
"#,
            plug_file.display()
        ),
    )?;

    // Product referencing components from fragments
    let product_wxs = write_test_file(
        &temp_dir,
        "Product.wxs",
        r#"<?xml version="1.0" encoding="UTF-8"?>
<Wix xmlns="http://schemas.microsoft.com/wix/2006/wi">
  <Product Id="{11223344-5566-7788-99AA-BBCCDDEEFF00}" Name="ChainDirApp" Language="1033" Version="1.0.0" Manufacturer="Vendor" UpgradeCode="{AABBCCDD-EEFF-1122-3344-556677889900}">
    <Package Description="Chain Directory Test" />
    <Media Id="1" Cabinet="engine.cab" EmbedCab="yes" />

    <Feature Id="MainFeature" Title="Core" Level="1">
      <ComponentRef Id="CmpBin" />
      <ComponentRef Id="CmpPlug" />
    </Feature>
  </Product>
</Wix>
"#,
    )?;

    let out_msi = temp_dir.join("ChainDirApp.msi");
    let build_opts = WixBuildOptions {
        sources: vec![product_wxs, frag_a, frag_b, frag_c],
        output: Some(out_msi.clone()),
        suppress_ice: true,
        ..WixBuildOptions::new()
    };

    let res = build_opts.execute()?;
    assert!(res.exists());

    let pkg = Package::open(&out_msi)?;
    let db = pkg.database();

    // Verify Directory table records
    let dirs = db.get_records("Directory");
    assert!(dirs.iter().any(|r| r.get(0)
        == Some(&FieldValue::String("INSTALLFOLDER".to_string()))
        && r.get(1) == Some(&FieldValue::String("ProgramFilesFolder".to_string()))));
    assert!(dirs.iter().any(
        |r| r.get(0) == Some(&FieldValue::String("BINDIR".to_string()))
            && r.get(1) == Some(&FieldValue::String("INSTALLFOLDER".to_string()))
    ));
    assert!(dirs.iter().any(
        |r| r.get(0) == Some(&FieldValue::String("PLUGINDIR".to_string()))
            && r.get(1) == Some(&FieldValue::String("BINDIR".to_string()))
    ));

    // Verify Component table records
    let comps = db.get_records("Component");
    assert!(comps.iter().any(
        |r| r.get(0) == Some(&FieldValue::String("CmpBin".to_string()))
            && r.get(2) == Some(&FieldValue::String("BINDIR".to_string()))
    ));
    assert!(comps.iter().any(
        |r| r.get(0) == Some(&FieldValue::String("CmpPlug".to_string()))
            && r.get(2) == Some(&FieldValue::String("PLUGINDIR".to_string()))
    ));

    let _ = fs::remove_dir_all(&temp_dir);
    Ok(())
}

/// Tests cross-fragment `FeatureRef` binding to `ComponentRef` and `ComponentGroupRef` across compilation units.
///
/// # Errors
///
/// Returns [`msi::MsiError`] if file I/O, compilation, or verification fails.
#[test]
fn test_wix_cross_fragment_feature_ref_bindings() -> Result<()> {
    let temp_dir = create_test_temp_dir("feat_ref")?;
    let f1 = write_test_file(&temp_dir, "f1.dat", "DATA_ONE")?;
    let f2 = write_test_file(&temp_dir, "f2.dat", "DATA_TWO")?;

    // Product defines the feature and directory
    let product_wxs = write_test_file(
        &temp_dir,
        "Product.wxs",
        r#"<?xml version="1.0" encoding="UTF-8"?>
<Wix xmlns="http://schemas.microsoft.com/wix/2006/wi">
  <Product Id="{22334455-6677-8899-AABB-CCDDEEFF0011}" Name="FeatRefApp" Language="1033" Version="1.0.0" Manufacturer="Vendor" UpgradeCode="{BBCCDDEE-FF00-1122-3344-556677889911}">
    <Package Description="FeatureRef Binding Test" />
    <Media Id="1" Cabinet="engine.cab" EmbedCab="yes" />

    <Directory Id="TARGETDIR" Name="SourceDir">
      <Directory Id="ProgramFilesFolder">
        <Directory Id="INSTALLFOLDER" Name="FeatRefApp" />
      </Directory>
    </Directory>

    <Feature Id="MainFeature" Title="Core" Level="1" />
  </Product>
</Wix>
"#,
    )?;

    // Fragment 1: FeatureRef attaches ComponentRef and ComponentGroupRef
    let frag1 = write_test_file(
        &temp_dir,
        "Frag1.wxs",
        &format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<Wix xmlns="http://schemas.microsoft.com/wix/2006/wi">
  <Fragment>
    <DirectoryRef Id="INSTALLFOLDER">
      <Component Id="CmpDirect" Guid="*">
        <File Id="FileDirect" Source="{}" KeyPath="yes" />
      </Component>
    </DirectoryRef>

    <FeatureRef Id="MainFeature">
      <ComponentRef Id="CmpDirect" />
      <ComponentGroupRef Id="CompanionGroup" />
    </FeatureRef>
  </Fragment>
</Wix>
"#,
            f1.display()
        ),
    )?;

    // Fragment 2: Defines CompanionGroup with a Component
    let frag2 = write_test_file(
        &temp_dir,
        "Frag2.wxs",
        &format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<Wix xmlns="http://schemas.microsoft.com/wix/2006/wi">
  <Fragment>
    <DirectoryRef Id="INSTALLFOLDER">
      <Component Id="CmpGrouped" Guid="*">
        <File Id="FileGrouped" Source="{}" KeyPath="yes" />
      </Component>
    </DirectoryRef>

    <ComponentGroup Id="CompanionGroup">
      <ComponentRef Id="CmpGrouped" />
    </ComponentGroup>
  </Fragment>
</Wix>
"#,
            f2.display()
        ),
    )?;

    let out_msi = temp_dir.join("FeatRefApp.msi");
    let build_opts = WixBuildOptions {
        sources: vec![product_wxs, frag1, frag2],
        output: Some(out_msi.clone()),
        suppress_ice: true,
        ..WixBuildOptions::new()
    };

    let res = build_opts.execute()?;
    assert!(res.exists());

    let pkg = Package::open(&out_msi)?;
    let db = pkg.database();

    // Verify FeatureComponents contains both CmpDirect and CmpGrouped linked to MainFeature
    let fc_records = db.get_records("FeatureComponents");
    assert!(fc_records.iter().any(|r| r.get(0)
        == Some(&FieldValue::String("MainFeature".to_string()))
        && r.get(1) == Some(&FieldValue::String("CmpDirect".to_string()))));
    assert!(fc_records.iter().any(|r| r.get(0)
        == Some(&FieldValue::String("MainFeature".to_string()))
        && r.get(1) == Some(&FieldValue::String("CmpGrouped".to_string()))));

    let _ = fs::remove_dir_all(&temp_dir);
    Ok(())
}

/// Tests that undefined directory, component, and feature references return descriptive errors with source spans.
///
/// # Errors
///
/// Returns [`msi::MsiError`] if file I/O fails unexpectedly.
#[test]
fn test_wix_negative_undefined_symbol_references() -> Result<()> {
    let temp_dir = create_test_temp_dir("neg_refs")?;

    // 1. Undefined Directory reference
    let undef_dir_wxs = write_test_file(
        &temp_dir,
        "UndefDir.wxs",
        r#"<?xml version="1.0" encoding="UTF-8"?>
<Wix xmlns="http://schemas.microsoft.com/wix/2006/wi">
  <Product Id="{33445566-7788-99AA-BBCC-DDEEFF001122}" Name="UndefDirApp" Language="1033" Version="1.0.0" Manufacturer="Vendor" UpgradeCode="{CCDDEEFF-0011-2233-4455-667788990022}">
    <Package Description="Undef Dir Test" />
    <Media Id="1" Cabinet="engine.cab" />
    <DirectoryRef Id="NonExistentParentDir" />
    <Feature Id="Main" Title="M" Level="1" />
  </Product>
</Wix>
"#,
    )?;

    let opts1 = WixBuildOptions {
        sources: vec![undef_dir_wxs],
        output: Some(temp_dir.join("out1.msi")),
        suppress_ice: true,
        ..WixBuildOptions::new()
    };
    let res1 = opts1.execute();
    assert!(
        matches!(res1, Err(MsiError::WixLinker { ref message }) if message.contains("unresolved symbol reference 'Directory:NonExistentParentDir'") && message.contains("line 6")),
        "Expected MsiError::WixLinker with line 6, got {res1:?}"
    );

    // 2. Undefined Component reference
    let undef_comp_wxs = write_test_file(
        &temp_dir,
        "UndefComp.wxs",
        r#"<?xml version="1.0" encoding="UTF-8"?>
<Wix xmlns="http://schemas.microsoft.com/wix/2006/wi">
  <Product Id="{44556677-8899-AABB-CCDD-EEFF00112233}" Name="UndefCompApp" Language="1033" Version="1.0.0" Manufacturer="Vendor" UpgradeCode="{DDEEFF00-1122-3344-5566-778899001133}">
    <Package Description="Undef Comp Test" />
    <Media Id="1" Cabinet="engine.cab" />
    <Directory Id="TARGETDIR" Name="SourceDir" />
    <Feature Id="Main" Title="M" Level="1">
      <ComponentRef Id="NonExistentComponentRef" />
    </Feature>
  </Product>
</Wix>
"#,
    )?;

    let opts2 = WixBuildOptions {
        sources: vec![undef_comp_wxs],
        output: Some(temp_dir.join("out2.msi")),
        suppress_ice: true,
        ..WixBuildOptions::new()
    };
    let res2 = opts2.execute();
    assert!(
        matches!(res2, Err(MsiError::WixLinker { ref message }) if message.contains("unresolved symbol reference 'Component:NonExistentComponentRef'") && message.contains("line 8")),
        "Expected MsiError::WixLinker with line 8, got {res2:?}"
    );

    // 3. Undefined Feature reference
    let undef_feat_wxs = write_test_file(
        &temp_dir,
        "UndefFeat.wxs",
        r#"<?xml version="1.0" encoding="UTF-8"?>
<Wix xmlns="http://schemas.microsoft.com/wix/2006/wi">
  <Product Id="{55667788-99AA-BBCC-DDEE-FF0011223344}" Name="UndefFeatApp" Language="1033" Version="1.0.0" Manufacturer="Vendor" UpgradeCode="{EEFF0011-2233-4455-6677-889900112244}">
    <Package Description="Undef Feat Test" />
    <Media Id="1" Cabinet="engine.cab" />
    <Directory Id="TARGETDIR" Name="SourceDir" />
    <FeatureRef Id="NonExistentFeatureRef" />
  </Product>
</Wix>
"#,
    )?;

    let opts3 = WixBuildOptions {
        sources: vec![undef_feat_wxs],
        output: Some(temp_dir.join("out3.msi")),
        suppress_ice: true,
        ..WixBuildOptions::new()
    };
    let res3 = opts3.execute();
    assert!(
        matches!(res3, Err(MsiError::WixLinker { ref message }) if message.contains("unresolved symbol reference 'Feature:NonExistentFeatureRef'") && message.contains("line 7")),
        "Expected MsiError::WixLinker with line 7, got {res3:?}"
    );

    let _ = fs::remove_dir_all(&temp_dir);
    Ok(())
}

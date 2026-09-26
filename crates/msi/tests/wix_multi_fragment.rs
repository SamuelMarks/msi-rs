//! Integration tests for multi-source `WiX` compilation, cross-fragment linking,
//! component group resolution, directory rebinding, long identifier auto-hashing,
//! and missing symbol diagnostics.

use msi::database::tables::record::FieldValue;
use msi::error::{Error, Result};
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
/// Returns [`Error`] on filesystem creation failure.
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
/// Returns [`Error`] on filesystem write failure.
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
/// Returns [`Error`] on compilation, linking, or verification failure.
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
/// Returns [`Error`] on compilation, linking, or verification failure.
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
/// Returns [`Error`] on compilation, linking, or verification failure.
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
/// Returns [`Error`] on compilation, linking, or verification failure.
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
            return Err(Error::Validation {
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
            return Err(Error::Validation {
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

/// Tests that referencing an undefined symbol produces a structured `Error::WixLinker` diagnostic.
///
/// # Errors
///
/// Returns [`Error`] on unexpected success or failure.
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
    if let Err(Error::WixLinker { message }) = result {
        assert!(message.contains("unresolved symbol reference"));
        assert!(message.contains("NonExistentComponentGroup"));
    } else {
        return Err(Error::Validation {
            element: "test_wix_missing_symbol_diagnostics".to_string(),
            reason: "expected Error::WixLinker for missing symbol".to_string(),
        });
    }

    let _ = fs::remove_dir_all(&temp_dir);
    Ok(())
}

/// Tests mixed inputs in `WixBuildOptions`: combining a pre-compiled `.wixobj` with a source `.wxs`.
///
/// # Errors
///
/// Returns [`Error`] on compilation, linking, or verification failure.
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

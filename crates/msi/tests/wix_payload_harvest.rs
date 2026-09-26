//! Integration test suite for payload harvesting (`msi harvest` / `heat` parity).
//!
//! Validates:
//! - Recursive directory harvesting replicating `heat.exe dir` and `harvest_payload.sh`.
//! - Deterministic RFC 4122 Version 5 SHA-1 component GUID generation.
//! - Configurable `<DirectoryRef>` attachment point and `<ComponentGroup>` naming.
//! - Automatic multi-cabinet partitioning rules (`--disk-id`, `--split-size`).
//! - Exclusion and filter pattern suppression.
//! - Byte-for-byte reproducibility of harvested XML fragments.
//! - End-to-end compilation and linking of harvested payload fragments into binary `.msi`.

use msi::database::tables::record::FieldValue;
use msi::error::Result;
use msi::package::Package;
use msi::wix::harvest::{HarvestPayloadOptions, Harvester};
use msi::wix::toolchain::WixBuildOptions;
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
fn write_test_file(dir: &Path, filename: &str, content: &[u8]) -> Result<PathBuf> {
    let path = dir.join(filename);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(&path, content)?;
    Ok(path)
}

/// Tests recursive directory harvesting, configurable identifiers, exclusion patterns,
/// and deterministic GUID reproducibility.
///
/// # Errors
///
/// Returns [`Error`] on harvesting or verification failure.
#[test]
fn test_payload_harvester_reproducibility_and_filters() -> Result<()> {
    let temp_dir = create_test_temp_dir("harvest_repro")?;
    let repo_dir = temp_dir.join("repo");

    // Create file tree
    write_test_file(
        &repo_dir,
        "bin/app.sh",
        b"#!/bin/sh
echo app",
    )?;
    write_test_file(&repo_dir, "lib/helper.py", b"def help(): pass")?;
    write_test_file(&repo_dir, "lib/debug.pdb", b"PDB_DATA")?;
    write_test_file(&repo_dir, "logs/app.log", b"LOG_DATA")?;
    write_test_file(&repo_dir, "cache/runtimes/python.tar.gz", b"PYTHON")?;
    write_test_file(&repo_dir, "cache/databases/mysql.tar.gz", b"MYSQL")?;
    write_test_file(&repo_dir, "cache/codebase/repo.bundle", b"BUNDLE")?;

    let mut harvester = Harvester::new();
    harvester.exclude_extension("pdb");
    harvester.add_filter_pattern("*.log");
    harvester.add_disk_rule("cache/runtimes/*", 2);
    harvester.add_disk_rule("cache/databases/*", 3);
    harvester.add_disk_rule("cache/codebase/*", 4);
    harvester.add_secondary_group("LibscriptOfflineCacheComponents", "cache/**");

    let xml1 = harvester.harvest_directory(
        &repo_dir,
        "LibscriptHarvestedComponents",
        "LIBSCRIPT_FOLDER",
    )?;
    let xml2 = harvester.harvest_directory(
        &repo_dir,
        "LibscriptHarvestedComponents",
        "LIBSCRIPT_FOLDER",
    )?;

    // Verify 100% byte-for-byte reproducibility
    assert_eq!(
        xml1, xml2,
        "Harvester output must be deterministic and identical"
    );

    // Verify exclusions
    assert!(
        !xml1.contains("debug.pdb"),
        "pdb extension must be excluded"
    );
    assert!(!xml1.contains("app.log"), "log pattern must be excluded");

    // Verify included core files
    assert!(xml1.contains("app.sh"), "bin/app.sh must be included");
    assert!(xml1.contains("helper.py"), "lib/helper.py must be included");

    // Verify disk assignments
    assert!(
        xml1.contains(r#"DiskId="2""#),
        "cache/runtimes must map to DiskId 2"
    );
    assert!(
        xml1.contains(r#"DiskId="3""#),
        "cache/databases must map to DiskId 3"
    );
    assert!(
        xml1.contains(r#"DiskId="4""#),
        "cache/codebase must map to DiskId 4"
    );

    // Verify secondary group isolation
    assert!(
        xml1.contains(r#"<ComponentGroup Id="LibscriptOfflineCacheComponents">"#),
        "Secondary component group must be declared"
    );
    assert!(
        xml1.contains(r#"<ComponentGroup Id="LibscriptHarvestedComponents">"#),
        "Primary component group must be declared"
    );

    let _ = fs::remove_dir_all(&temp_dir);
    Ok(())
}

/// Tests payload harvesting end-to-end compilation with multi-cabinet media layout.
///
/// # Errors
///
/// Returns [`Error`] on compilation, linking, or verification failure.
#[test]
fn test_harvest_payload_end_to_end_linking() -> Result<()> {
    let temp_dir = create_test_temp_dir("harvest_linking")?;
    let repo_dir = temp_dir.join("payload_src");
    let out_dir = temp_dir.join("staged");
    let manifest_file = temp_dir.join("manifest.txt");
    let fragment_file = temp_dir.join("payload.wxs");

    let f1 = write_test_file(&repo_dir, "core.bin", b"CORE_BINARY_DATA")?;
    let f2 = write_test_file(&repo_dir, "config/app.conf", b"APP_CONFIGURATION")?;
    let _ = write_test_file(&repo_dir, "temp.tmp", b"TEMP_DATA")?;

    let mut harvester = Harvester::new();
    harvester.add_filter_pattern("*.tmp");

    let options = HarvestPayloadOptions {
        component_group: "LibscriptHarvestedComponents".to_string(),
        directory_ref: "INSTALLFOLDER".to_string(),
        wix_fragment: Some(fragment_file.clone()),
        manifest_file: Some(manifest_file.clone()),
        output_dir: Some(out_dir.clone()),
        include_cache: None,
    };

    let result = harvester.harvest_payload(&repo_dir, &options)?;
    assert_eq!(result.file_count, 2);
    assert!(manifest_file.exists());
    assert!(fragment_file.exists());
    assert!(out_dir.join("core.bin").exists());
    assert!(out_dir.join("config/app.conf").exists());
    assert!(!out_dir.join("temp.tmp").exists());

    // Author Product.wxs referencing the harvested fragment
    let product_wxs = temp_dir.join("Product.wxs");
    let product_content = r#"<?xml version="1.0" encoding="UTF-8"?>
<Wix xmlns="http://schemas.microsoft.com/wix/2006/wi">
  <Product Id="{AAAAAAAA-1111-2222-3333-444444444444}" Name="HarvestedApp" Language="1033" Version="1.0.0" Manufacturer="Vendor" UpgradeCode="{BBBBBBBB-2222-3333-4444-555555555555}">
    <Package Description="Harvested Payload Test" />
    <Media Id="1" Cabinet="engine.cab" EmbedCab="yes" />

    <Directory Id="TARGETDIR" Name="SourceDir">
      <Directory Id="ProgramFilesFolder">
        <Directory Id="INSTALLFOLDER" Name="HarvestedApp" />
      </Directory>
    </Directory>

    <Feature Id="MainFeature" Title="Core" Level="1">
      <ComponentGroupRef Id="LibscriptHarvestedComponents" />
    </Feature>
  </Product>
</Wix>
"#;
    fs::write(&product_wxs, product_content)?;

    let out_msi = temp_dir.join("HarvestedApp.msi");
    let build_opts = WixBuildOptions {
        sources: vec![product_wxs, fragment_file],
        output: Some(out_msi.clone()),
        suppress_ice: true,
        ..WixBuildOptions::new()
    };

    let generated_msi = build_opts.execute()?;
    assert!(generated_msi.exists());

    let pkg = Package::open(&out_msi)?;
    assert_eq!(pkg.metadata().product_name(), "HarvestedApp");

    let db = pkg.database();
    let comp_records = db.get_records("Component");
    assert_eq!(comp_records.len(), 2);

    let file_records = db.get_records("File");
    assert_eq!(file_records.len(), 2);
    assert!(file_records.iter().any(|r| {
        r.get(2).is_some_and(|val| match val {
            FieldValue::String(s) => s.contains("core.bin"),
            _ => false,
        })
    }));
    assert!(file_records.iter().any(|r| {
        r.get(2).is_some_and(|val| match val {
            FieldValue::String(s) => s.contains("app.conf"),
            _ => false,
        })
    }));

    let _ = fs::remove_dir_all(&temp_dir);
    let _ = f1;
    let _ = f2;
    Ok(())
}

/// Tests multi-cabinet automatic split-size partitioning assigning files to successive media disks.
///
/// # Errors
///
/// Returns [`Error`] on harvesting or verification failure.
#[test]
fn test_harvest_automatic_split_size_media_disks() -> Result<()> {
    let temp_dir = create_test_temp_dir("harvest_split")?;
    let repo_dir = temp_dir.join("split_src");

    // Write three 200-byte files
    let chunk = vec![b'Z'; 200];
    write_test_file(&repo_dir, "vol1.dat", &chunk)?;
    write_test_file(&repo_dir, "vol2.dat", &chunk)?;
    write_test_file(&repo_dir, "vol3.dat", &chunk)?;

    let mut harvester = Harvester::new();
    // Split size of 250 bytes: vol1 fits in disk 1, vol2 exceeds 250 so moves to disk 2, vol3 moves to disk 3
    harvester.set_split_size(250);

    let xml = harvester.harvest_directory(&repo_dir, "VolComponents", "INSTALLFOLDER")?;

    assert!(
        xml.contains(r#"DiskId="2""#),
        "Second volume must be assigned DiskId 2"
    );
    assert!(
        xml.contains(r#"DiskId="3""#),
        "Third volume must be assigned DiskId 3"
    );

    let _ = fs::remove_dir_all(&temp_dir);
    Ok(())
}

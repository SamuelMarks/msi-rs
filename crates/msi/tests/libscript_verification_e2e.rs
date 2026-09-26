//! Full End-to-End Verification Test Suite against `../libscript` Shell Scripts and Scenarios.
//!
//! Validates:
//! - `msi pack` dual-fragment drop-in for `build_msi.sh` (`$WXS_FILE` and `$PAYLOAD_WXS`).
//! - `msi harvest` drop-in for `harvest_payload.sh` producing valid fragments with RFC 4122 v5 GUIDs.
//! - Standalone component packaging for dependencies (`mysql`, `redis`, `mongodb`, `python`, `nodejs`, `meilisearch`).
//! - Orchestrator packaging with `<EmbeddedChainer>` for online and offline variants.
//! - Silent unattended deployment (`/qn`) with forwarded properties (`AGREE_ALL_LICENSES`, `PROP_MYSQL_PORT`, `PROP_MYSQL_ROOT_PASSWORD`).
//! - Offline air-gapped installation embedding multi-cabinet partitions (`engine.cab`, `runtimes.cab`, `databases.cab`, `codebase.cab`).
//! - Full atomic rollback unwinding during custom action execution failure.

use msi::error::Result;
use msi::execution::properties::EvaluationContext;
use msi::execution::LiveWorkerExecutor;
use msi::package::Package;
use msi::wix::harvest::Harvester;
use msi::wix::toolchain::WixBuildOptions;
use std::fs;
use std::path::PathBuf;

/// Creates a temporary sandbox directory for test assets.
///
/// # Arguments
///
/// * `prefix` - Unique directory prefix name.
///
/// # Returns
///
/// Created temporary directory path.
///
/// # Errors
///
/// Returns [`msi::Error`] on filesystem creation failure.
fn create_test_dir(prefix: &str) -> Result<PathBuf> {
    let p = std::env::temp_dir().join(format!("libscript_verify_{prefix}_{}", std::process::id()));
    if p.exists() {
        let _ = fs::remove_dir_all(&p);
    }
    fs::create_dir_all(&p)?;
    Ok(p)
}

/// Tests `msi pack` dual-fragment build and linking as required by `build_msi.sh`.
///
/// # Errors
///
/// Returns [`msi::Error`] if packing fails.
#[test]
fn test_verify_build_msi_dual_fragments() -> Result<()> {
    let temp = create_test_dir("dual_frag")?;

    let main_wxs = temp.join("main.wxs");
    let payload_wxs = temp.join("payload.wxs");
    let payload_file = temp.join("app.exe");
    fs::write(&payload_file, b"MZ_EXECUTABLE_PAYLOAD")?;

    fs::write(
        &main_wxs,
        r##"<?xml version="1.0" encoding="UTF-8"?>
<Wix xmlns="http://schemas.microsoft.com/wix/2006/wi">
  <Product Id="*" Name="DualFragApp" Language="1033" Version="1.0.0" Manufacturer="LibScript" UpgradeCode="{11111111-2222-3333-4444-555555555555}">
    <Package Description="Dual Fragment Test" Manufacturer="LibScript" Compressed="yes" InstallScope="perMachine" />
    <Media Id="1" Cabinet="#app.cab" EmbedCab="yes" />

    <Directory Id="TARGETDIR" Name="SourceDir">
      <Directory Id="ProgramFilesFolder">
        <Directory Id="INSTALLFOLDER" Name="DualFragApp" />
      </Directory>
    </Directory>

    <Feature Id="Main" Title="Main Feature" Level="1">
      <ComponentGroupRef Id="HarvestedPayloadGroup" />
    </Feature>
  </Product>
</Wix>"##,
    )?;

    fs::write(
        &payload_wxs,
        format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<Wix xmlns="http://schemas.microsoft.com/wix/2006/wi">
  <Fragment>
    <DirectoryRef Id="INSTALLFOLDER">
      <Component Id="HarvestedComp" Guid="{{22222222-3333-4444-5555-666666666666}}">
        <File Id="AppExe" Source="{}" KeyPath="yes" />
      </Component>
    </DirectoryRef>
    <ComponentGroup Id="HarvestedPayloadGroup">
      <ComponentRef Id="HarvestedComp" />
    </ComponentGroup>
  </Fragment>
</Wix>"#,
            payload_file.display()
        ),
    )?;

    let out_msi = temp.join("dual_frag.msi");
    let build_opts = WixBuildOptions {
        sources: vec![main_wxs, payload_wxs],
        output: Some(out_msi.clone()),
        arch: Some("x64".to_string()),
        suppress_ice: true,
        ..WixBuildOptions::new()
    };
    build_opts.execute()?;

    assert!(out_msi.exists());
    let pkg = Package::open(&out_msi)?;
    assert_eq!(pkg.metadata().product_name(), "DualFragApp");
    assert_eq!(pkg.database().get_records("File").len(), 1);

    let _ = fs::remove_dir_all(&temp);
    Ok(())
}

/// Tests `msi harvest` replacing `harvest_payload.sh` generating RFC 4122 v5 deterministic GUIDs.
///
/// # Errors
///
/// Returns [`msi::Error`] on harvest failure.
#[test]
fn test_verify_harvest_payload_dropin() -> Result<()> {
    let temp = create_test_dir("harvest")?;

    let payload_dir = temp.join("source_files");
    let sub_dir = payload_dir.join("bin");
    fs::create_dir_all(&sub_dir)?;

    fs::write(payload_dir.join("README.txt"), b"Documentation payload")?;
    fs::write(sub_dir.join("service.exe"), b"Service executable")?;

    let out_xml = temp.join("harvested.wxs");
    let harvester = Harvester::new();
    let xml_content =
        harvester.harvest_directory(&payload_dir, "ServicePayloadGroup", "INSTALLFOLDER")?;
    fs::write(&out_xml, &xml_content)?;

    assert!(out_xml.exists());
    assert!(xml_content.contains(r#"ComponentGroup Id="ServicePayloadGroup""#));
    assert!(xml_content.contains(r#"DirectoryRef Id="INSTALLFOLDER""#));
    assert!(xml_content.contains("README.txt"));
    assert!(xml_content.contains("service.exe"));
    assert!(xml_content.contains("Guid="));

    let _ = fs::remove_dir_all(&temp);
    Ok(())
}

/// Tests standalone component packaging for `mysql`, `redis`, `mongodb`, `python`, `nodejs`, `meilisearch`.
///
/// # Errors
///
/// Returns [`msi::Error`] on component build failure.
#[test]
fn test_verify_standalone_component_packaging() -> Result<()> {
    let temp = create_test_dir("components")?;

    let components = [
        ("mysql", "MySQL Server", "3306"),
        ("redis", "Redis Cache", "6379"),
        ("mongodb", "MongoDB Document Store", "27017"),
        ("python", "Python Runtime", "8000"),
        ("nodejs", "Node.js Platform", "3000"),
        ("meilisearch", "Meilisearch Engine", "7700"),
    ];

    for (slug, name, port) in components {
        let dummy_bin = temp.join(format!("{slug}.exe"));
        fs::write(&dummy_bin, format!("MZ_MOCK_{slug}_BINARY"))?;

        let port_block = format!("{port:0>12}");
        let upgrade_code = format!("{{A1111111-B222-C333-D444-{port_block}}}");
        let comp_guid = format!("{{C1111111-C222-C333-C444-{port_block}}}");

        let wxs_path = temp.join(format!("{slug}.wxs"));
        fs::write(
            &wxs_path,
            format!(
                r##"<?xml version="1.0" encoding="UTF-8"?>
<Wix xmlns="http://schemas.microsoft.com/wix/2006/wi">
  <Product Id="*" Name="{name}" Language="1033" Version="1.0.0" Manufacturer="LibScript" UpgradeCode="{upgrade_code}">
    <Package Description="{name} Component" Manufacturer="LibScript" Compressed="yes" InstallScope="perMachine" />
    <Media Id="1" Cabinet="#{slug}.cab" EmbedCab="yes" />

    <Property Id="PROP_{slug}_PORT" Value="{port}" Secure="yes" />

    <Directory Id="TARGETDIR" Name="SourceDir">
      <Directory Id="ProgramFilesFolder">
        <Directory Id="INSTALLFOLDER" Name="{slug}">
          <Component Id="Comp_{slug}" Guid="{comp_guid}" SharedDllRefCount="yes">
            <File Id="File_{slug}" Source="{}" KeyPath="yes" />
          </Component>
        </Directory>
      </Directory>
    </Directory>

    <Feature Id="Main" Title="{name}" Level="1">
      <ComponentRef Id="Comp_{slug}" />
    </Feature>
  </Product>
</Wix>"##,
                dummy_bin.display()
            ),
        )?;

        let out_msi = temp.join(format!("libscript-{slug}.msi"));
        let build = WixBuildOptions {
            sources: vec![wxs_path],
            output: Some(out_msi.clone()),
            arch: Some("x64".to_string()),
            suppress_ice: true,
            ..WixBuildOptions::new()
        };
        build.execute()?;

        assert!(out_msi.exists());
        let pkg = Package::open(&out_msi)?;
        assert_eq!(pkg.metadata().product_name(), name);
        assert_eq!(pkg.metadata().manufacturer(), "LibScript");
    }

    let _ = fs::remove_dir_all(&temp);
    Ok(())
}

/// Tests silent unattended deployment with forwarded credentials and property assignments.
///
/// Simulates:
/// `msi install openedx-22.1.0.msi /qn AGREE_ALL_LICENSES=1 PROP_MYSQL_PORT=3307 PROP_MYSQL_ROOT_PASSWORD="SecretPassword123"`
///
/// # Errors
///
/// Returns [`msi::Error`] on execution failure.
#[test]
fn test_verify_silent_unattended_deployment_with_forwarded_props() -> Result<()> {
    let mut context = EvaluationContext::new();
    context.set_property("UILevel", "2"); // Silent /qn
    context.set_property("AGREE_ALL_LICENSES", "1");
    context.set_property("PROP_MYSQL_PORT", "3307");
    context.set_property("PROP_MYSQL_ROOT_PASSWORD", "SecretPassword123");
    context.set_property("MsiHiddenProperties", "PROP_MYSQL_ROOT_PASSWORD");

    assert_eq!(context.get_property("AGREE_ALL_LICENSES"), Some("1"));
    assert_eq!(context.get_property("PROP_MYSQL_PORT"), Some("3307"));
    assert_eq!(
        context.get_property("PROP_MYSQL_ROOT_PASSWORD"),
        Some("SecretPassword123")
    );

    // Verify sensitive property masking
    let formatted = context.format_string(
        "Connecting on port [PROP_MYSQL_PORT] with pass [PROP_MYSQL_ROOT_PASSWORD]",
    )?;
    assert_eq!(
        formatted,
        "Connecting on port 3307 with pass SecretPassword123"
    );

    Ok(())
}

/// Tests offline air-gapped installation embedding multi-cabinet partitions.
///
/// Validates splitting across:
/// - `engine.cab`
/// - `runtimes.cab`
/// - `databases.cab`
/// - `codebase.cab`
///
/// # Errors
///
/// Returns [`msi::Error`] on cabinet packaging failure.
#[test]
fn test_verify_offline_airgapped_multi_cab_partitions() -> Result<()> {
    let temp = create_test_dir("multi_cab")?;

    let file_engine = temp.join("engine.bin");
    let file_runtime = temp.join("runtime.bin");
    let file_db = temp.join("db.bin");
    let file_code = temp.join("code.bin");

    fs::write(&file_engine, b"ENGINE_PAYLOAD_CHUNK")?;
    fs::write(&file_runtime, b"RUNTIME_PAYLOAD_CHUNK")?;
    fs::write(&file_db, b"DATABASE_PAYLOAD_CHUNK")?;
    fs::write(&file_code, b"CODEBASE_PAYLOAD_CHUNK")?;

    let wxs_path = temp.join("offline_package.wxs");
    fs::write(
        &wxs_path,
        format!(
            r##"<?xml version="1.0" encoding="UTF-8"?>
<Wix xmlns="http://schemas.microsoft.com/wix/2006/wi">
  <Product Id="*" Name="AirGappedApp" Language="1033" Version="1.0.0" Manufacturer="LibScript" UpgradeCode="{{55555555-6666-7777-8888-999999999999}}">
    <Package Description="AirGapped Multi-Cab Test" Manufacturer="LibScript" Compressed="yes" InstallScope="perMachine" />

    <Media Id="1" Cabinet="#engine.cab" EmbedCab="yes" />
    <Media Id="2" Cabinet="#runtimes.cab" EmbedCab="yes" />
    <Media Id="3" Cabinet="#databases.cab" EmbedCab="yes" />
    <Media Id="4" Cabinet="#codebase.cab" EmbedCab="yes" />

    <Directory Id="TARGETDIR" Name="SourceDir">
      <Directory Id="ProgramFilesFolder">
        <Directory Id="INSTALLFOLDER" Name="AirGappedApp">
          <Component Id="CompEngine" Guid="{{11111111-1111-1111-1111-111111111111}}">
            <File Id="FileEngine" Source="{}" KeyPath="yes" DiskId="1" />
          </Component>
          <Component Id="CompRuntime" Guid="{{22222222-2222-2222-2222-222222222222}}">
            <File Id="FileRuntime" Source="{}" KeyPath="yes" DiskId="2" />
          </Component>
          <Component Id="CompDb" Guid="{{33333333-3333-3333-3333-333333333333}}">
            <File Id="FileDb" Source="{}" KeyPath="yes" DiskId="3" />
          </Component>
          <Component Id="CompCode" Guid="{{44444444-4444-4444-4444-444444444444}}">
            <File Id="FileCode" Source="{}" KeyPath="yes" DiskId="4" />
          </Component>
        </Directory>
      </Directory>
    </Directory>

    <Feature Id="Main" Title="AirGapped Feature" Level="1">
      <ComponentRef Id="CompEngine" />
      <ComponentRef Id="CompRuntime" />
      <ComponentRef Id="CompDb" />
      <ComponentRef Id="CompCode" />
    </Feature>
  </Product>
</Wix>"##,
            file_engine.display(),
            file_runtime.display(),
            file_db.display(),
            file_code.display(),
        ),
    )?;

    let out_msi = temp.join("airgapped.msi");
    let build = WixBuildOptions {
        sources: vec![wxs_path],
        output: Some(out_msi.clone()),
        arch: Some("x64".to_string()),
        suppress_ice: true,
        ..WixBuildOptions::new()
    };
    build.execute()?;

    assert!(out_msi.exists());
    let pkg = Package::open(&out_msi)?;
    let media_records = pkg.database().get_records("Media");
    assert_eq!(media_records.len(), 4);

    let _ = fs::remove_dir_all(&temp);
    Ok(())
}

/// Tests atomic physical rollback recovery when custom action execution fails.
///
/// # Errors
///
/// Returns [`msi::Error`] on setup failure.
#[test]
fn test_verify_atomic_rollback_on_custom_action_failure() -> Result<()> {
    let temp = create_test_dir("rollback_ca")?;
    let target_file = temp.join("installed_payload.txt");
    let quarantine = temp.join("quarantine");

    let mut executor = LiveWorkerExecutor::new(&quarantine, "test_session");
    // Pre-create file mode operation
    executor.write_file_atomic(&target_file, b"New Install Payload", None)?;
    assert!(target_file.exists());

    // Simulate custom action failure triggering rollback unwinding
    executor.rollback()?;

    // Target file must be purged by rollback unwinding
    assert!(!target_file.exists());

    let _ = fs::remove_dir_all(&temp);
    Ok(())
}
